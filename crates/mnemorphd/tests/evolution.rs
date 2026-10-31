//! `mnemorphd meters` shows each creature's meters with their count and interval, or `no reports`;
//! `mnemorphd evolution` is the human's door to an evolution: a swap is asked for at the terminal,
//! and `judge` runs the controls a swap waits on, then proposes keep or rollback.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use mnemorph_server::world::evolution::{
    Arm, Facts, Plan, Staged, Stamp, journal_at, restore_before, stamp_for, swap_at,
};

const MIND: &str = ".mnemorph/minds/careful/README.md";

/// Where the harness keeps a world's evolution ledger: `<state>/mnemorph/evolution/<name>-<fnv64a>.jsonl`.
fn ledger(state: &Path, world: &Path) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world.file_name().unwrap().to_string_lossy();
    state
        .join("mnemorph/evolution")
        .join(format!("{name}-{h:016x}.jsonl"))
}

struct Fixture {
    root: PathBuf,
    world: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorphd-evolution-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("world");
        for (path, text) in [
            (MIND, "# Careful\n\nRead before writing.\n"),
            (
                ".mnemorph/slime/sly/README.md",
                "# sly\n\n- **Race:** slime\n",
            ),
            (
                ".mnemorph/dark-elf/glim/README.md",
                "# glim\n\n- **Race:** dark-elf\n",
            ),
        ] {
            fs::create_dir_all(world.join(path).parent().unwrap()).unwrap();
            fs::write(world.join(path), text).unwrap();
        }
        let world = world.canonicalize().unwrap();
        Self { root, world }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn ledger(&self) -> PathBuf {
        ledger(&self.state(), &self.world)
    }

    fn run(&self, args: &[&str]) -> Output {
        // In a session of its own, on Linux: no controlling terminal, as a body's command or a
        // script has none — and never the terminal of whoever runs the tests.
        let mut cmd = if cfg!(target_os = "linux") {
            let mut c = Command::new("setsid");
            c.args(["-w", env!("CARGO_BIN_EXE_mnemorphd")]);
            c
        } else {
            Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        };
        cmd.args(args)
            .args(["--world", self.world.to_str().unwrap()])
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("HOME", &self.root)
            .env_remove("MNEMORPH_BODY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("OPENROUTER_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// `args` run at a terminal (util-linux `script`), `input` typed once the question is up.
    #[cfg(target_os = "linux")]
    fn at_tty(&self, args: &[&str], input: &str) -> Output {
        use std::io::Write as _;
        let quoted: Vec<String> = args
            .iter()
            .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
            .collect();
        let line = format!(
            "{} {} --world '{}'",
            env!("CARGO_BIN_EXE_mnemorphd"),
            quoted.join(" "),
            self.world.display()
        );
        let mut child = Command::new("script")
            .args(["-qec", &line, "/dev/null"])
            .env("XDG_STATE_HOME", self.state())
            .env_remove("MNEMORPH_BODY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = stdin.write_all(input.as_bytes());
        drop(stdin);
        child.wait_with_output().unwrap()
    }

    /// The swap and its full window, every task passing; with `controls`, each task's control
    /// failing, recorded on a copy of the world put back to the before state.
    fn swapped(&self, controls: bool) {
        let path = self.ledger();
        let s = swap_at(
            &self.world,
            &journal_at(&path),
            &Plan {
                id: "careful-1".into(),
                creature: "sly".into(),
                bodies: vec!["sly".into()],
                arm: Arm::Mutate,
                files: vec![Staged {
                    path: MIND.into(),
                    after: Some(b"# Careful\n\nWrite once.\n".to_vec()),
                    creates: false,
                }],
                proven_on: Vec::new(),
            },
        )
        .unwrap();
        let pass = Facts {
            gate: Some(mnemorph_server::world::evolution::Gate::Pass),
            first_try: true,
            holes_found: 0,
        };
        let copy = self.root.join("copy");
        fs::create_dir_all(&copy).unwrap();
        restore_before(&path, &s, &copy).unwrap();
        for i in 0..10 {
            let ask = format!("task {i}");
            stamp_for(&self.world, &path, "sly", &ask)
                .unwrap()
                .record(&self.world, pass)
                .unwrap();
            if controls {
                Stamp {
                    ledger: path.clone(),
                    swap: s.clone(),
                    body: "sly".into(),
                    ask,
                    state: s.before_state(),
                    control: true,
                }
                .record(
                    &copy,
                    Facts {
                        gate: Some(mnemorph_server::world::evolution::Gate::Fail),
                        ..Facts::default()
                    },
                )
                .unwrap();
            }
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text(o: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn meters_show_by_race_and_say_no_reports_where_no_fold_reads() {
    let f = Fixture::new("meters");
    let out = f.run(&["meters"]);
    let (stdout, err) = text(&out);
    assert!(out.status.success(), "{err}");
    assert!(
        stdout.contains("slime\n  sly (slime): no reports — landRate, gateRate, holeRate\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "dark-elf\n  glim (dark-elf): precision no reports · upheldRate no reports · \
             cannotVerifyRate no reports\n"
        ),
        "{stdout}"
    );
    let out = f.run(&["meters", "--json"]);
    let (stdout, _) = text(&out);
    let sly = stdout
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["creature"] == "sly")
        .unwrap();
    assert_eq!(sly["meters"][0]["meter"], "landRate");
    assert!(
        sly["meters"][0]["n"].is_null(),
        "no fold: no count, never a zero"
    );
    let glim = stdout
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["creature"] == "glim")
        .unwrap();
    assert!(
        glim["meters"][0]["n"].is_null(),
        "a live meter with no report: no count"
    );
}

#[test]
fn a_swap_is_asked_at_the_terminal_and_never_a_bodys() {
    let f = Fixture::new("swap");
    let staged = f.world.join(".mnemorph/tmp/evolution/careful-1").join(MIND);
    fs::create_dir_all(staged.parent().unwrap()).unwrap();
    fs::write(&staged, "# Careful\n\nWrite once.\n").unwrap();
    let before = fs::read(f.world.join(MIND)).unwrap();
    // The staging must hold exactly the file set named: an unnamed staged file is extra.
    let out = f.run(&["evolution", "swap", "careful-1", "--creature", "sly"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).1.contains("an extra file"), "{}", text(&out).1);
    let out = f.run(&[
        "evolution",
        "swap",
        "careful-1",
        "--creature",
        "sly",
        "--file",
        MIND,
    ]);
    let (stdout, err) = text(&out);
    assert_eq!(out.status.code(), Some(2), "{stdout}{err}");
    assert!(stdout.contains(MIND), "the plan is shown first: {stdout}");
    assert!(err.contains("refused"), "{err}");
    assert_eq!(fs::read(f.world.join(MIND)).unwrap(), before);
    assert!(!f.ledger().exists(), "nothing swapped, nothing written");
    // At the terminal, `y` swaps it in: the Mind rewritten, one swap row naming it.
    #[cfg(target_os = "linux")]
    swapped_at_tty(&f);
    let out = Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(["evolution", "judge", "careful-1"])
        .env("MNEMORPH_BODY", "slime-a")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out).1.contains("never a body's"));
}

#[cfg(target_os = "linux")]
fn swapped_at_tty(f: &Fixture) {
    let out = f.at_tty(
        &[
            "evolution",
            "swap",
            "careful-1",
            "--creature",
            "sly",
            "--file",
            MIND,
        ],
        "y\n",
    );
    let (stdout, err) = text(&out);
    assert!(
        stdout.contains("swapped careful-1 under build"),
        "{stdout}{err}"
    );
    assert_eq!(
        fs::read_to_string(f.world.join(MIND)).unwrap(),
        "# Careful\n\nWrite once.\n"
    );
    let rows = fs::read_to_string(f.ledger()).unwrap();
    assert_eq!(rows.lines().count(), 1, "{rows}");
    assert!(
        rows.contains(r#""row":"swap""#) && rows.contains(MIND),
        "{rows}"
    );
}

#[test]
fn judge_runs_what_waits_then_proposes_by_the_rule() {
    let f = Fixture::new("judge");
    let out = f.run(&["evolution", "judge", "careful-1"]);
    assert_eq!(out.status.code(), Some(1), "{:?}", text(&out));
    assert!(text(&out).0.starts_with("refused: "), "{:?}", text(&out));
    // Controls wait, and no model is configured: judging asks for one, and writes nothing.
    f.swapped(false);
    let rows = fs::read_to_string(f.ledger()).unwrap().lines().count();
    let out = f.run(&["evolution", "judge", "careful-1"]);
    assert_eq!(out.status.code(), Some(2), "{:?}", text(&out));
    assert_eq!(
        fs::read_to_string(f.ledger()).unwrap().lines().count(),
        rows
    );
    // Every control on record: no model is reached, and the rule proposes.
    let g = Fixture::new("judge-whole");
    g.swapped(true);
    let out = g.run(&["evolution", "judge", "careful-1"]);
    let (stdout, err) = text(&out);
    assert!(out.status.success(), "{stdout}{err}");
    assert!(
        stdout.contains("keep: netWins 10 >= 2 (over 10 runs beside do-nothing); ledger head "),
        "{stdout}"
    );
    assert!(
        stdout.contains("proposed to the human: keep — nothing changes until the human applies it"),
        "{stdout}"
    );
}
