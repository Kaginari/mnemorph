//! `mnemorphd roster …` is the human's door to the creatures' witness, kept under the human's
//! state directory (a temporary `XDG_STATE_HOME` here): a cloned world's cards are strangers
//! until the human adopts them at the terminal; a body, a harness process or a state directory
//! inside the world adopts nothing; a card raised after runs at the lower, with a finding; a
//! witness edited trusts no row.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture {
    root: PathBuf,
    world: PathBuf,
    state: PathBuf,
}

const FORGE: &str = ".mnemorph/kijin/forge/README.md";
const ONI: &str =
    "# kijin-forge\n\n- **Race:** oni\n- **Reports to:** rimuru\n- **Post:** .github/workflows/\n";

impl Fixture {
    /// A cloned world: the throne, an Oni card named for the forge post, and a Slime.
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorphd-roster-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("world");
        let state = root.join("state");
        fs::create_dir_all(&world).unwrap();
        let world = world.canonicalize().unwrap();
        let f = Self { root, world, state };
        f.card(
            ".mnemorph/rimuru/rimuru/README.md",
            "# rimuru\n\n- **Race:** rimuru\n",
        );
        f.card(FORGE, ONI);
        f.card(
            ".mnemorph/slime/s/README.md",
            "# slime-s\n\n- **Race:** slime\n- **Reports to:** rimuru\n- **Territory:** src/\n",
        );
        f
    }

    fn card(&self, at: &str, text: &str) {
        let path = self.world.join(at);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
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
        cmd.args(args)
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
            "{} {} --world '{}'",
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
        fs::read_dir(self.state.join("mnemorph/roster"))
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "jsonl"))
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

/// A cloned world's cards are strangers: listed so, and owning nothing their cards claim.
#[test]
fn a_cloned_worlds_cards_are_strangers() {
    let f = Fixture::new("clone");
    let out = f.run(&["roster", "list"], None);
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        text(&out).contains("kijin-forge [stranger] card: kijin-forge — oni, post kijin-forge")
            && text(&out).contains("rimuru [throne]")
            && text(&out).contains("mnemorphd roster adopt kijin-forge"),
        "{}",
        text(&out)
    );
    let ci = f.world.join(".github/workflows/ci.yml");
    let src = f.world.join("src/a.rs");
    let out = f.run(
        &["onto", "owner", ci.to_str().unwrap(), src.to_str().unwrap()],
        None,
    );
    let said = text(&out);
    assert!(
        said.contains("ci.yml\tnobody\tunclaimed")
            && said.contains("its card says kijin-forge: it owns nothing until")
            && said.contains("a.rs\tnobody\tunclaimed")
            && said.contains("its card says slime-s"),
        "{said}"
    );
}

/// A body's command adopts nothing — `--yes` too — and piped input is no human's answer.
#[test]
fn a_body_never_adopts_and_piped_input_is_no_answer() {
    let f = Fixture::new("body");
    for args in [&["roster", "adopt"][..], &["roster", "adopt", "--yes"]] {
        let out = f.run(args, Some("slime-s"));
        assert_eq!(out.status.code(), Some(2), "{}", text(&out));
        assert!(
            text(&out).contains("slime-s runs this: a creature's standing is the human's word"),
            "{}",
            text(&out)
        );
    }
    for args in [
        &["roster", "set-race", "kijin-forge", "oni", "--yes"][..],
        &["roster", "retire", "slime-s", "--yes"],
    ] {
        assert_eq!(f.run(args, Some("slime-s")).status.code(), Some(2));
    }
    let out = f.run(&["roster", "adopt"], None);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("terminal"), "{}", text(&out));
    assert_eq!(f.rows(), 0, "nothing was adopted");
}

/// `MNEMORPH_BODY` is a body's to unset: an adoption asked under the harness — here a grandchild
/// of a process named `mnemorph` — is refused all the same.
#[cfg(target_os = "linux")]
#[test]
fn an_adoption_under_the_harness_is_refused_whatever_its_environment() {
    let f = Fixture::new("ancestry");
    let fake = f.root.join("bin/mnemorph");
    fs::create_dir_all(fake.parent().unwrap()).unwrap();
    fs::copy("/bin/sh", &fake).unwrap();
    let inner = format!(
        "{} roster adopt --yes --world '{}'; true",
        env!("CARGO_BIN_EXE_mnemorphd"),
        f.world.display()
    );
    let out = Command::new(&fake)
        .args(["-c", &format!("/bin/sh -c \"{inner}\"; true")])
        .env("XDG_STATE_HOME", &f.state)
        .env_remove("MNEMORPH_BODY")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(
        text(&out).contains("this runs under mnemorph"),
        "{}",
        text(&out)
    );
    assert_eq!(f.rows(), 0, "nothing was adopted");
}

/// A state directory inside the world is the body's: no row is written or read there.
#[test]
fn a_state_directory_inside_the_world_is_refused() {
    let f = Fixture::new("inside");
    let out = Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(["roster", "adopt", "--yes", "--world"])
        .arg(&f.world)
        .env("XDG_STATE_HOME", f.world.join(".mnemorph/tmp/state"))
        .env_remove("MNEMORPH_BODY")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("lies inside the world"),
        "{}",
        text(&out)
    );
    assert!(!f.world.join(".mnemorph/tmp/state/mnemorph/roster").exists());
}

/// At the terminal, every stranger is shown; `n` adopts nothing, `y` adopts them all. A card the
/// body raises after runs at the lower, `onto check` failing on the mismatch; the human's
/// `set-race` makes the rise the witness's, card and row alike.
#[cfg(target_os = "linux")]
#[test]
fn the_human_adopts_at_the_terminal_and_a_raised_card_runs_lower() {
    let f = Fixture::new("tty");
    let out = f.at_tty(&["roster", "adopt"], "n\n");
    assert!(
        text(&out).contains(
            "kijin-forge — oni, post kijin-forge, reports to rimuru, ground Post .github/workflows/"
        ),
        "{}",
        text(&out)
    );
    assert!(text(&out).contains("nothing was written"), "{}", text(&out));
    assert_eq!(f.rows(), 0);
    let out = f.at_tty(&["roster", "adopt"], "y\n");
    assert!(text(&out).contains("adopted 2 card(s)"), "{}", text(&out));
    assert_eq!(f.rows(), 2, "the throne is implicit");
    let out = f.run(&["roster", "list"], None);
    assert!(
        text(&out).contains("kijin-forge [trusted]"),
        "{}",
        text(&out)
    );
    // The Slime's body commits its card as a Named Slime with a wider zone.
    f.card(
        ".mnemorph/slime/s/README.md",
        "# slime-s\n\n- **Race:** named-slime\n- **Reports to:** rimuru\n- **Territory:** src/, ./Cargo.toml\n",
    );
    let out = f.run(&["roster", "list"], None);
    assert!(
        text(&out).contains("slime-s [mismatched]")
            && text(&out).contains("differs on Race, Territory"),
        "{}",
        text(&out)
    );
    let out = f.run(&["onto", "check"], None);
    assert!(!out.status.success(), "{}", text(&out));
    assert!(
        text(&out).contains("high\troster-mismatch\t.mnemorph/slime/s/README.md:3"),
        "{}",
        text(&out)
    );
    let cargo = f.world.join("Cargo.toml");
    let out = f.run(&["onto", "owner", cargo.to_str().unwrap()], None);
    assert!(text(&out).contains("Cargo.toml\tnobody"), "{}", text(&out));
    // The human's rise: row and card both.
    let out = f.run(
        &["roster", "set-race", "kijin-forge", "kijin", "--yes"],
        None,
    );
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        fs::read_to_string(f.world.join(FORGE))
            .unwrap()
            .contains("- **Race:** kijin\n")
    );
    let out = f.run(&["roster", "list"], None);
    assert!(
        text(&out).contains("kijin-forge [trusted]"),
        "{}",
        text(&out)
    );
}

/// A witness edited after it was written trusts no row: the CLI says where it breaks, and every
/// creature is a stranger.
#[test]
fn a_tampered_witness_trusts_nobody() {
    let f = Fixture::new("tamper");
    let out = f.run(&["roster", "adopt", "--yes"], None);
    assert!(out.status.success(), "{}", text(&out));
    let at = f.witness().unwrap();
    let rows = fs::read_to_string(&at).unwrap();
    fs::write(&at, rows.replace("\"slime\"", "\"named-slime\"")).unwrap();
    let out = f.run(&["roster", "list"], None);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("does not verify"), "{}", text(&out));
    let src = f.world.join("src/a.rs");
    let out = f.run(&["onto", "owner", src.to_str().unwrap()], None);
    assert!(
        text(&out).contains("a.rs\tnobody\tunclaimed") && text(&out).contains("does not verify"),
        "{}",
        text(&out)
    );
    let out = f.run(&["roster", "adopt", "--yes"], None);
    assert_eq!(
        out.status.code(),
        Some(2),
        "no adoption over a broken chain"
    );
}

impl Fixture {
    /// `n` runs of `slime-s` an hour apart, each landing passed on the human's pins, written to
    /// the usage witness and the gate verdicts as the harness writes them.
    fn passes(&self, n: i64) {
        use mnemorph_server::world::ledger::{Build, Journal, Timestamp, Usage, UsageRow};
        use mnemorph_server::world::queue::state_file_in;
        use mnemorph_server::world::verdicts::{self, VerdictRow};
        let t0 = 1_793_000_000;
        for k in 0..n {
            let run = format!("slime-s-{k}");
            Journal::<UsageRow>::chained(state_file_in(&self.state, &self.world, "usage"))
                .append(&UsageRow {
                    ts: Timestamp(t0 + k * 3600),
                    session: "s".into(),
                    run: run.clone(),
                    body: "slime-s".into(),
                    race: "slime".into(),
                    commission: Some("rimuru".into()),
                    model: "m".into(),
                    provider: "p".into(),
                    usage: Usage::default(),
                    prefix: 0,
                    wt: 0,
                    usd: None,
                    reported_cost: None,
                    estimated: false,
                    envelope: None,
                    purpose: None,
                })
                .unwrap();
            verdicts::journal_at(state_file_in(&self.state, &self.world, "gate"))
                .append(&VerdictRow {
                    ts: Timestamp(t0 + k * 3600 + 600),
                    body: "slime-s".into(),
                    verdict: "PASS".into(),
                    changed: vec!["src/a.rs".into()],
                    failures: Vec::new(),
                    unverified: Vec::new(),
                    unchecked: false,
                    verified_by: vec!["the human's pin".into()],
                    entry: None,
                    build: Build::default(),
                    run: Some(run),
                })
                .unwrap();
        }
    }
}

/// A rise the law proposes is shown wherever the meters are, with the command that applies it,
/// and applies itself nowhere; only the human's `y` at the terminal writes it — never a body's
/// command, piped input, `--yes`, a harness process or a state directory inside the world.
#[test]
fn a_proposed_rise_is_applied_by_the_human_at_the_terminal_alone() {
    let f = Fixture::new("rise");
    let out = f.run(&["roster", "adopt", "--yes"], None);
    assert!(out.status.success(), "{}", text(&out));
    f.passes(22);
    let out = f.run(&["meters"], None);
    assert!(
        text(&out)
            .contains("proposed by the compiled law — nothing changes until the human applies it")
            && text(&out).contains("slime-s: rise to named-slime proposed (landRate 1.00 (21/21")
            && text(&out)
                .contains("the human applies it with: mnemorphd roster rise slime-s --world "),
        "{}",
        text(&out)
    );
    let out = f.run(&["meters", "--json"], None);
    assert!(
        text(&out).contains(r#""act":"rise""#)
            && text(&out).contains(r#""command":"mnemorphd roster rise slime-s"#),
        "{}",
        text(&out)
    );
    assert_eq!(f.rows(), 2, "a proposal writes nothing");
    let refusals: [(&[&str], Option<&str>, &str); 3] = [
        (
            &["roster", "rise", "slime-s"],
            Some("slime-s"),
            "slime-s runs this",
        ),
        (&["roster", "rise", "slime-s"], None, "terminal"),
        (
            &["roster", "rise", "slime-s", "--yes"],
            None,
            "there is no --yes",
        ),
    ];
    for (args, body, says) in refusals {
        let out = f.run(args, body);
        assert_eq!(out.status.code(), Some(2), "{}", text(&out));
        assert!(text(&out).contains(says), "{says}: {}", text(&out));
    }
    let out = f.run(&["roster", "fall", "slime-s"], None);
    assert!(
        text(&out).contains("proposes a rise for slime-s, not a fall"),
        "{}",
        text(&out)
    );
    assert_eq!(f.rows(), 2, "nothing was written");
    #[cfg(target_os = "linux")]
    applied_at_the_terminal(&f);
}

/// A state directory inside the world is the body's: no rise is read or written there.
#[test]
fn a_rise_with_a_state_directory_inside_the_world_is_refused() {
    let f = Fixture::new("rise-inside");
    let out = Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(["roster", "rise", "slime-s", "--world"])
        .arg(&f.world)
        .env("XDG_STATE_HOME", f.world.join(".mnemorph/tmp/state"))
        .env_remove("MNEMORPH_BODY")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("lies inside the world"),
        "{}",
        text(&out)
    );
    assert!(!f.world.join(".mnemorph/tmp/state/mnemorph/roster").exists());
}

/// The rise of `a_proposed_rise_is_applied_by_the_human_at_the_terminal_alone` under a harness
/// process, then at a terminal of its own: `n` writes nothing, `y` the row and the card.
#[cfg(target_os = "linux")]
fn applied_at_the_terminal(f: &Fixture) {
    // Under a harness process, whatever its environment says.
    let fake = f.root.join("bin/mnemorph");
    fs::create_dir_all(fake.parent().unwrap()).unwrap();
    fs::copy("/bin/sh", &fake).unwrap();
    let inner = format!(
        "{} roster rise slime-s --world '{}'; true",
        env!("CARGO_BIN_EXE_mnemorphd"),
        f.world.display()
    );
    let out = Command::new(&fake)
        .args(["-c", &format!("/bin/sh -c \"{inner}\"; true")])
        .env("XDG_STATE_HOME", &f.state)
        .env_remove("MNEMORPH_BODY")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(
        text(&out).contains("this runs under mnemorph"),
        "{}",
        text(&out)
    );
    // At the terminal: `n` writes nothing, after the proposal is shown in full.
    let out = f.at_tty(&["roster", "rise", "slime-s"], "n\n");
    assert!(
        text(&out).contains("slime-s (slime): rise to named-slime proposed — proposal ")
            && text(&out).contains("rise: landRate 1.00 >= 0.90 (over 21 reports)")
            && text(&out).contains("nothing was written"),
        "{}",
        text(&out)
    );
    assert_eq!(f.rows(), 2);
    let out = f.at_tty(&["roster", "rise", "slime-s"], "y\n");
    assert!(
        text(&out).contains("slime-s is a named-slime now"),
        "{}",
        text(&out)
    );
    assert_eq!(f.rows(), 3);
    let last = fs::read_to_string(f.witness().unwrap()).unwrap();
    let last = last.lines().last().unwrap().to_owned();
    assert!(
        last.contains(r#""act":"rise""#)
            && last.contains(r#""race":"named-slime""#)
            && last.contains(r#""by":"human""#)
            && last.contains(r#""proposal":""#)
            && last.contains(r#""inputs_sha256":""#),
        "{last}"
    );
    assert!(
        fs::read_to_string(f.world.join(".mnemorph/slime/s/README.md"))
            .unwrap()
            .contains("- **Race:** named-slime\n")
    );
    let out = f.run(&["roster", "list"], None);
    assert!(text(&out).contains("slime-s [trusted]"), "{}", text(&out));
    let out = f.run(&["roster", "replay"], None);
    assert!(
        out.status.success()
            && text(&out).contains("row 3: slime-s rise to named-slime — proposal ")
            && text(&out).contains("1 rise(s) and fall(s) reproduce exactly"),
        "{}",
        text(&out)
    );
}
