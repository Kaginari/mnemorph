//! `mnemorphd incident …` is the human's door to the Kijin's incidents, kept under the human's
//! state directory (a temporary `XDG_STATE_HOME` here): marking a `reconcile` an incident, listing
//! what it misses, recording its lesson, settling a `reconcile` that is none — and never a body's.

#[path = "support/isolated.rs"]
mod isolated;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Where the runner keeps a post's witness: `<state>/mnemorph/kijin/<name>-<fnv64a>/<post>.jsonl`.
fn witness(state: &Path, world: &Path, post: &str) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world.file_name().unwrap().to_string_lossy();
    state
        .join("mnemorph/kijin")
        .join(format!("{name}-{h:016x}"))
        .join(format!("{post}.jsonl"))
}

struct Fixture {
    root: PathBuf,
    world: PathBuf,
    state: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorphd-incident-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("world");
        let state = root.join("state");
        fs::create_dir_all(&world).unwrap();
        fs::create_dir_all(&state).unwrap();
        let world = world.canonicalize().unwrap();
        // A world with a commit: a lesson is what changed since the incident opened.
        fs::create_dir_all(world.join("tests")).unwrap();
        fs::write(world.join("tests/old.rs"), "#[test]\nfn old() {}\n").unwrap();
        for args in [
            &["init", "-q"][..],
            &["add", "tests/old.rs"],
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "one",
            ],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&world)
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .status()
                    .unwrap()
                    .success()
            );
        }
        // Two forge acts the probe could not place: `abc123` and `def456`, each with its intent.
        let w = witness(&state, &world, "kijin-forge");
        fs::create_dir_all(w.parent().unwrap()).unwrap();
        let at = "2026-10-27T10:00:00Z";
        let mut rows = String::new();
        let mut intents = String::new();
        for act in ["abc123", "def456"] {
            for state in ["committed", "reconcile"] {
                rows.push_str(
                    &serde_json::json!({"act": act, "state": state, "at": at}).to_string(),
                );
                rows.push('\n');
            }
            intents.push_str(
                &serde_json::json!({
                    "act": act, "key": "k", "request": "r", "pinned": at, "at": at,
                    "typed": {"kind": "add-label", "repo": "me/proj", "pr": 12, "label": "ready"},
                    "pin": {"pin": "forge", "host": "github.com"}
                })
                .to_string(),
            );
            intents.push('\n');
        }
        fs::write(&w, rows).unwrap();
        fs::write(w.with_extension("intents.jsonl"), intents).unwrap();
        Self { root, world, state }
    }

    fn run(&self, args: &[&str]) -> Output {
        self.answer(args, "")
    }

    /// Run with `input` on stdin: the human's answer to a confirmation.
    fn answer(&self, args: &[&str], input: &str) -> Output {
        use std::io::Write as _;
        let mut child = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
            .arg("incident")
            .args(args)
            .args(["--world", self.world.to_str().unwrap()])
            .env("XDG_STATE_HOME", &self.state)
            .env_remove("MNEMORPH_BODY")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let _ = child.stdin.take().unwrap().write_all(input.as_bytes());
        child.wait_with_output().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn the_human_marks_lists_and_settles_incidents_and_records_a_lesson() {
    let f = Fixture::new("door");
    let out = f.run(&["list"]);
    assert_eq!(text(&out), "no incident is open\n");
    let out = f.run(&[
        "open",
        "forge",
        "abc123",
        "--why",
        "the wrong\npull request",
    ]);
    assert!(out.status.success(), "{}", text(&out));
    let out = f.run(&["list"]);
    let listed = text(&out);
    assert!(
        listed.contains("kijin-forge incident on act abc123 (reconcile)")
            && listed.contains("missing undone, journal corrected, downstream told, lesson")
            && listed.contains("the wrong\\npull request"),
        "{listed}"
    );
    assert_eq!(listed.lines().count(), 1, "{listed}");
    // The lesson must exist in the world.
    let out = f.run(&["lesson", "forge", "abc123", "test", "tests/missing.rs"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    // A test the world held unchanged when the incident opened is no lesson.
    let out = f.answer(
        &["lesson", "forge", "abc123", "test", "tests/old.rs"],
        "y\n",
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("unchanged"), "{}", text(&out));
    fs::write(
        f.world.join("tests/label.rs"),
        "#[test]\nfn asks_first() {}\n",
    )
    .unwrap();
    // The human sees the lesson and declines: nothing is recorded.
    let lesson = ["lesson", "kijin-forge", "abc123", "test", "tests/label.rs"];
    let out = f.answer(&lesson, "n\n");
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("fn asks_first"), "{}", text(&out));
    let out = f.answer(&lesson, "y\n");
    assert!(out.status.success(), "{}", text(&out));
    let said = text(&out);
    assert!(
        said.contains("still open") && said.contains("missing undone"),
        "{said}"
    );
    // The other is no incident: settled, and it is listed nowhere.
    let out = f.run(&["settle", "forge", "def456"]);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(text(&f.run(&["list"])).lines().count(), 1);
    // An incident is not settled.
    let out = f.run(&["settle", "forge", "abc123"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    // The incidents are kept beside the witness; the world holds only the journal's rows.
    let kept: Vec<_> = fs::read_dir(f.world.join(".mnemorph/kijin/kijin-forge"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(kept, ["post.jsonl"]);
    let w = witness(&f.state, &f.world, "kijin-forge");
    assert!(w.with_extension("incidents.jsonl").exists());
}

#[test]
fn a_body_never_opens_or_settles_an_incident() {
    let f = Fixture::new("body");
    let out = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(["incident", "settle", "forge", "abc123", "--world"])
        .arg(&f.world)
        .env("XDG_STATE_HOME", &f.state)
        .env("MNEMORPH_BODY", "kijin-forge")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).contains("the human's"), "{}", text(&out));
    let out = f.run(&["list"]);
    assert_eq!(text(&out), "no incident is open\n");
    let w = witness(&f.state, &f.world, "kijin-forge");
    assert!(!fs::read_to_string(w).unwrap().contains("settled"));
}
