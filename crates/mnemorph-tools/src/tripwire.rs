//! The tripwire: files a command may not change, though they lie inside the writable world.
//!
//! Landlock only adds access, so a read-only corner cannot be carved out of the writable world.
//! Instead each command runs between two snapshots of the protected files: whatever it changed,
//! added or removed there is put back once its process group is dead, and the body is told.
//!
//! Protected: git's executable configuration — `.git/config` (`core.fsmonitor`,
//! `core.sshCommand`, filter drivers), `.git/hooks`, `.git/info`, or a `.git` file pointing
//! elsewhere — which would otherwise run code outside the sandbox the next time the human runs
//! git; and the world's own records in `.mnemorph/` — except `shared/` (the work queue every
//! body writes), `tmp/` (each command's scratch) and `intel/` (the scout's mirror).

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

/// Bytes of protected content a snapshot keeps; beyond, a change is still seen but not undone.
const KEEP_BYTES: usize = 16 << 20;

/// The roots the tripwire watches in `world`.
pub(crate) fn roots(world: &Path) -> Vec<PathBuf> {
    let git = world.join(".git");
    let mut out = if git.is_file() {
        vec![git]
    } else {
        vec![git.join("config"), git.join("hooks"), git.join("info")]
    };
    out.push(world.join(".mnemorph"));
    out
}

/// Whether a path the file tools would write is protected.
pub(crate) fn protects(world: &Path, path: &Path) -> bool {
    roots(world).iter().any(|r| path.starts_with(r)) && !skipped(world, path)
}

fn skipped(world: &Path, path: &Path) -> bool {
    ["shared", "tmp", "intel"]
        .iter()
        .any(|d| path.starts_with(world.join(".mnemorph").join(d)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    File {
        bytes: Option<Vec<u8>>,
        mode: u32,
        len: u64,
        hash: u64,
    },
    Link(PathBuf),
    Dir,
}

/// The protected files of a world at one moment.
#[derive(Debug, Default)]
pub(crate) struct Snapshot {
    world: PathBuf,
    /// The roots that existed when the snapshot was taken; only these are watched, so a
    /// repository the command itself creates is its own.
    watched: Vec<PathBuf>,
    entries: BTreeMap<PathBuf, Entry>,
}

impl Snapshot {
    pub(crate) fn take(world: &Path) -> Self {
        let watched: Vec<PathBuf> = roots(world)
            .into_iter()
            .filter(|r| fs::symlink_metadata(r).is_ok())
            .collect();
        Self::of(world, &watched)
    }

    fn of(world: &Path, watched: &[PathBuf]) -> Self {
        let mut s = Self {
            world: world.to_path_buf(),
            watched: watched.to_vec(),
            entries: BTreeMap::new(),
        };
        let mut kept = 0usize;
        for root in watched {
            s.walk(root, &mut kept);
        }
        s
    }

    fn walk(&mut self, path: &Path, kept: &mut usize) {
        if skipped(&self.world, path) {
            return;
        }
        let Ok(meta) = fs::symlink_metadata(path) else {
            return;
        };
        if meta.file_type().is_symlink() {
            if let Ok(target) = fs::read_link(path) {
                self.entries.insert(path.to_path_buf(), Entry::Link(target));
            }
        } else if meta.is_dir() {
            self.entries.insert(path.to_path_buf(), Entry::Dir);
            if let Ok(rd) = fs::read_dir(path) {
                let mut children: Vec<PathBuf> =
                    rd.filter_map(Result::ok).map(|e| e.path()).collect();
                children.sort();
                for c in children {
                    self.walk(&c, kept);
                }
            }
        } else if let Ok(bytes) = fs::read(path) {
            let hash = hash(&bytes);
            let len = bytes.len() as u64;
            let keep = *kept + bytes.len() <= KEEP_BYTES;
            if keep {
                *kept += bytes.len();
            }
            let mode = meta.permissions().mode();
            self.entries.insert(
                path.to_path_buf(),
                Entry::File {
                    bytes: keep.then_some(bytes),
                    mode,
                    len,
                    hash,
                },
            );
        }
    }

    /// Compare with the world now and put back what changed. Returns one line per change; a
    /// change that could not be undone says so.
    pub(crate) fn restore(&self) -> Vec<String> {
        let now = Self::of(&self.world, &self.watched);
        let mut notes = Vec::new();
        let shown = |p: &Path| {
            p.strip_prefix(&self.world)
                .unwrap_or(p)
                .display()
                .to_string()
        };
        // Added since: removed, deepest first.
        for path in now.entries.keys().rev() {
            if !self.entries.contains_key(path) {
                let gone = match now.entries[path] {
                    Entry::Dir => fs::remove_dir_all(path),
                    _ => fs::remove_file(path),
                };
                notes.push(match gone {
                    Ok(()) => format!("removed the new `{}`", shown(path)),
                    Err(e) => format!("could not remove the new `{}`: {e}", shown(path)),
                });
            }
        }
        // Changed or removed: written back, shallowest first.
        for (path, before) in &self.entries {
            let after = now.entries.get(path);
            if after == Some(before) {
                continue;
            }
            if let (
                Some(Entry::File {
                    hash: a,
                    len: la,
                    mode: ma,
                    ..
                }),
                Entry::File {
                    hash: b,
                    len: lb,
                    mode: mb,
                    ..
                },
            ) = (after, before)
                && a == b
                && la == lb
                && ma == mb
            {
                continue;
            }
            notes.push(match put_back(path, before, after) {
                Ok(()) => format!("restored `{}`", shown(path)),
                Err(e) => format!("could not restore `{}`: {e}", shown(path)),
            });
        }
        notes
    }
}

fn put_back(path: &Path, before: &Entry, after: Option<&Entry>) -> std::io::Result<()> {
    match after {
        Some(Entry::Dir) if !matches!(before, Entry::Dir) => fs::remove_dir_all(path)?,
        Some(Entry::File { .. } | Entry::Link(_)) if !matches!(before, Entry::File { .. }) => {
            fs::remove_file(path)?;
        }
        Some(Entry::Link(_)) => fs::remove_file(path)?,
        _ => {}
    }
    match before {
        Entry::Dir => fs::create_dir_all(path),
        Entry::Link(target) => std::os::unix::fs::symlink(target, path),
        Entry::File {
            bytes: Some(bytes),
            mode,
            ..
        } => {
            fs::write(path, bytes)?;
            fs::set_permissions(path, fs::Permissions::from_mode(*mode))
        }
        Entry::File { bytes: None, .. } => {
            Err(std::io::Error::other("its content was too large to keep"))
        }
    }
}

fn hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}
