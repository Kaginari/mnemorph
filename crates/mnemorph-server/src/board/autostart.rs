//! The board started with the terminal app (`board.autostart`, the human's global file alone):
//! make sure one board runs — the running one reused, never a second —, list the world on it, and
//! with `board.open` have the board open the browser on a fresh one-time link. The answer is one
//! status line that never holds a key; every failure is that line, never a panic or a hang.
//!
//! A board this starts is `mnemorphd board` run with `--background`, the binary beside the
//! running one —
//! never one found on `PATH` or in the world —, in its own process group, so it outlives the app
//! and a Ctrl-C at the terminal never reaches it. Its output goes to
//! `$XDG_STATE_HOME/mnemorph/board.log`, which never holds a key either. It inherits the app's
//! environment, the human's own shell, as a board they start by hand does. `mnemorphd board stop`
//! stops it.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::io::{BufRead as _, BufReader};
use std::os::unix::fs::OpenOptionsExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::control;
use crate::world::dimension;

/// The log of a board this starts, under the state directory.
pub const LOG: &str = "board.log";

/// The largest log kept: past it, the next start begins it again.
const LOG_CAP: u64 = 1 << 20;

/// What the terminal app asks for, from the human's global file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ask {
    /// Open the browser on a fresh one-time link.
    pub open: bool,
    /// The port a board started here listens on; `None` for the default.
    pub port: Option<u16>,
}

/// `mnemorphd` beside the running executable.
///
/// # Errors
///
/// The running executable cannot be found.
pub fn sibling() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("this executable: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "this executable has no directory".to_owned())?;
    Ok(dir.join("mnemorphd"))
}

/// One line for the terminal: printable, short.
fn line(text: &str) -> String {
    let t: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = t.trim();
    match t.char_indices().nth(200) {
        Some((at, _)) => format!("{}…", &t[..at]),
        None => t.to_owned(),
    }
}

/// The status line of a board running at `base`.
fn running(base: &str, opened: Result<bool, String>) -> String {
    match opened {
        Ok(true) => format!("board: {base} (opened in your browser)"),
        Ok(false) => format!("board: {base} (run `mnemorphd board` for a sign-in link)"),
        Err(e) => format!("board: {base} ({}; {})", super::UNOPENED, line(&e)),
    }
}

/// The last line `log` holds, if any.
fn last_line(log: &Path) -> Option<String> {
    let f = std::fs::File::open(log).ok()?;
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .last()
        .map(|l| line(l.trim_start_matches("mnemorphd board: ")))
}

/// Make sure a board runs, with `world` listed on it, and say how in one line: `mnemorphd` is the
/// binary a board is started from, and `wait` how long a new one may take to answer.
#[must_use]
pub fn ensure(mnemorphd: &Path, world: &Path, ask: Ask, wait: Duration) -> String {
    let Some(dir) = dimension::registry()
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
    else {
        return "board: not started: no state directory (set XDG_STATE_HOME or HOME)".to_owned();
    };
    ensure_in(mnemorphd, &dir, world, ask, wait, &[])
}

/// As [`ensure`], under the state directory `dir` (`$XDG_STATE_HOME/mnemorph`), a board started
/// with `env` added to its environment: for a test, which never touches the human's.
#[must_use]
pub fn ensure_in(
    mnemorphd: &Path,
    dir: &Path,
    world: &Path,
    ask: Ask,
    wait: Duration,
    env: &[(&str, &OsStr)],
) -> String {
    let registry = dir.join("worlds.jsonl");
    let hand = dimension::hand_file(&registry);
    let world = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    let listed = dimension::listed(&registry, &hand)
        .iter()
        .any(|l| l.world == world);
    let mut note = String::new();
    // The state directory is the human's alone: a loose one is tightened before anything in it
    // is trusted, and that said once on this line.
    if let Err(e) = super::browser::state_dir(dir, |said| {
        note = format!("; {}", line(said));
    }) {
        return format!("board: not started: {}", line(&e));
    }
    if !listed && let Err(e) = dimension::hand(&hand, &registry, dimension::Hand::Add, &world) {
        let _ = write!(note, "; this world is not on it: {}", line(&e));
    }
    if let Ok(r) = control::ping(dir) {
        let opened = if ask.open {
            control::open(dir).map(|()| true)
        } else {
            Ok(false)
        };
        return running(&r.base, opened) + &note;
    }
    let started = match start(mnemorphd, dir, ask.port, wait, env) {
        Ok(base) => base,
        Err(e) => return e + &note,
    };
    let opened = if ask.open {
        control::open(dir).map(|()| true)
    } else {
        Ok(false)
    };
    running(&started, opened) + &note
}

/// Start a board from `mnemorphd` under the state directory `dir`, on `port`, and wait up to
/// `wait` for it to answer; where it answers, or the status line saying why not.
fn start(
    mnemorphd: &Path,
    dir: &Path,
    port: Option<u16>,
    wait: Duration,
    env: &[(&str, &OsStr)],
) -> Result<String, String> {
    if !mnemorphd.is_file() {
        return Err(format!(
            "board: not started: no mnemorphd beside this mnemorph ({})",
            mnemorphd.display()
        ));
    }
    let log = dir.join(LOG);
    let opened = {
        let big = std::fs::metadata(&log).is_ok_and(|m| m.len() > LOG_CAP);
        std::fs::OpenOptions::new()
            .create(true)
            .append(!big)
            .write(true)
            .truncate(big)
            .mode(0o600)
            .open(&log)
    };
    let out = match opened {
        Ok(f) => f,
        Err(e) => return Err(format!("board: not started: {}: {e}", log.display())),
    };
    let err = match out.try_clone() {
        Ok(f) => f,
        Err(e) => return Err(format!("board: not started: {}: {e}", log.display())),
    };
    let mut cmd = Command::new(mnemorphd);
    cmd.args(["board", "--background"]);
    if let Some(p) = port {
        cmd.args(["--port", &p.to_string()]);
    }
    // The board reads the state directory this one does.
    if let Some(state) = dir.parent() {
        cmd.env("XDG_STATE_HOME", state);
    }
    // The board inherits this app's environment whole: the human's own shell, as when they run
    // `mnemorphd board` themselves — its chats need the provider key it names. Each chat's child
    // it starts gets only the allowlist the bridge passes, never this environment whole.
    cmd.envs(env.iter().copied())
        .env_remove("MNEMORPH_BODY")
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        // Its own process group: a Ctrl-C or a hang-up at this terminal never reaches it.
        .process_group(0);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Err(format!("board: not started: {}: {e}", mnemorphd.display())),
    };
    let deadline = Instant::now() + wait;
    loop {
        if let Ok(r) = control::ping(dir) {
            // Reaped when it stops, off this thread.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return Ok(r.base);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                // A board that raced this one's start answers now, or the start failed.
                if status.success()
                    && let Ok(r) = control::ping(dir)
                {
                    return Ok(r.base);
                }
                let why = last_line(&log).unwrap_or_else(|| format!("it exited ({status})"));
                return Err(format!(
                    "board: not started: {why} (log: {})",
                    log.display()
                ));
            }
            Ok(None) => {}
            Err(e) => return Err(format!("board: not started: {e}")),
        }
        if Instant::now() >= deadline {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return Err(format!(
                "board: starting, not answering yet (log: {})",
                log.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_browser_that_did_not_open_says_how_to_sign_in_without_a_key() {
        let said = running(
            "http://127.0.0.1:7878",
            Err("xdg-open failed (exit status: 3)\nagain".to_owned()),
        );
        assert_eq!(
            said,
            format!(
                "board: http://127.0.0.1:7878 ({}; xdg-open failed (exit status: 3) again)",
                super::super::UNOPENED
            )
        );
        assert!(said.contains("run `mnemorphd board` and paste the link it prints"));
        assert!(!said.contains("#k="), "{said}");
    }

    #[test]
    fn a_loose_state_directory_is_tightened_and_said_on_the_line() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("mnemorph");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o775)).unwrap();
        let world = root.path().join("world");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        let said = ensure_in(
            &root.path().join("nowhere/mnemorphd"),
            &dir,
            &world,
            Ask::default(),
            Duration::from_millis(10),
            &[],
        );
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        assert!(said.contains("was 0775: tightened to 0700"), "{said}");
    }
}
