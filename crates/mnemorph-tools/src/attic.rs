//! The attic: what a command or a write took away, kept where no command can reach.
//!
//! Before every command and every file write the working tree is snapshotted into the attic's own
//! git object store — outside the world, so the kernel guard lets no command touch it. Only
//! changed files are hashed again, and ignored files (`node_modules`, `target`) are left out: they
//! rebuild. A deletion is reported with the snapshot that holds it, and any path restores from
//! it.
//!
//! The world's own `.git` cannot be snapshotted that way, so its objects — immutable, named by
//! their hash — are hard-linked into the attic and its refs copied: an `rm -rf .git` unlinks the
//! world's names and leaves the attic's, and the repository is put back.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
        Command::new("git")
            .args([
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
            .current_dir(&self.world)
            .output()
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

    /// Files present in `snap` and gone from the world now.
    #[must_use]
    pub fn deleted_since(&self, snap: &Snapshot) -> Vec<String> {
        let Ok(out) = self.git(&["ls-tree", "-r", "-z", "--name-only", &snap.tree]) else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
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

    /// Put `paths` back as they were in `snap` (all of them when `paths` is empty).
    ///
    /// # Errors
    ///
    /// Git fails, or a path was not in the snapshot.
    pub fn restore(&self, snap: &Snapshot, paths: &[&str]) -> Result<(), String> {
        let _held = self.lock()?;
        let mut args = vec!["checkout", snap.tree.as_str(), "--"];
        if paths.is_empty() {
            args.push(".");
        } else {
            args.extend_from_slice(paths);
        }
        self.ok(&args).map(|_| ())
    }

    /// Keep the world's repository: hard links to its objects (immutable), copies of its refs.
    fn keep_git(&self) -> Result<(), String> {
        let git = self.world.join(".git");
        // A worktree's or a submodule's `.git` is a file naming the repository elsewhere: only
        // that file is the world's to lose, and it is kept.
        if fs::symlink_metadata(&git).is_ok_and(|m| m.is_file()) {
            fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
            return fs::copy(&git, self.dir.join("dotgit-file"))
                .map(|_| ())
                .map_err(|e| format!("attic .git file: {e}"));
        }
        if !git.is_dir() {
            return Ok(());
        }
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
