//! The act runner: a Kijin's typed outward act takes one path — post, plan, pin, gate, recheck,
//! intent, act, result, probe, receipt — and a cut act is probed before it is ever asked again.
//! Local bare repositories stand in for remotes, and a stub forge for GitHub: no network.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mnemorph_config::{
    Act, Cause, Envelope, Hands, Incident, Lesson, Observer, Planned, PostJournal, Probe, Refusal,
    Request, Row, Runner, Sandboxed, Seen, State, reconciling,
};
use mnemorph_ledger::{Day, Journal, Timestamp};
use mnemorph_races::{Post, Race, ToolClass};
use mnemorph_tools::shell::{BeforeSpawn, Spawned};
use mnemorph_tools::{Answer, Ask, Context, Fingerprint, Gate, Outcome, Sandbox};

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

/// The human: answers every ask, counting them; may change the world while asked.
#[derive(Default)]
struct Human {
    asks: Mutex<Vec<Ask>>,
    deny: AtomicBool,
    /// Run on the n-th ask (from 1), before answering.
    meanwhile: Mutex<Option<Meanwhile>>,
}

/// What the human does while an ask is open, by its number.
type Meanwhile = Box<dyn FnMut(usize) + Send>;

impl Human {
    fn asked(&self) -> usize {
        self.asks.lock().unwrap().len()
    }
}

impl Gate for Human {
    fn ask(&self, ask: &Ask) -> Answer {
        let n = {
            let mut asks = self.asks.lock().unwrap();
            asks.push(ask.clone());
            asks.len()
        };
        if let Some(f) = self.meanwhile.lock().unwrap().as_mut() {
            f(n);
        }
        if self.deny.load(Ordering::SeqCst) {
            Answer::Deny("no".into())
        } else {
            Answer::Approve
        }
    }
}

/// A world with a commit, its `origin` a local bare repository.
struct Fixture {
    _tmp: tempfile::TempDir,
    world: PathBuf,
    home: PathBuf,
    remote: PathBuf,
    witness: PathBuf,
    head: String,
    human: Arc<Human>,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let (world, home, remote) = (root.join("world"), root.join("home"), root.join("r.git"));
        for d in [&world, &home, &remote] {
            fs::create_dir_all(d).unwrap();
        }
        git(&remote, &["init", "-q", "--bare"]);
        git(&world, &["init", "-q"]);
        fs::write(world.join("a.txt"), "one\n").unwrap();
        git(&world, &["add", "a.txt"]);
        git(&world, &["commit", "-qm", "one"]);
        let head = git(&world, &["rev-parse", "HEAD"]);
        git(
            &world,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        Self {
            _tmp: tmp,
            world,
            home,
            remote,
            witness: root.join("state/kijin/w/kijin-forge.jsonl"),
            head,
            human: Arc::new(Human::default()),
        }
    }

    fn cx(&self, body: &str) -> Context {
        Context::new(Race::Kijin, &self.world, &["."])
            .with_body(body)
            .with_home(&self.home)
            .with_gate(self.human.clone())
            .with_sandbox(Sandbox::None("tests run unguarded".into()))
            .with_unguarded(true)
    }

    fn journal(&self, post: Post) -> PostJournal {
        // Never the human's own state directory: the witness is the test's.
        PostJournal::of(&self.world, post).witnessed_at(&self.witness)
    }

    fn observer(&self) -> Observer {
        Observer::new(Sandbox::None("tests run unguarded".into()), &self.home)
    }

    /// Run `request` as `kijin-forge`, through `probe` and `hands`.
    fn act(
        &self,
        request: &Request,
        probe: &dyn Probe,
        hands: &dyn Hands,
    ) -> Result<mnemorph_config::Report, Refusal> {
        let cx = self.cx("kijin-forge");
        Runner::new(&cx, self.journal(Post::Forge), probe, hands).act(request)
    }

    /// Undo act `receipt` as `kijin-forge`, through `probe` and `hands`.
    fn undo(
        &self,
        receipt: &str,
        probe: &dyn Probe,
        hands: &dyn Hands,
    ) -> Result<mnemorph_config::Report, Refusal> {
        let cx = self.cx("kijin-forge");
        Runner::new(&cx, self.journal(Post::Forge), probe, hands).undo(receipt)
    }

    /// Push `feat/x` the harness's way: the real sandboxed spawn, the real probe.
    fn push(&self) -> Result<mnemorph_config::Report, Refusal> {
        self.act(&push_request(), &self.observer(), &Sandboxed)
    }

    /// The post's rows as its witness holds them: the record the tripwire rebuilds the world's
    /// journal from (in a test, the tripwire reads the human's own state directory instead, so
    /// the world's copy is not what is checked here).
    fn rows(&self) -> Vec<Row> {
        Journal::<Row>::new(&self.witness).read().unwrap().rows
    }

    fn states(&self) -> Vec<State> {
        self.rows().iter().map(|r| r.state).collect()
    }

    fn remote_branch(&self) -> Option<String> {
        let out = Command::new("git")
            .args(["rev-parse", "-q", "--verify", "refs/heads/feat/x"])
            .current_dir(&self.remote)
            .output()
            .unwrap();
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }
}

fn push_request() -> Request {
    Request {
        act: Act::PushNewBranch {
            remote: "origin".into(),
            name: "feat/x".into(),
        },
        key: None,
        tells: None,
    }
}

/// Hands that journal the intent, maybe do the act, and then die before the result is written:
/// a crash, a kill, a cut run.
struct Cut {
    act: bool,
}

impl Hands for Cut {
    fn spawn(
        &self,
        planned: &Planned,
        cx: &Context,
        _: Fingerprint,
        before: BeforeSpawn<'_>,
        _: Option<&[u8]>,
    ) -> Spawned {
        before().unwrap();
        if self.act {
            let ok = Command::new("bash")
                .args(["-c", planned.command()])
                .current_dir(cx.world())
                .status()
                .unwrap();
            assert!(ok.success());
        }
        panic!("the harness was killed between the act and its result");
    }
}

/// Run `f`, which is cut short by a panic, and say it was.
fn cut_short(f: impl FnOnce()) {
    assert!(
        catch_unwind(AssertUnwindSafe(f)).is_err(),
        "the run was not cut"
    );
}

/// A probe that cannot reach anything.
struct Dark;

impl Probe for Dark {
    fn run(&self, _: &[String], _: Duration) -> Result<Seen, String> {
        Err("could not resolve host".into())
    }
}

#[test]
fn the_full_pipeline_lands_observed() {
    let f = Fixture::new();
    let report = f.push().unwrap();
    assert_eq!(report.state, State::Observed, "{report:?}");
    assert!(!report.replayed);
    assert_eq!(report.exit, Some(0));
    assert_eq!(f.remote_branch().as_deref(), Some(f.head.as_str()));
    // The human was asked once, bound to a plan, and told why no envelope admitted it.
    let asks = f.human.asks.lock().unwrap().clone();
    assert_eq!(asks.len(), 1);
    assert_eq!(asks[0].class, ToolClass::Outward);
    assert!(asks[0].fingerprint.is_some());
    assert!(
        asks[0]
            .why
            .iter()
            .any(|w| w == "no envelope admits it: only an Oni acts under an envelope, and this body is a kijin"),
        "{:?}",
        asks[0].why
    );
    assert!(
        asks[0]
            .why
            .iter()
            .any(|w| w.starts_with("undo: delete branch feat/x"))
    );
    // Intent, result, receipt: one act, in order, bound to the plan the human approved.
    assert_eq!(
        f.states(),
        [State::Committed, State::PendingObservation, State::Observed]
    );
    let plan = asks[0].fingerprint.as_ref().unwrap().short();
    assert_eq!(f.rows()[0].plan.as_deref(), Some(plan.as_str()));
    // The witness, the intents and the lock are the human's alone.
    let dir = f.witness.parent().unwrap();
    assert_eq!(
        fs::metadata(dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file in [
        "kijin-forge.jsonl",
        "kijin-forge.intents.jsonl",
        "kijin-forge.lock",
    ] {
        let mode = fs::metadata(dir.join(file)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{file}");
    }
    // Nothing of the key or the pin's URLs is in the world.
    let journal = fs::read_to_string(f.journal(Post::Forge).path()).unwrap();
    let intents = fs::read_to_string(dir.join("kijin-forge.intents.jsonl")).unwrap();
    assert!(intents.contains("\"key\""));
    assert!(!journal.contains("\"key\""));
}

#[test]
fn a_world_changed_between_the_ask_and_the_spawn_is_asked_again() {
    let f = Fixture::new();
    let world = f.world.clone();
    *f.human.meanwhile.lock().unwrap() = Some(Box::new(move |n| {
        if n == 1 {
            fs::write(
                world.join("b.txt"),
                "written while the human read the card\n",
            )
            .unwrap();
        }
    }));
    let report = f.push().unwrap();
    assert_eq!(report.state, State::Observed);
    let asks = f.human.asks.lock().unwrap().clone();
    assert_eq!(asks.len(), 2, "the stale approval did not ask again");
    assert!(
        asks[1].why[0].starts_with("the world changed since approval"),
        "{:?}",
        asks[1].why
    );
    assert_ne!(asks[0].fingerprint, asks[1].fingerprint);
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("did not run and the gate asked again"))
    );
    // The stale pass journaled nothing: one intent, one act.
    assert_eq!(
        f.states(),
        [State::Committed, State::PendingObservation, State::Observed]
    );
}

#[test]
fn an_act_off_the_post_is_refused_before_the_gate() {
    let f = Fixture::new();
    let cx = f.cx("kijin-ci");
    let journal = PostJournal::of(&f.world, Post::Ci).witnessed_at(&f.witness);
    let err = Runner::new(&cx, journal, &f.observer(), &Sandboxed)
        .act(&push_request())
        .unwrap_err();
    let Refusal::Refused(why) = &err else {
        panic!("{err:?}");
    };
    assert!(
        why.contains("kijin-ci") && why.contains("push-branch"),
        "{why}"
    );
    assert_eq!(
        f.human.asked(),
        0,
        "the gate was asked for an act off the post"
    );
    assert!(f.remote_branch().is_none());
    assert_eq!(f.rows(), []);
}

#[test]
fn an_untyped_command_is_not_the_runners() {
    let f = Fixture::new();
    let err = f
        .act(
            &Request {
                act: Act::Command("gh pr merge 3".into()),
                key: None,
                tells: None,
            },
            &f.observer(),
            &Sandboxed,
        )
        .unwrap_err();
    assert!(
        matches!(err, Refusal::Refused(ref w) if w.contains("bash")),
        "{err:?}"
    );
    assert_eq!(f.human.asked(), 0);
}

#[test]
fn a_denied_act_runs_nothing_and_journals_nothing() {
    let mut f = Fixture::new();
    f.human = Arc::new(Human {
        deny: AtomicBool::new(true),
        ..Human::default()
    });
    assert!(matches!(f.push(), Err(Refusal::Denied(_))));
    assert!(f.remote_branch().is_none());
    assert_eq!(f.rows(), []);
}

#[test]
fn a_cut_push_that_landed_is_settled_and_never_acted_twice() {
    let f = Fixture::new();
    cut_short(|| {
        let _ = f.act(&push_request(), &f.observer(), &Cut { act: true });
    });
    assert_eq!(f.states(), [State::Committed]);
    assert_eq!(f.remote_branch().as_deref(), Some(f.head.as_str()));
    // The resumed run probes the cut act first: it landed, so nobody is asked and nothing runs.
    let report = f.push().unwrap();
    assert!(report.replayed, "{report:?}");
    assert_eq!(report.state, State::Observed);
    assert!(
        report.notes.iter().any(|n| n.contains("found landed")),
        "{report:?}"
    );
    assert_eq!(
        f.human.asked(),
        1,
        "the human was asked again for what had landed"
    );
    assert_eq!(f.states(), [State::Committed, State::Observed]);
    assert_eq!(reconciling(&f.world), Vec::<String>::new());
}

#[test]
fn a_cut_push_that_never_landed_is_asked_again_as_a_new_act() {
    let f = Fixture::new();
    cut_short(|| {
        let _ = f.act(&push_request(), &f.observer(), &Cut { act: false });
    });
    assert!(f.remote_branch().is_none());
    let report = f.push().unwrap();
    assert!(!report.replayed);
    assert_eq!(report.state, State::Observed);
    assert!(
        report.notes.iter().any(|n| n.contains("never landed")),
        "{report:?}"
    );
    assert_eq!(
        f.human.asked(),
        2,
        "an act that never landed is asked again"
    );
    assert_eq!(
        f.states(),
        [
            State::Committed,
            State::Absent,
            State::Committed,
            State::PendingObservation,
            State::Observed
        ]
    );
    assert_eq!(f.journal(Post::Forge).tally().unwrap().absent, 1);
}

#[test]
fn a_cut_act_the_probe_cannot_place_goes_to_the_human() {
    let f = Fixture::new();
    cut_short(|| {
        let _ = f.act(&push_request(), &f.observer(), &Cut { act: true });
    });
    let err = f.act(&push_request(), &Dark, &Sandboxed).unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("is reconcile")),
        "{err:?}"
    );
    assert_eq!(f.human.asked(), 1, "an act in reconcile was asked again");
    assert_eq!(f.states(), [State::Committed, State::Reconcile]);
    let status = reconciling(&f.world);
    assert_eq!(status.len(), 1, "{status:?}");
    assert!(status[0].contains("cannot place"), "{status:?}");
}

/// A stub of GitHub: labels, comments and their events on pull request `me/proj#12`, through
/// `gh api` reads (the probe) and `gh pr` writes (the hands).
struct Forge {
    state: Mutex<ForgeState>,
    login: &'static str,
}

#[derive(Default)]
struct ForgeState {
    labels: Vec<String>,
    /// (id, body, author)
    comments: Vec<(u64, String, String)>,
    /// (event, actor, label, at)
    events: Vec<(String, String, String, String)>,
    spawns: usize,
    /// Die after the act, before its result is written.
    cut: bool,
    /// Die before the act, its intent journaled.
    cut_before: bool,
    /// Undos spawned.
    undos: usize,
    /// An undo changes nothing.
    noop_undo: bool,
    /// What an undo exits with.
    undo_exit: i32,
}

impl Forge {
    fn new() -> Self {
        Self {
            state: Mutex::default(),
            login: "kijin-bot",
        }
    }

    fn post_comment(&self, body: &str, author: &str) -> u64 {
        let mut s = self.state.lock().unwrap();
        let id = 4000 + s.comments.len() as u64;
        s.comments.push((id, body.to_owned(), author.to_owned()));
        id
    }

    fn remove_label(&self, label: &str, actor: &str) {
        let mut s = self.state.lock().unwrap();
        if s.labels.iter().any(|l| l.eq_ignore_ascii_case(label)) {
            s.labels.retain(|l| !l.eq_ignore_ascii_case(label));
            s.events.push((
                "unlabeled".into(),
                actor.into(),
                label.into(),
                Timestamp::now().to_string(),
            ));
        }
    }

    fn add_label(&self, label: &str, actor: &str) {
        let mut s = self.state.lock().unwrap();
        if !s.labels.iter().any(|l| l.eq_ignore_ascii_case(label)) {
            s.labels.push(label.to_owned());
            s.events.push((
                "labeled".into(),
                actor.into(),
                label.into(),
                Timestamp::now().to_string(),
            ));
        }
    }
}

impl Probe for Forge {
    fn run(&self, argv: &[String], _: Duration) -> Result<Seen, String> {
        assert_eq!(&argv[..2], ["gh", "api"]);
        assert!(argv.contains(&"GET".to_owned()), "{argv:?}");
        let path = argv.last().unwrap();
        let s = self.state.lock().unwrap();
        let issue = "https://api.github.com/repos/me/proj/issues/12";
        let out = if path == "user" {
            serde_json::json!({"login": self.login})
        } else if path.starts_with("repos/me/proj/issues/12/labels") {
            serde_json::json!(
                s.labels
                    .iter()
                    .map(|l| serde_json::json!({"name": l}))
                    .collect::<Vec<_>>()
            )
        } else if path.starts_with("repos/me/proj/issues/12/comments") {
            serde_json::json!(
                s.comments
                    .iter()
                    .map(|(id, body, author)| {
                        serde_json::json!({"id": id, "body": body, "issue_url": issue, "user": {"login": author}})
                    })
                    .collect::<Vec<_>>()
            )
        } else if let Some(id) = path.strip_prefix("repos/me/proj/issues/comments/") {
            let (id, body, author) = s
                .comments
                .iter()
                .find(|(c, _, _)| c.to_string() == id)
                .unwrap();
            serde_json::json!({"id": id, "body": body, "issue_url": issue, "user": {"login": author}})
        } else if path == "repos/me/proj/pulls/12" {
            serde_json::json!({"head": {"ref": "feat/x", "repo": {"full_name": "me/proj"}}})
        } else if path.starts_with("repos/me/proj/issues/12/events") {
            serde_json::json!(s.events.iter().map(|(e, actor, label, at)| serde_json::json!({
                "event": e, "actor": {"login": actor}, "label": {"name": label}, "created_at": at
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
        assert!(
            !self.state.lock().unwrap().cut_before,
            "the harness was killed before the act"
        );
        if planned.is_undo() {
            let (noop, exit) = {
                let s = self.state.lock().unwrap();
                (s.noop_undo, s.undo_exit)
            };
            if !noop {
                match planned.act() {
                    Act::AddLabel { label, .. } => self.remove_label(label, self.login),
                    Act::Comment { .. } => {
                        let id: u64 = planned
                            .command()
                            .rsplit('/')
                            .next()
                            .unwrap()
                            .parse()
                            .unwrap();
                        self.state.lock().unwrap().comments.retain(|c| c.0 != id);
                    }
                    other => panic!("not a forge act: {other:?}"),
                }
            }
            self.state.lock().unwrap().undos += 1;
            let mut out = Outcome::ok(String::new(), ToolClass::Outward);
            out.meta.exit = Some(exit);
            return Spawned::Ran(out);
        }
        let printed = match planned.act() {
            Act::AddLabel { label, .. } => {
                self.add_label(label, self.login);
                String::new()
            }
            Act::Comment { body, .. } => {
                let id = self.post_comment(body, self.login);
                format!("https://github.com/me/proj/pull/12#issuecomment-{id}\n")
            }
            other => panic!("not a forge act: {other:?}"),
        };
        let cut = {
            let mut s = self.state.lock().unwrap();
            s.spawns += 1;
            s.cut
        };
        assert!(
            !cut,
            "the harness was killed between the act and its result"
        );
        let mut out = Outcome::ok(printed, ToolClass::Outward);
        out.meta.exit = Some(0);
        Spawned::Ran(out)
    }
}

fn comment(body: &str, key: Option<&str>) -> Request {
    Request {
        act: Act::Comment {
            repo: "me/proj".into(),
            pr: 12,
            body: body.into(),
        },
        key: key.map(str::to_owned),
        tells: None,
    }
}

fn label(name: &str) -> Request {
    Request {
        act: Act::AddLabel {
            repo: "me/proj".into(),
            pr: 12,
            label: name.into(),
        },
        key: None,
        tells: None,
    }
}

#[test]
fn the_same_request_returns_its_receipt_without_acting_again() {
    let f = Fixture::new();
    let forge = Forge::new();
    let first = f
        .act(&comment("Ready for review.", None), &forge, &forge)
        .unwrap();
    assert_eq!(first.state, State::Observed);
    let again = f
        .act(&comment("Ready for review.", None), &forge, &forge)
        .unwrap();
    assert!(again.replayed);
    assert_eq!(again.receipt, first.receipt);
    assert_eq!(
        forge.state.lock().unwrap().comments.len(),
        1,
        "commented twice"
    );
    assert_eq!(f.human.asked(), 1);
    // Another body is another request: it is asked and made.
    let other = f
        .act(&comment("CI is green.", None), &forge, &forge)
        .unwrap();
    assert!(!other.replayed);
    assert_eq!(forge.state.lock().unwrap().comments.len(), 2);
}

#[test]
fn a_key_given_to_another_request_is_a_conflict() {
    let f = Fixture::new();
    let forge = Forge::new();
    f.act(
        &comment("Summary: all green.", Some("summary")),
        &forge,
        &forge,
    )
    .unwrap();
    let err = f
        .act(
            &comment("Summary: two failures.", Some("summary")),
            &forge,
            &forge,
        )
        .unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("given to another request")),
        "{err:?}"
    );
    assert_eq!(f.human.asked(), 1, "a conflicting request reached the gate");
    assert_eq!(forge.state.lock().unwrap().comments.len(), 1);
    // The same name with the same body is the same request.
    let again = f
        .act(
            &comment("Summary: all green.", Some("summary")),
            &forge,
            &forge,
        )
        .unwrap();
    assert!(again.replayed);
}

#[test]
fn a_resume_after_a_landed_comment_does_not_comment_twice() {
    let f = Fixture::new();
    let forge = Forge::new();
    forge.state.lock().unwrap().cut = true;
    cut_short(|| {
        let _ = f.act(&comment("Ready for review.", None), &forge, &forge);
    });
    forge.state.lock().unwrap().cut = false;
    assert_eq!(f.states(), [State::Committed]);
    let report = f
        .act(&comment("Ready for review.", None), &forge, &forge)
        .unwrap();
    assert!(report.replayed, "{report:?}");
    let s = forge.state.lock().unwrap();
    assert_eq!((s.comments.len(), s.spawns), (1, 1), "commented twice");
    assert_eq!(f.human.asked(), 1);
}

#[test]
fn a_label_is_undone_only_while_it_is_still_the_acts_own() {
    let f = Fixture::new();
    let forge = Forge::new();
    let ours = f.act(&label("ready"), &forge, &forge).unwrap();
    assert_eq!(ours.state, State::Observed);
    let cx = f.cx("kijin-forge");
    let runner = Runner::new(&cx, f.journal(Post::Forge), &forge, &forge);
    assert_eq!(
        runner.undo_command(&ours.receipt).unwrap(),
        "gh pr edit 12 --repo me/proj --remove-label ready"
    );

    // Someone adds the label between the pin and the act: the act adds nothing, its probe sees
    // the label, and the undo would remove someone else's. It is not undone: reconcile.
    let f = Fixture::new();
    let theirs = Arc::new(Forge::new());
    let meanwhile = theirs.clone();
    *f.human.meanwhile.lock().unwrap() = Some(Box::new(move |_| {
        meanwhile.add_label("ready", "someone-else");
    }));
    let report = f.act(&label("ready"), &*theirs, &*theirs).unwrap();
    assert_eq!(report.state, State::Observed);
    let cx = f.cx("kijin-forge");
    let runner = Runner::new(&cx, f.journal(Post::Forge), &*theirs, &*theirs);
    let err = runner.undo_command(&report.receipt).unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("added by someone-else")),
        "{err:?}"
    );
    assert_eq!(f.states().last(), Some(&State::Reconcile));
    assert_eq!(theirs.state.lock().unwrap().labels, ["ready"]);
}

#[test]
fn a_label_whose_events_cannot_be_read_is_not_undone() {
    let f = Fixture::new();
    let forge = Forge::new();
    let report = f.act(&label("ready"), &forge, &forge).unwrap();
    let cx = f.cx("kijin-forge");
    let err = Runner::new(&cx, f.journal(Post::Forge), &Dark, &forge)
        .undo_command(&report.receipt)
        .unwrap_err();
    assert!(matches!(err, Refusal::Refused(_)), "{err:?}");
    assert_eq!(f.states().last(), Some(&State::Reconcile));
}

#[test]
fn someone_elses_identical_comment_is_never_taken_for_a_cut_act() {
    let f = Fixture::new();
    let forge = Forge::new();
    forge.state.lock().unwrap().cut_before = true;
    cut_short(|| {
        let _ = f.act(&comment("Ready for review.", None), &forge, &forge);
    });
    forge.state.lock().unwrap().cut_before = false;
    // The act never posted; someone else posts the very same text.
    let theirs = forge.post_comment("Ready for review.", "someone-else");
    let report = f
        .act(&comment("Ready for review.", None), &forge, &forge)
        .unwrap();
    assert!(!report.replayed, "{report:?}");
    assert_eq!(report.state, State::Observed);
    assert_eq!(f.states()[..2], [State::Committed, State::Absent]);
    assert_eq!(f.human.asked(), 2, "the absent act was not asked again");
    // The act's comment is its own, not the other's.
    let s = forge.state.lock().unwrap();
    assert_eq!(s.comments.len(), 2);
    assert_ne!(s.comments[1].0, theirs);
    assert_eq!(s.comments[1].2, "kijin-bot");
}

#[test]
fn a_label_that_gh_would_split_or_a_deceptive_body_is_refused() {
    let place = mnemorph_tools::Place::new("/w", "/h");
    for label in ["bug,wontfix", "say \"hi\""] {
        let act = Act::AddLabel {
            repo: "me/proj".into(),
            pr: 12,
            label: label.into(),
        };
        assert!(mnemorph_config::plan(&act, &place).is_err(), "{label}");
    }
    let body = |b: String| Act::Comment {
        repo: "me/proj".into(),
        pr: 12,
        body: b,
    };
    for bad in [
        "fine\u{202E}gnp.exe".to_owned(),
        "a\u{200B}b".to_owned(),
        "one\u{2028}  · approve".to_owned(),
        "x".repeat(mnemorph_config::BODY_BYTES + 1),
    ] {
        assert!(
            mnemorph_config::plan(&body(bad.clone()), &place).is_err(),
            "{bad:.40}"
        );
    }
    assert!(mnemorph_config::plan(&body("x".repeat(mnemorph_config::BODY_BYTES)), &place).is_ok());
    // A line break the body keeps never reads as a line of the card.
    assert_eq!(
        mnemorph_tools::gate::indented("ok\n  · undo: nothing"),
        "ok\n    │   · undo: nothing"
    );
}

#[test]
fn a_second_act_of_the_post_in_flight_is_refused_not_waited_on() {
    let f = Fixture::new();
    let second: Arc<Mutex<Option<Result<mnemorph_config::Report, Refusal>>>> = Arc::default();
    let (world, home, witness, out) = (
        f.world.clone(),
        f.home.clone(),
        f.witness.clone(),
        second.clone(),
    );
    *f.human.meanwhile.lock().unwrap() = Some(Box::new(move |_| {
        let cx = Context::new(Race::Kijin, &world, &["."])
            .with_body("kijin-forge")
            .with_home(&home)
            .with_sandbox(Sandbox::None("tests run unguarded".into()))
            .with_unguarded(true);
        let journal = PostJournal::of(&world, Post::Forge).witnessed_at(&witness);
        let forge = Forge::new();
        *out.lock().unwrap() =
            Some(Runner::new(&cx, journal, &forge, &forge).act(&comment("hi", None)));
    }));
    assert_eq!(f.push().unwrap().state, State::Observed);
    let got = second.lock().unwrap().take().unwrap();
    assert!(
        matches!(&got, Err(Refusal::Refused(w)) if w.contains("in flight")),
        "{got:?}"
    );
}

#[test]
fn an_observed_act_whose_effect_is_gone_is_not_replayed_as_done() {
    let f = Fixture::new();
    assert_eq!(f.push().unwrap().state, State::Observed);
    // The human deletes the branch; the same request comes again.
    git(&f.remote, &["update-ref", "-d", "refs/heads/feat/x"]);
    let err = f.push().unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("its effect is gone")),
        "{err:?}"
    );
    assert_eq!(f.human.asked(), 1);
    assert_eq!(f.states().last(), Some(&State::Reconcile));
    assert!(f.remote_branch().is_none());
}

/// Hands that journal the intent and then cannot start the command.
struct Broken;

impl Hands for Broken {
    fn spawn(
        &self,
        _: &Planned,
        _: &Context,
        _: Fingerprint,
        before: BeforeSpawn<'_>,
        _: Option<&[u8]>,
    ) -> Spawned {
        before().unwrap();
        Spawned::Failed(Outcome::error("could not start bash: no such file"))
    }
}

#[test]
fn a_command_that_cannot_start_after_its_intent_is_probed_and_the_lock_let_go() {
    let f = Fixture::new();
    let err = f.act(&push_request(), &f.observer(), &Broken).unwrap_err();
    assert!(
        matches!(&err, Refusal::Failed(w) if w.contains("never landed")),
        "{err:?}"
    );
    assert_eq!(f.states(), [State::Committed, State::Absent]);
    // The lock is free: the next act runs, and is asked anew.
    assert_eq!(f.push().unwrap().state, State::Observed);
    assert_eq!(f.human.asked(), 2);
}

// --- Rollback and closure -------------------------------------------------------------------

fn tells(body: &str, of: &str) -> Request {
    Request {
        tells: Some(of.to_owned()),
        ..comment(body, None)
    }
}

impl Fixture {
    fn incident(&self, act: &str) -> Incident {
        self.journal(Post::Forge)
            .incident(act)
            .unwrap()
            .expect("an incident on the act")
    }

    fn incidents(&self) -> Vec<Incident> {
        self.journal(Post::Forge).incidents().unwrap()
    }

    /// A lesson written in the world: a test.
    fn lesson(&self, act: &str) -> Incident {
        fs::create_dir_all(self.world.join("tests")).unwrap();
        fs::write(
            self.world.join("tests/rollback.rs"),
            "#[test]\nfn the_label_waits_for_review() {}\n",
        )
        .unwrap();
        let lesson = Lesson::test(&self.world, "tests/rollback.rs").unwrap();
        self.journal(Post::Forge).lesson(act, lesson).unwrap()
    }
}

fn envelope(allow: &str) -> Envelope {
    let today = Day::new(2026, 10, 27).unwrap();
    Envelope {
        name: "forge".into(),
        allow: vec![allow.to_owned()],
        signed: today.plus(-1),
        expires: today.plus(29),
        uses: None,
        revoked: None,
        world: None,
        repo: None,
        post: None,
        body: None,
    }
}

#[test]
fn an_undo_runs_only_once_the_human_approves_and_is_done_when_the_branch_is_gone() {
    let f = Fixture::new();
    let act = f.push().unwrap().receipt;
    // Denied: nothing runs, nothing is journaled, no incident opens.
    f.human.deny.store(true, Ordering::SeqCst);
    let denied = f.undo(&act, &f.observer(), &Sandboxed).unwrap_err();
    assert!(matches!(denied, Refusal::Denied(_)), "{denied:?}");
    assert_eq!(f.remote_branch().as_deref(), Some(f.head.as_str()));
    assert_eq!(f.rows().len(), 3);
    assert_eq!(f.incidents(), []);
    // Approved: the harness's undo, leased on the pushed commit; the probe sees the branch gone.
    f.human.deny.store(false, Ordering::SeqCst);
    let undo = f.undo(&act, &f.observer(), &Sandboxed).unwrap();
    assert_eq!(undo.state, State::Observed, "{undo:?}");
    assert!(f.remote_branch().is_none());
    let asks = f.human.asks.lock().unwrap().clone();
    assert_eq!(asks.len(), 3, "push, denied undo, approved undo");
    let card = &asks[2];
    assert!(card.fingerprint.is_some());
    assert!(
        card.act
            .contains(&format!("--force-with-lease=refs/heads/feat/x:{}", f.head)),
        "{}",
        card.act
    );
    assert!(
        card.why[0].starts_with("the undo of `git push"),
        "{:?}",
        card.why
    );
    // The undo's rows name the act; the act's own row is superseded, appended, by `corrected`.
    let rows = f.rows();
    let undo_rows: Vec<&Row> = rows.iter().filter(|r| r.act == undo.receipt).collect();
    assert_eq!(
        undo_rows.iter().map(|r| r.state).collect::<Vec<_>>(),
        [State::Committed, State::PendingObservation, State::Observed]
    );
    assert!(
        undo_rows
            .iter()
            .all(|r| r.undoes.as_deref() == Some(act.as_str()))
    );
    let last = rows.last().unwrap();
    assert_eq!(
        (last.act.as_str(), last.state),
        (act.as_str(), State::Corrected)
    );
    assert_eq!(last.by.as_deref(), Some(undo.receipt.as_str()));
    // Receipts (1) and (2); the incident stays open.
    let inc = f.incident(&act);
    assert_eq!(inc.cause, Cause::Rollback);
    assert_eq!(inc.undone.as_deref(), Some(undo.receipt.as_str()));
    assert!(inc.corrected.is_some());
    assert_eq!(inc.missing(), ["downstream told", "lesson"]);
    // A corrected act is undone no more.
    let again = f.undo(&act, &f.observer(), &Sandboxed).unwrap_err();
    assert!(
        matches!(&again, Refusal::Refused(w) if w.contains("is corrected")),
        "{again:?}"
    );
}

#[test]
fn an_undo_whose_lease_fails_is_reconcile_and_never_undone() {
    let f = Fixture::new();
    let act = f.push().unwrap().receipt;
    // Someone moves the branch after the act: the lease on the pushed commit refuses the delete.
    git(&f.world, &["commit", "-q", "--allow-empty", "-m", "two"]);
    git(
        &f.world,
        &["push", "-q", "-f", "origin", "HEAD:refs/heads/feat/x"],
    );
    let moved = f.remote_branch().unwrap();
    assert_ne!(moved, f.head);
    let undo = f.undo(&act, &f.observer(), &Sandboxed).unwrap();
    assert_eq!(undo.state, State::Reconcile, "{undo:?}");
    assert_ne!(undo.exit, Some(0));
    assert_eq!(
        f.remote_branch(),
        Some(moved),
        "someone else's branch was deleted"
    );
    assert!(!f.states().contains(&State::Corrected));
    let inc = f.incident(&act);
    assert_eq!(inc.undone, None);
    assert_eq!(inc.corrected, None);
    assert!(inc.is_open());
}

#[test]
fn an_undo_whose_effect_is_still_seen_is_reconcile() {
    let f = Fixture::new();
    let forge = Forge::new();
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    // It exits 0 and leaves the label: the exit is not the receipt.
    forge.state.lock().unwrap().noop_undo = true;
    let undo = f.undo(&act, &forge, &forge).unwrap();
    assert_eq!((undo.state, undo.exit), (State::Reconcile, Some(0)));
    assert_eq!(forge.state.lock().unwrap().labels, ["ready"]);
    let inc = f.incident(&act);
    assert_eq!((inc.undone, inc.corrected), (None, None));
}

#[test]
fn an_undo_that_exits_other_than_0_is_reconcile_even_when_the_effect_is_gone() {
    let f = Fixture::new();
    let forge = Forge::new();
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    forge.state.lock().unwrap().undo_exit = 1;
    let undo = f.undo(&act, &forge, &forge).unwrap();
    assert_eq!(undo.state, State::Reconcile);
    assert_eq!(forge.state.lock().unwrap().labels, Vec::<String>::new());
    assert!(!f.states().contains(&State::Corrected));
    assert_eq!(f.incident(&act).undone, None);
}

#[test]
fn an_incident_stays_open_with_three_receipts_and_closes_with_the_fourth() {
    let f = Fixture::new();
    let forge = Forge::new();
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    let undo = f.undo(&act, &forge, &forge).unwrap();
    assert_eq!(undo.state, State::Observed, "{undo:?}");
    assert_eq!(forge.state.lock().unwrap().labels, Vec::<String>::new());
    // (3) downstream told, through the runner like any comment.
    let told = f
        .act(
            &tells("The `ready` label was a mistake: removed.", &act),
            &forge,
            &forge,
        )
        .unwrap();
    assert_eq!(told.state, State::Observed);
    let inc = f.incident(&act);
    assert_eq!(inc.told.as_deref(), Some(told.receipt.as_str()));
    assert_eq!(inc.missing(), ["lesson"]);
    assert!(inc.is_open(), "three receipts closed the incident");
    // (4) the lesson closes it.
    let inc = f.lesson(&act);
    assert!(matches!(inc.lesson, Some(Lesson::Test { .. })), "{inc:?}");
    assert_eq!(inc.missing(), Vec::<&str>::new());
    assert!(!inc.is_open());
    assert_ne!(inc.line(), "");
}

#[test]
fn receipts_forged_in_the_world_or_unbacked_by_the_witness_count_for_nothing() {
    let f = Fixture::new();
    let forge = Forge::new();
    // A comment made before any incident, not telling of one.
    let early = f
        .act(&comment("Looks good.", None), &forge, &forge)
        .unwrap()
        .receipt;
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    forge.state.lock().unwrap().noop_undo = true;
    assert_eq!(
        f.undo(&act, &forge, &forge).unwrap().state,
        State::Reconcile
    );
    // A body writes what would be receipts (1) and (2) into the world's journal.
    let world = Journal::<Row>::new(f.journal(Post::Forge).path());
    let mut undone = Row {
        undoes: Some(act.clone()),
        ..Row {
            act: "f0f0f0f0f0f0".into(),
            state: State::Observed,
            at: Timestamp::now(),
            command: None,
            target: Vec::new(),
            plan: None,
            exit: Some(0),
            expect: None,
            probe: None,
            seen: None,
            why: Some("the probe saw the effect gone".into()),
            undoes: None,
            by: None,
        }
    };
    world.append(&undone).unwrap();
    undone.act.clone_from(&act);
    undone.state = State::Corrected;
    undone.undoes = None;
    undone.by = Some("f0f0f0f0f0f0".into());
    world.append(&undone).unwrap();
    // And entries naming receipts the witness does not hold as such: a made-up undo, an observed
    // act that is no undo, and a comment that told of nothing.
    let file = f.witness.with_extension("incidents.jsonl");
    let at = serde_json::to_value(Timestamp::now()).unwrap();
    let mut text = fs::read_to_string(&file).unwrap();
    for (event, receipt) in [
        ("undone", "f0f0f0f0f0f0"),
        ("undone", act.as_str()),
        ("told", early.as_str()),
        ("told", act.as_str()),
    ] {
        text.push_str(
            &serde_json::json!({"event": event, "act": act, "receipt": receipt, "at": at})
                .to_string(),
        );
        text.push('\n');
    }
    fs::write(&file, text).unwrap();
    let inc = f.incident(&act);
    assert_eq!(inc.missing().len(), 4, "a forged receipt counted: {inc:?}");
}

#[test]
fn an_open_incident_suspends_the_envelopes_that_read_its_act_until_it_closes() {
    let f = Fixture::new();
    let forge = Forge::new();
    let labels = envelope("gh pr edit * --repo me/proj --add-label *");
    let branches = envelope("git push origin feat/*");
    let journal = f.journal(Post::Forge);
    assert!(!journal.suspends(&labels));
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    assert!(
        !journal.suspends(&labels),
        "an act with no incident suspended its envelope"
    );
    f.undo(&act, &forge, &forge).unwrap();
    assert!(journal.suspends(&labels));
    assert!(
        !journal.suspends(&branches),
        "an envelope that never read the act was suspended"
    );
    f.act(&tells("Removed the label.", &act), &forge, &forge)
        .unwrap();
    assert!(
        journal.suspends(&labels),
        "suspension lifted before the fourth receipt"
    );
    f.lesson(&act);
    assert!(!journal.suspends(&labels), "suspension kept after closure");
    // Incidents that cannot be read suspend every envelope.
    fs::remove_file(f.witness.with_extension("incidents.jsonl")).unwrap();
    fs::create_dir(f.witness.with_extension("incidents.jsonl")).unwrap();
    assert!(journal.suspends(&branches));
}

#[test]
fn a_new_act_on_the_target_of_an_open_incident_is_refused_until_it_closes() {
    let f = Fixture::new();
    let forge = Forge::new();
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    f.undo(&act, &forge, &forge).unwrap();
    let asked = f.human.asked();
    for request in [label("ready"), label("other"), comment("Done.", None)] {
        let err = f.act(&request, &forge, &forge).unwrap_err();
        assert!(
            matches!(&err, Refusal::Refused(w) if w.contains("is open on this target")),
            "{err:?}"
        );
    }
    assert_eq!(f.human.asked(), asked, "a refused act reached the gate");
    // Another target is not the incident's.
    assert_eq!(f.push().unwrap().state, State::Observed);
    // A comment that tells of nothing open is refused.
    let err = f
        .act(&tells("x", "f0f0f0f0f0f0"), &forge, &forge)
        .unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("no open incident")),
        "{err:?}"
    );
    f.act(&tells("Removed the label.", &act), &forge, &forge)
        .unwrap();
    f.lesson(&act);
    // Closed: the same request is asked again as a new act.
    let again = f.act(&label("ready"), &forge, &forge).unwrap();
    assert!(!again.replayed);
    assert_eq!(again.state, State::Observed);
}

#[test]
fn a_comment_is_undone_only_while_it_is_still_the_acts_own() {
    let f = Fixture::new();
    let forge = Forge::new();
    let act = f
        .act(&comment("Ready.", None), &forge, &forge)
        .unwrap()
        .receipt;
    // Edited since the act: not undone, and the human is not asked.
    let asked = f.human.asked();
    let body = forge.state.lock().unwrap().comments[0].1.clone();
    forge.state.lock().unwrap().comments[0].1 = "Ready, says someone else.".into();
    let err = f.undo(&act, &forge, &forge).unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("edited")),
        "{err:?}"
    );
    assert_eq!(f.human.asked(), asked);
    assert_eq!(f.states().last(), Some(&State::Reconcile));
    assert_eq!(forge.state.lock().unwrap().undos, 0);
    // Its own again: a `reconcile` is undone too, after approval, and the comment is gone.
    forge.state.lock().unwrap().comments[0].1 = body;
    let undo = f.undo(&act, &forge, &forge).unwrap();
    assert_eq!(undo.state, State::Observed, "{undo:?}");
    assert_eq!(forge.state.lock().unwrap().comments.len(), 0);
    assert_eq!(f.human.asked(), asked + 1);
}

#[test]
fn the_human_marks_a_reconcile_an_incident_or_settles_it_as_none() {
    let f = Fixture::new();
    cut_short(|| {
        let _ = f.act(&push_request(), &f.observer(), &Cut { act: true });
    });
    f.act(&push_request(), &Dark, &Sandboxed).unwrap_err();
    let act = f.rows()[0].act.clone();
    assert_eq!(f.states().last(), Some(&State::Reconcile));
    let journal = f.journal(Post::Forge);
    // Settled as no incident: a `settled` row, nothing undone.
    let g = Fixture::new();
    cut_short(|| {
        let _ = g.act(&push_request(), &g.observer(), &Cut { act: true });
    });
    g.act(&push_request(), &Dark, &Sandboxed).unwrap_err();
    let other = g.rows()[0].act.clone();
    g.journal(Post::Forge)
        .settle(&other, Some("I pushed it by hand"))
        .unwrap();
    assert_eq!(g.states().last(), Some(&State::Settled));
    assert_eq!(g.incidents(), []);
    // Marked an incident: all four receipts missing, its reason escaped on the human's line.
    let inc = journal
        .open_incident(&act, Some("pushed\nto the wrong remote"))
        .unwrap();
    assert_eq!(inc.cause, Cause::Reconcile);
    assert_eq!(inc.missing().len(), 4);
    let line = inc.line();
    assert!(
        line.contains("missing undone, journal corrected, downstream told, lesson"),
        "{line}"
    );
    assert!(
        line.contains("pushed\\nto") && !line.contains('\n'),
        "{line}"
    );
    // An incident is not settled, nor opened twice.
    assert!(journal.settle(&act, None).is_err());
    assert!(journal.open_incident(&act, None).is_err());
    // A reconcile is undone like an observed act, only once approved.
    let undo = f.undo(&act, &f.observer(), &Sandboxed).unwrap();
    assert_eq!(undo.state, State::Observed, "{undo:?}");
    assert!(f.remote_branch().is_none());
    assert_eq!(f.incident(&act).cause, Cause::Reconcile);
}

#[test]
fn a_lesson_must_be_written_in_the_world() {
    let f = Fixture::new();
    for bad in ["../outside.rs", "/etc/passwd", "missing.rs", ""] {
        assert!(Lesson::test(&f.world, bad).is_err(), "{bad}");
    }
    fs::write(f.world.join("card.md"), "# Card\nVerify: cargo test -p x\n").unwrap();
    assert!(Lesson::verify(&f.world, "card.md", 1).is_err());
    assert!(Lesson::verify(&f.world, "card.md", 3).is_err());
    assert!(matches!(
        Lesson::verify(&f.world, "card.md", 2),
        Ok(Lesson::Verify { line: 2, .. })
    ));
    std::os::unix::fs::symlink("/etc/hostname", f.world.join("link.rs")).unwrap();
    assert!(Lesson::test(&f.world, "link.rs").is_err());
    assert!(Lesson::thought("a b", "text", 0).is_err());
    assert!(Lesson::thought("19a-ff", " ", 0).is_err());
    assert!(Lesson::thought("19a-ff", "Ask before labelling.", 0).is_ok());
    // No incident, no lesson.
    let lesson = Lesson::verify(&f.world, "card.md", 2).unwrap();
    assert!(
        f.journal(Post::Forge)
            .lesson("f0f0f0f0f0f0", lesson)
            .is_err()
    );
}

#[test]
fn an_effect_already_gone_is_recorded_undone_once_approved_and_nothing_runs() {
    let f = Fixture::new();
    let act = f.push().unwrap().receipt;
    // The human deletes the branch by hand.
    git(&f.remote, &["update-ref", "-d", "refs/heads/feat/x"]);
    let undo = f.undo(&act, &f.observer(), &Broken).unwrap();
    assert_eq!(undo.state, State::Observed, "{undo:?}");
    let asks = f.human.asks.lock().unwrap().clone();
    assert_eq!(asks.len(), 2);
    assert!(asks[1].act.contains("nothing runs"), "{}", asks[1].act);
    let inc = f.incident(&act);
    assert_eq!(inc.undone.as_deref(), Some(undo.receipt.as_str()));
    assert!(inc.corrected.is_some());
}

#[test]
fn an_undo_whose_remote_now_points_elsewhere_is_refused_before_the_gate() {
    let f = Fixture::new();
    let act = f.push().unwrap().receipt;
    let other = f.remote.with_file_name("other.git");
    fs::create_dir_all(&other).unwrap();
    git(&other, &["init", "-q", "--bare"]);
    git(
        &f.world,
        &["remote", "set-url", "origin", other.to_str().unwrap()],
    );
    let err = f.undo(&act, &f.observer(), &Sandboxed).unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("the undo would go to")),
        "{err:?}"
    );
    assert_eq!(f.human.asked(), 1);
    assert_eq!(f.remote_branch().as_deref(), Some(f.head.as_str()));
    assert_eq!(f.incidents(), []);
}

#[test]
fn a_telling_comment_lands_only_on_its_own_incidents_pull_request() {
    let f = Fixture::new();
    let forge = Forge::new();
    // Two incidents: a label on me/proj#12, and a branch pushed to a local remote.
    let label_act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    f.undo(&label_act, &forge, &forge).unwrap();
    let branch = f.push().unwrap().receipt;
    f.undo(&branch, &f.observer(), &Sandboxed).unwrap();
    let asked = f.human.asked();
    // Telling of the branch on the pull request another incident suspends: refused.
    let err = f
        .act(&tells("Deleted feat/x.", &branch), &forge, &forge)
        .unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("telling of another incident")),
        "{err:?}"
    );
    // Telling of the label on another pull request: refused.
    let elsewhere = Request {
        act: Act::Comment {
            repo: "me/other".into(),
            pr: 3,
            body: "Removed the label.".into(),
        },
        key: None,
        tells: Some(label_act.clone()),
    };
    let err = f.act(&elsewhere, &forge, &forge).unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("not the act's pull request")),
        "{err:?}"
    );
    // The label's incident told and closed, the branch's telling on #12 is still refused: no push
    // URL of a local remote names a repository the pull request could be from.
    f.act(&tells("Removed the label.", &label_act), &forge, &forge)
        .unwrap();
    f.lesson(&label_act);
    let err = f
        .act(&tells("Deleted feat/x.", &branch), &forge, &forge)
        .unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("no push URL")),
        "{err:?}"
    );
    assert_eq!(
        f.human.asked(),
        asked + 1,
        "a refused telling reached the gate"
    );
}

#[test]
fn a_telling_comment_opens_with_the_harnesss_words_or_it_tells_nothing() {
    let f = Fixture::new();
    let forge = Forge::new();
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    f.undo(&act, &forge, &forge).unwrap();
    let err = f.act(&tells("  ", &act), &forge, &forge).unwrap_err();
    assert!(matches!(err, Refusal::Refused(_)), "{err:?}");
    let told = f.act(&tells("Removed it.", &act), &forge, &forge).unwrap();
    let body = forge
        .state
        .lock()
        .unwrap()
        .comments
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(body, format!("Rolled back act {act}: Removed it."));
    assert_eq!(
        f.incident(&act).told.as_deref(),
        Some(told.receipt.as_str())
    );
    // An intent whose body lost the harness's words tells nothing.
    let intents = f.witness.with_extension("intents.jsonl");
    let text = fs::read_to_string(&intents).unwrap();
    fs::write(&intents, text.replace("Rolled back act ", "Rolled act ")).unwrap();
    assert_eq!(f.incident(&act).told, None);
}

#[test]
fn a_lesson_is_written_after_the_incident_and_dropped_when_it_changes() {
    let f = Fixture::new();
    let forge = Forge::new();
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    f.undo(&act, &forge, &forge).unwrap();
    let journal = f.journal(Post::Forge);
    // A file the world held unchanged when the incident opened is no lesson.
    let old = Lesson::test(&f.world, "a.txt").unwrap();
    let err = journal.lesson(&act, old).unwrap_err();
    assert!(err.contains("unchanged since the incident opened"), "{err}");
    // Nor a Thought born before it.
    let early = Lesson::thought("19a-ff", "Ask before labelling.", 1000).unwrap();
    assert!(
        journal
            .lesson(&act, early)
            .unwrap_err()
            .contains("born before")
    );
    // A file changed since is one, and the human sees the change.
    fs::write(f.world.join("a.txt"), "one\nlabels wait for review\n").unwrap();
    let edited = Lesson::test(&f.world, "a.txt").unwrap();
    let diff = journal.lesson_change(&act, &edited).unwrap();
    assert!(diff.contains("+labels wait for review"), "{diff}");
    let inc = journal.lesson(&act, edited).unwrap();
    assert!(inc.lesson.is_some());
    // Edited after it was recorded, it counts no more.
    fs::write(f.world.join("a.txt"), "one\n").unwrap();
    assert_eq!(f.incident(&act).lesson, None);
}

#[test]
fn a_comment_edited_while_the_human_reads_the_card_is_not_undone() {
    let f = Fixture::new();
    let forge = Arc::new(Forge::new());
    let act = f
        .act(&comment("Ready.", None), &*forge, &*forge)
        .unwrap()
        .receipt;
    let meanwhile = forge.clone();
    *f.human.meanwhile.lock().unwrap() = Some(Box::new(move |n| {
        if n == 2 {
            meanwhile.state.lock().unwrap().comments[0].1 = "Edited meanwhile.".into();
        }
    }));
    let err = f.undo(&act, &*forge, &*forge).unwrap_err();
    assert!(
        matches!(&err, Refusal::Failed(w) if w.contains("edited")),
        "{err:?}"
    );
    let s = forge.state.lock().unwrap();
    assert_eq!(
        (s.comments.len(), s.undos),
        (1, 0),
        "the edited comment was deleted"
    );
}

#[test]
fn a_correction_counts_only_while_it_is_the_acts_latest_row() {
    let f = Fixture::new();
    let forge = Forge::new();
    let act = f.act(&label("ready"), &forge, &forge).unwrap().receipt;
    f.undo(&act, &forge, &forge).unwrap();
    assert!(f.incident(&act).corrected.is_some());
    let mut later = Row {
        act: act.clone(),
        state: State::Reconcile,
        at: Timestamp::now(),
        command: None,
        target: Vec::new(),
        plan: None,
        exit: None,
        expect: None,
        probe: None,
        seen: None,
        why: Some("seen again".into()),
        undoes: None,
        by: None,
    };
    Journal::<Row>::new(&f.witness).append(&later).unwrap();
    assert_eq!(f.incident(&act).corrected, None);
    later.state = State::Corrected;
    later.by = Some("f0f0f0f0f0f0".into());
    Journal::<Row>::new(&f.witness).append(&later).unwrap();
    assert_eq!(
        f.incident(&act).corrected,
        None,
        "a correction naming another undo counted"
    );
}
