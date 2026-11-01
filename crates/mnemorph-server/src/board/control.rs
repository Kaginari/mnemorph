//! The running board's local control channel: `board.sock` beside `board.lock` under
//! `$XDG_STATE_HOME/mnemorph/` (`0700`), a Unix socket made `0600` by the board that holds the
//! lock. Each connection is one line asked and one line answered, and both ends hold the other to
//! the same user: the board refuses a peer of another uid, and an asker refuses a socket another
//! uid owns. No key ever crosses it.
//!
//! - `ping` → `ok <pid> <base>`: the board runs, at `base` (`http://127.0.0.1:<port>`);
//! - `open` → `opened`, or `unopened <why>`: the board opens the browser itself on a fresh key of
//!   its own, valid once and for a minute, so the key never leaves its process but to the opener;
//! - `stop` → `stopping <pid>`: the board stops as on Ctrl-C.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _};

use super::Board;

/// The socket's name under the state directory.
pub const SOCKET: &str = "board.sock";

/// The longest line read either way.
const LINE: u64 = 512;

/// How long an asker waits for the board's answer.
const WAIT: Duration = Duration::from_secs(5);

/// A board that answered `ping`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    /// Its process id.
    pub pid: u32,
    /// Where it answers, without a key: `http://127.0.0.1:<port>`.
    pub base: String,
}

/// This process's effective uid.
fn uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// The socket under `dir`.
#[must_use]
pub fn socket(dir: &Path) -> PathBuf {
    dir.join(SOCKET)
}

/// Ask the board whose socket is under `dir` one line, and read its answer.
///
/// # Errors
///
/// No socket, one that is not a socket this user owns, or no answer in time.
pub fn ask(dir: &Path, line: &str) -> Result<String, String> {
    let path = socket(dir);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| "no board is running".to_owned())?;
    if !meta.file_type().is_socket() || meta.uid() != uid() {
        return Err(format!(
            "{} is not this user's board socket: refusing to speak to it",
            path.display()
        ));
    }
    let mut s = UnixStream::connect(&path).map_err(|_| "no board is running".to_owned())?;
    s.set_read_timeout(Some(WAIT)).map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(WAIT)).map_err(|e| e.to_string())?;
    s.write_all(format!("{line}\n").as_bytes())
        .map_err(|e| format!("the board's socket: {e}"))?;
    let mut answer = String::new();
    BufReader::new(std::io::Read::take(&s, LINE))
        .read_line(&mut answer)
        .map_err(|e| format!("the board's socket: {e}"))?;
    let answer = answer.trim();
    if answer.is_empty() {
        return Err("the board did not answer".to_owned());
    }
    Ok(answer.to_owned())
}

/// Whether a board runs under `dir`, and where.
///
/// # Errors
///
/// None runs, or it answers what no board would.
pub fn ping(dir: &Path) -> Result<Running, String> {
    let answer = ask(dir, "ping")?;
    let mut parts = answer.splitn(3, ' ');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("ok"), Some(pid), Some(base))
            if base.starts_with("http://127.0.0.1:") || base.starts_with("http://[::1]:") =>
        {
            Ok(Running {
                pid: pid
                    .parse()
                    .map_err(|_| format!("a strange answer: {answer}"))?,
                base: base.to_owned(),
            })
        }
        _ => Err(format!("a strange answer: {answer}")),
    }
}

/// Ask the board under `dir` to open the browser on a fresh one-time link.
///
/// # Errors
///
/// None runs, or it could not open the browser.
pub fn open(dir: &Path) -> Result<(), String> {
    let answer = ask(dir, "open")?;
    match answer.split_once(' ') {
        _ if answer == "opened" => Ok(()),
        Some(("unopened", why)) => Err(why.to_owned()),
        _ => Err(format!("a strange answer: {answer}")),
    }
}

/// Ask the board under `dir` to stop; its pid.
///
/// # Errors
///
/// None runs.
pub fn stop(dir: &Path) -> Result<u32, String> {
    let answer = ask(dir, "stop")?;
    answer
        .strip_prefix("stopping ")
        .and_then(|p| p.parse().ok())
        .ok_or_else(|| format!("a strange answer: {answer}"))
}

/// The socket a board serves under `dir`, bound `0600`. Only the board that holds the lock binds
/// it, so a socket left by a board that died is removed first.
///
/// # Errors
///
/// The socket cannot be bound.
pub(crate) fn bind(dir: &Path) -> Result<tokio::net::UnixListener, String> {
    let path = socket(dir);
    let _ = std::fs::remove_file(&path);
    let l =
        tokio::net::UnixListener::bind(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(l)
}

/// Answer the control socket until the board stops; `stop` is told on a `stop`.
pub(crate) async fn serve(
    listener: tokio::net::UnixListener,
    board: Arc<Board>,
    stop: tokio::sync::watch::Sender<bool>,
) {
    loop {
        let Ok((mut conn, _)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        if conn.peer_cred().map(|c| c.uid()).ok() != Some(uid()) {
            continue;
        }
        let board = Arc::clone(&board);
        let stop = stop.clone();
        tokio::spawn(async move {
            let (read, mut write) = conn.split();
            let mut line = String::new();
            let mut read = tokio::io::BufReader::new(read.take(LINE));
            let got = tokio::time::timeout(WAIT, read.read_line(&mut line)).await;
            if !matches!(got, Ok(Ok(_))) {
                return;
            }
            let answer = match line.trim() {
                "ping" => format!("ok {} {}", std::process::id(), board.base()),
                "open" if !super::built() => {
                    "unopened this mnemorphd has no board built in, so no page would spend the key"
                        .to_owned()
                }
                "open" => match board.open_url().and_then(|u| super::browse(&u)) {
                    Ok(()) => "opened".to_owned(),
                    Err(e) => format!("unopened {}", e.replace('\n', " ")),
                },
                "stop" => {
                    let _ = stop.send(true);
                    format!("stopping {}", std::process::id())
                }
                _ => "unknown".to_owned(),
            };
            let _ = write.write_all(format!("{answer}\n").as_bytes()).await;
        });
    }
}
