//! The Kijin's act runner: the one harness path a typed outward act takes, in an order no step
//! can skip.
//!
//! 1. the post check — the act's kinds must all be the Kijin's post's;
//! 2. the plan's fingerprint ([`Fingerprint`]), what the human's approval binds to;
//! 3. the pin — where the act lands, read now; the request's key looked up; what the act would
//!    make must not exist yet;
//! 4. the gate — an Oni's envelope, read now and again at the moment of use
//!    ([`crate::EnvelopeGate`]); else the human, the card saying why no envelope admitted it;
//! 5. the fingerprint rechecked just before the spawn — a world that moved is asked again;
//!    for an envelope's act, the envelope read again there, the uses ledger held: one that no
//!    longer admits it sends the act to the human;
//! 6. `commit` — the intent journaled, under the request's key, with the envelope's use;
//! 7. the rendered command, run in the sandbox;
//! 8. `ran`;
//! 9. `observe`, with the read-only probe;
//! 10. the receipt.
//!
//! Acts of one post run one at a time (a lock beside its witness), and each is observed before
//! the next is gated, so no probe reads a later act's effect as an earlier one's. An act cut
//! between its intent and its result is never replayed: before the post's next act is gated, its
//! probe classifies it — landed (`observed`, not asked again), absent (asked again as a new act
//! when it is requested again), or unknown (`reconcile`, for the human).
//!
//! A request's key is the hash of its kind, its pinned target and its body — or, when the Kijin
//! names the request, of its kind, pinned target and that name. The key and the request it was
//! given to are kept beside the witness, outside the world: a body can neither forge nor read
//! them. The same key with the same request returns the stored receipt without acting; with a
//! different request it is a conflict, refused.
//!
//! A push and the pull request from it may share one approval ([`Runner::push_and_open`]): one
//! card shows both commands, bound to a fingerprint of both; each act is still pinned, journaled,
//! observed and receipted on its own, and the pull request runs only once the push is observed,
//! while the world is as approved but for what the push wrote.
//!
//! An act's undo ([`Runner::undo`]) takes the same path: the post check, the fingerprint, the
//! target still where the act went and what the act left still its own, the human — always —,
//! the recheck, the harness-rendered undo under its lease, and a probe that sees the effect gone.
//! Approving it opens an incident ([`crate::closure`]); a failed undo is `reconcile`, never undone.

use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mnemorph_ledger::{Journal, Timestamp};
use mnemorph_races::{Post, Race, ToolClass, check_class};
use mnemorph_tools::shell::{BeforeSpawn, Spawned, spawn_approved};
use mnemorph_tools::{Answer, Context, Fingerprint, Outcome, Place, Tool};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::closure::{Entry, forge_repo, told_prefix};
use crate::forge::{self, Login, PAGE, forge_get, login, probed};
use crate::oni::{Admitted, EnvelopeGate, admit, use_row};
use crate::receipt::{
    branch_tips, carries, command_line, excerpt, labels_argv, ls_remote_argv, private_file,
    pull_vacant, receipt_id, redact, sha256,
};
use crate::{Act, Done, PROBE_TIMEOUT, Pin, Planned, PostJournal, Probe, Receipt, Row, State};

/// What the card of a push and its pull request under one approval says of envelopes.
pub const PAIR_ASKS: &str = "no envelope admits a push and its pull request under one approval: the human decides the pair; asked for as two acts, each is read against the Oni's envelopes";

/// What the card of an undo says of envelopes.
pub const UNDO_ASKS: &str = "no envelope admits an undo: an undo always asks the human";

/// How many times an act whose world changed between the approval and the spawn is asked again.
const MAX_REASKS: usize = 3;
/// How long the act's command may run.
const ACT_TIMEOUT: Duration = Duration::from_secs(120);
/// How far before a recorded instant a read of the forge looks, for clocks that disagree.
const SKEW_SECS: i64 = 300;
/// The longest name a Kijin may give a request.
const KEY_CHARS: usize = 128;

/// One typed outward act a Kijin asks for, and the name it gives the request, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The act. A [`Act::Command`] is not the runner's: untyped commands go through `bash`.
    pub act: Act,
    /// The request's name: the same name with another request is a conflict.
    pub key: Option<String>,
    /// For a comment: the act whose open incident it tells downstream of — receipt (3). Such a
    /// comment is the one new act its incident's target admits.
    pub tells: Option<String>,
}

/// What a runner did with a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The receipt's id in the post journal.
    pub receipt: String,
    /// Where the act stands.
    pub state: State,
    /// The request had landed already: its stored receipt, nothing acted, nobody asked.
    pub replayed: bool,
    /// The command's exit code; `None` when nothing ran or a signal ended it.
    pub exit: Option<i32>,
    /// What the command printed, as the bash tool renders it.
    pub output: String,
    /// What else the runner did: cut acts it settled, approvals that went stale.
    pub notes: Vec<String>,
}

/// Why a request did not end in a receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Refused before the gate was asked: nothing was asked and nothing ran.
    Refused(String),
    /// The human did not approve: nothing ran.
    Denied(String),
    /// The act could not be run, or its journal written.
    Failed(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Refused(why) => write!(f, "refused before the gate: {why}"),
            Refusal::Denied(why) => write!(f, "the human gate did not approve: {why}"),
            Refusal::Failed(why) => write!(f, "the act failed: {why}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// What spawns an approved act. The harness's is [`Sandboxed`]; a test may stand in its own.
pub trait Hands {
    /// Spawn `planned`'s command, approved against `plan`, running `before` between the plan's
    /// recheck and the spawn, and giving it `stdin` — the approved bytes the harness holds
    /// ([`Planned::stdin`]) — on its standard input.
    fn spawn(
        &self,
        planned: &Planned,
        cx: &Context,
        plan: Fingerprint,
        before: BeforeSpawn<'_>,
        stdin: Option<&[u8]>,
    ) -> Spawned;
}

/// The harness's hands: the bash tool's own spawn — the sandbox, the tripwire, the attic, and
/// the plan rechecked at the spawn.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sandboxed;

impl Hands for Sandboxed {
    fn spawn(
        &self,
        planned: &Planned,
        cx: &Context,
        plan: Fingerprint,
        before: BeforeSpawn<'_>,
        stdin: Option<&[u8]>,
    ) -> Spawned {
        spawn_approved(
            planned.command(),
            cx,
            ACT_TIMEOUT,
            planned.class(),
            plan,
            before,
            stdin,
        )
    }
}

/// The typed act as the intents file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum Typed {
    PushNewBranch {
        remote: String,
        name: String,
    },
    AddLabel {
        repo: String,
        pr: u64,
        label: String,
    },
    Comment {
        repo: String,
        pr: u64,
        body: String,
    },
    OpenPullRequest {
        repo: String,
        base: String,
        head: String,
        title: String,
        body: String,
    },
}

impl Typed {
    fn of(act: &Act) -> Option<Self> {
        Some(match act.clone() {
            Act::PushNewBranch { remote, name } => Typed::PushNewBranch { remote, name },
            Act::AddLabel { repo, pr, label } => Typed::AddLabel { repo, pr, label },
            Act::Comment { repo, pr, body } => Typed::Comment { repo, pr, body },
            Act::OpenPullRequest {
                repo,
                base,
                head,
                title,
                body,
            } => Typed::OpenPullRequest {
                repo,
                base,
                head,
                title,
                body,
            },
            Act::Command(_) => return None,
        })
    }

    pub(crate) fn act(&self) -> Act {
        match self.clone() {
            Typed::PushNewBranch { remote, name } => Act::PushNewBranch { remote, name },
            Typed::AddLabel { repo, pr, label } => Act::AddLabel { repo, pr, label },
            Typed::Comment { repo, pr, body } => Act::Comment { repo, pr, body },
            Typed::OpenPullRequest {
                repo,
                base,
                head,
                title,
                body,
            } => Act::OpenPullRequest {
                repo,
                base,
                head,
                title,
                body,
            },
        }
    }
}

/// The pin as the intents file keeps it: whole, credentials and all, outside the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "pin", rename_all = "kebab-case")]
pub(crate) enum Pinned {
    Push { urls: Vec<String>, sha: String },
    Forge { host: String },
}

impl Pinned {
    fn of(pin: &Pin) -> Option<Self> {
        match pin {
            Pin::Push { urls, sha } => Some(Pinned::Push {
                urls: urls.clone(),
                sha: sha.clone(),
            }),
            Pin::Forge { host } => Some(Pinned::Forge { host: host.clone() }),
            Pin::Unpinned => None,
        }
    }

    pub(crate) fn pin(&self) -> Pin {
        match self {
            Pinned::Push { urls, sha } => Pin::Push {
                urls: urls.clone(),
                sha: sha.clone(),
            },
            Pinned::Forge { host } => Pin::Forge { host: host.clone() },
        }
    }
}

/// What an act left behind, as the intents file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "done", rename_all = "kebab-case")]
pub(crate) enum Left {
    Pushed { sha: String },
    Labelled,
    Commented { id: u64 },
    Opened { number: u64 },
}

impl Left {
    fn of(done: &Done) -> Self {
        match done {
            Done::Pushed { sha } => Left::Pushed { sha: sha.clone() },
            Done::Labelled => Left::Labelled,
            Done::Commented { id } => Left::Commented { id: *id },
            Done::Opened { number } => Left::Opened { number: *number },
        }
    }

    pub(crate) fn done(&self) -> Done {
        match self {
            Left::Pushed { sha } => Done::Pushed { sha: sha.clone() },
            Left::Labelled => Done::Labelled,
            Left::Commented { id } => Done::Commented { id: *id },
            Left::Opened { number } => Done::Opened { number: *number },
        }
    }
}

/// One act's intent, kept beside the witness before its `committed` row is written: the key, the
/// request it was given to, and what a probe needs to find the act again after a cut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Intent {
    /// The receipt's id.
    pub(crate) act: String,
    /// The request's key.
    key: String,
    /// The hash of the request: its kind, pinned target and body.
    request: String,
    /// When its target was found vacant.
    pinned: Timestamp,
    /// When it was committed.
    pub(crate) at: Timestamp,
    pub(crate) typed: Typed,
    pub(crate) pin: Pinned,
    /// What it left, once known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) left: Option<Left>,
    /// For a comment: the act whose incident it tells downstream of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tells: Option<String>,
    /// The envelope that admitted it, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) envelope: Option<String>,
}

impl Intent {
    /// The intent of `planned`, ready at the gate, before its receipt's id is known.
    fn draft(planned: &Planned, ready: &Ready, request: &Request) -> Result<Self, Refusal> {
        Ok(Self {
            act: String::new(),
            key: ready.key.clone(),
            request: ready.digest.clone(),
            pinned: ready.pinned,
            at: ready.pinned,
            typed: Typed::of(planned.act())
                .ok_or_else(|| Refusal::Refused("an untyped command is not a typed act".into()))?,
            pin: Pinned::of(&ready.pin)
                .ok_or_else(|| Refusal::Refused("the pin is not of this act".into()))?,
            left: None,
            tells: request.tells.clone(),
            envelope: None,
        })
    }
}

/// Where a post's intents are kept: beside its witness.
pub(crate) fn intents_path(witness: &Path) -> PathBuf {
    witness.with_extension("intents.jsonl")
}

/// The kind, the pinned target and the body of a typed act, each its own field.
fn request_parts(act: &Act, pin: &Pin) -> Option<(String, String)> {
    let target = |parts: &[&str]| parts.join("\u{0}");
    Some(match (act, pin) {
        (Act::PushNewBranch { remote, name }, Pin::Push { urls, sha }) => (
            target(&["push-new-branch", remote, name, &urls.join("\u{1}")]),
            sha.clone(),
        ),
        (Act::AddLabel { repo, pr, label }, Pin::Forge { host }) => (
            target(&["add-label", host, repo, &pr.to_string()]),
            label.to_lowercase(),
        ),
        (Act::Comment { repo, pr, body }, Pin::Forge { host }) => (
            target(&["comment", host, repo, &pr.to_string()]),
            body.clone(),
        ),
        (
            Act::OpenPullRequest {
                repo,
                base,
                head,
                title,
                body,
            },
            Pin::Forge { host },
        ) => (
            target(&["open-pull-request", host, repo, base, head]),
            format!("{title}\u{0}{body}"),
        ),
        _ => return None,
    })
}

/// The request's key and its hash: `(key, request)`. A comment that tells of an incident names
/// it in both: it is never the replay of a comment made before.
fn keyed(
    act: &Act,
    pin: &Pin,
    name: Option<&str>,
    tells: Option<&str>,
) -> Option<(String, String)> {
    let (mut target, body) = request_parts(act, pin)?;
    if let Some(act) = tells {
        target = format!("{target}\u{0}tells\u{0}{act}");
    }
    let request = sha256(&format!("request\u{0}{target}\u{0}{body}"));
    let key = match name {
        Some(name) => sha256(&format!("named\u{0}{target}\u{0}{name}")),
        None => request.clone(),
    };
    Some((key, request))
}

/// A name a Kijin may give a request: plain, short.
fn key_ok(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= KEY_CHARS
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/' | b':'))
}

/// What a probe found of a cut act.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Landing {
    /// It landed: this is what it left.
    Verified(Done),
    /// Nothing of it is outside.
    Absent,
    /// The probe cannot tell.
    Unknown(String),
}

/// What a forge list holds of comments.
#[derive(Deserialize)]
struct Comment {
    id: u64,
    body: String,
    issue_url: String,
    #[serde(default)]
    user: Option<Login>,
}

/// Look for a cut act's effect at its pin: whether it landed, is absent, or cannot be told.
fn landed(
    intent: &Intent,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Landing {
    let unknown = Landing::Unknown;
    match (&intent.typed, &intent.pin) {
        (Typed::PushNewBranch { name, .. }, Pinned::Push { urls, sha }) => {
            let (mut there, mut gone) = (0, 0);
            for url in urls {
                let out = match probed(probe, &ls_remote_argv(url, name), probes, seen) {
                    Ok(out) => out,
                    Err(why) => return unknown(why),
                };
                match branch_tips(&out, name).as_slice() {
                    [] => gone += 1,
                    [one] if one == sha => there += 1,
                    [one] => {
                        return unknown(format!(
                            "refs/heads/{name} is at {}, not the pushed {sha}",
                            excerpt(one.as_bytes(), 80)
                        ));
                    }
                    _ => return unknown(format!("the remote lists refs/heads/{name} twice")),
                }
            }
            match (there, gone) {
                (_, 0) if there > 0 => Landing::Verified(Done::Pushed { sha: sha.clone() }),
                (0, _) if gone > 0 => Landing::Absent,
                _ => unknown(format!(
                    "refs/heads/{name} is on {there} of the push URLs and missing from {gone}"
                )),
            }
        }
        (Typed::AddLabel { repo, pr, label }, Pinned::Forge { host }) => {
            match probed(probe, &labels_argv(repo, *pr, host), probes, seen)
                .and_then(|out| carries(&out, label))
            {
                Ok(true) => Landing::Verified(Done::Labelled),
                Ok(false) => Landing::Absent,
                Err(why) => unknown(why),
            }
        }
        (Typed::Comment { repo, pr, body }, Pinned::Forge { host }) => {
            // As strict as `observe`: a comment is the act's only when `gh`'s own login wrote
            // it, with the act's body, on the act's pull request — and, once its id is known,
            // that id. Someone else's identical comment is never taken for it.
            let me = match login(host, probe, probes, seen) {
                Ok(me) => me,
                Err(why) => return unknown(why),
            };
            let known = match &intent.left {
                Some(Left::Commented { id }) => Some(*id),
                _ => None,
            };
            let since = Timestamp(intent.at.0 - SKEW_SECS);
            let argv = forge_get(
                host,
                &format!("repos/{repo}/issues/{pr}/comments?per_page={PAGE}&since={since}"),
            );
            let out = match probed(probe, &argv, probes, seen) {
                Ok(out) => out,
                Err(why) => return unknown(why),
            };
            let Ok(list) = serde_json::from_slice::<Vec<Comment>>(&out) else {
                return unknown("the comments do not read".into());
            };
            let on = format!("/repos/{repo}/issues/{pr}").to_ascii_lowercase();
            let want = sha256(body);
            let ours: Vec<u64> = list
                .iter()
                .filter(|c| c.issue_url.to_ascii_lowercase().ends_with(&on))
                .filter(|c| sha256(&c.body) == want)
                .filter(|c| c.user.as_ref().is_some_and(|u| u.login == me))
                .filter(|c| known.is_none_or(|id| id == c.id))
                .map(|c| c.id)
                .collect();
            match ours.as_slice() {
                [id] => Landing::Verified(Done::Commented { id: *id }),
                [] if list.len() < PAGE => Landing::Absent,
                [] => unknown(format!(
                    "{PAGE} comments since the act and none of them its: the rest are unread"
                )),
                _ => unknown(format!(
                    "{} comments since the act carry its body: which is its cannot be told",
                    ours.len()
                )),
            }
        }
        (Typed::OpenPullRequest { .. }, Pinned::Forge { host }) => {
            pull_landed(intent, host, probe, probes, seen)
        }
        _ => unknown("the pin is not of this act".into()),
    }
}

/// Whether a cut pull request act landed. As strict as `observe`: the one pull request open from
/// the act's head into its base, by `gh`'s own login, with the act's title and description — and,
/// once its number is known, that number. Someone else's is never taken for it.
fn pull_landed(
    intent: &Intent,
    host: &str,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Landing {
    let act = intent.typed.act();
    let Some(want) = forge::Wanted::of(&act) else {
        return Landing::Unknown("the act opens no pull request".into());
    };
    let known = match &intent.left {
        Some(Left::Opened { number }) => Some(*number),
        _ => None,
    };
    match forge::find(host, &want, known, probe, PROBE_TIMEOUT, probes, seen) {
        Ok(Some(number)) => Landing::Verified(Done::Opened { number }),
        Ok(None) => Landing::Absent,
        Err(why) => Landing::Unknown(why),
    }
}

/// Whether what an act left is gone, for its undo: `Absent` only when nothing of it is seen. A
/// comment is gone only when no comment of the pull request holds its id — an edited one, whose
/// body is no longer the act's, is still there.
fn gone(
    intent: &Intent,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Landing {
    if let (Typed::OpenPullRequest { repo, .. }, Pinned::Forge { host }) =
        (&intent.typed, &intent.pin)
    {
        return pull_gone(intent, repo, host, probe, probes, seen);
    }
    let (Typed::Comment { repo, pr, .. }, Pinned::Forge { host }) = (&intent.typed, &intent.pin)
    else {
        return landed(intent, probe, probes, seen);
    };
    let Some(Left::Commented { id }) = &intent.left else {
        return Landing::Unknown("the comment's id was never known".into());
    };
    let since = Timestamp(intent.at.0 - SKEW_SECS);
    let argv = forge_get(
        host,
        &format!("repos/{repo}/issues/{pr}/comments?per_page={PAGE}&since={since}"),
    );
    let out = match probed(probe, &argv, probes, seen) {
        Ok(out) => out,
        Err(why) => return Landing::Unknown(why),
    };
    let Ok(list) = serde_json::from_slice::<Vec<Comment>>(&out) else {
        return Landing::Unknown("the comments do not read".into());
    };
    if list.iter().any(|c| c.id == *id) {
        Landing::Verified(Done::Commented { id: *id })
    } else if list.len() < PAGE {
        Landing::Absent
    } else {
        Landing::Unknown(format!(
            "{PAGE} comments since the act and none of them {id}: the rest are unread"
        ))
    }
}

/// Whether the pull request an act opened is gone, for its undo: closed, by its number, and never
/// merged — a merged pull request is not put back by closing it. An edited one is still there.
fn pull_gone(
    intent: &Intent,
    repo: &str,
    host: &str,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Landing {
    let Some(Left::Opened { number }) = &intent.left else {
        return Landing::Unknown("the pull request's number was never known".into());
    };
    match forge::pull(host, repo, *number, probe, probes, seen) {
        Ok(p) if p.merged_at.is_some() => Landing::Unknown(format!(
            "#{number} was merged: closing it does not put it back"
        )),
        Ok(p) if p.state == "closed" => Landing::Absent,
        Ok(p) if p.state == "open" => Landing::Verified(Done::Opened { number: *number }),
        Ok(p) => Landing::Unknown(format!("#{number} is {}", excerpt(p.state.as_bytes(), 40))),
        Err(why) => Landing::Unknown(why),
    }
}

/// Whether a label on a pull request is still the one an act added: the only label event for
/// it since the act was pinned is the act's own add, by the login `gh` acts as, and the pull
/// request still carries it. Anything else — someone else's add, a remove and add again, a page
/// too full to read whole, a probe that fails — is not certain.
fn label_is_ours(
    intent: &Intent,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Named {
        name: String,
    }
    #[derive(Deserialize)]
    struct Event {
        #[serde(rename = "event")]
        kind: String,
        #[serde(default)]
        actor: Option<Login>,
        #[serde(default)]
        label: Option<Named>,
        created_at: String,
    }
    let (Typed::AddLabel { repo, pr, label }, Pinned::Forge { host }) =
        (&intent.typed, &intent.pin)
    else {
        return Err("the act is not a label's".into());
    };
    let me = Login {
        login: login(host, probe, probes, seen)?,
    };
    let path = format!("repos/{repo}/issues/{pr}/events?per_page={PAGE}");
    let events: Vec<Event> =
        serde_json::from_slice(&probed(probe, &forge_get(host, &path), probes, seen)?)
            .map_err(|e| format!("the events do not read: {e}"))?;
    if events.len() >= PAGE {
        return Err(format!(
            "{repo}#{pr} has {PAGE} events or more: the later ones are unread"
        ));
    }
    let since = intent.pinned.0 - SKEW_SECS;
    let want = label.to_lowercase();
    let mut touched = Vec::new();
    for e in &events {
        let named = e
            .label
            .as_ref()
            .is_some_and(|l| l.name.to_lowercase() == want);
        if !named || !matches!(e.kind.as_str(), "labeled" | "unlabeled") {
            continue;
        }
        let at = Timestamp::parse(&e.created_at)
            .ok_or_else(|| format!("an event's time does not read: {:?}", e.created_at))?;
        if at.0 >= since {
            touched.push(e);
        }
    }
    match touched.as_slice() {
        [one]
            if one.kind == "labeled" && one.actor.as_ref().is_some_and(|a| a.login == me.login) => {
        }
        [] => return Err(format!("no event shows label {label:?} added by this act")),
        [one] if one.kind == "labeled" => {
            return Err(format!(
                "label {label:?} was added by {}, not by {}: it is not this act's",
                one.actor.as_ref().map_or("someone unknown", |a| &a.login),
                me.login
            ));
        }
        _ => {
            return Err(format!(
                "label {label:?} was added or removed {} times since the act was pinned",
                touched.len()
            ));
        }
    }
    let labels = probed(probe, &labels_argv(repo, *pr, host), probes, seen)?;
    if carries(&labels, label)? {
        Ok(())
    } else {
        Err(format!("{repo}#{pr} no longer carries label {label:?}"))
    }
}

/// The act runner of one Kijin body: its context, its post journal, its probe and its hands.
pub struct Runner<'a> {
    cx: &'a Context,
    journal: PostJournal,
    probe: &'a dyn Probe,
    hands: &'a dyn Hands,
    envelopes: Option<EnvelopeGate>,
}

/// Why an act an envelope admitted before the recheck does not run under it.
enum AtUse {
    /// The envelope no longer admits it: the human decides it.
    Lapsed(String),
    /// The uses ledger cannot be held: nothing runs.
    Failed(String),
}

/// What the gate is asked about a re-ask, once.
struct Reask {
    changed: Vec<String>,
    notes: Vec<String>,
    /// Why an envelope that admitted the act no longer did at the moment of use: the human
    /// decides it from then on.
    human: Option<String>,
}

/// An act ready for the gate: the plan its approval binds to, where it lands, its key, when its
/// target was found vacant, and what the pin read for the card.
struct Ready {
    plan: Fingerprint,
    pin: Pin,
    key: String,
    digest: String,
    pinned: Timestamp,
    card: Vec<String>,
}

/// What preparing an act came to.
enum Prepared {
    /// The request had landed already: its stored receipt.
    Replayed(Report),
    /// Asked, then run.
    Ready(Ready),
}

/// What a push and the pull request from it came to: each its own receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paired {
    /// The push's report.
    pub push: Report,
    /// The pull request's report, or why it did not open.
    pub pull: Result<Report, Refusal>,
}

/// One fingerprint of two plans, for the card of a pair: each part the two share, or a hash of
/// both. Each act is still rechecked against its own plan at its spawn.
fn both(a: &Fingerprint, b: &Fingerprint) -> Fingerprint {
    let join = |x: &str, y: &str| {
        if x == y {
            x.to_owned()
        } else {
            sha256(&format!("{x}\u{0}{y}"))
        }
    };
    Fingerprint {
        act: sha256(&format!("{}\u{0}{}", a.act, b.act)),
        head: match (&a.head, &b.head) {
            (Some(x), Some(y)) => Some(join(x, y)),
            (x, y) => x.clone().or_else(|| y.clone()),
        },
        diff: join(&a.diff, &b.diff),
        git: join(&a.git, &b.git),
        target: join(&a.target, &b.target),
    }
}

impl<'a> Runner<'a> {
    /// A runner for the body of `cx`, journaling in `journal`, probing through `probe` and
    /// spawning through `hands`.
    #[must_use]
    pub fn new(
        cx: &'a Context,
        journal: PostJournal,
        probe: &'a dyn Probe,
        hands: &'a dyn Hands,
    ) -> Self {
        Self {
            cx,
            journal,
            probe,
            hands,
            envelopes: None,
        }
    }

    /// The same runner, reading an Oni's envelopes and counting their uses through `envelopes`;
    /// with none, every act asks the human.
    #[must_use]
    pub fn with_envelopes(mut self, envelopes: Option<EnvelopeGate>) -> Self {
        self.envelopes = envelopes;
        self
    }

    /// Whether an envelope admits `planned`, landing at `pin`, today; `held` at the moment of use.
    fn admit(
        &self,
        planned: &Planned,
        pin: &Pin,
        held: Option<&mnemorph_ledger::Held<'_>>,
    ) -> Result<Admitted, String> {
        admit(
            self.envelopes.as_ref(),
            self.cx.race(),
            planned,
            pin,
            self.cx.world(),
            self.probe,
            &self.journal,
            self.cx.body(),
            Timestamp::now().day(),
            held,
        )
    }

    fn witness(&self) -> Result<&Path, Refusal> {
        self.journal.witness().ok_or_else(|| {
            Refusal::Refused(
                "there is no state directory to keep the post's witness and keys outside the world, so no typed act runs".into(),
            )
        })
    }

    fn intents(&self) -> Result<Journal<Intent>, Refusal> {
        Ok(Journal::new(intents_path(self.witness()?)))
    }

    /// Hold the post's lock: one act of a post at a time, from its first check to its receipt.
    fn hold(&self) -> Result<fs::File, Refusal> {
        let lock = self.witness()?.with_extension("lock");
        let f = private_file(&lock)
            .map_err(|e| Refusal::Failed(format!("the post's lock {}: {e}", lock.display())))?;
        // Never waited on: the act in flight may be waiting on its human for minutes.
        match f.try_lock() {
            Ok(()) => Ok(f),
            Err(fs::TryLockError::WouldBlock) => Err(Refusal::Refused(format!(
                "an act of the {} post is in flight; ask again once it has its receipt",
                self.journal.post.creature()
            ))),
            Err(fs::TryLockError::Error(e)) => Err(Refusal::Failed(format!(
                "the post's lock {}: {e}",
                lock.display()
            ))),
        }
    }

    /// Every intent kept, the last row of each act winning.
    fn kept(&self) -> Result<Vec<Intent>, Refusal> {
        let back = self
            .intents()?
            .read()
            .map_err(|e| Refusal::Failed(format!("the post's intents cannot be read: {e}")))?;
        let mut out: Vec<Intent> = Vec::new();
        for row in back.rows {
            if let Some(at) = out.iter().position(|i| i.act == row.act) {
                out[at] = row;
            } else {
                out.push(row);
            }
        }
        Ok(out)
    }

    fn keep(&self, intent: &Intent) -> io::Result<()> {
        let path = intents_path(
            self.journal
                .witness()
                .ok_or_else(|| io::Error::other("no witness"))?,
        );
        drop(private_file(&path)?);
        Journal::new(path).append(intent)
    }

    /// The acts the witness holds, each at its latest row.
    fn standing(&self) -> Result<Vec<Row>, Refusal> {
        self.journal
            .standing()
            .map(|(rows, _)| rows)
            .map_err(|e| Refusal::Failed(format!("the post journal cannot be read: {e}")))
    }

    fn settle(&self, id: &str, state: State, why: String, probes: &[String], seen: &[String]) {
        let mut row = Row::new(id, state);
        row.why = Some(why);
        row.probe = (!probes.is_empty()).then(|| probes.join(" ; "));
        row.seen =
            (!seen.is_empty()).then(|| excerpt(seen.join(" ; ").as_bytes(), crate::SEEN_BYTES));
        // A row that cannot be written leaves the act open: the next act tries again.
        let _ = self.journal.append(&row);
    }

    /// Settle every act of the post cut between its intent and its result: probe it, and
    /// journal it `observed` (it landed), `absent` (it did not) or `reconcile` (the probe cannot
    /// tell). Returns a line for each. The act runner does this itself before it gates an act.
    ///
    /// # Errors
    ///
    /// The post's journal or intents cannot be read, or its lock taken.
    pub fn recover(&self) -> Result<Vec<String>, Refusal> {
        let _held = self.hold()?;
        self.recover_held()
    }

    /// [`Self::recover`], the post's lock already held.
    fn recover_held(&self) -> Result<Vec<String>, Refusal> {
        let open: Vec<Row> = self
            .standing()?
            .into_iter()
            .filter(|r| matches!(r.state, State::Committed | State::PendingObservation))
            .collect();
        if open.is_empty() {
            return Ok(Vec::new());
        }
        let kept = self.kept()?;
        let mut lines = Vec::new();
        for row in open {
            let (mut probes, mut seen) = (Vec::new(), Vec::new());
            let Some(intent) = kept.iter().find(|i| i.act == row.act) else {
                let why = "a cut act whose intent was not kept: no probe can find it".to_owned();
                self.settle(&row.act, State::Reconcile, why.clone(), &[], &[]);
                lines.push(format!("act {} is in reconcile: {why}", row.act));
                continue;
            };
            let (state, why) = match landed(intent, self.probe, &mut probes, &mut seen) {
                Landing::Verified(done) => {
                    let mut with = intent.clone();
                    with.left = Some(Left::of(&done));
                    let _ = self.keep(&with);
                    (
                        State::Observed,
                        "a cut act the probe found landed: settled, not asked again".to_owned(),
                    )
                }
                Landing::Absent => (
                    State::Absent,
                    "a cut act the probe found never landed: asked again as a new act when it is requested again".to_owned(),
                ),
                Landing::Unknown(why) => (
                    State::Reconcile,
                    format!("a cut act the probe cannot place: {why}"),
                ),
            };
            self.settle(&row.act, state, why.clone(), &probes, &seen);
            lines.push(format!("act {} was cut: {state} — {why}", row.act));
        }
        Ok(lines)
    }

    /// Run a Kijin's typed outward act through the whole pipeline.
    ///
    /// # Errors
    ///
    /// [`Refusal::Refused`] when it is refused before the gate is asked; [`Refusal::Denied`]
    /// when the human does not approve; [`Refusal::Failed`] when it cannot run or be journaled.
    pub fn act(&self, request: &Request) -> Result<Report, Refusal> {
        let refuse = |why: String| Err(Refusal::Refused(why));
        Self::well_formed(request)?;
        self.may_act()?;
        // A telling comment opens with the harness's words, naming the act; the Kijin's follow.
        let told;
        let request = match (&request.tells, &request.act) {
            (Some(of), Act::Comment { repo, pr, body }) => {
                if body.trim().is_empty() {
                    return refuse("a comment that tells downstream says what happened".into());
                }
                told = Request {
                    act: Act::Comment {
                        repo: repo.clone(),
                        pr: *pr,
                        body: format!("{}{body}", told_prefix(of)),
                    },
                    key: request.key.clone(),
                    tells: request.tells.clone(),
                };
                &told
            }
            _ => request,
        };
        let planned = self.planned(&request.act)?;
        let _held = self.hold()?;
        // Cut acts first: each is probed before the next act of the post is gated.
        let mut reask = Reask {
            changed: Vec::new(),
            notes: self.recover_held()?,
            human: None,
        };
        self.run_held(&planned, request, &mut reask)
    }

    /// What every request must be before anything is read: a typed act, a plain key, and `tells`
    /// only on a comment.
    fn well_formed(request: &Request) -> Result<(), Refusal> {
        let refuse = |why: String| Err(Refusal::Refused(why));
        if let Act::Command(_) = request.act {
            return refuse(
                "an untyped command is not a typed act: run it with bash, where it is destructive and asks the human".into(),
            );
        }
        if let Some(name) = &request.key
            && !key_ok(name)
        {
            return refuse(format!(
                "a request's key is at most {KEY_CHARS} letters, digits, `-`, `_`, `.`, `/` or `:`"
            ));
        }
        if request.tells.is_some() && !matches!(request.act, Act::Comment { .. }) {
            return refuse("only a comment tells downstream of an incident".into());
        }
        Ok(())
    }

    /// The act planned, its class the race's, and — step 1 — its kinds all the post's: an act
    /// the post does not hold is refused before anything is read or asked.
    fn planned(&self, act: &Act) -> Result<Planned, Refusal> {
        let planned =
            crate::plan(act, &self.place()).map_err(|e| Refusal::Refused(e.to_string()))?;
        check_class(self.cx.race(), planned.class())
            .map_err(|e| Refusal::Refused(e.to_string()))?;
        planned
            .check_post(self.cx.body())
            .map_err(|e| Refusal::Refused(e.to_string()))?;
        Ok(planned)
    }

    /// Steps 2 to 10 of one act, the post's lock held and its cut acts settled: asked again when
    /// the world moves between the approval and the spawn, at most [`MAX_REASKS`] times.
    fn run_held(
        &self,
        planned: &Planned,
        request: &Request,
        reask: &mut Reask,
    ) -> Result<Report, Refusal> {
        for _ in 0..=MAX_REASKS {
            if let Some(mut report) = self.once(planned, request, reask)? {
                if let Some(of) = &request.tells
                    && report.state == State::Observed
                {
                    let told = Entry::Told {
                        act: of.clone(),
                        receipt: report.receipt.clone(),
                        at: Timestamp::now(),
                    };
                    report.notes.push(match self.journal.record(&told) {
                        Ok(()) => format!("recorded as the incident on act {of} told downstream"),
                        Err(e) => {
                            format!("not recorded as the incident on act {of} told downstream: {e}")
                        }
                    });
                }
                return Ok(report);
            }
        }
        Err(Refusal::Failed(format!(
            "the world changed between each approval and the spawn, {} times: the act did not run; {}",
            MAX_REASKS + 1,
            reask.notes.join("; ")
        )))
    }

    /// One pass from the fingerprint to the receipt; `None` when the world moved before the spawn
    /// and the gate must be asked again.
    fn once(
        &self,
        planned: &Planned,
        request: &Request,
        reask: &mut Reask,
    ) -> Result<Option<Report>, Refusal> {
        let ready = match self.prepare(planned, request, reask, None)? {
            Prepared::Replayed(report) => return Ok(Some(report)),
            Prepared::Ready(ready) => ready,
        };
        // 4. The gate: an Oni's envelope, read again at the moment of use; else the human.
        let preview = match reask
            .human
            .clone()
            .map_or_else(|| self.admit(planned, &ready.pin, None), Err)
        {
            Ok(admitted) => Some(admitted),
            Err(why) => {
                self.ask(planned, &ready.plan, &reask.changed, &ready.card, &why)?;
                None
            }
        };
        self.execute(planned, request, ready, reask, preview.as_ref())
    }

    /// Steps 2 and 3: the plan the approval binds to, the pin, the key, and what the act would make
    /// absent — or the stored receipt of a request that landed already. `pushing` is the commit a
    /// push approved with a pull request makes its head: that head need not exist yet.
    fn prepare(
        &self,
        planned: &Planned,
        request: &Request,
        reask: &mut Reask,
        pushing: Option<&str>,
    ) -> Result<Prepared, Refusal> {
        let world = self.cx.world();
        // 2. The plan the approval binds to, read before the human is asked.
        let plan = Fingerprint::of(planned.command(), world).map_err(|e| {
            Refusal::Refused(format!(
                "the plan this act would be approved against could not be read: {e}"
            ))
        })?;
        // 3. The pin: where it lands, then the key, then what it would make must not exist.
        let pin = planned.aim(world).map_err(Refusal::Refused)?;
        let (key, digest) = keyed(
            planned.act(),
            &pin,
            request.key.as_deref(),
            request.tells.as_deref(),
        )
        .ok_or_else(|| Refusal::Refused("the pin is not of this act".into()))?;
        self.clear(planned.act(), &pin, request.tells.as_deref())?;
        if let Some(report) = self.replay(&key, &digest, reask)? {
            return Ok(Prepared::Replayed(report));
        }
        let card = if let (Act::OpenPullRequest { .. }, Pin::Forge { host }) = (planned.act(), &pin)
        {
            self.pull_pin(planned.act(), host, pushing)?
        } else {
            planned.vacant(&pin, self.probe).map_err(Refusal::Refused)?;
            Vec::new()
        };
        Ok(Prepared::Ready(Ready {
            plan,
            pin,
            key,
            digest,
            pinned: Timestamp::now(),
            card,
        }))
    }

    /// Whether a pull request may open from its head: the pin's reads ([`pull_vacant`]), and —
    /// when this post pushed the head — the head still at the commit it pushed. Returns what the
    /// card says of the head.
    fn pull_pin(
        &self,
        act: &Act,
        host: &str,
        pushing: Option<&str>,
    ) -> Result<Vec<String>, Refusal> {
        let Act::OpenPullRequest { repo, head, .. } = act else {
            return Err(Refusal::Refused("the act opens no pull request".into()));
        };
        let tip =
            pull_vacant(act, host, pushing.is_some(), self.probe).map_err(Refusal::Refused)?;
        let Some(tip) = tip else {
            return Ok(vec![format!(
                "head {head}: the branch the push above makes, at {}",
                pushing.unwrap_or_default()
            )]);
        };
        Ok(vec![match self.pushed(head, repo, host)? {
            Some((act, sha)) if sha == tip => {
                format!("head {head} at {tip}, as act {act} of this post pushed it")
            }
            Some((act, sha)) => {
                return Err(Refusal::Refused(format!(
                    "the head `{head}` is at {tip} on {repo}, not at {sha}, where act {act} of this post pushed it: the branch moved since, so no pull request opens from it"
                )));
            }
            None => format!("head {head} at {tip}: a branch of {repo} this post did not push"),
        }])
    }

    /// The latest act of this post that pushed `head` to a URL naming `repo` on `host`, still
    /// standing ([`may_stand`]): its receipt and the commit — a head it may have left is held to it.
    fn pushed(
        &self,
        head: &str,
        repo: &str,
        host: &str,
    ) -> Result<Option<(String, String)>, Refusal> {
        let standing = self.standing()?;
        Ok(self.kept()?.into_iter().rev().find_map(|i| {
            let (Typed::PushNewBranch { name, .. }, Pinned::Push { urls, sha }) =
                (&i.typed, &i.pin)
            else {
                return None;
            };
            let holds = standing
                .iter()
                .any(|r| r.act == i.act && may_stand(r.state));
            let there = urls
                .iter()
                .filter_map(|u| forge_repo(u, host))
                .any(|r| r.eq_ignore_ascii_case(repo));
            (name == head && holds && there).then(|| (i.act.clone(), sha.clone()))
        }))
    }

    /// Steps 5 to 10 of an approved act: the recheck, the intent, the spawn, the result and the
    /// probe; `None` when the world moved before the spawn and the gate must be asked again.
    /// `preview` is the envelope that admitted it before the recheck, if one did: it is read
    /// again at the moment of use, and only that reading admits.
    fn execute(
        &self,
        planned: &Planned,
        request: &Request,
        ready: Ready,
        reask: &mut Reask,
        preview: Option<&Admitted>,
    ) -> Result<Option<Report>, Refusal> {
        // 5–7. The recheck, the intent, the spawn — in that order, in the hands.
        let draft = Intent::draft(planned, &ready, request)?;
        let (plan, pin) = (ready.plan, ready.pin);
        let short = plan.short();
        let shown_plan = plan.clone();
        let mut committed: Option<Receipt> = None;
        let mut admitted: Option<Admitted> = None;
        let mut lapsed: Option<String> = None;
        let mut before = || {
            // The moment of use: the envelope read again, the uses ledger held until the use and
            // the intent are written, so no other act takes the same use meanwhile.
            let used = match self.at_use(planned, &pin, preview) {
                Ok(used) => used,
                Err(AtUse::Lapsed(why)) => {
                    let said = format!(
                        "the envelope no longer admits the act at the moment of use: {why}"
                    );
                    lapsed = Some(why);
                    return Err(said);
                }
                Err(AtUse::Failed(e)) => return Err(e),
            };
            let line = used.as_ref().map(|(_, by)| by.to_string());
            let receipt = {
                let mut first = |id: &str| {
                    if let Some((held, by)) = &used {
                        held.record(&use_row(by, id, &self.journal, planned))?;
                    }
                    let mut intent = draft.clone();
                    id.clone_into(&mut intent.act);
                    intent.at = Timestamp::now();
                    intent.envelope = used.as_ref().map(|(_, by)| by.envelope.name.clone());
                    self.keep(&intent)
                };
                self.journal
                    .commit_then(
                        planned,
                        pin.clone(),
                        Some(&short),
                        line.as_deref(),
                        &mut first,
                    )
                    .map_err(|e| format!("the intent could not be journaled: {e}"))?
            };
            committed = Some(receipt);
            if let Some((held, by)) = used {
                drop(held);
                self.show(planned, &shown_plan, &by);
                admitted = Some(by);
            }
            Ok(())
        };
        // A pull request's description goes to `gh` on its input, from the approved bytes.
        let spawned = self
            .hands
            .spawn(planned, self.cx, plan, &mut before, planned.stdin());
        let out = match spawned {
            Spawned::Ran(out) => out,
            Spawned::Changed { approved, now } => {
                reask.changed = approved.changes(&now);
                reask.notes.push(format!(
                    "the world changed since approval of plan {}, so the act did not run and the gate asked again: {}",
                    approved.id(),
                    reask.changed.join("; ")
                ));
                return Ok(None);
            }
            Spawned::Failed(_) if committed.is_none() && lapsed.is_some() => {
                // The envelope lapsed between the preview and the use: nothing was journaled, and
                // the human decides it now.
                let why = lapsed.unwrap_or_default();
                reask.notes.push(format!(
                    "the envelope that admitted the act no longer did at the moment of use, so the human was asked: {why}"
                ));
                reask.human = Some(why);
                return Ok(None);
            }
            Spawned::Failed(out) => {
                // Committed and not run: the probe settles it now, as it would after a cut.
                if committed.is_some()
                    && let Ok(lines) = self.recover_held()
                {
                    reask.notes.extend(lines);
                }
                return Err(Refusal::Failed(format!(
                    "{}{}",
                    out.content,
                    if reask.notes.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", reask.notes.join("; "))
                    }
                )));
            }
        };
        let receipt = committed
            .ok_or_else(|| Refusal::Failed("the act ran without its intent journaled".into()))?;
        let mut notes = std::mem::take(&mut reask.notes);
        if let Some(by) = admitted {
            notes.insert(0, format!("{by}: the human was not asked, and is shown it"));
        }
        self.finish(receipt, out, notes).map(Some)
    }

    /// The moment of use of an act `preview` admitted before the recheck: the uses ledger held,
    /// the envelopes read again. `None` when no envelope admitted it, and the human did.
    fn at_use(
        &self,
        planned: &Planned,
        pin: &Pin,
        preview: Option<&Admitted>,
    ) -> Result<Option<(mnemorph_ledger::Held<'_>, Admitted)>, AtUse> {
        if preview.is_none() {
            return Ok(None);
        }
        let uses = self
            .envelopes
            .as_ref()
            .map(EnvelopeGate::uses)
            .ok_or_else(|| AtUse::Lapsed("there is no envelope uses ledger".into()))?;
        let held = uses
            .hold()
            .map_err(|e| AtUse::Failed(format!("the envelope uses cannot be held: {e}")))?;
        let by = self
            .admit(planned, pin, Some(&held))
            .map_err(AtUse::Lapsed)?;
        Ok(Some((held, by)))
    }

    /// Show the human an act an envelope admitted: nothing outward happens unseen.
    fn show(&self, planned: &Planned, plan: &Fingerprint, by: &Admitted) {
        let mut ask = planned.ask(self.cx.body(), self.cx.race());
        ask.fingerprint = Some(plan.clone());
        self.cx.gate().admitted(&ask, &by.to_string());
    }

    /// Put the act to the human, bound to `plan`; `changed` is what made an earlier approval
    /// stale, `card` what the pin read of the target, `no_envelope` why no envelope admitted it.
    fn ask(
        &self,
        planned: &Planned,
        plan: &Fingerprint,
        changed: &[String],
        card: &[String],
        no_envelope: &str,
    ) -> Result<(), Refusal> {
        let mut ask = planned.ask(self.cx.body(), self.cx.race());
        ask.fingerprint = Some(plan.clone());
        ask.why.extend(card.iter().cloned());
        ask.why
            .push(format!("no envelope admits it: {no_envelope}"));
        if !changed.is_empty() {
            ask.why.insert(
                0,
                format!(
                    "the world changed since approval — {}; approving again binds to the world as it is now",
                    changed.join("; ")
                ),
            );
        }
        match self.cx.gate().ask(&ask) {
            Answer::Approve => Ok(()),
            Answer::Deny(why) => Err(Refusal::Denied(format!("{why} ({})", plan.short()))),
        }
    }

    /// Push a new branch and open a pull request from it, under one approval: one card shows both
    /// rendered commands — the push URLs and the commit, the branch, the base, the title and the
    /// description — bound to a fingerprint of both. Each act is still pinned, journaled, observed
    /// and receipted on its own, and undone on its own. The pull request runs only once the push's
    /// probe sees the branch at the pushed commit, its pin read again then; if the world moved
    /// since the approval but by the push itself, or the push had landed before, the pull request
    /// is asked for alone.
    ///
    /// # Errors
    ///
    /// As [`Self::act`], for the push or for both; what became of the pull request once the push
    /// ran is in [`Paired::pull`].
    pub fn push_and_open(&self, push: &Request, pull: &Request) -> Result<Paired, Refusal> {
        let refuse = |why: &str| Err(Refusal::Refused(why.to_owned()));
        Self::well_formed(push)?;
        Self::well_formed(pull)?;
        let (Act::PushNewBranch { name, .. }, Act::OpenPullRequest { head, .. }) =
            (&push.act, &pull.act)
        else {
            return refuse(
                "one approval covers a push of a new branch and a pull request from it, nothing else",
            );
        };
        if name != head {
            return refuse("the pull request's head is the branch the push makes");
        }
        if push.tells.is_some() || pull.tells.is_some() {
            return refuse("only a comment tells downstream of an incident");
        }
        self.may_act()?;
        let push_plan = self.planned(&push.act)?;
        let pull_plan = self.planned(&pull.act)?;
        let _held = self.hold()?;
        let mut reask = Reask {
            changed: Vec::new(),
            notes: self.recover_held()?,
            human: None,
        };
        for _ in 0..=MAX_REASKS {
            if let Some(paired) = self.pair_once(&push_plan, push, &pull_plan, pull, &mut reask)? {
                return Ok(paired);
            }
        }
        Err(Refusal::Failed(format!(
            "the world changed between each approval and the spawn, {} times: nothing ran; {}",
            MAX_REASKS + 1,
            reask.notes.join("; ")
        )))
    }

    /// One pass of [`Self::push_and_open`]; `None` when the world moved before the push.
    fn pair_once(
        &self,
        push_plan: &Planned,
        push: &Request,
        pull_plan: &Planned,
        pull: &Request,
        reask: &mut Reask,
    ) -> Result<Option<Paired>, Refusal> {
        let push_ready = match self.prepare(push_plan, push, reask, None)? {
            Prepared::Ready(ready) => ready,
            Prepared::Replayed(report) => {
                // The branch had landed already: the pull request is asked for alone.
                reask.notes.push(format!(
                    "the push had landed already (act {}): the pull request is asked for alone",
                    report.receipt
                ));
                let pull = self.run_held(pull_plan, pull, reask);
                return Ok(Some(Paired { push: report, pull }));
            }
        };
        let Pin::Push { urls, sha } = &push_ready.pin else {
            return Err(Refusal::Refused("the push's pin is not a push".into()));
        };
        let (Act::OpenPullRequest { repo, .. }, Pin::Forge { host }) = (
            pull_plan.act(),
            pull_plan.aim(self.cx.world()).map_err(Refusal::Refused)?,
        ) else {
            return Err(Refusal::Refused(
                "the pull request's pin is not a forge's".into(),
            ));
        };
        // The head the pull request names is the branch the push makes only when the push lands
        // in the pull request's repository.
        if !urls
            .iter()
            .filter_map(|u| forge_repo(u, &host))
            .any(|r| r.eq_ignore_ascii_case(repo))
        {
            return Err(Refusal::Refused(format!(
                "no push URL of the remote is {repo} on {host}: the pull request's head would not be the pushed branch"
            )));
        }
        let pull_ready = match self.prepare(pull_plan, pull, reask, Some(sha))? {
            Prepared::Ready(ready) => ready,
            Prepared::Replayed(report) => {
                return Err(Refusal::Refused(format!(
                    "the pull request of this request landed already (act {}) while its branch is not pushed: the human settles it (`mnemorph status`)",
                    report.receipt
                )));
            }
        };
        // What the pull request's plan reads of the world but the post journal, which the push
        // itself writes: the yardstick the pull request is held to once the push is observed.
        let yardstick = self.beside_journal(pull_plan)?;
        // 4. One gate for both.
        self.ask_pair(
            push_plan,
            &push_ready,
            pull_plan,
            &pull_ready,
            &reask.changed,
        )?;
        let Some(pushed) = self.execute(push_plan, push, push_ready, reask, None)? else {
            return Ok(None);
        };
        if pushed.state != State::Observed {
            let state = pushed.state;
            return Ok(Some(Paired {
                push: pushed,
                pull: Err(Refusal::Refused(format!(
                    "the push is {state}, not observed: the pull request was not opened"
                ))),
            }));
        }
        let pull = self.pull_after_push(pull_plan, pull, pull_ready, &yardstick, reask);
        Ok(Some(Paired { push: pushed, pull }))
    }

    /// The fingerprint of `planned` with the post's own journal read as absent: the push of a
    /// pair writes its rows there, and nothing else of the world may move.
    fn beside_journal(&self, planned: &Planned) -> Result<Fingerprint, Refusal> {
        let journal = self.journal.path().to_path_buf();
        Fingerprint::of_except(planned.command(), self.cx.world(), &[journal]).map_err(|e| {
            Refusal::Refused(format!(
                "the plan the pull request would be approved against could not be read: {e}"
            ))
        })
    }

    /// The pull request of an approved pair, its push observed: run under the pair's approval only
    /// while the world is as approved but for what the push itself wrote — the refs, which the
    /// pull request does not read, and the post journal's rows — `gh`'s host pinned again, and its
    /// pin, read again, holds the head at the pushed commit. Otherwise it is asked for alone,
    /// saying why.
    fn pull_after_push(
        &self,
        planned: &Planned,
        request: &Request,
        ready: Ready,
        yardstick: &Fingerprint,
        reask: &mut Reask,
    ) -> Result<Report, Refusal> {
        let world = self.cx.world();
        // The plan the spawn rechecks is read first, then the world held to the approval: a change
        // after the first read is seen by the second, and one after the second by the recheck.
        let fresh = Fingerprint::of(planned.command(), world).map_err(|e| {
            Refusal::Refused(format!(
                "the plan the pull request was approved against could not be read again: {e}"
            ))
        })?;
        let now = self.beside_journal(planned)?;
        // The refs the push moved are the target's; every other part must be as approved.
        let masked = Fingerprint {
            target: yardstick.target.clone(),
            ..now
        };
        let mut moved = yardstick.changes(&masked);
        let host_now = planned.aim(world).map_err(Refusal::Refused)?;
        if host_now != ready.pin {
            moved.push("gh's host changed".into());
        }
        if !moved.is_empty() {
            reask.changed = moved;
            reask.notes.push(
                "the world changed since the pair was approved: the pull request is asked for alone"
                    .into(),
            );
            return self.run_held(planned, request, reask);
        }
        // The pin read again, the push now this post's: the head must be at the pushed commit.
        let card = match &ready.pin {
            Pin::Forge { host } => self.pull_pin(planned.act(), host, None)?,
            _ => {
                return Err(Refusal::Refused(
                    "the pull request's pin is not a forge's".into(),
                ));
            }
        };
        let again = Ready {
            plan: fresh,
            pinned: Timestamp::now(),
            card,
            ..ready
        };
        match self.execute(planned, request, again, reask, None)? {
            Some(report) => Ok(report),
            None => self.run_held(planned, request, reask),
        }
    }

    /// The one card of a pair: both commands, every line of both, and a fingerprint of both.
    fn ask_pair(
        &self,
        push: &Planned,
        push_ready: &Ready,
        pull: &Planned,
        pull_ready: &Ready,
        changed: &[String],
    ) -> Result<(), Refusal> {
        let (body, race) = (self.cx.body(), self.cx.race());
        let mut ask = push.ask(body, race);
        let second = pull.ask(body, race);
        ask.act = format!("{}\n{}", push.command(), pull.command());
        if second.class == ToolClass::Destructive {
            ask.class = ToolClass::Destructive;
        }
        let shown = match &push_ready.pin {
            Pin::Push { urls, sha } => format!(
                "pushes commit {sha} to {}",
                urls.iter()
                    .map(|u| redact(u))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => String::new(),
        };
        ask.why.insert(
            0,
            "two acts under one approval: the push, then — only once a probe sees the branch at the pushed commit — the pull request; each is journaled, observed and undone on its own".into(),
        );
        ask.why.insert(1, shown);
        ask.why.extend(push_ready.card.iter().cloned());
        ask.why.extend(second.why);
        ask.why.extend(pull_ready.card.iter().cloned());
        ask.outside.extend(second.outside);
        let plan = both(&push_ready.plan, &pull_ready.plan);
        ask.fingerprint = Some(plan.clone());
        ask.why.push(PAIR_ASKS.to_owned());
        if !changed.is_empty() {
            ask.why.insert(
                0,
                format!(
                    "the world changed since approval — {}; approving again binds to the world as it is now",
                    changed.join("; ")
                ),
            );
        }
        match self.cx.gate().ask(&ask) {
            Answer::Approve => Ok(()),
            Answer::Deny(why) => Err(Refusal::Denied(format!("{why} ({})", plan.short()))),
        }
    }

    /// The act ran: journal its result, keep what it left, and probe it. Steps 8 to 10.
    fn finish(
        &self,
        mut receipt: Receipt,
        out: Outcome,
        notes: Vec<String>,
    ) -> Result<Report, Refusal> {
        // 8. Ran: its exit and what it left.
        self.journal
            .ran(&mut receipt, out.meta.exit, &out.content)
            .map_err(|e| Refusal::Failed(format!("the result could not be journaled: {e}")))?;
        if let Some(done) = receipt.done() {
            let mut with = self
                .kept()?
                .into_iter()
                .find(|i| i.act == receipt.id())
                .ok_or_else(|| Refusal::Failed("the act's intent is gone".into()))?;
            with.left = Some(Left::of(done));
            let _ = self.keep(&with);
        }
        // 9. Observe: the probe, never the exit, says what happened.
        if receipt.state() == State::PendingObservation {
            let before = receipt.done().cloned();
            self.journal
                .observe(&mut receipt, self.probe, PROBE_TIMEOUT)
                .map_err(|e| Refusal::Failed(format!("the receipt could not be journaled: {e}")))?;
            // What only the probe could tell — a pull request's number — is kept for the undo.
            if let Some(done) = receipt.done()
                && before.as_ref() != Some(done)
                && let Some(mut with) = self.kept()?.into_iter().find(|i| i.act == receipt.id())
            {
                with.left = Some(Left::of(done));
                let _ = self.keep(&with);
            }
        }
        // 10. The receipt.
        Ok(Report {
            receipt: receipt.id().to_owned(),
            state: receipt.state(),
            replayed: false,
            exit: out.meta.exit,
            output: out.content,
            notes,
        })
    }

    /// The stored receipt of a request already made under `key`, or the refusal of a conflicting
    /// one; `None` when the request is to be acted as new.
    fn replay(
        &self,
        key: &str,
        digest: &str,
        reask: &mut Reask,
    ) -> Result<Option<Report>, Refusal> {
        let standing = self.standing()?;
        let prior = self.kept()?.into_iter().rev().find_map(|i| {
            let row = standing.iter().find(|r| r.act == i.act && i.key == key)?;
            Some((i, row.state))
        });
        let Some((intent, state)) = prior else {
            return Ok(None);
        };
        if intent.request != digest {
            return Err(Refusal::Refused(format!(
                "the key of this request was given to another request (act {}): a key names one request, so this one is refused; give it its own key",
                intent.act
            )));
        }
        match state {
            State::Observed | State::Settled => {
                // Its effect is seen again before it is called done: the human may have
                // removed it since.
                let (mut probes, mut seen) = (Vec::new(), Vec::new());
                match landed(&intent, self.probe, &mut probes, &mut seen) {
                    Landing::Verified(_) => Ok(Some(Report {
                        receipt: intent.act.clone(),
                        state,
                        replayed: true,
                        exit: None,
                        output: String::new(),
                        notes: std::mem::take(&mut reask.notes),
                    })),
                    gone => {
                        let why = match gone {
                            Landing::Unknown(why) => {
                                format!("an observed act asked for again cannot be seen: {why}")
                            }
                            _ => "an observed act asked for again: its effect is gone".to_owned(),
                        };
                        self.settle(&intent.act, State::Reconcile, why.clone(), &probes, &seen);
                        Err(Refusal::Refused(format!(
                            "act {} of this request: {why}; the human settles it (`mnemorph status`)",
                            intent.act
                        )))
                    }
                }
            }
            State::Absent => {
                reask.notes.push(format!(
                    "act {} of this request never landed: it is asked again as a new act",
                    intent.act
                ));
                Ok(None)
            }
            State::Corrected => {
                reask.notes.push(format!(
                    "act {} of this request was put back: it is asked again as a new act",
                    intent.act
                ));
                Ok(None)
            }
            state => Err(Refusal::Refused(format!(
                "act {} of this request is {state}: the human settles it before it is asked again (`mnemorph status`)",
                intent.act
            ))),
        }
    }

    /// Whether this runner may act at all: a kernel guard in force (or unguarded running allowed
    /// by the human), and the body the Kijin of the journal's post.
    fn may_act(&self) -> Result<(), Refusal> {
        if let mnemorph_tools::Sandbox::None(reason) = self.cx.sandbox()
            && !self.cx.allows_unguarded()
        {
            return Err(Refusal::Refused(format!(
                "no kernel guard is in force ({reason}), so the act did not run; to run unguarded, set `sandbox.allow_unguarded = true` in your global config"
            )));
        }
        if self.cx.race().base() != Race::Kijin
            || Post::of(self.cx.body()) != Some(self.journal.post)
        {
            return Err(Refusal::Refused(format!(
                "only the Kijin of the {} post runs its typed acts",
                self.journal.post.creature()
            )));
        }
        Ok(())
    }

    fn place(&self) -> Place {
        Place {
            world: self.cx.world().to_path_buf(),
            home: self.cx.home().to_path_buf(),
            cwd: self.cx.world().to_path_buf(),
            roots: self.cx.deploy_roots().to_vec(),
        }
    }

    /// Whether a new act may land at `pin`: no open incident of the post is on that target — but
    /// for the comment that tells downstream of the one incident open there, on the pull request
    /// its act concerns.
    fn clear(&self, act: &Act, pin: &Pin, tells: Option<&str>) -> Result<(), Refusal> {
        let open = self.journal.blocking(act, pin).map_err(Refusal::Refused)?;
        let Some(of) = tells else {
            return match open.first() {
                Some(incident) => Err(Refusal::Refused(format!(
                    "the incident on act {} is open on this target, missing {}: no new act lands here until it closes; only a comment that `tells` of it is admitted (`mnemorph status` lists it)",
                    incident.act,
                    incident.missing().join(", ")
                ))),
                None => Ok(()),
            };
        };
        let incident = self
            .journal
            .incident(of)
            .map_err(|e| Refusal::Refused(format!("the post's incidents cannot be read: {e}")))?
            .filter(crate::Incident::is_open)
            .ok_or_else(|| {
                Refusal::Refused(format!(
                    "act {} has no open incident of the {} post to tell of",
                    excerpt(of.as_bytes(), 64),
                    self.journal.post.creature()
                ))
            })?;
        if let Some(other) = open.iter().find(|i| i.act != of) {
            return Err(Refusal::Refused(format!(
                "the incident on act {} is open on this target: a comment telling of another incident does not land here",
                other.act
            )));
        }
        self.speaks_to(&incident, act, pin).map_err(|why| {
            Refusal::Refused(format!(
                "a comment telling of the incident on act {of} goes on the pull request its act concerns: {why}"
            ))
        })
    }

    /// Whether a comment at `pin` is on the pull request the incident's act concerns: the act's
    /// own, for a label or a comment; for a branch, one whose head is that branch, in the
    /// repository a pinned push URL names on the comment's host — read through the probe.
    fn speaks_to(&self, incident: &crate::Incident, act: &Act, pin: &Pin) -> Result<(), String> {
        let (Act::Comment { repo, pr, .. }, Pin::Forge { host }) = (act, pin) else {
            return Err("only a comment tells downstream".into());
        };
        match &incident.typed {
            Some((Act::AddLabel { .. } | Act::Comment { .. } | Act::OpenPullRequest { .. }, _)) => {
                if incident.same_target(act, pin) {
                    Ok(())
                } else {
                    Err("it is not the act's pull request".into())
                }
            }
            Some((Act::PushNewBranch { name, .. }, Pin::Push { urls, .. })) => {
                let (mut probes, mut seen) = (Vec::new(), Vec::new());
                branch_pr(
                    name,
                    urls,
                    repo,
                    *pr,
                    host,
                    self.probe,
                    &mut probes,
                    &mut seen,
                )
            }
            _ => Err("the incident's act is not known".into()),
        }
    }

    /// The act `receipt` as an undo needs it: in `observed` or `reconcile`, not itself an undo,
    /// its intent kept, and what it left known.
    fn undoable(&self, receipt: &str) -> Result<(Intent, Done), Refusal> {
        let row = self.standing()?.into_iter().find(|r| r.act == receipt);
        let state = row.as_ref().map(|r| r.state);
        if !matches!(state, Some(State::Observed | State::Reconcile)) {
            return Err(Refusal::Refused(format!(
                "act {} is {}: only an act observed or in reconcile is undone",
                excerpt(receipt.as_bytes(), 64),
                state.map_or_else(|| "not in this post's journal".into(), |s| s.to_string())
            )));
        }
        if row.is_some_and(|r| r.undoes.is_some()) {
            return Err(Refusal::Refused(format!(
                "act {receipt} is an undo: nothing undoes an undo but the human"
            )));
        }
        let intent = self
            .kept()?
            .into_iter()
            .find(|i| i.act == receipt)
            .ok_or_else(|| Refusal::Refused(format!("act {receipt} has no intent kept")))?;
        let done = match (&intent.left, &intent.typed, &intent.pin) {
            (Some(left), _, _) => left.done(),
            (None, Typed::AddLabel { .. }, _) => Done::Labelled,
            // The lease is on the commit the act pushed: a branch elsewhere is never deleted.
            (None, Typed::PushNewBranch { .. }, Pinned::Push { sha, .. }) => {
                Done::Pushed { sha: sha.clone() }
            }
            (None, _, _) => {
                return Err(Refusal::Refused(format!(
                    "act {receipt} left nothing recorded to undo"
                )));
            }
        };
        Ok((intent, done))
    }

    /// Whether what the act left is still its own, read now: a label's one add since the pin is
    /// the act's, by `gh`'s login, and still there; a comment is by `gh`'s login, on the act's pull
    /// request, with the act's body; a pull request is open, unmerged, by `gh`'s login, from the
    /// act's head into its base, with its title and description. A branch is leased on its commit
    /// instead. Anything uncertain
    /// moves the act to `reconcile`, and nothing is undone.
    fn still_its_own(&self, intent: &Intent) -> Result<(), Refusal> {
        let (mut probes, mut seen) = (Vec::new(), Vec::new());
        let held = match &intent.typed {
            Typed::AddLabel { .. } => label_is_ours(intent, self.probe, &mut probes, &mut seen),
            Typed::Comment { .. } => comment_is_ours(intent, self.probe, &mut probes, &mut seen),
            Typed::OpenPullRequest { .. } => match (&intent.pin, &intent.left) {
                (Pinned::Forge { host }, Some(Left::Opened { number })) => {
                    let act = intent.typed.act();
                    forge::Wanted::of(&act).map_or_else(
                        || Err("the act opens no pull request".into()),
                        |want| {
                            forge::still_open_and_ours(
                                host,
                                &want,
                                *number,
                                self.probe,
                                &mut probes,
                                &mut seen,
                            )
                        },
                    )
                }
                _ => Err("the act is not a pull request with a known number".into()),
            },
            Typed::PushNewBranch { .. } => Ok(()),
        };
        held.map_err(|why| {
            let why = format!("not undone: {why}");
            self.settle(&intent.act, State::Reconcile, why.clone(), &probes, &seen);
            Refusal::Refused(why)
        })
    }

    /// A pushed branch is not deleted while a pull request this post opened from it is open:
    /// deleting its head would close it, unrecorded. Undo the pull request first.
    fn no_pull_open_on(&self, intent: &Intent) -> Result<(), Refusal> {
        let Typed::PushNewBranch { name, .. } = &intent.typed else {
            return Ok(());
        };
        let standing = self.standing()?;
        for pr in self.kept()? {
            let (
                Typed::OpenPullRequest { repo, head, .. },
                Pinned::Forge { host },
                Some(Left::Opened { number }),
            ) = (&pr.typed, &pr.pin, &pr.left)
            else {
                continue;
            };
            let holds = standing
                .iter()
                .any(|r| r.act == pr.act && may_stand(r.state));
            if head != name || !holds {
                continue;
            }
            let (mut probes, mut seen) = (Vec::new(), Vec::new());
            match forge::pull(host, repo, *number, self.probe, &mut probes, &mut seen) {
                Ok(p) if p.state != "open" => {}
                Ok(_) => {
                    return Err(Refusal::Refused(format!(
                        "act {} opened {repo}#{number} from branch {name}, and it is open: undo it first, or deleting its head closes it unrecorded",
                        pr.act
                    )));
                }
                Err(why) => {
                    return Err(Refusal::Refused(format!(
                        "whether {repo}#{number}, opened by act {} from branch {name}, is still open cannot be read: {why}",
                        pr.act
                    )));
                }
            }
        }
        Ok(())
    }

    /// The undo of act `receipt`, rendered from the act's own fields — the undo of an observed
    /// act, rendered only once a probe shows that what it left is still its own. Nothing is run or
    /// asked; [`Self::undo`] runs it.
    ///
    /// # Errors
    ///
    /// The act is not this post's act observed or in reconcile, or what it left is no longer
    /// certainly its own (it is moved to `reconcile`).
    pub fn undo_command(&self, receipt: &str) -> Result<String, Refusal> {
        let _held = self.hold()?;
        let (intent, done) = self.undoable(receipt)?;
        self.still_its_own(&intent)?;
        let planned = crate::outward::plan_undo(&intent.typed.act(), &done, &self.place())
            .map_err(|e| Refusal::Refused(e.to_string()))?;
        Ok(planned.command().to_owned())
    }

    /// Undo act `receipt` through the whole pipeline: the post check; the undo's fingerprint; the
    /// target still where the act went, and what it left still its own; the human, always — no
    /// envelope covers an undo; the recheck; the incident opened (a rollback) and the undo's
    /// intent journaled; the harness-rendered undo, leased; `ran`; and a probe that must see the
    /// effect gone. Only then is the undo `observed`, the act `corrected`, and the incident given
    /// its first two receipts. An undo that fails, exits other than 0, or whose effect is still
    /// seen is `reconcile`, never undone.
    ///
    /// # Errors
    ///
    /// [`Refusal::Refused`] before the gate, [`Refusal::Denied`] by the human, [`Refusal::Failed`]
    /// when it cannot run or be journaled.
    pub fn undo(&self, receipt: &str) -> Result<Report, Refusal> {
        self.may_act()?;
        let _held = self.hold()?;
        let notes = self.recover_held()?;
        let (intent, done) = self.undoable(receipt)?;
        self.no_pull_open_on(&intent)?;
        let undo = crate::outward::plan_undo(&intent.typed.act(), &done, &self.place())
            .map_err(|e| Refusal::Refused(e.to_string()))?;
        check_class(self.cx.race(), undo.class()).map_err(|e| Refusal::Refused(e.to_string()))?;
        // 1. The post: the act undone must be the post's.
        undo.check_post(self.cx.body())
            .map_err(|e| Refusal::Refused(e.to_string()))?;
        let mut reask = Reask {
            changed: Vec::new(),
            notes,
            human: None,
        };
        for _ in 0..=MAX_REASKS {
            if let Some(report) = self.undo_once(&intent, &undo, &mut reask)? {
                return Ok(report);
            }
        }
        Err(Refusal::Failed(format!(
            "the world changed between each approval and the spawn, {} times: the undo did not run; {}",
            MAX_REASKS + 1,
            reask.notes.join("; ")
        )))
    }

    /// One pass of an undo from the fingerprint to its receipt; `None` when the world moved before
    /// the spawn.
    #[allow(clippy::too_many_lines)]
    fn undo_once(
        &self,
        intent: &Intent,
        undo: &Planned,
        reask: &mut Reask,
    ) -> Result<Option<Report>, Refusal> {
        let world = self.cx.world();
        let of = intent.act.as_str();
        // 2. The plan the approval binds to.
        let plan = Fingerprint::of(undo.command(), world).map_err(|e| {
            Refusal::Refused(format!(
                "the plan this undo would be approved against could not be read: {e}"
            ))
        })?;
        // 3. Where it goes must be where the act went: a remote or host since retargeted is
        //    refused, so an undo never lands on what the act never touched.
        let pin = intent.pin.pin();
        let now = undo.aim(world).map_err(Refusal::Refused)?;
        let same = match (&now, &pin) {
            (Pin::Push { urls: a, .. }, Pin::Push { urls: b, .. }) => {
                a.len() == b.len() && a.iter().all(|u| b.contains(u))
            }
            (Pin::Forge { host: a }, Pin::Forge { host: b }) => a == b,
            _ => false,
        };
        if !same {
            return Err(Refusal::Refused(format!(
                "act {of} landed at {}, and the undo would go to {}: not undone",
                pin_shown(&pin).join(", "),
                pin_shown(&now).join(", ")
            )));
        }
        // What it left may be gone already — the human removed it by hand: then nothing runs.
        let (mut probes, mut seen) = (Vec::new(), Vec::new());
        if gone(intent, self.probe, &mut probes, &mut seen) == Landing::Absent {
            return self.already_gone(intent, undo, &plan, reask).map(Some);
        }
        self.still_its_own(intent)?;
        // 4. The human, always: no envelope covers an undo.
        self.ask(
            undo,
            &plan,
            &reask.changed,
            &[],
            "an undo always asks the human",
        )?;
        let short = plan.short();
        let mut committed: Option<String> = None;
        let mut before = || {
            // What the act left is read again after the recheck: edited while the human read the
            // card, it is not undone.
            self.still_its_own(intent).map_err(|e| e.to_string())?;
            self.journal
                .open_rollback(of)
                .map_err(|e| format!("the incident could not be opened: {e}"))?;
            let id = receipt_id(undo.command());
            let mut row = undo_row(&id, of, State::Committed);
            row.command = Some(command_line(undo.command()));
            row.target = pin_shown(&pin);
            row.plan = Some(short.clone());
            row.expect = Some(format!("the effect of act {of} gone"));
            self.journal
                .append(&row)
                .map_err(|e| format!("the undo's intent could not be journaled: {e}"))?;
            committed = Some(id);
            Ok(())
        };
        let out = match self
            .hands
            .spawn(undo, self.cx, plan, &mut before, undo.stdin())
        {
            Spawned::Ran(out) => out,
            Spawned::Changed { approved, now } => {
                reask.changed = approved.changes(&now);
                reask.notes.push(format!(
                    "the world changed since approval of plan {}, so the undo did not run and the gate asked again: {}",
                    approved.id(),
                    reask.changed.join("; ")
                ));
                return Ok(None);
            }
            Spawned::Failed(out) => {
                if let Some(id) = &committed {
                    let mut row = undo_row(id, of, State::Reconcile);
                    row.why = Some(format!(
                        "the undo could not run: {}",
                        excerpt(out.content.as_bytes(), 300)
                    ));
                    let _ = self.journal.append(&row);
                }
                return Err(Refusal::Failed(out.content));
            }
        };
        let id = committed
            .ok_or_else(|| Refusal::Failed("the undo ran without its intent journaled".into()))?;
        let journal = |row: &Row| {
            self.journal
                .append(row)
                .map_err(|e| Refusal::Failed(format!("the undo could not be journaled: {e}")))
        };
        let mut report = |state: State| Report {
            receipt: id.clone(),
            state,
            replayed: false,
            exit: out.meta.exit,
            output: out.content.clone(),
            notes: std::mem::take(&mut reask.notes),
        };
        // 8. Ran: an undo that did not exit 0 did something unknown; it is never undone.
        if out.meta.exit != Some(0) {
            let mut row = undo_row(&id, of, State::Reconcile);
            row.exit = out.meta.exit;
            row.why = Some(format!(
                "the undo {}: what it did outside is unknown; act {of} is not undone",
                out.meta.exit.map_or_else(
                    || "was ended by a signal".to_owned(),
                    |c| format!("exited {c}")
                )
            ));
            journal(&row)?;
            return Ok(Some(report(State::Reconcile)));
        }
        let mut row = undo_row(&id, of, State::PendingObservation);
        row.exit = Some(0);
        row.why = Some("ran; the effect is not yet seen gone".into());
        journal(&row)?;
        let state = self.conclude(intent, &id)?;
        let mut done = report(state);
        done.notes.push(closing_note(of));
        Ok(Some(done))
    }

    /// Observe an undo (step 9): only a probe that sees the effect gone makes it `observed`; then the
    /// act's own row is superseded by `corrected` (receipt 2) and the undo's receipt is recorded
    /// in the incident (receipt 1). Anything else is `reconcile`.
    fn conclude(&self, intent: &Intent, id: &str) -> Result<State, Refusal> {
        let of = intent.act.as_str();
        let journal = |row: &Row| {
            self.journal
                .append(row)
                .map_err(|e| Refusal::Failed(format!("the undo could not be journaled: {e}")))
        };
        let (mut probes, mut seen) = (Vec::new(), Vec::new());
        let gone = gone(intent, self.probe, &mut probes, &mut seen);
        let mut row = undo_row(id, of, State::Reconcile);
        row.probe = (!probes.is_empty()).then(|| probes.join(" ; "));
        row.seen =
            (!seen.is_empty()).then(|| excerpt(seen.join(" ; ").as_bytes(), crate::SEEN_BYTES));
        row.expect = Some(format!("the effect of act {of} gone"));
        let state = match gone {
            Landing::Absent => {
                row.why = Some("the probe saw the effect gone".into());
                State::Observed
            }
            Landing::Verified(_) => {
                row.why = Some(format!("the probe still sees the effect of act {of}"));
                State::Reconcile
            }
            Landing::Unknown(why) => {
                row.why = Some(format!("the probe cannot tell the effect is gone: {why}"));
                State::Reconcile
            }
        };
        row.state = state;
        journal(&row)?;
        if state == State::Observed {
            // (2) the act's own row, superseded: appended, never edited.
            let mut corrected = Row::new(of, State::Corrected);
            corrected.by = Some(id.to_owned());
            corrected.why = Some(format!(
                "put back by undo {id}, whose probe saw the effect gone"
            ));
            journal(&corrected)?;
            // (1) the undo's receipt, counted only while the witness holds it observed.
            let undone = Entry::Undone {
                act: of.to_owned(),
                receipt: id.to_owned(),
                at: Timestamp::now(),
            };
            self.journal.record(&undone).map_err(|e| {
                Refusal::Failed(format!("the incident's receipt could not be recorded: {e}"))
            })?;
        }
        Ok(state)
    }

    /// The undo of an act whose effect a probe already sees gone — the human removed it by hand:
    /// nothing runs. The human approves recording it, and the probe must see it gone again after
    /// the approval; only then is it `observed`, with its receipts.
    fn already_gone(
        &self,
        intent: &Intent,
        undo: &Planned,
        plan: &Fingerprint,
        reask: &mut Reask,
    ) -> Result<Report, Refusal> {
        let of = intent.act.as_str();
        let mut ask = undo.ask(self.cx.body(), self.cx.race());
        ask.fingerprint = Some(plan.clone());
        ask.act = format!("record act {of} as undone; nothing runs");
        ask.why.insert(
            0,
            format!(
                "a probe sees the effect of act {of} already gone: approving records its undo, and runs nothing"
            ),
        );
        ask.why.push(UNDO_ASKS.to_owned());
        if let Answer::Deny(why) = self.cx.gate().ask(&ask) {
            return Err(Refusal::Denied(format!("{why} ({})", plan.short())));
        }
        self.journal
            .open_rollback(of)
            .map_err(|e| Refusal::Failed(format!("the incident could not be opened: {e}")))?;
        let id = receipt_id(&format!("already undone {of}"));
        let mut row = undo_row(&id, of, State::Committed);
        row.command = Some("nothing runs: the effect was already gone".into());
        row.target = pin_shown(&intent.pin.pin());
        row.plan = Some(plan.short());
        row.expect = Some(format!("the effect of act {of} gone"));
        self.journal
            .append(&row)
            .map_err(|e| Refusal::Failed(format!("the undo could not be journaled: {e}")))?;
        let state = self.conclude(intent, &id)?;
        let mut notes = std::mem::take(&mut reask.notes);
        notes.push(closing_note(of));
        Ok(Report {
            receipt: id,
            state,
            replayed: false,
            exit: None,
            output: String::new(),
            notes,
        })
    }
}

/// Whether an act at `state` may still stand outside: seen landed (`observed`), uncertain
/// (`reconcile`), or settled by the human as fine (`settled`). One set for every question of what
/// this post's earlier acts left — the head a pull request opens from, the pull request a branch
/// still carries — so no act counts as standing for one and gone for the other.
fn may_stand(state: State) -> bool {
    matches!(state, State::Observed | State::Reconcile | State::Settled)
}

/// What every undo's report says of the incident it opened.
fn closing_note(of: &str) -> String {
    format!(
        "the incident on act {of} stays open until it is undone, its journal corrected, downstream told and a lesson written"
    )
}

/// A row of the undo of act `of`.
fn undo_row(id: &str, of: &str, state: State) -> Row {
    let mut row = Row::new(id, state);
    row.undoes = Some(of.to_owned());
    row
}

/// What the journal says of a pin: URLs with any credentials blanked.
fn pin_shown(pin: &Pin) -> Vec<String> {
    match pin {
        Pin::Push { urls, .. } => urls.iter().map(|u| redact(u)).collect(),
        Pin::Forge { host } => vec![format!("host {host}")],
        Pin::Unpinned => Vec::new(),
    }
}

/// Whether pull request `repo#pr` on `host` has the branch `name` as its head, in a repository one
/// of the act's push `urls` names on that host.
#[allow(clippy::too_many_arguments)]
fn branch_pr(
    name: &str,
    urls: &[String],
    repo: &str,
    pr: u64,
    host: &str,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Head {
        #[serde(rename = "ref")]
        branch: String,
        repo: Option<Full>,
    }
    #[derive(Deserialize)]
    struct Full {
        full_name: String,
    }
    #[derive(Deserialize)]
    struct Pull {
        head: Head,
    }
    let pushed: Vec<String> = urls.iter().filter_map(|u| forge_repo(u, host)).collect();
    if pushed.is_empty() {
        return Err(format!(
            "no push URL of the branch is a repository on {host}: nothing ties a pull request to it"
        ));
    }
    let pull: Pull = serde_json::from_slice(&probed(
        probe,
        &forge_get(host, &format!("repos/{repo}/pulls/{pr}")),
        probes,
        seen,
    )?)
    .map_err(|e| format!("the pull request does not read: {e}"))?;
    let head = pull.head.repo.map(|r| r.full_name).unwrap_or_default();
    if pull.head.branch == name && pushed.iter().any(|r| r.eq_ignore_ascii_case(&head)) {
        Ok(())
    } else {
        Err(format!(
            "{repo}#{pr}'s head is {}:{}, not {name} on {}",
            excerpt(head.as_bytes(), 100),
            excerpt(pull.head.branch.as_bytes(), 100),
            pushed.join(", ")
        ))
    }
}

/// Whether a comment is still the act's own: by the login `gh` acts as, on the act's pull
/// request, with the act's body, under the id it was given.
fn comment_is_ours(
    intent: &Intent,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<(), String> {
    let (Typed::Comment { repo, pr, body }, Pinned::Forge { host }, Some(Left::Commented { id })) =
        (&intent.typed, &intent.pin, &intent.left)
    else {
        return Err("the act is not a comment with a known id".into());
    };
    let me = login(host, probe, probes, seen)?;
    let path = format!("repos/{repo}/issues/comments/{id}");
    let c: Comment = serde_json::from_slice(&probed(probe, &forge_get(host, &path), probes, seen)?)
        .map_err(|e| format!("the comment does not read: {e}"))?;
    let on = format!("/repos/{repo}/issues/{pr}").to_ascii_lowercase();
    if c.id != *id || !c.issue_url.to_ascii_lowercase().ends_with(&on) {
        return Err(format!("comment {id} is not on {repo}#{pr}"));
    }
    if c.user.as_ref().is_none_or(|u| u.login != me) {
        return Err(format!(
            "comment {id} is by {}, not by {me}: it is not this act's",
            c.user.as_ref().map_or("someone unknown", |u| &u.login)
        ));
    }
    if sha256(&c.body) != sha256(body) {
        return Err(format!("comment {id} was edited since the act"));
    }
    Ok(())
}

/// The `outward` tool's arguments: one typed act, by kind.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Args {
    PushNewBranch {
        remote: String,
        name: String,
        #[serde(default)]
        key: Option<String>,
    },
    AddLabel {
        repo: String,
        pr: u64,
        label: String,
        #[serde(default)]
        key: Option<String>,
    },
    Comment {
        repo: String,
        pr: u64,
        body: String,
        #[serde(default)]
        key: Option<String>,
        #[serde(default)]
        tells: Option<String>,
    },
    OpenPullRequest {
        repo: String,
        base: String,
        head: String,
        title: String,
        body: String,
        #[serde(default)]
        remote: Option<String>,
        #[serde(default)]
        key: Option<String>,
    },
    Undo {
        receipt: String,
    },
}

/// What the tool is asked: an act, or the undo of one.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Act(Request),
    /// A push of the head, then the pull request, under one approval.
    Pair(Request, Request),
    Undo(String),
}

impl Args {
    fn call(self) -> Call {
        Call::Act(match self {
            Args::PushNewBranch { remote, name, key } => Request {
                act: Act::PushNewBranch { remote, name },
                key,
                tells: None,
            },
            Args::AddLabel {
                repo,
                pr,
                label,
                key,
            } => Request {
                act: Act::AddLabel { repo, pr, label },
                key,
                tells: None,
            },
            Args::Comment {
                repo,
                pr,
                body,
                key,
                tells,
            } => Request {
                act: Act::Comment { repo, pr, body },
                key,
                tells,
            },
            Args::OpenPullRequest {
                repo,
                base,
                head,
                title,
                body,
                remote,
                key,
            } => {
                let pull = Request {
                    act: Act::OpenPullRequest {
                        repo,
                        base,
                        head: head.clone(),
                        title,
                        body,
                    },
                    key,
                    tells: None,
                };
                let Some(remote) = remote else {
                    return Call::Act(pull);
                };
                let push = Request {
                    act: Act::PushNewBranch { remote, name: head },
                    key: None,
                    tells: None,
                };
                return Call::Pair(push, pull);
            }
            Args::Undo { receipt } => return Call::Undo(receipt),
        })
    }
}

/// The `outward` tool: a Kijin's typed outward act, run by the harness through [`Runner`].
#[derive(Debug, Clone, Copy, Default)]
pub struct OutwardTool;

/// The tool's name.
pub const OUTWARD: &str = "outward";

impl Tool for OutwardTool {
    fn name(&self) -> &'static str {
        OUTWARD
    }

    fn description(&self) -> &'static str {
        "Do one typed outward act of your post: push a new branch, open a pull request, label a pull request, or comment on one — or undo one by its receipt. `open-pull-request` with `remote` pushes `head` from HEAD first and opens the pull request from it, both under one approval, each with its own receipt. The harness renders the command and its undo, pins the target, asks the human, journals the intent, runs it, and probes that it landed (or, for an undo, that it is gone); you get the receipt. The same request again returns its receipt without acting; name a request with `key` to have a different request under the same name refused. An undo opens an incident: no new act lands on its target until it is undone, its journal corrected, downstream told (a comment with `tells` set to the undone act's receipt) and a lesson written. Any other outward command goes through `bash`, where it is destructive."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "kind": {"type": "string", "enum": ["push-new-branch", "open-pull-request", "add-label", "comment", "undo"], "description": "The act. `push-new-branch` takes `remote` and `name`; `open-pull-request` takes `repo`, `base`, `head`, `title`, `body` and, to push `head` first under the same approval, `remote`; `add-label` takes `repo`, `pr` and `label`; `comment` takes `repo`, `pr`, `body` and, to tell downstream of an incident, `tells`; `undo` takes `receipt`."},
                "remote": {"type": "string", "description": "push-new-branch: a configured remote's name (`origin`). open-pull-request, optional: push `head` from HEAD to this remote first; its push URL must be `repo`."},
                "base": {"type": "string", "description": "open-pull-request: the branch to merge into; it must exist."},
                "head": {"type": "string", "description": "open-pull-request: the branch to propose, a plain name in `repo`; it must exist (pushed by this post, or by anyone), unless `remote` pushes it now."},
                "title": {"type": "string", "maxLength": 256, "description": "open-pull-request: the title, one line."},
                "name": {"type": "string", "description": "push-new-branch: the new branch, a plain name (`feat/undo`); it must not exist yet. HEAD's commit is pushed."},
                "repo": {"type": "string", "description": "open-pull-request, add-label, comment: the repository, `owner/name`."},
                "pr": {"type": "integer", "minimum": 1, "description": "add-label, comment: the pull request's number."},
                "label": {"type": "string", "maxLength": 50, "description": "add-label: the label."},
                "body": {"type": "string", "description": "comment: the comment. open-pull-request: the description."},
                "key": {"type": "string", "maxLength": KEY_CHARS, "description": "Optional: the request's name. The same name with a different request is refused."},
                "tells": {"type": "string", "description": "comment, optional: the receipt of the act whose open incident this comment tells downstream of."},
                "receipt": {"type": "string", "description": "undo: the receipt of the act to put back."}
            },
            "required": ["kind"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Outward
    }

    fn example(&self) -> Value {
        json!({"kind": "add-label", "repo": "octo/app", "pr": 42, "label": "ready"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let call = match serde_json::from_value::<Args>(raw) {
            Ok(a) => a.call(),
            Err(e) => {
                return Outcome::error(format!(
                    "the arguments to {OUTWARD} do not fit its schema: {e}"
                ));
            }
        };
        let Some(post) = Post::of(cx.body()) else {
            return Outcome::error(format!(
                "`{}` holds no post: only a Kijin of a post does its typed outward acts",
                cx.body()
            ));
        };
        let journal = PostJournal::of(cx.world(), post);
        let observer = crate::Observer::new(cx.sandbox().clone(), cx.home());
        let envelopes = crate::EnvelopeGate::of(cx.world());
        let runner = Runner::new(cx, journal, &observer, &Sandboxed).with_envelopes(envelopes);
        match call {
            Call::Act(request) => outcome(runner.act(&request)),
            Call::Pair(push, pull) => paired(runner.push_and_open(&push, &pull)),
            Call::Undo(receipt) => outcome(runner.undo(&receipt)),
        }
    }
}

/// What the body reads of a pair's result: the push's receipt, then the pull request's.
fn paired(result: Result<Paired, Refusal>) -> Outcome {
    let Paired { push, pull } = match result {
        Ok(p) => p,
        Err(refusal) => return outcome(Err(refusal)),
    };
    let first = outcome(Ok(push));
    let second = outcome(pull);
    let mut out = Outcome::ok(
        format!(
            "the push — {}\nthe pull request — {}",
            first.content, second.content
        ),
        ToolClass::Outward,
    );
    out.is_error = first.is_error || second.is_error;
    out.meta.exit = second.meta.exit.or(first.meta.exit);
    out.meta.denied = first.meta.denied || second.meta.denied;
    out
}

/// What the body reads of a runner's result.
fn outcome(result: Result<Report, Refusal>) -> Outcome {
    match result {
        Ok(r) => {
            let mut text = if r.replayed {
                format!(
                    "receipt {}: {} — this request had landed already; nothing was asked or run",
                    r.receipt, r.state
                )
            } else {
                format!("receipt {}: {}\n{}", r.receipt, r.state, r.output)
            };
            for note in &r.notes {
                let _ = write!(text, "\n[{note}]");
            }
            let mut out = Outcome::ok(text, ToolClass::Outward);
            out.is_error = r.state != State::Observed;
            out.meta.exit = r.exit;
            out
        }
        Err(refusal) => {
            let mut out = Outcome::error(refusal.to_string());
            out.meta.denied = matches!(refusal, Refusal::Denied(_));
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Args, Call, keyed};
    use crate::{Act, Pin};

    #[test]
    fn a_key_binds_the_kind_the_target_and_the_body_or_a_name() {
        let pin = Pin::Forge {
            host: crate::receipt::DEFAULT_HOST.into(),
        };
        let comment = |body: &str| Act::Comment {
            repo: "o/r".into(),
            pr: 7,
            body: body.into(),
        };
        let (a, ra) = keyed(&comment("one"), &pin, None, None).unwrap();
        let (b, rb) = keyed(&comment("two"), &pin, None, None).unwrap();
        assert_eq!(a, ra);
        assert_ne!(a, b);
        assert_ne!(ra, rb);
        // A name keeps the key while the body changes: that is what a conflict is.
        let (na, nra) = keyed(&comment("one"), &pin, Some("summary"), None).unwrap();
        let (nb, nrb) = keyed(&comment("two"), &pin, Some("summary"), None).unwrap();
        assert_eq!(na, nb);
        assert_ne!(nra, nrb);
        // Another host is another target.
        let elsewhere = Pin::Forge {
            host: "ghe.example.com".into(),
        };
        assert_ne!(keyed(&comment("one"), &elsewhere, None, None).unwrap().0, a);
        // A comment that tells of an incident is never the replay of one that does not.
        let (t, rt) = keyed(&comment("one"), &pin, None, Some("abc")).unwrap();
        assert_ne!((t, rt), (a, ra));
    }

    #[test]
    fn a_branch_is_told_of_only_on_a_pull_request_whose_head_it_is() {
        use crate::{Probe, Seen};
        struct Pulls(&'static str);
        impl Probe for Pulls {
            fn run(&self, _: &[String], _: std::time::Duration) -> Result<Seen, String> {
                Ok(Seen {
                    code: Some(0),
                    out: self.0.as_bytes().to_vec(),
                    ..Seen::default()
                })
            }
        }
        let urls = ["git@github.com:me/proj.git".to_owned()];
        let check = |body: &'static str, urls: &[String]| {
            let (mut p, mut s) = (Vec::new(), Vec::new());
            super::branch_pr(
                "feat/x",
                urls,
                "up/proj",
                4,
                "github.com",
                &Pulls(body),
                &mut p,
                &mut s,
            )
        };
        let ours = r#"{"head": {"ref": "feat/x", "repo": {"full_name": "Me/Proj"}}}"#;
        assert_eq!(check(ours, &urls), Ok(()));
        for other in [
            r#"{"head": {"ref": "feat/y", "repo": {"full_name": "me/proj"}}}"#,
            r#"{"head": {"ref": "feat/x", "repo": {"full_name": "you/proj"}}}"#,
            r#"{"head": {"ref": "feat/x", "repo": null}}"#,
        ] {
            assert!(check(other, &urls).is_err(), "{other}");
        }
        assert!(check(ours, &["/srv/me/proj.git".to_owned()]).is_err());
    }

    #[test]
    fn the_tool_has_an_object_schema_its_example_fits() {
        use mnemorph_tools::Tool as _;
        let tool = super::OutwardTool;
        let schema = tool.schema();
        assert_eq!(schema["type"], "object");
        let props = schema["properties"].as_object().unwrap();
        let example = tool.example();
        assert!(
            example
                .as_object()
                .unwrap()
                .keys()
                .all(|k| props.contains_key(k))
        );
        assert!(serde_json::from_value::<Args>(example).is_ok());
    }

    #[test]
    fn the_tool_takes_only_typed_acts() {
        let ok: Args = serde_json::from_value(serde_json::json!({
            "kind": "comment", "repo": "o/r", "pr": 3, "body": "hi"
        }))
        .unwrap();
        let Call::Act(request) = ok.call() else {
            panic!("a comment is an act");
        };
        assert_eq!(
            request.act,
            Act::Comment {
                repo: "o/r".into(),
                pr: 3,
                body: "hi".into()
            }
        );
        let undo: Args =
            serde_json::from_value(serde_json::json!({"kind": "undo", "receipt": "abc"})).unwrap();
        assert_eq!(undo.call(), Call::Undo("abc".into()));
        for bad in [
            serde_json::json!({"kind": "command", "command": "gh pr merge 3"}),
            serde_json::json!({"kind": "comment", "repo": "o/r", "pr": 3, "body": "x", "command": "rm"}),
            serde_json::json!({"command": "git push origin main"}),
        ] {
            assert!(
                serde_json::from_value::<Args>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }
}
