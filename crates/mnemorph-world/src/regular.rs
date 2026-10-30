//! Reading a file inside a world as a reader that grants it nothing: a body can write the world, so
//! a path there may be a link to `/dev/zero`, a pipe that never answers, a device, or a file of any
//! size. Only a regular file is read — opened without following a link and without blocking, then
//! checked again on the handle — and never past a cap.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Why a file in a world was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// A link, a pipe, a device, a socket — anything but a regular file.
    NotRegular,
    /// Larger than the cap.
    TooLarge(u64),
    /// It could not be opened or read.
    Io(String),
}

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unread::NotRegular => f.write_str("not a regular file (a link, a pipe or a device)"),
            Unread::TooLarge(cap) => write!(f, "too large: over {cap} bytes"),
            Unread::Io(e) => f.write_str(e),
        }
    }
}

/// `path` opened for reading if it is a regular file: a link is not followed, a pipe does not
/// block the open, and the handle itself is checked. `Ok(None)` when it is absent.
///
/// # Errors
///
/// It is not a regular file, or it cannot be opened.
pub fn open(path: &Path) -> Result<Option<File>, Unread> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_file() => {}
        Ok(_) => return Err(Unread::NotRegular),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Unread::Io(e.to_string())),
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // A link swapped in after the check is refused by the kernel; a pipe opens at once.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Unread::Io(e.to_string())),
    };
    match file.metadata() {
        Ok(m) if m.file_type().is_file() => Ok(Some(file)),
        Ok(_) => Err(Unread::NotRegular),
        Err(e) => Err(Unread::Io(e.to_string())),
    }
}

/// The bytes of `path`, a regular file of at most `cap` bytes; `Ok(None)` when it is absent.
///
/// # Errors
///
/// It is not a regular file, it is over `cap`, or it cannot be read.
pub fn read(path: &Path, cap: u64) -> Result<Option<Vec<u8>>, Unread> {
    let Some(file) = open(path)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| Unread::Io(e.to_string()))?;
    if bytes.len() as u64 > cap {
        return Err(Unread::TooLarge(cap));
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_regular_file_within_the_cap_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let at = |n: &str| dir.path().join(n);
        std::fs::write(at("ok"), "hello").unwrap();
        assert_eq!(read(&at("ok"), 5).unwrap(), Some(b"hello".to_vec()));
        assert_eq!(read(&at("ok"), 4), Err(Unread::TooLarge(4)));
        assert_eq!(read(&at("absent"), 4), Ok(None));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/dev/zero", at("zero")).unwrap();
            assert_eq!(read(&at("zero"), 1 << 20), Err(Unread::NotRegular));
            std::os::unix::fs::symlink(at("ok"), at("link")).unwrap();
            assert_eq!(read(&at("link"), 1 << 20), Err(Unread::NotRegular));
            assert!(
                std::process::Command::new("mkfifo")
                    .arg(at("pipe"))
                    .status()
                    .unwrap()
                    .success()
            );
            assert_eq!(read(&at("pipe"), 1 << 20), Err(Unread::NotRegular));
        }
        std::fs::create_dir(at("dir")).unwrap();
        assert_eq!(read(&at("dir"), 4), Err(Unread::NotRegular));
    }
}
