//! The guts classes of every act, kept by the harness outside the world:
//! `$XDG_STATE_HOME/mnemorph/guts/<world>-<fnv64a>.jsonl`, one row per act the loop read, written
//! by the harness alone, chained by hash ([`mnemorph_ledger::chain`]) and anchored with the world's
//! other chained journals ([`crate::chains`]). Each row carries the build that wrote it, so a
//! learned value trained on these rows can keep one build's classifier apart from another's.
//!
//! The classes are signals: nothing reads them to raise or fall a creature. The loop classifies
//! (`mnemorph_loop::guts`); this module only keeps the record.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use mnemorph_ledger::{Build, Journal, Timestamp};
use serde::{Deserialize, Serialize};

/// One class of an act, as isekai's guts names them, so lessons and a learned value group by
/// class. Read by harness code from the act's structured outcome and the acts before it, never
/// from what the model or a tool wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// The act errored: an error result, a non-zero exit, a kill at the deadline, a refusal by the
    /// envelope, the sandbox or the shelf. A human's denial is the human's choice, never this; a
    /// test bench the shell saw exit non-zero is a red pin instead.
    Fail,
    /// A test bench the shell saw exit non-zero: TDD's red, the right process. Its weight in any
    /// learned value is pinned at 0.
    RedPin,
    /// A test bench the shell saw exit 0: evidence, the work checked.
    Positive,
    /// The same tool with the same input as the act just before.
    Repeat,
    /// The third or later visit to one read window or command head among the recent acts, with no
    /// write since the last visit: a closer look is not new evidence.
    Echo,
    /// A test bench struck for the sixth time or more in one ask: a check loop, not development.
    Thrash,
}

impl Class {
    /// Every class, in the order a line lists them.
    pub const ALL: [Class; 6] = [
        Class::Fail,
        Class::RedPin,
        Class::Positive,
        Class::Repeat,
        Class::Echo,
        Class::Thrash,
    ];

    /// The class's stable word: `fail`, `red-pin`, `positive`, `repeat`, `echo` or `thrash`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Class::Fail => "fail",
            Class::RedPin => "red-pin",
            Class::Positive => "positive",
            Class::Repeat => "repeat",
            Class::Echo => "echo",
            Class::Thrash => "thrash",
        }
    }
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}

/// One act as the guts classed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GutsRow {
    /// When it was read.
    pub ts: Timestamp,
    /// The run it belongs to: its journal's name under `.mnemorph/instruments/loop/`.
    pub run: String,
    /// The body that acted.
    pub body: String,
    /// The tool call it reads.
    pub tool_use_id: String,
    /// The tool.
    pub tool: String,
    /// Its classes, in [`Class::ALL`]'s order; empty for a plain act.
    #[serde(default)]
    pub classes: Vec<Class>,
    /// The build that classed it; `unknown` for a row from before stamps.
    #[serde(default)]
    pub build: Build,
}

/// Where `world`'s guts rows are kept: `$XDG_STATE_HOME/mnemorph/guts/<world>-<fnv64a>.jsonl`, else
/// under `~/.local/state`; `None` when there is no home.
#[must_use]
pub fn guts_of(world: &Path) -> Option<PathBuf> {
    crate::queue::state_file(world, "guts")
}

/// `world`'s guts rows, chained, for the harness to append to; `None` when there is no home.
#[must_use]
pub fn journal(world: &Path) -> Option<Journal<GutsRow>> {
    guts_of(world).map(journal_at)
}

/// The guts rows kept at `path`, chained.
#[must_use]
pub fn journal_at(path: impl Into<PathBuf>) -> Journal<GutsRow> {
    Journal::chained(path)
}

/// Every guts row of `world`, in order; an empty list with no home.
///
/// # Errors
///
/// The rows exist but cannot be read.
pub fn read(world: &Path) -> io::Result<Vec<GutsRow>> {
    journal(world).map_or_else(|| Ok(Vec::new()), |j| Ok(j.read()?.rows))
}

/// Appends `row` to `world`'s guts rows, stamped with this binary's build. Nothing is written with
/// no home.
///
/// # Errors
///
/// The rows cannot be written.
pub fn record(world: &Path, row: GutsRow) -> io::Result<()> {
    journal(world).map_or(Ok(()), |j| append_stamped(&j, row))
}

/// `row` appended to `journal`, stamped with this binary's build.
///
/// # Errors
///
/// The rows cannot be written.
pub fn append_stamped(journal: &Journal<GutsRow>, mut row: GutsRow) -> io::Result<()> {
    row.build = Build::this();
    journal.append(&row)
}

/// One run's acts, counted per class.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    /// The run.
    pub run: String,
    /// The body that ran it.
    pub body: String,
    /// Its acts, classed or not.
    pub acts: u32,
    /// The acts of each class; a class no act had is absent.
    pub by_class: BTreeMap<Class, u32>,
}

impl Counts {
    /// The acts of `class`.
    #[must_use]
    pub fn of(&self, class: Class) -> u32 {
        self.by_class.get(&class).copied().unwrap_or(0)
    }
}

impl fmt::Display for Counts {
    /// `<run> (<body>): 12 acts · fail 2 · red-pin 1 · positive 3 · repeat 0 · echo 0 · thrash 0`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}): {} acts", self.run, self.body, self.acts)?;
        for c in Class::ALL {
            write!(f, " · {c} {}", self.of(c))?;
        }
        Ok(())
    }
}

/// The last run `rows` hold — the run of the newest row — with its acts counted per class.
#[must_use]
pub fn last_run(rows: &[GutsRow]) -> Option<Counts> {
    let last = rows.last()?;
    let mut c = Counts {
        run: last.run.clone(),
        body: last.body.clone(),
        ..Counts::default()
    };
    for r in rows.iter().filter(|r| r.run == last.run) {
        c.acts += 1;
        for class in &r.classes {
            *c.by_class.entry(*class).or_default() += 1;
        }
    }
    Some(c)
}

/// The last run of `world`'s guts rows, counted per class, read while their chain holds to the
/// newest head anchored outside them; `None` when no act was classed yet.
///
/// # Errors
///
/// Why the rows are not believed: no home to keep them under, a chain that does not hold, or a
/// file that cannot be read.
pub fn last_run_held(world: &Path) -> Result<Option<Counts>, String> {
    held(world).map(|rows| last_run(&rows))
}

/// Every guts row of `world`, read while their chain holds to the newest head anchored outside
/// them: what `status` and the board show.
///
/// # Errors
///
/// As [`last_run_held`].
pub fn held(world: &Path) -> Result<Vec<GutsRow>, String> {
    let path = guts_of(world).ok_or("no state directory: the classes are kept nowhere")?;
    held_at(&path, crate::chains::anchor_of(world, "guts").as_ref())
}

/// The most rows [`held`] keeps and answers, the newest: what `status` and a timeline page read.
pub const HELD_ROWS: usize = 100_000;

/// How many bytes before a walk's end are read again on the next call: the last row, which an edit
/// in place would change without growing the file.
const RECHECK: u64 = 4096;

/// A file's walk so far, kept between calls so each call walks only the bytes appended since.
struct Kept {
    /// The anchor the walk was held to: another anchor walks the file again.
    anchor: Option<mnemorph_ledger::chain::Head>,
    /// The file it walked, by device and inode: another file walks again.
    id: (u64, u64),
    /// The bytes walked: a file shorter than this walks again.
    len: u64,
    /// The last [`RECHECK`] bytes walked, compared again on the next call.
    end: Vec<u8>,
    /// The walk.
    walk: mnemorph_ledger::chain::Walk,
    /// The rows of the whole lines walked, the newest [`HELD_ROWS`].
    rows: std::collections::VecDeque<GutsRow>,
    /// A last line not yet ended, walked but not read as a row.
    partial: Vec<u8>,
}

/// Every walk this process keeps, by path.
static KEPT: std::sync::Mutex<BTreeMap<PathBuf, Kept>> = std::sync::Mutex::new(BTreeMap::new());

/// Bytes walked by [`held_at`], for the test that it walks only what was appended.
#[cfg(test)]
static WALKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// [`held`] over the rows at `path`, held to `anchor`. The file is walked once whole, then only
/// what was appended since, its last row read again each call; another anchor, another file, a
/// shorter one or a last row changed in place walks it again from its first byte.
fn held_at(
    path: &Path,
    anchor: Option<&mnemorph_ledger::chain::Head>,
) -> Result<Vec<GutsRow>, String> {
    use std::io::{Read, Seek, SeekFrom};
    use std::os::unix::fs::MetadataExt;
    let unread = |e: io::Error| format!("the classes cannot be read: {e}");
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(unread(e)),
    };
    let meta = file.metadata().map_err(unread)?;
    let (id, len) = ((meta.dev(), meta.ino()), meta.len());
    let mut all = KEPT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fresh = || Kept {
        anchor: anchor.cloned(),
        id,
        len: 0,
        end: Vec::new(),
        walk: anchor.map_or_else(mnemorph_ledger::chain::Walk::new, |a| {
            mnemorph_ledger::chain::Walk::new().anchored(a.clone())
        }),
        rows: std::collections::VecDeque::new(),
        partial: Vec::new(),
    };
    let mut kept = all.remove(path).unwrap_or_else(fresh);
    let same_end = || -> io::Result<bool> {
        let n = kept.end.len() as u64;
        let mut f = &file;
        f.seek(SeekFrom::Start(kept.len - n))?;
        let mut end = vec![0u8; kept.end.len()];
        f.read_exact(&mut end)?;
        Ok(end == kept.end)
    };
    if kept.anchor.as_ref() != anchor
        || kept.id != id
        || kept.len > len
        || !same_end().map_err(unread)?
    {
        kept = fresh();
    }
    file.seek(SeekFrom::Start(kept.len)).map_err(unread)?;
    let mut new = Vec::new();
    (&file)
        .take(len - kept.len)
        .read_to_end(&mut new)
        .map_err(unread)?;
    #[cfg(test)]
    WALKED.fetch_add(new.len() as u64, std::sync::atomic::Ordering::Relaxed);
    kept.walk.feed(&new);
    kept.len += new.len() as u64;
    let mut end = std::mem::take(&mut kept.end);
    end.extend_from_slice(&new);
    let cut = end
        .len()
        .saturating_sub(usize::try_from(RECHECK).unwrap_or(usize::MAX));
    end.drain(..cut);
    kept.end = end;
    kept.partial.extend_from_slice(&new);
    if let Some(at) = kept.partial.iter().rposition(|&b| b == b'\n') {
        let whole: Vec<u8> = kept.partial.drain(..=at).collect();
        let back: mnemorph_ledger::ReadBack<GutsRow> =
            mnemorph_ledger::parse_rows(whole.as_slice()).map_err(unread)?;
        kept.rows.extend(back.rows);
        let over = kept.rows.len().saturating_sub(HELD_ROWS);
        kept.rows.drain(..over);
    }
    let report = kept.walk.report();
    let rows = kept.rows.iter().cloned().collect();
    all.insert(path.to_path_buf(), kept);
    if !report.holds() {
        return Err(format!(
            "the classes are not believed: their chain is {report}"
        ));
    }
    Ok(rows)
}

/// [`last_run_held`] over the rows at `path`, held to `anchor`.
#[cfg(test)]
fn last_run_at(
    path: &Path,
    anchor: Option<&mnemorph_ledger::chain::Head>,
) -> Result<Option<Counts>, String> {
    held_at(path, anchor).map(|rows| last_run(&rows))
}

/// Each run's acts in order, by run: the classes of every act, as the board's timeline shows them.
#[must_use]
pub fn by_run(rows: &[GutsRow]) -> BTreeMap<&str, Vec<&[Class]>> {
    let mut out: BTreeMap<&str, Vec<&[Class]>> = BTreeMap::new();
    for r in rows {
        out.entry(r.run.as_str()).or_default().push(&r.classes);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(run: &str, classes: &[Class]) -> GutsRow {
        GutsRow {
            ts: Timestamp::now(),
            run: run.into(),
            body: "slime-a".into(),
            tool_use_id: "t".into(),
            tool: "bash".into(),
            classes: classes.to_vec(),
            build: Build::default(),
        }
    }

    #[test]
    fn the_rows_live_outside_the_world_and_are_anchored_with_the_others() {
        let world = Path::new("/srv/worlds/jura");
        let Some(at) = guts_of(world) else {
            return;
        };
        assert!(!at.starts_with(world), "{}", at.display());
        assert!(at.to_string_lossy().contains("mnemorph/guts/jura-"));
        assert!(journal(world).is_some_and(|j| j.is_chained()));
        assert!(
            crate::chains::journals(world)
                .iter()
                .any(|j| j.name == "guts" && j.path == at)
        );
    }

    #[test]
    fn the_words_are_isekais() {
        let words: Vec<String> = Class::ALL
            .iter()
            .map(|c| serde_json::to_string(c).unwrap())
            .collect();
        assert_eq!(
            words,
            [
                "\"fail\"",
                "\"red-pin\"",
                "\"positive\"",
                "\"repeat\"",
                "\"echo\"",
                "\"thrash\""
            ]
        );
        for c in Class::ALL {
            assert_eq!(serde_json::to_string(&c).unwrap(), format!("\"{c}\""));
        }
    }

    /// Rows are chained and stamped: an edited class is found at its line.
    #[test]
    fn rows_are_chained_and_stamped() {
        use mnemorph_ledger::chain::{self, Break};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guts/w.jsonl");
        let j = journal_at(&path);
        append_stamped(&j, row("r1", &[Class::Fail])).unwrap();
        append_stamped(&j, row("r1", &[Class::RedPin, Class::Thrash])).unwrap();
        append_stamped(&j, row("r1", &[])).unwrap();
        let report = chain::verify(&path).unwrap();
        assert!(report.holds(), "{report}");
        let rows = j.read().unwrap().rows;
        assert!(rows.iter().all(|r| r.build == Build::this()));
        let raw = std::fs::read_to_string(&path).unwrap();
        // A body that reached the file and turned its red pin into a positive breaks the chain.
        let forged = raw.replacen("\"red-pin\"", "\"positive\"", 1);
        std::fs::write(&path, forged).unwrap();
        assert_eq!(
            chain::verify(&path).unwrap().broken,
            Some((2, Break::Edited))
        );
    }

    /// `status` believes the rows while their chain holds: a body that reached the file and
    /// turned a fail into a positive is refused, never counted.
    #[test]
    fn the_last_run_is_believed_only_while_the_chain_holds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guts/w.jsonl");
        assert_eq!(last_run_at(&path, None), Ok(None), "no act classed yet");
        let j = journal_at(&path);
        append_stamped(&j, row("r1", &[Class::Fail])).unwrap();
        append_stamped(&j, row("r1", &[Class::Positive])).unwrap();
        let held = last_run_at(&path, None).unwrap().unwrap();
        assert_eq!((held.of(Class::Fail), held.of(Class::Positive)), (1, 1));
        let raw = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, raw.replacen("\"fail\"", "\"positive\"", 1)).unwrap();
        let refused = last_run_at(&path, None).unwrap_err();
        assert!(refused.contains("not believed"), "{refused}");
    }

    /// A large record is walked once whole, then only what was appended; an edit in place of the
    /// last row, a shorter file or a new anchor walks it again, and the edit is found.
    #[test]
    fn a_large_record_is_walked_once_then_only_its_new_rows() {
        use std::sync::atomic::Ordering;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guts/w.jsonl");
        let j = journal_at(&path);
        // Sealed in memory and written once: twenty thousand synced appends would take a minute.
        let mut walk = mnemorph_ledger::chain::Walk::new();
        let mut bytes = Vec::new();
        for n in 0..20_000 {
            let classes = if n % 2 == 0 {
                vec![Class::Fail]
            } else {
                vec![]
            };
            let mut r = row(&format!("r{}", n / 100), &classes);
            r.build = Build::this();
            let (line, next) = walk.seal(&serde_json::to_vec(&r).unwrap()).unwrap();
            bytes.extend_from_slice(&line);
            walk = next;
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let size = std::fs::metadata(&path).unwrap().len();
        let walked = || WALKED.load(Ordering::Relaxed);
        let before = walked();
        assert_eq!(held_at(&path, None).unwrap().len(), 20_000);
        assert!(walked() - before >= size, "the first call walks it whole");
        let before = walked();
        assert_eq!(held_at(&path, None).unwrap().len(), 20_000);
        assert_eq!(walked() - before, 0, "nothing new, nothing walked");
        append_stamped(&j, row("r-last", &[Class::Echo])).unwrap();
        let grown = std::fs::metadata(&path).unwrap().len();
        let before = walked();
        let rows = held_at(&path, None).unwrap();
        assert_eq!(walked() - before, grown - size, "only the new row");
        assert_eq!(rows.last().unwrap().classes, [Class::Echo]);
        // The last row edited in place, the file's length unchanged: found.
        let raw = std::fs::read_to_string(&path).unwrap();
        let at = raw.rfind("\"echo\"").unwrap();
        let mut edited = raw.clone();
        edited.replace_range(at..at + 6, "\"fail\"");
        std::fs::write(&path, &edited).unwrap();
        let refused = held_at(&path, None).unwrap_err();
        assert!(refused.contains("not believed"), "{refused}");
        // Cut short: walked again, and believed.
        let cut = raw.lines().take(10).collect::<Vec<_>>().join("\n") + "\n";
        std::fs::write(&path, cut).unwrap();
        assert_eq!(held_at(&path, None).unwrap().len(), 10);
    }

    #[test]
    fn the_last_run_is_counted_per_class() {
        let rows = [
            row("r0", &[Class::Positive]),
            row("r1", &[Class::Fail, Class::Repeat]),
            row("r1", &[]),
            row("r1", &[Class::Fail]),
        ];
        let c = last_run(&rows).unwrap();
        assert_eq!((c.run.as_str(), c.acts), ("r1", 3));
        assert_eq!((c.of(Class::Fail), c.of(Class::Repeat)), (2, 1));
        assert_eq!(c.of(Class::Positive), 0, "another run's act");
        assert_eq!(
            c.to_string(),
            "r1 (slime-a): 3 acts · fail 2 · red-pin 0 · positive 0 · repeat 1 · echo 0 · thrash 0"
        );
        assert_eq!(last_run(&[]), None);
        let runs = by_run(&rows);
        assert_eq!(runs["r1"].len(), 3);
        assert_eq!(runs["r0"], [&[Class::Positive][..]]);
    }
}
