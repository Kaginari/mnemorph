//! Oni envelopes at the gate: an act an envelope admits runs without the human, read again at
//! the moment of use; anything that stops it — a lapse, a suspension, an exclusion, a file that is
//! not the human's own, a body that is no Oni — sends the act to the human, saying why. Local bare
//! repositories stand in for remotes, and a stub forge for GitHub: no network.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use mnemorph_config::{
    Act, EnvelopeGate, Hands, Observer, Paths, Planned, PostJournal, Probe, Refusal, Report,
    Request, Row, Runner, Sandboxed, Seen, State, excluded, plan,
};
use mnemorph_ledger::{Day, Journal, Timestamp, UseRow};
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

fn today() -> Day {
    Timestamp::now().day()
}

/// The human: approves every ask, keeping each; and sees every act an envelope admitted.
#[derive(Default)]
struct Human {
    asks: Mutex<Vec<Ask>>,
    shown: Mutex<Vec<String>>,
}

impl Human {
    fn asked(&self) -> usize {
        self.asks.lock().unwrap().len()
    }

    /// Every reason the cards gave for no envelope admitting the act.
    fn why_not(&self) -> Vec<String> {
        self.asks
            .lock()
            .unwrap()
            .iter()
            .flat_map(|a| a.why.iter())
            .filter(|w| w.starts_with("no envelope admits it"))
            .cloned()
            .collect()
    }
}

impl Gate for Human {
    fn ask(&self, ask: &Ask) -> Answer {
        self.asks.lock().unwrap().push(ask.clone());
        Answer::Approve
    }

    fn admitted(&self, ask: &Ask, by: &str) {
        self.shown
            .lock()
            .unwrap()
            .push(format!("{} {} — {by}", ask.body, ask.act));
    }
}

/// A world with a commit, its `origin` a local bare repository; the human's own global file, and
/// the uses ledger, under the test's directory.
struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    world: PathBuf,
    home: PathBuf,
    remote: PathBuf,
    global: PathBuf,
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
        git(
            &world,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        let global = root.join("config/mnemorph/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::set_permissions(global.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            _tmp: tmp,
            world,
            home,
            remote,
            global,
            human: Arc::new(Human::default()),
            root,
        }
    }

    /// Sign envelopes in the human's global file, private to them, each for this world unless it
    /// names its own world or repository (or `# unbound`).
    fn sign(&self, toml: &str) {
        fs::write(&self.global, self.bound(toml)).unwrap();
        fs::set_permissions(&self.global, fs::Permissions::from_mode(0o600)).unwrap();
    }

    /// `toml` with each envelope signed for this world, unless one already names its scope.
    fn bound(&self, toml: &str) -> String {
        if toml.contains("world =") || toml.contains("repo =") || toml.contains("# unbound") {
            return toml.to_owned();
        }
        let line = format!("world = {:?}", self.world.display().to_string());
        toml.lines()
            .flat_map(|l| {
                if l.starts_with("name = ") {
                    vec![l.to_owned(), line.clone()]
                } else {
                    vec![l.to_owned()]
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn paths(&self) -> Paths {
        Paths::with_global(self.global.clone(), &self.world)
    }

    fn uses_path(&self) -> PathBuf {
        self.root.join("state/oni/uses.jsonl")
    }

    fn gate(&self) -> EnvelopeGate {
        EnvelopeGate::new(self.paths(), self.uses_path())
    }

    fn uses(&self) -> Vec<UseRow> {
        Journal::<UseRow>::new(self.uses_path())
            .read()
            .unwrap()
            .rows
    }

    /// Record `n` earlier uses of the envelope `name` signed on `signed`.
    fn used(&self, name: &str, signed: Day, n: u64) {
        let j = Journal::<UseRow>::new(self.uses_path());
        for i in 1..=n {
            j.append(&UseRow {
                envelope: name.into(),
                signed: signed.to_string(),
                act: format!("earlier{i}"),
                post: "kijin-forge".into(),
                world: self.world.display().to_string(),
                command: "gh pr comment".into(),
                at: Timestamp::now(),
                n: i,
            })
            .unwrap();
        }
    }

    fn witness(&self, n: usize) -> PathBuf {
        self.root
            .join(format!("state/kijin/w{n}/kijin-forge.jsonl"))
    }

    fn cx(&self, race: Race, body: &str) -> Context {
        Context::new(race, &self.world, &["."])
            .with_body(body)
            .with_home(&self.home)
            .with_gate(self.human.clone())
            .with_sandbox(Sandbox::None("tests run unguarded".into()))
            .with_unguarded(true)
    }

    /// Run `request` as `body` of `race`, witnessed in witness `n`.
    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        race: Race,
        body: &str,
        n: usize,
        request: &Request,
        probe: &dyn Probe,
        hands: &dyn Hands,
    ) -> Result<Report, Refusal> {
        let cx = self.cx(race, body);
        let journal = PostJournal::of(&self.world, Post::Forge).witnessed_at(&self.witness(n));
        Runner::new(&cx, journal, probe, hands)
            .with_envelopes(Some(self.gate()))
            .act(request)
    }

    /// An Oni of the forge post comments, through `forge`.
    fn oni_comment(&self, body: &str, forge: &Forge) -> Result<Report, Refusal> {
        self.run(Race::Oni, "kijin-forge", 0, &comment(body), forge, forge)
    }

    fn rows(&self, n: usize) -> Vec<Row> {
        Journal::<Row>::new(self.witness(n)).read().unwrap().rows
    }
}

/// An envelope over comments on `me/proj` and `feat/*` branches, signed `from` days from today,
/// expiring `to` days from today, with `extra` lines.
fn envelope(from: i64, to: i64, extra: &str) -> String {
    let t = today();
    format!(
        "[[oni.envelopes]]\nname = \"forge\"\nallow = [\"gh pr comment * --repo me/proj --body *\", \"git push origin feat/*\"]\nsigned = {}\nexpires = {}\n{extra}\n",
        t.plus(from),
        t.plus(to)
    )
}

fn comment(body: &str) -> Request {
    comment_on(12, body)
}

fn comment_on(pr: u64, body: &str) -> Request {
    Request {
        act: Act::Comment {
            repo: "me/proj".into(),
            pr,
            body: body.into(),
        },
        key: None,
        tells: None,
    }
}

fn push(name: &str) -> Request {
    Request {
        act: Act::PushNewBranch {
            remote: "origin".into(),
            name: name.into(),
        },
        key: None,
        tells: None,
    }
}

/// A stub of GitHub's comments on `me/proj#12`: `gh api` reads (the probe) and `gh pr comment`
/// (the hands). It may wait on a barrier before its first acts, and run something first.
struct Forge {
    /// (id, pull request, body)
    comments: Mutex<Vec<(u64, u64, String)>>,
    barrier: Option<Arc<Barrier>>,
    waited: AtomicUsize,
    first: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Forge {
    fn new() -> Self {
        Self {
            comments: Mutex::default(),
            barrier: None,
            waited: AtomicUsize::new(0),
            first: Mutex::new(None),
        }
    }

    fn posted(&self) -> usize {
        self.comments.lock().unwrap().len()
    }
}

impl Probe for Forge {
    fn run(&self, argv: &[String], _: Duration) -> Result<Seen, String> {
        assert_eq!(&argv[..2], ["gh", "api"], "{argv:?}");
        let path = argv.last().unwrap();
        let issue = |pr: u64| format!("https://api.github.com/repos/me/proj/issues/{pr}");
        let shown = |(id, pr, body): &(u64, u64, String)| serde_json::json!({"id": id, "body": body, "issue_url": issue(*pr), "user": {"login": "kijin-bot"}});
        let comments = self.comments.lock().unwrap();
        let listed = path
            .strip_prefix("repos/me/proj/issues/")
            .and_then(|rest| rest.split_once("/comments"))
            .and_then(|(pr, _)| pr.parse::<u64>().ok());
        let out = if path == "user" {
            serde_json::json!({"login": "kijin-bot"})
        } else if let Some(id) = path.strip_prefix("repos/me/proj/issues/comments/") {
            match comments.iter().find(|c| c.0.to_string() == id) {
                Some(c) => shown(c),
                None => {
                    return Ok(Seen {
                        code: Some(1),
                        err: b"HTTP 404: Not Found".to_vec(),
                        ..Seen::default()
                    });
                }
            }
        } else if let Some(pr) = listed {
            serde_json::json!(
                comments
                    .iter()
                    .filter(|c| c.1 == pr)
                    .map(shown)
                    .collect::<Vec<_>>()
            )
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
        if let Some(b) = &self.barrier
            && self.waited.fetch_add(1, Ordering::SeqCst) < b_parties()
        {
            b.wait();
        }
        if let Some(f) = self.first.lock().unwrap().take() {
            f();
        }
        if let Err(e) = before() {
            return Spawned::Failed(Outcome::error(e));
        }
        let Act::Comment { body, pr, .. } = planned.act() else {
            panic!("not a comment: {:?}", planned.act());
        };
        let id = {
            let mut c = self.comments.lock().unwrap();
            let id = 4000 + c.len() as u64;
            c.push((id, *pr, body.clone()));
            id
        };
        let mut out = Outcome::ok(
            format!("https://github.com/me/proj/pull/{pr}#issuecomment-{id}\n"),
            ToolClass::Outward,
        );
        out.meta.exit = Some(0);
        Spawned::Ran(out)
    }
}

/// How many acts wait on the barrier: the two racing for the last use.
fn b_parties() -> usize {
    2
}

#[test]
fn an_onis_act_inside_a_live_envelope_runs_without_the_human_and_is_shown() {
    let w = Fixture::new();
    w.sign(&envelope(-1, 20, "uses = 3"));
    let forge = Forge::new();
    let report = w
        .oni_comment("LGTM, the label waits for review.", &forge)
        .unwrap();
    assert_eq!(report.state, State::Observed, "{report:?}");
    assert_eq!(w.human.asked(), 0, "the human was asked");
    assert_eq!(forge.posted(), 1);
    // The receipt says which envelope admitted it, and which use.
    let committed = w
        .rows(0)
        .into_iter()
        .find(|r| r.state == State::Committed)
        .unwrap();
    assert_eq!(
        committed.why.as_deref(),
        Some("admitted by envelope forge (use 1/3)")
    );
    assert!(
        report.notes[0].starts_with("admitted by envelope forge (use 1/3)"),
        "{:?}",
        report.notes
    );
    // The human sees it, and the ledger counts it.
    let shown = w.human.shown.lock().unwrap().clone();
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert!(
        shown[0].contains("admitted by envelope forge (use 1/3)"),
        "{shown:?}"
    );
    let uses = w.uses();
    assert_eq!(uses.len(), 1);
    assert_eq!(
        (uses[0].act.as_str(), uses[0].n),
        (report.receipt.as_str(), 1)
    );
    // The list shows the envelope, its count, and the act.
    let lines = w.gate().lines(&w.world).join("\n");
    assert!(lines.contains("forge — in force"), "{lines}");
    assert!(lines.contains("uses 1/3"), "{lines}");
    assert!(
        lines.contains(&format!("act {}", report.receipt)),
        "{lines}"
    );
}

#[test]
fn every_lapse_falls_back_to_the_human_saying_why() {
    let t = today();
    // (what, the file, earlier uses, what the card says)
    let cases: [(&str, String, u64, &str); 4] = [
        ("expired", envelope(-29, -1, ""), 0, "expired on"),
        (
            "at the cap",
            envelope(-1, 20, "uses = 2"),
            2,
            "all 2 uses spent",
        ),
        (
            "revoked",
            envelope(-1, 20, &format!("revoked = {t}")),
            0,
            "revoked on",
        ),
        ("not yet signed", envelope(2, 20, ""), 0, "not yet in force"),
    ];
    for (what, file, earlier, says) in cases {
        let w = Fixture::new();
        w.sign(&file);
        let signed = if what == "not yet signed" {
            t.plus(2)
        } else if what == "expired" {
            t.plus(-29)
        } else {
            t.plus(-1)
        };
        w.used("forge", signed, earlier);
        let forge = Forge::new();
        let report = w.oni_comment("Looks fine.", &forge).unwrap();
        assert_eq!(report.state, State::Observed, "{what}");
        assert_eq!(w.human.asked(), 1, "{what}: the human was not asked");
        let why = w.human.why_not();
        assert!(why.iter().any(|w| w.contains(says)), "{what}: {why:?}");
        assert_eq!(w.uses().len() as u64, earlier, "{what}: a use was counted");
        assert!(w.human.shown.lock().unwrap().is_empty(), "{what}");
    }
}

#[test]
fn an_open_incident_suspends_the_envelope_and_the_human_decides() {
    let w = Fixture::new();
    w.sign(&envelope(-1, 20, ""));
    let forge = Forge::new();
    let first = w.oni_comment("First pass.", &forge).unwrap();
    assert_eq!(w.human.asked(), 0);
    // The human undoes it: an incident opens on an act the envelope reads, naming it.
    let cx = w.cx(Race::Oni, "kijin-forge");
    let journal = PostJournal::of(&w.world, Post::Forge).witnessed_at(&w.witness(0));
    let undo = UndoForge(&forge);
    Runner::new(&cx, journal, &forge, &undo)
        .with_envelopes(Some(w.gate()))
        .undo(&first.receipt)
        .unwrap();
    let asked = w.human.asked();
    let incidents = PostJournal::of(&w.world, Post::Forge)
        .witnessed_at(&w.witness(0))
        .incidents()
        .unwrap();
    assert_eq!(incidents[0].envelope.as_deref(), Some("forge"));
    // Another pull request: the incident's own target takes no new act at all.
    let report = w
        .run(
            Race::Oni,
            "kijin-forge",
            0,
            &comment_on(13, "Second pass."),
            &forge,
            &forge,
        )
        .unwrap();
    assert_eq!(report.state, State::Observed);
    assert_eq!(
        w.human.asked(),
        asked + 1,
        "a suspended envelope admitted the act"
    );
    assert!(
        w.human.why_not().iter().any(|w| w.contains("suspended")),
        "{:?}",
        w.human.why_not()
    );
    assert_eq!(w.uses().len(), 1);
}

/// The forge's hands for an undo: the comment deleted.
struct UndoForge<'a>(&'a Forge);

impl Hands for UndoForge<'_> {
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
        assert!(planned.is_undo());
        let id: u64 = planned
            .command()
            .rsplit('/')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        self.0.comments.lock().unwrap().retain(|c| c.0 != id);
        // Counted apart from the acts: the next comment gets a fresh id.
        self.0
            .comments
            .lock()
            .unwrap()
            .push((9000 + id, 0, String::new()));
        let mut out = Outcome::ok(String::new(), ToolClass::Outward);
        out.meta.exit = Some(0);
        Spawned::Ran(out)
    }
}

/// What a project tries, the global file, the tracked file's kind, the local file, whether git
/// tracks the local file, and its mode.
type ProjectCase = (
    &'static str,
    String,
    &'static str,
    Option<String>,
    bool,
    u32,
);

#[test]
fn a_project_file_never_signs_or_extends_an_envelope() {
    let t = today();
    // (what, the global file, the tracked file, the local file, commit the local file, its mode)
    let cases: [ProjectCase; 6] = [
        (
            "the tracked file signs one",
            String::new(),
            "tracked",
            None,
            false,
            0o600,
        ),
        (
            "the tracked file extends a spent one",
            envelope(-1, 20, "uses = 1"),
            "extend",
            None,
            false,
            0o600,
        ),
        (
            "a local file git tracks",
            String::new(),
            "",
            Some(envelope(-1, 20, "")),
            true,
            0o600,
        ),
        (
            "a local file of the human's own, untracked and private",
            String::new(),
            "",
            Some(envelope(-1, 20, "")),
            false,
            0o600,
        ),
        (
            "a local file others may write",
            String::new(),
            "",
            Some(envelope(-1, 20, "")),
            false,
            0o666,
        ),
        (
            "the tracked file raises the lifetime",
            envelope(-200, 100, ""),
            "days",
            None,
            false,
            0o600,
        ),
    ];
    for (what, global, tracked, local, commit, mode) in cases {
        let w = Fixture::new();
        w.sign(&global);
        w.used("forge", t.plus(-1), 1);
        let dir = w.world.join(".mnemorph");
        fs::create_dir_all(&dir).unwrap();
        let tracked_text = match tracked {
            "tracked" => envelope(-1, 20, ""),
            "extend" => envelope(-1, 20, "uses = 100"),
            "days" => "[oni]\nenvelope_days = 365\n".to_owned(),
            _ => String::new(),
        };
        if !tracked_text.is_empty() {
            fs::write(dir.join("config.toml"), tracked_text).unwrap();
            git(&w.world, &["add", ".mnemorph/config.toml"]);
            git(&w.world, &["commit", "-qm", "envelope"]);
        }
        if let Some(text) = local {
            let path = dir.join("config.local.toml");
            fs::write(&path, w.bound(&text)).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            if commit {
                git(&w.world, &["add", "-f", ".mnemorph/config.local.toml"]);
                git(&w.world, &["commit", "-qm", "local"]);
            }
        }
        let forge = Forge::new();
        let report = w.oni_comment("Looks fine.", &forge);
        assert!(report.is_ok(), "{what}: {report:?}");
        assert_eq!(w.human.asked(), 1, "{what}: admitted without the human");
        assert_eq!(w.uses().len(), 1, "{what}: a use was counted");
    }
}

#[test]
fn an_envelope_admits_only_where_it_is_signed_for() {
    // (what, the envelope's scope given the world and another place, admitted, what the card says)
    type Scope = fn(&Path, &Path) -> String;
    let cases: [(&str, Scope, bool, &str); 8] = [
        (
            "its world",
            |w, _| format!("world = {:?}", w.display().to_string()),
            true,
            "",
        ),
        (
            "the same envelope in another world",
            |_, o| format!("world = {:?}", o.display().to_string()),
            false,
            "signed for the world",
        ),
        (
            "its repository",
            |_, _| "repo = \"Me/Proj\"".into(),
            true,
            "",
        ),
        (
            "another repository",
            |_, _| "repo = \"them/other\"".into(),
            false,
            "signed for the repository them/other",
        ),
        (
            "neither a world nor a repository",
            |_, _| "# unbound".into(),
            false,
            "signed for nothing",
        ),
        (
            "its post and body",
            |w, _| {
                format!(
                    "world = {:?}\npost = \"kijin-forge\"\nbody = \"kijin-forge\"",
                    w.display().to_string()
                )
            },
            true,
            "",
        ),
        (
            "another post",
            |w, _| format!("world = {:?}\npost = \"kijin-ci\"", w.display().to_string()),
            false,
            "signed for the post kijin-ci",
        ),
        (
            "another body",
            |w, _| {
                format!(
                    "world = {:?}\nbody = \"oni-other\"",
                    w.display().to_string()
                )
            },
            false,
            "signed for the body oni-other",
        ),
    ];
    for (what, scope, admitted, says) in cases {
        let w = Fixture::new();
        let other = Fixture::new();
        w.sign(&envelope(-1, 20, &scope(&w.world, &other.world)));
        let forge = Forge::new();
        let report = w.oni_comment("Looks fine.", &forge).unwrap();
        assert_eq!(report.state, State::Observed, "{what}");
        if admitted {
            assert_eq!(w.human.asked(), 0, "{what}: the human was asked");
            assert_eq!(w.uses().len(), 1, "{what}");
        } else {
            assert_eq!(w.human.asked(), 1, "{what}: admitted without the human");
            let why = w.human.why_not();
            assert!(why.iter().any(|w| w.contains(says)), "{what}: {why:?}");
            assert!(w.uses().is_empty(), "{what}: a use was counted");
        }
    }
    // An unbound envelope is listed as admitting nothing.
    let w = Fixture::new();
    w.sign(&envelope(-1, 20, "# unbound"));
    let lines = w.gate().lines(&w.world).join("\n");
    assert!(lines.contains("signed for no world or repo"), "{lines}");
}

#[test]
fn a_local_file_and_a_folder_others_may_write_are_never_read_for_envelopes() {
    // A local file's envelopes are ignored, and the list says so.
    let w = Fixture::new();
    let dir = w.world.join(".mnemorph");
    fs::create_dir_all(&dir).unwrap();
    let local = dir.join("config.local.toml");
    fs::write(&local, w.bound(&envelope(-1, 20, ""))).unwrap();
    fs::set_permissions(&local, fs::Permissions::from_mode(0o600)).unwrap();
    let forge = Forge::new();
    w.oni_comment("Looks fine.", &forge).unwrap();
    assert_eq!(w.human.asked(), 1, "a local file's envelope admitted");
    let lines = w.gate().lines(&w.world).join("\n");
    assert!(lines.contains("config.local.toml are ignored"), "{lines}");
    // The global file in a folder others may write: it may be replaced.
    let w = Fixture::new();
    w.sign(&envelope(-1, 20, ""));
    let parent = w.global.parent().unwrap();
    fs::set_permissions(parent, fs::Permissions::from_mode(0o777)).unwrap();
    w.oni_comment("Looks fine.", &forge).unwrap();
    assert_eq!(w.human.asked(), 1, "a file in a shared folder admitted");
    let why = w.human.why_not();
    assert!(why.iter().any(|w| w.contains("may be replaced")), "{why:?}");
    fs::set_permissions(parent, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn guarded_and_destructive_pushes_are_never_admitted() {
    // (what, the files the pushed commit changes, what the card says)
    let cases: [(&str, &[&str], &str); 4] = [
        (
            "a workflow",
            &[".github/workflows/ci.yml"],
            "CI configuration",
        ),
        ("a manifest", &["crates/x/Cargo.toml"], "a manifest"),
        ("a deploy", &["deploy/prod.yaml"], "deploy configuration"),
        ("a release", &["release-plz.toml"], "release configuration"),
    ];
    for (what, files, says) in cases {
        let w = Fixture::new();
        w.sign(&envelope(-1, 20, ""));
        for f in files {
            let p = w.world.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, "x\n").unwrap();
            git(&w.world, &["add", f]);
        }
        git(&w.world, &["commit", "-qm", what]);
        let observer = Observer::new(Sandbox::None("tests".into()), &w.home);
        let report = w
            .run(
                Race::Oni,
                "kijin-forge",
                0,
                &push("feat/x"),
                &observer,
                &Sandboxed,
            )
            .unwrap();
        assert_eq!(report.state, State::Observed, "{what}: {report:?}");
        assert_eq!(w.human.asked(), 1, "{what}: admitted without the human");
        assert!(
            w.human.why_not().iter().any(|w| w.contains(says)),
            "{what}: {:?}",
            w.human.why_not()
        );
        assert_eq!(w.uses().len(), 0, "{what}");
    }
    // A command is destructive: refused as untyped before any gate, never admitted.
    let w = Fixture::new();
    w.sign(&envelope(-1, 20, ""));
    let forge = Forge::new();
    let forced = Request {
        act: Act::Command("git push --force origin feat/x".into()),
        key: None,
        tells: None,
    };
    assert!(matches!(
        w.run(Race::Oni, "kijin-forge", 0, &forced, &forge, &forge),
        Err(Refusal::Refused(_))
    ));
    assert_eq!(w.uses().len(), 0);
    let destructive = plan(&forced.act, &Place::new(&w.world, &w.home)).unwrap();
    assert_eq!(destructive.class(), ToolClass::Destructive);
}

#[test]
fn a_push_is_judged_by_what_it_brings_not_the_whole_tree() {
    let w = Fixture::new();
    w.sign(&envelope(-1, 20, ""));
    // The workflow is on the remote already; the push brings only a.txt.
    let p = w.world.join(".github/workflows/ci.yml");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, "x\n").unwrap();
    git(&w.world, &["add", "."]);
    git(&w.world, &["commit", "-qm", "ci"]);
    git(&w.world, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
    fs::write(w.world.join("a.txt"), "two\n").unwrap();
    git(&w.world, &["commit", "-qam", "two"]);
    let observer = Observer::new(Sandbox::None("tests".into()), &w.home);
    let report = w
        .run(
            Race::Oni,
            "kijin-forge",
            0,
            &push("feat/x"),
            &observer,
            &Sandboxed,
        )
        .unwrap();
    assert_eq!(report.state, State::Observed, "{report:?}");
    assert_eq!(w.human.asked(), 0, "{:?}", w.human.why_not());
    assert_eq!(w.uses().len(), 1);
}

#[test]
fn long_or_secret_shaped_text_is_never_admitted() {
    let token = "ghp_aB3dE5gH7jK9mN1pQ3sT5vW7yZ9bC1dE3fG5";
    // (what, the body, admitted, what the card says)
    let cases: [(&str, String, bool, &str); 5] = [
        ("4096 bytes", "a".repeat(4096), true, ""),
        ("4097 bytes", "a".repeat(4097), false, "above 4096 bytes"),
        (
            "a token",
            format!("the token is {token} now"),
            false,
            "looks like a secret",
        ),
        (
            "a random-looking string",
            "key Zx9Qw3Er5Ty7Ui9Op1As3Df5Gh7Jk9Lz2Xc4".into(),
            false,
            "looks like a secret",
        ),
        (
            "plain words",
            "Rebased on main, ready for review.".into(),
            true,
            "",
        ),
    ];
    for (what, body, admitted, says) in cases {
        let w = Fixture::new();
        // gate.approve in the human's own file widens nothing.
        w.sign(&format!(
            "[gate]\napprove = [\"gh pr comment *\"]\n{}",
            envelope(-1, 20, "")
        ));
        let forge = Forge::new();
        let report = w.oni_comment(&body, &forge).unwrap();
        assert_eq!(report.state, State::Observed, "{what}");
        assert_eq!(w.human.asked(), usize::from(!admitted), "{what}");
        if !admitted {
            assert!(
                w.human.why_not().iter().any(|w| w.contains(says)),
                "{what}: {:?}",
                w.human.why_not()
            );
        }
        assert_eq!(w.uses().len(), usize::from(admitted), "{what}");
    }
}

#[test]
fn two_acts_racing_for_the_last_use_admit_only_one() {
    let w = Arc::new(Fixture::new());
    w.sign(&envelope(-1, 20, "uses = 2"));
    w.used("forge", today().plus(-1), 1);
    let forge = Arc::new(Forge {
        barrier: Some(Arc::new(Barrier::new(b_parties()))),
        ..Forge::new()
    });
    let threads: Vec<_> = (0..2)
        .map(|n| {
            let (w, forge) = (Arc::clone(&w), Arc::clone(&forge));
            std::thread::spawn(move || {
                // Two witnesses: two locks, so the post's own lock does not order them.
                w.run(
                    Race::Oni,
                    "kijin-forge",
                    n + 1,
                    &comment(&format!("Pass {n}.")),
                    &*forge,
                    &*forge,
                )
                .unwrap()
            })
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap().state, State::Observed);
    }
    assert_eq!(forge.posted(), 2);
    assert_eq!(w.uses().len(), 2, "both took the last use");
    assert_eq!(
        w.human.asked(),
        1,
        "the one past the cap did not go to the human"
    );
    assert!(
        w.human
            .why_not()
            .iter()
            .any(|w| w.contains("all 2 uses spent")),
        "{:?}",
        w.human.why_not()
    );
}

#[test]
fn an_envelope_revoked_after_the_preview_is_read_again_at_the_moment_of_use() {
    let w = Arc::new(Fixture::new());
    w.sign(&envelope(-1, 20, ""));
    let forge = Forge::new();
    let revoking = Arc::clone(&w);
    *forge.first.lock().unwrap() = Some(Box::new(move || {
        revoking.gate().revoke("forge").unwrap();
    }));
    let report = w.oni_comment("Looks fine.", &forge).unwrap();
    assert_eq!(report.state, State::Observed);
    assert_eq!(w.human.asked(), 1, "a revoked envelope admitted the act");
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("no longer did at the moment of use")),
        "{:?}",
        report.notes
    );
    assert_eq!(w.uses().len(), 0);
    // It stays on record, revoked, and is journaled.
    let lines = w.gate().lines(&w.world).join("\n");
    assert!(lines.contains("forge — revoked on"), "{lines}");
    assert!(
        fs::read_to_string(&w.global)
            .unwrap()
            .contains("revoked = ")
    );
    let journaled = fs::read_to_string(w.gate().revocations()).unwrap();
    assert!(journaled.contains("\"envelope\":\"forge\""), "{journaled}");
    assert!(w.gate().revoke("forge").is_err(), "revoked twice");
}

/// Lesson: a revoke rewrote the human's private global file through a sibling the umask made, so
/// under a group-writable umask every other envelope in it stopped being honoured. The file keeps
/// its own mode.
#[test]
fn a_revoke_keeps_the_global_file_private_and_its_other_envelopes_honoured() {
    let w = Fixture::new();
    let other = envelope(-1, 20, "").replace("name = \"forge\"", "name = \"other\"");
    w.sign(&format!("{}\n{other}", envelope(-1, 20, "")));
    w.gate().revoke("forge").unwrap();
    let mode = fs::metadata(&w.global).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "{mode:o}");
    let signed = w.gate().signed(&w.world).unwrap();
    assert_eq!(signed.len(), 2, "{signed:?}");
    let forge = Forge::new();
    assert_eq!(
        w.oni_comment("Looks fine.", &forge).unwrap().state,
        State::Observed
    );
    assert_eq!(w.human.asked(), 0, "the other envelope was not honoured");
}

#[test]
fn a_body_that_is_no_oni_is_never_admitted() {
    for race in [Race::Kijin, Race::Slime, Race::Orc] {
        let w = Fixture::new();
        w.sign(&envelope(-1, 20, ""));
        let forge = Forge::new();
        let result = w.run(
            race,
            "kijin-forge",
            0,
            &comment("Looks fine."),
            &forge,
            &forge,
        );
        if race == Race::Kijin {
            assert_eq!(result.unwrap().state, State::Observed);
            assert_eq!(w.human.asked(), 1);
            assert!(
                w.human.why_not().iter().any(|w| w.contains("only an Oni")),
                "{:?}",
                w.human.why_not()
            );
        } else {
            // A race whose shelf holds no outward act is refused before any gate.
            assert!(result.is_err(), "{race}");
        }
        assert_eq!(w.uses().len(), 0, "{race}");
    }
}

/// A probe that changes the world the first time a branch is read: between the plan and the spawn.
struct Meddling {
    inner: Observer,
    world: PathBuf,
    done: AtomicBool,
}

impl Probe for Meddling {
    fn run(&self, argv: &[String], timeout: Duration) -> Result<Seen, String> {
        if argv.iter().any(|a| a == "ls-remote") && !self.done.swap(true, Ordering::SeqCst) {
            fs::write(self.world.join("b.txt"), "meanwhile\n").unwrap();
        }
        self.inner.run(argv, timeout)
    }
}

#[test]
fn an_admitted_act_still_takes_every_step() {
    let w = Fixture::new();
    w.sign(&envelope(-1, 20, ""));
    let probe = Meddling {
        inner: Observer::new(Sandbox::None("tests".into()), &w.home),
        world: w.world.clone(),
        done: AtomicBool::new(false),
    };
    let report = w
        .run(
            Race::Oni,
            "kijin-forge",
            0,
            &push("feat/x"),
            &probe,
            &Sandboxed,
        )
        .unwrap();
    // The recheck caught the world moving: nothing ran, and the envelope was read again.
    assert!(
        report.notes.iter().any(|n| n.contains("the world changed")),
        "{:?}",
        report.notes
    );
    assert_eq!(w.human.asked(), 0);
    assert_eq!(w.uses().len(), 1, "the stale pass took a use");
    // Fingerprint, intent, result, probe: committed with its plan, then observed.
    let states: Vec<State> = w.rows(0).iter().map(|r| r.state).collect();
    assert_eq!(
        states,
        [State::Committed, State::PendingObservation, State::Observed]
    );
    assert!(w.rows(0)[0].plan.is_some());
    let head = git(&w.world, &["rev-parse", "HEAD"]);
    assert_eq!(git(&w.remote, &["rev-parse", "refs/heads/feat/x"]), head);
    // The post check: an Oni of another post is refused before any envelope is read.
    let forge = Forge::new();
    assert!(matches!(
        w.run(
            Race::Oni,
            "kijin-ci",
            1,
            &comment("Looks fine."),
            &forge,
            &forge
        ),
        Err(Refusal::Refused(_))
    ));
    assert_eq!(w.uses().len(), 1);
}

#[test]
fn the_exclusions_hold_for_every_standing_approval() {
    // `excluded` is what any standing approval reads first: a pull request's diff on the forge.
    struct Compare(serde_json::Value);
    impl Probe for Compare {
        fn run(&self, argv: &[String], _: Duration) -> Result<Seen, String> {
            assert!(
                argv.last()
                    .unwrap()
                    .starts_with("repos/me/proj/compare/main...feat/x")
            );
            Ok(Seen {
                code: Some(0),
                out: self.0.to_string().into_bytes(),
                ..Seen::default()
            })
        }
    }
    let w = Fixture::new();
    let act = Act::OpenPullRequest {
        repo: "me/proj".into(),
        base: "main".into(),
        head: "feat/x".into(),
        title: "Add the undo".into(),
        body: "It undoes.".into(),
    };
    let planned = plan(&act, &Place::new(&w.world, &w.home)).unwrap();
    let pin = mnemorph_config::Pin::Forge {
        host: "github.com".into(),
    };
    let files = |names: &[&str]| {
        Compare(
            serde_json::json!({"files": names.iter().map(|n| serde_json::json!({"filename": n})).collect::<Vec<_>>()}),
        )
    };
    assert_eq!(
        excluded(&planned, &pin, &w.world, &files(&["src/undo.rs"])),
        Ok(())
    );
    let e = excluded(
        &planned,
        &pin,
        &w.world,
        &files(&["src/undo.rs", ".github/workflows/x.yml"]),
    );
    assert!(e.unwrap_err().contains("CI configuration"));
    let many: Vec<String> = (0..300).map(|i| format!("src/{i}.rs")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    assert!(excluded(&planned, &pin, &w.world, &files(&many)).is_err());
    let secret = plan(
        &Act::OpenPullRequest {
            repo: "me/proj".into(),
            base: "main".into(),
            head: "feat/x".into(),
            title: "ghp_aB3dE5gH7jK9mN1pQ3sT5vW7yZ9bC1dE3fG5".into(),
            body: "x".into(),
        },
        &Place::new(&w.world, &w.home),
    )
    .unwrap();
    assert!(
        excluded(&secret, &pin, &w.world, &files(&["src/a.rs"]))
            .unwrap_err()
            .contains("secret")
    );
}
