//! The pull request act: pinned (its base and head branches there, none open from head into base),
//! rendered with its description in a file, observed only as the one open pull request by `gh`'s
//! login with the approved title and description, undone by closing it while it is still open and
//! its own — and, with a push, both under one approval, each receipted on its own. A stub forge
//! stands in for GitHub, for both `gh` and `git ls-remote`: no network.

use std::fmt::Write as _;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mnemorph_config::{
    Act, Hands, Incident, Paired, Planned, PostJournal, Probe, Refusal, Report, Request, Row,
    Runner, Seen, State,
};
use mnemorph_ledger::Journal;
use mnemorph_races::{Post, Race, ToolClass};
use mnemorph_tools::shell::{BeforeSpawn, Spawned};
use mnemorph_tools::{Answer, Ask, Context, Fingerprint, Gate, Outcome, Sandbox};

const TITLE: &str = "Add the undo";
const BODY: &str = "Adds the undo.\n\n- it closes, never deletes\n- it is observed";
/// The commit `main` names on the stub forge.
const MAIN: &str = "1111111111111111111111111111111111111111";

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

/// The human: approves (or denies) every ask, keeping them.
#[derive(Default)]
struct Human {
    asks: Mutex<Vec<Ask>>,
    deny: AtomicBool,
}

impl Human {
    fn asked(&self) -> usize {
        self.asks.lock().unwrap().len()
    }

    fn ask(&self, n: usize) -> Ask {
        self.asks.lock().unwrap()[n].clone()
    }
}

impl Gate for Human {
    fn ask(&self, ask: &Ask) -> Answer {
        self.asks.lock().unwrap().push(ask.clone());
        if self.deny.load(Ordering::SeqCst) {
            Answer::Deny("no".into())
        } else {
            Answer::Approve
        }
    }
}

/// A pull request on the stub forge.
#[derive(Debug, Clone)]
struct Pr {
    number: u64,
    open: bool,
    merged: bool,
    title: String,
    body: String,
    author: String,
    head: String,
    base: String,
}

/// What the stub does next, switch by switch.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct HubState {
    /// Branches of `me/proj`: (name, commit).
    branches: Vec<(String, String)>,
    pulls: Vec<Pr>,
    /// Comments on pull requests: (id, pr, body).
    comments: Vec<(u64, u64, String)>,
    spawns: usize,
    undos: usize,
    /// Die after the act, before its result is written.
    cut: bool,
    /// Die before the act, its intent journaled.
    cut_before: bool,
    /// Whom the forge credits with the next pull request.
    author: Option<String>,
    /// The next pull request's title and description, edited the moment it opens.
    edit: Option<(String, String)>,
    /// A push also changes a tracked file of the world.
    move_world: bool,
    /// A push exits 0 and leaves nothing.
    push_lost: bool,
    /// `gh pr create` prints another pull request's URL than the one it opened.
    misprint: bool,
    /// What the last spawn was given on its input.
    stdin: Option<Vec<u8>>,
}

/// A stub of GitHub's `me/proj`: `gh api` reads and `git ls-remote` (the probe), `git push`,
/// `gh pr create`, `gh pr close` and `gh pr comment` (the hands, which recheck the approved plan
/// against the world as the bash tool's spawn does).
struct Hub {
    state: Mutex<HubState>,
    login: &'static str,
}

impl Hub {
    fn new() -> Self {
        let hub = Self {
            state: Mutex::default(),
            login: "kijin-bot",
        };
        hub.set_branch("main", MAIN);
        hub
    }

    fn set_branch(&self, name: &str, sha: &str) {
        let mut s = self.state.lock().unwrap();
        s.branches.retain(|(n, _)| n != name);
        s.branches.push((name.to_owned(), sha.to_owned()));
    }

    fn branch(&self, name: &str) -> Option<String> {
        let s = self.state.lock().unwrap();
        s.branches
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, sha)| sha.clone())
    }

    fn open_by(&self, author: &str, head: &str, title: &str, body: &str) -> u64 {
        let mut s = self.state.lock().unwrap();
        let number = 7 + s.pulls.len() as u64;
        s.pulls.push(Pr {
            number,
            open: true,
            merged: false,
            title: title.to_owned(),
            body: body.to_owned(),
            author: author.to_owned(),
            head: head.to_owned(),
            base: "main".to_owned(),
        });
        number
    }

    fn pull(&self, number: u64) -> Pr {
        self.state
            .lock()
            .unwrap()
            .pulls
            .iter()
            .find(|p| p.number == number)
            .unwrap()
            .clone()
    }

    fn with_pull(&self, number: u64, f: impl FnOnce(&mut Pr)) {
        let mut s = self.state.lock().unwrap();
        f(s.pulls.iter_mut().find(|p| p.number == number).unwrap());
    }

    fn pulls(&self) -> usize {
        self.state.lock().unwrap().pulls.len()
    }
}

fn pr_json(p: &Pr) -> serde_json::Value {
    serde_json::json!({
        "number": p.number,
        "state": if p.open { "open" } else { "closed" },
        "merged_at": if p.merged { serde_json::json!("2026-10-27T12:00:00Z") } else { serde_json::Value::Null },
        "title": p.title,
        "body": p.body,
        "user": {"login": p.author},
        "head": {"ref": p.head, "repo": {"full_name": "me/proj"}},
        "base": {"ref": p.base, "repo": {"full_name": "me/proj"}},
    })
}

fn ok(out: &serde_json::Value) -> Seen {
    Seen {
        code: Some(0),
        out: out.to_string().into_bytes(),
        ..Seen::default()
    }
}

impl Probe for Hub {
    fn run(&self, argv: &[String], _: Duration) -> Result<Seen, String> {
        let s = self.state.lock().unwrap();
        if argv[0] == "git" {
            assert!(argv.contains(&"ls-remote".to_owned()), "{argv:?}");
            let name = argv.last().unwrap().strip_prefix("refs/heads/").unwrap();
            let out = s.branches.iter().filter(|(n, _)| n == name).fold(
                String::new(),
                |mut out, (n, sha)| {
                    let _ = writeln!(out, "{sha}\trefs/heads/{n}");
                    out
                },
            );
            return Ok(Seen {
                code: Some(0),
                out: out.into_bytes(),
                ..Seen::default()
            });
        }
        assert_eq!(&argv[..2], ["gh", "api"]);
        assert!(argv.contains(&"GET".to_owned()), "{argv:?}");
        let path = argv.last().unwrap();
        if path == "user" {
            return Ok(ok(&serde_json::json!({"login": self.login})));
        }
        if let Some(branch) = path.strip_prefix("repos/me/proj/git/ref/heads/") {
            return Ok(match s.branches.iter().find(|(n, _)| n == branch) {
                Some((n, sha)) => ok(&serde_json::json!({
                    "ref": format!("refs/heads/{n}"), "object": {"sha": sha}
                })),
                None => Seen {
                    code: Some(1),
                    out: br#"{"message":"Not Found","status":"404"}"#.to_vec(),
                    err: b"gh: Not Found (HTTP 404)".to_vec(),
                    cut: false,
                },
            });
        }
        if let Some(query) = path.strip_prefix("repos/me/proj/pulls?") {
            let field = |k: &str| {
                query
                    .split('&')
                    .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
                    .unwrap()
                    .to_owned()
            };
            assert_eq!(field("state"), "open");
            let head = field("head");
            let head = head.strip_prefix("me:").unwrap();
            let base = field("base");
            let list: Vec<serde_json::Value> = s
                .pulls
                .iter()
                .filter(|p| p.open && p.head == head && p.base == base)
                .map(pr_json)
                .collect();
            return Ok(ok(&serde_json::json!(list)));
        }
        if let Some(n) = path.strip_prefix("repos/me/proj/pulls/") {
            let p = s.pulls.iter().find(|p| p.number.to_string() == n).unwrap();
            return Ok(ok(&pr_json(p)));
        }
        if let Some(id) = path.strip_prefix("repos/me/proj/issues/comments/") {
            let (id, pr, body) = s.comments.iter().find(|c| c.0.to_string() == id).unwrap();
            return Ok(ok(&serde_json::json!({
                "id": id, "body": body, "user": {"login": self.login},
                "issue_url": format!("https://api.github.com/repos/me/proj/issues/{pr}"),
            })));
        }
        panic!("an unexpected probe: {argv:?}");
    }
}

impl Hands for Hub {
    fn spawn(
        &self,
        planned: &Planned,
        cx: &Context,
        plan: Fingerprint,
        before: BeforeSpawn<'_>,
        stdin: Option<&[u8]>,
    ) -> Spawned {
        self.state.lock().unwrap().stdin = stdin.map(<[u8]>::to_vec);
        // As the bash tool's spawn does: the plan rechecked, then the step before the spawn.
        let now = Fingerprint::of(planned.command(), cx.world()).unwrap();
        if now != plan {
            return Spawned::Changed {
                approved: plan,
                now,
            };
        }
        if let Err(e) = before() {
            return Spawned::Failed(Outcome::error(e));
        }
        assert!(
            !self.state.lock().unwrap().cut_before,
            "the harness was killed before the act"
        );
        let words: Vec<&str> = planned.command().split(' ').collect();
        let (exit, printed) = match (planned.is_undo(), planned.act()) {
            (false, Act::PushNewBranch { name, .. }) => {
                let sha = git(cx.world(), &["rev-parse", "HEAD"]);
                let (lost, moved) = {
                    let s = self.state.lock().unwrap();
                    (s.push_lost, s.move_world)
                };
                if !lost {
                    self.set_branch(name, &sha);
                }
                // As a real push does: the remote-tracking ref moves in the world.
                git(
                    cx.world(),
                    &["update-ref", &format!("refs/remotes/origin/{name}"), &sha],
                );
                if moved {
                    fs::write(cx.world().join("a.txt"), "two\n").unwrap();
                }
                (0, String::new())
            }
            (true, Act::PushNewBranch { name, .. }) => {
                self.state
                    .lock()
                    .unwrap()
                    .branches
                    .retain(|(n, _)| n != name);
                (0, String::new())
            }
            (false, Act::OpenPullRequest { head, .. }) => {
                // `gh` reads the description from the file the command names, in the world.
                // `gh` reads the description on its input: `--body-file -`.
                assert_eq!(words.last(), Some(&"-"));
                let body =
                    String::from_utf8(stdin.expect("the description on stdin").to_vec()).unwrap();
                let mut s = self.state.lock().unwrap();
                if s.pulls.iter().any(|p| p.open && &p.head == head) {
                    (1, "a pull request for branch already exists".to_owned())
                } else {
                    let number = 7 + s.pulls.len() as u64;
                    let author = s.author.clone().unwrap_or_else(|| self.login.to_owned());
                    let (title, body) = s.edit.clone().unwrap_or((TITLE.to_owned(), body));
                    s.pulls.push(Pr {
                        number,
                        open: true,
                        merged: false,
                        title,
                        body,
                        author,
                        head: head.clone(),
                        base: "main".to_owned(),
                    });
                    let shown = if s.misprint { number + 1 } else { number };
                    (0, format!("https://github.com/me/proj/pull/{shown}\n"))
                }
            }
            (true, Act::OpenPullRequest { .. }) => {
                assert_eq!(&words[..3], ["gh", "pr", "close"]);
                let number: u64 = words[3].parse().unwrap();
                self.state.lock().unwrap().undos += 1;
                self.with_pull(number, |p| p.open = false);
                (0, String::new())
            }
            (false, Act::Comment { pr, body, .. }) => {
                let mut s = self.state.lock().unwrap();
                let id = 9000 + s.comments.len() as u64;
                s.comments.push((id, *pr, body.clone()));
                (
                    0,
                    format!("https://github.com/me/proj/pull/{pr}#issuecomment-{id}\n"),
                )
            }
            other => panic!("not an act of the stub: {other:?}"),
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
        out.meta.exit = Some(exit);
        Spawned::Ran(out)
    }
}

/// A world with a commit, its `origin` GitHub's `me/proj` — never reached: the stub answers.
struct Fixture {
    _tmp: tempfile::TempDir,
    world: PathBuf,
    home: PathBuf,
    witness: PathBuf,
    head: String,
    human: Arc<Human>,
    hub: Hub,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let (world, home) = (root.join("world"), root.join("home"));
        for d in [&world, &home] {
            fs::create_dir_all(d).unwrap();
        }
        git(&world, &["init", "-q"]);
        fs::write(world.join("a.txt"), "one\n").unwrap();
        // `.mnemorph/` is not ignored: the post journal the push writes is the world's own file.
        git(&world, &["add", "a.txt"]);
        git(&world, &["commit", "-qm", "one"]);
        let head = git(&world, &["rev-parse", "HEAD"]);
        git(
            &world,
            &["remote", "add", "origin", "https://github.com/me/proj.git"],
        );
        Self {
            _tmp: tmp,
            world,
            home,
            witness: root.join("state/kijin/w/kijin-forge.jsonl"),
            head,
            human: Arc::new(Human::default()),
            hub: Hub::new(),
        }
    }

    fn cx(&self) -> Context {
        Context::new(Race::Kijin, &self.world, &["."])
            .with_body("kijin-forge")
            .with_home(&self.home)
            .with_gate(self.human.clone())
            .with_sandbox(Sandbox::None("tests run unguarded".into()))
            .with_unguarded(true)
    }

    fn journal(&self) -> PostJournal {
        PostJournal::of(&self.world, Post::Forge).witnessed_at(&self.witness)
    }

    fn act(&self, request: &Request) -> Result<Report, Refusal> {
        let cx = self.cx();
        Runner::new(&cx, self.journal(), &self.hub, &self.hub).act(request)
    }

    fn pair(&self, pull: &Request) -> Result<Paired, Refusal> {
        let cx = self.cx();
        let Act::OpenPullRequest { head, .. } = &pull.act else {
            panic!("not a pull request");
        };
        let push = Request {
            act: Act::PushNewBranch {
                remote: "origin".into(),
                name: head.clone(),
            },
            key: None,
            tells: None,
        };
        Runner::new(&cx, self.journal(), &self.hub, &self.hub).push_and_open(&push, pull)
    }

    fn undo(&self, receipt: &str) -> Result<Report, Refusal> {
        let cx = self.cx();
        Runner::new(&cx, self.journal(), &self.hub, &self.hub).undo(receipt)
    }

    fn undo_command(&self, receipt: &str) -> Result<String, Refusal> {
        let cx = self.cx();
        Runner::new(&cx, self.journal(), &self.hub, &self.hub).undo_command(receipt)
    }

    /// Push `feat/x` through the runner: the stub's push, observed by its `ls-remote`.
    fn push(&self) -> String {
        let report = self
            .act(&Request {
                act: Act::PushNewBranch {
                    remote: "origin".into(),
                    name: "feat/x".into(),
                },
                key: None,
                tells: None,
            })
            .unwrap();
        assert_eq!(report.state, State::Observed, "{report:?}");
        report.receipt
    }

    fn rows(&self) -> Vec<Row> {
        Journal::<Row>::new(&self.witness).read().unwrap().rows
    }

    fn states_of(&self, act: &str) -> Vec<State> {
        self.rows()
            .iter()
            .filter(|r| r.act == act)
            .map(|r| r.state)
            .collect()
    }

    fn latest(&self, act: &str) -> State {
        *self.states_of(act).last().unwrap()
    }

    fn incident(&self, act: &str) -> Incident {
        self.journal().incident(act).unwrap().unwrap()
    }
}

fn pull_request() -> Request {
    Request {
        act: Act::OpenPullRequest {
            repo: "me/proj".into(),
            base: "main".into(),
            head: "feat/x".into(),
            title: TITLE.into(),
            body: BODY.into(),
        },
        key: None,
        tells: None,
    }
}

fn with(f: impl FnOnce(&mut Act)) -> Request {
    let mut r = pull_request();
    f(&mut r.act);
    r
}

fn cut_short(f: impl FnOnce()) {
    assert!(
        catch_unwind(AssertUnwindSafe(f)).is_err(),
        "the run was not cut"
    );
}

fn refused(result: &Result<Report, Refusal>, says: &str) {
    assert!(
        matches!(&result, Err(Refusal::Refused(w)) if w.contains(says)),
        "{says}: {result:?}"
    );
}

#[test]
fn a_pull_request_opens_observed_and_its_undo_closes_it_never_deletes_it() {
    let f = Fixture::new();
    let push = f.push();
    let report = f.act(&pull_request()).unwrap();
    assert_eq!(report.state, State::Observed, "{report:?}");
    assert_eq!(
        f.states_of(&report.receipt),
        [State::Committed, State::PendingObservation, State::Observed]
    );
    // The card: the rendered command, the title, the description, the head this post pushed.
    let card = f.human.ask(1);
    assert_eq!(
        card.act,
        "gh pr create --repo me/proj --base main --head feat/x --title 'Add the undo' --body-file -"
    );
    assert!(card.fingerprint.is_some());
    for line in [
        format!("title: {TITLE}"),
        format!(
            "head feat/x at {}, as act {push} of this post pushed it",
            f.head
        ),
    ] {
        assert!(card.why.contains(&line), "{line}: {:?}", card.why);
    }
    assert!(card.why.iter().any(|w| w.ends_with(BODY)), "{:?}", card.why);
    assert!(
        card.why
            .iter()
            .any(|w| w.starts_with("undo: close this pull request"))
    );
    // `gh` read the description on its input: the approved bytes, and no file in the world.
    let opened = f.hub.pull(7);
    assert_eq!((opened.title.as_str(), opened.body.as_str()), (TITLE, BODY));
    assert_eq!(
        f.hub.state.lock().unwrap().stdin.as_deref(),
        Some(BODY.as_bytes())
    );
    let scratch = f.world.join(".mnemorph/tmp");
    assert!(
        !scratch.exists() || fs::read_dir(&scratch).unwrap().next().is_none(),
        "a file was written in the world's scratch folder"
    );
    let observed = f.rows().into_iter().last().unwrap();
    assert!(observed.expect.unwrap().contains("pull request #7"));
    // The undo closes #7, by its number, and sees it closed.
    assert_eq!(
        f.undo_command(&report.receipt).unwrap(),
        "gh pr close 7 --repo me/proj"
    );
    let undo = f.undo(&report.receipt).unwrap();
    assert_eq!(undo.state, State::Observed, "{undo:?}");
    let closed = f.hub.pull(7);
    assert!(!closed.open, "the pull request is still open");
    assert_eq!(f.hub.pulls(), 1, "the pull request was deleted");
    assert_eq!(f.latest(&report.receipt), State::Corrected);
    assert_eq!(
        f.incident(&report.receipt).missing(),
        ["downstream told", "lesson"]
    );
}

#[test]
fn the_pin_refuses_a_pull_request_already_open_from_the_head_into_the_base() {
    let f = Fixture::new();
    f.push();
    f.hub
        .open_by("someone-else", "feat/x", "Theirs", "Their text.");
    refused(&f.act(&pull_request()), "already open: #7");
    assert_eq!(f.human.asked(), 1, "the refused act reached the gate");
    assert_eq!(f.hub.state.lock().unwrap().spawns, 1, "only the push ran");
}

#[test]
fn a_pull_request_by_another_login_or_edited_as_it_opens_is_not_the_acts() {
    for (author, edit, misprint, says) in [
        (
            Some("someone-else"),
            None,
            false,
            "by someone-else, not by kijin-bot",
        ),
        (
            None,
            Some(("Retitled", BODY)),
            false,
            "title is not the act's",
        ),
        (
            None,
            Some((TITLE, "Rewritten.")),
            false,
            "description sha256",
        ),
        (None, None, true, "not #8, the one the act printed"),
    ] {
        let f = Fixture::new();
        f.push();
        {
            let mut s = f.hub.state.lock().unwrap();
            s.author = author.map(str::to_owned);
            s.edit = edit.map(|(t, b)| (t.to_owned(), b.to_owned()));
            s.misprint = misprint;
        }
        let report = f.act(&pull_request()).unwrap();
        assert_eq!(report.state, State::Reconcile, "{says}: {report:?}");
        let why = f.rows().into_iter().last().unwrap().why.unwrap();
        assert!(why.contains(says), "{says}: {why}");
    }
}

#[test]
fn the_undo_closes_only_the_acts_own_pull_request_while_it_is_open() {
    // Edited after it was observed: not undone, reconcile, still open.
    let f = Fixture::new();
    f.push();
    let act = f.act(&pull_request()).unwrap().receipt;
    f.hub
        .with_pull(7, |p| p.body = "Rewritten by a reviewer.".into());
    refused(&f.undo(&act), "it was edited");
    assert_eq!(f.latest(&act), State::Reconcile);
    assert!(f.hub.pull(7).open);
    assert_eq!(f.hub.state.lock().unwrap().undos, 0);

    // Merged: closing undoes nothing; never recorded as undone.
    let f = Fixture::new();
    f.push();
    let act = f.act(&pull_request()).unwrap().receipt;
    f.hub.with_pull(7, |p| {
        p.merged = true;
        p.open = false;
    });
    refused(&f.undo(&act), "was merged");
    assert_eq!(f.latest(&act), State::Reconcile);
    assert_eq!(f.hub.state.lock().unwrap().undos, 0);
    assert!(f.journal().incident(&act).unwrap().is_none());

    // Closed by hand: recorded undone once approved, nothing run.
    let f = Fixture::new();
    f.push();
    let act = f.act(&pull_request()).unwrap().receipt;
    f.hub.with_pull(7, |p| p.open = false);
    let undo = f.undo(&act).unwrap();
    assert_eq!(undo.state, State::Observed, "{undo:?}");
    assert_eq!(f.hub.state.lock().unwrap().undos, 0);
    assert_eq!(f.latest(&act), State::Corrected);
}

#[test]
fn a_cut_pull_request_is_probed_first_and_never_opened_twice() {
    // Landed: settled from the probe, its number kept for the undo; never asked or run again.
    let f = Fixture::new();
    f.push();
    f.hub.state.lock().unwrap().cut = true;
    cut_short(|| {
        let _ = f.act(&pull_request());
    });
    f.hub.state.lock().unwrap().cut = false;
    let again = f.act(&pull_request()).unwrap();
    assert!(again.replayed, "{again:?}");
    assert_eq!(f.hub.pulls(), 1, "opened twice");
    assert_eq!(f.human.asked(), 2, "push and one pull request");
    assert_eq!(
        f.undo_command(&again.receipt).unwrap(),
        "gh pr close 7 --repo me/proj"
    );

    // Never ran: absent, asked again as a new act.
    let f = Fixture::new();
    f.push();
    f.hub.state.lock().unwrap().cut_before = true;
    cut_short(|| {
        let _ = f.act(&pull_request());
    });
    f.hub.state.lock().unwrap().cut_before = false;
    let report = f.act(&pull_request()).unwrap();
    assert!(!report.replayed);
    assert_eq!(report.state, State::Observed);
    assert_eq!(f.human.asked(), 3);
    assert!(f.rows().iter().any(|r| r.state == State::Absent));

    // Never ran, and someone else opened the same head into the same base: not the act's — the
    // human settles it, and nothing is asked or opened.
    let f = Fixture::new();
    f.push();
    f.hub.state.lock().unwrap().cut_before = true;
    cut_short(|| {
        let _ = f.act(&pull_request());
    });
    f.hub.state.lock().unwrap().cut_before = false;
    f.hub.open_by("someone-else", "feat/x", TITLE, BODY);
    refused(&f.act(&pull_request()), "is reconcile");
    assert_eq!(f.human.asked(), 2);
    assert_eq!(f.hub.pulls(), 1);

    // Landed, then retitled before the run resumed: as strict as observe — reconcile.
    let f = Fixture::new();
    f.push();
    f.hub.state.lock().unwrap().cut = true;
    cut_short(|| {
        let _ = f.act(&pull_request());
    });
    f.hub.state.lock().unwrap().cut = false;
    f.hub.with_pull(7, |p| p.title = "Retitled".into());
    refused(&f.act(&pull_request()), "is reconcile");
    assert!(
        f.rows().iter().any(|r| r.state == State::Reconcile
            && r.why.as_deref().is_some_and(|w| w.contains("title")))
    );
}

#[test]
fn a_pull_request_whose_branches_are_not_there_is_refused_before_the_gate() {
    let f = Fixture::new();
    // The head was never pushed.
    refused(&f.act(&pull_request()), "push it first");
    // The base does not exist.
    f.push();
    refused(
        &f.act(&with(|a| {
            if let Act::OpenPullRequest { base, .. } = a {
                *base = "develop".into();
            }
        })),
        "the base `develop` is not a branch of me/proj",
    );
    // A deceptive title never reaches the forge.
    refused(
        &f.act(&with(|a| {
            if let Act::OpenPullRequest { title, .. } = a {
                *title = "Fix \u{202E}gnp.exe".into();
            }
        })),
        "`title`",
    );
    // The head this post pushed has moved since: no pull request opens from it.
    f.hub
        .set_branch("feat/x", "2222222222222222222222222222222222222222");
    refused(&f.act(&pull_request()), "the branch moved since");
    assert_eq!(f.human.asked(), 1, "only the push was asked");
    // A head someone else pushed is admitted, and the card says so.
    f.hub
        .set_branch("feat/theirs", "3333333333333333333333333333333333333333");
    let theirs = f
        .act(&with(|a| {
            if let Act::OpenPullRequest { head, .. } = a {
                *head = "feat/theirs".into();
            }
        }))
        .unwrap();
    assert_eq!(theirs.state, State::Observed);
    assert!(
        f.human
            .ask(1)
            .why
            .iter()
            .any(|w| w.ends_with("a branch of me/proj this post did not push"))
    );
}

#[test]
fn a_description_file_in_the_world_is_never_what_gh_reads() {
    let f = Fixture::new();
    f.push();
    // A file another command left where the old rendering named one: never read, never touched.
    let planted = f.world.join(".mnemorph/tmp/pr-body.md");
    fs::create_dir_all(planted.parent().unwrap()).unwrap();
    fs::write(&planted, "Planted text.").unwrap();
    let report = f.act(&pull_request()).unwrap();
    assert_eq!(report.state, State::Observed);
    assert_eq!(f.hub.pull(7).body, BODY);
    assert_eq!(fs::read_to_string(&planted).unwrap(), "Planted text.");
}

#[test]
fn a_branch_is_held_to_a_pull_request_the_human_settled_as_any_standing_one() {
    // The pull request edited, its undo refused (reconcile), then settled as fine: it still stands
    // open on the branch, so the branch is not deleted under it.
    let f = Fixture::new();
    let push = f.push();
    let act = f.act(&pull_request()).unwrap().receipt;
    f.hub
        .with_pull(7, |p| p.body = "Edited by a reviewer.".into());
    refused(&f.undo(&act), "it was edited");
    f.journal().settle(&act, Some("the edit is fine")).unwrap();
    assert_eq!(f.latest(&act), State::Settled);
    refused(&f.undo(&push), "undo it first");
    assert_eq!(f.hub.branch("feat/x").as_deref(), Some(f.head.as_str()));
}

#[test]
fn a_push_and_its_pull_request_ask_once_and_are_receipted_each_on_its_own() {
    let f = Fixture::new();
    let Paired { push, pull } = f.pair(&pull_request()).unwrap();
    let pull = pull.unwrap();
    assert_eq!((push.state, pull.state), (State::Observed, State::Observed));
    assert_ne!(push.receipt, pull.receipt);
    assert_eq!(f.human.asked(), 1, "the pair asked more than once");
    let card = f.human.ask(0);
    let lines: Vec<&str> = card.act.lines().collect();
    assert_eq!(
        lines,
        [
            "git push origin HEAD:refs/heads/feat/x --force-with-lease=refs/heads/feat/x:",
            "gh pr create --repo me/proj --base main --head feat/x --title 'Add the undo' --body-file -"
        ]
    );
    for line in [
        format!("pushes commit {} to https://github.com/me/proj.git", f.head),
        format!("title: {TITLE}"),
        format!(
            "head feat/x: the branch the push above makes, at {}",
            f.head
        ),
    ] {
        assert!(card.why.contains(&line), "{line}: {:?}", card.why);
    }
    assert!(card.why.iter().any(|w| w.ends_with(BODY)));
    assert!(card.fingerprint.is_some());
    // Each act has its own rows, and the push moved a ref in the world without a second ask.
    for act in [&push.receipt, &pull.receipt] {
        assert_eq!(
            f.states_of(act),
            [State::Committed, State::PendingObservation, State::Observed]
        );
    }
    assert_eq!(
        git(&f.world, &["rev-parse", "refs/remotes/origin/feat/x"]),
        f.head
    );
    assert_eq!(f.hub.branch("feat/x").as_deref(), Some(f.head.as_str()));
    // The branch is not deleted under its open pull request: the pull request is undone first.
    refused(&f.undo(&push.receipt), "undo it first");
    f.undo(&pull.receipt).unwrap();
    let undone = f.undo(&push.receipt).unwrap();
    assert_eq!(undone.state, State::Observed);
    assert_eq!(f.hub.branch("feat/x"), None);
}

#[test]
fn a_pair_runs_its_pull_request_only_on_an_observed_push_in_its_repository() {
    // Denied: nothing runs.
    let f = Fixture::new();
    f.human.deny.store(true, Ordering::SeqCst);
    let denied = f.pair(&pull_request()).unwrap_err();
    assert!(matches!(denied, Refusal::Denied(_)), "{denied:?}");
    assert_eq!(f.hub.state.lock().unwrap().spawns, 0);

    // The push leaves nothing: the pull request is not opened.
    let f = Fixture::new();
    f.hub.state.lock().unwrap().push_lost = true;
    let Paired { push, pull } = f.pair(&pull_request()).unwrap();
    assert_eq!(push.state, State::Reconcile);
    assert!(
        matches!(&pull, Err(Refusal::Refused(w)) if w.contains("not observed")),
        "{pull:?}"
    );
    assert_eq!(f.hub.pulls(), 0);

    // The remote pushes to another repository than the pull request's: refused before the gate.
    let f = Fixture::new();
    git(
        &f.world,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/me/other.git",
        ],
    );
    let err = f.pair(&pull_request()).unwrap_err();
    assert!(
        matches!(&err, Refusal::Refused(w) if w.contains("no push URL of the remote is me/proj")),
        "{err:?}"
    );
    assert_eq!(f.human.asked(), 0);

    // The world moved between the approval and the pull request, by more than the push's refs:
    // the pull request is asked for alone, saying what changed.
    let f = Fixture::new();
    f.hub.state.lock().unwrap().move_world = true;
    let Paired { push, pull } = f.pair(&pull_request()).unwrap();
    assert_eq!(push.state, State::Observed);
    assert_eq!(pull.unwrap().state, State::Observed);
    assert_eq!(f.human.asked(), 2);
    let second = f.human.ask(1);
    assert!(
        second.why[0].contains("the working tree changed"),
        "{:?}",
        second.why
    );
    assert!(!second.act.contains("git push"));
}

#[test]
fn an_incident_on_a_pull_request_refuses_its_target_but_the_telling_comment() {
    let f = Fixture::new();
    f.push();
    let act = f.act(&pull_request()).unwrap().receipt;
    f.undo(&act).unwrap();
    let asked = f.human.asked();
    // The same head into the same base: refused while the incident is open.
    refused(&f.act(&pull_request()), "is open on this target");
    // A comment on the pull request the act opened, not telling of it: refused.
    let comment = |tells: Option<&str>| Request {
        act: Act::Comment {
            repo: "me/proj".into(),
            pr: 7,
            body: "Closed: opened too early.".into(),
        },
        key: None,
        tells: tells.map(str::to_owned),
    };
    refused(&f.act(&comment(None)), "is open on this target");
    assert_eq!(f.human.asked(), asked);
    // Telling of it there: admitted, and counted as receipt (3).
    let told = f.act(&comment(Some(&act))).unwrap();
    assert_eq!(told.state, State::Observed, "{told:?}");
    assert_eq!(f.incident(&act).missing(), ["lesson"]);
}
