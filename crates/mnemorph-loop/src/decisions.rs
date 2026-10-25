//! The run's decision points: what the configuration's decider is asked, and what the loop does
//! with a sure answer.
//!
//! The first point is the piped bench. `cargo test | tail` hides its exit status, so the bench
//! tally counts it muted, never green. With decisions on, the run's output is read instead: a sure
//! green or red counts as if the status had been seen; an unsure answer leaves it muted, as it was.

use std::path::Path;
use std::sync::Arc;

use mnemorph_ledger::{DecisionRow, Journal, Prices, Timestamp, UsageRow};
use mnemorph_provider::{
    Ask, Caller, Complete, Completion, Cutoffs, Decider, Decision, Fallback, HttpTransport,
    Primary, Question, Request, SystemOne,
};

use crate::model::Model;
use crate::stress::Read;

/// The decision point that reads a piped bench run.
pub const BENCH_POINT: &str = "bench.outcome";

/// The most of a bench's output a decision reads: its tail, where every runner prints its summary.
const BENCH_TAIL: usize = 12_000;

/// A run's decisions: who decides, how sure they must be, and where each decision is kept.
#[derive(Clone)]
pub struct Decisions {
    /// The decider.
    pub decider: Arc<Decider>,
    /// The least confidence to act on, per basis.
    pub cutoffs: Cutoffs,
    /// The decision journal, `.mnemorph/instruments/decisions.jsonl`.
    pub journal: Journal<DecisionRow>,
    /// Whether a piped test run is read (`decide.bench`).
    pub bench: bool,
    /// Whether Metis routes commissions to a tier (`decide.route`).
    pub route: bool,
}

/// What a decision cost, as a usage row: priced like any call, `usd: null` for a model the price
/// table does not know.
#[must_use]
pub fn decision_usage(decision: &Decision, caller: &Caller, prices: &Prices) -> UsageRow {
    UsageRow {
        ts: Timestamp::now(),
        session: caller.session.clone(),
        run: caller.run.clone(),
        body: caller.body.clone(),
        race: caller.race.clone(),
        commission: caller.commission.clone(),
        model: decision.source.model.clone(),
        provider: decision.source.provider.clone(),
        usage: decision.usage,
        prefix: 0,
        wt: 0,
        usd: None,
        envelope: None,
    }
    .priced(prices)
}

/// The run's own model, as a decision's fallback: asked for `decide.llm` through the provider
/// the run already uses.
struct Own(Arc<dyn Model>);

impl Complete for Own {
    fn complete<'a>(&'a self, request: &'a Request) -> Completion<'a> {
        Box::pin(async move {
            let mut quiet = |_| {};
            self.0.complete(request, &mut quiet).await
        })
    }
}

/// The run's decisions from the configuration: `decide.via`'s System One provider first, its key
/// read from the environment variable it names (`env`); then `decide.llm` through the run's own
/// `model`, when it fails. `None` when decisions are off — not switched on (`decide.enabled`), no
/// classifier named, or every point off (`decide.bench`, `decide.route`).
///
/// # Errors
///
/// The `decide` settings are invalid, `decide.via`'s key variable is not set, or the TLS stack
/// cannot start.
pub fn decisions(
    config: &mnemorph_config::Config,
    world: &Path,
    model: Arc<dyn Model>,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Option<Decisions>, String> {
    let d = config.decide()?;
    if !d.bench && !d.route {
        return Ok(None);
    }
    let primary = match &d.via {
        None => None,
        Some(name) => {
            let p = config.providers().remove(name).ok_or_else(|| {
                format!("decide.via names `{name}`, and no provider has that name")
            })?;
            let key = match &p.key_env {
                None => None,
                Some(var) => Some(env(var).ok_or_else(|| {
                    format!("decide.via names `{name}`, whose key variable {var} is not set")
                })?),
            };
            let policy = SystemOne::policy();
            let transport = HttpTransport::new(policy.connect).map_err(|e| e.to_string())?;
            Some(Primary {
                client: SystemOne::new(Arc::new(transport), &p.base_url, key, policy),
                provider: name.clone(),
                model: d.model.clone(),
            })
        }
    };
    let fallback = Fallback {
        provider: model.provider().to_owned(),
        client: Arc::new(Own(model)),
        model: d.llm,
    };
    Ok(Some(Decisions {
        decider: Arc::new(Decider { primary, fallback }),
        cutoffs: Cutoffs {
            measured: d.cutoff_measured,
            stated: d.cutoff_stated,
        },
        journal: Journal::new(world.join(".mnemorph/instruments/decisions.jsonl")),
        bench: d.bench,
        route: d.route,
    }))
}

/// The question a piped bench run puts: how did it end, from its command and its output's tail.
#[must_use]
pub fn bench_ask(command: &str, output: &str) -> Ask {
    let mut start = output.len().saturating_sub(BENCH_TAIL);
    while !output.is_char_boundary(start) {
        start += 1;
    }
    Ask {
        state: format!("$ {command}\n{}", &output[start..]),
        questions: vec![(
            "outcome".into(),
            Question::Choice {
                instructions: "How did this test run end, judged only from its output?".into(),
                options: vec![
                    (
                        "green".into(),
                        "The run finished and every test passed.".into(),
                    ),
                    ("red".into(), "At least one test failed or errored.".into()),
                    (
                        "unknown".into(),
                        "The output does not show how the run ended.".into(),
                    ),
                ],
            },
        )],
    }
}

/// What the loop takes from a bench decision: green or red when the answer is sure for its basis,
/// nothing otherwise.
#[must_use]
pub fn bench_read(decision: &Decision, cutoffs: Cutoffs) -> Option<Read> {
    match decision.sure("outcome", cutoffs)?.choice() {
        "green" => Some(Read::Green),
        "red" => Some(Read::Red),
        _ => None,
    }
}
