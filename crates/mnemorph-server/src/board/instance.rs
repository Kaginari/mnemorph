//! One board per machine: `board.lock` under `$XDG_STATE_HOME/mnemorph/` is held for the server's
//! life, and `board.url` beside it holds the URL with the key waiting now (`0600`, rewritten
//! whenever a key is traded). A second launch finds the lock held and prints that URL.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What claiming the board found.
pub enum Claim {
    /// The board is this process's to run.
    Ours(Instance),
    /// A board runs already, at this URL.
    Running(String),
}

/// This process's hold on the board: the lock, and the URL file it keeps.
pub struct Instance {
    _lock: File,
    url: PathBuf,
}

/// Open `path` for writing, created `0600` where there are modes.
fn private(path: &Path, create_new: bool) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.write(true);
    if create_new {
        o.create_new(true);
    } else {
        o.create(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        o.mode(0o600);
    }
    o.open(path)
}

/// Write `text` to `path` whole: a fresh `0600` file renamed over it.
fn replace(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("url.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut f = private(&tmp, true)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)
}

/// Whether `text` is a URL this board would print: its own scheme and a loopback literal, no
/// control character.
fn plausible(text: &str) -> bool {
    (text.starts_with("http://127.0.0.1:") || text.starts_with("http://[::1]:"))
        && !text.chars().any(char::is_control)
}

/// Claim the board under `dir` (`$XDG_STATE_HOME/mnemorph`), made `0700` — one of this user's
/// with group or other bits is tightened to `0700`.
///
/// # Errors
///
/// The directory or the lock cannot be made, or a running board's URL cannot be read.
pub fn claim(dir: &Path) -> Result<Claim, String> {
    // Made `0700`, or tightened to it — said once, on the board's own output.
    #[cfg(unix)]
    super::browser::state_dir(dir, |said| eprintln!("mnemorphd board: {said}"))?;
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let lock_path = dir.join("board.lock");
    let url = dir.join("board.url");
    let lock = lock_file(&lock_path).map_err(|e| format!("{}: {e}", lock_path.display()))?;
    // Held: the running board may be between its lock and its first URL — or the hold is only
    // `board stop`'s instant probe, or a board just letting go, so the lock is tried again too.
    for _ in 0..50 {
        match lock.try_lock() {
            Ok(()) => {
                // A board that died left its URL: it names no server now — and maybe launchers.
                let _ = std::fs::remove_file(&url);
                #[cfg(unix)]
                {
                    let home = std::env::var_os("HOME").map(PathBuf::from);
                    super::browser::sweep_launchers(dir, home.as_deref());
                }
                return Ok(Claim::Ours(Instance { _lock: lock, url }));
            }
            Err(TryLockError::WouldBlock) => {
                if let Ok(text) = std::fs::read_to_string(&url) {
                    let text = text.trim();
                    if plausible(text) {
                        return Ok(Claim::Running(text.to_owned()));
                    }
                }
            }
            Err(TryLockError::Error(e)) => return Err(format!("{}: {e}", lock_path.display())),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!(
        "a board is running, but its URL cannot be read at {}",
        url.display()
    ))
}

/// `board.lock` opened `0600` as a regular file only: a link or anything else in its place is
/// refused, and so is one swapped in between the look and the open.
fn lock_file(path: &Path) -> std::io::Result<File> {
    let refused = || std::io::Error::other("not a regular file: refusing it as the board's lock");
    match std::fs::symlink_metadata(path) {
        Ok(m) if !m.file_type().is_file() => return Err(refused()),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let f = private(path, false)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let (opened, there) = (f.metadata()?, std::fs::symlink_metadata(path)?);
        if !there.file_type().is_file()
            || (opened.dev(), opened.ino()) != (there.dev(), there.ino())
        {
            return Err(refused());
        }
    }
    Ok(f)
}

/// What a probe of `board.lock` found.
#[derive(Debug, PartialEq, Eq)]
pub enum Lock {
    /// A board holds it.
    Held,
    /// No board holds it: none runs.
    Free,
    /// It cannot be told: not a regular file, or unreadable.
    Unknown(String),
}

/// Whether a board holds the lock under `dir` now: probed for an instant, shared, opened only as
/// a regular file. A board lets go of it last, after its socket and nonce are gone and its chats
/// have ended.
#[must_use]
pub fn held(dir: &Path) -> Lock {
    let path = dir.join("board.lock");
    match mnemorph_loop::world::regular::open(&path) {
        Ok(None) => Lock::Free,
        Ok(Some(f)) => match f.try_lock_shared() {
            Ok(()) => Lock::Free,
            Err(TryLockError::WouldBlock) => Lock::Held,
            Err(TryLockError::Error(e)) => Lock::Unknown(format!("{}: {e}", path.display())),
        },
        Err(e) => Lock::Unknown(format!("{}: {e:?}", path.display())),
    }
}

impl Instance {
    /// Keep `url` for a second launch to print.
    ///
    /// # Errors
    ///
    /// The URL file cannot be written.
    pub fn publish(&self, url: &str) -> Result<(), String> {
        replace(&self.url, url).map_err(|e| format!("{}: {e}", self.url.display()))
    }

    /// A function that keeps each new URL, for [`super::Board::new`].
    pub fn publisher(&self) -> Box<dyn Fn(&str) + Send + Sync> {
        let path = self.url.clone();
        Box::new(move |url| {
            let _ = replace(&path, url);
        })
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.url);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn ours(c: Result<Claim, String>) -> Instance {
        match c {
            Ok(Claim::Ours(i)) => i,
            Ok(Claim::Running(url)) => panic!("running at {url}"),
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn the_probe_says_free_held_or_unknown_never_free_for_what_it_cannot_read() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(held(dir.path()), Lock::Free, "no lock file: no board");
        let board = ours(claim(dir.path()));
        assert_eq!(held(dir.path()), Lock::Held);
        drop(board);
        assert_eq!(held(dir.path()), Lock::Free);

        // A link in the lock's place: not told free, whatever it points at.
        let other = tempfile::tempdir().unwrap();
        let held_there = ours(claim(other.path()));
        std::fs::remove_file(dir.path().join("board.lock")).unwrap();
        std::os::unix::fs::symlink(
            other.path().join("board.lock"),
            dir.path().join("board.lock"),
        )
        .unwrap();
        assert!(
            matches!(held(dir.path()), Lock::Unknown(_)),
            "{:?}",
            held(dir.path())
        );
        drop(held_there);
        assert!(matches!(held(dir.path()), Lock::Unknown(_)));
    }

    #[test]
    fn a_board_never_takes_a_linked_lock() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        std::fs::write(&target, b"").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("board.lock")).unwrap();
        let Err(e) = claim(dir.path()) else {
            panic!("a linked lock was taken")
        };
        assert!(e.contains("not a regular file"), "{e}");
    }

    #[test]
    fn a_board_starting_during_a_stops_probe_still_starts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("board.lock"), b"").unwrap();
        // `board stop`'s probe, held far longer than its instant.
        let probe = File::open(dir.path().join("board.lock")).unwrap();
        probe.lock_shared().unwrap();
        let let_go = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(probe);
        });
        let board = ours(claim(dir.path()));
        let_go.join().unwrap();
        assert_eq!(held(dir.path()), Lock::Held);
        drop(board);
    }
}
