//! `mnemorphd verify …` is the human's door to the verify pins, kept under the human's state
//! directory (a temporary `XDG_STATE_HOME` here): listing is anyone's; pinning is the human's, at
//! the terminal or with `--yes` from a script they run, and refused inside a body's command.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture {
    root: PathBuf,
    world: PathBuf,
    state: PathBuf,
}

const CARD: &str = ".mnemorph/rimuru/rimuru/README.md";

impl Fixture {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorphd-pins-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("world");
        let state = root.join("state");
        fs::create_dir_all(world.join(".mnemorph/rimuru/rimuru")).unwrap();
        let world = world.canonicalize().unwrap();
        let f = Self { root, world, state };
        f.card("pytest -q");
        f
    }

    fn card(&self, line: &str) {
        fs::write(
            self.world.join(CARD),
            format!("# rimuru\n\n- **Race:** rimuru\n- **Verify:** `{line}`\n"),
        )
        .unwrap();
    }

    /// Run with no controlling terminal (a session of its own, on Linux), `body` as the asker
    /// when one is given.
    fn run(&self, args: &[&str], body: Option<&str>) -> Output {
        let mut cmd = if cfg!(target_os = "linux") {
            let mut c = Command::new("setsid");
            c.args(["-w", env!("CARGO_BIN_EXE_mnemorphd")]);
            c
        } else {
            Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        };
        cmd.arg("verify")
            .args(args)
            .args(["--world", self.world.to_str().unwrap()])
            .env("XDG_STATE_HOME", &self.state)
            .env_remove("MNEMORPH_BODY")
            .stdin(std::process::Stdio::null());
        if let Some(b) = body {
            cmd.env("MNEMORPH_BODY", b);
        }
        cmd.output().unwrap()
    }

    /// Run at a terminal of its own (a pseudo-terminal `script` makes), `input` typed at it.
    #[cfg(target_os = "linux")]
    fn at_tty(&self, args: &[&str], input: &str) -> Output {
        use std::io::Write as _;
        let line = format!(
            "{} verify {} --world '{}'",
            env!("CARGO_BIN_EXE_mnemorphd"),
            args.join(" "),
            self.world.display()
        );
        let mut child = Command::new("script")
            .args(["-qec", &line, "/dev/null"])
            .env("XDG_STATE_HOME", &self.state)
            .env_remove("MNEMORPH_BODY")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        // Typed once the question is up: the terminal reads it, not the pipe.
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = stdin.write_all(input.as_bytes());
        drop(stdin);
        child.wait_with_output().unwrap()
    }

    fn witness(&self) -> Option<PathBuf> {
        fs::read_dir(self.state.join("mnemorph/verify"))
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .next()
    }

    fn rows(&self) -> usize {
        self.witness()
            .map_or(0, |p| fs::read_to_string(p).unwrap().lines().count())
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

/// A body's command is refused before anything is asked — `--yes` too — and piped input is no
/// human's answer. Listing is anyone's.
#[test]
fn a_body_never_pins_and_piped_input_is_no_answer() {
    let f = Fixture::new("body");
    for args in [&["pin", "rimuru"][..], &["pin", "rimuru", "--yes"]] {
        let out = f.run(args, Some("slime-a"));
        assert_eq!(out.status.code(), Some(2), "{}", text(&out));
        assert!(
            text(&out).contains("slime-a runs this: a verify line is pinned by the human alone"),
            "{}",
            text(&out)
        );
    }
    let out = f.run(&["pin", "rimuru"], None);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("answered at a terminal"),
        "{}",
        text(&out)
    );
    assert_eq!(f.rows(), 0, "nothing was pinned");
    let out = f.run(&["list"], Some("slime-a"));
    assert!(
        out.status.success() && text(&out).contains("rimuru — not pinned: its gate runs nothing"),
        "{}",
        text(&out)
    );
}

/// At the terminal, the line is shown; `n` pins nothing, `y` pins it, and `list` says so. A card
/// edited after is said to differ.
#[cfg(target_os = "linux")]
#[test]
fn the_human_pins_at_the_terminal() {
    let f = Fixture::new("tty");
    let out = f.at_tty(&["pin", "rimuru"], "n\n");
    assert!(text(&out).contains("`pytest -q`"), "{}", text(&out));
    assert!(text(&out).contains("nothing was pinned"), "{}", text(&out));
    assert_eq!(f.rows(), 0);
    let out = f.at_tty(&["pin", "rimuru"], "y\n");
    assert!(text(&out).contains("rimuru pinned"), "{}", text(&out));
    assert_eq!(f.rows(), 1);
    let out = f.run(&["list"], None);
    assert!(
        text(&out).contains("rimuru — pinned") && text(&out).contains("pinned  `pytest -q`"),
        "{}",
        text(&out)
    );
    f.card("true");
    let out = f.run(&["list"], None);
    assert!(
        text(&out).contains("rimuru — differs from the pin: its gate runs nothing"),
        "{}",
        text(&out)
    );
}

/// `--yes` pins with no terminal, for a script the human runs: a benchmark harness that writes the
/// card itself.
#[test]
fn yes_pins_for_a_script() {
    let f = Fixture::new("yes");
    let out = f.run(&["pin", "rimuru", "--yes"], None);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(f.rows(), 1);
    let out = f.run(&["list", "--yes"], None);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
}

/// A witness edited in place breaks its chain: `list` and `pin` both refuse.
#[test]
fn a_tampered_witness_is_refused() {
    let f = Fixture::new("tamper");
    assert!(f.run(&["pin", "rimuru", "--yes"], None).status.success());
    let at = f.witness().unwrap();
    let t = fs::read_to_string(&at).unwrap();
    fs::write(&at, t.replace("pytest -q", "true")).unwrap();
    for args in [&["list"][..], &["pin", "rimuru", "--yes"]] {
        let out = f.run(args, None);
        assert_eq!(out.status.code(), Some(2), "{}", text(&out));
        assert!(
            text(&out).contains("the verify pins do not verify"),
            "{}",
            text(&out)
        );
    }
}
