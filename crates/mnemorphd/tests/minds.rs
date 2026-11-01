//! `mnemorphd mind …` is the human's door to the world's Minds, whose provenance — adoption,
//! owner, retirement — is kept under the human's state directory (a temporary `XDG_STATE_HOME`
//! here): listing is anyone's; adopting, retiring and unretiring are the human's, at the terminal,
//! and refused inside a body's command.

#[path = "support/isolated.rs"]
mod isolated;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

struct Fixture {
    root: PathBuf,
    world: PathBuf,
    state: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorphd-minds-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("world");
        let state = root.join("state");
        fs::create_dir_all(world.join(".mnemorph/minds")).unwrap();
        fs::write(
            world.join(".mnemorph/minds/release.md"),
            "# Release\n\nCutting a release by hand.\n",
        )
        .unwrap();
        let world = world.canonicalize().unwrap();
        Self { root, world, state }
    }

    /// Run with no controlling terminal (a session of its own, on Linux), `body` as the asker
    /// when one is given.
    fn run(&self, args: &[&str], body: Option<&str>) -> Output {
        let mut cmd = if cfg!(target_os = "linux") {
            let mut c = isolated::bin("setsid");
            c.args(["-w", env!("CARGO_BIN_EXE_mnemorphd")]);
            c
        } else {
            isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
        };
        cmd.arg("mind")
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
            "{} mind {} --world '{}'",
            env!("CARGO_BIN_EXE_mnemorphd"),
            args.join(" "),
            self.world.display()
        );
        let mut child = isolated::bin("script")
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

    fn witness_rows(&self) -> usize {
        let Ok(rd) = fs::read_dir(self.state.join("mnemorph/minds")) else {
            return 0;
        };
        rd.filter_map(Result::ok)
            .map(|e| {
                fs::read_to_string(e.path())
                    .unwrap_or_default()
                    .lines()
                    .count()
            })
            .sum()
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

/// Adopting, retiring and unretiring are the human's: a body's command is refused before
/// anything is asked, and piped input is no human's answer. Listing is anyone's.
#[test]
fn a_body_never_adopts_retires_or_unretires_a_mind() {
    let f = Fixture::new("body");
    for verb in ["adopt", "retire", "unretire"] {
        let out = f.run(&[verb, "release"], Some("slime-a"));
        assert_eq!(out.status.code(), Some(2), "{verb}: {}", text(&out));
        assert!(
            text(&out).contains(
                "slime-a runs this: a Mind is adopted, retired and unretired by the human alone"
            ),
            "{verb}: {}",
            text(&out)
        );
    }
    let out = f.run(&["adopt", "release"], None);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("answered at a terminal"),
        "{}",
        text(&out)
    );
    assert_eq!(f.witness_rows(), 0, "nothing was recorded");
    let out = f.run(&["list"], Some("slime-a"));
    assert!(
        out.status.success()
            && text(&out).contains("release — unadopted: adopt to use (not loaded)"),
        "{}",
        text(&out)
    );
}

/// At the terminal, `n` records nothing; `y` adopts the Mind with its owner, and `list` shows it.
/// Retired, it is said so; a doc a body edits after is said changed.
#[cfg(target_os = "linux")]
#[test]
fn the_human_adopts_and_retires_a_mind_at_the_terminal() {
    let f = Fixture::new("tty");
    let out = f.at_tty(&["adopt", "release"], "n\n");
    assert!(
        text(&out).contains("nothing was recorded"),
        "{}",
        text(&out)
    );
    assert_eq!(f.witness_rows(), 0);
    let out = f.at_tty(&["adopt", "release"], "y\n");
    let shown = text(&out);
    assert!(
        shown.contains("doc     .mnemorph/minds/release.md")
            && shown.contains("│ # Release")
            && shown.contains("│ Cutting a release by hand.")
            && shown.contains("owner   human")
            && shown.contains("release adopted"),
        "{shown}"
    );
    assert_eq!(f.witness_rows(), 1);
    let listed = text(&f.run(&["list"], None));
    assert!(
        listed.contains("release — adopted, owner human, sha256 "),
        "{listed}"
    );
    assert!(!listed.contains("adopt each"), "{listed}");
    fs::write(
        f.world.join(".mnemorph/minds/release.md"),
        "# Release\n\n- **Retired:** yes\n",
    )
    .unwrap();
    let listed = text(&f.run(&["list"], None));
    assert!(
        listed.contains("release — changed since adoption: re-adopt (not loaded), owner human"),
        "{listed}"
    );
    let out = f.at_tty(&["retire", "release"], "y\n");
    assert!(text(&out).contains("release retired"), "{}", text(&out));
    let listed = text(&f.run(&["list"], None));
    assert!(
        listed.contains("release — retired (not loaded), owner human"),
        "{listed}"
    );
}

/// The adopt prompt shows the doc's text, escaped and capped, and the digest recorded is of
/// exactly the bytes shown.
#[cfg(target_os = "linux")]
#[test]
fn adoption_records_the_digest_of_the_text_shown() {
    use sha2::Digest as _;
    use std::fmt::Write as _;
    let f = Fixture::new("shown");
    let mut doc = String::from("# Long\n\nA \u{1b}[31mred\u{1b}[0m line.\n");
    for n in 0..100 {
        let _ = writeln!(doc, "step {n}");
    }
    fs::write(f.world.join(".mnemorph/minds/long.md"), &doc).unwrap();
    let out = f.at_tty(&["adopt", "long"], "y\n");
    let shown = text(&out);
    assert!(
        shown.contains("│ # Long") && shown.contains("│ step 56"),
        "{shown}"
    );
    assert!(
        !shown.contains("step 57") && shown.contains("… 43 more lines"),
        "{shown}"
    );
    assert!(!shown.contains('\u{1b}'), "escaped: {shown}");
    let digest = sha2::Sha256::digest(doc.as_bytes())
        .iter()
        .fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        });
    assert!(shown.contains(&format!("sha256  {digest}")), "{shown}");
    let witness: String = fs::read_dir(f.state.join("mnemorph/minds"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| fs::read_to_string(e.path()).unwrap())
        .collect();
    assert!(
        witness.contains(&format!("\"sha256\":\"{digest}\"")),
        "{witness}"
    );
}
