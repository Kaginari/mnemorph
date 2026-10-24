//! System One: typed, fast decisions with a probability over every option.
//!
//! One wire serves TypeSafe's Jev and the open jeff, which speaks the same protocol on your own
//! machine: `POST /v1/systemone` with a state and named questions, each a yes-or-no (`noul`), a
//! choice between labelled options, or a level on an ordered scale. The answer carries a
//! probability for every option, so a caller can act on a decision only when it is sure enough.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use mnemorph_ledger::Usage;
use serde_json::Value;
use tokio::time::Instant;

use crate::drive::api_error;
use crate::error::{Error, Wait};
use crate::retry::{Policy, retry_after};
use crate::transport::{HttpRequest, Transport};

/// The largest answer body read; a decision is a few hundred bytes.
const MAX_BODY: usize = 1024 * 1024;

/// One question.
#[derive(Debug, Clone, PartialEq)]
pub enum Question {
    /// Yes or no: the chance that `instructions`, a statement about the state, holds.
    Noul {
        /// The statement.
        instructions: String,
    },
    /// One of the options, each a label and what it means, in order.
    Choice {
        /// What to decide.
        instructions: String,
        /// The labels and their descriptions. Order matters: a model may read options by
        /// position.
        options: Vec<(String, String)>,
    },
    /// One level of an ordered scale, lowest first.
    Score {
        /// What to rate.
        instructions: String,
        /// Each level's description, lowest first.
        levels: Vec<String>,
    },
}

impl Question {
    /// The labels an answer gives probabilities to, in order: `false` and `true` for a noul,
    /// the options for a choice, `0` … `n−1` for a score.
    #[must_use]
    pub fn labels(&self) -> Vec<String> {
        match self {
            Question::Noul { .. } => vec!["false".into(), "true".into()],
            Question::Choice { options, .. } => options.iter().map(|(l, _)| l.clone()).collect(),
            Question::Score { levels, .. } => (0..levels.len()).map(|i| i.to_string()).collect(),
        }
    }

    fn check(&self, name: &str) -> Result<(), Error> {
        let bad = |why: &str| Err(Error::Invalid(format!("question `{name}`: {why}")));
        match self {
            Question::Noul { instructions } if instructions.trim().is_empty() => {
                bad("a yes-or-no question needs its statement")
            }
            Question::Choice { options, .. } if options.len() < 2 => {
                bad("a choice needs two options")
            }
            Question::Choice { options, .. } => {
                let mut seen = std::collections::BTreeSet::new();
                for (label, _) in options {
                    if label.is_empty() || label.parse::<f64>().is_ok() {
                        return bad("a label is a word, never empty and never a bare number");
                    }
                    if !seen.insert(label) {
                        return bad("two options share a label");
                    }
                }
                Ok(())
            }
            Question::Score { levels, .. } if !(2..=10).contains(&levels.len()) => {
                bad("a score has two to ten levels")
            }
            _ => Ok(()),
        }
    }
}

/// What to decide about: the state, and the questions by name, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    /// The state the questions are about, as text.
    pub state: String,
    /// The questions, each under a name the answer echoes.
    pub questions: Vec<(String, Question)>,
}

/// The most a decision's state may hold, in bytes. A decision point sends the part it needs (the
/// tail of a test log, the head of a commission); past this, the System One models refuse it, and
/// the fallback would pay full price for all of it.
pub const MAX_STATE_BYTES: usize = 24_000;

impl Ask {
    pub(crate) fn check(&self) -> Result<(), Error> {
        if self.state.len() > MAX_STATE_BYTES {
            return Err(Error::Invalid(format!(
                "a decision's state is {} bytes; at most {MAX_STATE_BYTES} — send the part that decides",
                self.state.len()
            )));
        }
        if self.questions.is_empty() {
            return Err(Error::Invalid(
                "a decision needs at least one question".into(),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (name, q) in &self.questions {
            if !seen.insert(name) {
                return Err(Error::Invalid(format!("two questions are named `{name}`")));
            }
            q.check(name)?;
        }
        Ok(())
    }
}

/// One answer: a probability for every label of its question, in the question's order, summing
/// to 1.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Answer {
    /// The labels, as [`Question::labels`] gives them.
    pub labels: Vec<String>,
    /// Their probabilities.
    pub probabilities: Vec<f64>,
    /// The labels are ordered levels (a score), so a near miss is closer than a far one.
    pub ordered: bool,
    /// The confidence the provider measured and sent, when it sent one (choice and score).
    pub measured: Option<f64>,
}

impl Answer {
    /// The most likely label (the first, on a tie); empty for an answer with no labels.
    #[must_use]
    pub fn choice(&self) -> &str {
        self.labels.get(self.top()).map_or("", String::as_str)
    }

    /// The index of the most likely label, the first on a tie.
    fn top(&self) -> usize {
        let mut best = 0;
        for (i, p) in self.probabilities.iter().enumerate() {
            if *p > self.probabilities[best] {
                best = i;
            }
        }
        best
    }

    /// The probability of one label; 0 for a label the question did not have.
    #[must_use]
    pub fn probability(&self, label: &str) -> f64 {
        self.labels
            .iter()
            .position(|l| l == label)
            .map_or(0.0, |i| self.probabilities[i])
    }

    /// How far the answer is from a coin toss, 0 for an even spread, 1 for a certain answer — the
    /// provider's own measure when it sent one, else TypeSafe's definitions: for a choice
    /// `(max p − 1/n) / (1 − 1/n)` (for a yes-or-no, `|2p − 1|`); for a score, adjacency-aware,
    /// `max(0, 1 − Σ pᵢ·|i − m| / MAD)`, where `m` is the most likely level and MAD the mean
    /// distance of an even spread from its middle — so an even split between two neighbouring
    /// levels is surer than one between the ends. An answer with no label, or with a probability
    /// that is not a number, reads 0.
    #[must_use]
    pub fn confidence(&self) -> f64 {
        if let Some(c) = self.measured.filter(|c| (0.0..=1.0).contains(c)) {
            return c;
        }
        let n = self.probabilities.len();
        if n == 0 || self.probabilities.iter().any(|p| !p.is_finite()) {
            return 0.0;
        }
        if n == 1 {
            return 1.0;
        }
        if self.ordered {
            let m = self.top();
            let spread: f64 = self
                .probabilities
                .iter()
                .enumerate()
                .map(|(i, p)| p * count(i.abs_diff(m)))
                .sum();
            let middle = (count(n) - 1.0) / 2.0;
            let mad = (0..n).map(|i| (count(i) - middle).abs()).sum::<f64>() / count(n);
            return (1.0 - spread / mad).clamp(0.0, 1.0);
        }
        let max = self.probabilities.iter().copied().fold(0.0, f64::max);
        let floor = 1.0 / count(n);
        ((max - floor) / (1.0 - floor)).clamp(0.0, 1.0)
    }

    /// A score's expected level, `Σ i·pᵢ`; for any other question the index of [`Self::choice`].
    #[must_use]
    pub fn expected_level(&self) -> f64 {
        self.probabilities
            .iter()
            .enumerate()
            .map(|(i, p)| count(i) * p)
            .sum()
    }

    /// An answer from probabilities by label, in `labels`' order. A missing label reads 0;
    /// values are clamped to [0, 1] and rescaled to sum to 1; an answer with nothing left reads
    /// uniform. `None` when a value is not a number. For an LLM's stated answer, which may be
    /// sloppy; a provider's answer is read [`Self::exact`]ly.
    pub(crate) fn from_map(
        labels: Vec<String>,
        map: &serde_json::Map<String, Value>,
    ) -> Option<Answer> {
        let mut probabilities = Vec::with_capacity(labels.len());
        for l in &labels {
            probabilities.push(match map.get(l) {
                None => 0.0,
                Some(v) => v.as_f64()?.clamp(0.0, 1.0),
            });
        }
        Some(Answer::normalised(labels, probabilities))
    }

    /// A provider's answer, read strictly: exactly the asked labels, each a probability, summing
    /// to 1 within rounding. A System One server always returns the full distribution, so any
    /// other shape is a protocol fault, never noise to repair. `None` when it does not fit.
    pub(crate) fn exact(
        labels: Vec<String>,
        map: &serde_json::Map<String, Value>,
    ) -> Option<Answer> {
        if map.len() != labels.len() {
            return None;
        }
        let mut probabilities = Vec::with_capacity(labels.len());
        for l in &labels {
            let p = map.get(l)?.as_f64()?;
            if !(0.0..=1.0).contains(&p) {
                return None;
            }
            probabilities.push(p);
        }
        let sum: f64 = probabilities.iter().sum();
        ((sum - 1.0).abs() <= 1e-3).then(|| Answer::normalised(labels, probabilities))
    }

    pub(crate) fn normalised(labels: Vec<String>, mut probabilities: Vec<f64>) -> Answer {
        let sum: f64 = probabilities.iter().sum();
        if sum > 0.0 && sum.is_finite() {
            for p in &mut probabilities {
                *p /= sum;
            }
        } else {
            let even = 1.0 / count(probabilities.len().max(1));
            probabilities.fill(even);
        }
        Answer {
            labels,
            probabilities,
            ordered: false,
            measured: None,
        }
    }
}

/// A small count as a float: option lists hold at most a few hundred entries.
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// What a System One provider answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    /// The model that answered, as the provider names it.
    pub model: String,
    /// The answers, in the ask's order.
    pub answers: Vec<(String, Answer)>,
    /// Tokens.
    pub usage: Usage,
    /// The provider's request id, when it sent one.
    pub request_id: Option<String>,
}

/// A client for a System One provider: TypeSafe's API, or a jeff server.
pub struct SystemOne {
    transport: Arc<dyn Transport>,
    url: String,
    key: Option<String>,
    policy: Policy,
}

impl SystemOne {
    /// A client sending to `base_url` (`https://api.typesafe.ai`, or a jeff server such as
    /// `http://127.0.0.1:8765`), with a bearer `key` when the provider needs one.
    #[must_use]
    pub fn new(
        transport: Arc<dyn Transport>,
        base_url: &str,
        key: Option<String>,
        policy: Policy,
    ) -> SystemOne {
        SystemOne {
            transport,
            url: format!("{}/v1/systemone", base_url.trim_end_matches('/')),
            key,
            policy,
        }
    }

    /// The policy a decision deserves: a decision is meant to take milliseconds, so it waits
    /// seconds, not minutes, and gives up early enough for a fallback to answer instead.
    #[must_use]
    pub fn policy() -> Policy {
        Policy {
            max_retries: 2,
            base: Duration::from_millis(500),
            cap: Duration::from_secs(5),
            connect: Duration::from_secs(5),
            first_byte: Duration::from_secs(10),
            stall: Duration::from_secs(10),
            turn: Duration::from_secs(30),
        }
    }

    /// Asks `model` to decide.
    ///
    /// # Errors
    ///
    /// The ask is invalid; every attempt failed; the answer is not one the ask could have.
    pub async fn decide(&self, model: &str, ask: &Ask) -> Result<Reply, Error> {
        ask.check()?;
        let body = encode(model, ask);
        let mut headers = vec![
            ("content-type".to_owned(), "application/json".to_owned()),
            ("accept".to_owned(), "application/json".to_owned()),
        ];
        if let Some(key) = &self.key {
            headers.push(("authorization".to_owned(), format!("Bearer {key}")));
        }
        let (bytes, request_id) = post_json(
            self.transport.as_ref(),
            &self.policy,
            &self.url,
            &headers,
            body.as_bytes(),
        )
        .await?;
        decode(ask, &bytes, request_id)
    }
}

/// The request body, keys in the order the protocol documents and options in the ask's order —
/// written by hand, since a JSON map would sort them.
#[must_use]
pub fn encode(model: &str, ask: &Ask) -> String {
    let s = |text: &str| Value::String(text.to_owned()).to_string();
    let mut out = format!(
        "{{\"model\":{},\"state\":{},\"questions\":{{",
        s(model),
        s(&ask.state)
    );
    for (i, (name, q)) in ask.questions.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&s(name));
        out.push(':');
        match q {
            Question::Noul { instructions } => {
                let _ = write!(
                    out,
                    "{{\"type\":\"noul\",\"instructions\":{}}}",
                    s(instructions)
                );
            }
            Question::Choice {
                instructions,
                options,
            } => {
                let criteria: Vec<String> = options
                    .iter()
                    .map(|(l, d)| format!("{}:{}", s(l), s(d)))
                    .collect();
                let _ = write!(
                    out,
                    "{{\"type\":\"choice\",\"instructions\":{},\"criteria\":{{{}}}}}",
                    s(instructions),
                    criteria.join(",")
                );
            }
            Question::Score {
                instructions,
                levels,
            } => {
                let criteria: Vec<String> = levels.iter().map(|l| s(l)).collect();
                let _ = write!(
                    out,
                    "{{\"type\":\"score\",\"instructions\":{},\"criteria\":[{}]}}",
                    s(instructions),
                    criteria.join(",")
                );
            }
        }
    }
    out.push_str("}}");
    out
}

/// Reads an answer body against the ask: every question answered, each with the type it was
/// asked as. Unknown fields (a server's `stats`) are ignored.
///
/// # Errors
///
/// The body is not JSON, a question is missing, or an answer has the wrong shape.
pub fn decode(ask: &Ask, body: &[u8], request_id: Option<String>) -> Result<Reply, Error> {
    let v: Value =
        serde_json::from_slice(body).map_err(|e| Error::Decode(format!("not JSON: {e}")))?;
    let answers = v["answers"]
        .as_object()
        .ok_or_else(|| Error::Decode("no `answers` object".into()))?;
    let mut out = Vec::with_capacity(ask.questions.len());
    for (name, q) in &ask.questions {
        let a = answers
            .get(name)
            .ok_or_else(|| Error::Decode(format!("no answer to `{name}`")))?;
        let wrong = || Error::Decode(format!("the answer to `{name}` has the wrong shape"));
        let answer = match q {
            Question::Noul { .. } => {
                if a["type"] != "noul" {
                    return Err(wrong());
                }
                let p = a["noul"]
                    .as_f64()
                    .filter(|p| (0.0..=1.0).contains(p))
                    .ok_or_else(wrong)?;
                Answer::normalised(q.labels(), vec![1.0 - p, p])
            }
            Question::Choice { .. } | Question::Score { .. } => {
                let kind = if matches!(q, Question::Choice { .. }) {
                    "choice"
                } else {
                    "score"
                };
                if a["type"] != kind {
                    return Err(wrong());
                }
                let map = a["probabilities"].as_object().ok_or_else(wrong)?;
                let mut answer = Answer::exact(q.labels(), map).ok_or_else(wrong)?;
                answer.ordered = kind == "score";
                answer.measured = a["confidence"].as_f64().filter(|c| (0.0..=1.0).contains(c));
                answer
            }
        };
        out.push((name.clone(), answer));
    }
    let tokens = |k: &str| v["usage"][k].as_u64().unwrap_or(0);
    Ok(Reply {
        model: v["model"].as_str().unwrap_or_default().to_owned(),
        answers: out,
        usage: Usage {
            input: tokens("input_tokens"),
            output: tokens("output_tokens"),
            cache_read: 0,
            cache_write: 0,
        },
        request_id,
    })
}

/// An error body's message, as a System One server writes it: `error` (text or `{message}`),
/// then `message`, then `detail` (text, `{message}` or a validation list), else the generic
/// reading.
fn error_message(body: &[u8]) -> (String, String) {
    let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let text = |x: &Value| {
        x.as_str()
            .map(str::to_owned)
            .or_else(|| x["message"].as_str().map(str::to_owned))
    };
    let detail = || match &v["detail"] {
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(|i| i["msg"].as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ),
        other => text(other),
    };
    let message = text(&v["error"])
        .or_else(|| text(&v["message"]))
        .or_else(detail);
    let (kind, fallback) = api_error(body);
    (kind, message.filter(|m| !m.is_empty()).unwrap_or(fallback))
}

/// Posts `body` and reads the whole JSON answer, on the provider's retry ladder: the same
/// statuses, waits and `retry-after` as a model call, and a deadline over every attempt.
async fn post_json(
    transport: &dyn Transport,
    policy: &Policy,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<(Vec<u8>, Option<String>), Error> {
    let deadline = Instant::now() + policy.turn;
    let mut attempt = 0;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let outcome = tokio::time::timeout(left, once(transport, policy, url, headers, body))
            .await
            .unwrap_or(Err((Error::Timeout(Wait::Turn), None)));
        let (error, after) = match outcome {
            Ok(answer) => return Ok(answer),
            Err(failure) => failure,
        };
        if !error.retryable() || attempt >= policy.max_retries {
            return Err(error);
        }
        attempt += 1;
        let wait = policy.wait(attempt, after);
        if Instant::now() + wait >= deadline {
            return Err(error);
        }
        tokio::time::sleep(wait).await;
    }
}

type Failure = (Error, Option<Duration>);

async fn once(
    transport: &dyn Transport,
    policy: &Policy,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<(Vec<u8>, Option<String>), Failure> {
    let request = HttpRequest {
        url: url.to_owned(),
        headers: headers.to_vec(),
        body: body.to_vec(),
    };
    let response = tokio::time::timeout(policy.first_byte, transport.send(request))
        .await
        .map_err(|_| (Error::Timeout(Wait::FirstByte), None))?
        .map_err(|e| (Error::Transport(e.to_string()), None))?;
    let after = retry_after(
        response.header("retry-after-ms"),
        response.header("retry-after"),
    );
    let request_id = response
        .header("x-typesafe-request-id")
        .or_else(|| response.header("x-request-id"))
        .map(str::to_owned);
    let status = response.status;
    let mut chunks = response.body;
    let mut text = Vec::new();
    let mut wait = (policy.first_byte, Wait::FirstByte);
    loop {
        let next = tokio::time::timeout(wait.0, chunks.next())
            .await
            .map_err(|_| (Error::Timeout(wait.1), after))?;
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|e| (Error::Transport(e.to_string()), after))?;
        wait = (policy.stall, Wait::Stall);
        text.extend_from_slice(&chunk);
        if text.len() > MAX_BODY {
            return Err((
                Error::Decode("the answer is larger than any decision".into()),
                None,
            ));
        }
    }
    if status != 200 {
        let (kind, message) = error_message(&text);
        return Err((
            Error::Api {
                status,
                kind,
                message,
            },
            after,
        ));
    }
    Ok((text, request_id))
}
