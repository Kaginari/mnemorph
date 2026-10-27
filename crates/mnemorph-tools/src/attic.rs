//! The attic: what a command or a write took away, kept where no command can reach.
//!
//! Before every command and every file write the working tree is snapshotted into the attic's own
//! git object store — outside the world, so the kernel guard lets no command touch it. Only
//! changed files are hashed again, and ignored files (`node_modules`, `target`) are left out: they
//! rebuild. A deletion is reported with the snapshot that holds it. An act sealed with a second
//! snapshot when it ends can be undone, but only while every path it changed is still as it left
//! it: a later writer is kept, and the undo is refused whole.
//!
//! The world's own `.git` cannot be snapshotted that way, so its objects — immutable, named by
//! their hash — are hard-linked into the attic and its refs copied: an `rm -rf .git` unlinks the
//! world's names and leaves the attic's, and the repository is put back.

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead as _, Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStringExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};

use crate::pinned;

/// A world's attic.
#[derive(Debug, Clone)]
pub struct Attic {
    dir: PathBuf,
    world: PathBuf,
    keep_secs: u64,
    max_bytes: u64,
}

/// How long snapshots are kept, by default.
pub const KEEP_DAYS: u64 = 14;
/// The attic's size past which the oldest snapshots go, by default.
pub const MAX_MB: u64 = 2048;
/// How often the attic prunes itself, at most.
const PRUNE_EVERY_SECS: u64 = 24 * 60 * 60;

/// One snapshot of the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The tree's id in the attic's store.
    pub tree: String,
    /// Files too large to keep, left out of this snapshot.
    pub skipped: Vec<String>,
}

/// An act as the attic kept it: the snapshot before it, and the one it left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act {
    /// The world before the act.
    pub before: Snapshot,
    /// The world as the act left it.
    pub after: Snapshot,
}

/// What a path is, as an undo compares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Nothing there.
    Absent,
    /// A regular file: its content's blob id, and whether it is executable.
    File {
        /// The content's git blob id; empty when the size alone already differs, so the
        /// content was never read.
        blob: String,
        /// The owner's execute bit.
        executable: bool,
        /// Its size in bytes.
        size: u64,
    },
    /// A symlink, and where it points.
    Link {
        /// The link's target, as written.
        target: PathBuf,
    },
    /// A directory where a file or a link was.
    Dir,
    /// A fifo, a socket or a device.
    Other,
    /// Reached only through `lead`, a symlink or a file the act did not leave.
    Beyond {
        /// The leading path that is not a directory.
        lead: String,
    },
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Absent => f.write_str("nothing"),
            State::File {
                blob,
                executable,
                size,
            } => {
                let a = if *executable { "an executable" } else { "a" };
                if blob.is_empty() {
                    write!(f, "{a} file of {size} bytes")
                } else {
                    write!(f, "{a} file {} ({size} bytes)", &blob[..blob.len().min(12)])
                }
            }
            State::Link { target } => write!(f, "a symlink to {}", target.display()),
            State::Dir => f.write_str("a directory"),
            State::Other => f.write_str("a special file"),
            State::Beyond { lead } => write!(f, "a path beyond `{lead}`, not a directory"),
        }
    }
}

/// A path changed since the act: what the act left there, and what is there now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// The path, relative to the world.
    pub path: String,
    /// What the act left.
    pub left: State,
    /// What is there now.
    pub now: State,
}

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "`{}`: the act left {}, now {}",
            self.path, self.left, self.now
        )
    }
}

/// An undo's outcome.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restore {
    /// Every path was as the act left it, and is as it was before; these paths changed.
    Restored(Vec<String>),
    /// Some path changed since the act; nothing was undone.
    Conflict(Vec<Conflict>),
}

/// The attic's lock, held: restores made under it run in turn with every snapshot and restore
/// of the world, and the caller's own removals can share it. Released when dropped.
#[derive(Debug)]
pub struct Held {
    _file: fs::File,
    dir: PathBuf,
}

/// One path an act changed: its tree entry before and after (`None`: absent).
struct Change {
    path: String,
    before: Option<Entry>,
    after: Option<Entry>,
}

struct Entry {
    mode: String,
    oid: String,
}

/// A relative path with no empty, `.` or `..` component, and not inside `.git`.
fn plain(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && p.split('/')
            .all(|c| !c.is_empty() && c != "." && c != ".." && c != ".git")
}

/// What the attic never keeps, whatever the world ignores: build output and dependencies
/// rebuild, and the world's machine-local records are not its to keep.
const EXCLUDE: &[&str] = &[
    "target/",
    "node_modules/",
    ".venv/",
    "venv/",
    "dist/",
    "build/",
    "__pycache__/",
    ".gradle/",
    ".next/",
    ".nuxt/",
    ".cache/",
    ".tox/",
    "*.pyc",
    ".mnemorph/tmp/",
    ".mnemorph/instruments/",
    ".mnemorph/index/",
    ".mnemorph/intel/",
];

/// The largest file a snapshot keeps.
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

impl Attic {
    /// The attic for `world` under the human's state directory
    /// (`$XDG_STATE_HOME/mnemorph/attic/<world>`, else `~/.local/state/…`); `None` when there is
    /// no home or no `git`.
    #[must_use]
    pub fn for_world(world: &Path) -> Option<Self> {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::home_dir().map(|h| h.join(".local/state")))?;
        let mut h = DefaultHasher::new();
        world.hash(&mut h);
        let name = format!(
            "{}-{:016x}",
            world
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            h.finish()
        );
        Self::at(&state.join("mnemorph/attic").join(name), world)
    }

    /// An attic at `dir` for `world`; `None` when `git` is missing.
    #[must_use]
    pub fn at(dir: &Path, world: &Path) -> Option<Self> {
        Command::new("git")
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        Some(Self {
            dir: dir.to_path_buf(),
            world: world.to_path_buf(),
            keep_secs: KEEP_DAYS * 24 * 60 * 60,
            max_bytes: MAX_MB * 1024 * 1024,
        })
    }

    /// Keep snapshots `keep_days` days, and no more than `max_mb` megabytes of them.
    #[must_use]
    pub fn with_limits(mut self, keep_days: u64, max_mb: u64) -> Self {
        self.keep_secs = keep_days * 24 * 60 * 60;
        self.max_bytes = max_mb * 1024 * 1024;
        self
    }

    /// Where it keeps things.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The attic's lock, held while its index is in use: two runs on one world take their
    /// snapshots and restores in turn, never at once. Released when dropped.
    fn lock(&self) -> Result<fs::File, String> {
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.dir.join("lock"))
            .map_err(|e| format!("attic lock: {e}"))?;
        file.lock().map_err(|e| format!("attic lock: {e}"))?;
        Ok(file)
    }

    /// Git, pointed at the attic's store and the world's tree, deaf to every configuration but
    /// its own: no hooks, no filters, no fsmonitor from the world or the human's files.
    fn git(&self, args: &[&str]) -> std::io::Result<Output> {
        self.command(args).output()
    }

    /// The attic's git command, not yet run.
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.quotePath=false",
        ])
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("GIT_DIR", self.dir.join("store.git"))
        .env("GIT_WORK_TREE", &self.world)
        .env("GIT_INDEX_FILE", self.dir.join("index"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", &self.dir)
        .current_dir(&self.world);
        cmd
    }

    fn ok(&self, args: &[&str]) -> Result<String, String> {
        let out = self.git(args).map_err(|e| format!("git: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        } else {
            Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    fn ok_bytes(&self, args: &[&str]) -> Result<String, String> {
        let out = self.git(args).map_err(|e| format!("git: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    /// Snapshot the working tree (and keep the world's repository) before something changes it.
    ///
    /// # Errors
    ///
    /// The attic cannot be written or git fails; the caller decides whether to go on.
    pub fn snapshot(&self, why: &str) -> Result<Snapshot, String> {
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let _held = self.lock()?;
        if !self.dir.join("store.git/HEAD").exists() {
            let out = Command::new("git")
                .args(["init", "--bare", "-q"])
                .arg(self.dir.join("store.git"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .output()
                .map_err(|e| format!("git: {e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "git init: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
        }
        // The attic's own rules, out of every command's reach: what it never keeps, the files too
        // large to keep this time, and attributes switched off so a restore is byte for byte.
        let skipped = large_files(&self.world);
        let info = self.dir.join("store.git/info");
        fs::create_dir_all(&info).map_err(|e| format!("{}: {e}", info.display()))?;
        let mut exclude = EXCLUDE.join("\n");
        for big in &skipped {
            exclude.push_str("\n/");
            exclude.push_str(&big.replace(['*', '?', '[', '\\'], "?"));
        }
        fs::write(info.join("exclude"), exclude + "\n")
            .map_err(|e| format!("attic exclude: {e}"))?;
        fs::write(
            info.join("attributes"),
            "* -text -eol -ident -filter -diff -merge\n",
        )
        .map_err(|e| format!("attic attributes: {e}"))?;
        self.ok(&["add", "-A", "--ignore-errors", "--", "."])?;
        // What is ignored now leaves the index, even if an earlier snapshot kept it.
        let stale = self.ok_bytes(&[
            "ls-files",
            "-z",
            "--cached",
            "--ignored",
            "--exclude-standard",
        ])?;
        let stale: Vec<String> = stale
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        for chunk in stale.chunks(200) {
            let mut args = vec!["rm", "--cached", "-q", "--ignore-unmatch", "--"];
            args.extend(chunk.iter().map(String::as_str));
            self.ok(&args)?;
        }
        let tree = self.ok(&["write-tree"])?;
        // A ref per snapshot tells git which trees are still wanted, so pruning keeps them whole.
        let now = mnemorph_now();
        self.ok(&[
            "update-ref",
            &format!("refs/attic/{now:012}-{}", &tree[..12.min(tree.len())]),
            &tree,
        ])?;
        self.keep_git()?;
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("snapshots.log"))
            .map_err(|e| format!("attic log: {e}"))?;
        let _ = writeln!(log, "{now} {tree} {}", why.replace('\n', " "));
        self.prune_when_due(now);
        Ok(Snapshot { tree, skipped })
    }

    /// Prune at most once a day; a failed prune is retried the next day and never stops a
    /// snapshot.
    fn prune_when_due(&self, now: u64) {
        let stamp = self.dir.join("pruned");
        let last = fs::read_to_string(&stamp)
            .ok()
            .and_then(|t| t.trim().parse::<u64>().ok())
            .unwrap_or(0);
        if now.saturating_sub(last) >= PRUNE_EVERY_SECS {
            let _ = fs::write(&stamp, now.to_string());
            let _ = self.prune(now);
        }
    }

    /// Drop snapshots older than the keep window, then the oldest while the attic is over its
    /// size (the newest always stays); pack what remains; forget kept copies of `.git` objects
    /// the world itself no longer has, once they are past the window. Returns the snapshot
    /// store's size after, in bytes — the cap measures only that store: the kept `.git` objects
    /// are hard links, and cost no disk beside the world's own.
    ///
    /// # Errors
    ///
    /// Git fails.
    pub fn prune(&self, now: u64) -> Result<u64, String> {
        let refs = self.ok(&["for-each-ref", "--format=%(refname)", "refs/attic/"])?;
        let mut refs: Vec<(u64, String)> = refs
            .lines()
            .filter_map(|r| {
                let ts = r
                    .strip_prefix("refs/attic/")?
                    .split('-')
                    .next()?
                    .parse()
                    .ok()?;
                Some((ts, r.to_owned()))
            })
            .collect();
        refs.sort();
        let cutoff = now.saturating_sub(self.keep_secs);
        let keep_from = refs
            .iter()
            .position(|(ts, _)| *ts >= cutoff)
            .unwrap_or(refs.len())
            .min(refs.len().saturating_sub(1));
        for (_, r) in refs.drain(..keep_from) {
            self.ok(&["update-ref", "-d", &r])?;
        }
        self.ok(&["gc", "--quiet", "--prune=now"])?;
        while dir_size(&self.dir.join("store.git")) > self.max_bytes && refs.len() > 1 {
            let drop = (refs.len() / 4).max(1).min(refs.len() - 1);
            for (_, r) in refs.drain(..drop) {
                self.ok(&["update-ref", "-d", &r])?;
            }
            self.ok(&["gc", "--quiet", "--prune=now"])?;
        }
        forget_old_links(
            &self.dir.join("dotgit/objects"),
            &self.world.join(".git/objects"),
            cutoff,
        );
        trim_log(&self.dir.join("snapshots.log"), cutoff);
        Ok(dir_size(&self.dir.join("store.git")))
    }

    /// Files present in `snap` and gone from the world now: each one kept, so restorable.
    #[must_use]
    pub fn deleted_since(&self, snap: &Snapshot) -> Vec<String> {
        self.gone_since(snap, false)
    }

    /// Nested repositories present in `snap` and gone now: held only as gitlinks, so lost — a
    /// delete says so, never that they are kept.
    #[must_use]
    pub fn lost_since(&self, snap: &Snapshot) -> Vec<String> {
        self.gone_since(snap, true)
    }

    /// The entries of `snap` gone from the world: its gitlinks, or everything else.
    fn gone_since(&self, snap: &Snapshot, gitlinks: bool) -> Vec<String> {
        let Ok(out) = self.git(&["ls-tree", "-r", "-z", &snap.tree]) else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter_map(|line| {
                let (meta, path) = line.split_once('\t')?;
                (meta.starts_with("160000 ") == gitlinks).then_some(path)
            })
            .filter(|p| fs::symlink_metadata(self.world.join(p)).is_err())
            .map(str::to_owned)
            .collect()
    }

    /// Whether `snap` holds the file `rel` (relative to the world): what a delete must know
    /// before it may say a copy is kept.
    #[must_use]
    ///
    /// A nested repository is held as a gitlink — its commit's name, none of its files — so it
    /// is never held: a delete says so, instead of promising a copy that restores empty.
    pub fn holds(&self, snap: &Snapshot, rel: &str) -> bool {
        self.git(&["ls-tree", snap.tree.as_str(), "--", rel])
            .is_ok_and(|o| {
                let line = String::from_utf8_lossy(&o.stdout);
                let line = line.trim();
                o.status.success()
                    && !line.starts_with("160000 ")
                    && line.split_once('\t').is_some_and(|(_, name)| name == rel)
            })
    }

    /// Close an act: snapshot what it left, beside the snapshot taken before it. Only a sealed
    /// act can be undone, because an undo first checks that the world is still as the act left
    /// it.
    ///
    /// # Errors
    ///
    /// The snapshot fails.
    pub fn seal(&self, before: Snapshot, why: &str) -> Result<Act, String> {
        let after = self.snapshot(why)?;
        Ok(Act { before, after })
    }

    /// Take the attic's lock and keep it until the returned guard drops.
    ///
    /// # Errors
    ///
    /// The lock file cannot be made or locked.
    pub fn hold(&self) -> Result<Held, String> {
        Ok(Held {
            _file: self.lock()?,
            dir: self.dir.clone(),
        })
    }

    /// Undo `act` on `paths` (everything it changed when `paths` is empty), but only if each
    /// path it changed there is still as the act left it — content, mode, a link's target, or
    /// its absence. One path changed since, by anyone, and nothing is undone: the later writer
    /// is kept and every such path is named. A path the act created is removed; one it changed
    /// or deleted is written back. Every look and every write goes through directory handles
    /// opened beneath the world, following no symlink: a path under a symlink or a file the act
    /// did not leave is a conflict, and a component swapped for a symlink after the check makes
    /// the write fail. The outcome is written to the attic's log. Takes the attic's lock.
    ///
    /// # Errors
    ///
    /// A path is not a plain relative path inside the world, a changed path was too large for
    /// either snapshot to keep, a snapshot is gone, or git or the filesystem fails — a write
    /// that fails part-way is journaled with what was restored and what was not.
    pub fn restore(&self, act: &Act, paths: &[&str]) -> Result<Restore, String> {
        let held = self.hold()?;
        self.restore_held(&held, act, paths)
    }

    /// [`Attic::restore`] under a lock the caller already holds.
    ///
    /// # Errors
    ///
    /// As [`Attic::restore`]; and the lock is another attic's.
    pub fn restore_held(&self, held: &Held, act: &Act, paths: &[&str]) -> Result<Restore, String> {
        if held.dir != self.dir {
            return Err("the lock held is another attic's".into());
        }
        for p in paths {
            if *p != "." && !plain(p) {
                return Err(format!(
                    "`{p}` is not a plain relative path inside the world"
                ));
            }
        }
        let changes: Vec<Change> = self
            .changes(act)?
            .into_iter()
            .filter(|c| {
                paths.is_empty()
                    || paths
                        .iter()
                        .any(|p| *p == "." || c.path == *p || c.path.starts_with(&format!("{p}/")))
            })
            .collect();
        for c in &changes {
            if !plain(&c.path) {
                return Err(format!("the attic names `{}`, not a plain path", c.path));
            }
            if act.before.skipped.contains(&c.path) || act.after.skipped.contains(&c.path) {
                return Err(format!(
                    "`{}` was too large for the attic to keep, so the act cannot be undone there",
                    c.path
                ));
            }
        }
        let root = pinned::root(&self.world).map_err(|e| format!("the world: {e}"))?;
        let mut batch = Batch::new(self, false)?;
        let mut check = Batch::new(self, true)?;
        let removed: HashSet<&str> = changes
            .iter()
            .filter(|c| c.before.is_none())
            .map(|c| c.path.as_str())
            .collect();
        let undone: HashSet<&str> = changes.iter().map(|c| c.path.as_str()).collect();
        let mut conflicts = Vec::new();
        for c in &changes {
            let left = batch.state(&mut check, c.after.as_ref())?;
            let now = self.state_now(&root, &c.path, &left, &undone, &removed)?;
            if left != now {
                conflicts.push(Conflict {
                    path: c.path.clone(),
                    left,
                    now,
                });
            }
        }
        if !conflicts.is_empty() {
            let named: Vec<String> = conflicts.iter().map(ToString::to_string).collect();
            self.journal(act, &format!("refused: {}", named.join("; ")));
            return Ok(Restore::Conflict(conflicts));
        }
        #[cfg(test)]
        tests::between();
        // Every path is as the act left it: what it created goes, then what it changed or
        // deleted comes back. Removals first, so a file the act put where a directory was is
        // gone before the directory is made again.
        let mut done: Vec<&str> = Vec::new();
        let order = changes
            .iter()
            .filter(|c| c.before.is_none())
            .chain(changes.iter().filter(|c| c.before.is_some()));
        for c in order {
            if let Err(e) = Self::undo_one(&root, &mut batch, c) {
                let rest: Vec<&str> = changes
                    .iter()
                    .map(|c| c.path.as_str())
                    .filter(|p| !done.contains(p))
                    .collect();
                let why = format!(
                    "failed at `{}`: {e}; restored: {}; not restored: {}",
                    c.path,
                    done.join(", "),
                    rest.join(", ")
                );
                self.journal(act, &why);
                return Err(why);
            }
            done.push(&c.path);
        }
        let restored: Vec<String> = changes.into_iter().map(|c| c.path).collect();
        self.journal(act, &format!("restored: {}", restored.join(", ")));
        Ok(Restore::Restored(restored))
    }

    /// Undo one change, through a handle on its parent opened beneath the world.
    fn undo_one(root: &OwnedFd, batch: &mut Batch, c: &Change) -> Result<(), String> {
        let parts: Vec<&str> = c.path.split('/').collect();
        let name = parts.last().copied().unwrap_or_default();
        let io = |e: std::io::Error| e.to_string();
        let Some(entry) = &c.before else {
            return match pinned::parent(root, &parts, false) {
                Ok(dir) => pinned::unlink(&dir, name).map_err(io),
                // The act's parent went with the act's other removals: nothing left to remove.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(io(e)),
            };
        };
        let dir = pinned::parent(root, &parts, true).map_err(io)?;
        let replace = c.after.is_some();
        let (size, content) = batch.open(&entry.oid)?;
        let mut content = content.take(size);
        let placed = if entry.mode == "120000" {
            let mut target = Vec::new();
            std::io::Read::read_to_end(&mut content, &mut target).map_err(io)?;
            let target = PathBuf::from(std::ffi::OsString::from_vec(target));
            pinned::put(&dir, name, replace, |d, tmp| {
                pinned::link_new(d, tmp, &target)
            })
        } else {
            let executable = entry.mode == "100755";
            pinned::put(&dir, name, replace, |d, tmp| {
                pinned::write_new(d, tmp, &mut content, executable)
            })
        };
        // The rest of the blob, and git's newline after it, are read whatever happened.
        let _ = std::io::copy(&mut content, &mut std::io::sink());
        batch.end_object()?;
        placed.map_err(io)
    }

    /// What `act` changed, path by path; a nested repository's gitlink is never held, so never
    /// undone.
    fn changes(&self, act: &Act) -> Result<Vec<Change>, String> {
        let raw = self.ok_bytes(&[
            "diff-tree",
            "-r",
            "-z",
            "--no-renames",
            act.before.tree.as_str(),
            act.after.tree.as_str(),
        ])?;
        let mut fields = raw.split('\0');
        let mut out = Vec::new();
        while let Some(meta) = fields.next().filter(|m| !m.is_empty()) {
            let path = fields
                .next()
                .ok_or("git diff-tree: a change without a path")?;
            let parts: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
            let [old_mode, new_mode, old_oid, new_oid, ..] = parts[..] else {
                return Err(format!("git diff-tree: unreadable `{meta}`"));
            };
            if old_mode == "160000" || new_mode == "160000" {
                continue;
            }
            let entry = |mode: &str, oid: &str| {
                (mode != "000000").then(|| Entry {
                    mode: mode.to_owned(),
                    oid: oid.to_owned(),
                })
            };
            out.push(Change {
                path: path.to_owned(),
                before: entry(old_mode, old_oid),
                after: entry(new_mode, new_oid),
            });
        }
        Ok(out)
    }

    /// One path's state now, read through handles beneath the world. A leading component that
    /// is not a directory hides the path, unless this undo removes it; a directory holding only
    /// what this undo removes is as good as absent. A file is hashed only when it is what `left`
    /// says it should be — a regular file of the same size — so a fifo or a grown file is never
    /// read.
    fn state_now(
        &self,
        root: &OwnedFd,
        rel: &str,
        left: &State,
        undone: &HashSet<&str>,
        removed: &HashSet<&str>,
    ) -> Result<State, String> {
        use rustix::fs::FileType;
        let io = |e: std::io::Error| format!("{rel}: {e}");
        let parts: Vec<&str> = rel.split('/').collect();
        let name = parts.last().copied().unwrap_or_default();
        let dir = match pinned::walk(root, &parts).map_err(io)? {
            pinned::Walk::Parent(dir) => dir,
            pinned::Walk::Absent => return Ok(State::Absent),
            pinned::Walk::Blocked(lead) if undone.contains(lead.as_str()) => {
                return Ok(State::Absent);
            }
            pinned::Walk::Blocked(lead) => return Ok(State::Beyond { lead }),
        };
        let Some(st) = pinned::stat(&dir, name).map_err(io)? else {
            return Ok(State::Absent);
        };
        Ok(match pinned::kind(&st) {
            FileType::Symlink => State::Link {
                target: pinned::read_link(&dir, name).map_err(io)?,
            },
            FileType::RegularFile => {
                let executable = st.st_mode & 0o100 != 0;
                let size = u64::try_from(st.st_size).unwrap_or(0);
                let blob = match left {
                    State::File { size: want, .. } if *want == size => {
                        match pinned::open_regular(&dir, name, size).map_err(io)? {
                            Some(file) => self.hash(file)?,
                            None => return Ok(State::Other),
                        }
                    }
                    _ => String::new(),
                };
                State::File {
                    blob,
                    executable,
                    size,
                }
            }
            FileType::Directory
                if pinned::holds_only(&dir, name, rel, &|p| removed.contains(p)) =>
            {
                State::Absent
            }
            FileType::Directory => State::Dir,
            _ => State::Other,
        })
    }

    /// The blob id of what `file` holds, read from its handle, in the store's own hash.
    fn hash(&self, file: fs::File) -> Result<String, String> {
        let out = self
            .command(&["hash-object", "-t", "blob", "--no-filters", "--stdin"])
            .stdin(file)
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        } else {
            Err(format!(
                "git hash-object: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    /// Write an undo's outcome to the attic's log.
    fn journal(&self, act: &Act, what: &str) {
        if let Ok(mut log) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("snapshots.log"))
        {
            let _ = writeln!(
                log,
                "{} restore {}..{} {}",
                mnemorph_now(),
                act.before.tree,
                act.after.tree,
                what.replace('\n', " ")
            );
        }
    }

    /// Keep the world's repository: hard links to its objects (immutable), copies of its refs.
    fn keep_git(&self) -> Result<(), String> {
        let git = self.world.join(".git");
        // A worktree's or a submodule's `.git` is a file naming the repository elsewhere: only
        // that file is the world's to lose, and it is kept. Only the form `.git` has now is kept:
        // a world that turned one into the other is never repaired back to the old one.
        if fs::symlink_metadata(&git).is_ok_and(|m| m.is_file()) {
            fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
            let _ = fs::remove_dir_all(self.dir.join("dotgit"));
            return fs::copy(&git, self.dir.join("dotgit-file"))
                .map(|_| ())
                .map_err(|e| format!("attic .git file: {e}"));
        }
        if !git.is_dir() {
            return Ok(());
        }
        let _ = fs::remove_file(self.dir.join("dotgit-file"));
        let mirror = self.dir.join("dotgit");
        link_tree(&git.join("objects"), &mirror.join("objects"))?;
        for small in ["HEAD", "packed-refs", "config", "description"] {
            let from = git.join(small);
            if from.is_file() {
                let _ = fs::copy(&from, mirror.join(small));
            }
        }
        let refs = mirror.join("refs");
        let _ = fs::remove_dir_all(&refs);
        copy_tree(&git.join("refs"), &refs)?;
        Ok(())
    }

    /// If the world's repository is gone or broken, put it back from the attic. Says what it did.
    #[must_use]
    pub fn repair_git(&self) -> Option<String> {
        let mirror = self.dir.join("dotgit");
        let git = self.world.join(".git");
        let kept_file = self.dir.join("dotgit-file");
        if kept_file.is_file() && fs::symlink_metadata(&git).is_err() {
            return Some(match fs::copy(&kept_file, &git) {
                Ok(_) => "the repository's .git file (a worktree's or a submodule's) was deleted; it was put back from the attic".into(),
                Err(e) => format!("the repository's .git file was deleted, and the attic could not put it back: {e}"),
            });
        }
        if !mirror.join("HEAD").is_file() {
            return None;
        }
        let healthy =
            git.join("HEAD").is_file() && git.join("objects").is_dir() && git.join("refs").is_dir();
        if healthy {
            return None;
        }
        let restored = fs::create_dir_all(&git)
            .and_then(|()| {
                link_tree(&mirror.join("objects"), &git.join("objects"))
                    .map_err(std::io::Error::other)
            })
            .and_then(|()| {
                copy_tree(&mirror.join("refs"), &git.join("refs")).map_err(std::io::Error::other)
            })
            .and_then(|()| {
                for small in ["HEAD", "packed-refs", "config", "description"] {
                    let from = mirror.join(small);
                    if from.is_file() && !git.join(small).is_file() {
                        fs::copy(&from, git.join(small))?;
                    }
                }
                Ok(())
            });
        Some(match restored {
            Ok(()) => "the repository (.git) was deleted or broken; it was put back from the attic — run `git status` to see the working tree".into(),
            Err(e) => format!("the repository (.git) was deleted or broken, and the attic could not put it back: {e}"),
        })
    }
}

/// One `git cat-file --batch` for a whole restore: each object's size and content, asked one at
/// a time and streamed, never held whole in memory.
struct Batch {
    child: Child,
    input: ChildStdin,
    output: std::io::BufReader<ChildStdout>,
}

impl Batch {
    /// `check`: sizes only (`--batch-check`), no content.
    fn new(attic: &Attic, check: bool) -> Result<Self, String> {
        let mut child = attic
            .command(&["cat-file", if check { "--batch-check" } else { "--batch" }])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("git cat-file: {e}"))?;
        let input = child.stdin.take().ok_or("git cat-file: no stdin")?;
        let output = std::io::BufReader::new(child.stdout.take().ok_or("git cat-file: no stdout")?);
        Ok(Self {
            child,
            input,
            output,
        })
    }

    /// Ask for `oid`: its size, and a reader on its content (read exactly that many bytes,
    /// then call [`Batch::end_object`]).
    fn open(&mut self, oid: &str) -> Result<(u64, &mut std::io::BufReader<ChildStdout>), String> {
        let lost = |e: std::io::Error| format!("git cat-file: {e}");
        writeln!(self.input, "{oid}").map_err(lost)?;
        self.input.flush().map_err(lost)?;
        let mut header = String::new();
        self.output.read_line(&mut header).map_err(lost)?;
        let size = header
            .trim_end()
            .rsplit(' ')
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .filter(|_| header.split(' ').nth(1) == Some("blob"))
            .ok_or_else(|| format!("the attic lost `{oid}`: {}", header.trim_end()))?;
        Ok((size, &mut self.output))
    }

    /// Read the newline git writes after an object's content.
    fn end_object(&mut self) -> Result<(), String> {
        let mut nl = [0u8; 1];
        self.output
            .read_exact(&mut nl)
            .map_err(|e| format!("git cat-file: {e}"))
    }

    /// The state a tree entry records: a link's target from this (content) batch, a file's
    /// size from `check`.
    fn state(&mut self, check: &mut Batch, entry: Option<&Entry>) -> Result<State, String> {
        let Some(e) = entry else {
            return Ok(State::Absent);
        };
        if e.mode != "120000" {
            let (size, _) = check.open(&e.oid)?;
            return Ok(State::File {
                blob: e.oid.clone(),
                executable: e.mode == "100755",
                size,
            });
        }
        let (size, content) = self.open(&e.oid)?;
        let mut target = Vec::new();
        content
            .take(size)
            .read_to_end(&mut target)
            .map_err(|err| format!("git cat-file: {err}"))?;
        self.end_object()?;
        Ok(State::Link {
            target: PathBuf::from(std::ffi::OsString::from_vec(target)),
        })
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Hard-link every file under `from` into `to` that is not there yet (copy across devices).
fn link_tree(from: &Path, to: &Path) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(());
    };
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for e in entries.filter_map(Result::ok) {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() {
            link_tree(&src, &dst)?;
        } else if kind.is_file() && fs::symlink_metadata(&dst).is_err() {
            fs::hard_link(&src, &dst)
                .or_else(|_| fs::copy(&src, &dst).map(|_| ()))
                .map_err(|err| format!("{}: {err}", dst.display()))?;
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(());
    };
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for e in entries.filter_map(Result::ok) {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() {
            copy_tree(&src, &dst)?;
        } else if kind.is_file() {
            fs::copy(&src, &dst).map_err(|err| format!("{}: {err}", dst.display()))?;
        }
    }
    Ok(())
}

fn mnemorph_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Bytes under `dir`.
fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

/// Remove kept `.git` object copies that only the attic still names, once older than `cutoff`.
fn forget_old_links(dir: &Path, world: &Path, cutoff: u64) {
    use std::os::unix::fs::MetadataExt as _;
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.filter_map(Result::ok) {
        let Ok(meta) = e.metadata() else { continue };
        let theirs = world.join(e.file_name());
        if meta.is_dir() {
            forget_old_links(&e.path(), &theirs, cutoff);
        } else if !theirs.exists() && u64::try_from(meta.mtime()).unwrap_or(0) < cutoff {
            // Only a copy the world itself no longer has, and only once past the window —
            // whether the copy is a hard link or, across devices, a real copy.
            let _ = fs::remove_file(e.path());
        }
    }
}

/// Keep only the snapshot log's lines inside the window.
fn trim_log(log: &Path, cutoff: u64) {
    let Ok(text) = fs::read_to_string(log) else {
        return;
    };
    let kept: String = text
        .lines()
        .filter(|l| {
            l.split(' ')
                .next()
                .and_then(|t| t.parse::<u64>().ok())
                .is_some_and(|t| t >= cutoff)
        })
        .flat_map(|l| [l, "\n"])
        .collect();
    let _ = fs::write(log, kept);
}

/// Files under the world, not ignored, larger than [`MAX_FILE_BYTES`], relative to it.
fn large_files(world: &Path) -> Vec<String> {
    let skip: Vec<&str> = EXCLUDE.iter().filter_map(|e| e.strip_suffix('/')).collect();
    ignore::WalkBuilder::new(world)
        .hidden(false)
        .filter_entry(move |e| e.file_name() != ".git" && !skip.iter().any(|s| e.file_name() == *s))
        .build()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_some_and(|t| t.is_file())
                && e.metadata().is_ok_and(|m| m.len() > MAX_FILE_BYTES)
        })
        .filter_map(|e| {
            e.path()
                .strip_prefix(world)
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{Attic, Restore};

    thread_local! {
        static BETWEEN: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    }

    /// Run the test's racer, if any: after the check, before the writes.
    pub(super) fn between() {
        if let Some(racer) = BETWEEN.with(|b| b.borrow_mut().take()) {
            racer();
        }
    }

    struct Ground(PathBuf);

    impl Drop for Ground {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A world whose `src` the act changes; then a racer swaps `src` for a symlink to a
    /// directory outside, holding the same files, after the check and before the writes.
    fn raced(name: &str, act: impl FnOnce(&Path)) -> (Ground, Result<Restore, String>) {
        let root =
            std::env::temp_dir().join(format!("mnemorph-race-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world/src")).unwrap();
        fs::write(root.join("world/src/a.rs"), "before\n").unwrap();
        let world = root.join("world");
        let attic = Attic::at(&root.join("attic"), &world).expect("git");
        let before = attic.snapshot("before").unwrap();
        act(&world);
        let act = attic.seal(before, "after").unwrap();
        let (w, outside) = (world.clone(), root.join("outside"));
        BETWEEN.with(|b| {
            *b.borrow_mut() = Some(Box::new(move || {
                fs::rename(w.join("src"), &outside).unwrap();
                std::os::unix::fs::symlink(&outside, w.join("src")).unwrap();
            }));
        });
        let r = attic.restore(&act, &[]);
        (Ground(root), r)
    }

    fn listing(dir: &Path) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                let name = e.file_name().to_string_lossy().into_owned();
                (name, fs::read_to_string(e.path()).unwrap_or_default())
            })
            .collect();
        v.sort();
        v
    }

    /// A write raced by a swap goes nowhere: the file outside keeps what it held, and no
    /// temporary is left there.
    #[test]
    fn a_parent_swapped_after_the_check_is_never_written_through() {
        let (g, r) = raced("write", |w| {
            fs::write(w.join("src/a.rs"), "the act\n").unwrap();
        });
        assert!(r.is_err(), "{r:?}");
        assert_eq!(
            listing(&g.0.join("outside")),
            [("a.rs".to_owned(), "the act\n".to_owned())]
        );
        let log = fs::read_to_string(g.0.join("attic/snapshots.log")).unwrap();
        assert!(
            log.lines().last().unwrap().contains("failed at `src/a.rs`")
                && log.contains("not restored: src/a.rs"),
            "{log}"
        );
    }

    /// A removal raced by a swap removes nothing outside.
    #[test]
    fn a_parent_swapped_after_the_check_is_never_removed_through() {
        let (g, r) = raced("unlink", |w| {
            fs::write(w.join("src/new.rs"), "the act\n").unwrap();
        });
        assert!(r.is_err(), "{r:?}");
        assert_eq!(
            listing(&g.0.join("outside")),
            [
                ("a.rs".to_owned(), "before\n".to_owned()),
                ("new.rs".to_owned(), "the act\n".to_owned())
            ]
        );
    }
}
