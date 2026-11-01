//! The report fold: every creature's reports, folded from the harness's journals under the human's
//! state directory into the report ledger ([`mnemorph_ledger::reports`]), and the inputs the race
//! law reads from it.
//!
//! **Only trusted journals are read**, each a witness the harness keeps outside the world, its
//! hash chain walked over the very bytes the fold reads and held to the head anchored under the
//! state directory:
//!
//! - the **usage witness** (`mnemorph/usage/`): each run, its body, its race and the body that
//!   commissioned it;
//! - the **gate verdicts** (`mnemorph/gate/`, anchored as `gate`): each landing's word, its
//!   failures and unverified checks, the paths it changed;
//! - the **challenge records** (`mnemorph/challenges/`, anchored as `value`): the findings upheld
//!   on a landing, which overturn the gate's pass on it;
//! - each **Kijin post's witness** (`mnemorph/kijin/`, anchored as `kijin-<post>`): each outward
//!   act and where it ended.
//!
//! A world's own copies — `.mnemorph/instruments/usage.jsonl`, a post journal, a run journal — are
//! never read: a body can write them. A journal whose chain does not hold, or whose tail is torn,
//! stops the fold ([`Unavailable::ChainBroken`]): its meters read unavailable, never stale, never
//! zero.
//!
//! The ledger is kept at `$XDG_STATE_HOME/mnemorph/reports/<world>-<fnv64a>.jsonl` ([`record`]):
//! rebuilt whole from the journals and put in place by a rename only when its bytes change, so a
//! replay of the same journals gives the same bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use mnemorph_ledger::chain::Walk;
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
            .filter(|((c, r), _)| {
                c == creature && Race::from_name(r).is_some_and(|r| r.base() == race.base())
            })
            .fold(Inputs::default(), |mut acc, (_, i)| {
                merge(&mut acc, i);
                acc
            })
    }
}

/// `i` added to `acc`.
fn merge(acc: &mut Inputs, i: &Inputs) {
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
    let usage: Vec<UsageRow> = trusted(
        world,
        state,
        &state_file_in(state, world, "usage"),
        "usage",
        false,
    )?;
    let verdicts: Vec<VerdictRow> = trusted(
        world,
        state,
        &state_file_in(state, world, "gate"),
        "gate",
        true,
    )?;
    let upheld = upheld_runs(world, state)?;
    let mut acts = Vec::new();
    for post in Post::ALL {
        let path = crate::chains::post_witness_in(state, world, post);
        let rows: Vec<PostRow> = trusted(world, state, &path, post.creature(), true)?;
        acts.extend(act_reports(post, &rows));
    }
    let mut rows = run_reports(&usage, &verdicts, &upheld);
    rows.extend(ask_reports(&usage, &verdicts));
    rows.extend(acts);
    let inputs = fold_rows(&rows);
    Ok(Folded { rows, inputs })
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

/// The rows of the journal `name` at `path`, read once: a regular file outside the world, its
/// chain walked over the bytes read — held to the head anchored under the state directory when
/// `anchored` — and every line a row. An absent journal holds no row.
fn trusted<T: DeserializeOwned>(
    world: &Path,
    state: &Path,
    path: &Path,
    name: &str,
    anchored: bool,
) -> Result<Vec<T>, Unavailable> {
    let journal = || name.to_owned();
    if real(path).starts_with(real(world)) || real(state).starts_with(real(world)) {
        return Err(Unavailable::InWorld { journal: journal() });
    }
    let bytes = match crate::regular::read(path, CAP) {
        Ok(Some(b)) => b,
        Ok(None) => return Ok(Vec::new()),
        Err(e) => {
            return Err(Unavailable::Unreadable {
                journal: journal(),
                why: e.to_string(),
            });
        }
    };
    let mut walk = Walk::new();
    if anchored {
        let kept = state_file_in(state, world, "anchors");
        if let Some(head) = crate::chains::anchored_in(&kept, name) {
            walk = walk.anchored(head);
        }
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
    let back = mnemorph_ledger::parse_rows(&bytes[..]).map_err(|e| Unavailable::Unreadable {
        journal: journal(),
        why: e.to_string(),
    })?;
    if let Some(line) = back.skipped.first() {
        return Err(Unavailable::Unreadable {
            journal: journal(),
            why: format!("line {line} is no row it can read"),
        });
    }
    Ok(back.rows)
}

/// The runs a Dark Elf's upheld finding names a landing of, as the challenge records hold them:
/// a landing's id is its run's, then `:` and its step.
fn upheld_runs(world: &Path, state: &Path) -> Result<BTreeSet<String>, Unavailable> {
    let path = state_file_in(state, world, "challenges");
    // The chain, walked and anchored over the records' lines, before any finding is read.
    trusted::<serde_json::Value>(world, state, &path, "value", true)?;
    let (rows, chain) = crate::challenge::Challenges::at(&path).finding_rows_chained();
    if let Some(r) = chain.filter(|r| !r.holds() || r.truncated.is_some()) {
        return Err(Unavailable::ChainBroken {
            journal: "value".to_owned(),
            line: r
                .broken
                .map_or_else(|| r.truncated.unwrap_or(0), |(l, _)| l),
            why: r.to_string(),
        });
    }
    Ok(rows
        .into_iter()
        .filter(|f| matches!(f.standing, Standing::Confirmed | Standing::Upheld))
        .filter_map(|f| f.landing.rsplit_once(':').map(|(run, _)| run.to_owned()))
        .collect())
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

/// The gate's word, as the fold reads it; `None` for a verdict no check could run for (no kernel
/// guard): the machine's doing, never the body's.
fn word(v: &str) -> Option<Gate> {
    if v.contains(", unrun") {
        None
    } else if v.starts_with("PASS") {
        Some(Gate::Pass)
    } else if v.starts_with("NOT_VERIFIED") {
        Some(Gate::NotVerified)
    } else {
        Some(Gate::Fail)
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
        .map(|vs| vs.last().and_then(|v| word(&v.verdict)))
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
                build: last.map(|v| v.build.clone()).unwrap_or_default(),
                gate: gate[i],
                first_try: vs.len() == 1 && gate[i] == Some(Gate::Pass),
                holes: u32::try_from(holes).unwrap_or(u32::MAX),
                verify_green: last.map(|v| {
                    v.unverified.is_empty()
                        && !v.failures.iter().any(|f| f.starts_with("traits hold:"))
                }),
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
                build: v.build.clone(),
                done: word(&v.verdict) == Some(Gate::Pass) && !reopened,
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
}

/// One report per outward act of `post` that reached an outcome: its latest state, an undo's own
/// acts aside (the harness's, putting one back).
fn act_reports(post: Post, rows: &[PostRow]) -> Vec<ReportRow> {
    let mut order: Vec<&str> = Vec::new();
    let mut last: BTreeMap<&str, &str> = BTreeMap::new();
    let undos: BTreeSet<&str> = rows
        .iter()
        .filter(|r| r.undoes.is_some())
        .map(|r| r.act.as_str())
        .collect();
    for r in rows {
        if last.insert(&r.act, &r.state).is_none() {
            order.push(&r.act);
        }
    }
    order
        .into_iter()
        .filter(|a| !undos.contains(a))
        .filter_map(|a| {
            let (clean, rolled_back) = match last[a] {
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
                build: Build::default(),
                clean,
                rolled_back,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests;
