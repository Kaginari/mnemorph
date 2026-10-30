//! The outward lifecycles, explored breadth-first over the real code: an act's receipt through
//! its post journal, and an Oni's act through the act runner — its envelope, its incident and the
//! incident's four receipts — every state reachable from a fresh world within the stated depth,
//! with the shortest event path to each, and the invariants asserted in every one.
//!
//! - **The receipt** ([`PostJournal::commit`], [`PostJournal::ran`], [`PostJournal::observe`]):
//!   explored to its fixpoint (three moves), replayed from a fresh journal for every path.
//! - **The runner** ([`Runner::act`], [`Runner::undo`], [`PostJournal::settle`],
//!   [`PostJournal::open_incident`], [`PostJournal::lesson`], [`EnvelopeGate::revoke`]): to depth
//!   [`DEPTH`]. Each state is a copy of the world, the witness and the human's files on disk, and
//!   of the stub forge: every event runs on a fork of its parent, never on a model of it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mnemorph_config::{
    Act, Envelope, EnvelopeGate, Hands, Lesson, Paths, Pin, Planned, PostJournal, Probe, Request,
    Row, Runner, Seen, State, plan,
};
use mnemorph_ledger::{Journal, Timestamp, UseRow};
use mnemorph_races::{Post, Race, ToolClass};
use mnemorph_tools::shell::{BeforeSpawn, Spawned};
use mnemorph_tools::{Answer, Ask, Context, Fingerprint, Gate, Outcome, Place, Sandbox};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn label_act(name: &str) -> Act {
    Act::AddLabel {
        repo: "me/proj".into(),
        pr: 12,
        label: name.into(),
    }
}

// ---------------------------------------------------------------------------------------------
// The receipt.

/// What a receipt-level move does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    /// Commit a typed act: a label.
    CommitTyped,
    /// Commit an untyped command.
    CommitCommand,
    /// It ran, and exited 0.
    RanOk,
    /// It ran, and exited 1.
    RanFailed,
    /// A signal ended it.
    RanKilled,
    /// The probe sees the label.
    ProbeSees,
    /// The probe sees no label.
    ProbeSeesOther,
    /// The probe fails.
    ProbeFails,
}

const MOVES: [Move; 8] = [
    Move::CommitTyped,
    Move::CommitCommand,
    Move::RanOk,
    Move::RanFailed,
    Move::RanKilled,
    Move::ProbeSees,
    Move::ProbeSeesOther,
    Move::ProbeFails,
];

/// A forge probe from a script: the labels on `me/proj#12`, or a failure.
struct Labels(Result<Vec<&'static str>, &'static str>);

impl Probe for Labels {
    fn run(&self, argv: &[String], _: Duration) -> Result<Seen, String> {
        assert_eq!(&argv[..2], ["gh", "api"], "{argv:?}");
        let labels = self.0.clone().map_err(str::to_owned)?;
        let out = serde_json::json!(
            labels
                .iter()
                .map(|l| serde_json::json!({ "name": l }))
                .collect::<Vec<_>>()
        );
        Ok(Seen {
            code: Some(0),
            out: out.to_string().into_bytes(),
            ..Seen::default()
        })
    }
}

/// Replay `path` on a fresh journal: the receipt's kind and state — `typed committed`,
/// `command unobserved` — or `none` before any commit. Every refusal is a move that changes
/// nothing, and is checked to.
fn replay(path: &[Move]) -> String {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (world, home) = (root.join("world"), root.join("home"));
    fs::create_dir_all(&world).unwrap();
    fs::create_dir_all(&home).unwrap();
    let journal = PostJournal::of(&world, Post::Forge).witnessed_at(&root.join("state/w.jsonl"));
    let place = Place::new(&world, &home);
    let mut receipt = None;
    for m in path {
        let before = receipt.as_ref().map(mnemorph_config::Receipt::state);
        let rows = journal_rows(&journal);
        let done = match (*m, receipt.as_mut()) {
            (Move::CommitTyped | Move::CommitCommand, None) => {
                let (act, pin) = if *m == Move::CommitTyped {
                    let pin = Pin::Forge {
                        host: "github.com".into(),
                    };
                    (label_act("ready"), pin)
                } else {
                    (Act::Command("make release".into()), Pin::Unpinned)
                };
                let planned = plan(&act, &place).unwrap();
                receipt = Some(journal.commit(&planned, pin, Some("plan 0")).unwrap());
                true
            }
            (Move::RanOk | Move::RanFailed | Move::RanKilled, Some(r)) => {
                let exit = match m {
                    Move::RanOk => Some(0),
                    Move::RanFailed => Some(1),
                    _ => None,
                };
                journal.ran(r, exit, "").is_ok()
            }
            (Move::ProbeSees | Move::ProbeSeesOther | Move::ProbeFails, Some(r)) => {
                let probe = Labels(match m {
                    Move::ProbeSees => Ok(vec!["ready"]),
                    Move::ProbeSeesOther => Ok(vec!["other"]),
                    _ => Err("could not resolve host"),
                });
                journal.observe(r, &probe, Duration::from_secs(5)).is_ok()
            }
            (Move::CommitTyped | Move::CommitCommand, Some(_)) | (_, None) => false,
        };
        let after = receipt.as_ref().map(mnemorph_config::Receipt::state);
        let now = journal_rows(&journal);
        if done {
            // A move that took writes exactly its row, of the state it moved to.
            assert_eq!(now.len(), rows.len() + 1, "{path:?}");
            assert_eq!(now.last().map(|r| r.state), after, "{path:?}");
        } else {
            assert_eq!((now.len(), after), (rows.len(), before), "{path:?}: {m:?}");
        }
        for row in &now {
            // A receipt is observed only when a probe ran and was seen.
            if row.state == State::Observed {
                assert!(row.probe.is_some() && row.seen.is_some(), "{row:?}");
            }
            // A reconcile after a failed exit ran no probe.
            if row.state == State::Reconcile && row.exit.is_some_and(|c| c != 0) {
                assert!(row.probe.is_none(), "{row:?}");
            }
        }
        if let Some(r) = &receipt
            && matches!(r.state(), State::Unobserved)
        {
            assert!(now.iter().all(|row| row.state != State::Observed));
        }
    }
    let kind = if path.first() == Some(&Move::CommitCommand) {
        "command"
    } else {
        "typed"
    };
    receipt.map_or_else(|| "none".to_owned(), |r| format!("{kind} {}", r.state()))
}

fn journal_rows(journal: &PostJournal) -> Vec<Row> {
    let path = journal.witness().unwrap();
    if path.exists() {
        Journal::<Row>::new(path).read().unwrap().rows
    } else {
        Vec::new()
    }
}

#[test]
fn a_receipt_reaches_its_states_and_is_never_observed_without_its_probe() {
    // To the fixpoint: every state reached, with the shortest path to it.
    let mut seen: BTreeMap<String, Vec<Move>> = BTreeMap::from([("none".into(), Vec::new())]);
    let mut frontier = vec![Vec::new()];
    let mut depth = 0;
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for path in frontier {
            for m in MOVES {
                let mut p: Vec<Move> = path.clone();
                p.push(m);
                if let std::collections::btree_map::Entry::Vacant(v) = seen.entry(replay(&p)) {
                    v.insert(p.clone());
                    next.push(p);
                }
            }
        }
        frontier = next;
        depth += 1;
        assert!(depth <= 4, "the receipt has no fixpoint: {seen:?}");
    }
    let reached: BTreeSet<&str> = seen.keys().map(String::as_str).collect();
    // Absent, corrected and settled are the runner's and the human's, never the journal's alone.
    // A command is never probed: unobserved is where it rests, whatever its exit.
    let intended: BTreeSet<&str> = [
        "none",
        "typed committed",
        "typed pending observation",
        "typed observed",
        "typed reconcile",
        "command committed",
        "command unobserved",
    ]
    .into();
    assert_eq!(reached, intended, "{seen:#?}");
    assert_eq!(
        seen["typed observed"],
        [Move::CommitTyped, Move::RanOk, Move::ProbeSees]
    );
    assert_eq!(
        seen["typed reconcile"],
        [Move::CommitTyped, Move::RanFailed]
    );
    assert_eq!(
        seen["command unobserved"],
        [Move::CommitCommand, Move::RanOk]
    );
}

// ---------------------------------------------------------------------------------------------
// The runner: an Oni's act, its envelope, its incident.

/// How deep the runner's walk goes: a rollback incident closes in four moves (act, undo, tell,
/// lesson); a reconcile incident in five (act, mark, undo, tell, lesson).
const DEPTH: usize = 5;

/// The envelope's cap.
const CAP: usize = 2;

/// The human: approves, or denies when told to; counts what they approved.
#[derive(Default)]
struct Human {
    deny: AtomicBool,
    approved: AtomicUsize,
}

impl Gate for Human {
    fn ask(&self, _: &Ask) -> Answer {
        if self.deny.load(Ordering::SeqCst) {
            Answer::Deny("no".into())
        } else {
            self.approved.fetch_add(1, Ordering::SeqCst);
            Answer::Approve
        }
    }

    fn admitted(&self, _: &Ask, _: &str) {}
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "the stub's switches, one per fault"
)]
/// A stub of GitHub's pull request `me/proj#12`: its labels, comments and label events, read
/// through `gh api` (the probe) and written through `gh pr` (the hands).
#[derive(Debug, Clone, Default)]
struct ForgeState {
    labels: Vec<String>,
    /// (id, body)
    comments: Vec<(u64, String)>,
    /// (event, label, at)
    events: Vec<(String, String, String)>,
    /// Acts and undos the hands ran.
    spawns: usize,
    /// Die after the act, before its result is written.
    cut: bool,
    /// Die before the act, its intent journaled.
    cut_before: bool,
    /// An undo changes nothing.
    noop_undo: bool,
    /// The probe reaches nothing once an act has run.
    dark: bool,
    /// An act ran while `dark`.
    blind: bool,
}

const LOGIN: &str = "kijin-bot";

struct Forge(Mutex<ForgeState>);

impl Forge {
    fn event(s: &mut ForgeState, event: &str, label: &str) {
        s.events
            .push((event.into(), label.into(), Timestamp::now().to_string()));
    }
}

impl Probe for Forge {
    fn run(&self, argv: &[String], _: Duration) -> Result<Seen, String> {
        assert_eq!(&argv[..2], ["gh", "api"], "{argv:?}");
        let path = argv.last().unwrap();
        let s = self.0.lock().unwrap();
        if s.blind {
            return Err("could not resolve host".into());
        }
        let issue = "https://api.github.com/repos/me/proj/issues/12";
        let comment = |(id, body): &(u64, String)| serde_json::json!({"id": id, "body": body, "issue_url": issue, "user": {"login": LOGIN}});
        let out = if path == "user" {
            serde_json::json!({ "login": LOGIN })
        } else if path.starts_with("repos/me/proj/issues/12/labels") {
            serde_json::json!(
                s.labels
                    .iter()
                    .map(|l| serde_json::json!({ "name": l }))
                    .collect::<Vec<_>>()
            )
        } else if path.starts_with("repos/me/proj/issues/12/comments") {
            serde_json::json!(s.comments.iter().map(comment).collect::<Vec<_>>())
        } else if let Some(id) = path.strip_prefix("repos/me/proj/issues/comments/") {
            match s.comments.iter().find(|(c, _)| c.to_string() == id) {
                Some(c) => comment(c),
                None => {
                    return Ok(Seen {
                        code: Some(1),
                        err: b"HTTP 404: Not Found".to_vec(),
                        ..Seen::default()
                    });
                }
            }
        } else if path == "repos/me/proj/pulls/12" {
            serde_json::json!({"head": {"ref": "feat/x", "repo": {"full_name": "me/proj"}}})
        } else if path.starts_with("repos/me/proj/issues/12/events") {
            serde_json::json!(s.events.iter().map(|(e, label, at)| serde_json::json!({
                "event": e, "actor": {"login": LOGIN}, "label": {"name": label}, "created_at": at
            })).collect::<Vec<_>>())
        } else {
            panic!("an unexpected probe: {argv:?}");
        };
        Ok(Seen {
            code: Some(0),
            out: out.to_string().into_bytes(),
            ..Seen::default()
        })
    }
}

impl Hands for Forge {
    fn spawn(
        &self,
        planned: &Planned,
        _: &Context,
        _: Fingerprint,
        before: BeforeSpawn<'_>,
        _: Option<&[u8]>,
    ) -> Spawned {
        if let Err(e) = before() {
            return Spawned::Failed(Outcome::error(e));
        }
        let mut s = self.0.lock().unwrap();
        if s.cut_before {
            drop(s);
            panic!("the harness was killed before the act");
        }
        s.spawns += 1;
        s.blind = s.dark;
        let printed = if planned.is_undo() {
            if !s.noop_undo {
                match planned.act() {
                    Act::AddLabel { label, .. } => {
                        if s.labels.iter().any(|l| l == label) {
                            s.labels.retain(|l| l != label);
                            Forge::event(&mut s, "unlabeled", label);
                        }
                    }
                    Act::Comment { .. } => {
                        let id: u64 = planned
                            .command()
                            .rsplit('/')
                            .next()
                            .unwrap()
                            .parse()
                            .unwrap();
                        s.comments.retain(|c| c.0 != id);
                    }
                    other => panic!("not a forge act: {other:?}"),
                }
            }
            String::new()
        } else {
            match planned.act() {
                Act::AddLabel { label, .. } => {
                    if !s.labels.iter().any(|l| l == label) {
                        s.labels.push(label.clone());
                        Forge::event(&mut s, "labeled", label);
                    }
                    String::new()
                }
                Act::Comment { body, .. } => {
                    let id = 4000 + s.comments.len() as u64;
                    s.comments.push((id, body.clone()));
                    format!("https://github.com/me/proj/pull/12#issuecomment-{id}\n")
                }
                other => panic!("not a forge act: {other:?}"),
            }
        };
        if s.cut && !planned.is_undo() {
            drop(s);
            panic!("the harness was killed between the act and its result");
        }
        let mut out = Outcome::ok(printed, ToolClass::Outward);
        out.meta.exit = Some(0);
        Spawned::Ran(out)
    }
}

/// What the world, the forge or the human does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Ev {
    /// The Oni labels the pull request `ready`.
    Act,
    /// The same, and the human, if asked, denies it.
    ActDenied,
    /// The same, cut after the label lands and before its result is written.
    ActCut,
    /// The same, cut after its intent and before it runs.
    ActCutBefore,
    /// The same, the probe reaching nothing once it has run.
    ActDark,
    /// The human approves the undo of the first act.
    Undo,
    /// The same, and the undo leaves the label.
    UndoNoop,
    /// The human settles the first act, in reconcile, as no incident.
    Settle,
    /// The human marks the first act an incident.
    Mark,
    /// The Oni tells downstream of the first act's incident.
    Tell,
    /// The incident's lesson is written and recorded.
    Lesson,
    /// The human revokes the envelope.
    Revoke,
}

const EVENTS: [Ev; 12] = [
    Ev::Act,
    Ev::ActDenied,
    Ev::ActCut,
    Ev::ActCutBefore,
    Ev::ActDark,
    Ev::Undo,
    Ev::UndoNoop,
    Ev::Settle,
    Ev::Mark,
    Ev::Tell,
    Ev::Lesson,
    Ev::Revoke,
];

/// One state, on disk: a world, its witness, the human's files, and the forge.
struct Node {
    tmp: tempfile::TempDir,
    forge: ForgeState,
    /// Asks the human approved, so far.
    approved: usize,
}

/// A state as the walk tells states apart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Abs {
    /// The first label act's latest state.
    first: Option<String>,
    /// Label acts committed, up to three.
    acts: usize,
    /// The first act's incident: its cause and the receipts it still misses.
    incident: Option<(String, Vec<&'static str>)>,
    /// The label is on the pull request.
    labelled: bool,
    /// The envelope's uses.
    uses: usize,
    revoked: bool,
}

impl Node {
    fn root(&self) -> PathBuf {
        self.tmp.path().to_path_buf()
    }

    fn world(&self) -> PathBuf {
        self.root().join("world")
    }

    fn witness(&self) -> PathBuf {
        self.root().join("state/kijin/w/kijin-forge.jsonl")
    }

    fn gate(&self) -> EnvelopeGate {
        EnvelopeGate::new(
            Paths::with_global(
                self.root().join("config/mnemorph/config.toml"),
                &self.world(),
            ),
            self.root().join("state/oni/uses.jsonl"),
        )
    }

    fn journal(&self) -> PostJournal {
        PostJournal::of(&self.world(), Post::Forge).witnessed_at(&self.witness())
    }

    fn fresh() -> Node {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let world = root.join("world");
        fs::create_dir_all(&world).unwrap();
        fs::create_dir_all(root.join("home")).unwrap();
        // No template: a fork copies the world, and the samples would only slow it.
        git(&world, &["init", "-q", "--template="]);
        fs::write(world.join("a.txt"), "one\n").unwrap();
        git(&world, &["add", "a.txt"]);
        git(&world, &["commit", "-qm", "one"]);
        let global = root.join("config/mnemorph/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        let today = Timestamp::now().day();
        fs::write(
            &global,
            format!(
                "[[oni.envelopes]]\nname = \"forge\"\nallow = [\"gh pr edit * --repo me/proj --add-label *\"]\nsigned = {}\nexpires = {}\nuses = {CAP}\nrepo = \"me/proj\"\n",
                today.plus(-1),
                today.plus(20)
            ),
        )
        .unwrap();
        set_private(&global);
        Node {
            tmp,
            forge: ForgeState::default(),
            approved: 0,
        }
    }

    /// A copy of this state: its files, its forge, its human's count.
    fn fork(&self) -> Node {
        let tmp = tempfile::tempdir().unwrap();
        copy_tree(self.tmp.path(), tmp.path());
        Node {
            tmp,
            forge: self.forge.clone(),
            approved: self.approved,
        }
    }

    fn rows(&self) -> Vec<Row> {
        if self.witness().exists() {
            Journal::<Row>::new(self.witness()).read().unwrap().rows
        } else {
            Vec::new()
        }
    }

    /// The label acts' receipts, oldest first: committed, not undos, labels.
    fn label_acts(&self) -> Vec<String> {
        self.rows()
            .into_iter()
            .filter(|r| {
                r.state == State::Committed
                    && r.undoes.is_none()
                    && r.command
                        .as_deref()
                        .is_some_and(|c| c.contains("--add-label"))
            })
            .map(|r| r.act)
            .collect()
    }

    fn latest(&self, act: &str) -> Option<Row> {
        self.rows().into_iter().rev().find(|r| r.act == act)
    }

    fn uses(&self) -> Vec<UseRow> {
        let path = self.gate().uses().path().to_path_buf();
        if path.exists() {
            Journal::<UseRow>::new(path).read().unwrap().rows
        } else {
            Vec::new()
        }
    }

    fn revoked(&self) -> bool {
        self.gate().revocations().exists()
    }

    fn envelope(&self) -> Envelope {
        self.gate().signed(&self.world()).unwrap().remove(0)
    }

    fn abs(&self) -> Abs {
        let acts = self.label_acts();
        let first = acts.first();
        let incident = first.and_then(|a| self.journal().incident(a).unwrap());
        Abs {
            first: first
                .and_then(|a| self.latest(a))
                .map(|r| r.state.to_string()),
            acts: acts.len().min(3),
            incident: incident.map(|i| (i.cause.to_string(), i.missing())),
            labelled: !self.forge.labels.is_empty(),
            uses: self.uses().len(),
            revoked: self.revoked(),
        }
    }

    fn runner<'a>(
        &self,
        cx: &'a Context,
        probe: &'a dyn Probe,
        hands: &'a dyn Hands,
    ) -> Runner<'a> {
        Runner::new(cx, self.journal(), probe, hands).with_envelopes(Some(self.gate()))
    }

    /// Run `e` on this state.
    fn apply(&mut self, e: Ev) {
        let human = Arc::new(Human::default());
        human.deny.store(e == Ev::ActDenied, Ordering::SeqCst);
        let mut forge = self.forge.clone();
        forge.cut = e == Ev::ActCut;
        forge.cut_before = e == Ev::ActCutBefore;
        forge.noop_undo = e == Ev::UndoNoop;
        forge.dark = e == Ev::ActDark;
        let forge = Forge(Mutex::new(forge));
        let world = self.world();
        let cx = Context::new(Race::Oni, &world, &["."])
            .with_body("kijin-forge")
            .with_home(self.root().join("home"))
            .with_gate(human.clone())
            .with_sandbox(Sandbox::None("tests run unguarded".into()))
            .with_unguarded(true);
        let first = self.label_acts().first().cloned();
        let journal = self.journal();
        let request = |act: Act, tells: Option<String>| Request {
            act,
            key: None,
            tells,
        };
        match e {
            Ev::Act | Ev::ActDenied | Ev::ActDark => {
                let _ = self
                    .runner(&cx, &forge, &forge)
                    .act(&request(label_act("ready"), None));
            }
            Ev::ActCut | Ev::ActCutBefore => {
                let cut = catch_unwind(AssertUnwindSafe(|| {
                    self.runner(&cx, &forge, &forge)
                        .act(&request(label_act("ready"), None))
                }));
                // Refused before the hands, or a request that landed already, it is not cut at all.
                if let Ok(r) = cut {
                    assert!(
                        r.as_ref().map_or(true, |r| r.replayed),
                        "{e:?} ran whole: {r:?}"
                    );
                }
            }
            Ev::Undo | Ev::UndoNoop => {
                if let Some(a) = &first {
                    let _ = self.runner(&cx, &forge, &forge).undo(a);
                }
            }
            Ev::Settle => {
                if let Some(a) = &first {
                    let _ = journal.settle(a, Some("pushed by hand"));
                }
            }
            Ev::Mark => {
                if let Some(a) = &first {
                    let _ = journal.open_incident(a, Some("the label was wrong"));
                }
            }
            Ev::Tell => {
                if let Some(a) = &first {
                    let body = "The `ready` label was a mistake: removed.";
                    let comment = Act::Comment {
                        repo: "me/proj".into(),
                        pr: 12,
                        body: body.into(),
                    };
                    let _ = self
                        .runner(&cx, &forge, &forge)
                        .act(&request(comment, Some(a.clone())));
                }
            }
            Ev::Lesson => {
                if let Some(a) = &first {
                    fs::create_dir_all(world.join("tests")).unwrap();
                    fs::write(
                        world.join("tests/lesson.rs"),
                        "#[test]\nfn the_label_waits_for_review() {}\n",
                    )
                    .unwrap();
                    let lesson = Lesson::test(&world, "tests/lesson.rs").unwrap();
                    let _ = journal.lesson(a, lesson);
                }
            }
            Ev::Revoke => {
                let _ = self.gate().revoke("forge");
            }
        }
        let mut after = forge
            .0
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        after.cut = false;
        after.cut_before = false;
        after.noop_undo = false;
        after.dark = false;
        after.blind = false;
        self.forge = after;
        self.approved += human.approved.load(Ordering::SeqCst);
    }

    /// The invariants every state keeps, read from its files.
    fn check(&self, path: &[Ev]) {
        let rows = self.rows();
        let latest = |act: &str| rows.iter().rev().find(|r| r.act == act);
        let mut by_act: BTreeMap<&str, Vec<State>> = BTreeMap::new();
        for r in &rows {
            by_act.entry(&r.act).or_default().push(r.state);
            // Never observed without a probe that ran.
            if r.state == State::Observed {
                assert!(r.probe.is_some(), "{path:?}: observed with no probe: {r:?}");
            }
            // A correction names an undo the probe saw land.
            if r.state == State::Corrected {
                let undo = latest(r.by.as_deref().unwrap()).unwrap();
                assert_eq!(undo.state, State::Observed, "{path:?}");
                assert_eq!(undo.undoes.as_deref(), Some(r.act.as_str()), "{path:?}");
            }
        }
        // Each act moves only along the receipt's edges.
        for (act, states) in &by_act {
            assert_eq!(states[0], State::Committed, "{path:?}: {act} {states:?}");
            for w in states.windows(2) {
                assert!(
                    edge(w[0], w[1]),
                    "{path:?}: act {act} moved {} → {}: {states:?}",
                    w[0],
                    w[1]
                );
            }
        }
        // Every incident closed has its four receipts, each read again from the witness.
        for inc in self.journal().incidents().unwrap() {
            if inc.is_open() {
                continue;
            }
            let undo = latest(inc.undone.as_deref().unwrap()).unwrap();
            assert_eq!(undo.state, State::Observed, "{path:?}");
            assert_eq!(undo.undoes.as_deref(), Some(inc.act.as_str()));
            let act = latest(&inc.act).unwrap();
            assert_eq!(
                (act.state, act.by.as_deref()),
                (State::Corrected, inc.undone.as_deref()),
                "{path:?}"
            );
            assert_eq!(
                latest(inc.told.as_deref().unwrap()).unwrap().state,
                State::Observed,
                "{path:?}"
            );
            assert!(inc.lesson.is_some() && self.world().join("tests/lesson.rs").exists());
        }
        // The human's file stays theirs alone, whatever wrote it last: under the walk's umask a
        // rewrite at the umask's mode would leave it group-writable, and no envelope honoured.
        let global = self.root().join("config/mnemorph/config.toml");
        let mode = fs::metadata(&global).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{path:?}: the global file is {mode:o}");
        // The envelope: never past its cap; every use an act it admitted, before any revocation.
        let uses = self.uses();
        assert!(uses.len() <= CAP, "{path:?}: {} uses", uses.len());
        let revoked_at = self.revoked().then(|| {
            Journal::<mnemorph_config::Revocation>::new(self.gate().revocations())
                .read()
                .unwrap()
                .rows[0]
                .at
        });
        for u in &uses {
            let committed = rows
                .iter()
                .find(|r| r.act == u.act && r.state == State::Committed)
                .unwrap_or_else(|| panic!("{path:?}: a use of no act: {u:?}"));
            assert!(
                committed
                    .why
                    .as_deref()
                    .is_some_and(|w| w.starts_with("admitted by envelope forge")),
                "{path:?}: {committed:?}"
            );
            if let Some(at) = revoked_at {
                assert!(u.at.0 <= at.0, "{path:?}: a use after the revocation");
            }
        }
        // Nothing ran that neither the human nor an envelope let run: a cut act is never replayed.
        assert!(
            self.forge.spawns <= self.approved + uses.len(),
            "{path:?}: {} spawns, {} approved, {} uses",
            self.forge.spawns,
            self.approved,
            uses.len()
        );
    }
}

/// The receipt's edges: what one row of an act may follow another with.
fn edge(from: State, to: State) -> bool {
    use State as S;
    matches!(
        (from, to),
        // It ran, or was cut and probed.
        (S::Committed, S::PendingObservation | S::Reconcile | S::Unobserved | S::Observed | S::Absent)
            // Probed, or cut before its probe and probed again.
            | (S::PendingObservation, S::Observed | S::Reconcile | S::Absent)
            // Asked again, its effect no longer seen: the human looks.
            | (S::Observed | S::Settled, S::Reconcile)
            // Undone, its undo seen; or settled by the human.
            | (S::Observed | S::Reconcile | S::Settled, S::Corrected)
            | (S::Reconcile, S::Settled)
    )
}

fn set_private(file: &Path) {
    fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(file.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
}

/// Copy `from` into `to`, files with their permissions.
fn copy_tree(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            fs::create_dir_all(&dst).unwrap();
            copy_tree(&src, &dst);
            let mode = fs::metadata(&src).unwrap().permissions().mode();
            fs::set_permissions(&dst, fs::Permissions::from_mode(mode)).unwrap();
        } else if kind.is_file() {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

/// One move from `node`, on a fork of it: the fork and its state, every invariant checked.
fn advance(node: &Node, abs: &Abs, path: &[Ev], e: Ev, suspended: bool, open: bool) -> (Node, Abs) {
    let mut n = node.fork();
    n.apply(e);
    let mut path = path.to_vec();
    path.push(e);
    n.check(&path);
    let after = n.abs();
    // An envelope admits nothing past its cap, after its revocation, or while an incident on an
    // act it reads is open.
    if after.uses > abs.uses {
        assert!(
            abs.uses < CAP && !abs.revoked && !suspended,
            "{path:?}: admitted from {abs:?}"
        );
    }
    // No new act lands on the target of an open incident.
    if after.acts > abs.acts {
        assert!(!open, "{path:?}: an act on an incident's target");
    }
    (n, after)
}

/// Breadth-first from a fresh world to [`DEPTH`]: each state, with the shortest path to it. The
/// moves of one layer run on as many threads as there are cores, each on its own fork; the layer
/// is merged in order, so the paths are the same however the threads ran.
fn explore() -> BTreeMap<Abs, Vec<Ev>> {
    // A permissive umask, so a file rewritten at the umask's mode instead of its own is caught.
    rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o002));
    let start = Node::fresh();
    start.check(&[]);
    let mut seen = BTreeMap::from([(start.abs(), Vec::new())]);
    let mut frontier = vec![(start.abs(), start)];
    let threads = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
    for _ in 0..DEPTH {
        let guards: Vec<(bool, bool)> = frontier
            .iter()
            .map(|(_, node)| {
                let incidents = node.journal().incidents().unwrap();
                (
                    node.journal().suspends(&node.envelope()),
                    incidents.iter().any(mnemorph_config::Incident::is_open),
                )
            })
            .collect();
        let jobs: Vec<(usize, Ev)> = (0..frontier.len())
            .flat_map(|i| EVENTS.map(|e| (i, e)))
            .collect();
        let taken = AtomicUsize::new(0);
        let done: Mutex<Vec<(usize, Node, Abs)>> = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        let k = taken.fetch_add(1, Ordering::SeqCst);
                        let Some(&(i, e)) = jobs.get(k) else { break };
                        let (abs, node) = &frontier[i];
                        let (suspended, open) = guards[i];
                        let moved = advance(node, abs, &seen[abs], e, suspended, open);
                        done.lock().unwrap().push((k, moved.0, moved.1));
                    }
                });
            }
        });
        let mut done = done.into_inner().unwrap();
        done.sort_by_key(|(k, _, _)| *k);
        let mut next = Vec::new();
        for (k, n, after) in done {
            let (i, e) = jobs[k];
            if !seen.contains_key(&after) {
                let mut path = seen[&frontier[i].0].clone();
                path.push(e);
                seen.insert(after.clone(), path);
                next.push((after, n));
            }
        }
        frontier = next;
    }
    seen
}

#[test]
fn an_onis_act_its_envelope_and_its_incident_reach_their_states_and_no_others() {
    let states = explore();
    // The shortest witness of each.
    let find = |what: &str, f: &dyn Fn(&Abs) -> bool| {
        states
            .iter()
            .filter(|(a, _)| f(a))
            .map(|(_, p)| p.clone())
            .min_by_key(Vec::len)
            .unwrap_or_else(|| panic!("unreachable: {what}"))
    };
    let first = |a: &Abs, s: &str| a.first.as_deref() == Some(s);
    let open = |a: &Abs, cause: &str| {
        a.incident
            .as_ref()
            .is_some_and(|(c, m)| c == cause && !m.is_empty())
    };
    // Witnesses: every state the lifecycle intends, with its shortest path.
    let observed = find("observed", &|a| {
        first(a, "observed") && a.incident.is_none()
    });
    assert_eq!(observed, [Ev::Act]);
    find("reconcile", &|a| first(a, "reconcile"));
    find("cut, committed", &|a| first(a, "committed"));
    find("absent", &|a| first(a, "absent"));
    find("settled", &|a| first(a, "settled"));
    find("corrected", &|a| first(a, "corrected"));
    find("a rollback incident open", &|a| open(a, "rollback"));
    find("a correction incident open", &|a| open(a, "correction"));
    find("a reconcile incident open", &|a| open(a, "reconcile"));
    let closed = find("an incident closed", &|a| {
        a.incident.as_ref().is_some_and(|(_, m)| m.is_empty())
    });
    assert_eq!(closed, [Ev::Act, Ev::Undo, Ev::Tell, Ev::Lesson]);
    find("a reconcile incident closed", &|a| {
        a.incident
            .as_ref()
            .is_some_and(|(c, m)| c == "reconcile" && m.is_empty())
    });
    // A use is counted with the act's intent, under one lock: an act cut before it ran spends
    // one too, since it may have run.
    let cap = find("the envelope at its cap", &|a| a.uses == CAP);
    assert_eq!(cap.len(), CAP, "{cap:?}");
    find("the envelope revoked", &|a| a.revoked);
    find("an act the human asked for once the cap is spent", &|a| {
        a.uses == CAP && a.acts > CAP
    });
    // And nothing the lifecycle does not intend.
    for (a, path) in &states {
        // A cut act is probed before any other act: it never rests half-observed, and a typed
        // act is never left unobserved.
        assert!(
            !matches!(
                a.first.as_deref(),
                Some("pending observation" | "unobserved")
            ),
            "{path:?}: {a:?}"
        );
        // An absent act was never done: no incident is opened on it.
        if first(a, "absent") || first(a, "committed") {
            assert!(a.incident.is_none(), "{path:?}: {a:?}");
        }
        // An incident closes only once its act is corrected.
        if a.incident.as_ref().is_some_and(|(_, m)| m.is_empty()) {
            assert!(first(a, "corrected"), "{path:?}: {a:?}");
        }
    }
    println!("runner: {} states to depth {DEPTH}", states.len());
}

// ---------------------------------------------------------------------------------------------
// The envelope's own time and count ([`Envelope::admits`]), to its fixpoint: signed tomorrow,
// expiring three days on, two uses; each day passes, a use is taken while it admits, the human
// revokes it.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tick {
    Day,
    Use,
    Revoke,
}

/// Days passed, uses taken, revoked.
type Days = (i64, u64, bool);

#[test]
fn an_envelope_admits_only_in_force_under_its_cap_and_never_again_once_lapsed() {
    let start = Timestamp::now().day();
    let envelope = |revoked: bool| Envelope {
        name: "forge".into(),
        allow: vec!["gh pr edit * --repo me/proj --add-label *".into()],
        signed: start.plus(1),
        expires: start.plus(4),
        uses: Some(CAP as u64),
        revoked: revoked.then_some(start),
        world: None,
        repo: Some("me/proj".into()),
        post: None,
        body: None,
    };
    let reading = |s: Days| match envelope(s.2).admits(start.plus(s.0), s.1) {
        Ok(()) => "in force",
        Err(mnemorph_config::Lapse::NotYet(_)) => "not yet",
        Err(mnemorph_config::Lapse::Expired(_)) => "expired",
        Err(mnemorph_config::Lapse::Revoked(_)) => "revoked",
        Err(mnemorph_config::Lapse::Spent(_)) => "spent",
    };
    let mut seen: BTreeMap<Days, Vec<Tick>> = BTreeMap::from([((0, 0, false), Vec::new())]);
    let mut frontier = vec![(0, 0, false)];
    while !frontier.is_empty() {
        let s = frontier.remove(0);
        for t in [Tick::Day, Tick::Use, Tick::Revoke] {
            let n = match t {
                // Past expiry, more days change nothing it reads.
                Tick::Day => (s.0.min(5) + 1, s.1, s.2),
                Tick::Use if reading(s) == "in force" => (s.0, s.1 + 1, s.2),
                Tick::Use => s,
                Tick::Revoke => (s.0, s.1, true),
            };
            let (was, now) = (reading(s), reading(n));
            // Never past its cap; once revoked, expired or spent, never in force again.
            assert!(n.1 <= CAP as u64, "{n:?}");
            if matches!(was, "revoked" | "expired" | "spent") {
                assert_ne!(now, "in force", "{s:?} → {t:?}");
            }
            if !seen.contains_key(&n) {
                let mut p = seen[&s].clone();
                p.push(t);
                seen.insert(n, p);
                frontier.push(n);
            }
        }
    }
    let readings: BTreeSet<&str> = seen.keys().map(|s| reading(*s)).collect();
    assert_eq!(
        readings,
        ["expired", "in force", "not yet", "revoked", "spent"].into()
    );
    let spent = seen
        .iter()
        .filter(|(s, _)| reading(**s) == "spent")
        .map(|(_, p)| p.len())
        .min();
    assert_eq!(
        spent,
        Some(1 + CAP),
        "a day to come into force, then every use"
    );
}
