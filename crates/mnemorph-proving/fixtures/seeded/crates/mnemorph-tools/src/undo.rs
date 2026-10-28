//! Undo: removing a file the body wrote, once it is shown to be inside the world.

use std::io;
use std::path::Path;

/// Removes `written`, a path the body wrote, relative to the world at `root`.
pub fn undo_write(root: &Path, written: &str) -> io::Result<()> {
    let target = root.join(written);
    let real = target.canonicalize()?;
    if !real.starts_with(root.canonicalize()?) {
        return Err(io::Error::other("outside the world"));
    }
    std::fs::remove_file(&target)
}
