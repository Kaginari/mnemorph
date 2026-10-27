//! The plan an approval is bound to: what the world looked like when the human said yes.
//!
//! A human may answer minutes after the ask, and other courts keep working meanwhile. An
//! approval is therefore given to a [`Fingerprint`] — the act, the commit HEAD names, the
//! working tree as git would see it, and where an outward act lands — and the bash tool
//! recomputes it just before the command is spawned. Any difference and the command does not
//! run: the gate asks again, saying what changed.
//!
//! The fingerprint reads the world through git, deaf to anything git could run (no fsmonitor, no
//! hooks, no filters: the index is listed, never refreshed, and the working tree's bytes are
//! hashed here, not by git), and opens each file one component at a time from the root,
//! following no symlink.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs::File;
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

/// What an approval was given against. Two fingerprints are equal exactly when the act, the
/// commit, the working tree and the target are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// SHA-256 of the act, as it will run.
    pub act: String,
    /// The commit HEAD names; `None` outside a repository or before the first commit.
    pub head: Option<String>,
    /// SHA-256 over the index (what is staged), every tracked file's bytes in the working tree
    /// (what is not), and every untracked file git does not ignore. A world with no repository
    /// hashes every file its ignore files leave in.
    pub diff: String,
    /// SHA-256 over where an outward act lands: the branch HEAD names, every ref and the commit
    /// it names, the remote, branch, push and url configuration (every scope git reads), each
    /// remote's resolved push URLs, and `GH_REPO` and `GH_HOST`.
    pub target: String,
}

impl Fingerprint {
    /// The fingerprint of running `act` in `world`, now.
    #[must_use]
    pub fn of(act: &str, world: &Path) -> Self {
        Self::of_except(act, world, &[])
    }

    /// The fingerprint of running `act` in `world`, now, reading the files in `made` — the empty
    /// files the harness itself made, after the approval, for the sandbox to open — as the
    /// absence they were when the human was asked. Only a world inside a larger repository can
    /// see them: they are outside the world.
    #[must_use]
    pub fn of_except(act: &str, world: &Path, made: &[PathBuf]) -> Self {
        let act = hex(&Sha256::digest(act.as_bytes()));
        let made: Vec<PathBuf> = made
            .iter()
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
            .collect();
        match toplevel(world) {
            Some(top) => Self {
                act,
                head: git(&top, &["rev-parse", "-q", "--verify", "HEAD^{commit}"])
                    .map(|out| String::from_utf8_lossy(&out).trim().to_owned())
                    .filter(|sha| !sha.is_empty()),
                diff: repo_diff(&top, &made),
                target: target(&top),
            },
            None => Self {
                act,
                head: None,
                diff: tree_diff(world),
                target: hex(&Sha256::digest(b"no repository")),
            },
        }
    }

    /// The plan's name: the first digits of a hash over every part.
    #[must_use]
    pub fn id(&self) -> String {
        let mut h = Sha256::new();
        for part in [
            self.act.as_str(),
            self.head.as_deref().unwrap_or("none"),
            &self.diff,
            &self.target,
        ] {
            h.update(part.as_bytes());
            h.update([0]);
        }
        hex(&h.finalize())[..ID].to_owned()
    }

    /// The short form a human reads on the approval card and in the journal:
    /// `plan 3f9a1c2e4b5d · head 1a2b3c4d · diff 5d6e7f8a · target 9a8b7c6d`.
    #[must_use]
    pub fn short(&self) -> String {
        format!(
            "plan {} · head {} · diff {} · target {}",
            self.id(),
            self.head.as_deref().map_or_else(|| "none".into(), cut),
            cut(&self.diff),
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

/// Git, read-only and deaf to what it could run: the world's own fsmonitor, hooks and untracked
/// cache are off, and nothing here makes git read a file through a filter. The environment is
/// the one the command itself gets, so git resolves the same repository and configuration.
fn git(dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
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
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// The repository's root, when `world` is in one.
fn toplevel(world: &Path) -> Option<PathBuf> {
    let out = git(world, &["rev-parse", "--show-toplevel"])?;
    let out = out.strip_suffix(b"\n").unwrap_or(&out);
    (!out.is_empty()).then(|| PathBuf::from(OsStr::from_bytes(out)))
}

/// Paths in git's `-z` output, in its order.
fn paths(out: &[u8]) -> impl Iterator<Item = &[u8]> {
    out.split(|b| *b == 0).filter(|p| !p.is_empty())
}

/// The index as listed (modes, blobs, stages, paths), then each tracked path's bytes in the
/// working tree, then each untracked path's.
fn repo_diff(top: &Path, made: &[PathBuf]) -> String {
    let mut h = Sha256::new();
    let index = git(top, &["ls-files", "-s", "-z"]);
    h.update(b"index\0");
    h.update(index.as_deref().unwrap_or(b"unreadable"));
    h.update(b"\0tree\0");
    if let Some(index) = &index {
        let mut last: Option<&[u8]> = None;
        for line in paths(index) {
            // `<mode> <blob> <stage>\t<path>`: a conflicted path is listed once per stage.
            let Some(tab) = line.iter().position(|b| *b == b'\t') else {
                continue;
            };
            let path = &line[tab + 1..];
            if last == Some(path) {
                continue;
            }
            last = Some(path);
            entry_into(&mut h, top, path, made);
        }
    }
    h.update(b"untracked\0");
    match git(top, &["ls-files", "-o", "--exclude-standard", "-z"]) {
        Some(others) => paths(&others)
            .filter(|p| !made.contains(&top.join(OsStr::from_bytes(p))))
            .for_each(|p| entry_into(&mut h, top, p, made)),
        None => h.update(b"unreadable"),
    }
    hex(&h.finalize())
}

/// A world with no repository: every file its ignore files leave in, in path order.
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
    hex(&h.finalize())
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
/// kind. Opened one component at a time from the root following no symlink, and without
/// blocking, so a link or a pipe swapped in cannot lead the harness elsewhere or hang it.
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
            let mut file = File::from(fd);
            let meta = file.metadata()?;
            if !meta.is_file() {
                return Ok("other".into());
            }
            let mut h = Sha256::new();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            let exec = if meta.permissions().mode() & 0o111 == 0 {
                '-'
            } else {
                'x'
            };
            Ok(format!("file {exec} {}", hex(&h.finalize())))
        }
        _ => Ok("other".into()),
    }
}

/// Where an outward act lands: the branch, every ref, and the configuration that names remotes.
fn target(top: &Path) -> String {
    let mut h = Sha256::new();
    h.update(b"branch\0");
    h.update(
        git(top, &["symbolic-ref", "-q", "HEAD"])
            .as_deref()
            .unwrap_or(b"detached"),
    );
    h.update(b"\0refs\0");
    h.update(
        git(top, &["for-each-ref", "--format=%(objectname) %(refname)"])
            .as_deref()
            .unwrap_or(b"unreadable"),
    );
    h.update(b"\0config\0");
    match git(top, &["config", "--list", "-z"]) {
        Some(config) => {
            for item in paths(&config) {
                if TARGET_KEYS.iter().any(|k| item.starts_with(k.as_bytes())) {
                    h.update(item);
                    h.update([0]);
                }
            }
        }
        None => h.update(b"unreadable"),
    }
    // Each remote's push URLs as git resolves them, `insteadOf` and `pushInsteadOf` applied: a
    // reroute is a different target however it was written.
    h.update(b"\0push urls\0");
    if let Some(remotes) = git(top, &["remote"]) {
        for remote in String::from_utf8_lossy(&remotes).lines() {
            h.update(remote.as_bytes());
            h.update(b"\n");
            h.update(
                git(top, &["remote", "get-url", "--push", "--all", remote])
                    .as_deref()
                    .unwrap_or(b"unresolved"),
            );
            h.update([0]);
        }
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
    hex(&h.finalize())
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
            target: "t".repeat(64),
        };
        let short = f.short();
        assert!(short.starts_with("plan "), "{short}");
        assert!(short.contains("head 11111111 · diff dddddddd · target tttttttt"));
        assert_eq!(f.changes(&f), Vec::<String>::new());
        let moved = Fingerprint {
            head: None,
            ..f.clone()
        };
        assert_eq!(f.changes(&moved), ["head moved 11111111 → none"]);
        assert_ne!(f.id(), moved.id());
    }
}
