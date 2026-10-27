//! The plan an approval is bound to: what the world looked like when the human said yes.
//!
//! A human may answer minutes after the ask, and other courts keep working meanwhile. An
//! approval is therefore given to a [`Fingerprint`] — the act, the commit HEAD names, the
//! working tree as git would see it, git's own configuration and executables, and where an
//! outward act lands — and the bash tool recomputes it just before the command is spawned. Any
//! difference and the command does not run: the gate asks again, saying what changed. A plan
//! that cannot be read is no plan: the command is refused.
//!
//! The fingerprint reads the world through git, deaf to anything git could run (no fsmonitor, no
//! hooks, no filters: the index is listed, never refreshed, and the working tree's bytes are
//! hashed here, not by git), and opens each file one component at a time from its root,
//! following no symlink, without blocking, and reading no further than the size it had.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Read as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rustix::fs::{AtFlags, FileType, Mode, OFlags, openat, readlinkat, statat};
use sha2::{Digest, Sha256};

use crate::beneath;
use crate::shell::scrubbed_env;

/// How many hex digits of each part the short form shows.
const SHORT: usize = 8;
/// How many hex digits name the whole plan.
const ID: usize = 12;
/// The configuration that decides where a push or a fetch goes, by key prefix.
const TARGET_KEYS: &[&str] = &["remote.", "branch.", "push.", "url."];
/// Files that steer a publish or carry its secrets, bound by name wherever they sit beside the
/// world's files, whatever the ignore rules say.
const NAMED: &[&str] = &[".npmrc", ".yarnrc", ".yarnrc.yml", ".env"];
/// The same, one folder down.
const NAMED_CARGO: &[&str] = &[".cargo/config", ".cargo/config.toml"];
/// The pieces of a git directory that run code or steer git.
const GIT_PIECES: &[&str] = &["config", "config.worktree", "hooks", "info"];
/// How many entries git's hooks and info may hold before the plan refuses to read them.
const GIT_ENTRIES: usize = 10_000;
/// How deep submodules are followed.
const SUBMODULE_DEPTH: usize = 8;

/// What an approval was given against. Two fingerprints are equal exactly when every part is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// SHA-256 of the act, as it will run.
    pub act: String,
    /// The commit HEAD names; `None` outside a repository or before the first commit.
    pub head: Option<String>,
    /// SHA-256 over the index (what is staged), every tracked file's bytes in the working tree
    /// (what is not), each submodule's own HEAD and tree, every untracked file git does not
    /// ignore, and the publish files by name (`.npmrc`, `.yarnrc`, `.yarnrc.yml`, `.env`,
    /// `.env.*`, `.cargo/config`, `.cargo/config.toml`) ignored or not. A world with no
    /// repository hashes every file its ignore files leave in, and the publish files.
    pub diff: String,
    /// SHA-256 over git's whole configuration as git reads it (every scope, includes expanded)
    /// and the bytes of the git directory's `config`, `config.worktree`, `hooks/` and `info/`
    /// (the common directory's too, in a worktree), and a `.git` file.
    pub git: String,
    /// SHA-256 over where an outward act lands: the branch HEAD names, every ref and the commit
    /// it names, the remote, branch, push and url configuration (every scope git reads), each
    /// remote's resolved push URLs, and `GH_REPO` and `GH_HOST`.
    pub target: String,
}

impl Fingerprint {
    /// The fingerprint of running `act` in `world`, now.
    ///
    /// # Errors
    ///
    /// What could not be read: a git command that failed (a corrupt index, a broken repository)
    /// leaves no plan to approve, and the caller refuses.
    pub fn of(act: &str, world: &Path) -> Result<Self, String> {
        Self::of_except(act, world, &[])
    }

    /// The fingerprint of running `act` in `world`, now, reading the files in `made` — the empty
    /// files the harness itself made, after the approval, for the sandbox to open — as the
    /// absence they were when the human was asked. Only a world inside a larger repository can
    /// see them: they are outside the world.
    ///
    /// # Errors
    ///
    /// As [`Self::of`].
    pub fn of_except(act: &str, world: &Path, made: &[PathBuf]) -> Result<Self, String> {
        let act = hex(&Sha256::digest(act.as_bytes()));
        let made: Vec<PathBuf> = made
            .iter()
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
            .collect();
        Ok(match toplevel(world)? {
            Some(top) => Self {
                act,
                head: head(&top)?,
                diff: repo_diff(&top, &made, 0)?,
                git: git_pieces(&top)?,
                target: target(&top)?,
            },
            None => Self {
                act,
                head: None,
                diff: tree_diff(world),
                git: hex(&Sha256::digest(b"no repository")),
                target: hex(&Sha256::digest(b"no repository")),
            },
        })
    }

    /// The plan's name: the first digits of a hash over every part.
    #[must_use]
    pub fn id(&self) -> String {
        let mut h = Sha256::new();
        for part in [
            self.act.as_str(),
            self.head.as_deref().unwrap_or("none"),
            &self.diff,
            &self.git,
            &self.target,
        ] {
            h.update(part.as_bytes());
            h.update([0]);
        }
        hex(&h.finalize())[..ID].to_owned()
    }

    /// The short form a human reads on the approval card and in the journal:
    /// `plan 3f9a1c2e4b5d · head 1a2b3c4d · diff 5d6e7f8a · git 0c1d2e3f · target 9a8b7c6d`.
    #[must_use]
    pub fn short(&self) -> String {
        format!(
            "plan {} · head {} · diff {} · git {} · target {}",
            self.id(),
            self.head.as_deref().map_or_else(|| "none".into(), cut),
            cut(&self.diff),
            cut(&self.git),
            cut(&self.target)
        )
    }

    /// What differs in `now`, part by part (`diff 5d6e7f8a → 0b1c2d3e`); empty when nothing does.
    #[must_use]
    pub fn changes(&self, now: &Self) -> Vec<String> {
        let mut changed = Vec::new();
        if self.head != now.head {
            changed.push(format!(
                "head moved {} → {}",
                self.head.as_deref().map_or_else(|| "none".into(), cut),
                now.head.as_deref().map_or_else(|| "none".into(), cut)
            ));
        }
        if self.diff != now.diff {
            changed.push(format!(
                "the working tree changed: diff {} → {}",
                cut(&self.diff),
                cut(&now.diff)
            ));
        }
        if self.git != now.git {
            changed.push(format!(
                "git's configuration, hooks or info changed: git {} → {}",
                cut(&self.git),
                cut(&now.git)
            ));
        }
        if self.target != now.target {
            changed.push(format!(
                "the target changed (a ref, the branch, or the remote configuration): target {} → {}",
                cut(&self.target),
                cut(&now.target)
            ));
        }
        if self.act != now.act {
            changed.push("the act changed".into());
        }
        changed
    }
}

fn cut(digest: &str) -> String {
    digest.chars().take(SHORT).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// What a git command gave: its exit code, its output, and its complaint.
struct Ran {
    code: Option<i32>,
    out: Vec<u8>,
    err: String,
}

/// Git, read-only and deaf to what it could run: the world's own fsmonitor, hooks and untracked
/// cache are off, and nothing here makes git read a file through a filter. The environment is
/// the one the command itself gets, so git resolves the same repository and configuration.
fn run_git(dir: &Path, args: &[&str]) -> io::Result<Ran> {
    let out = Command::new("git")
        .args([
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(scrubbed_env())
        .stdin(Stdio::null())
        .output()?;
    Ok(Ran {
        code: out.status.code(),
        out: out.stdout,
        err: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
    })
}

/// A git command that must succeed: any failure is the plan's.
fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let ran = run_git(dir, args).map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if ran.code == Some(0) {
        Ok(ran.out)
    } else {
        Err(format!("git {} failed: {}", args.join(" "), ran.err))
    }
}

/// The repository's root, when `world` is in one; `None` when it is not, or git is missing.
fn toplevel(world: &Path) -> Result<Option<PathBuf>, String> {
    let ran = match run_git(world, &["rev-parse", "--show-toplevel"]) {
        Ok(ran) => ran,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("git rev-parse: {e}")),
    };
    if ran.code != Some(0) {
        return if ran.err.contains("not a git repository") {
            Ok(None)
        } else {
            Err(format!("git rev-parse --show-toplevel failed: {}", ran.err))
        };
    }
    let out = ran.out.strip_suffix(b"\n").unwrap_or(&ran.out);
    if out.is_empty() {
        return Err("git named no repository root".into());
    }
    Ok(Some(PathBuf::from(OsStr::from_bytes(out))))
}

/// The commit HEAD names: `None` before the first commit (git's quiet "no such commit").
fn head(top: &Path) -> Result<Option<String>, String> {
    let args = ["rev-parse", "-q", "--verify", "HEAD^{commit}"];
    let ran = run_git(top, &args).map_err(|e| format!("git rev-parse: {e}"))?;
    match ran.code {
        Some(0) => Ok(Some(String::from_utf8_lossy(&ran.out).trim().to_owned())),
        Some(1) if ran.err.is_empty() => Ok(None),
        _ => Err(format!("git {} failed: {}", args.join(" "), ran.err)),
    }
}

/// Paths in git's `-z` output, in its order.
fn paths(out: &[u8]) -> impl Iterator<Item = &[u8]> {
    out.split(|b| *b == 0).filter(|p| !p.is_empty())
}

/// The index as listed (modes, blobs, stages, paths), then each tracked path's bytes in the
/// working tree (a submodule's own HEAD and tree), then each untracked path's, then the publish
/// files beside them.
fn repo_diff(top: &Path, made: &[PathBuf], depth: usize) -> Result<String, String> {
    let mut h = Sha256::new();
    let index = git(top, &["ls-files", "-s", "-z"])?;
    h.update(b"index\0");
    h.update(&index);
    h.update(b"\0tree\0");
    let mut seen: Vec<&[u8]> = Vec::new();
    let mut last: Option<&[u8]> = None;
    for line in paths(&index) {
        // `<mode> <blob> <stage>\t<path>`: a conflicted path is listed once per stage.
        let Some(tab) = line.iter().position(|b| *b == b'\t') else {
            continue;
        };
        let path = &line[tab + 1..];
        if last == Some(path) {
            continue;
        }
        last = Some(path);
        seen.push(path);
        if line.starts_with(b"160000 ") {
            h.update(path);
            h.update(b"\0submodule\0");
            h.update(submodule(top, Path::new(OsStr::from_bytes(path)), depth)?.as_bytes());
            h.update([0]);
        } else {
            entry_into(&mut h, top, path, made);
        }
    }
    h.update(b"untracked\0");
    let others = git(top, &["ls-files", "-o", "--exclude-standard", "-z"])?;
    for p in paths(&others) {
        if !made.contains(&top.join(OsStr::from_bytes(p))) {
            entry_into(&mut h, top, p, made);
            seen.push(p);
        }
    }
    named_into(
        &mut h,
        top,
        seen.iter().map(|p| Path::new(OsStr::from_bytes(p))),
    );
    Ok(hex(&h.finalize()))
}

/// A submodule's own HEAD and working tree, or that it is not checked out.
fn submodule(top: &Path, rel: &Path, depth: usize) -> Result<String, String> {
    match entry(top, &rel.join(".git")) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok("not checked out".into()),
        Err(e) => return Err(format!("submodule {}: {e}", rel.display())),
        Ok(_) => {}
    }
    if depth >= SUBMODULE_DEPTH {
        return Err(format!(
            "submodule {} is nested deeper than {SUBMODULE_DEPTH}",
            rel.display()
        ));
    }
    let sub = top.join(rel);
    if toplevel(&sub)?.as_deref() != Some(sub.as_path()) {
        return Err(format!(
            "submodule {} is not its own repository",
            rel.display()
        ));
    }
    Ok(format!(
        "head {} diff {}",
        head(&sub)?.unwrap_or_else(|| "none".into()),
        repo_diff(&sub, &[], depth + 1)?
    ))
}

/// A world with no repository: every file its ignore files leave in, in path order, then the
/// publish files beside them.
fn tree_diff(world: &Path) -> String {
    let mut found: Vec<PathBuf> = ignore::WalkBuilder::new(world)
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .build()
        .flatten()
        .filter(|e| e.depth() > 0 && !e.file_type().is_some_and(|t| t.is_dir()))
        .filter_map(|e| e.path().strip_prefix(world).ok().map(Path::to_path_buf))
        .collect();
    found.sort();
    let mut h = Sha256::new();
    h.update(b"files\0");
    for rel in &found {
        entry_into(&mut h, world, rel.as_os_str().as_bytes(), &[]);
    }
    named_into(&mut h, world, found.iter().map(PathBuf::as_path));
    hex(&h.finalize())
}

/// The publish files (`NAMED`, `.env.*`, `NAMED_CARGO`) in the root and in every folder that
/// holds one of `files`, by content, whatever the ignore rules say.
fn named_into<'a>(h: &mut Sha256, root: &Path, files: impl Iterator<Item = &'a Path>) {
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    dirs.insert(PathBuf::new());
    for f in files {
        let mut at = f.parent();
        while let Some(d) = at {
            if !dirs.insert(d.to_path_buf()) {
                break;
            }
            at = d.parent();
        }
    }
    h.update(b"named\0");
    for dir in &dirs {
        let Ok(listing) = fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut names: Vec<PathBuf> = listing
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .filter(|n| {
                let n = n.as_bytes();
                NAMED.iter().any(|w| n == w.as_bytes()) || n.starts_with(b".env.")
            })
            .map(|n| dir.join(n))
            .collect();
        names.extend(
            NAMED_CARGO
                .iter()
                .map(|n| dir.join(n))
                .filter(|p| fs::symlink_metadata(root.join(p)).is_ok()),
        );
        names.sort();
        for rel in names {
            entry_into(h, root, rel.as_os_str().as_bytes(), &[]);
        }
    }
}

/// One path and what is at it, into `h`; a path in `made` reads as missing.
fn entry_into(h: &mut Sha256, root: &Path, rel: &[u8], made: &[PathBuf]) {
    let rel = Path::new(OsStr::from_bytes(rel.strip_suffix(b"/").unwrap_or(rel)));
    let what = if made.contains(&root.join(rel)) {
        "missing".into()
    } else {
        match entry(root, rel) {
            Ok(what) => what,
            Err(e) if e.kind() == io::ErrorKind::NotFound => "missing".into(),
            Err(e) => format!("unreadable {:?}", e.kind()),
        }
    };
    h.update(rel.as_os_str().as_bytes());
    h.update([0]);
    h.update(what.as_bytes());
    h.update([0]);
}

/// What is at `rel` under `root`: a file's bytes and executable bit, a link's target, or its
/// kind. Opened one component at a time from the root following no symlink, without blocking,
/// and read no further than the size it had when opened, so a link, a pipe or a file that keeps
/// growing cannot lead the harness elsewhere or hold it.
fn entry(root: &Path, rel: &Path) -> io::Result<String> {
    let (dir, name) = beneath::parent(root, &root.join(rel), false)?;
    let stat = statat(&dir, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)?;
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::Symlink => {
            let to = readlinkat(&dir, name.as_str(), Vec::new())?;
            Ok(format!("link {}", hex(&Sha256::digest(to.as_bytes()))))
        }
        FileType::Directory => Ok("dir".into()),
        FileType::RegularFile => {
            let fd = openat(
                &dir,
                name.as_str(),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )?;
            let file = File::from(fd);
            let meta = file.metadata()?;
            if !meta.is_file() {
                return Ok("other".into());
            }
            let mut h = Sha256::new();
            let mut buf = vec![0u8; 64 * 1024];
            let mut file = file.take(meta.len());
            let mut read = 0u64;
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                read += n as u64;
                h.update(&buf[..n]);
            }
            let exec = if meta.permissions().mode() & 0o111 == 0 {
                '-'
            } else {
                'x'
            };
            Ok(format!("file {exec} {read} {}", hex(&h.finalize())))
        }
        _ => Ok("other".into()),
    }
}

/// Git's whole configuration as git reads it, and the bytes of what in its directory runs code
/// or steers it: `config`, `config.worktree`, `hooks/`, `info/` — in the common directory too,
/// when the world is a worktree — and a `.git` file.
fn git_pieces(top: &Path) -> Result<String, String> {
    let mut h = Sha256::new();
    h.update(b"config\0");
    h.update(git(top, &["config", "--list", "-z"])?);
    h.update(b"\0dot git\0");
    entry_into(&mut h, top, b".git", &[]);
    let dirs = git(top, &["rev-parse", "--git-dir", "--git-common-dir"])?;
    let mut roots: Vec<PathBuf> = String::from_utf8_lossy(&dirs)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| top.join(l))
        .collect();
    roots.dedup();
    if roots.is_empty() {
        return Err("git named no git directory".into());
    }
    let mut left = GIT_ENTRIES;
    for root in &roots {
        h.update(b"git dir\0");
        for piece in GIT_PIECES {
            walk_into(&mut h, root, Path::new(piece), &mut left)?;
        }
    }
    Ok(hex(&h.finalize()))
}

/// `rel` under `root` and, for a folder, everything in it, in name order.
fn walk_into(h: &mut Sha256, root: &Path, rel: &Path, left: &mut usize) -> Result<(), String> {
    *left = left
        .checked_sub(1)
        .ok_or_else(|| format!("more than {GIT_ENTRIES} entries in git's hooks and info"))?;
    entry_into(h, root, rel.as_os_str().as_bytes(), &[]);
    if matches!(entry(root, rel).as_deref(), Ok("dir")) {
        let mut names: Vec<_> = fs::read_dir(root.join(rel))
            .map_err(|e| format!("{}: {e}", root.join(rel).display()))?
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect();
        names.sort();
        for name in names {
            walk_into(h, root, &rel.join(name), left)?;
        }
    }
    Ok(())
}

/// Where an outward act lands: the branch, every ref, and the configuration that names remotes.
fn target(top: &Path) -> Result<String, String> {
    let mut h = Sha256::new();
    h.update(b"branch\0");
    let branch = run_git(top, &["symbolic-ref", "-q", "HEAD"])
        .map_err(|e| format!("git symbolic-ref: {e}"))?;
    match branch.code {
        Some(0) => h.update(&branch.out),
        // Quiet 1: HEAD names a commit, not a branch.
        Some(1) if branch.err.is_empty() => h.update(b"detached"),
        _ => return Err(format!("git symbolic-ref HEAD failed: {}", branch.err)),
    }
    h.update(b"\0refs\0");
    h.update(git(
        top,
        &["for-each-ref", "--format=%(objectname) %(refname)"],
    )?);
    h.update(b"\0config\0");
    for item in paths(&git(top, &["config", "--list", "-z"])?) {
        if TARGET_KEYS.iter().any(|k| item.starts_with(k.as_bytes())) {
            h.update(item);
            h.update([0]);
        }
    }
    // Each remote's push URLs as git resolves them, `insteadOf` and `pushInsteadOf` applied: a
    // reroute is a different target however it was written.
    h.update(b"\0push urls\0");
    for remote in String::from_utf8_lossy(&git(top, &["remote"])?).lines() {
        h.update(remote.as_bytes());
        h.update(b"\n");
        h.update(git(top, &["remote", "get-url", "--push", "--all", remote])?);
        h.update([0]);
    }
    // Where `gh` acts when the command names no `-R`: its repository and host from the
    // environment the command gets (its default remote is in the configuration above).
    h.update(b"gh\0");
    for (name, value) in scrubbed_env() {
        if GH_TARGET.contains(&name.as_str()) {
            h.update(format!("{name}={value}").as_bytes());
            h.update([0]);
        }
    }
    Ok(hex(&h.finalize()))
}

/// The environment that tells `gh` which repository and host to act on.
const GH_TARGET: &[&str] = &["GH_REPO", "GH_HOST"];

#[cfg(test)]
mod tests {
    use super::Fingerprint;

    #[test]
    fn the_short_form_names_every_part() {
        let f = Fingerprint {
            act: "a".repeat(64),
            head: Some("1".repeat(40)),
            diff: "d".repeat(64),
            git: "g".repeat(64),
            target: "t".repeat(64),
        };
        let short = f.short();
        assert!(short.starts_with("plan "), "{short}");
        assert!(short.contains("head 11111111 · diff dddddddd · git gggggggg · target tttttttt"));
        assert_eq!(f.changes(&f), Vec::<String>::new());
        let moved = Fingerprint {
            head: None,
            ..f.clone()
        };
        assert_eq!(f.changes(&moved), ["head moved 11111111 → none"]);
        assert_ne!(f.id(), moved.id());
    }
}
