//! The act runner: a Kijin's typed outward act takes one path — post, plan, pin, gate, recheck,
//! intent, act, result, probe, receipt — and a cut act is probed before it is ever asked again.
//! Local bare repositories stand in for remotes, and a stub forge for GitHub: no network.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mnemorph_config::{
    Act, ENVELOPES_UNWIRED, Hands, Observer, Planned, PostJournal, Probe, Refusal, Request, Row,
    Runner, Sandboxed, Seen, State, reconciling,
};
use mnemorph_ledger::{Journal, Timestamp};
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
    deny: bool,
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
        if self.deny {
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
    // The human was asked once, bound to a plan, and told no envelope decides.
    let asks = f.human.asks.lock().unwrap().clone();
    assert_eq!(asks.len(), 1);
    assert_eq!(asks[0].class, ToolClass::Outward);
    assert!(asks[0].fingerprint.is_some());
    assert!(
        asks[0].why.iter().any(|w| w == ENVELOPES_UNWIRED),
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
        deny: true,
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
    comments: Vec<(u64, String)>,
    /// (event, actor, label, at)
    events: Vec<(String, String, String, String)>,
    spawns: usize,
    /// Die after the act, before its result is written.
    cut: bool,
}

impl Forge {
    fn new() -> Self {
        Self {
            state: Mutex::default(),
            login: "kijin-bot",
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
                    .map(
                        |(id, body)| serde_json::json!({"id": id, "body": body, "issue_url": issue})
                    )
                    .collect::<Vec<_>>()
            )
        } else if let Some(id) = path.strip_prefix("repos/me/proj/issues/comments/") {
            let (id, body) = s
                .comments
                .iter()
                .find(|(c, _)| c.to_string() == id)
                .unwrap();
            serde_json::json!({"id": id, "body": body, "issue_url": issue})
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
    ) -> Spawned {
        if let Err(e) = before() {
            return Spawned::Failed(Outcome::error(e));
        }
        let printed = match planned.act() {
            Act::AddLabel { label, .. } => {
                self.add_label(label, self.login);
                String::new()
            }
            Act::Comment { body, .. } => {
                let mut s = self.state.lock().unwrap();
                let id = 4000 + s.comments.len() as u64;
                s.comments.push((id, body.clone()));
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
        runner.undo(&ours.receipt).unwrap(),
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
    let err = runner.undo(&report.receipt).unwrap_err();
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
        .undo(&report.receipt)
        .unwrap_err();
    assert!(matches!(err, Refusal::Refused(_)), "{err:?}");
    assert_eq!(f.states().last(), Some(&State::Reconcile));
}
