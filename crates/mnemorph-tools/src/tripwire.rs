//! The tripwire: files a command may not change, though they lie inside the writable world.
//!
//! Landlock only adds access, so a read-only corner cannot be carved out of the writable world.
//! Instead each command runs between two snapshots of the protected files: whatever it changed,
//! added or removed there is put back once its process group is dead, and the body is told.
//!
//! Protected: git's executable configuration — `.git/config` (`core.fsmonitor`,
//! `core.sshCommand`, filter drivers), `.git/hooks`, `.git/info`, or a `.git` file pointing
//! elsewhere — which would otherwise run code outside the sandbox the next time the human runs
//! git; and the world's own records in `.mnemorph/` — except `tmp/` (each command's scratch) and
//! `intel/` (the scout's mirror).
//!
//! The shared work queue is put back line by line: what it held stays, and of the lines added
//! since, only those its witness holds — every line the harness or the human's `mnemorph q` wrote,
//! kept again under the human's state directory, out of every command's reach. So a body's
//! command cannot forge a move, and the human's moves made while it ran are kept.

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
    ["tmp", "intel"]
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
    /// The shared work queue's bytes, kept whole and apart from the budget.
    queue: Option<Vec<u8>>,
    /// Where the queue's witness is.
    witness: Option<PathBuf>,
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
            queue: fs::read(world.join(QUEUE)).ok(),
            witness: queue_witness(world),
        };
        let mut kept = 0usize;
        for root in watched {
            s.walk(root, &mut kept);
        }
        s
    }

    fn walk(&mut self, path: &Path, kept: &mut usize) {
        if skipped(&self.world, path) || path == self.world.join(QUEUE) {
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

    /// A repository the human approved deleting is gone, not tampered with: stop watching its
    /// pieces, so none of them is put back into a `.git` that is no longer there.
    pub(crate) fn release_deleted_repository(&mut self) {
        let git = self.world.join(".git");
        if fs::symlink_metadata(&git).is_err() {
            self.watched.retain(|r| !r.starts_with(&git));
            self.entries.retain(|p, _| !p.starts_with(&git));
        }
    }

    /// Compare with the world now and put back what changed. Returns one line per change; a
    /// change that could not be undone says so.
    pub(crate) fn restore(&self) -> Vec<String> {
        let now = Self::of(&self.world, &self.watched);
        let mut notes: Vec<String> = self.restore_queue().into_iter().collect();
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

/// The shared work queue, relative to the world.
const QUEUE: &str = ".mnemorph/shared/queue.jsonl";

impl Snapshot {
    /// Put the queue back line by line: what it held, and the added lines its witness holds.
    fn restore_queue(&self) -> Option<String> {
        let path = self.world.join(QUEUE);
        let now = fs::read(&path).ok();
        if now == self.queue {
            return None;
        }
        let witness: std::collections::HashSet<Vec<u8>> = self
            .witness
            .as_ref()
            .and_then(|w| fs::read(w).ok())
            .map(|b| lines(&b).map(<[u8]>::to_vec).collect())
            .unwrap_or_default();
        let old = self.queue.clone().unwrap_or_default();
        let held: std::collections::HashSet<&[u8]> = lines(&old).collect();
        let mut kept = old.clone();
        let (mut added, mut forged) = (0, 0);
        for line in lines(now.as_deref().unwrap_or_default()).filter(|l| !held.contains(l)) {
            if witness.contains(line) {
                if !kept.is_empty() && !kept.ends_with(b"\n") {
                    kept.push(b'\n');
                }
                kept.extend_from_slice(line);
                kept.push(b'\n');
                added += 1;
            } else {
                forged += 1;
            }
        }
        if now.as_deref() == Some(kept.as_slice()) {
            return None;
        }
        let done = if kept.is_empty() && self.queue.is_none() {
            fs::remove_file(&path)
        } else {
            fs::write(&path, &kept)
        };
        Some(match done {
            Ok(()) => format!(
                "put back the work queue: {forged} line(s) its witness does not hold dropped, {added} kept"
            ),
            Err(e) => format!("could not put back the work queue: {e}"),
        })
    }
}

/// The non-empty lines of `bytes`.
fn lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty())
}

/// Where the queue's witness lives: `$XDG_STATE_HOME/mnemorph/queue/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`. The world crate writes it at the same place
/// (`mnemorph_world::queue::witness_of`); a test in the loop holds the two together.
#[must_use]
pub fn queue_witness(world: &Path) -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".local/state")))?;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(
        state
            .join("mnemorph/queue")
            .join(format!("{name}-{h:016x}.jsonl")),
    )
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

#[cfg(test)]
mod queue_tests {
    use super::*;

    /// A body's command added a forged line and the human's `q` added a witnessed one while it
    /// ran: the forged line goes, the human's stays, the old lines are untouched.
    #[test]
    fn the_queue_is_put_back_by_its_witness() {
        let root = std::env::temp_dir().join(format!("mnemorph-witness-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("w");
        fs::create_dir_all(world.join(".mnemorph/shared")).unwrap();
        let witness = root.join("witness.jsonl");
        fs::write(world.join(QUEUE), "old\n").unwrap();
        fs::write(&witness, "old\n").unwrap();
        let take = || Snapshot {
            witness: Some(witness.clone()),
            ..Snapshot::take(&world)
        };
        let before = take();
        fs::write(world.join(QUEUE), "old\nforged\nhuman\n").unwrap();
        fs::write(&witness, "old\nhuman\n").unwrap();
        let notes = before.restore();
        assert_eq!(
            fs::read_to_string(world.join(QUEUE)).unwrap(),
            "old\nhuman\n"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.contains("1 line(s) its witness does not hold dropped")),
            "{notes:?}"
        );
        // A history rewritten by the command comes back whole.
        let before = take();
        fs::write(world.join(QUEUE), "rewritten\n").unwrap();
        before.restore();
        assert_eq!(
            fs::read_to_string(world.join(QUEUE)).unwrap(),
            "old\nhuman\n"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
