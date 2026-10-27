//! The world a record is checked against: the landing's tree, read by repository path.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::PathBuf;

use mnemorph_wire::repo_path;

/// The most bytes a read takes from one file: a longer file reads as absent.
pub const READ_BYTES: u64 = 16 * 1024 * 1024;

/// The landing's tree, as the validator reads it.
pub trait World {
    /// The bytes of the file at `path`, a repository-relative path; `None` when there is no
    /// such file, or it may not be read.
    fn read(&self, path: &str) -> Option<Vec<u8>>;
}

/// A tree in memory, path to bytes.
impl World for BTreeMap<String, Vec<u8>> {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.get(&repo_path(path)?).cloned()
    }
}

/// A tree on disk. A path is read only when the wire's ASCII path rule takes it, and no part of
/// it is a symbolic link, so nothing outside the root is ever read.
#[derive(Debug, Clone)]
pub struct Tree {
    root: PathBuf,
}

impl Tree {
    /// The tree rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Tree { root: root.into() }
    }
}

impl World for Tree {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let path = repo_path(path)?;
        let mut at = self.root.clone();
        for segment in path.split('/') {
            at.push(segment);
            if std::fs::symlink_metadata(&at)
                .ok()?
                .file_type()
                .is_symlink()
            {
                return None;
            }
        }
        let file = std::fs::File::open(&at).ok()?;
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(READ_BYTES + 1).read_to_end(&mut bytes).ok()?;
        (bytes.len() as u64 <= READ_BYTES).then_some(bytes)
    }
}
