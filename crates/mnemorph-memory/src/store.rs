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
    pub fn journal(&self, world: &Path) -> Journal<Event> {
        Journal::new(world.join(self.file()))
    }
}

/// A fresh event id: the time, the process and a counter, unique on this machine.
#[must_use]
pub fn event_id(time: u64) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{time:x}-{}-{}",
        std::process::id(),
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

/// Append `change` about `note` to `tier`, by `by`, citing `proof`: one full line, synced.
///
/// # Errors
///
/// The file cannot be opened, written or synced.
pub fn append(
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

/// On a split, each heir inherits `from`'s notes whose source lies in its ground — a note with no
/// path to read goes to every heir. The inherited note keeps its id and its standing: the same
/// events, appended to the heir's file. `from`'s own file is kept whole, never deleted.
/// Returns how many events each heir received.
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
    let source_of = |note: &str| {
        events.iter().find_map(|e| match &e.change {
            Change::Create { source, .. } if e.note == note => Some(source.clone()),
            _ => None,
        })?
    };
    let mut given = Vec::new();
    for (heir, ground) in heirs {
        let journal = Tier::Personal(heir.clone()).journal(world);
        let mut n = 0;
        for e in &events {
            let path = source_of(&e.note).map(|s| path_of(&s));
            let fits = match path {
                Some(Some(p)) => ground.iter().any(|g| covers(g, &p)),
                _ => true,
            };
            if fits {
                journal.append(e)?;
                n += 1;
            }
        }
        given.push((heir.clone(), n));
    }
    Ok(given)
}

/// The path a source points at, when it is one — `src/a.rs:12` reads as `src/a.rs`; a journal
/// entry or a commit is no path.
fn path_of(source: &str) -> Option<String> {
    let path = source.split(':').next()?.trim_start_matches("./");
    (path.contains('/') && !path.starts_with('/')).then(|| path.to_owned())
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
    let git = world.join(".git");
    let linked = |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
    if !git.is_dir() || linked(&git) || linked(&git.join("info")) {
        return Ok(());
    }
    let info = git.join("info");
    std::fs::create_dir_all(&info)?;
    let file = info.join("attributes");
    if linked(&file) {
        return Ok(());
    }
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
