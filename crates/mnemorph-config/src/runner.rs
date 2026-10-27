//! The Kijin's act runner: the one harness path a typed outward act takes, in an order no step
//! can skip.
//!
//! 1. the post check — the act's kinds must all be the Kijin's post's;
//! 2. the plan's fingerprint ([`Fingerprint`]), what the human's approval binds to;
//! 3. the pin — where the act lands, read now; the request's key looked up; what the act would
//!    make must not exist yet;
//! 4. the gate — the human, always: an Oni envelope is not honoured before M11
//!    ([`ENVELOPES_UNWIRED`]);
//! 5. the fingerprint rechecked just before the spawn — a world that moved is asked again;
//! 6. `commit` — the intent journaled, under the request's key;
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

use crate::receipt::{
    branch_tips, carries, excerpt, labels_argv, ls_remote_argv, private_file, redact, sha256,
};
use crate::{Act, Done, PROBE_TIMEOUT, Pin, Planned, PostJournal, Probe, Receipt, Row, State};

/// What every approval card of a typed act says while envelopes are unwired.
pub const ENVELOPES_UNWIRED: &str =
    "no Oni envelope is honoured before M11: the human decides every outward act";

/// How many times an act whose world changed between the approval and the spawn is asked again.
const MAX_REASKS: usize = 3;
/// How long the act's command may run.
const ACT_TIMEOUT: Duration = Duration::from_secs(120);
/// How far before a recorded instant a read of the forge looks, for clocks that disagree.
const SKEW_SECS: i64 = 300;
/// The most a forge list returns in one page: a full page cannot tell what is past it.
const PAGE: usize = 100;
/// The longest name a Kijin may give a request.
const KEY_CHARS: usize = 128;

/// One typed outward act a Kijin asks for, and the name it gives the request, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The act. A [`Act::Command`] is not the runner's: untyped commands go through `bash`.
    pub act: Act,
    /// The request's name: the same name with another request is a conflict.
    pub key: Option<String>,
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
    /// recheck and the spawn.
    fn spawn(
        &self,
        planned: &Planned,
        cx: &Context,
        plan: Fingerprint,
        before: BeforeSpawn<'_>,
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
    ) -> Spawned {
        spawn_approved(
            planned.command(),
            cx,
            ACT_TIMEOUT,
            planned.class(),
            plan,
            before,
        )
    }
}

/// The typed act as the intents file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Typed {
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
}

impl Typed {
    fn of(act: &Act) -> Option<Self> {
        Some(match act.clone() {
            Act::PushNewBranch { remote, name } => Typed::PushNewBranch { remote, name },
            Act::AddLabel { repo, pr, label } => Typed::AddLabel { repo, pr, label },
            Act::Comment { repo, pr, body } => Typed::Comment { repo, pr, body },
            Act::Command(_) => return None,
        })
    }

    fn act(&self) -> Act {
        match self.clone() {
            Typed::PushNewBranch { remote, name } => Act::PushNewBranch { remote, name },
            Typed::AddLabel { repo, pr, label } => Act::AddLabel { repo, pr, label },
            Typed::Comment { repo, pr, body } => Act::Comment { repo, pr, body },
        }
    }
}

/// The pin as the intents file keeps it: whole, credentials and all, outside the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "pin", rename_all = "kebab-case")]
enum Pinned {
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
}

/// What an act left behind, as the intents file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "done", rename_all = "kebab-case")]
enum Left {
    Pushed { sha: String },
    Labelled,
    Commented { id: u64 },
}

impl Left {
    fn of(done: &Done) -> Self {
        match done {
            Done::Pushed { sha } => Left::Pushed { sha: sha.clone() },
            Done::Labelled => Left::Labelled,
            Done::Commented { id } => Left::Commented { id: *id },
        }
    }

    fn done(&self) -> Done {
        match self {
            Left::Pushed { sha } => Done::Pushed { sha: sha.clone() },
            Left::Labelled => Done::Labelled,
            Left::Commented { id } => Done::Commented { id: *id },
        }
    }
}

/// One act's intent, kept beside the witness before its `committed` row is written: the key, the
/// request it was given to, and what a probe needs to find the act again after a cut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Intent {
    /// The receipt's id.
    act: String,
    /// The request's key.
    key: String,
    /// The hash of the request: its kind, pinned target and body.
    request: String,
    /// When its target was found vacant.
    pinned: Timestamp,
    /// When it was committed.
    at: Timestamp,
    typed: Typed,
    pin: Pinned,
    /// What it left, once known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    left: Option<Left>,
}

/// Where a post's intents are kept: beside its witness.
fn intents_path(witness: &Path) -> PathBuf {
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
        _ => return None,
    })
}

/// The request's key and its hash: `(key, request)`.
fn keyed(act: &Act, pin: &Pin, name: Option<&str>) -> Option<(String, String)> {
    let (target, body) = request_parts(act, pin)?;
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

/// Who a forge says someone is.
#[derive(Deserialize)]
struct Login {
    login: String,
}

/// The login `gh` acts as on `host`.
fn login(
    host: &str,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<String, String> {
    let me: Login = serde_json::from_slice(&probed(probe, &forge_get(host, "user"), probes, seen)?)
        .map_err(|e| format!("the login does not read: {e}"))?;
    Ok(me.login)
}

/// Run one probe, keeping what it was and what it printed for the journal.
fn probed(
    probe: &dyn Probe,
    argv: &[String],
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<Vec<u8>, String> {
    probes.push(redact(&argv.join(" ")));
    let s = probe
        .run(argv, PROBE_TIMEOUT)
        .map_err(|e| format!("the probe failed: {}", redact(&e)))?;
    seen.push(excerpt(&s.out, crate::SEEN_BYTES));
    if s.code != Some(0) {
        return Err(format!(
            "the probe exited {}: {}",
            s.code
                .map_or_else(|| "on a signal".into(), |c| c.to_string()),
            excerpt(&s.err, 200)
        ));
    }
    if s.cut {
        return Err("the probe printed more than it reads: it cannot tell".into());
    }
    Ok(s.out)
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
        _ => unknown("the pin is not of this act".into()),
    }
}

/// `gh api` GET of `path` on `host`, as every forge probe runs it.
fn forge_get(host: &str, path: &str) -> Vec<String> {
    ["gh", "api", "--hostname", host, "--method", "GET", path]
        .map(str::to_owned)
        .to_vec()
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
}

/// What the gate is asked about a re-ask, once.
struct Reask {
    changed: Vec<String>,
    notes: Vec<String>,
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
        }
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
        if let mnemorph_tools::Sandbox::None(reason) = self.cx.sandbox()
            && !self.cx.allows_unguarded()
        {
            return refuse(format!(
                "no kernel guard is in force ({reason}), so the act did not run; to run unguarded, set `sandbox.allow_unguarded = true` in your local or global config"
            ));
        }
        if self.cx.race().base() != Race::Kijin
            || Post::of(self.cx.body()) != Some(self.journal.post)
        {
            return refuse(format!(
                "only the Kijin of the {} post runs its typed acts",
                self.journal.post.creature()
            ));
        }
        let place = Place {
            world: self.cx.world().to_path_buf(),
            home: self.cx.home().to_path_buf(),
            cwd: self.cx.world().to_path_buf(),
            roots: self.cx.deploy_roots().to_vec(),
        };
        let planned =
            crate::plan(&request.act, &place).map_err(|e| Refusal::Refused(e.to_string()))?;
        check_class(self.cx.race(), planned.class())
            .map_err(|e| Refusal::Refused(e.to_string()))?;
        // 1. The post: an act the post does not hold is refused before anything is read or asked.
        planned
            .check_post(self.cx.body())
            .map_err(|e| Refusal::Refused(e.to_string()))?;
        let _held = self.hold()?;
        // Cut acts first: each is probed before the next act of the post is gated.
        let mut reask = Reask {
            changed: Vec::new(),
            notes: self.recover_held()?,
        };
        for _ in 0..=MAX_REASKS {
            if let Some(report) = self.once(&planned, request, &mut reask)? {
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
        let world = self.cx.world();
        // 2. The plan the approval binds to, read before the human is asked.
        let plan = Fingerprint::of(planned.command(), world).map_err(|e| {
            Refusal::Refused(format!(
                "the plan this act would be approved against could not be read: {e}"
            ))
        })?;
        // 3. The pin: where it lands, then the key, then what it would make must not exist.
        let pin = planned.aim(world).map_err(Refusal::Refused)?;
        let (key, digest) = keyed(planned.act(), &pin, request.key.as_deref())
            .ok_or_else(|| Refusal::Refused("the pin is not of this act".into()))?;
        if let Some(report) = self.replay(&key, &digest, reask)? {
            return Ok(Some(report));
        }
        planned.vacant(&pin, self.probe).map_err(Refusal::Refused)?;
        let pinned = Timestamp::now();
        // 4. The gate: the human, always, until M11 honours envelopes.
        self.ask(planned, &plan, &reask.changed)?;
        // 5–7. The recheck, the intent, the spawn — in that order, in the hands.
        let typed = Typed::of(planned.act())
            .ok_or_else(|| Refusal::Refused("an untyped command is not a typed act".into()))?;
        let kept_pin = Pinned::of(&pin)
            .ok_or_else(|| Refusal::Refused("the pin is not of this act".into()))?;
        let short = plan.short();
        let mut committed: Option<Receipt> = None;
        let mut intent_written = |id: &str| {
            self.keep(&Intent {
                act: id.to_owned(),
                key: key.clone(),
                request: digest.clone(),
                pinned,
                at: Timestamp::now(),
                typed: typed.clone(),
                pin: kept_pin.clone(),
                left: None,
            })
        };
        let mut before = || {
            let receipt = self
                .journal
                .commit_then(planned, pin.clone(), Some(&short), &mut intent_written)
                .map_err(|e| format!("the intent could not be journaled: {e}"))?;
            committed = Some(receipt);
            Ok(())
        };
        let spawned = self.hands.spawn(planned, self.cx, plan, &mut before);
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
        self.finish(receipt, out, std::mem::take(&mut reask.notes))
            .map(Some)
    }

    /// Put the act to the human, bound to `plan`; `changed` is what made an earlier approval
    /// stale. No envelope is read: until M11 the human decides every act.
    fn ask(
        &self,
        planned: &Planned,
        plan: &Fingerprint,
        changed: &[String],
    ) -> Result<(), Refusal> {
        let mut ask = planned.ask(self.cx.body(), self.cx.race());
        ask.fingerprint = Some(plan.clone());
        ask.why.push(ENVELOPES_UNWIRED.to_owned());
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
            self.journal
                .observe(&mut receipt, self.probe, PROBE_TIMEOUT)
                .map_err(|e| Refusal::Failed(format!("the receipt could not be journaled: {e}")))?;
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
            State::Observed => {
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
            state => Err(Refusal::Refused(format!(
                "act {} of this request is {state}: the human settles it before it is asked again (`mnemorph status`)",
                intent.act
            ))),
        }
    }

    /// The undo of an observed act, rendered only once a probe shows that what the act left is
    /// still its own: a label is removed only while the one add since its pin is this act's, by
    /// the login `gh` acts as, and the pull request still carries it. Anything uncertain moves the
    /// act to `reconcile`, for the human, and nothing is undone. The undo is an act of its own: it
    /// goes through the gate like any other.
    ///
    /// # Errors
    ///
    /// The act is not this post's observed act, or what it left is no longer certainly its own.
    pub fn undo(&self, receipt: &str) -> Result<String, Refusal> {
        let _held = self.hold()?;
        let state = self
            .standing()?
            .into_iter()
            .find(|r| r.act == receipt)
            .map(|r| r.state);
        if state != Some(State::Observed) {
            return Err(Refusal::Refused(format!(
                "act {receipt} is {}: only an observed act is undone",
                state.map_or_else(|| "not in this post's journal".into(), |s| s.to_string())
            )));
        }
        let intent = self
            .kept()?
            .into_iter()
            .find(|i| i.act == receipt)
            .ok_or_else(|| Refusal::Refused(format!("act {receipt} has no intent kept")))?;
        let place = Place::new(self.cx.world(), self.cx.home());
        let planned = crate::plan(&intent.typed.act(), &place)
            .map_err(|e| Refusal::Refused(e.to_string()))?;
        let done = match (&intent.left, &intent.typed) {
            (Some(left), _) => left.done(),
            (None, Typed::AddLabel { .. }) => Done::Labelled,
            (None, _) => {
                return Err(Refusal::Refused(format!(
                    "act {receipt} left nothing recorded to undo"
                )));
            }
        };
        if let Typed::AddLabel { .. } = intent.typed {
            let (mut probes, mut seen) = (Vec::new(), Vec::new());
            if let Err(why) = label_is_ours(&intent, self.probe, &mut probes, &mut seen) {
                let why = format!("not undone: {why}");
                self.settle(receipt, State::Reconcile, why.clone(), &probes, &seen);
                return Err(Refusal::Refused(why));
            }
        }
        planned
            .undo(&done)
            .map_err(|e| Refusal::Refused(e.to_string()))
    }
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
    },
}

impl Args {
    fn request(self) -> Request {
        match self {
            Args::PushNewBranch { remote, name, key } => Request {
                act: Act::PushNewBranch { remote, name },
                key,
            },
            Args::AddLabel {
                repo,
                pr,
                label,
                key,
            } => Request {
                act: Act::AddLabel { repo, pr, label },
                key,
            },
            Args::Comment {
                repo,
                pr,
                body,
                key,
            } => Request {
                act: Act::Comment { repo, pr, body },
                key,
            },
        }
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
        "Do one typed outward act of your post: push a new branch, label a pull request, or comment on one. The harness renders the command and its undo, pins the target, asks the human, journals the intent, runs it, and probes that it landed; you get the receipt. The same request again returns its receipt without acting; name a request with `key` to have a different request under the same name refused. Any other outward command goes through `bash`, where it is destructive."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "kind": {"type": "string", "enum": ["push-new-branch", "add-label", "comment"], "description": "The act. `push-new-branch` takes `remote` and `name`; `add-label` takes `repo`, `pr` and `label`; `comment` takes `repo`, `pr` and `body`."},
                "remote": {"type": "string", "description": "push-new-branch: a configured remote's name (`origin`)."},
                "name": {"type": "string", "description": "push-new-branch: the new branch, a plain name (`feat/undo`); it must not exist yet. HEAD's commit is pushed."},
                "repo": {"type": "string", "description": "add-label, comment: the repository, `owner/name`."},
                "pr": {"type": "integer", "minimum": 1, "description": "add-label, comment: the pull request's number."},
                "label": {"type": "string", "maxLength": 50, "description": "add-label: the label."},
                "body": {"type": "string", "description": "comment: the comment."},
                "key": {"type": "string", "maxLength": KEY_CHARS, "description": "Optional: the request's name. The same name with a different request is refused."}
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
        let request = match serde_json::from_value::<Args>(raw) {
            Ok(a) => a.request(),
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
        let runner = Runner::new(cx, journal, &observer, &Sandboxed);
        outcome(runner.act(&request))
    }
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
    use super::{Args, keyed};
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
        let (a, ra) = keyed(&comment("one"), &pin, None).unwrap();
        let (b, rb) = keyed(&comment("two"), &pin, None).unwrap();
        assert_eq!(a, ra);
        assert_ne!(a, b);
        assert_ne!(ra, rb);
        // A name keeps the key while the body changes: that is what a conflict is.
        let (na, nra) = keyed(&comment("one"), &pin, Some("summary")).unwrap();
        let (nb, nrb) = keyed(&comment("two"), &pin, Some("summary")).unwrap();
        assert_eq!(na, nb);
        assert_ne!(nra, nrb);
        // Another host is another target.
        let elsewhere = Pin::Forge {
            host: "ghe.example.com".into(),
        };
        assert_ne!(keyed(&comment("one"), &elsewhere, None).unwrap().0, a);
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
        assert_eq!(
            ok.request().act,
            Act::Comment {
                repo: "o/r".into(),
                pr: 3,
                body: "hi".into()
            }
        );
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
