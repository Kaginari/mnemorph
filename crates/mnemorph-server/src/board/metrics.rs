//! The board's metrics, as `GET /api/metrics?world=<id|all>&window=7d|28d|all` shows them: spend
//! and weighted tokens by session, day, body, race and model, the cache hit rate, the burn per
//! five minutes and the unpriced calls — folded by [`mnemorph_loop::world::spend`], the fold
//! `mnemorphd usage` prints — beside every living creature's meters with their count and 95%
//! interval (`mnemorphd meters`), the Dark Elf's scores (`mnemorphd challenge score`) and the
//! bench's result rows, when the human's global file names where they are.
//!
//! The usage ledger is read from its witness under the state directory, trusted while its chain
//! holds; a world with no witness yet is read from its own copy, untrusted, as the roster is. The
//! challenge records are the harness's, under the state directory. Dollars cover
//! priced calls only: no priced call reads no dollars, never $0. The bench is read from the
//! directory the human named, never from a world, and a held-out suite's rows are never opened.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mnemorph_loop::world::challenge::{self, Ascension};
use mnemorph_loop::world::ledger::{SpecialityFold, Timestamp, Totals};
use mnemorph_loop::world::meters::{Race, Rate};
use mnemorph_loop::world::regular;
use mnemorph_loop::world::spend::{self, BURN_SLOT, Window};
use mnemorph_loop::world::worth::USAGE;
use serde::Serialize;
use ts_rs::TS;

use super::overview::{Meter, Sourced, meter};
use super::snapshot;
use super::{BoardWorld, WorldState};

/// What a group of calls spent.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Spent {
    /// Calls.
    #[ts(type = "number")]
    pub calls: u64,
    /// Weighted tokens.
    #[ts(type = "number")]
    pub wt: u64,
    /// Uncached input tokens.
    #[ts(type = "number")]
    pub input: u64,
    /// Output tokens.
    #[ts(type = "number")]
    pub output: u64,
    /// Input tokens read from the cache.
    #[ts(type = "number")]
    pub cache_read: u64,
    /// Input tokens written to the cache.
    #[ts(type = "number")]
    pub cache_write: u64,
    /// US dollars over the priced calls; `null` when none was priced — never a made-up zero.
    pub usd: Option<f64>,
    /// Calls on unpriced models: `usd` does not cover them.
    #[ts(type = "number")]
    pub unpriced: u64,
    /// The share of input served from the cache; `null` with no input.
    pub cache_hit: Option<f64>,
}

impl From<&Totals> for Spent {
    fn from(t: &Totals) -> Self {
        Self {
            calls: t.calls,
            wt: t.wt,
            input: t.usage.input,
            output: t.usage.output,
            cache_read: t.usage.cache_read,
            cache_write: t.usage.cache_write,
            usd: spend::usd(t),
            unpriced: t.unpriced,
            cache_hit: spend::cache_hit(t),
        }
    }
}

/// A group's name and what it spent.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Named {
    /// The session, day, body, race or model.
    pub name: String,
    /// What it spent.
    pub spent: Spent,
}

/// One UTC day, by race: a stacked bar.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct DayRaces {
    /// The day, `YYYY-MM-DD`.
    pub day: String,
    /// The day's total.
    pub spent: Spent,
    /// Its races, by name.
    pub races: Vec<Named>,
}

/// One session in the window.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct SessionSpent {
    /// The session.
    pub session: String,
    /// The world it ran in, by id.
    pub world: String,
    /// Its first call in the window (RFC 3339, UTC).
    pub first: String,
    /// Its last.
    pub last: String,
    /// What it spent.
    pub spent: Spent,
}

/// The usage ledgers' fold over the window.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Usage {
    /// Everything.
    pub total: Spent,
    /// By UTC day, oldest first, each by race.
    pub days: Vec<DayRaces>,
    /// By race.
    pub races: Vec<Named>,
    /// By model.
    pub models: Vec<Named>,
    /// By body.
    pub bodies: Vec<Named>,
    /// By session, the latest first.
    pub sessions: Vec<SessionSpent>,
    /// Weighted tokens per slot of `burnSlotSecs`, oldest first; the last slot ends now.
    #[ts(type = "number[]")]
    pub burn: Vec<u64>,
    /// The burn's slot, in seconds.
    pub burn_slot_secs: u32,
    /// Weighted tokens per minute over the last slot.
    pub burn_per_minute: f64,
}

/// A living creature's meters, and what a silent one is silent since.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricsCreature {
    /// The world, by id.
    pub world: String,
    /// The creature.
    pub creature: String,
    /// Its race, by name.
    pub race: String,
    /// Its meters, in the race's order; one with no report has no count, never a zero.
    pub meters: Vec<Meter>,
    /// Its first call the usage ledger holds (RFC 3339, UTC): a meter with no report has been
    /// silent since; `null` when the ledger holds none of its calls.
    pub first_call: Option<String>,
}

/// One Dark Elf speciality's value ledger, as `mnemorphd challenge score` says it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Score {
    /// The world, by id.
    pub world: String,
    /// The speciality.
    pub speciality: String,
    /// The race of the living body that bears its name; `null` with none.
    pub form: Option<String>,
    /// Findings filed.
    pub findings: u32,
    /// Upheld.
    pub upheld: u32,
    /// Refuted.
    pub refuted: u32,
    /// Its verifier could not verify.
    pub cannot_verify: u32,
    /// Held for the human.
    pub held: u32,
    /// No verdict yet.
    pub unverified: u32,
    /// Precision: upheld of upheld and refuted.
    pub precision: Meter,
    /// Upheld of every finding.
    pub upheld_rate: Meter,
    /// Precision over the last ten settled.
    pub recent: Meter,
    /// Cannot-verify over the last ten filed.
    pub cannot_verify_rate: Meter,
    /// What the fold proposes to the human — `rise` or `probation` — which only the human
    /// applies; `null` for nothing.
    pub proposal: Option<String>,
}

/// One harness on one model in one mode of a bench suite.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BenchRow {
    /// The suite, as its results directory names it.
    pub suite: String,
    /// The harness.
    pub harness: String,
    /// The model.
    pub model: String,
    /// The harness's mode.
    pub mode: String,
    /// Tasks with a valid result: the last row of each task stands.
    pub valid: u32,
    /// Of those, passed.
    pub passed: u32,
    /// Rows not counted: a machine, provider or harness fault, to be rerun.
    pub invalid: u32,
    /// The bench's dollars over the valid rows priced; `null` when none was.
    pub usd: Option<f64>,
    /// Valid rows with no price: `usd` does not cover them.
    pub unpriced: u32,
    /// When its results file was last written (RFC 3339, UTC).
    pub date: Option<String>,
}

/// The bench's results, as the human's global file points to them.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Bench {
    /// Whether `board.bench_results` names a directory.
    pub configured: bool,
    /// The rows, by suite, harness, model and mode.
    pub rows: Vec<BenchRow>,
    /// Result files that did not read, or lines in them that did not parse.
    pub unreadable: u32,
}

/// `GET /api/metrics`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BoardMetrics {
    /// The world asked for, by id, or `all`.
    pub world: String,
    /// The window: `7d`, `28d` or `all`.
    pub window: String,
    /// The worlds folded, by id.
    pub worlds: Vec<String>,
    /// Their usage ledgers' fold.
    pub usage: Sourced<Usage>,
    /// Every living creature's meters.
    pub meters: Sourced<Vec<MetricsCreature>>,
    /// The Dark Elf's scores.
    pub scores: Sourced<Vec<Score>>,
    /// The bench's results.
    pub bench: Sourced<Bench>,
}

/// A count, as the protocol carries it.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn named(map: &BTreeMap<String, Totals>) -> Vec<Named> {
    map.iter()
        .map(|(name, t)| Named {
            name: name.clone(),
            spent: t.into(),
        })
        .collect()
}

/// The usage ledgers of `worlds` folded over `window` at `now`.
fn usage(worlds: &[(BoardWorld, PathBuf)], window: Window, now: Timestamp) -> Sourced<Usage> {
    let mut rows = Vec::new();
    let mut of = Vec::new();
    let mut why = Vec::new();
    // Trusted only while every ledger read is a witness whose chain holds.
    let mut trusted = true;
    let mut witnessed = false;
    for (w, path) in worlds {
        match spend::read(path) {
            Ok(l) => {
                trusted &= l.trusted;
                witnessed |= l.trusted;
                if let Some(y) = &l.why {
                    why.push(format!("{}: {y}", w.name));
                }
                if l.cut {
                    why.push(format!(
                        "{}: read from the ledger's last {} MiB; older calls are not counted",
                        w.name,
                        spend::READ_CAP >> 20
                    ));
                }
                if l.skipped > 0 {
                    why.push(format!(
                        "{}: {} line(s) of the ledger do not read and are not counted",
                        w.name, l.skipped
                    ));
                }
                of.extend(std::iter::repeat_n(w.id.clone(), l.rows.len()));
                rows.extend(l.rows);
            }
            Err(e) => {
                trusted = false;
                why.push(format!("{}: the ledger cannot be read: {e}", w.name));
            }
        }
    }
    let r = spend::fold(&rows, window, now);
    // Each session's world: the world whose ledger booked it.
    let mut home: BTreeMap<&str, &str> = BTreeMap::new();
    for (row, world) in rows.iter().zip(&of) {
        home.entry(row.session.as_str()).or_insert(world.as_str());
    }
    let value = Usage {
        total: (&r.total).into(),
        days: r
            .days
            .iter()
            .map(|(day, t)| DayRaces {
                day: day.clone(),
                spent: t.into(),
                races: r.day_races.get(day).map(named).unwrap_or_default(),
            })
            .collect(),
        races: named(&r.races),
        models: named(&r.models),
        bodies: named(&r.bodies),
        sessions: r
            .sessions
            .iter()
            .map(|s| SessionSpent {
                session: s.id.clone(),
                world: home
                    .get(s.id.as_str())
                    .map(|w| (*w).to_owned())
                    .unwrap_or_default(),
                first: s.first.to_string(),
                last: s.last.to_string(),
                spent: (&s.totals).into(),
            })
            .collect(),
        burn: r.burn.clone(),
        burn_slot_secs: u32::try_from(BURN_SLOT).unwrap_or(u32::MAX),
        burn_per_minute: r.burn_per_minute(),
    };
    let source = if witnessed {
        "the usage witness under the state directory (as `mnemorphd usage` folds it)".to_owned()
    } else {
        format!("{USAGE} (as `mnemorphd usage` folds it)")
    };
    let out = Sourced::known(value, source, trusted && !worlds.is_empty());
    if why.is_empty() {
        out
    } else {
        out.with(why.join("; "))
    }
}

/// Every living creature's meters in `worlds`, each with its first call in `first`.
fn meters(
    worlds: &[(BoardWorld, PathBuf)],
    first: &BTreeMap<(String, String), Timestamp>,
) -> Sourced<Vec<MetricsCreature>> {
    let mut all = Vec::new();
    let mut why = Vec::new();
    for (w, path) in worlds {
        let snap = snapshot::take(path);
        let read = super::overview::meters_of(path, snap.as_ref().map_err(String::as_str));
        match read.value {
            Some(creatures) => all.extend(creatures.into_iter().map(|c| {
                MetricsCreature {
                    world: w.id.clone(),
                    first_call: first
                        .get(&(w.id.clone(), c.creature.clone()))
                        .map(ToString::to_string),
                    creature: c.creature,
                    race: c.race,
                    meters: c.meters,
                }
            })),
            None => why.push(format!(
                "{}: {}",
                w.name,
                read.why
                    .unwrap_or_else(|| "the roster does not read".to_owned())
            )),
        }
    }
    let mut note = vec![super::overview::METERS_NOTE.to_owned()];
    note.extend(why);
    Sourced::known(all, super::overview::METERS_SOURCE, false).with(note.join("; "))
}

/// One speciality's score.
fn score(world: &str, speciality: &str, f: &SpecialityFold, form: Option<Race>) -> Score {
    Score {
        world: world.to_owned(),
        speciality: speciality.to_owned(),
        form: form.map(|r| r.name().to_owned()),
        findings: f.findings,
        upheld: f.upheld,
        refuted: f.refuted,
        cannot_verify: f.cannot_verify,
        held: f.held,
        unverified: f.unverified,
        precision: meter("precision", Some(f.precision_rate()), None),
        upheld_rate: meter("upheldRate", Some(Rate::of(f.upheld, f.findings)), None),
        recent: meter("recentPrecision", Some(f.recent_rate()), None),
        cannot_verify_rate: meter("cannotVerifyRate", Some(f.cannot_verify_counts()), None),
        proposal: form.and_then(|r| f.proposal(r)).map(|a| {
            match a {
                Ascension::Rise => "rise",
                Ascension::Probation => "probation",
            }
            .to_owned()
        }),
    }
}

/// The Dark Elf's scores in `worlds`, from the challenge records under the state directory.
fn scores(worlds: &[(BoardWorld, PathBuf)]) -> Sourced<Vec<Score>> {
    const SOURCE: &str = "the challenge records under the state directory (as `mnemorphd challenge score` folds them)";
    let mut all = Vec::new();
    let mut why = Vec::new();
    for (w, path) in worlds {
        match challenge::scored(path) {
            Ok((folded, chain)) => {
                if let Some(c) = chain.as_ref().filter(|c| !c.holds()) {
                    why.push(format!("{}: the records' chain does not hold: {c}", w.name));
                }
                let snap = snapshot::take(path);
                let roster = snap
                    .as_ref()
                    .map(|s| mnemorph_loop::world::Roster::load(s.path()))
                    .unwrap_or_default();
                for (speciality, f) in &folded {
                    let form = roster
                        .living()
                        .find(|c| &c.name == speciality)
                        .map(|c| c.race);
                    all.push(score(&w.id, speciality, f, form));
                }
            }
            Err(e) => why.push(format!("{}: {e}", w.name)),
        }
    }
    let out = Sourced::known(all, SOURCE, true);
    if why.is_empty() {
        out
    } else {
        out.with(why.join("; "))
    }
}

/// The most result files the bench walk opens.
const BENCH_FILES: usize = 512;
/// The most bytes one result file may hold.
const BENCH_FILE_CAP: u64 = 8 << 20;

/// Whether a directory under the bench's results is skipped by name: a held-out suite — never
/// read, by its README's rule — the traces, and anything hidden.
fn skipped(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("heldout")
        || lower.starts_with("held-out")
        || lower == "traces"
        || lower == "suites"
        || name.starts_with('.')
}

/// The directories under `dir`, sorted, links and skipped names left out.
fn dirs(dir: &Path, budget: &mut usize) -> Vec<(String, PathBuf)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = read
        .filter_map(Result::ok)
        .take(*budget)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            (!skipped(&name)).then(|| (name, e.path()))
        })
        .collect();
    *budget = budget.saturating_sub(out.len());
    out.sort();
    out
}

/// A bench group: suite, harness, model and mode.
type Group = (String, String, String, String);

/// A task's last row: passed, valid, and its dollars.
type Task = (bool, bool, Option<f64>);

/// The result files under `dir` (`<suite>/<harness>/<model>.jsonl`), read: the rows, how many
/// files or lines did not read, and the files opened — never one under a held-out suite.
#[must_use]
pub fn bench_at(dir: &Path) -> (Vec<BenchRow>, u32, Vec<PathBuf>) {
    use serde_json::Value;
    let mut budget = BENCH_FILES * 4;
    let mut opened = Vec::new();
    let mut unreadable = 0u32;
    let mut groups: BTreeMap<Group, (BTreeMap<String, Task>, u32)> = BTreeMap::new();
    let mut dates: BTreeMap<Group, Timestamp> = BTreeMap::new();
    for (suite, at) in dirs(dir, &mut budget) {
        for (harness, at) in dirs(&at, &mut budget) {
            let Ok(read) = std::fs::read_dir(&at) else {
                continue;
            };
            let mut files: Vec<PathBuf> = read
                .filter_map(Result::ok)
                .take(BENCH_FILES)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .collect();
            files.sort();
            for file in files {
                if opened.len() >= BENCH_FILES {
                    break;
                }
                opened.push(file.clone());
                let bytes = match regular::read(&file, BENCH_FILE_CAP) {
                    Ok(Some(b)) => b,
                    Ok(None) => continue,
                    Err(_) => {
                        unreadable += 1;
                        continue;
                    }
                };
                let written = std::fs::symlink_metadata(&file)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| Timestamp(i64::try_from(d.as_secs()).unwrap_or(i64::MAX)));
                for line in bytes.split(|&b| b == b'\n') {
                    if line.iter().all(u8::is_ascii_whitespace) {
                        continue;
                    }
                    let Ok(Value::Object(row)) = serde_json::from_slice::<Value>(line) else {
                        unreadable += 1;
                        continue;
                    };
                    let text = |k: &str| row.get(k).and_then(Value::as_str).map(str::to_owned);
                    let (Some(task), Some(model)) = (text("task"), text("model")) else {
                        unreadable += 1;
                        continue;
                    };
                    let mode = text("mode").unwrap_or_else(|| "default".to_owned());
                    let key = (suite.clone(), harness.clone(), model, mode);
                    let valid = row.get("valid").and_then(Value::as_bool) != Some(false);
                    let passed = row.get("passed").and_then(Value::as_bool) == Some(true);
                    let usd = row
                        .get("usd")
                        .and_then(Value::as_f64)
                        .filter(|u| u.is_finite());
                    if let Some(t) = written {
                        dates
                            .entry(key.clone())
                            .and_modify(|d| *d = (*d).max(t))
                            .or_insert(t);
                    }
                    let g = groups.entry(key).or_default();
                    if valid {
                        g.0.insert(task, (passed, valid, usd));
                    } else {
                        g.1 += 1;
                    }
                }
            }
        }
    }
    let rows = groups
        .into_iter()
        .map(|(key, (tasks, invalid))| {
            let priced: Vec<f64> = tasks.values().filter_map(|t| t.2).collect();
            BenchRow {
                date: dates.get(&key).map(ToString::to_string),
                suite: key.0,
                harness: key.1,
                model: key.2,
                mode: key.3,
                valid: count(tasks.len()),
                passed: count(tasks.values().filter(|t| t.0 && t.1).count()),
                invalid,
                usd: (!priced.is_empty()).then(|| priced.iter().sum()),
                unpriced: count(tasks.len() - priced.len()),
            }
        })
        .collect();
    (rows, unreadable, opened)
}

/// The bench's results, from the directory `board.bench_results` names in `config`.
fn bench(dir: Option<PathBuf>, config_error: Option<String>) -> Sourced<Bench> {
    const SOURCE: &str = "board.bench_results in your global file";
    if let Some(e) = config_error {
        return Sourced::unknown(SOURCE, true, e);
    }
    let Some(dir) = dir else {
        return Sourced::known(
            Bench {
                configured: false,
                rows: Vec::new(),
                unreadable: 0,
            },
            SOURCE,
            true,
        )
        .with("no bench results configured");
    };
    if !dir.is_dir() {
        return Sourced::unknown(
            format!("{}", dir.display()),
            true,
            "board.bench_results names no directory",
        );
    }
    let (rows, unreadable, _) = bench_at(&dir);
    Sourced::known(
        Bench {
            configured: true,
            rows,
            unreadable,
        },
        format!("{} (named by board.bench_results)", dir.display()),
        true,
    )
    .with("a held-out suite's results are never read")
}

/// The metrics of `worlds` (the listed ones asked for) over `window` at `now`, `world` naming
/// what was asked; the bench from `bench_dir`, or why the global file did not say.
#[must_use]
pub fn metrics(
    world: &str,
    worlds: &[BoardWorld],
    window: Window,
    now: Timestamp,
    bench_dir: Result<Option<PathBuf>, String>,
) -> BoardMetrics {
    let present: Vec<(BoardWorld, PathBuf)> = worlds
        .iter()
        .filter(|w| w.state == WorldState::Present)
        .map(|w| (w.clone(), PathBuf::from(&w.path)))
        .collect();
    let usage_read = usage(&present, window, now);
    // A silent meter is silent since its creature's first call, whatever the window.
    let mut first: BTreeMap<(String, String), Timestamp> = BTreeMap::new();
    for (w, path) in &present {
        if let Ok(l) = spend::read(path) {
            for (body, at) in spend::fold(&l.rows, Window::All, now).first_call {
                first.insert((w.id.clone(), body), at);
            }
        }
    }
    let (dir, error) = match bench_dir {
        Ok(d) => (d, None),
        Err(e) => (None, Some(e)),
    };
    BoardMetrics {
        world: world.to_owned(),
        window: window.word().to_owned(),
        worlds: present.iter().map(|(w, _)| w.id.clone()).collect(),
        usage: usage_read,
        meters: meters(&present, &first),
        scores: scores(&present),
        bench: bench(dir, error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(at: &Path, text: &str) {
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }

    #[test]
    fn the_bench_reads_its_rows_and_never_a_held_out_suite() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(
            &d.join("large/mnemorph/m.jsonl"),
            concat!(
                r#"{"task":"t1","mode":"solo","model":"m","passed":false,"valid":true,"usd":0.5}"#,
                "\n",
                // A rerun stands in for the first row.
                r#"{"task":"t1","mode":"solo","model":"m","passed":true,"valid":true,"usd":0.25}"#,
                "\n",
                r#"{"task":"t2","mode":"solo","model":"m","passed":false,"valid":true,"usd":null}"#,
                "\n",
                r#"{"task":"t3","mode":"solo","model":"m","passed":false,"valid":false}"#,
                "\n",
                "not json\n",
            ),
        );
        write(
            &d.join("heldout-campaign/mnemorph/m.jsonl"),
            r#"{"task":"HELDOUT","mode":"solo","model":"m","passed":true}"#,
        );
        write(
            &d.join("traces/mnemorph/x.jsonl"),
            r#"{"task":"TRACE","model":"m"}"#,
        );
        let (rows, unreadable, opened) = bench_at(d);
        assert!(
            opened.iter().all(|p| {
                let s = p.to_string_lossy();
                !s.contains("heldout") && !s.contains("traces")
            }),
            "{opened:?}"
        );
        assert_eq!(opened.len(), 1);
        assert_eq!(unreadable, 1);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(
            (r.suite.as_str(), r.harness.as_str(), r.mode.as_str()),
            ("large", "mnemorph", "solo")
        );
        assert_eq!((r.valid, r.passed, r.invalid), (2, 1, 1));
        assert_eq!(r.usd, Some(0.25));
        assert_eq!(r.unpriced, 1);
        assert!(r.date.is_some());
    }

    #[test]
    fn no_bench_path_is_none_configured() {
        let b = bench(None, None);
        let v = b.value.unwrap();
        assert!(!v.configured && v.rows.is_empty());
        assert_eq!(b.why.as_deref(), Some("no bench results configured"));
        let missing = bench(Some(PathBuf::from("/nonexistent/bench")), None);
        assert!(missing.value.is_none());
    }

    #[test]
    fn unpriced_spend_reads_no_dollars() {
        let t = Totals {
            calls: 2,
            unpriced: 2,
            ..Totals::default()
        };
        assert_eq!(Spent::from(&t).usd, None);
        let some = Totals {
            calls: 3,
            unpriced: 2,
            usd: 0.0,
            ..Totals::default()
        };
        assert_eq!(
            Spent::from(&some).usd,
            Some(0.0),
            "a priced free call is $0"
        );
    }
}
