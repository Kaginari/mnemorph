//! Reading and writing a file under a root a body can write, without a race: the path is opened
//! one component at a time from the root, following no link, and the file itself is opened without
//! following one and checked on its handle. A component swapped for a link between a check and the
//! act makes the act fail instead of leading out.

use std::fs::File;
use std::io::{self, Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{
    AtFlags, CWD, Mode, OFlags, RenameFlags, mkdirat, openat, renameat_with, unlinkat,
};

const DIR: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

fn denied(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, why.to_owned())
}

/// The folder holding `rel` under `root`, opened component by component following no link, and
/// the last component's name; with `create`, missing folders are made on the way.
fn parent(root: &Path, rel: &str, create: bool) -> io::Result<(OwnedFd, String)> {
    let mut parts: Vec<&str> = rel.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err(denied("not a plain path"));
    }
    let name = parts.pop().ok_or_else(|| denied("no path"))?;
    let mut dir = openat(CWD, root, DIR, Mode::empty())?;
    for part in parts {
        dir = match openat(&dir, part, DIR, Mode::empty()) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) if create => {
                mkdirat(&dir, part, Mode::from_raw_mode(0o755))?;
                openat(&dir, part, DIR, Mode::empty())?
            }
            Err(e) => return Err(e.into()),
        };
    }
    Ok((dir, name.to_owned()))
}

/// The bytes of the regular file `rel` under `root`, at most `cap` of them; `None` when it or a
/// folder on its way is absent.
///
/// # Errors
///
/// A link on the way or at the file, anything but a regular file, more than `cap` bytes, or it
/// cannot be read.
pub(crate) fn read(root: &Path, rel: &str, cap: u64) -> io::Result<Option<Vec<u8>>> {
    let (dir, name) = match parent(root, rel, false) {
        Ok(p) => p,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let fd = match openat(
        &dir,
        name.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let file = File::from(fd);
    if !file.metadata()?.file_type().is_file() {
        return Err(denied("not a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > cap {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("over {cap} bytes"),
        ));
    }
    Ok(Some(bytes))
}

/// Writes `bytes` as `rel` under `root`, through a fresh file renamed into place: with `create`,
/// only where nothing stands yet; else over the file there, never through a link.
///
/// # Errors
///
/// A link on the way, something already at `rel` when creating, or it cannot be written.
pub(crate) fn write(root: &Path, rel: &str, bytes: &[u8], create: bool) -> io::Result<()> {
    let (dir, name) = parent(root, rel, true)?;
    let tmp = format!(".{name}.mnemorph-swap");
    let fd = openat(
        &dir,
        tmp.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o644),
    )?;
    let written = File::from(fd).write_all(bytes).and_then(|()| {
        let flags = if create {
            RenameFlags::NOREPLACE
        } else {
            RenameFlags::empty()
        };
        renameat_with(&dir, tmp.as_str(), &dir, name.as_str(), flags).map_err(io::Error::from)
    });
    if written.is_err() {
        let _ = unlinkat(&dir, tmp.as_str(), AtFlags::empty());
    }
    written
}

/// Removes the file `rel` under `root` (a link there goes, never what it names); absent is done.
///
/// # Errors
///
/// A link on the way, or it cannot be removed.
pub(crate) fn remove(root: &Path, rel: &str) -> io::Result<()> {
    let (dir, name) = match parent(root, rel, false) {
        Ok(p) => p,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    match unlinkat(&dir, name.as_str(), AtFlags::empty()) {
        Ok(()) | Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{read, remove, write};

    #[test]
    fn a_link_swapped_in_after_a_check_is_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("world");
        let outside = dir.path().join("outside");
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(root.join("a/f"), "inside").unwrap();
        fs::write(outside.join("f"), "secret").unwrap();
        assert_eq!(read(&root, "a/f", 64).unwrap(), Some(b"inside".to_vec()));
        assert!(read(&root, "a/f", 3).is_err(), "over the cap");
        assert_eq!(read(&root, "b/f", 64).unwrap(), None);
        write(&root, "a/new", b"n", true).unwrap();
        assert!(
            write(&root, "a/new", b"again", true).is_err(),
            "created twice"
        );
        assert_eq!(fs::read(root.join("a/new")).unwrap(), b"n");
        // The folder swapped for a link, after the check.
        fs::remove_dir_all(root.join("a")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("a")).unwrap();
        assert!(read(&root, "a/f", 64).is_err());
        assert!(write(&root, "a/f", b"pwned", false).is_err());
        assert!(write(&root, "a/g", b"pwned", true).is_err());
        assert!(remove(&root, "a/f").is_err());
        assert_eq!(fs::read_to_string(outside.join("f")).unwrap(), "secret");
        assert!(!outside.join("g").exists());
        // The file itself swapped for a link: read refuses, a create refuses, a replace puts a
        // file where the link was and leaves its target alone.
        fs::remove_file(root.join("a")).unwrap();
        fs::create_dir_all(root.join("a")).unwrap();
        std::os::unix::fs::symlink(outside.join("f"), root.join("a/f")).unwrap();
        assert!(read(&root, "a/f", 64).is_err());
        assert!(write(&root, "a/f", b"x", true).is_err());
        write(&root, "a/f", b"x", false).unwrap();
        assert!(!fs::symlink_metadata(root.join("a/f")).unwrap().is_symlink());
        assert_eq!(fs::read_to_string(outside.join("f")).unwrap(), "secret");
        assert!(read(&root, "../x", 64).is_err());
    }
}
