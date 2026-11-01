//! The running board's local control channel: `board.sock` beside `board.lock` under
//! `$XDG_STATE_HOME/mnemorph/` (`0700`), a Unix socket made `0600` by the board that holds the
//! lock. Each connection is one line asked and one line answered.
//!
//! A body runs as the human's uid, and a body with the network may connect to a pathname socket,
//! so the uid is not enough: every line opens with the nonce the board wrote at its start to
//! `board.ctl` beside the socket (`0600`), which only the human's own processes read — a body's
//! sandbox reads no state directory. The board compares it in constant time and answers
//! `refused` to anything else, and an asker speaks only to a socket and a nonce its uid owns.
//! No key ever crosses it.
//!
//! - `<nonce> ping` → `ok <pid> <base>`: the board runs, at `base` (`http://127.0.0.1:<port>`);
//! - `<nonce> open` → `opened`, or `unopened <why>`: the board opens the browser itself on a
//!   launcher file holding a fresh key ([`Board::launch`]), so the key is on no command line;
//! - `<nonce> stop` → `stopping <pid>`: the board stops as on Ctrl-C.

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

/// The nonce file's name under the state directory.
pub const NONCE: &str = "board.ctl";

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

/// The nonce the board under `dir` wrote: a private regular file this user owns, 64 hex
/// characters.
///
/// # Errors
///
/// No board wrote one, or it is not this user's alone.
fn nonce(dir: &Path) -> Result<String, String> {
    let path = dir.join(NONCE);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| "no board is running".to_owned())?;
    if !meta.file_type().is_file() || meta.uid() != uid() || meta.mode() & 0o077 != 0 {
        return Err(format!(
            "{} is not this user's private board nonce: refusing to use it",
            path.display()
        ));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = text.trim();
    if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(text.to_owned())
    } else {
        Err(format!("{} holds no nonce", path.display()))
    }
}

/// Ask the board whose socket is under `dir` one line, with its nonce, and read its answer.
///
/// # Errors
///
/// No socket, one that is not a socket this user owns, no nonce, or no answer in time.
pub fn ask(dir: &Path, line: &str) -> Result<String, String> {
    owned_socket(dir)?;
    let nonce = nonce(dir)?;
    ask_raw(dir, &format!("{nonce} {line}"))
}

/// The socket under `dir`, if it is a socket this user owns.
fn owned_socket(dir: &Path) -> Result<PathBuf, String> {
    let path = socket(dir);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| "no board is running".to_owned())?;
    if !meta.file_type().is_socket() || meta.uid() != uid() {
        return Err(format!(
            "{} is not this user's board socket: refusing to speak to it",
            path.display()
        ));
    }
    Ok(path)
}

/// Send `line` to the socket under `dir` as it is, and read the answer: for a test of what the
/// board refuses.
///
/// # Errors
///
/// No socket, one that is not a socket this user owns, or no answer in time.
#[doc(hidden)]
pub fn ask_raw(dir: &Path, line: &str) -> Result<String, String> {
    let path = owned_socket(dir)?;
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

/// The socket a board serves under `dir`, bound `0600`, and the fresh nonce it wrote to
/// `board.ctl` (`0600`). Only the board that holds the lock binds it, so a socket left by a board
/// that died is removed first — a socket only: anything else there is refused, never unlinked.
///
/// # Errors
///
/// Something not a socket is in its place, the nonce cannot be written, or the socket cannot be
/// bound.
pub(crate) fn bind(dir: &Path) -> Result<(tokio::net::UnixListener, String), String> {
    let path = socket(dir);
    match std::fs::symlink_metadata(&path) {
        Ok(m) if m.file_type().is_socket() => {
            let _ = std::fs::remove_file(&path);
        }
        Ok(_) => {
            return Err(format!(
                "{} is not a socket: refusing to remove it",
                path.display()
            ));
        }
        Err(_) => {}
    }
    let nonce = super::secret()?;
    write_nonce(&dir.join(NONCE), &nonce)?;
    let l =
        tokio::net::UnixListener::bind(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((l, nonce))
}

/// Write `nonce` to `path` whole: a fresh `0600` file renamed over it.
fn write_nonce(path: &Path, nonce: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let tmp = path.with_extension("ctl.tmp");
    let _ = std::fs::remove_file(&tmp);
    let at = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(at)?;
    f.write_all(nonce.as_bytes()).map_err(at)?;
    f.sync_all().map_err(at)?;
    std::fs::rename(&tmp, path).map_err(at)
}

/// Remove the socket and the nonce under `dir`: the board stopped.
pub(crate) fn unbind(dir: &Path) {
    let _ = std::fs::remove_file(socket(dir));
    let _ = std::fs::remove_file(dir.join(NONCE));
}

/// Answer the control socket under `dir` until the board stops, each line held to `nonce`; `stop`
/// is told on a `stop`.
pub(crate) async fn serve(
    (listener, nonce): (tokio::net::UnixListener, String),
    dir: PathBuf,
    board: Arc<Board>,
    stop: tokio::sync::watch::Sender<bool>,
) {
    let nonce = Arc::new(nonce);
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
        let nonce = Arc::clone(&nonce);
        let dir = dir.clone();
        tokio::spawn(async move {
            let (read, mut write) = conn.split();
            let mut line = String::new();
            let mut read = tokio::io::BufReader::new(read.take(LINE));
            let got = tokio::time::timeout(WAIT, read.read_line(&mut line)).await;
            if !matches!(got, Ok(Ok(_))) {
                return;
            }
            // The nonce first, compared in constant time: anything else is refused whole.
            let (sent, ask) = line.trim().split_once(' ').unwrap_or(("", ""));
            if !super::same(sent, &nonce) {
                let _ = write.write_all(b"refused\n").await;
                return;
            }
            let answer = match ask {
                "ping" => format!("ok {} {}", std::process::id(), board.base()),
                "open" if !super::built() => {
                    "unopened this mnemorphd has no board built in, so no page would spend the key"
                        .to_owned()
                }
                "open" => match board.launch(&dir).and_then(|f| super::browse(&f)) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn only_a_socket_is_removed_from_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let there = socket(dir.path());
        std::fs::write(&there, "mine").unwrap();
        let e = bind(dir.path()).unwrap_err();
        assert!(e.contains("is not a socket"), "{e}");
        assert_eq!(std::fs::read_to_string(&there).unwrap(), "mine");
        assert!(!dir.path().join(NONCE).exists(), "nothing written");
        // A socket a dead board left is replaced.
        std::fs::remove_file(&there).unwrap();
        drop(std::os::unix::net::UnixListener::bind(&there).unwrap());
        let (_, nonce) = bind(dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join(NONCE)).unwrap(),
            nonce
        );
    }
}
