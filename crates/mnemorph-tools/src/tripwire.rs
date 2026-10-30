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
//! since, only those its witness holds — every line the harness or the human's `mnemorphd q` wrote,
//! kept again under the human's state directory, out of every command's reach. So a body's
//! command cannot forge a move, and the human's moves made while it ran are kept. Each Kijin
//! post journal (`.mnemorph/kijin/<post>/post.jsonl`) is put back the same way from its own
//! witness, so the rows the harness journals while a command runs are never lost; and so is the
//! usage ledger (`.mnemorph/instruments/usage.jsonl`), whose every chained byte the harness writes
//! to its witness first, so a parallel run's rows written while a command ran are kept.

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
    ["tmp", "intel", "report"]
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
    /// The files put back from a witness: the work queue first, then each post journal.
    witnessed: Vec<Witnessed>,
}

/// A file only the harness (and the human) append to, its every line written again to a witness
/// out of every command's reach.
#[derive(Debug)]
struct Witnessed {
    /// Relative to the world.
    rel: PathBuf,
    /// What the file is, in a note.
    what: String,
    /// Its bytes, kept whole and apart from the budget.
    held: Option<Vec<u8>>,
    /// Where its witness is.
    witness: Option<PathBuf>,
    /// How long the witness was then: what it gains after is the harness's and the human's.
    witness_len: usize,
}

/// The witnessed files of `world`: the work queue, the usage ledger, and each Kijin post journal.
fn witnessed(world: &Path) -> Vec<Witnessed> {
    let mut out = vec![
        Witnessed {
            rel: PathBuf::from(QUEUE),
            what: "the work queue: only the harness and the human move it".into(),
            held: None,
            witness: queue_witness(world),
            witness_len: 0,
        },
        Witnessed {
            rel: PathBuf::from(USAGE),
            what: "the usage ledger: only the harness writes it".into(),
            held: None,
            witness: usage_witness(world),
            witness_len: 0,
        },
    ];
    for post in mnemorph_races::Post::ALL {
        out.push(Witnessed {
            rel: Path::new(KIJIN).join(post.creature()).join(POST_JOURNAL),
            what: format!(
                "the {} post journal: only the harness writes it",
                post.creature()
            ),
            held: None,
            witness: kijin_witness(world, post.creature()),
            witness_len: 0,
        });
    }
    out
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
            witnessed: witnessed(world),
        };
        // Each file and its witness's length are read together, under the witness's lock.
        for w in &mut s.witnessed {
            let lock = w.witness.as_deref().and_then(locked);
            w.held = fs::read(world.join(&w.rel)).ok();
            w.witness_len = lock
                .as_ref()
                .and_then(|f| f.metadata().ok())
                .map_or(0, |m| usize::try_from(m.len()).unwrap_or(usize::MAX));
        }
        let mut kept = 0usize;
        for root in watched {
            s.walk(root, &mut kept);
        }
        s
    }

    fn walk(&mut self, path: &Path, kept: &mut usize) {
        if skipped(&self.world, path)
            || self
                .witnessed
                .iter()
                .any(|w| path == self.world.join(&w.rel))
        {
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
        let mut notes = Vec::new();
        let shown = |p: &Path| {
            p.strip_prefix(&self.world)
                .unwrap_or(p)
                .display()
                .to_string()
        };
        // Added since: removed, deepest first — but a folder the harness made for a witnessed
        // file, which that file's rebuild answers for.
        for path in now.entries.keys().rev() {
            let harness_folder = now.entries[path] == Entry::Dir
                && self
                    .witnessed
                    .iter()
                    .any(|w| self.world.join(&w.rel).starts_with(path));
            if !self.entries.contains_key(path) && !harness_folder {
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
        // Last: the witnessed files stand again, whatever the command made of them.
        notes.extend(self.witnessed.iter().filter_map(|w| self.rebuild(w)));
        notes
    }
}

/// The shared work queue, relative to the world.
const QUEUE: &str = ".mnemorph/shared/queue.jsonl";
/// The usage ledger, relative to the world.
const USAGE: &str = ".mnemorph/instruments/usage.jsonl";
/// The Kijin posts' folder, relative to the world.
const KIJIN: &str = ".mnemorph/kijin";
/// A post journal's name in its post's folder.
const POST_JOURNAL: &str = "post.jsonl";

impl Snapshot {
    /// Rebuild a witnessed file from what it held and what its witness gained since the
    /// snapshot, in the witness's order: whatever the command did to the file — a forged line, a
    /// deletion, a link in its place — never counts.
    fn rebuild(&self, w: &Witnessed) -> Option<String> {
        let path = self.world.join(&w.rel);
        // Held until the file is written: no line lands between the read and the write.
        let _lock = w.witness.as_deref().and_then(locked);
        let gained = w
            .witness
            .as_ref()
            .and_then(|f| fs::read(f).ok())
            .map(|f| f.get(w.witness_len..).unwrap_or_default().to_vec())
            .unwrap_or_default();
        let mut want = w.held.clone().unwrap_or_default();
        if !want.is_empty() && !gained.is_empty() && !want.ends_with(b"\n") {
            want.push(b'\n');
        }
        want.extend_from_slice(&gained);
        let absent = w.held.is_none() && want.is_empty();
        let meta = fs::symlink_metadata(&path);
        let unchanged = match &meta {
            Ok(m) if m.file_type().is_file() => fs::read(&path).is_ok_and(|now| now == want),
            Ok(_) => false,
            Err(_) => absent,
        };
        if unchanged {
            return None;
        }
        Some(match write_whole(&path, &want, absent) {
            Ok(()) => format!("put back {}", w.what),
            Err(e) => format!("could not put back {}: {e}", w.what),
        })
    }
}

/// The witness, locked whole until dropped; `None` when it does not exist (nothing to wait for).
fn locked(witness: &Path) -> Option<fs::File> {
    let f = fs::File::open(witness).ok()?;
    f.lock().ok()?;
    Some(f)
}

/// Write a witnessed file afresh: whatever stands at `path` — a link, a directory, a file — is
/// removed first, and the new file is created new (never through a link), under a real folder.
fn write_whole(path: &Path, bytes: &[u8], absent: bool) -> std::io::Result<()> {
    if let Ok(m) = fs::symlink_metadata(path) {
        if m.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    if absent {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        if fs::symlink_metadata(dir).is_ok_and(|m| !m.is_dir()) {
            fs::remove_file(dir)?;
        }
        fs::create_dir_all(dir)?;
    }
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    std::io::Write::write_all(&mut f, bytes)
}

/// Where the queue's witness lives: `$XDG_STATE_HOME/mnemorph/queue/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`. The world crate writes it at the same place
/// (`mnemorph_world::queue::witness_of`); a test in the loop holds the two together.
#[must_use]
pub fn queue_witness(world: &Path) -> Option<PathBuf> {
    Some(
        state_dir()?
            .join("mnemorph/queue")
            .join(format!("{}.jsonl", world_key(world))),
    )
}

/// Where the usage ledger's witness lives: `$XDG_STATE_HOME/mnemorph/usage/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` when there is no home. The loop's chained usage journal
/// writes every byte it appends here first (`Journal::witnessed_at`).
#[must_use]
pub fn usage_witness(world: &Path) -> Option<PathBuf> {
    Some(
        state_dir()?
            .join("mnemorph/usage")
            .join(format!("{}.jsonl", world_key(world))),
    )
}

/// Where a Kijin post journal's witness lives:
/// `$XDG_STATE_HOME/mnemorph/kijin/<world>-<fnv64a>/<post>.jsonl`, else under `~/.local/state`;
/// `None` when there is no home. Found again by the world's path, as the queue's and the attic's.
#[must_use]
pub fn kijin_witness(world: &Path, post: &str) -> Option<PathBuf> {
    Some(
        state_dir()?
            .join("mnemorph/kijin")
            .join(world_key(world))
            .join(format!("{post}.jsonl")),
    )
}

/// The human's state directory: `$XDG_STATE_HOME` when absolute, else `~/.local/state`.
pub(crate) fn state_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".local/state")))
}

/// `<world's name>-<fnv64a of its path>`.
pub(crate) fn world_key(world: &Path) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("{name}-{h:016x}")
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

    /// Whatever a body's command does to the queue, it comes back as what it held plus what its
    /// witness gained since, in the witness's order.
    #[test]
    fn the_queue_is_rebuilt_from_its_witness() {
        let root = std::env::temp_dir().join(format!("mnemorph-witness-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("w");
        fs::create_dir_all(world.join(".mnemorph/shared")).unwrap();
        let witness = root.join("witness.jsonl");
        let queue = world.join(QUEUE);
        let take = || {
            let mut s = Snapshot::take(&world);
            s.witnessed[0].witness = Some(witness.clone());
            s.witnessed[0].witness_len = fs::read(&witness).map_or(0, |w| w.len());
            s
        };
        fs::write(&queue, "old\n").unwrap();
        fs::write(&witness, "old\n").unwrap();

        // A forged line, the human's two moves reversed, one of them deleted.
        let before = take();
        fs::write(&witness, "old\nhuman-1\nhuman-2\n").unwrap();
        fs::write(&queue, "old\nforged\nhuman-2\n").unwrap();
        let notes = before.restore();
        assert_eq!(
            fs::read_to_string(&queue).unwrap(),
            "old\nhuman-1\nhuman-2\n"
        );
        assert!(
            notes.iter().any(|n| n.contains("put back the work queue")),
            "{notes:?}"
        );

        // A link in the queue's place, pointing outside: the target is never written.
        let outside = root.join("outside");
        fs::write(&outside, "mine\n").unwrap();
        let before = take();
        fs::remove_file(&queue).unwrap();
        std::os::unix::fs::symlink(&outside, &queue).unwrap();
        before.restore();
        assert!(fs::symlink_metadata(&queue).unwrap().file_type().is_file());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "mine\n");
        assert_eq!(
            fs::read_to_string(&queue).unwrap(),
            "old\nhuman-1\nhuman-2\n"
        );

        // The shelf deleted, or a link in its place.
        let before = take();
        fs::remove_dir_all(world.join(".mnemorph/shared")).unwrap();
        std::os::unix::fs::symlink(&root, world.join(".mnemorph/shared")).unwrap();
        before.restore();
        assert!(
            fs::symlink_metadata(world.join(".mnemorph/shared"))
                .unwrap()
                .is_dir()
        );
        assert_eq!(
            fs::read_to_string(&queue).unwrap(),
            "old\nhuman-1\nhuman-2\n"
        );
        assert!(!root.join("queue.jsonl").exists());

        // Untouched, nothing is said.
        assert_eq!(take().restore(), Vec::<String>::new());
        let _ = fs::remove_dir_all(&root);
    }

    /// A post journal comes back as what it held plus the rows the harness journaled while the
    /// command ran — its own forged rows gone — and a first row the harness wrote mid-command is
    /// neither lost nor reported as the command's.
    #[test]
    fn a_post_journal_keeps_the_harness_rows_and_loses_the_forged_ones() {
        let root = std::env::temp_dir().join(format!("mnemorph-post-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("w");
        fs::create_dir_all(world.join(".mnemorph")).unwrap();
        let witness = root.join("forge.jsonl");
        let journal = world.join(".mnemorph/kijin/kijin-forge/post.jsonl");
        let take = || {
            let mut s = Snapshot::take(&world);
            let w = s
                .witnessed
                .iter_mut()
                .find(|w| w.rel.ends_with("kijin-forge/post.jsonl"))
                .unwrap();
            w.witness = Some(witness.clone());
            w.witness_len = fs::read(&witness).map_or(0, |b| b.len());
            s
        };

        // No journal yet: the harness writes its first row while a command runs.
        let before = take();
        fs::create_dir_all(journal.parent().unwrap()).unwrap();
        fs::write(&witness, "committed\n").unwrap();
        fs::write(&journal, "committed\n").unwrap();
        assert_eq!(before.restore(), Vec::<String>::new());
        assert_eq!(fs::read_to_string(&journal).unwrap(), "committed\n");

        // A forged `observed` row, and the harness's own row journaled meanwhile.
        let before = take();
        fs::write(&witness, "committed\nreconcile\n").unwrap();
        fs::write(&journal, "committed\nobserved\nreconcile\n").unwrap();
        let notes = before.restore();
        assert_eq!(
            fs::read_to_string(&journal).unwrap(),
            "committed\nreconcile\n"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.contains("the kijin-forge post journal")),
            "{notes:?}"
        );

        // Deleted whole: it comes back.
        let before = take();
        fs::remove_dir_all(world.join(".mnemorph/kijin")).unwrap();
        before.restore();
        assert_eq!(
            fs::read_to_string(&journal).unwrap(),
            "committed\nreconcile\n"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// A command that rewrites the usage ledger loses nothing the harness wrote: a parallel run's
    /// rows, witnessed while the command ran, stand after it, and the command's own edit is gone.
    #[test]
    fn the_usage_ledger_keeps_the_rows_a_parallel_run_wrote() {
        let root = std::env::temp_dir().join(format!("mnemorph-usage-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("w");
        fs::create_dir_all(world.join(".mnemorph/instruments")).unwrap();
        let witness = root.join("usage.jsonl");
        let ledger = world.join(USAGE);
        let take = || {
            let mut s = Snapshot::take(&world);
            let w = s
                .witnessed
                .iter_mut()
                .find(|w| w.rel == Path::new(USAGE))
                .unwrap();
            w.witness = Some(witness.clone());
            w.witness_len = fs::read(&witness).map_or(0, |b| b.len());
            s
        };
        fs::write(&ledger, "row-1\n").unwrap();
        fs::write(&witness, "row-1\n").unwrap();
        let before = take();
        // While the command runs, a parallel run appends its row, through the witness first.
        fs::write(&witness, "row-1\nparallel\n").unwrap();
        fs::write(&ledger, "row-1\nparallel\n").unwrap();
        // The command edits the ledger.
        fs::write(&ledger, "forged\nparallel\n").unwrap();
        let notes = before.restore();
        assert_eq!(fs::read_to_string(&ledger).unwrap(), "row-1\nparallel\n");
        assert!(
            notes.iter().any(|n| n.contains("the usage ledger")),
            "{notes:?}"
        );
        // Untouched, with a parallel row: kept, nothing said.
        let before = take();
        fs::write(&witness, "row-1\nparallel\nmore\n").unwrap();
        fs::write(&ledger, "row-1\nparallel\nmore\n").unwrap();
        assert_eq!(before.restore(), Vec::<String>::new());
        assert_eq!(
            fs::read_to_string(&ledger).unwrap(),
            "row-1\nparallel\nmore\n"
        );
        assert!(usage_witness(&world).is_none_or(|p| p.ends_with(format!(
            "mnemorph/usage/w-{:016x}.jsonl",
            {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for b in world.as_os_str().as_encoded_bytes() {
                    h ^= u64::from(*b);
                    h = h.wrapping_mul(0x0100_0000_01b3);
                }
                h
            }
        ))));
        let _ = fs::remove_dir_all(&root);
    }
}
