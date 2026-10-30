//! An evolution judged after the freeze: on its post-swap window alone, against doing nothing,
//! over exactly the files it named.
//!
//! - **The swap** ([`Swap`]) is one row of the evolution ledger: the creature, the arm, the build
//!   that swapped it in, the exact files it changes, each hashed before and after, and the diff
//!   the harness saw between the soil before and after the swap ([`snapshot`], [`diff`]).
//! - **The file set.** The diff must be exactly the named files, with the named hashes: a file the
//!   swap changed and did not name contaminates it, a named file the swap did not change is
//!   missing, a file created must not have existed. A run of the window must find and leave the
//!   named files in their after state, a control run in its before state ([`state`]).
//! - **The window.** Only runs after the swap's row, stamped with its id and its build, by one of
//!   its bodies, are judged — the first [`EVOLUTION_WINDOW`] of them. A row before the swap never
//!   counts, whatever it is stamped with.
//! - **The held-out tasks.** A window's task is used once: never one the plan was proven on before
//!   the swap, and never one any earlier run row carries — stamped or not, of this swap or another
//!   — but this swap's own controls. Tasks are told apart by fingerprint.
//! - **The control.** Every window task is also run without the change, after the swap, by the
//!   same body — a do-nothing fixture, stamped as the control. Paired by task, the evolution is
//!   kept only on at least `min_net_wins` net wins (tasks it landed and the control did not, less
//!   the reverse; `evolution.min_net_wins`, 2 unless the human sets it) and a `holeRate` no higher.
//!   Else it rolls back.
//! - **Replay.** The ledger is chained and harness-only, kept outside the world; it is read held to
//!   the newest head anchored for it, and an unchained ledger is never judged. A judgement's lines
//!   carry the chain digest of the window's last row and the window's first and last row; the
//!   ledger read again up to that row gives the same judgement ([`replay`]).
//!
//! A keep or a rollback is only proposed here: applying either is the human's.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use mnemorph_races::{Explanation, GateWord, Kind, Op, Outcome, Said, Scale, fold_reports};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::Build;
use crate::chain::{self, Head, Report, Walk};

/// How many post-swap runs an evolution is judged on.
pub const EVOLUTION_WINDOW: usize = 10;

/// The unit a judgement's lines read their window in.
pub const UNIT: &str = "runs beside do-nothing";

/// How a creature evolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arm {
    /// Two young of the same race.
    Split,
    /// A repeated lesson becomes a trait, a verify line or a Mind.
    Mutate,
    /// The doc rewritten from the code as it is.
    Rebirth,
}

/// One file of a swap: its path in the world and its SHA-256 before and after; `None` is absent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileChange {
    /// The path, relative to the world, `/`-separated.
    pub path: String,
    /// Its digest before the swap; `None` when the swap creates it.
    pub before: Option<String>,
    /// Its digest after the swap; `None` when the swap removes it.
    pub after: Option<String>,
}

/// An evolution swapped in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    /// The swap's id, which every run of its window is stamped with.
    pub id: String,
    /// The creature that evolved.
    pub creature: String,
    /// The bodies whose runs judge it: the creature, or for a split its young.
    pub bodies: Vec<String>,
    /// The arm.
    pub arm: Arm,
    /// The build that swapped it in: every run of its window carries it.
    pub build: Build,
    /// The exact files it names.
    pub files: Vec<FileChange>,
    /// What the harness saw change between the soil before and after the swap.
    pub diff: Vec<FileChange>,
    /// The fingerprints of the tasks the plan was proven on before the swap.
    #[serde(default)]
    pub proven_on: Vec<String>,
}

/// The gate's word on a run's landing, as the ledger keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Gate {
    /// Every check ran and held.
    Pass,
    /// A check ran and did not hold.
    Fail,
    /// Nothing failed, but a check gave no result.
    NotVerified,
}

/// One run of a task: an evolved body's, or the do-nothing control's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// The swap the run was stamped with; `None` for a run no swap stamped.
    #[serde(default)]
    pub swap: Option<String>,
    /// The build that ran it.
    #[serde(default)]
    pub build: Build,
    /// The body that ran it.
    pub body: String,
    /// The task's fingerprint.
    pub task: String,
    /// The do-nothing control: the same task, without the change.
    #[serde(default)]
    pub control: bool,
    /// The named files' state as the run found them ([`state`]).
    pub state: String,
    /// The named files' state as the run left them: a change made mid-run shows here.
    pub state_after: String,
    /// The gate's word on its landing; `None` when nothing landed.
    pub gate: Option<Gate>,
    /// The gate passed on its first try.
    #[serde(default)]
    pub first_try: bool,
    /// Holes the harness found: an edited or deleted verify line.
    #[serde(default)]
    pub holes_found: u32,
}

impl Run {
    /// What the report fold reads: the harness's facts alone.
    fn outcome(&self) -> Outcome {
        Outcome {
            said: Said::Other,
            holes_said: 0,
            gate: self.gate.map(|g| match g {
                Gate::Pass => GateWord::Pass,
                Gate::Fail => GateWord::Fail,
                Gate::NotVerified => GateWord::NotVerified,
            }),
            first_try: self.first_try,
            holes_found: self.holes_found,
        }
    }
}

/// One row of the evolution ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "row", rename_all = "kebab-case")]
pub enum EvolutionRow {
    /// An evolution swapped in.
    Swap(Swap),
    /// A run, evolved or control.
    Run(Run),
}

/// A row as read back: its line, from 1, and its own chain digest when it is a holding link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The row's line in the ledger, from 1, as its chain counts lines.
    pub at: u64,
    /// The row.
    pub row: EvolutionRow,
    /// The row's digest when it is chained and the chain holds through it.
    pub digest: Option<String>,
}

/// The evolution ledger read once: its rows and its chain, over the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ledger {
    /// Every row that parsed, in order.
    pub entries: Vec<Entry>,
    /// The chain walked over the bytes read.
    pub chain: Report,
    /// Lines that did not parse.
    pub skipped: Vec<u64>,
}

/// The SHA-256 of `bytes`, 64 lowercase hex digits.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    use fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Every file under `root`, by its `/`-separated path relative to it, with its digest. A link is
/// hashed as its target's name, never followed.
///
/// # Errors
///
/// A directory or file under `root` cannot be read.
pub fn snapshot(root: &Path) -> io::Result<BTreeMap<String, String>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> io::Result<()> {
        for e in fs::read_dir(dir)? {
            let e = e?;
            let path = e.path();
            let kind = e.file_type()?;
            let rel = path
                .strip_prefix(root)
                .map_err(io::Error::other)?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if kind.is_symlink() {
                let to = fs::read_link(&path)?;
                let mut bytes = b"link:".to_vec();
                bytes.extend_from_slice(to.to_string_lossy().as_bytes());
                out.insert(rel, digest(&bytes));
            } else if kind.is_dir() {
                walk(root, &path, out)?;
            } else {
                out.insert(rel, digest(&fs::read(&path)?));
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

/// Every path whose digest differs between two snapshots, in path order.
#[must_use]
pub fn diff(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<FileChange> {
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter(|p| before.get(*p) != after.get(*p))
        .map(|p| FileChange {
            path: p.clone(),
            before: before.get(p).cloned(),
            after: after.get(p).cloned(),
        })
        .collect()
}

/// The state of a set of files: one digest over each path and its digest, in path order; an
/// absent file reads `-`.
#[must_use]
pub fn state<'a>(files: impl IntoIterator<Item = (&'a str, Option<&'a str>)>) -> String {
    let sorted: BTreeMap<&str, Option<&str>> = files.into_iter().collect();
    let mut bytes = Vec::new();
    for (p, d) in sorted {
        bytes.extend_from_slice(p.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(d.unwrap_or("-").as_bytes());
        bytes.push(b'\n');
    }
    digest(&bytes)
}

/// The state of `swap`'s named files as they stand under `root` now: what a run stamps.
///
/// # Errors
///
/// A named file exists but cannot be read.
pub fn state_now(root: &Path, swap: &Swap) -> io::Result<String> {
    let mut found = Vec::new();
    for f in &swap.files {
        let d = match fs::read(root.join(&f.path)) {
            Ok(b) => Some(digest(&b)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        found.push((f.path.as_str(), d));
    }
    Ok(state(found.iter().map(|(p, d)| (*p, d.as_deref()))))
}

impl Swap {
    /// The named files' state before the swap: what a control run finds.
    #[must_use]
    pub fn before_state(&self) -> String {
        state(
            self.files
                .iter()
                .map(|f| (f.path.as_str(), f.before.as_deref())),
        )
    }

    /// The named files' state after the swap: what every run of its window finds.
    #[must_use]
    pub fn after_state(&self) -> String {
        state(
            self.files
                .iter()
                .map(|f| (f.path.as_str(), f.after.as_deref())),
        )
    }

    /// The exact file set: the swap names at least one file, each once, each changed; and the
    /// diff the harness saw is exactly the named files with the named digests.
    ///
    /// # Errors
    ///
    /// The first way the file set fails.
    pub fn check(&self) -> Result<(), Refusal> {
        if self.files.is_empty() {
            return Err(Refusal::NoFiles);
        }
        let mut named: BTreeMap<&str, &FileChange> = BTreeMap::new();
        for f in &self.files {
            if f.before == f.after {
                return Err(Refusal::Unchanged(f.path.clone()));
            }
            if named.insert(&f.path, f).is_some() {
                return Err(Refusal::NamedTwice(f.path.clone()));
            }
        }
        let mut seen = BTreeSet::new();
        for d in &self.diff {
            match named.get(d.path.as_str()) {
                None => return Err(Refusal::Contaminated(d.path.clone())),
                Some(f) if f.before != d.before || f.after != d.after => {
                    return Err(Refusal::Mismatch(d.path.clone()));
                }
                Some(_) => {
                    seen.insert(d.path.as_str());
                }
            }
        }
        match named.keys().find(|p| !seen.contains(**p)) {
            Some(p) => Err(Refusal::Missing((*p).to_owned())),
            None => Ok(()),
        }
    }
}

/// Why an evolution cannot be judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No swap with that id in the ledger.
    NoSwap(String),
    /// Two swaps carry that id.
    TwiceSwapped(String),
    /// The ledger's chain breaks at that line, or does not hold its anchored head.
    ChainBroken(usize),
    /// The ledger carries no chain, or the window's last row, at that line, is not a link of it:
    /// nothing would anchor the judgement for a replay.
    Unchained(Option<u64>),
    /// The swap names no file.
    NoFiles,
    /// A named file whose digest before is its digest after.
    Unchanged(String),
    /// A file named twice.
    NamedTwice(String),
    /// A file the swap changed and did not name.
    Contaminated(String),
    /// A named file the swap did not change.
    Missing(String),
    /// A named file the swap changed, but not from or to the named digest — a file it creates
    /// that already existed among them.
    Mismatch(String),
    /// A run of the window, at that line, found the named files other than the swap left them.
    Drifted(u64),
    /// A control run, at that line, did not find the named files as they were before the swap.
    ControlState(u64),
    /// A window task the plan was proven on before the swap.
    Leaked(String),
    /// A held-out task used before: twice in the window, or first by another swap's window.
    Reused(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NoSwap(id) => write!(f, "no swap `{id}` in the ledger"),
            Refusal::TwiceSwapped(id) => write!(f, "two swaps carry the id `{id}`"),
            Refusal::ChainBroken(line) => write!(f, "the ledger's chain breaks at line {line}"),
            Refusal::Unchained(None) => f.write_str("the ledger carries no chain"),
            Refusal::Unchained(Some(at)) => {
                write!(f, "the row at {at} is not a link of the ledger's chain")
            }
            Refusal::NoFiles => f.write_str("the swap names no file"),
            Refusal::Unchanged(p) => write!(f, "{p}: named, but the same before and after"),
            Refusal::NamedTwice(p) => write!(f, "{p}: named twice"),
            Refusal::Contaminated(p) => {
                write!(
                    f,
                    "{p}: changed in the same swap, and not named: contaminated"
                )
            }
            Refusal::Missing(p) => write!(f, "{p}: named, and the swap did not change it"),
            Refusal::Mismatch(p) => {
                write!(
                    f,
                    "{p}: changed, but not from or to the digest the swap names"
                )
            }
            Refusal::Drifted(at) => write!(
                f,
                "the run at row {at} found the named files other than the swap left them"
            ),
            Refusal::ControlState(at) => write!(
                f,
                "the control at row {at} did not find the named files as they were before"
            ),
            Refusal::Leaked(t) => write!(f, "task {t}: the plan was proven on it before the swap"),
            Refusal::Reused(t) => write!(f, "task {t}: a held-out task already used"),
        }
    }
}

/// What judging an evolution came to. A keep or a rollback is a proposal to the human.
#[derive(Debug, Clone, PartialEq)]
pub enum Judgement {
    /// The window or its controls are not complete yet.
    Pending {
        /// Runs in the window so far.
        runs: usize,
        /// Of those, how many have their control.
        controls: usize,
    },
    /// It cannot be judged.
    Refused(Refusal),
    /// It beat doing nothing on every bar: every bar, explained.
    Keep(Vec<Explanation>),
    /// It did not: each bar it failed, explained.
    Rollback(Vec<Explanation>),
}

impl fmt::Display for Judgement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Judgement::Pending { runs, controls } => write!(
                f,
                "pending: {runs}/{EVOLUTION_WINDOW} runs, {controls} with their control"
            ),
            Judgement::Refused(r) => write!(f, "refused: {r}"),
            Judgement::Keep(lines) | Judgement::Rollback(lines) => {
                let lines: Vec<String> = lines.iter().map(ToString::to_string).collect();
                f.write_str(&lines.join("\n"))
            }
        }
    }
}

/// Reads the evolution ledger at `path` once: its rows, numbered by line, and its chain over the
/// same bytes, held to `anchor` — the newest head kept for it outside the file — so a chain dropped
/// at its end or rooted again is broken. A missing file is an empty ledger.
///
/// # Errors
///
/// The file exists but cannot be read.
pub fn read(path: &Path, anchor: Option<&Head>) -> io::Result<Ledger> {
    read_upto(path, None, anchor)
}

/// [`read`], its rows up to and including line `to`; the chain is still walked over the whole file
/// and held to `anchor`.
///
/// # Errors
///
/// The file exists but cannot be read.
pub fn read_to(path: &Path, to: u64, anchor: Option<&Head>) -> io::Result<Ledger> {
    read_upto(path, Some(to), anchor)
}

fn read_upto(path: &Path, to: Option<u64>, anchor: Option<&Head>) -> io::Result<Ledger> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Ledger::default()),
        Err(e) => return Err(e),
    };
    let mut out = Ledger::default();
    let mut walk = Walk::new();
    if let Some(a) = anchor {
        walk = walk.anchored(a.clone());
    }
    let mut chained = false;
    let mut rest: &[u8] = &bytes;
    let mut n: u64 = 0;
    while !rest.is_empty() {
        n += 1;
        let (line, next) = match rest.iter().position(|&b| b == b'\n') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => (rest, &rest[rest.len()..]),
        };
        let whole = &rest[..rest.len() - next.len()];
        rest = next;
        walk.feed(whole);
        if to.is_some_and(|t| n > t) || line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let sealed = chain::is_sealed(line);
        chained |= sealed;
        if chained && !sealed {
            out.skipped.push(n);
            continue;
        }
        let Ok(row) = serde_json::from_slice::<EvolutionRow>(&chain::body(line)) else {
            out.skipped.push(n);
            continue;
        };
        let report = walk.report();
        let digest = report
            .head
            .filter(|h| u64::try_from(h.line).ok() == Some(n))
            .map(|h| h.digest);
        out.entries.push(Entry { at: n, row, digest });
    }
    out.chain = walk.report();
    Ok(out)
}

/// Judges the swap `id` over `ledger`: its file set, its post-swap window, its held-out tasks and
/// its do-nothing controls; kept on at least `min_net_wins` net paired wins
/// (`evolution.min_net_wins`).
#[must_use]
pub fn judge(ledger: &Ledger, id: &str, min_net_wins: u32) -> Judgement {
    match judged(ledger, id, min_net_wins) {
        Ok(j) => j,
        Err(r) => Judgement::Refused(r),
    }
}

/// Reads the ledger at `path` up to line `to` — the last row a judgement's lines name — its chain
/// held to `anchor`, and judges `id` again: the same ledger gives the same judgement.
///
/// # Errors
///
/// The file exists but cannot be read.
pub fn replay(
    path: &Path,
    id: &str,
    to: u64,
    anchor: Option<&Head>,
    min_net_wins: u32,
) -> io::Result<Judgement> {
    Ok(judge(&read_to(path, to, anchor)?, id, min_net_wins))
}

fn judged(ledger: &Ledger, id: &str, min_net_wins: u32) -> Result<Judgement, Refusal> {
    if let Some((line, _)) = ledger.chain.broken {
        return Err(Refusal::ChainBroken(line));
    }
    if ledger.chain.head.is_none() {
        return Err(Refusal::Unchained(None));
    }
    let mut swaps = ledger.entries.iter().filter_map(|e| match &e.row {
        EvolutionRow::Swap(s) if s.id == id => Some((e.at, s)),
        _ => None,
    });
    let (swapped_at, swap) = swaps.next().ok_or_else(|| Refusal::NoSwap(id.to_owned()))?;
    if swaps.next().is_some() {
        return Err(Refusal::TwiceSwapped(id.to_owned()));
    }
    swap.check()?;
    let after = swap.after_state();
    let before = swap.before_state();
    let runs = || {
        ledger.entries.iter().filter_map(|e| match &e.row {
            EvolutionRow::Run(r) => Some((e, r)),
            EvolutionRow::Swap(_) => None,
        })
    };
    let window: Vec<(&Entry, &Run)> = runs()
        .filter(|(e, r)| {
            !r.control
                && e.at > swapped_at
                && r.swap.as_deref() == Some(id)
                && r.build == swap.build
                && swap.bodies.contains(&r.body)
        })
        .take(EVOLUTION_WINDOW)
        .collect();
    let proven: BTreeSet<&str> = swap.proven_on.iter().map(String::as_str).collect();
    let mut used = BTreeSet::new();
    for (e, r) in &window {
        if r.state != after || r.state_after != after {
            return Err(Refusal::Drifted(e.at));
        }
        if proven.contains(r.task.as_str()) {
            return Err(Refusal::Leaked(r.task.clone()));
        }
        // Any earlier run of the task, stamped or not, but this swap's own controls.
        let elsewhere = runs().any(|(o, other)| {
            o.at < e.at
                && other.task == r.task
                && !(other.control && other.swap.as_deref() == Some(id))
        });
        if !used.insert(r.task.as_str()) || elsewhere {
            return Err(Refusal::Reused(r.task.clone()));
        }
    }
    let controls: Vec<(&Entry, &Run)> = window
        .iter()
        .filter_map(|(_, w)| {
            runs().find(|(e, c)| {
                c.control
                    && e.at > swapped_at
                    && c.swap.as_deref() == Some(id)
                    && c.build == swap.build
                    && c.body == w.body
                    && c.task == w.task
            })
        })
        .collect();
    if window.len() < EVOLUTION_WINDOW || controls.len() < window.len() {
        return Ok(Judgement::Pending {
            runs: window.len(),
            controls: controls.len(),
        });
    }
    if let Some((e, _)) = controls
        .iter()
        .find(|(_, c)| c.state != before || c.state_after != before)
    {
        return Err(Refusal::ControlState(e.at));
    }
    verdict(ledger, &window, &controls, min_net_wins)
}

/// A full window against its controls, paired by task: kept when its net paired wins — tasks it
/// landed and the control did not, less those the control landed and it did not — reach
/// `min_net_wins` and its `holeRate` is no higher; else rolled back on each bar it failed. Every
/// line carries the chain digest of the window's last row.
fn verdict(
    ledger: &Ledger,
    window: &[(&Entry, &Run)],
    controls: &[(&Entry, &Run)],
    min_net_wins: u32,
) -> Result<Judgement, Refusal> {
    let lines = window.iter().chain(controls).map(|(e, _)| e.at);
    let rows = lines.clone().min().zip(lines.max());
    let last = rows.map(|(_, to)| to);
    let head = last
        .and_then(|to| ledger.entries.iter().find(|e| e.at == to))
        .and_then(|e| e.digest.clone())
        .ok_or(Refusal::Unchained(last))?;
    let landed = |r: &Run| r.gate == Some(Gate::Pass);
    let net: i64 = window
        .iter()
        .zip(controls)
        .map(|((_, w), (_, c))| i64::from(landed(w)) - i64::from(landed(c)))
        .sum();
    let evolved = fold_reports(&window.iter().map(|(_, r)| r.outcome()).collect::<Vec<_>>());
    let nothing = fold_reports(
        &controls
            .iter()
            .map(|(_, r)| r.outcome())
            .collect::<Vec<_>>(),
    );
    let n = u32::try_from(window.len()).unwrap_or(u32::MAX);
    let line = |meter, scale, reading, op, threshold| {
        Explanation {
            kind: Kind::Keep,
            meter,
            scale,
            reading,
            op,
            threshold,
            window: n,
            unit: UNIT,
            recover: None,
            rerise: None,
            head: None,
            from: None,
            to: None,
        }
        .on_ledger(Some(&head), rows)
    };
    #[allow(clippy::cast_precision_loss, reason = "at most EVOLUTION_WINDOW pairs")]
    let bars = [
        line(
            "netWins",
            Scale::Count,
            net as f64,
            Op::Ge,
            f64::from(min_net_wins),
        ),
        line(
            "holeRate",
            Scale::Rate,
            evolved.hole_rate,
            Op::Le,
            nothing.hole_rate,
        ),
    ];
    if bars.iter().all(Explanation::holds) {
        return Ok(Judgement::Keep(bars.to_vec()));
    }
    Ok(Judgement::Rollback(
        bars.into_iter()
            .filter(|b| !b.holds())
            .map(|b| Explanation {
                kind: Kind::Rollback,
                op: match b.op {
                    Op::Gt => Op::Le,
                    Op::Le => Op::Gt,
                    Op::Ge => Op::Lt,
                    Op::Lt => Op::Ge,
                },
                ..b
            })
            .collect(),
    ))
}
