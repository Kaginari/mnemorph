//! The receipt: an outward act is `observed` only when a read-only probe, run by the harness on
//! the target pinned at act time, sees the effect the plan promised. Exit 0 is never enough.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use mnemorph_config::{
    Act, Done, Observer, Pin, PostJournal, Probe, Receipt, Row, Seen, State, plan, reconciling,
};
use mnemorph_ledger::Journal;
use mnemorph_races::{Post, ToolClass};
use mnemorph_tools::{Place, Sandbox};

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

/// A world with two commits, its `origin` a local bare repository, and a second bare repository
/// nothing points at yet.
struct Fixture {
    _tmp: tempfile::TempDir,
    world: PathBuf,
    home: PathBuf,
    remote: PathBuf,
    other: PathBuf,
    first: String,
    head: String,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let (world, home) = (root.join("world"), root.join("home"));
        let (remote, other) = (root.join("remote.git"), root.join("other.git"));
        for d in [&world, &home, &remote, &other] {
            fs::create_dir_all(d).unwrap();
        }
        git(&remote, &["init", "-q", "--bare"]);
        git(&other, &["init", "-q", "--bare"]);
        git(&world, &["init", "-q"]);
        fs::write(world.join("a.txt"), "one\n").unwrap();
        git(&world, &["add", "a.txt"]);
        git(&world, &["commit", "-qm", "one"]);
        let first = git(&world, &["rev-parse", "HEAD"]);
        fs::write(world.join("a.txt"), "two\n").unwrap();
        git(&world, &["commit", "-qam", "two"]);
        let head = git(&world, &["rev-parse", "HEAD"]);
        git(
            &world,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        // The first commit is on the remote already, so a branch can be moved back onto it.
        git(
            &world,
            &["push", "-q", "origin", &format!("{first}:refs/heads/base")],
        );
        Self {
            _tmp: tmp,
            world,
            home,
            remote,
            other,
            first,
            head,
        }
    }

    fn planned(&self, act: &Act) -> mnemorph_config::Planned {
        plan(act, &Place::new(&self.world, &self.home)).unwrap()
    }

    fn journal(&self) -> PostJournal {
        PostJournal::of(&self.world, Post::Forge)
    }

    fn observer(&self) -> Observer {
        Observer::new(Sandbox::None("tests run unguarded".into()), &self.home)
    }

    /// Plan, pin, commit and run a push of `feat/x`, as the harness would: the receipt pending.
    fn pushed(&self) -> Receipt {
        let planned = self.planned(&branch("feat/x"));
        let pin = planned.pin(&self.world).unwrap();
        assert_eq!(
            pin,
            Pin::Push {
                urls: vec![self.remote.to_str().unwrap().to_owned()],
                sha: self.head.clone(),
            }
        );
        let journal = self.journal();
        let mut receipt = journal
            .commit(&planned, pin, Some("plan 3f9a1c2e4b5d"))
            .unwrap();
        assert_eq!(receipt.state(), State::Committed);
        let out = Command::new("bash")
            .args(["-c", planned.command()])
            .current_dir(&self.world)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        journal
            .ran(
                &mut receipt,
                out.status.code(),
                &String::from_utf8_lossy(&out.stdout),
            )
            .unwrap();
        receipt
    }

    fn rows(&self) -> Vec<Row> {
        Journal::<Row>::new(self.journal().path())
            .read()
            .unwrap()
            .rows
    }
}

fn branch(name: &str) -> Act {
    Act::PushNewBranch {
        remote: "origin".into(),
        name: name.into(),
    }
}

/// A probe that answers from a script and remembers what it was asked to run.
struct Fake {
    answer: Result<Seen, String>,
    asked: RefCell<Vec<Vec<String>>>,
}

impl Fake {
    fn says(out: &str) -> Self {
        Self::exits(0, out)
    }

    fn exits(code: i32, out: &str) -> Self {
        Self {
            answer: Ok(Seen {
                code: Some(code),
                out: out.as_bytes().to_vec(),
                err: b"HTTP 404: Not Found".to_vec(),
                cut: false,
            }),
            asked: RefCell::default(),
        }
    }

    fn fails(why: &str) -> Self {
        Self {
            answer: Err(why.to_owned()),
            asked: RefCell::default(),
        }
    }
}

impl Probe for Fake {
    fn run(&self, argv: &[String], _timeout: Duration) -> Result<Seen, String> {
        self.asked.borrow_mut().push(argv.to_vec());
        self.answer.clone()
    }
}

const PATIENCE: Duration = Duration::from_secs(20);

#[test]
fn a_push_the_remote_holds_at_the_pushed_commit_is_observed() {
    let f = Fixture::new();
    let mut r = f.pushed();
    // Exit 0 alone is never done: the act waits on its probe.
    assert_eq!(r.state(), State::PendingObservation);
    assert_eq!(
        r.done(),
        Some(&Done::Pushed {
            sha: f.head.clone()
        })
    );
    assert_eq!(
        f.journal()
            .observe(&mut r, &f.observer(), PATIENCE)
            .unwrap(),
        State::Observed
    );
    let rows = f.rows();
    let states: Vec<State> = rows.iter().map(|r| r.state).collect();
    assert_eq!(
        states,
        [State::Committed, State::PendingObservation, State::Observed]
    );
    assert!(rows.iter().all(|row| row.act == r.id()));
    let last = rows.last().unwrap();
    let probe = last.probe.as_deref().unwrap();
    assert!(
        probe.contains(" ls-remote --end-of-options ") && probe.ends_with("refs/heads/feat/x"),
        "{probe}"
    );
    assert!(probe.contains("protocol.allow=never"), "{probe}");
    assert!(last.seen.as_deref().unwrap().contains(&f.head), "{last:?}");
    assert_eq!(rows[0].plan.as_deref(), Some("plan 3f9a1c2e4b5d"));
    assert_eq!(reconciling(&f.world), Vec::<String>::new());
}

#[test]
fn a_remote_branch_at_another_commit_is_reconcile_never_observed() {
    let f = Fixture::new();
    let mut r = f.pushed();
    // Someone moved the branch after the act: the push "succeeded", the effect is not the plan's.
    git(&f.remote, &["update-ref", "refs/heads/feat/x", &f.first]);
    assert_eq!(
        f.journal()
            .observe(&mut r, &f.observer(), PATIENCE)
            .unwrap(),
        State::Reconcile
    );
    let why = f.rows().last().unwrap().why.clone().unwrap();
    assert!(
        why.contains(&format!("at {}, not the pushed {}", f.first, f.head)),
        "{why}"
    );
    // The human is shown it.
    let status = reconciling(&f.world);
    assert_eq!(status.len(), 1, "{status:?}");
    assert!(
        status[0].starts_with(&format!("kijin-forge act {} is in reconcile", r.id())),
        "{status:?}"
    );
    let tally = f.journal().tally().unwrap();
    assert_eq!((tally.reconcile, tally.observed), (1, 0));
}

#[test]
fn a_branch_missing_from_the_remote_is_reconcile() {
    let f = Fixture::new();
    let mut r = f.pushed();
    git(&f.remote, &["update-ref", "-d", "refs/heads/feat/x"]);
    assert_eq!(
        f.journal()
            .observe(&mut r, &f.observer(), PATIENCE)
            .unwrap(),
        State::Reconcile
    );
    let why = f.rows().last().unwrap().why.clone().unwrap();
    assert!(
        why.contains("refs/heads/feat/x is not on the remote"),
        "{why}"
    );
}

#[test]
fn the_probe_reads_the_url_pinned_at_act_time_not_the_config_now() {
    let f = Fixture::new();
    let mut r = f.pushed();
    // After the act, the remote is pointed elsewhere — where the branch is not.
    git(
        &f.world,
        &["remote", "set-url", "origin", f.other.to_str().unwrap()],
    );
    assert_eq!(
        f.journal()
            .observe(&mut r, &f.observer(), PATIENCE)
            .unwrap(),
        State::Observed
    );
    let probe = f.rows().last().unwrap().probe.clone().unwrap();
    assert!(probe.contains(f.remote.to_str().unwrap()), "{probe}");
    assert!(!probe.contains(f.other.to_str().unwrap()), "{probe}");

    // And the other way round: the pinned remote moved, the configured one would agree — the
    // probe still reads the pinned one, and reconciles.
    let g = Fixture::new();
    let mut r = g.pushed();
    git(&g.remote, &["update-ref", "-d", "refs/heads/feat/x"]);
    git(
        &g.world,
        &[
            "push",
            "-q",
            g.other.to_str().unwrap(),
            "HEAD:refs/heads/feat/x",
        ],
    );
    git(
        &g.world,
        &["remote", "set-url", "origin", g.other.to_str().unwrap()],
    );
    assert_eq!(
        g.journal()
            .observe(&mut r, &g.observer(), PATIENCE)
            .unwrap(),
        State::Reconcile
    );
}

#[test]
fn a_probe_that_errs_or_times_out_is_reconcile() {
    let f = Fixture::new();
    // The pinned remote is gone: git fails.
    let mut r = f.pushed();
    fs::remove_dir_all(&f.remote).unwrap();
    assert_eq!(
        f.journal()
            .observe(&mut r, &f.observer(), PATIENCE)
            .unwrap(),
        State::Reconcile
    );
    let why = f.rows().last().unwrap().why.clone().unwrap();
    assert!(why.starts_with("the probe exited "), "{why}");

    // A probe that cannot finish in time is killed, and says so.
    let started = Instant::now();
    let err = f
        .observer()
        .run(
            &["sleep".to_owned(), "30".to_owned()],
            Duration::from_millis(200),
        )
        .unwrap_err();
    assert!(err.starts_with("timed out after 0.2s"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(5));

    // That error, or any other, is reconcile.
    let g = Fixture::new();
    let mut r = g.pushed();
    let fake = Fake::fails("timed out after 30.0s and was killed");
    assert_eq!(
        g.journal().observe(&mut r, &fake, PATIENCE).unwrap(),
        State::Reconcile
    );
    let why = g.rows().last().unwrap().why.clone().unwrap();
    assert_eq!(
        why,
        "the probe failed: timed out after 30.0s and was killed"
    );
}

fn forge_receipt(f: &Fixture, act: &Act, printed: &str) -> Receipt {
    let planned = f.planned(act);
    assert_eq!(planned.class(), ToolClass::Outward);
    let journal = f.journal();
    let mut r = journal
        .commit(
            &planned,
            Pin::Forge {
                host: "github.com".into(),
            },
            None,
        )
        .unwrap();
    journal.ran(&mut r, Some(0), printed).unwrap();
    r
}

fn label() -> Act {
    Act::AddLabel {
        repo: "me/proj".into(),
        pr: 12,
        label: "ready".into(),
    }
}

#[test]
fn a_label_is_observed_only_when_the_pull_request_carries_it() {
    let f = Fixture::new();
    let mut r = forge_receipt(&f, &label(), "");
    assert_eq!(r.state(), State::PendingObservation);
    let fake = Fake::says(r#"[{"name":"bug"},{"name":"wip"}]"#);
    assert_eq!(
        f.journal().observe(&mut r, &fake, PATIENCE).unwrap(),
        State::Reconcile
    );
    // A fixed template: the pinned host and the act's own repository and number, GET only.
    assert_eq!(
        fake.asked.borrow()[0],
        [
            "gh",
            "api",
            "--hostname",
            "github.com",
            "--method",
            "GET",
            "repos/me/proj/issues/12/labels?per_page=100"
        ]
    );
    let why = f.rows().last().unwrap().why.clone().unwrap();
    assert!(why.contains(r#"does not carry label "ready""#), "{why}");

    let mut r = forge_receipt(&f, &label(), "");
    let fake = Fake::says(r#"[{"name":"bug"},{"name":"Ready"}]"#);
    assert_eq!(
        f.journal().observe(&mut r, &fake, PATIENCE).unwrap(),
        State::Observed
    );

    // A 404, or output that does not read, cannot tell.
    for fake in [Fake::exits(1, ""), Fake::says("<html>rate limited</html>")] {
        let mut r = forge_receipt(&f, &label(), "");
        assert_eq!(
            f.journal().observe(&mut r, &fake, PATIENCE).unwrap(),
            State::Reconcile
        );
    }
}

#[test]
fn a_comment_is_observed_only_by_its_id_with_its_body() {
    let f = Fixture::new();
    let body = "Ready for review.\n\nCI is green.";
    let act = Act::Comment {
        repo: "me/proj".into(),
        pr: 12,
        body: body.into(),
    };
    let printed = "https://github.com/me/proj/pull/12#issuecomment-4242\n";
    let answer = |body: &str, pr: u64| {
        serde_json::json!({
            "id": 4242,
            "body": body,
            "issue_url": format!("https://api.github.com/repos/me/proj/issues/{pr}"),
        })
        .to_string()
    };

    let mut r = forge_receipt(&f, &act, printed);
    assert_eq!(r.done(), Some(&Done::Commented { id: 4242 }));
    let fake = Fake::says(&answer(body, 12));
    assert_eq!(
        f.journal().observe(&mut r, &fake, PATIENCE).unwrap(),
        State::Observed
    );
    assert_eq!(
        fake.asked.borrow()[0].last().unwrap(),
        "repos/me/proj/issues/comments/4242"
    );

    // Edited since, or on another pull request: reconcile.
    for (seen, pr) in [("Ready for review.", 12), (body, 13)] {
        let mut r = forge_receipt(&f, &act, printed);
        let fake = Fake::says(&answer(seen, pr));
        assert_eq!(
            f.journal().observe(&mut r, &fake, PATIENCE).unwrap(),
            State::Reconcile
        );
    }

    // No id printed: nothing to probe, nothing run, reconcile.
    let mut r = forge_receipt(&f, &act, "done\n");
    let fake = Fake::says(&answer(body, 12));
    assert_eq!(
        f.journal().observe(&mut r, &fake, PATIENCE).unwrap(),
        State::Reconcile
    );
    assert!(fake.asked.borrow().is_empty());
}

#[test]
fn an_untyped_command_is_unobserved_and_says_so() {
    let f = Fixture::new();
    let planned = f.planned(&Act::Command(
        "curl -X POST https://example.com/deploy".into(),
    ));
    assert_eq!(planned.class(), ToolClass::Destructive);
    let pin = planned.pin(&f.world).unwrap();
    assert_eq!(pin, Pin::Unpinned);
    let journal = f.journal();
    let mut r = journal.commit(&planned, pin, None).unwrap();
    journal.ran(&mut r, Some(0), "ok").unwrap();
    assert_eq!(r.state(), State::Unobserved);
    let last = f.rows().last().cloned().unwrap();
    assert_eq!(last.state, State::Unobserved);
    assert!(last.why.unwrap().contains("not observed"));
    // The model's text is not kept in the tracked journal: its program and digest are.
    let committed = f.rows()[0].command.clone().unwrap();
    assert!(
        committed.starts_with("curl … (an untyped command, sha256 "),
        "{committed}"
    );
    assert!(!committed.contains("example.com"), "{committed}");
    // There is nothing to probe.
    assert!(journal.observe(&mut r, &Fake::says(""), PATIENCE).is_err());
    assert_eq!(journal.tally().unwrap().unobserved, 1);
}

#[test]
fn a_pin_must_be_of_its_act_and_a_receipt_moves_only_forward() {
    let f = Fixture::new();
    let planned = f.planned(&label());
    let journal = f.journal();
    assert!(journal.commit(&planned, Pin::Unpinned, None).is_err());
    let mut r = forge_receipt(&f, &label(), "");
    assert!(journal.ran(&mut r, Some(0), "").is_err());
    journal
        .observe(&mut r, &Fake::says(r#"[{"name":"ready"}]"#), PATIENCE)
        .unwrap();
    assert!(
        journal
            .observe(&mut r, &Fake::says("[]"), PATIENCE)
            .is_err()
    );
}

#[test]
fn a_remote_that_is_not_a_plain_url_cannot_be_pinned() {
    let f = Fixture::new();
    // Written as a body could write it, straight into the repository's config.
    let config = f.world.join(".git/config");
    let mut text = fs::read_to_string(&config).unwrap();
    text.push_str("[remote \"origin\"]\n\tpushurl = --upload-pack=touch pwned\n");
    fs::write(&config, text).unwrap();
    let planned = f.planned(&branch("feat/x"));
    let err = planned.pin(&f.world).unwrap_err();
    assert!(err.contains("cannot read plainly"), "{err}");
}

#[test]
fn credentials_in_a_pinned_url_never_reach_the_journal() {
    let f = Fixture::new();
    git(
        &f.world,
        &[
            "remote",
            "set-url",
            "origin",
            "https://me:hunter2@example.invalid/me/proj.git",
        ],
    );
    let planned = f.planned(&branch("feat/x"));
    let pin = planned.pin(&f.world).unwrap();
    let journal = f.journal();
    let mut r = journal.commit(&planned, pin, None).unwrap();
    journal.ran(&mut r, Some(128), "").unwrap();
    let fake = Fake::exits(128, "");
    journal.observe(&mut r, &fake, PATIENCE).unwrap();
    // The probe itself was given the URL as pinned; the journal only ever the blanked form.
    assert!(fake.asked.borrow()[0].iter().any(|w| w.contains("hunter2")));
    let text = fs::read_to_string(journal.path()).unwrap();
    assert!(!text.contains("hunter2"), "{text}");
    assert!(
        text.contains("https://***@example.invalid/me/proj.git"),
        "{text}"
    );
}

/// A probe inside the guard reads the outside but writes nothing of the machine.
#[cfg(target_os = "linux")]
#[test]
fn a_probe_inside_the_guard_writes_nothing() {
    let exe = std::env::current_exe().unwrap();
    let guard = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("mnemorph-guard");
    if !guard.is_file() {
        eprintln!("skipped: build the workspace so target/<profile>/mnemorph-guard exists");
        return;
    }
    let f = Fixture::new();
    let observer = Observer::new(Sandbox::Guard(guard), &f.home);
    let marker = f.world.join("written");
    let argv: Vec<String> = ["bash", "-c", &format!("echo x > {}", marker.display())]
        .map(str::to_owned)
        .to_vec();
    let seen = observer.run(&argv, PATIENCE).unwrap();
    assert_ne!(seen.code, Some(0), "{seen:?}");
    assert!(!marker.exists());
    let seen = observer
        .run(&["git".to_owned(), "--version".to_owned()], PATIENCE)
        .unwrap();
    assert_eq!(seen.code, Some(0), "{seen:?}");
}
