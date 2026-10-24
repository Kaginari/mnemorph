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
}

/// One snapshot of the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The tree's id in the attic's store.
    pub tree: String,
}

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
        })
    }

    /// Where it keeps things.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
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

    /// Snapshot the working tree (and keep the world's repository) before something changes it.
    ///
    /// # Errors
    ///
    /// The attic cannot be written or git fails; the caller decides whether to go on.
    pub fn snapshot(&self, why: &str) -> Result<Snapshot, String> {
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
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
        // The command's own scratch is not worth keeping; everything else that is not ignored is.
        self.ok(&[
            "add",
            "-A",
            "--ignore-errors",
            "--",
            ".",
            ":(exclude).mnemorph/tmp",
        ])?;
        let tree = self.ok(&["write-tree"])?;
        self.keep_git()?;
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("snapshots.log"))
            .map_err(|e| format!("attic log: {e}"))?;
        let _ = writeln!(log, "{} {tree} {}", mnemorph_now(), why.replace('\n', " "));
        Ok(Snapshot { tree })
    }

    /// Files present in `snap` and gone from the world now.
    #[must_use]
    pub fn deleted_since(&self, snap: &Snapshot) -> Vec<String> {
        let Ok(out) = self.git(&["ls-tree", "-r", "--name-only", &snap.tree]) else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|p| fs::symlink_metadata(self.world.join(p)).is_err())
            .map(str::to_owned)
            .collect()
    }

    /// Put `paths` back as they were in `snap` (all of them when `paths` is empty).
    ///
    /// # Errors
    ///
    /// Git fails, or a path was not in the snapshot.
    pub fn restore(&self, snap: &Snapshot, paths: &[&str]) -> Result<(), String> {
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
