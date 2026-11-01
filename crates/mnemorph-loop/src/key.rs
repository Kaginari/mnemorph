//! A provider's key: read from the variable its `key_env` names when that is set, else from the
//! file its `key_file` names. The key itself never crosses into a log, a hole, an error, an
//! answer or a panic: it is held as an [`ApiKey`], whose `Debug` and `Display` print `[key]`, and
//! every message here names the source, never what it holds.
//!
//! A key file is the human's alone: only the global file names one (`providers.*` is global
//! only), and the file is refused unless it is a regular file — opened without following a
//! symbolic link — owned by the human, readable by no one else (mode `0600` or `0400`), at most
//! [`MAX_KEY_FILE`] bytes, and outside every world this machine knows, where a body could write
//! it.

use std::fmt;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use mnemorph_config::Provider;

/// The largest key file read, in bytes: a key is a line, never a document.
pub const MAX_KEY_FILE: u64 = 4096;

/// A provider's key. Its `Debug` and `Display` print `[key]`: only [`ApiKey::expose`] gives the
/// value, to the client that sends it.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// The key, for the one client that sends it.
    #[must_use]
    pub fn expose(self) -> String {
        self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[key]")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[key]")
    }
}

/// A provider's key and where it was read from (the variable's name or the file's path, never
/// the value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The key.
    pub key: ApiKey,
    /// Where it was read from: `env OPENROUTER_API_KEY` or `file ~/.config/mnemorph/x.key`.
    pub source: String,
}

/// `path` with a leading `~/` (or a lone `~`) read as `home`; refused unless absolute after.
///
/// # Errors
///
/// A `~` with no home, or a path that is not absolute.
pub fn expand(path: &str, home: Option<&str>) -> Result<PathBuf, String> {
    let expanded = match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            let home = home
                .filter(|h| !h.is_empty())
                .ok_or("it starts with `~` and HOME is not set")?;
            PathBuf::from(format!("{home}{rest}"))
        }
        _ => PathBuf::from(path),
    };
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        Err("it is not an absolute path (start it with `/` or `~/`)".into())
    }
}

/// What a key file is, checked: its permission bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checked {
    /// Its mode's permission bits: `0o600` or `0o400`.
    pub mode: u32,
}

/// What [`refusal`] judges a key file on: what `fstat` said of it once opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    /// Whether it is a regular file.
    pub regular: bool,
    /// Its owner.
    pub uid: u32,
    /// Its mode.
    pub mode: u32,
    /// Its size, in bytes.
    pub size: u64,
}

/// Why a key file at `path` (as the human wrote it), stat'ed as `stat`, may not be read by the
/// user `uid`; `None` when it may.
#[must_use]
pub fn refusal(path: &str, stat: Stat, uid: u32) -> Option<String> {
    if !stat.regular {
        return Some("it is not a regular file".into());
    }
    if stat.uid != uid {
        return Some(format!(
            "it is owned by user {}, not by you (user {uid})",
            stat.uid
        ));
    }
    // Readable and writable by its owner alone: no group or other bit, no execute, setuid,
    // setgid or sticky bit.
    if stat.mode & 0o7177 != 0 {
        return Some(format!(
            "its mode is {:04o}, and a key file is yours alone: chmod 600 {path}",
            stat.mode & 0o7777
        ));
    }
    if stat.size > MAX_KEY_FILE {
        return Some(format!(
            "it is {} bytes, more than the {MAX_KEY_FILE} a key file may be",
            stat.size
        ));
    }
    None
}

/// The worlds a key file may not lie in: `current` and every world this machine knows — each
/// one a session opened in (the human's registry) or the board lists — each canonical.
#[must_use]
pub fn known_worlds(current: Option<&Path>) -> Vec<PathBuf> {
    let mut out = mnemorph_world::dimension::registry()
        .map(|r| worlds_at(&r))
        .unwrap_or_default();
    out.extend(current.map(Path::to_path_buf));
    for w in &mut out {
        if let Ok(c) = w.canonicalize() {
            *w = c;
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Every world the registry at `registry` knows, and every world the board beside it lists.
#[must_use]
pub fn worlds_at(registry: &Path) -> Vec<PathBuf> {
    use mnemorph_world::dimension::{hand_file, known, listed};
    let mut out: Vec<PathBuf> = known(registry).into_iter().map(|k| k.world).collect();
    out.extend(
        listed(registry, &hand_file(registry))
            .into_iter()
            .map(|l| l.world),
    );
    for w in &mut out {
        if let Ok(c) = w.canonicalize() {
            *w = c;
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The key file at `path` (as the human wrote it), opened and checked by what `fstat` says of it
/// alone — nothing in it is read: the open file and its permission bits.
fn open_stat(
    path: &str,
    home: Option<&str>,
    worlds: &[PathBuf],
) -> Result<(File, Checked), String> {
    use rustix::fs::{Mode, OFlags};
    use std::os::unix::fs::MetadataExt as _;
    let at = expand(path, home)?;
    // No symbolic link is followed at the last step, and a FIFO does not block the open: what
    // it is, is read from the open file itself.
    let fd = rustix::fs::open(
        &at,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY,
        Mode::empty(),
    )
    .map_err(|e| match e {
        rustix::io::Errno::LOOP => "it is a symbolic link".to_owned(),
        e => std::io::Error::from(e).to_string(),
    })?;
    let file = File::from(fd);
    let meta = file.metadata().map_err(|e| e.to_string())?;
    let stat = Stat {
        regular: meta.file_type().is_file(),
        uid: meta.uid(),
        mode: meta.mode(),
        size: meta.len(),
    };
    if let Some(why) = refusal(path, stat, rustix::process::geteuid().as_raw()) {
        return Err(why);
    }
    let real = real_path(&file, &at);
    if let Some(w) = worlds.iter().find(|w| real.starts_with(w)) {
        return Err(format!(
            "it lies inside the world {}, where a body could write it",
            w.display()
        ));
    }
    if stat.size == 0 {
        return Err("it is empty".into());
    }
    Ok((
        file,
        Checked {
            mode: stat.mode & 0o777,
        },
    ))
}

/// The key file at `path` (as the human wrote it), checked ([`open_stat`]), then read: what it
/// holds, trimmed, never empty.
fn open_checked(
    path: &str,
    home: Option<&str>,
    worlds: &[PathBuf],
) -> Result<(Checked, ApiKey), String> {
    let (mut file, checked) = open_stat(path, home, worlds)?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_KEY_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_KEY_FILE {
        return Err(format!(
            "it is more than the {MAX_KEY_FILE} bytes a key file may be"
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| "it is not UTF-8 text".to_owned())?;
    let key = text.trim();
    if key.is_empty() {
        return Err("it is empty".into());
    }
    if key.chars().any(char::is_control) {
        return Err("it holds more than one line, or a control character".into());
    }
    Ok((checked, ApiKey(key.to_owned())))
}

/// Where the open `file` really is: as the kernel names it, else `at` made canonical.
fn real_path(file: &File, at: &Path) -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd as _;
        if let Ok(p) = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())) {
            return p;
        }
    }
    let _ = file;
    at.canonicalize().unwrap_or_else(|_| at.to_path_buf())
}

/// Whether the key file at `path` (as the human wrote it) would be read: its mode if so, or why
/// not — from `fstat` alone. Nothing in it is read, so the key is never in memory for this; a
/// file whose contents a session would refuse (two lines, not UTF-8, only whitespace) passes.
///
/// # Errors
///
/// Why it is refused or unreadable.
pub fn inspect(path: &str, home: Option<&str>, worlds: &[PathBuf]) -> Result<Checked, String> {
    open_stat(path, home, worlds).map(|(_, c)| c)
}

/// Provider `name`'s key: from the variable its `key_env` names, if that is set (and not empty),
/// else from the file its `key_file` names; `None` when it names neither. `env` is asked for the
/// variable and for `HOME`; `worlds` are where a key file may not lie.
///
/// # Errors
///
/// Neither source gives a key: the message names each one tried, and why the file was refused.
pub fn resolve(
    name: &str,
    provider: &Provider,
    env: &dyn Fn(&str) -> Option<String>,
    worlds: &[PathBuf],
) -> Result<Option<Resolved>, String> {
    if let Some(var) = &provider.key_env
        && let Some(v) = env(var).filter(|v| !v.is_empty())
    {
        return Ok(Some(Resolved {
            key: ApiKey(v),
            source: format!("env {var}"),
        }));
    }
    let Some(path) = &provider.key_file else {
        return match &provider.key_env {
            Some(var) => Err(format!(
                "{var} is not set: provider `{name}` reads its key from it"
            )),
            None => Ok(None),
        };
    };
    let home = env("HOME");
    match open_checked(path, home.as_deref(), worlds) {
        Ok((_, key)) => Ok(Some(Resolved {
            key,
            source: format!("file {path}"),
        })),
        Err(why) => Err(match &provider.key_env {
            Some(var) => format!(
                "{var} is not set and {path} is unreadable: {why}: provider `{name}` reads its key from them"
            ),
            None => format!("{path} is unreadable: {why}: provider `{name}` reads its key from it"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiKey, Stat, expand, refusal};

    #[test]
    fn a_key_prints_as_a_mark() {
        let k = ApiKey("sk-fake-0123456789".into());
        assert_eq!(format!("{k:?}"), "[key]");
        assert_eq!(format!("{k}"), "[key]");
        assert_eq!(format!("{:?}", Some(k)), "Some([key])");
    }

    #[test]
    fn a_tilde_is_home_and_the_path_must_be_absolute() {
        assert_eq!(
            expand("~/k.key", Some("/h")).unwrap(),
            std::path::Path::new("/h/k.key")
        );
        assert!(expand("~/k.key", None).is_err());
        assert!(expand("k.key", Some("/h")).is_err());
        assert!(expand("~other/k.key", Some("/h")).is_err());
        assert!(expand("/abs/k.key", None).is_ok());
    }

    fn stat(uid: u32, mode: u32) -> Stat {
        Stat {
            regular: true,
            uid,
            mode: 0o100_000 | mode,
            size: 40,
        }
    }

    #[test]
    fn another_users_file_is_refused() {
        let why = refusal("/k", stat(1001, 0o600), 1000).unwrap();
        assert!(why.contains("owned by user 1001"), "{why}");
        assert_eq!(refusal("/k", stat(1000, 0o600), 1000), None);
    }

    #[test]
    fn only_the_owners_read_and_write_bits_pass() {
        assert_eq!(refusal("/k", stat(7, 0o400), 7), None);
        for mode in [0o644, 0o640, 0o604, 0o700, 0o4600, 0o1600] {
            let why = refusal("~/k", stat(7, mode), 7).unwrap();
            assert!(why.contains("chmod 600 ~/k"), "{mode:o}: {why}");
        }
    }

    #[test]
    fn a_large_or_irregular_file_is_refused() {
        let mut s = stat(7, 0o600);
        s.size = super::MAX_KEY_FILE + 1;
        assert!(refusal("/k", s, 7).unwrap().contains("bytes"));
        s.size = 1;
        s.regular = false;
        assert!(refusal("/k", s, 7).unwrap().contains("regular"));
    }
}
