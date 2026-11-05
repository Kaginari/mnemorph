//! The scout's advisory mirror. OSV publishes each ecosystem's advisories as one `all.zip`; each
//! ecosystem the world's lockfiles name is kept under `.mnemorph/intel/osv-scanner/<ecosystem>/`,
//! the layout `osv-scanner --offline` reads, so an audit needs no network. `.mnemorph/intel/` is
//! machine-local and a body may write it: no link is followed into it, a download lands whole or
//! not at all, and what the mirror holds is said as last read. It is a fetch: nothing is sent.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use mnemorph_ledger::{Journal, Timestamp};
use serde::{Deserialize, Serialize};

/// The mirror, relative to the world.
pub const MIRROR: &str = ".mnemorph/intel/osv-scanner";

/// Where OSV publishes each ecosystem.
const OSV: &str = "https://osv-vulnerabilities.storage.googleapis.com";

/// The lockfiles at a world's root and the ecosystem each names.
const LOCKFILES: &[(&str, &str)] = &[
    ("Cargo.lock", "crates.io"),
    ("package-lock.json", "npm"),
    ("pnpm-lock.yaml", "npm"),
    ("yarn.lock", "npm"),
    ("go.sum", "Go"),
    ("requirements.txt", "PyPI"),
    ("poetry.lock", "PyPI"),
    ("uv.lock", "PyPI"),
];

/// How long one ecosystem's download may take, in seconds.
const FETCH_SECS: &str = "600";

/// One ecosystem as it was mirrored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirrored {
    /// When.
    pub ts: Timestamp,
    /// The ecosystem.
    pub ecosystem: String,
    /// Its size in bytes.
    pub bytes: u64,
    /// The SHA-256 of what was fetched.
    pub sha256: String,
}

fn journal(world: &Path) -> Journal<Mirrored> {
    Journal::new(world.join(MIRROR).join("mirror.jsonl"))
}

/// The ecosystems `world`'s lockfiles name, each once, in the order above.
#[must_use]
pub fn ecosystems(world: &Path) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (file, eco) in LOCKFILES {
        let held = fs::symlink_metadata(world.join(file)).is_ok_and(|m| m.is_file());
        if held && !out.contains(eco) {
            out.push(*eco);
        }
    }
    out
}

/// Each folder from the world down to `rel` made where missing, none followed through a link.
fn folder(world: &Path, rel: &Path) -> Result<PathBuf, String> {
    let mut at = world.to_path_buf();
    for part in rel.components() {
        at.push(part);
        match fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "{}: a link in the mirror; it follows none",
                    at.display()
                ));
            }
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(format!("{}: a file where a folder goes", at.display())),
            Err(_) => fs::create_dir(&at).map_err(|e| format!("{}: {e}", at.display()))?,
        }
    }
    Ok(at)
}

/// One ecosystem fetched to a part file beside its place, checked, then moved in whole.
fn mirror(
    world: &Path,
    eco: &str,
    fetch: &dyn Fn(&str, &Path) -> Result<(), String>,
) -> Result<Mirrored, String> {
    let dir = folder(world, &Path::new(MIRROR).join(eco))?;
    let part = dir.join("all.zip.part");
    let _ = fs::remove_file(&part);
    let got = fetch(&format!("{OSV}/{eco}/all.zip"), &part).and_then(|()| {
        let bytes = fs::read(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        if !bytes.starts_with(b"PK\x03\x04") {
            return Err("the download is no zip".to_owned());
        }
        fs::rename(&part, dir.join("all.zip")).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Mirrored {
            ts: Timestamp::now(),
            ecosystem: eco.to_owned(),
            bytes: bytes.len() as u64,
            sha256: mnemorph_ledger::evolution::digest(&bytes),
        })
    });
    if got.is_err() {
        let _ = fs::remove_file(&part);
    }
    got
}

/// Each ecosystem of `world` fetched through `fetch` (a URL, and the file to write): one line
/// each, and whether every one came whole. One that fails keeps the mirror it had.
///
/// # Errors
///
/// A link stands in the mirror's way, or the record cannot be written.
pub fn refresh(
    world: &Path,
    fetch: &dyn Fn(&str, &Path) -> Result<(), String>,
) -> Result<(Vec<String>, bool), String> {
    let ecos = ecosystems(world);
    if ecos.is_empty() {
        return Ok((
            vec!["no lockfile at the world's root: nothing to mirror".into()],
            true,
        ));
    }
    folder(world, Path::new(MIRROR))?;
    let (mut said, mut whole) = (Vec::new(), true);
    for eco in ecos {
        match mirror(world, eco, fetch) {
            Ok(m) => {
                journal(world)
                    .append(&m)
                    .map_err(|e| format!("the mirror's record: {e}"))?;
                said.push(format!("{eco}: {} MB", m.bytes / 1_000_000));
            }
            Err(e) if e.contains("a link in the mirror") => return Err(e),
            Err(e) => {
                whole = false;
                said.push(format!("{eco}: not refreshed — {e}"));
            }
        }
    }
    Ok((said, whole))
}

/// [`refresh`] with `curl`, in the sandbox, HTTPS alone, the harness's keys kept from it.
///
/// # Errors
///
/// As [`refresh`].
pub fn refresh_now(world: &Path) -> Result<(Vec<String>, bool), String> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()));
    let fetch = |url: &str, to: &Path| {
        let dir = to.parent().unwrap_or(world);
        let mut policy = mnemorph_tools::Policy::new(dir, &home, dir, true);
        policy.die_with_parent = true;
        let to = to.display().to_string();
        let out = mnemorph_tools::Sandbox::detect()
            .command(
                &policy,
                "curl",
                &[
                    "--fail",
                    "--silent",
                    "--show-error",
                    "--location",
                    "--proto",
                    "=https",
                    "--tlsv1.2",
                    "--max-time",
                    FETCH_SECS,
                    "--output",
                    &to,
                    url,
                ],
            )
            .env_clear()
            .envs(mnemorph_tools::shell::scrubbed_env())
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("curl: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    };
    refresh(world, &fetch)
}

/// One line per ecosystem mirrored, newest fetch of each, its age, as last read.
#[must_use]
pub fn lines(world: &Path) -> Vec<String> {
    let rows = journal(world).read().map(|r| r.rows).unwrap_or_default();
    let mut newest = std::collections::BTreeMap::<String, Mirrored>::new();
    for r in rows {
        newest.insert(r.ecosystem.clone(), r);
    }
    let now = Timestamp::now().0;
    newest
        .values()
        .map(|m| {
            let days = (now - m.ts.0).max(0) / 86_400;
            let age = if days == 0 {
                "today".to_owned()
            } else {
                format!("{days} day(s) ago")
            };
            format!(
                "{}: {} MB, fetched {age}, sha256 {}, as last read",
                m.ecosystem,
                m.bytes / 1_000_000,
                m.sha256.chars().take(12).collect::<String>()
            )
        })
        .collect()
}
