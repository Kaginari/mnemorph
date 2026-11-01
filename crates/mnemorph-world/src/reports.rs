//! The report fold: every creature's reports, folded from the harness's journals under the human's
//! state directory into the report ledger ([`mnemorph_ledger::reports`]), and the inputs the race
//! law reads from it.
//!
//! **Only trusted journals are read**, each a witness the harness keeps outside the world, its
//! hash chain walked over the very bytes the fold reads and held to the head anchored under the
//! state directory:
//!
//! - the **usage witness** (`mnemorph/usage/`, anchored as `usage-witness`): each run, its body,
//!   its race and the body that commissioned it;
//! - the **gate verdicts** (`mnemorph/gate/`, anchored as `gate`): each landing's word, its
//!   failures and unverified checks, the paths it changed;
//! - the **challenge records** (`mnemorph/challenges/`, anchored as `value`): the findings upheld
//!   on a landing, which overturn the gate's pass on it;
//! - each **Kijin post's witness** (`mnemorph/kijin/`, anchored as `kijin-<post>`): each outward
//!   act and where it ended.
//!
//! A world's own copies — `.mnemorph/instruments/usage.jsonl`, a post journal, a run journal — are
//! never read: a body can write them. A journal whose chain does not hold, whose tail is torn, or
//! that is gone while a head stays anchored for it stops the fold ([`Unavailable::ChainBroken`]):
//! its meters read unavailable, never stale, never zero. A pass counts as verified only when a
//! line the human pinned held it.
//!
//! The fold is kept in memory: a journal the kernel says has not changed since its walk, held to
//! the same anchor, is not read again, and a fold none of whose journals moved is the last one.
//!
//! The ledger is kept at `$XDG_STATE_HOME/mnemorph/reports/<world>-<fnv64a>.jsonl` ([`record`]):
//! rebuilt whole from the journals and put in place by a rename only when its bytes change, so a
//! replay of the same journals gives the same bytes.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mnemorph_ledger::chain::{Head, Walk};
use mnemorph_ledger::reports::{
    ActReport, AskReport, Gate, Inputs, ReportRow, RunReport, fold as fold_rows, ledger_bytes,
};
use mnemorph_ledger::{Build, Standing, Timestamp, UsageRow};
use mnemorph_races::{Post, Race};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::queue::state_file_in;
use crate::verdicts::VerdictRow;

/// The most a journal the fold reads may hold: past it, the fold refuses rather than read a part.
const CAP: u64 = 256 * 1024 * 1024;

/// The throne's landings a done one must not be reopened by.
const REOPEN_WITHIN: usize = 3;

/// Why the fold gave no reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// No home: no state directory to keep trusted journals under.
    NoState,
    /// A journal's chain does not hold, or its tail is torn.
    ChainBroken {
        /// The journal, as the chains name it.
        journal: String,
        /// The first line that breaks it.
        line: usize,
        /// Why.
        why: String,
    },
    /// A journal cannot be read whole, or a line of it is no row.
    Unreadable {
        /// The journal.
        journal: String,
        /// Why.
        why: String,
    },
    /// A journal's path lies inside the world, where a body can write it.
    InWorld {
        /// The journal.
        journal: String,
    },
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unavailable::NoState => f.write_str("no state directory to keep the journals under"),
            Unavailable::ChainBroken { journal, line, why } => {
                write!(f, "the {journal} chain is broken at line {line}: {why}")
            }
            Unavailable::Unreadable { journal, why } => {
                write!(f, "the {journal} journal cannot be read: {why}")
            }
            Unavailable::InWorld { journal } => {
                write!(
                    f,
                    "the {journal} journal lies inside the world: not trusted"
                )
            }
        }
    }
}

/// The fold: the report rows, and each creature's inputs over them, by creature and race.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Folded {
    /// The report ledger's rows, in order.
    pub rows: Vec<ReportRow>,
    /// Each creature's inputs, by creature and race.
    pub inputs: BTreeMap<(String, String), Inputs>,
}

impl Folded {
    /// The inputs of `creature` as a body of `race`'s base race: the usage witness names the race
    /// a body ran as, so a doc that claims another race claims no record.
    #[must_use]
    pub fn of(&self, creature: &str, race: Race) -> Inputs {
        self.inputs
            .iter()
            .filter(|((c, r), _)| c == creature && counts_as(r, race))
            .fold(Inputs::default(), |mut acc, (_, i)| {
                merge(&mut acc, i);
                acc
            })
    }
}

/// Whether a record kept under the race `recorded` (as the usage witness names it) counts for a
/// creature of `race`: the one place meters key by race.
fn counts_as(recorded: &str, race: Race) -> bool {
    Race::from_name(recorded).is_some_and(|r| r.base() == race.base())
}

/// `i` added to `acc`.
pub(crate) fn merge(acc: &mut Inputs, i: &Inputs) {
    use mnemorph_races::Rate;
    let sum = |a: Rate, b: Rate| Rate::of(a.successes + b.successes, a.n + b.n);
    acc.reports += i.reports;
    acc.land = sum(acc.land, i.land);
    acc.gate = sum(acc.gate, i.gate);
    acc.holes = sum(acc.holes, i.holes);
    acc.verify_green = sum(acc.verify_green, i.verify_green);
    acc.plan_land = sum(acc.plan_land, i.plan_land);
    acc.reask = sum(acc.reask, i.reask);
    acc.gate_right = sum(acc.gate_right, i.gate_right);
    acc.outward_clean = sum(acc.outward_clean, i.outward_clean);
    acc.rollbacks += i.rollbacks;
    acc.ask_done = sum(acc.ask_done, i.ask_done);
    acc.outcomes.extend_from_slice(&i.outcomes);
}

/// `world`'s reports folded from the trusted journals under the human's state directory; nothing
/// is written.
///
/// # Errors
///
/// No state directory, or a journal that cannot be trusted ([`Unavailable`]).
pub fn fold(world: &Path) -> Result<Folded, Unavailable> {
    fold_in(
        world,
        &crate::queue::state_dir().ok_or(Unavailable::NoState)?,
    )
}

/// [`fold`], the state directory at `state`.
///
/// # Errors
///
/// As [`fold`].
pub fn fold_in(world: &Path, state: &Path) -> Result<Folded, Unavailable> {
    let usage: Arc<Vec<UsageRow>> = trusted(
        world,
        state,
        &state_file_in(state, world, "usage"),
        crate::chains::USAGE_WITNESS,
        rows_of,
    )?;
    let verdicts: Arc<Vec<VerdictRow>> = trusted(
        world,
        state,
        &state_file_in(state, world, "gate"),
        "gate",
        rows_of,
    )?;
    // The challenge records folded from the very bytes their chain was walked over: one read.
    let upheld: Arc<Vec<String>> = trusted(
        world,
        state,
        &state_file_in(state, world, "challenges"),
        "value",
        |bytes| Ok(upheld_runs(bytes)),
    )?;
    let mut posts = Vec::new();
    for post in Post::ALL {
        let path = crate::chains::post_witness_in(state, world, post);
        posts.push(trusted::<PostRow>(
            world,
            state,
            &path,
            post.creature(),
            rows_of,
        )?);
    }
    let mut inputs: Vec<Arc<dyn Any + Send + Sync>> =
        vec![usage.clone(), verdicts.clone(), upheld.clone()];
    inputs.extend(
        posts
            .iter()
            .map(|p| p.clone() as Arc<dyn Any + Send + Sync>),
    );
    let at = (world.to_path_buf(), state.to_path_buf());
    // No journal read again since the last fold: its rows are the fold's still.
    if let Some((was, folded)) = lock(&FOLDS).get(&at)
        && was.len() == inputs.len()
        && was.iter().zip(&inputs).all(|(a, b)| Arc::ptr_eq(a, b))
    {
        return Ok(folded.clone());
    }
    let upheld_set: BTreeSet<String> = upheld.iter().cloned().collect();
    let mut rows = run_reports(&usage, &verdicts, &upheld_set);
    rows.extend(ask_reports(&usage, &verdicts));
    for (post, rows_of_post) in Post::ALL.into_iter().zip(&posts) {
        rows.extend(act_reports(post, rows_of_post));
    }
    let folded = Folded {
        inputs: fold_rows(&rows),
        rows,
    };
    lock(&FOLDS).insert(at, (inputs, folded.clone()));
    Ok(folded)
}

/// Where `world`'s report ledger is kept: `$XDG_STATE_HOME/mnemorph/reports/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` with no home.
#[must_use]
pub fn ledger_of(world: &Path) -> Option<PathBuf> {
    crate::queue::state_file(world, "reports")
}

/// Folds `world`'s reports and keeps them in its report ledger: the ledger is put in place whole,
/// by a rename, when its bytes differ from the fold's; returns the fold and the ledger's path.
///
/// # Errors
///
/// The fold is unavailable, or the ledger cannot be written.
pub fn record(world: &Path) -> Result<(Folded, PathBuf), String> {
    let state = crate::queue::state_dir().ok_or_else(|| Unavailable::NoState.to_string())?;
    record_in(world, &state)
}

/// [`record`], the state directory at `state`.
///
/// # Errors
///
/// As [`record`].
pub fn record_in(world: &Path, state: &Path) -> Result<(Folded, PathBuf), String> {
    let folded = fold_in(world, state).map_err(|e| e.to_string())?;
    let path = state_file_in(state, world, "reports");
    let bytes = ledger_bytes(&folded.rows).map_err(|e| e.to_string())?;
    keep(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((folded, path))
}

/// `bytes` kept at `path`: unchanged when it holds them already, else written beside it and put
/// in place by a rename, never edited in place. The folder is `0700`, the file `0600`.
fn keep(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if std::fs::read(path).is_ok_and(|now| now == bytes) {
        return Ok(());
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    mnemorph_ledger::private_dir(dir)?;
    let tmp = path.with_extension("jsonl.new");
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let mut f = o.open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_data()?;
    drop(f);
    std::fs::rename(&tmp, path)?;
    std::fs::File::open(dir)?.sync_all()
}

/// `path` resolved, or its nearest existing ancestor resolved with the rest.
fn real(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| {
        p.ancestors()
            .find_map(|a| {
                let rest = p.strip_prefix(a).ok()?;
                a.canonicalize().ok().map(|r| r.join(rest))
            })
            .unwrap_or_else(|| p.to_path_buf())
    })
}

/// What a journal was when its chain was last walked: absent, or a file the kernel says has not
/// changed since — its device, inode, size, modification and change times (the change time no
/// writer can set) — each held to the same anchor. The same key, the same bytes: no reread.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Key {
    Absent(Option<Head>),
    File {
        dev: u64,
        ino: u64,
        len: u64,
        mtime: (i64, i64),
        ctime: (i64, i64),
        anchor: Option<Head>,
    },
}

#[cfg(unix)]
fn key_of(m: &std::fs::Metadata, anchor: Option<Head>) -> Key {
    use std::os::unix::fs::MetadataExt as _;
    Key::File {
        dev: m.dev(),
        ino: m.ino(),
        len: m.len(),
        mtime: (m.mtime(), m.mtime_nsec()),
        ctime: (m.ctime(), m.ctime_nsec()),
        anchor,
    }
}

#[cfg(not(unix))]
fn key_of(m: &std::fs::Metadata, anchor: Option<Head>) -> Key {
    let ns = |t: io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or((0, 0), |d| {
                (
                    i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
                    i64::from(d.subsec_nanos()),
                )
            })
    };
    Key::File {
        dev: 0,
        ino: 0,
        len: m.len(),
        mtime: ns(m.modified()),
        ctime: ns(m.created()),
        anchor,
    }
}

/// How long before a read a file's last change must lie for its key to stand for its bytes: the
/// kernel stamps change times at its clock tick, so two writes inside one tick can share a time.
const TICK: std::time::Duration = std::time::Duration::from_millis(100);

/// Whether `key`'s file last changed at least a [`TICK`] before `read_at`.
fn settled(key: &Key, read_at: std::time::SystemTime) -> bool {
    let Key::File { ctime, mtime, .. } = key else {
        return true;
    };
    let at = |(s, ns): (i64, i64)| {
        u64::try_from(s).ok().map(|s| {
            std::time::UNIX_EPOCH + std::time::Duration::new(s, u32::try_from(ns).unwrap_or(0))
        })
    };
    let last = at(*ctime).max(at(*mtime));
    last.is_some_and(|t| t + TICK <= read_at)
}

type Rows = Arc<dyn Any + Send + Sync>;

/// Each journal's rows as its last walk left them, by path.
static JOURNALS: Mutex<BTreeMap<PathBuf, (Key, Rows)>> = Mutex::new(BTreeMap::new());

/// Each world's last fold, with the journals' rows it was folded from.
static FOLDS: Mutex<BTreeMap<(PathBuf, PathBuf), LastFold>> = Mutex::new(BTreeMap::new());

/// A fold, and the journals' rows it was folded from.
type LastFold = (Vec<Rows>, Folded);

/// How many times each journal was read whole: what a test counts.
#[cfg(test)]
static SCANS: Mutex<BTreeMap<PathBuf, usize>> = Mutex::new(BTreeMap::new());

#[cfg(test)]
fn scans(path: &Path) -> usize {
    lock(&SCANS).get(path).copied().unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Every line of `bytes` a row of `T`: a line read past is a report lost.
fn rows_of<T: DeserializeOwned>(bytes: &[u8]) -> Result<Vec<T>, String> {
    let back = mnemorph_ledger::parse_rows(bytes).map_err(|e| e.to_string())?;
    match back.skipped.first() {
        Some(line) => Err(format!("line {line} is no row it can read")),
        None => Ok(back.rows),
    }
}

/// The rows of the journal `name` at `path`: a regular file outside the world, read under its
/// shared lock, its chain walked over the bytes read and held to the head anchored for `name`
/// under the state directory, then `parse`d from those same bytes. An absent journal holds no
/// row — unless a head was anchored for it, which it can no longer hold. A journal unchanged since
/// its last walk ([`Key`]) is not read again.
fn trusted<T: Send + Sync + 'static>(
    world: &Path,
    state: &Path,
    path: &Path,
    name: &str,
    parse: impl FnOnce(&[u8]) -> Result<Vec<T>, String>,
) -> Result<Arc<Vec<T>>, Unavailable> {
    let journal = || name.to_owned();
    if real(path).starts_with(real(world)) || real(state).starts_with(real(world)) {
        return Err(Unavailable::InWorld { journal: journal() });
    }
    let unreadable = |why: String| Unavailable::Unreadable {
        journal: journal(),
        why,
    };
    let anchor = crate::chains::anchor_under(state, world, name);
    let file = crate::regular::open(path).map_err(|e| unreadable(e.to_string()))?;
    let key = match &file {
        None => Key::Absent(anchor.clone()),
        Some(f) => key_of(
            &f.metadata().map_err(|e| unreadable(e.to_string()))?,
            anchor.clone(),
        ),
    };
    if let Some((k, rows)) = lock(&JOURNALS).get(path)
        && *k == key
        && let Ok(rows) = Arc::clone(rows).downcast::<Vec<T>>()
    {
        return Ok(rows);
    }
    // Read now; kept for the next fold only when the file's last change is older than the
    // clock's coarsest step, so a change within the same tick is never taken for none.
    let read_at = std::time::SystemTime::now();
    let Some(file) = file else {
        if let Some(head) = anchor {
            return Err(Unavailable::ChainBroken {
                journal: journal(),
                line: head.line,
                why: format!("the journal is gone, and the head anchored for it ({head}) with it"),
            });
        }
        let rows = Arc::new(Vec::new());
        lock(&JOURNALS).insert(path.to_path_buf(), (key, rows.clone()));
        return Ok(rows);
    };
    file.lock_shared().map_err(|e| unreadable(e.to_string()))?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(&file, CAP.saturating_add(1)),
        &mut bytes,
    )
    .map_err(|e| unreadable(e.to_string()))?;
    drop(file);
    #[cfg(test)]
    {
        *lock(&SCANS).entry(path.to_path_buf()).or_default() += 1;
    }
    if bytes.len() as u64 > CAP {
        return Err(unreadable(format!("it is over {CAP} bytes")));
    }
    let mut walk = Walk::new();
    if let Some(head) = anchor {
        walk = walk.anchored(head);
    }
    walk.feed(&bytes);
    let report = walk.report();
    if let Some((line, why)) = report.broken {
        return Err(Unavailable::ChainBroken {
            journal: journal(),
            line,
            why: why.to_string(),
        });
    }
    if let Some(line) = report.truncated {
        return Err(Unavailable::ChainBroken {
            journal: journal(),
            line,
            why: "its tail is torn: rows may be missing".to_owned(),
        });
    }
    if report.head.is_none() && report.legacy > 0 {
        return Err(Unavailable::ChainBroken {
            journal: journal(),
            line: 1,
            why: "no row is chained: nothing binds it".to_owned(),
        });
    }
    let rows = Arc::new(parse(&bytes).map_err(unreadable)?);
    if settled(&key, read_at) {
        lock(&JOURNALS).insert(path.to_path_buf(), (key, rows.clone()));
    } else {
        lock(&JOURNALS).remove(path);
    }
    Ok(rows)
}

/// The runs a Dark Elf's upheld finding names a landing of, folded from the challenge records'
/// bytes: a landing's id is its run's, then `:` and its step.
fn upheld_runs(bytes: &[u8]) -> Vec<String> {
    let found: BTreeSet<String> = crate::challenge::finding_rows_of(bytes)
        .into_iter()
        .filter(|f| matches!(f.standing, Standing::Confirmed | Standing::Upheld))
        .filter_map(|f| f.landing.rsplit_once(':').map(|(run, _)| run.to_owned()))
        .collect();
    found.into_iter().collect()
}

/// A run's id as a landing's id carries it: every character outside `[A-Za-z0-9._-]` a `-`, at
/// most 110 of them (the harness's `landing_id`).
fn landing_run(run: &str) -> String {
    run.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .take(110)
        .collect()
}

/// The gate's word, as the fold reads it; `None` for a verdict no check could run for — no kernel
/// guard (`, unrun`), or lines held back for the human to pin (`, unpinned`): the machine's or the
/// human's doing, never the body's. A pass no verify line ran on (`, unchecked`) lands as the
/// baseline and is never counted as verified: not verified.
fn word(v: &str) -> Option<Gate> {
    if v.contains(", unrun") || v.contains(", unpinned") {
        None
    } else if v.contains(", unchecked") {
        Some(Gate::NotVerified)
    } else if v.starts_with("PASS") {
        Some(Gate::Pass)
    } else if v.starts_with("NOT_VERIFIED") {
        Some(Gate::NotVerified)
    } else {
        Some(Gate::Fail)
    }
}

/// Whether a verdict's verify lines include one the human pinned (directly, or carried down by
/// relief): only such a line is the human's check ([`crate::pins::Pinned::source`]).
fn human_checked(v: &VerdictRow) -> bool {
    let human = crate::pins::Pinned {
        line: String::new(),
        by: crate::pins::HUMAN.to_owned(),
    }
    .source();
    v.verified_by.contains(&human)
}

/// The gate's word on `v` as the fold counts it: a pass is a landing verified only when a line
/// the human pinned held it — a pass on an Orc's pins alone, or from before verdicts named their
/// pins, is not verified.
fn gate_of(v: &VerdictRow) -> Option<Gate> {
    match word(&v.verdict)? {
        Gate::Pass if !human_checked(v) => Some(Gate::NotVerified),
        g => Some(g),
    }
}

/// What the verify lines said of `v`: green or not, read only when lines the human pinned ran.
fn verify_green(v: &VerdictRow) -> Option<bool> {
    (!v.unchecked && human_checked(v)).then(|| {
        v.unverified.is_empty() && !v.failures.iter().any(|f| f.starts_with("traits hold:"))
    })
}

/// A build as a report row carries it: a source row that carries none says `unstamped`.
fn stamped(b: Option<&Build>) -> Build {
    match b {
        Some(b) if *b != Build::default() => b.clone(),
        _ => mnemorph_ledger::reports::unstamped(),
    }
}

/// A run as the usage witness first names it.
struct RunAt {
    id: String,
    body: String,
    race: String,
    parent: Option<String>,
    ts: Timestamp,
}

/// Every run in the usage witness, in the order its first call was written.
fn runs_of(usage: &[UsageRow]) -> Vec<RunAt> {
    let mut seen = BTreeSet::new();
    usage
        .iter()
        .filter(|u| !u.run.is_empty() && !u.body.is_empty() && seen.insert(u.run.clone()))
        .map(|u| RunAt {
            id: u.run.clone(),
            body: u.body.clone(),
            race: u.race.clone(),
            parent: u.commission.clone().filter(|p| p != &u.body),
            ts: u.ts,
        })
        .collect()
}

/// The run of `runs` a verdict judged: the one it names, else its body's latest run begun at or
/// before it.
fn run_of(runs: &[RunAt], by_id: &BTreeMap<&str, usize>, v: &VerdictRow) -> Option<usize> {
    if let Some(run) = &v.run {
        return by_id.get(run.as_str()).copied();
    }
    runs.iter()
        .enumerate()
        .rev()
        .find(|(_, r)| r.body == v.body && r.ts <= v.ts)
        .map(|(i, _)| i)
}

/// Each verdict the fold reads, by its place, with its run's, in the journal's order.
fn judged(runs: &[RunAt], verdicts: &[VerdictRow]) -> Vec<(usize, usize)> {
    let by_id: BTreeMap<&str, usize> = runs
        .iter()
        .enumerate()
        .map(|(i, r)| (r.id.as_str(), i))
        .collect();
    verdicts
        .iter()
        .enumerate()
        .filter(|(_, v)| word(&v.verdict).is_some())
        .filter_map(|(n, v)| run_of(runs, &by_id, v).map(|i| (i, n)))
        .filter(|&(i, n)| runs[i].body == verdicts[n].body)
        .collect()
}

/// One report per run of the usage witness.
fn run_reports(
    usage: &[UsageRow],
    verdicts: &[VerdictRow],
    upheld: &BTreeSet<String>,
) -> Vec<ReportRow> {
    let runs = runs_of(usage);
    let mut of_run: Vec<Vec<&VerdictRow>> = vec![Vec::new(); runs.len()];
    for (i, n) in judged(&runs, verdicts) {
        of_run[i].push(&verdicts[n]);
    }
    let gate: Vec<Option<Gate>> = of_run
        .iter()
        .map(|vs| vs.last().and_then(|v| gate_of(v)))
        .collect();
    // Each run's parent: the commissioning body's latest run begun before it.
    let parent: Vec<Option<usize>> = runs
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let p = r.parent.as_deref()?;
            (0..i).rev().find(|&k| runs[k].body == p)
        })
        .collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); runs.len()];
    for (i, p) in parent.iter().enumerate() {
        if let Some(p) = p {
            children[*p].push(i);
        }
    }
    // Whether a pass and a fail lie under each run, itself included; a child comes after its
    // parent, so the walk from the end sees every child first.
    let mut under = vec![(false, false); runs.len()];
    for i in (0..runs.len()).rev() {
        let mut at = (gate[i] == Some(Gate::Pass), gate[i] == Some(Gate::Fail));
        for &c in &children[i] {
            at = (at.0 || under[c].0, at.1 || under[c].1);
        }
        under[i] = at;
    }
    runs.iter()
        .enumerate()
        .map(|(i, r)| {
            let vs = &of_run[i];
            let last = vs.last();
            let mut bodies = BTreeSet::new();
            let (mut landed, mut reasks, mut passes, mut overturned) = (0u32, 0u32, 0u32, 0u32);
            for &c in &children[i] {
                if under[c].0 && !under[c].1 {
                    landed += 1;
                }
                if !bodies.insert(runs[c].body.as_str()) {
                    reasks += 1;
                }
                if gate[c] == Some(Gate::Pass) {
                    passes += 1;
                    if upheld.contains(&landing_run(&runs[c].id)) {
                        overturned += 1;
                    }
                }
            }
            let holes = vs
                .iter()
                .flat_map(|v| &v.failures)
                .filter(|f| f.starts_with("tests intact:"))
                .count();
            ReportRow::Run(RunReport {
                creature: r.body.clone(),
                race: r.race.clone(),
                run: r.id.clone(),
                ts: r.ts,
                build: stamped(last.map(|v| &v.build)),
                gate: gate[i],
                first_try: vs.len() == 1 && gate[i] == Some(Gate::Pass),
                holes: u32::try_from(holes).unwrap_or(u32::MAX),
                verify_green: last.and_then(|v| verify_green(v)),
                dispatches: u32::try_from(children[i].len()).unwrap_or(u32::MAX),
                landed,
                reasks,
                passes,
                overturned,
            })
        })
        .collect()
}

/// One report per landing of a throne body the gate judged, a hand-back aside (the turn goes on).
fn ask_reports(usage: &[UsageRow], verdicts: &[VerdictRow]) -> Vec<ReportRow> {
    let runs = runs_of(usage);
    let asks: Vec<(&RunAt, usize, &VerdictRow)> = judged(&runs, verdicts)
        .into_iter()
        .map(|(i, n)| (&runs[i], n, &verdicts[n]))
        .filter(|(r, _, v)| {
            Race::from_name(&r.race).is_some_and(|x| x.base() == Race::Rimuru)
                && !v.verdict.contains("handed back")
        })
        .collect();
    asks.iter()
        .enumerate()
        .map(|(k, (r, n, v))| {
            let reopened = asks[k + 1..]
                .iter()
                .filter(|(o, _, _)| o.body == r.body)
                .take(REOPEN_WITHIN)
                .any(|(_, _, later)| later.changed.iter().any(|p| v.changed.contains(p)));
            ReportRow::Ask(AskReport {
                creature: r.body.clone(),
                race: r.race.clone(),
                verdict: n + 1,
                ts: v.ts,
                build: stamped(Some(&v.build)),
                done: gate_of(v) == Some(Gate::Pass) && !reopened,
            })
        })
        .collect()
}

/// One row of a Kijin post's witness, as the fold reads it: the act, the state it entered, and on
/// an undo's rows the act it undoes.
#[derive(Debug, Clone, Deserialize)]
struct PostRow {
    act: String,
    state: String,
    #[serde(default)]
    undoes: Option<String>,
    #[serde(default)]
    build: Option<Build>,
    #[serde(default)]
    at: Option<Timestamp>,
}

/// One report per outward act of `post` that reached an outcome: its latest state, an undo's own
/// acts aside (the harness's, putting one back).
fn act_reports(post: Post, rows: &[PostRow]) -> Vec<ReportRow> {
    let mut order: Vec<&str> = Vec::new();
    let mut last: BTreeMap<&str, &PostRow> = BTreeMap::new();
    let mut began: BTreeMap<&str, Timestamp> = BTreeMap::new();
    let undos: BTreeSet<&str> = rows
        .iter()
        .filter(|r| r.undoes.is_some())
        .map(|r| r.act.as_str())
        .collect();
    for r in rows {
        if last.insert(&r.act, r).is_none() {
            order.push(&r.act);
            began.insert(&r.act, r.at.unwrap_or(Timestamp(0)));
        }
    }
    order
        .into_iter()
        .filter(|a| !undos.contains(a))
        .filter_map(|a| {
            let row = last[a];
            let (clean, rolled_back) = match row.state.as_str() {
                "observed" | "unobserved" | "settled" => (true, false),
                "reconcile" => (false, false),
                "corrected" => (false, true),
                // Still open, or never landed: no outcome yet.
                _ => return None,
            };
            Some(ReportRow::Act(ActReport {
                creature: post.creature().to_owned(),
                race: Race::Kijin.name().to_owned(),
                act: a.to_owned(),
                ts: began[a],
                // The build of the row that settled it.
                build: stamped(row.build.as_ref()),
                clean,
                rolled_back,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests;
