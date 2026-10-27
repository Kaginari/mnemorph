//! A restore's hands: every look and every write through a directory handle opened beneath the
//! world, following no symlink. A component swapped for a symlink between a check and a write
//! makes the write fail; it never lands outside the world.

use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags, Stat};
use rustix::io::Errno;

const DIR: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// Where a walk to a path's parent ended.
pub(crate) enum Walk {
    /// The parent, opened.
    Parent(OwnedFd),
    /// A leading directory is missing, so the path is.
    Absent,
    /// A leading component, named, is a symlink or not a directory.
    Blocked(String),
}

/// The world's root, opened as a directory and not through a symlink.
pub(crate) fn root(world: &Path) -> io::Result<OwnedFd> {
    Ok(rustix::fs::openat(
        rustix::fs::CWD,
        world,
        DIR,
        Mode::empty(),
    )?)
}

/// Walk to the parent of `parts` (the path's components), one component at a time.
pub(crate) fn walk(root: &OwnedFd, parts: &[&str]) -> io::Result<Walk> {
    let mut dir = rustix::fs::openat(root, ".", DIR, Mode::empty())?;
    for (i, part) in parts[..parts.len().saturating_sub(1)].iter().enumerate() {
        dir = match rustix::fs::openat(&dir, *part, DIR, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Ok(Walk::Absent),
            Err(Errno::NOTDIR | Errno::LOOP) => return Ok(Walk::Blocked(parts[..=i].join("/"))),
            Err(e) => return Err(e.into()),
        };
    }
    Ok(Walk::Parent(dir))
}

/// The parent of `parts`, for a write: `openat2` with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`
/// where the kernel has it, else the component walk; with `create`, missing directories are
/// made on the way. A symlink anywhere on the way is an error.
pub(crate) fn parent(root: &OwnedFd, parts: &[&str], create: bool) -> io::Result<OwnedFd> {
    let lead = &parts[..parts.len().saturating_sub(1)];
    if lead.is_empty() {
        return Ok(rustix::fs::openat(root, ".", DIR, Mode::empty())?);
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use rustix::fs::ResolveFlags;
        match rustix::fs::openat2(
            root,
            lead.join("/"),
            DIR,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
        ) {
            Ok(fd) => return Ok(fd),
            Err(Errno::NOSYS | Errno::NOENT | Errno::AGAIN) => {}
            Err(e) => return Err(e.into()),
        }
    }
    let mut dir = rustix::fs::openat(root, ".", DIR, Mode::empty())?;
    for part in lead {
        dir = match rustix::fs::openat(&dir, *part, DIR, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) if create => {
                match rustix::fs::mkdirat(&dir, *part, Mode::from_raw_mode(0o755)) {
                    Ok(()) | Err(Errno::EXIST) => {}
                    Err(e) => return Err(e.into()),
                }
                rustix::fs::openat(&dir, *part, DIR, Mode::empty())?
            }
            Err(e) => return Err(e.into()),
        };
    }
    Ok(dir)
}

/// `name`'s metadata in `dir`, not following it; `None` when absent.
pub(crate) fn stat(dir: &OwnedFd, name: &str) -> io::Result<Option<Stat>> {
    match rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(s) => Ok(Some(s)),
        Err(Errno::NOENT) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The kind of a stat.
pub(crate) fn kind(s: &Stat) -> FileType {
    FileType::from_raw_mode(s.st_mode)
}

/// A symlink's target.
pub(crate) fn read_link(dir: &OwnedFd, name: &str) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt as _;
    let target = rustix::fs::readlinkat(dir, name, Vec::new())?;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(
        target.into_bytes(),
    )))
}

/// Open a regular file for reading without blocking or following a link, and only if it is
/// still a regular file of `size` bytes: a fifo never blocks the reader, a grown file is never
/// read. `None` when it is not.
pub(crate) fn open_regular(dir: &OwnedFd, name: &str, size: u64) -> io::Result<Option<File>> {
    let fd = match rustix::fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT | Errno::LOOP) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let s = rustix::fs::fstat(&fd)?;
    let same = kind(&s) == FileType::RegularFile && u64::try_from(s.st_size).ok() == Some(size);
    Ok(same.then(|| File::from(fd)))
}

/// Whether every non-directory under `name` in `dir` is one of `removed` (paths relative to the
/// world, `rel` being `name`'s): read through handles, following no symlink.
pub(crate) fn holds_only(
    dir: &OwnedFd,
    name: &str,
    rel: &str,
    removed: &dyn Fn(&str) -> bool,
) -> bool {
    let Ok(sub) = rustix::fs::openat(dir, name, DIR, Mode::empty()) else {
        return false;
    };
    let Ok(entries) = rustix::fs::Dir::read_from(&sub) else {
        return false;
    };
    for e in entries {
        let Ok(e) = e else { return false };
        let Ok(child) = e.file_name().to_str() else {
            return false;
        };
        if child == "." || child == ".." {
            continue;
        }
        let path = format!("{rel}/{child}");
        let is_dir = match e.file_type() {
            FileType::Unknown => stat(&sub, child)
                .ok()
                .flatten()
                .is_some_and(|s| kind(&s) == FileType::Directory),
            t => t == FileType::Directory,
        };
        let ok = if is_dir {
            holds_only(&sub, child, &path, removed)
        } else {
            removed(&path)
        };
        if !ok {
            return false;
        }
    }
    true
}

/// Remove `name` in `dir` and the empty directories under it, deepest first: `rmdir` removes
/// only empty directories, so whatever another writer put inside makes it fail.
pub(crate) fn remove_empty_tree(dir: &OwnedFd, name: &str) -> io::Result<()> {
    let sub = rustix::fs::openat(dir, name, DIR, Mode::empty())?;
    let mut children = Vec::new();
    for e in rustix::fs::Dir::read_from(&sub)? {
        let e = e?;
        let child = e.file_name().to_string_lossy().into_owned();
        if child != "." && child != ".." {
            children.push(child);
        }
    }
    for child in children {
        remove_empty_tree(&sub, &child)?;
    }
    Ok(rustix::fs::unlinkat(dir, name, AtFlags::REMOVEDIR)?)
}

/// Remove the file or link `name` in `dir`; a directory there is refused (`unlinkat` without
/// `AT_REMOVEDIR` never removes one), so is nothing.
pub(crate) fn unlink(dir: &OwnedFd, name: &str) -> io::Result<()> {
    match stat(dir, name)? {
        None => Ok(()),
        Some(s) if kind(&s) == FileType::Directory => {
            Err(io::Error::other(format!("`{name}` became a directory")))
        }
        Some(_) => Ok(rustix::fs::unlinkat(dir, name, AtFlags::empty())?),
    }
}

/// Put `name` in `dir` from a temporary sibling written by `fill`: renamed over what is there
/// when `replace`, else only where nothing is (a directory of empty directories in the way is
/// removed first). The temporary goes again on any failure.
pub(crate) fn put(
    dir: &OwnedFd,
    name: &str,
    replace: bool,
    fill: impl FnOnce(&OwnedFd, &str) -> io::Result<()>,
) -> io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let tmp = format!(
        ".mnemorph-restore-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let placed = fill(dir, &tmp).and_then(|()| {
        match stat(dir, name)? {
            Some(s) if kind(&s) == FileType::Directory => remove_empty_tree(dir, name)?,
            _ => {}
        }
        if replace {
            Ok(rustix::fs::renameat(dir, tmp.as_str(), dir, name)?)
        } else {
            Ok(rustix::fs::renameat_with(
                dir,
                tmp.as_str(),
                dir,
                name,
                RenameFlags::NOREPLACE,
            )?)
        }
    });
    if placed.is_err() {
        let _ = rustix::fs::unlinkat(dir, tmp.as_str(), AtFlags::empty());
    }
    placed
}

/// A new file `tmp` in `dir`, created exclusively, filled from `content`, its mode set.
pub(crate) fn write_new(
    dir: &OwnedFd,
    tmp: &str,
    content: &mut dyn io::Read,
    executable: bool,
) -> io::Result<()> {
    let fd = rustix::fs::openat(
        dir,
        tmp,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?;
    rustix::fs::fchmod(
        &fd,
        Mode::from_raw_mode(if executable { 0o755 } else { 0o644 }),
    )?;
    let mut file = File::from(fd);
    io::copy(content, &mut file)?;
    file.sync_data()
}

/// A new symlink `tmp` in `dir` to `target`.
pub(crate) fn link_new(dir: &OwnedFd, tmp: &str, target: &Path) -> io::Result<()> {
    Ok(rustix::fs::symlinkat(target, dir, tmp)?)
}

/// Remove the file `rel` (a plain relative path) the world did not have before, through
/// handles beneath it: a parent swapped for a symlink fails the removal; a directory is never
/// removed. Whether something was removed.
///
/// # Errors
///
/// A component on the way is a symlink or not a directory, the path is a directory, or the
/// filesystem fails.
pub fn remove_created(world: &Path, rel: &Path) -> io::Result<bool> {
    let parts: Vec<String> = rel
        .components()
        .map(|c| match c {
            std::path::Component::Normal(s) => Ok(s.to_string_lossy().into_owned()),
            _ => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "not a plain path",
            )),
        })
        .collect::<io::Result<_>>()?;
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    let Some(name) = parts.last() else {
        return Ok(false);
    };
    let dir = parent(&root(world)?, &parts, false)?;
    if stat(&dir, name)?.is_none() {
        return Ok(false);
    }
    unlink(&dir, name)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::remove_created;

    #[test]
    fn a_removal_never_goes_through_a_swapped_parent() {
        let root = std::env::temp_dir().join(format!("mnemorph-pinned-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world/a")).unwrap();
        fs::create_dir_all(root.join("outside")).unwrap();
        fs::write(root.join("world/a/new"), "the act").unwrap();
        fs::write(root.join("outside/new"), "not the world's").unwrap();
        let world = root.join("world");
        fs::remove_dir_all(world.join("a")).unwrap();
        std::os::unix::fs::symlink(root.join("outside"), world.join("a")).unwrap();
        assert!(remove_created(&world, Path::new("a/new")).is_err());
        assert!(root.join("outside/new").exists());
        fs::create_dir(world.join("d")).unwrap();
        assert!(
            remove_created(&world, Path::new("d")).is_err(),
            "a directory"
        );
        assert!(remove_created(&world, Path::new("../outside/new")).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
