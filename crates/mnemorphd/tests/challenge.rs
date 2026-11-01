//! `mnemorphd challenge …` is the human's door to the Dark Elf challenges, kept under the human's
//! state directory (a temporary `XDG_STATE_HOME` here): listing what holds a landing, showing one,
//! and waiving one with a reason — never a body's, never without a reason; the Dark Elf's score
//! folded from them, and the human's decision on one finding.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use mnemorph_server::world::Roster;
use mnemorph_server::world::challenge::{Challenges, Decision, Found, Opening, Status, route};

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
            ..Default::default()
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
        // In a session of its own, on Linux: no controlling terminal, as a body's command or a
        // script has none — and never the terminal of whoever runs the tests.
        let mut cmd = if cfg!(target_os = "linux") {
            let mut c = Command::new("setsid");
            c.args(["-w", env!("CARGO_BIN_EXE_mnemorphd")]);
            c
        } else {
            Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        };
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

impl Fixture {
    /// Run at a terminal of its own (a pseudo-terminal `script` makes), `input` typed at it.
    #[cfg(target_os = "linux")]
    fn at_tty(&self, args: &[&str], input: &str) -> Output {
        use std::io::Write as _;
        let quoted: Vec<String> = args
            .iter()
            .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
            .collect();
        let line = format!(
            "{} challenge {} --world '{}'",
            env!("CARGO_BIN_EXE_mnemorphd"),
            quoted.join(" "),
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

#[cfg(target_os = "linux")]
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
    // Piped, the answer is no human's: refused, nothing waived.
    let out = f.run(&["waive", "run-1:2", "--why", WHY], "y\n", None);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("answered at a terminal"),
        "{}",
        text(&out)
    );
    assert!(matches!(f.status(), Some(Status::Pending { .. })));
    // At the terminal: `n` waives nothing, `y` waives it with its reason.
    let out = f.at_tty(&["waive", "run-1:2", "--why", WHY], "n\n");
    assert!(
        matches!(f.status(), Some(Status::Pending { .. })),
        "{}",
        text(&out)
    );
    let out = f.at_tty(&["waive", "run-1:2", "--why", WHY], "y\n");
    assert!(
        text(&out).contains(&format!("waived run-1:2: {WHY}")),
        "{}",
        text(&out)
    );
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
    let out = f.at_tty(&["waive", "run-1:2", "--why", WHY], "y\n");
    assert!(
        text(&out).contains("waived already"),
        "waived once: {}",
        text(&out)
    );
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
        (vec!["frob"], None, "score or decide"),
    ] {
        let out = f.run(&args, "y\n", body);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out));
        assert!(text(&out).contains(says), "{args:?}: {}", text(&out));
    }
    assert!(matches!(f.status(), Some(Status::Pending { .. })));
}

/// A written path a model chose — a newline, an escape sequence, a backtick — is shown escaped,
/// never as the terminal would act on it.
#[test]
fn a_hostile_path_is_shown_escaped() {
    let f = Fixture::new("hostile");
    let hostile = "crates/mnemorph-tools/a`\n\u{1b}[31mred.rs";
    let landing = route(&Roster::default(), "run-1:3", &[hostile.to_owned()]);
    assert_ne!(landing.routed, []);
    Challenges::at(&records(&f.state, &f.world))
        .open_at(
            &landing,
            &Opening {
                tree: "cd".repeat(32),
                by: "orc-o\u{1b}]0;x\u{7}".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
    for args in [&["show", "run-1:3"][..], &["list"]] {
        let out = f.run(args, "", None);
        let shown = text(&out);
        assert!(out.status.success(), "{shown}");
        assert!(
            !shown.contains('\u{1b}') && !shown.contains('\u{7}'),
            "{shown:?}"
        );
        assert!(!shown.contains("a`"), "{shown:?}");
    }
    let out = f.run(&["show", "run-1:3"], "", None);
    assert!(
        text(&out).contains("a\\u{60}\\u{a}\\u{1b}[31mred.rs"),
        "{}",
        text(&out)
    );
}

/// A harness finding on `symbol`, kept on `run-1:2`: its claim carries what a terminal would act
/// on. Returns its fingerprint.
fn finding(f: &Fixture, symbol: &str) -> String {
    let fp = format!(
        "{:016x}",
        symbol.len() as u64 * 7919 + u64::from(symbol.as_bytes()[0])
    );
    let found: Found = serde_json::from_value(serde_json::json!({
        "finder": "dark-elf-harness",
        "rec": "",
        "record": {
            "verdict": "finding",
            "fingerprint": fp,
            "speciality": "dark-elf-harness",
            "class": "path-traversal",
            "file": "crates/mnemorph-tools/src/files.rs",
            "symbol": symbol,
            "claim": "read_path escapes\n\u{1b}[2J `the world`",
            "severity": "high",
            "impact": "disclosure",
            "impact_reason": "a file outside the world is read",
            "fix": "refuse ..",
            "preconditions": ["an untrusted repository"],
            "reachability": "a repository file names the path"
        }
    }))
    .unwrap();
    Challenges::at(&records(&f.state, &f.world))
        .found("run-1:2", found)
        .unwrap();
    fp
}

/// `score` folds the records per speciality, escaped, and says nothing is on record when so.
#[test]
fn the_score_folds_each_speciality() {
    let f = Fixture::new("score");
    let out = f.run(&["score"], "", None);
    assert_eq!(text(&out), "no Dark Elf finding is on record\n");
    finding(&f, "read_path");
    let out = f.run(&["score"], "", None);
    assert!(out.status.success(), "{}", text(&out));
    let shown = text(&out);
    assert!(
        shown.contains(
            "dark-elf-harness (no living body): 1 findings — 0 upheld, 0 refuted, 0 \
             cannot-verify, 0 held for the human, 1 unverified"
        ),
        "{shown}"
    );
    assert!(
        shown.contains(
            "  precision no reports · upheld 0.00 (0/1, 95% 0.00–0.79) (unranked)\n  last 0 \
             settled: precision no reports · last 1 filed: cannot-verify 0.00 (0/1, 95% \
             0.00–0.79)\n"
        ),
        "{shown}"
    );
    // The finding was settled under the build `--version` prints.
    let version = Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        .arg("--version")
        .output()
        .unwrap();
    let build = text(&version).trim().replace("mnemorphd ", "");
    assert!(
        shown.contains(&format!("  settled under: {build} 1\n")),
        "{shown}"
    );
    assert!(!shown.contains("proposed"), "{shown}");
    let out = f.run(&["score"], "", Some("dark-elf-harness"));
    assert_eq!(out.status.code(), Some(2), "a body never reads it here");
}

/// The human's decision is asked at the terminal, after the finding is shown escaped; a
/// false positive counts refuted. Piped, a bad word, no reason or no finding: refused.
#[cfg(target_os = "linux")]
#[test]
fn the_human_decides_a_finding_at_the_terminal() {
    let f = Fixture::new("decide");
    let fp = finding(&f, "read_path");
    for (args, says) in [
        (
            vec!["decide", "run-1:2", &fp, "maybe", "--why", WHY],
            "`upheld` or `false-positive`",
        ),
        (
            vec!["decide", "run-1:2", &fp, "upheld"],
            "a decision says why",
        ),
        (
            vec!["decide", "run-1:2", &fp, "upheld", "--why", "n/a"],
            "at least 20 characters",
        ),
        (
            vec!["decide", "run-1:2", "nope", "upheld", "--why", WHY],
            "no finding #nope",
        ),
        (
            vec!["decide", "run-1:2", &fp, "upheld", "--why", WHY],
            "answered at a terminal",
        ),
    ] {
        let out = f.run(&args, "y\n", None);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out));
        assert!(text(&out).contains(says), "{args:?}: {}", text(&out));
    }
    let out = f.at_tty(
        &["decide", "run-1:2", &fp, "false-positive", "--why", WHY],
        "n\n",
    );
    assert!(
        text(&out).contains("nothing was recorded"),
        "{}",
        text(&out)
    );
    let out = f.at_tty(
        &["decide", "run-1:2", &fp, "false-positive", "--why", WHY],
        "y\n",
    );
    let shown = text(&out);
    assert!(
        shown.contains(&format!("decided #{fp}: false-positive")),
        "{shown}"
    );
    assert!(
        !shown.contains('\u{1b}'),
        "the claim is shown escaped: {shown:?}"
    );
    assert!(shown.contains("no verdict yet"), "{shown}");
    let out = f.run(&["score"], "", None);
    assert!(
        text(&out).contains("1 findings — 0 upheld, 1 refuted"),
        "{}",
        text(&out)
    );
    assert!(
        text(&out).contains("precision 0.00 (0/1, 95% 0.00–0.79)"),
        "{}",
        text(&out)
    );
}

/// A row the fold cannot read is said, by `list` and by `score`, and holds the landing pending.
#[test]
fn an_unreadable_row_is_said() {
    let f = Fixture::new("unreadable");
    let file = records(&f.state, &f.world);
    let before = fs::read_to_string(&file).unwrap();
    fs::write(&file, format!("{before}{{\"row\":\"found\",\"landing\":\n")).unwrap();
    let n = before.lines().count() + 1;
    let out = f.run(&["list"], "", None);
    assert!(
        text(&out).contains(&format!("unreadable row {n}")),
        "{}",
        text(&out)
    );
    let out = f.run(&["score"], "", None);
    assert!(
        text(&out).starts_with(&format!("the records hold unreadable rows ({n})")),
        "{}",
        text(&out)
    );
    assert!(matches!(f.status(), Some(Status::Pending { .. })));
}

/// A living Dark Elf at the bars is proposed its rise — to the human, applied by nothing here.
#[test]
fn a_rise_is_proposed_to_the_human() {
    let f = Fixture::new("rise");
    let seat = f.world.join(".mnemorph/dark-elf/dark-elf-harness");
    fs::create_dir_all(&seat).unwrap();
    fs::write(
        seat.join("README.md"),
        "# dark-elf-harness\n\n- **Race:** dark-elf\n- **Reports to:** rimuru\n",
    )
    .unwrap();
    // The human adopts the card: the race a rise reads is the witness's.
    let creatures = mnemorph_server::world::creatures::adoption_in(Some(&f.state), &f.world);
    mnemorph_server::world::creatures::record_in(Some(&f.state), &f.world, creatures).unwrap();
    let c = Challenges::at(&records(&f.state, &f.world));
    let upheld = Decision::human(true, WHY).unwrap();
    for i in 0..20 {
        let fp = finding(&f, &format!("{}sym{i}", char::from(b'a' + i)));
        c.decide("run-1:2", &fp, &upheld).unwrap();
    }
    let out = f.run(&["score"], "", None);
    let shown = text(&out);
    assert!(
        shown.contains("dark-elf-harness (dark-elf): 20 findings — 20 upheld"),
        "{shown}"
    );
    assert!(
        shown.contains(
            "proposed to the human: rise to Arch Dark Elf — nothing changes until the human \
             applies it"
        ),
        "{shown}"
    );
    assert!(
        shown.contains("upheld 1.00 (20/20, 95% 0.84–1.00) (ok)"),
        "{shown}"
    );
    // Showing only: precision's lower bound, 0.84, is under its 85 bar, and the rise stands.
    assert!(
        shown.contains("precision 1.00 (20/20, 95% 0.84–1.00)")
            && shown.contains("last 10 settled: precision 1.00 (10/10, 95% 0.72–1.00)"),
        "{shown}"
    );
    // Each bar of the rise, explained, with the records' chain head: what recomputes it.
    let head = c.chain().unwrap().head.unwrap().digest;
    for bar in [
        "rise: findings 20 >= 20 (over 20 findings)",
        "rise: upheldRate 1.00 >= 0.80 (over 20 findings)",
        "rise: precision 1.00 >= 0.85 (over 20 findings)",
    ] {
        assert!(
            shown.contains(&format!("    {bar}; ledger head {head}, rows ")),
            "{shown}"
        );
    }
    assert!(!shown.contains("chain does not hold"), "{shown}");
}
