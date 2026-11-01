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
    let lock = private(&lock_path, false).map_err(|e| format!("{}: {e}", lock_path.display()))?;
    match lock.try_lock() {
        Ok(()) => {
            // A board that died left its URL: it names no server now — and maybe launchers.
            let _ = std::fs::remove_file(&url);
            #[cfg(unix)]
            {
                let home = std::env::var_os("HOME").map(PathBuf::from);
                super::browser::sweep_launchers(dir, home.as_deref());
            }
            Ok(Claim::Ours(Instance { _lock: lock, url }))
        }
        Err(TryLockError::WouldBlock) => {
            // The running board may be between its lock and its first URL.
            for _ in 0..50 {
                if let Ok(text) = std::fs::read_to_string(&url) {
                    let text = text.trim();
                    if plausible(text) {
                        return Ok(Claim::Running(text.to_owned()));
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(format!(
                "a board is running, but its URL cannot be read at {}",
                url.display()
            ))
        }
        Err(TryLockError::Error(e)) => Err(format!("{}: {e}", lock_path.display())),
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
