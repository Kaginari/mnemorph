//! `mnemorphd challenge …` is the human's door to the Dark Elf challenges, kept under the human's
//! state directory (a temporary `XDG_STATE_HOME` here): listing what holds a landing, showing one,
//! and waiving one with a reason — never a body's, never without a reason.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use mnemorph_server::world::Roster;
use mnemorph_server::world::challenge::{Challenges, Opening, Status, route};

/// Where the harness keeps a world's challenges: `<state>/mnemorph/challenges/<name>-<fnv64a>.jsonl`.
fn records(state: &Path, world: &Path) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world.file_name().unwrap().to_string_lossy();
    state
        .join("mnemorph/challenges")
        .join(format!("{name}-{h:016x}.jsonl"))
}

struct Fixture {
    root: PathBuf,
    world: PathBuf,
    state: PathBuf,
}

impl Fixture {
    /// A world whose landing `run-1:2`, by `orc-o`, waits on the harness speciality.
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorphd-challenge-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("world");
        let state = root.join("state");
        fs::create_dir_all(&world).unwrap();
        let world = world.canonicalize().unwrap();
        let landing = route(
            &Roster::default(),
            "run-1:2",
            &["crates/mnemorph-tools/src/files.rs".to_owned()],
        );
        let opening = Opening {
            tree: "ab".repeat(32),
            by: "orc-o".to_owned(),
        };
        Challenges::at(&records(&state, &world))
            .open_at(&landing, &opening)
            .unwrap();
        Self { root, world, state }
    }

    fn status(&self) -> Option<Status> {
        Challenges::at(&records(&self.state, &self.world)).status("run-1:2")
    }

    /// Run with `input` on stdin, and `body` as the asker when one is given.
    fn run(&self, args: &[&str], input: &str, body: Option<&str>) -> Output {
        use std::io::Write as _;
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_mnemorphd"));
        cmd.arg("challenge")
            .args(args)
            .args(["--world", self.world.to_str().unwrap()])
            .env("XDG_STATE_HOME", &self.state)
            .env_remove("MNEMORPH_BODY")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if let Some(b) = body {
            cmd.env("MNEMORPH_BODY", b);
        }
        let mut child = cmd.spawn().unwrap();
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

const WHY: &str = "the reader is a test fixture the human wrote by hand";

#[test]
fn the_human_lists_shows_and_waives_with_a_reason() {
    let f = Fixture::new("waive");
    let out = f.run(&["list"], "", None);
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        text(&out).contains("`run-1:2` by orc-o: challenge pending — awaiting dark-elf-harness"),
        "{}",
        text(&out)
    );
    let out = f.run(&["show", "run-1:2"], "", None);
    assert!(text(&out).contains(&format!("tree {}", "ab".repeat(32))));
    assert!(text(&out).contains("wrote crates/mnemorph-tools/src/files.rs"));
    // Not confirmed: nothing is waived.
    let out = f.run(&["waive", "run-1:2", "--why", WHY], "n\n", None);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(matches!(f.status(), Some(Status::Pending { .. })));
    let out = f.run(&["waive", "run-1:2", "--why", WHY], "y\n", None);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains(&format!("waived run-1:2: {WHY}")));
    assert_eq!(
        f.status(),
        Some(Status::Waived {
            reason: WHY.to_owned()
        })
    );
    let out = f.run(&["list"], "", None);
    assert_eq!(text(&out), "no challenge holds a landing\n");
    let out = f.run(&["list", "--all"], "", None);
    assert!(text(&out).contains("waived by the human"), "{}", text(&out));
    let out = f.run(&["waive", "run-1:2", "--why", WHY], "y\n", None);
    assert_eq!(out.status.code(), Some(2), "waived once: {}", text(&out));
}

/// A waiver from a body, one with no reason, or one for no landing, is refused and waives
/// nothing.
#[test]
fn a_forged_or_unreasoned_waiver_is_refused() {
    let f = Fixture::new("forged");
    for (args, body, says) in [
        (
            vec!["waive", "run-1:2", "--why", WHY],
            Some("orc-o"),
            "orc-o runs this",
        ),
        (vec!["list"], Some("dark-elf-harness"), "never a body's"),
        (vec!["waive", "run-1:2"], None, "a waiver says why"),
        (
            vec!["waive", "run-1:2", "--why", "n/a"],
            None,
            "at least 20 characters",
        ),
        (
            vec!["waive", "nope", "--why", WHY],
            None,
            "no challenge is open",
        ),
        (vec!["show", "nope"], None, "no challenge is open"),
        (vec!["frob"], None, "list, show or waive"),
    ] {
        let out = f.run(&args, "y\n", body);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out));
        assert!(text(&out).contains(says), "{args:?}: {}", text(&out));
    }
    assert!(matches!(f.status(), Some(Status::Pending { .. })));
}
