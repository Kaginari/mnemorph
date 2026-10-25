//! The world's report folder: where a check may write what it leaves behind — test reports,
//! coverage, snapshots — since a check never writes the tree. Prepared when a session opens:
//! `.mnemorph/report/` made, and it and the common test leftovers ignored through git's local
//! exclude file, which is never committed, so the project's own `.gitignore` stays the human's.

use std::fs;
use std::path::Path;

/// The report folder, relative to the world.
pub const REPORT: &str = ".mnemorph/report";

/// What a check run commonly leaves behind: a verify run may write these. The gate's view of a
/// body's own changes does not read the exclude file, so these hide nothing a body writes.
pub const LEFTOVERS: &[&str] = &[
    ".mnemorph/report/",
    "__pycache__/",
    ".pytest_cache/",
    // Anchored at the root: a source directory named `coverage` deeper down stays the project's.
    "/.coverage",
    "/htmlcov/",
    "/coverage/",
    "/.nyc_output/",
    ".mypy_cache/",
    ".ruff_cache/",
];

/// Make the report folder and ignore it and the common test leftovers locally. Lines already in
/// git's exclude file are not added twice; a world with no git, or whose git files are links, gets
/// the folder only.
///
/// # Errors
///
/// The folder or the exclude file cannot be written.
pub fn prepare(world: &Path) -> std::io::Result<()> {
    fs::create_dir_all(world.join(REPORT))?;
    let info = world.join(".git").join("info");
    let linked = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
    if !world.join(".git").is_dir() || linked(&world.join(".git")) || linked(&info) {
        return Ok(());
    }
    let exclude = info.join("exclude");
    if linked(&exclude) {
        return Ok(());
    }
    fs::create_dir_all(&info)?;
    if linked(&info) {
        return Ok(());
    }
    let had = fs::read_to_string(&exclude).unwrap_or_default();
    let missing: Vec<&str> = LEFTOVERS
        .iter()
        .copied()
        .filter(|l| !had.lines().any(|h| h.trim() == *l))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let mut next = had;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("# mnemorph: what checks leave behind (local, never committed)\n");
    for l in missing {
        next.push_str(l);
        next.push('\n');
    }
    // Written new beside it, then renamed over it: a link put in its place is replaced, never
    // followed.
    let temp = info.join(".exclude.mnemorph");
    let _ = fs::remove_file(&temp);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut f| std::io::Write::write_all(&mut f, next.as_bytes()))
        .and_then(|()| fs::rename(&temp, &exclude))
}
