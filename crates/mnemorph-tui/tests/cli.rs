//! The `mnemorph` command line: what each form reads as, and the binary's plain-text answers
//! before any terminal is taken.

use std::io::Write as _;
use std::path::PathBuf;
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
    Proc::new(env!("CARGO_BIN_EXE_mnemorph"))
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
/// `no reports` where no fold reads them yet — whether or not a session opens.
#[test]
fn status_shows_each_creatures_meters() {
    let w = empty_world("meters");
    let doc = w.join(".mnemorph/slime/sly/README.md");
    std::fs::create_dir_all(doc.parent().unwrap()).unwrap();
    std::fs::write(&doc, "# sly\n\n- **Race:** slime\n").unwrap();
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
        shown.contains("meters   sly (slime): no reports — landRate, gateRate, holeRate\n"),
        "{shown}"
    );
    let _ = std::fs::remove_dir_all(&w);
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
