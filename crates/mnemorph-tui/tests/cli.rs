//! The `mnemorph` command line: what each form reads as, and the binary's plain-text answers
//! before any terminal is taken.

#[path = "../../mnemorphd/tests/support/isolated.rs"]
mod isolated;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Stdio};

use mnemorph_tui::cli::{Command, parse};

fn p(args: &[&str]) -> Result<Command, String> {
    parse(&args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())
}

fn chat(world: Option<&str>, resume: Option<&str>, theme: Option<&str>) -> Command {
    Command::Chat {
        world: world.map(PathBuf::from),
        resume: resume.map(str::to_owned),
        theme: theme.map(str::to_owned),
    }
}

#[test]
fn no_subcommand_and_chat_open_the_app() {
    assert_eq!(p(&[]), Ok(chat(None, None, None)));
    assert_eq!(p(&["chat"]), Ok(chat(None, None, None)));
    assert_eq!(
        p(&["--world", "w", "--theme", "light"]),
        Ok(chat(Some("w"), None, Some("light")))
    );
    assert_eq!(p(&["--resume", "last"]), Ok(chat(None, Some("last"), None)));
    assert_eq!(p(&["resume", "r1-2"]), Ok(chat(None, Some("r1-2"), None)));
    assert_eq!(p(&["--probe"]), Ok(Command::Probe));
    assert!(p(&["resume"]).is_err());
    assert!(p(&["resume", "a", "b"]).is_err());
    assert!(p(&["--theme", "pink"]).is_err());
    assert!(p(&["chat", "more"]).is_err());
}

#[test]
fn run_reads_its_ask_from_the_words_or_stdin() {
    let run = |ask: Option<&str>, world: Option<&str>, json: bool, gate_tty: bool| Command::Run {
        ask: ask.map(str::to_owned),
        world: world.map(PathBuf::from),
        json,
        gate_tty,
        no_law: false,
    };
    assert_eq!(
        p(&["run", "fix", "the", "bug"]),
        Ok(run(Some("fix the bug"), None, false, false))
    );
    assert_eq!(p(&["run"]), Ok(run(None, None, false, false)));
    assert_eq!(p(&["run", "-"]), Ok(run(None, None, false, false)));
    // The bench's command line: the core's path is read and has no use.
    assert_eq!(
        p(&[
            "run",
            "an ask",
            "--world",
            "w",
            "--json",
            "--core",
            "x/mnemorphd"
        ]),
        Ok(run(Some("an ask"), Some("w"), true, false))
    );
    assert_eq!(
        p(&["run", "--gate-tty", "go"]),
        Ok(run(Some("go"), None, false, true))
    );
    // The law switch is the command line's, and only run's.
    assert_eq!(
        p(&["run", "go", "--no-law"]),
        Ok(Command::Run {
            ask: Some("go".into()),
            world: None,
            json: false,
            gate_tty: false,
            no_law: true,
        })
    );
    assert!(p(&["--no-law"]).is_err(), "--no-law goes with run");
    assert!(p(&["status", "--no-law"]).is_err());
    assert!(p(&["--json"]).is_err(), "--json goes with run");
    assert!(p(&["--gate-tty"]).is_err());
    assert!(p(&["run", "x", "--resume", "last"]).is_err());
    assert!(p(&["--core"]).is_err());
}

#[test]
fn status_version_and_help() {
    assert_eq!(
        p(&["status", "--world", "w"]),
        Ok(Command::Status {
            world: Some(PathBuf::from("w"))
        })
    );
    assert!(p(&["status", "more"]).is_err());
    assert_eq!(p(&["version"]), Ok(Command::Version));
    assert_eq!(p(&["help"]), Ok(Command::Help));
    assert_eq!(p(&["run", "x", "-V"]), Ok(Command::Version));
    assert_eq!(p(&["--help", "--bogus"]), Ok(Command::Help));
}

#[test]
fn the_world_s_admin_commands_are_the_core_s() {
    for cmd in [
        "memory",
        "onto",
        "q",
        "runs",
        "incident",
        "housekeep",
        "relief",
    ] {
        let e = p(&[cmd]).unwrap_err();
        assert!(e.contains("mnemorphd"), "{e}");
    }
    assert!(p(&["--bogus"]).is_err());
}

fn bin() -> Proc {
    isolated::bin(env!("CARGO_BIN_EXE_mnemorph"))
}

fn empty_world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&w);
    std::fs::create_dir_all(&w).unwrap();
    w
}

#[test]
fn the_binary_answers_its_version_and_help() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("mnemorph "));
    let out = bin().arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("mnemorph run \"<ask>\""), "{help}");
    let out = bin().arg("frobnicate").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

/// With no arguments the app opens: here, in a world with no provider, the session is refused in
/// plain text before the terminal is taken — the app's path, without a screen to take.
#[test]
fn no_arguments_open_the_app_and_a_refusal_is_plain_text() {
    let w = empty_world("smoke");
    let out = bin()
        .current_dir(&w)
        .env("XDG_CONFIG_HOME", w.join("config"))
        .env("HOME", &w)
        .env_remove("OPENROUTER_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{err}");
    assert!(err.starts_with("mnemorph: "), "{err}");
    assert!(!err.contains('\u{1b}'), "no terminal was taken: {err:?}");
    let _ = std::fs::remove_dir_all(&w);
}

/// `run` with no ask reads stdin: an empty one is nothing to ask; a real one reaches the session's
/// opening (here refused: no provider).
#[test]
fn run_reads_stdin() {
    let w = empty_world("stdin");
    let go = |input: &str| {
        let mut child = bin()
            .args(["run", "--world", w.to_str().unwrap()])
            .env("XDG_CONFIG_HOME", w.join("config"))
            .env("HOME", &w)
            .env_remove("OPENROUTER_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let out = go("  \n");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nothing to ask"));
    let out = go("what is here?\n");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(
        !err.contains("nothing to ask") && err.starts_with("mnemorph: "),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&w);
}

/// `status` shows each living creature's meters, compact, with their count and interval — or
/// `no reports` where its record holds none — whether or not a session opens; a state directory
/// inside the world is no trusted one, and the meters say unavailable.
#[test]
fn status_shows_each_creatures_meters() {
    let w = empty_world("meters");
    let doc = w.join(".mnemorph/slime/sly/README.md");
    std::fs::create_dir_all(doc.parent().unwrap()).unwrap();
    std::fs::write(&doc, "# sly\n\n- **Race:** slime\n").unwrap();
    let outside = PathBuf::from(format!("{}-state", w.display()));
    let status = |state: &Path| {
        let out = bin()
            .args(["status", "--world", w.to_str().unwrap()])
            .env("XDG_CONFIG_HOME", w.join("config"))
            .env("XDG_STATE_HOME", state)
            .env("HOME", &w)
            .env_remove("OPENROUTER_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let shown = status(&outside);
    assert!(
        shown.contains("meters   sly (slime): no reports — landRate, gateRate, holeRate\n"),
        "{shown}"
    );
    let shown = status(&w.join("state"));
    assert!(
        shown.contains(
            "meters   sly (slime): unavailable — the usage-witness journal lies inside the world"
        ),
        "{shown}"
    );
    let _ = std::fs::remove_dir_all(&w);
    let _ = std::fs::remove_dir_all(&outside);
}

/// `status` shows each Mind against its witness: a Mind never adopted does not load, and is said
/// unadopted, with the one hint to adopt it.
#[test]
fn status_shows_each_mind_and_its_standing() {
    let w = empty_world("minds");
    std::fs::create_dir_all(w.join(".mnemorph/minds")).unwrap();
    std::fs::write(
        w.join(".mnemorph/minds/release.md"),
        "# Release\n\nCutting a release by hand.\n",
    )
    .unwrap();
    let out = bin()
        .args(["status", "--world", w.to_str().unwrap()])
        .env("XDG_CONFIG_HOME", w.join("config"))
        .env("XDG_STATE_HOME", w.join("state"))
        .env("HOME", &w)
        .env_remove("OPENROUTER_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap();
    let shown = String::from_utf8_lossy(&out.stdout);
    assert!(
        shown.contains("mind     release — unadopted: adopt to use (not loaded)\n")
            && shown.contains("mind     adopt each unadopted Mind once, at a terminal"),
        "{shown}"
    );
    let _ = std::fs::remove_dir_all(&w);
}

/// Where the harness keeps `world`'s record of `kind` under the state directory `state`.
fn state_file(state: &std::path::Path, world: &std::path::Path, kind: &str) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world.file_name().unwrap().to_string_lossy();
    state
        .join("mnemorph")
        .join(kind)
        .join(format!("{name}-{h:016x}.jsonl"))
}

/// `status` counts the last run's acts per class from the harness's chained record outside the
/// world, and refuses that record once a link of it is edited.
#[test]
fn status_counts_the_last_runs_guts_classes() {
    use mnemorph_server::world::guts::{Class, GutsRow, append_stamped, journal_at};
    use mnemorph_server::world::ledger::{Build, Timestamp};
    let w = empty_world("guts");
    let state = w.join("state");
    // `status` names the world by its real path: macOS's temp directory is behind a link.
    let path = state_file(&state, &w.canonicalize().unwrap(), "guts");
    let j = journal_at(&path);
    for (run, classes) in [
        ("r0", vec![Class::Positive]),
        ("r1", vec![Class::Fail, Class::Repeat]),
        ("r1", vec![Class::RedPin]),
        ("r1", vec![]),
    ] {
        let row = GutsRow {
            ts: Timestamp::now(),
            run: run.into(),
            body: "slime-a".into(),
            tool_use_id: "t".into(),
            tool: "bash".into(),
            classes,
            build: Build::default(),
        };
        append_stamped(&j, row).unwrap();
    }
    let status = || {
        let out = bin()
            .args(["status", "--world", w.to_str().unwrap()])
            .env("XDG_CONFIG_HOME", w.join("config"))
            .env("XDG_STATE_HOME", &state)
            .env("HOME", &w)
            .env_remove("OPENROUTER_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let shown = status();
    assert!(
        shown.contains(
            "guts     last run r1 (slime-a): 3 acts · fail 1 · red-pin 1 · positive 0 · repeat 1 \
             · echo 0 · thrash 0 — signals, never a verdict\n"
        ),
        "{shown}"
    );
    let raw = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, raw.replacen("\"red-pin\"", "\"positive\"", 1)).unwrap();
    let shown = status();
    assert!(
        shown.contains("guts     the classes are not believed: their chain is broken at line 3"),
        "{shown}"
    );
    let _ = std::fs::remove_dir_all(&w);
}

#[cfg(unix)]
/// A home set up as a human's: a global file that autostarts and opens the board and names a
/// provider's key file, a `0700` state directory holding a board's control files and registry,
/// and a browser opener that records what it is asked to open.
fn hostile_home(name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let h = std::env::temp_dir().join(format!("mnemorph-hostile-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&h);
    let state = h.join(".local/state/mnemorph");
    for dir in [".config/mnemorph", ".local/share", ".cache", "run", "bin"] {
        std::fs::create_dir_all(h.join(dir)).unwrap();
    }
    std::fs::create_dir_all(&state).unwrap();
    for dir in [state.as_path(), &h.join("run")] {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let key = h.join("openrouter.key");
    std::fs::write(&key, "sk-or-hostile-not-a-key\n").unwrap();
    std::fs::write(
        h.join(".config/mnemorph/config.toml"),
        format!(
            "[board]\nautostart = true\nopen = true\nport = 1\n\n[providers.openrouter]\n\
             kind = \"openai\"\nbase_url = \"http://127.0.0.1:9\"\nkey_file = \"{}\"\n",
            key.display()
        ),
    )
    .unwrap();
    for file in ["board.ctl", "board.sock", "boards.jsonl", "worlds.jsonl"] {
        std::fs::write(state.join(file), "hostile\n").unwrap();
    }
    let opener = format!(
        "#!/bin/sh\necho \"$0 $*\" >> '{}'\n",
        h.join("opened").display()
    );
    for name in ["xdg-open", "open", "sensible-browser", "x-www-browser"] {
        let p = h.join("bin").join(name);
        std::fs::write(&p, &opener).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    h
}

#[cfg(unix)]
/// Every file under `dir` with its bytes: what a run must leave as it found it.
fn files(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push((p.clone(), std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    out.sort();
    out
}

/// A child spawned isolated keeps nothing of the home it was spawned from: its home and every XDG
/// directory are the isolated home's, and no other variable of the parent's comes through.
#[cfg(unix)]
#[test]
fn an_isolated_child_keeps_nothing_of_the_home_it_was_spawned_from() {
    let out = isolated::bin("env").output().unwrap();
    assert!(out.status.success());
    let home = isolated::home();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let (name, value) = line.split_once('=').unwrap();
        match name {
            "HOME" | "XDG_CONFIG_HOME" | "XDG_STATE_HOME" | "XDG_DATA_HOME" | "XDG_CACHE_HOME"
            | "XDG_RUNTIME_DIR" => assert!(Path::new(value).starts_with(home), "{line}"),
            "PATH" | "LANG" | "LLVM_PROFILE_FILE" | "TMPDIR" => {}
            _ => panic!("the parent's `{name}` came through"),
        }
    }
    let path = String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("PATH=").map(str::to_owned))
        .unwrap();
    for dir in std::env::split_paths(&path) {
        assert!(
            !dir.join("mnemorph").is_file(),
            "an installed harness on the path: {dir:?}"
        );
    }
}

/// A test that spawns the binary passes the same under a human's populated home: the binary reads
/// none of it, opens nothing, and writes nothing there; and the isolation holds there too.
#[cfg(unix)]
#[test]
fn a_binary_spawning_test_passes_the_same_under_a_populated_home() {
    let h = hostile_home("guard");
    let before = files(&h);
    let path = std::env::join_paths(std::iter::once(h.join("bin")).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let out = Proc::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "status_counts_the_last_runs_guts_classes",
            "an_isolated_child_keeps_nothing_of_the_home_it_was_spawned_from",
            "--test-threads",
            "1",
        ])
        .env("HOME", &h)
        .env("XDG_CONFIG_HOME", h.join(".config"))
        .env("XDG_STATE_HOME", h.join(".local/state"))
        .env("XDG_DATA_HOME", h.join(".local/share"))
        .env("XDG_CACHE_HOME", h.join(".cache"))
        .env("XDG_RUNTIME_DIR", h.join("run"))
        .env("PATH", path)
        .output()
        .unwrap();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{said}");
    assert!(said.contains("2 passed"), "both tests ran: {said}");
    assert_eq!(files(&h), before, "the populated home was left as it was");
    let _ = std::fs::remove_dir_all(&h);
}
