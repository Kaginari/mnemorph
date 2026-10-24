//! Opening a checked path without a race: the path the border check resolved is opened again one
//! component at a time from the world's root, following no symlink. A component swapped for a
//! symlink between the check and the open makes the open fail instead of leading out.

use std::fs::File;
use std::io::{self, Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::path::{Component, Path};

use rustix::fs::{CWD, Mode, OFlags, mkdirat, openat, renameat};

const DIR: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// The parent directory of `path` (a resolved path under `world`), opened without following a
/// symlink, and the file name in it. `create` makes missing directories on the way.
fn parent(world: &Path, path: &Path, create: bool) -> io::Result<(OwnedFd, String)> {
    let rel = path
        .strip_prefix(world)
        .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "outside the world"))?;
    let mut parts: Vec<String> = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "not a plain path",
                ));
            }
        }
    }
    let name = parts
        .pop()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the world itself"))?;
    let mut dir = openat(CWD, world, DIR, Mode::empty())?;
    for part in parts {
        dir = match openat(&dir, part.as_str(), DIR, Mode::empty()) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) if create => {
                mkdirat(&dir, part.as_str(), Mode::from_raw_mode(0o755))?;
                openat(&dir, part.as_str(), DIR, Mode::empty())?
            }
            Err(e) => return Err(e.into()),
        };
    }
    Ok((dir, name))
}

/// Read a file under the world, following no symlink on the way.
pub(crate) fn read(world: &Path, path: &Path) -> io::Result<Vec<u8>> {
    let (dir, name) = parent(world, path, false)?;
    let fd = openat(
        &dir,
        name.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let mut bytes = Vec::new();
    File::from(fd).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Replace a file under the world whole or not at all: a temporary sibling written and renamed
/// inside the same directory handle, following no symlink on the way.
pub(crate) fn write(world: &Path, path: &Path, content: &[u8]) -> io::Result<()> {
    let (dir, name) = parent(world, path, true)?;
    let tmp = format!(".{name}.mnemorph-tmp");
    let flags =
        OFlags::WRONLY | OFlags::CREATE | OFlags::TRUNC | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = openat(&dir, tmp.as_str(), flags, Mode::from_raw_mode(0o644))?;
    let mut file = File::from(fd);
    file.write_all(content)?;
    file.sync_data()?;
    renameat(&dir, tmp.as_str(), &dir, name.as_str())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{read, write};

    /// The race the border check alone cannot close: a directory swapped for a symlink after
    /// the path was resolved.
    #[test]
    fn a_component_swapped_for_a_symlink_is_refused() {
        let root = std::env::temp_dir().join(format!("mnemorph-beneath-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world/a")).unwrap();
        fs::create_dir_all(root.join("outside")).unwrap();
        fs::write(root.join("world/a/f"), "inside").unwrap();
        fs::write(root.join("outside/f"), "secret").unwrap();
        let world = root.join("world").canonicalize().unwrap();
        let checked = world.join("a/f");
        assert_eq!(read(&world, &checked).unwrap(), b"inside");

        // The swap, after the check.
        fs::remove_dir_all(world.join("a")).unwrap();
        std::os::unix::fs::symlink(root.join("outside"), world.join("a")).unwrap();
        assert!(
            read(&world, &checked).is_err(),
            "read followed the swapped link"
        );
        assert!(
            write(&world, &checked, b"pwned").is_err(),
            "write followed the swapped link"
        );
        assert_eq!(
            fs::read_to_string(root.join("outside/f")).unwrap(),
            "secret"
        );

        // The last component swapped for a link is refused too.
        fs::remove_file(world.join("a")).unwrap();
        fs::create_dir_all(world.join("a")).unwrap();
        std::os::unix::fs::symlink(root.join("outside/f"), world.join("a/f")).unwrap();
        assert!(read(&world, &checked).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn writes_make_their_directories_and_replace_whole() {
        let root = std::env::temp_dir().join(format!("mnemorph-beneath-w-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let world = root.canonicalize().unwrap();
        write(&world, &world.join("x/y/z.txt"), b"one").unwrap();
        write(&world, &world.join("x/y/z.txt"), b"two").unwrap();
        assert_eq!(fs::read_to_string(world.join("x/y/z.txt")).unwrap(), "two");
        assert!(!world.join("x/y/.z.txt.mnemorph-tmp").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
