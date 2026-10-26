//! Where notes live, and how they are written: every tier an append-only file of events, one
//! full line each, synced before the writer returns.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use mnemorph_ledger::{Journal, ReadBack};

use crate::note::{By, Change, Event};

/// A memory tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tier {
    /// One creature's notes: tracked, they travel with the world.
    Personal(String),
    /// The world's notes: tracked.
    Shared,
    /// One body's semantic cache: machine-local, dying with its task.
    Short(String),
}

impl Tier {
    /// The tier's file, relative to the world.
    #[must_use]
    pub fn file(&self) -> PathBuf {
        let base = Path::new(".mnemorph/memory");
        match self {
            Tier::Personal(name) => base.join("creature").join(format!("{name}.jsonl")),
            Tier::Shared => base.join("shared/notes.jsonl"),
            Tier::Short(body) => base.join("short").join(format!("{body}.jsonl")),
        }
    }

    /// The tier's journal in `world`.
    #[must_use]
    pub(crate) fn journal(&self, world: &Path) -> Journal<Event> {
        Journal::new(world.join(self.file()))
    }
}

/// A fresh event id: the time, a random mark drawn once per process, and a counter — unique across
/// machines, where two processes may share a pid and a millisecond.
#[must_use]
pub fn event_id(time: u64) -> String {
    use std::hash::{BuildHasher, Hasher};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    static MARK: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    let mark = MARK.get_or_init(|| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u32(std::process::id());
        h.finish()
    });
    format!(
        "{time:x}-{mark:016x}-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// The time now, in milliseconds since the epoch.
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Append `change` about `note` to `tier`, by `by`, citing `proof`: one full line, synced. Inside
/// the crate only: outside it, a body's words land through [`crate::propose`], and a standing
/// through [`crate::ground`] with its route's proof.
///
/// # Errors
///
/// The file cannot be opened, written or synced.
pub(crate) fn append(
    world: &Path,
    tier: &Tier,
    note: &str,
    by: By,
    proof: Option<String>,
    change: Change,
) -> std::io::Result<Event> {
    let time = now_ms();
    let event = Event {
        id: event_id(time),
        time,
        note: note.to_owned(),
        by,
        proof,
        change,
    };
    tier.journal(world).append(&event)?;
    Ok(event)
}

/// A tier's events, read tolerantly: a torn or foreign line is skipped and counted, never fatal.
///
/// # Errors
///
/// The file exists but cannot be read.
pub fn read(world: &Path, tier: &Tier) -> std::io::Result<ReadBack<Event>> {
    tier.journal(world).read()
}

/// On a split, each heir inherits `from`'s notes whose source lies in its ground; a note whose
/// source no heir's ground covers — a journal entry, a commit, a path outside them all — goes to
/// every heir. The inherited note keeps its id and its standing: the same events, appended to the
/// heir's file. `from`'s own file is kept whole, never deleted. Returns how many events each heir
/// received.
///
/// # Errors
///
/// A file cannot be read or written.
pub fn inherit(
    world: &Path,
    from: &str,
    heirs: &[(String, Vec<String>)],
) -> std::io::Result<Vec<(String, usize)>> {
    let events = read(world, &Tier::Personal(from.to_owned()))?.rows;
    let notes = crate::note::fold(&events);
    // Each note's heirs: those whose ground covers its source, else all.
    let heirs_of = |note: &str| -> Vec<&str> {
        let path = notes
            .get(note)
            .and_then(|n| n.source.as_deref())
            .map(path_of);
        let covering: Vec<&str> = heirs
            .iter()
            .filter(|(_, ground)| {
                path.as_ref()
                    .is_some_and(|p| ground.iter().any(|g| covers(g, p)))
            })
            .map(|(h, _)| h.as_str())
            .collect();
        if covering.is_empty() {
            heirs.iter().map(|(h, _)| h.as_str()).collect()
        } else {
            covering
        }
    };
    let mut given: Vec<(String, usize)> = heirs.iter().map(|(h, _)| (h.clone(), 0)).collect();
    for e in &events {
        for heir in heirs_of(&e.note) {
            Tier::Personal(heir.to_owned()).journal(world).append(e)?;
            if let Some(g) = given.iter_mut().find(|(h, _)| h == heir) {
                g.1 += 1;
            }
        }
    }
    Ok(given)
}

/// The path a source names, its `:line` and `#fragment` dropped: `src/a.rs:12` reads as
/// `src/a.rs`, `.mnemorph/log.md#4` as `.mnemorph/log.md`.
fn path_of(source: &str) -> String {
    let path = source.split(['#', ':']).next().unwrap_or_default();
    path.trim_start_matches("./").to_owned()
}

/// Whether a ground claim covers `path`: a directory claim what lies under it, a file claim itself.
fn covers(claim: &str, path: &str) -> bool {
    let claim = claim.trim_start_matches("./");
    if claim.ends_with('/') {
        path.starts_with(claim)
    } else {
        path == claim
    }
}

/// Git merges every note file by union, through its local attributes — never committed, so the
/// world's own files stay the human's: two machines that both appended never conflict. Lines
/// already there are not added twice; a world with no git, or whose git files are links, is left.
///
/// # Errors
///
/// The attributes file cannot be written.
pub fn union_merge(world: &Path) -> std::io::Result<()> {
    const LINE: &str = ".mnemorph/memory/**/*.jsonl merge=union";
    // Git says where its attributes live: a worktree's or a submodule's `.git` is a file.
    let Ok(out) = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "info/attributes"])
        .current_dir(world)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
    else {
        return Ok(());
    };
    let named = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !out.status.success() || named.is_empty() {
        return Ok(());
    }
    // Relative to the world when git answers so (every git before 2.31 does).
    let file = world.join(named);
    let Some(info) = file.parent().map(Path::to_path_buf) else {
        return Ok(());
    };
    let linked = |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
    if linked(&info) || linked(&file) {
        return Ok(());
    }
    std::fs::create_dir_all(&info)?;
    let had = std::fs::read_to_string(&file).unwrap_or_default();
    if had.lines().any(|l| l.trim() == LINE) {
        return Ok(());
    }
    let mut next = had;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(LINE);
    next.push('\n');
    // Written new beside it, then renamed over it: a link put in its place is replaced, never
    // followed.
    let temp = info.join(".attributes.mnemorph");
    let _ = std::fs::remove_file(&temp);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut f| std::io::Write::write_all(&mut f, next.as_bytes()))
        .and_then(|()| std::fs::rename(&temp, &file))
}
