//! A world's overview, as `GET /api/w/<id>` shows it: its status, its last activity, its memory
//! standing, its open challenges, its last gate verdict, its meters and its latest spend — each
//! field with its source, and `null` where the source cannot be read, never a guess.
//!
//! The reads are the CLI's own, in-process: `memory status`, `challenge list`, `meters`, the gate's
//! verdict journal, the usage ledger and the run journals. What lives under the human's state
//! directory and is chained by the harness (the challenge records, the gate's verdicts) is marked
//! `trusted`; what is read from inside the world (its memory, its run journals, `log.md`, the usage
//! ledger, its roster) is shown and marked untrusted — a body can write the world, so none of it
//! grants anything.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use std::io::{Read as _, Seek as _, SeekFrom};

use mnemorph_loop::world::challenge::{Challenges, Staff, Status as ChallengeStatus};
use mnemorph_loop::world::ledger::{Timestamp, UsageRow, parse_rows};
use mnemorph_loop::world::memory::audit::SHARED;
use mnemorph_loop::world::regular;
use mnemorph_loop::world::{log::LOG, meters, verdicts, worth::USAGE};
use serde::Serialize;
use ts_rs::TS;

use super::BoardWorld;
use super::snapshot::{self, FILE_CAP, Snapshot};

/// A reading and where it came from. `value` is `null` when the source cannot be read, and `why`
/// then says so: unknown is said, never guessed.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Sourced<T> {
    /// The reading; `null` when unknown.
    pub value: Option<T>,
    /// Where it was read: a path, a journal, a command's fold.
    pub source: String,
    /// Whether the source is the harness's own — under the human's state directory, chained —
    /// rather than read from inside the world, which a body can write.
    pub trusted: bool,
    /// Why it is unknown, or what qualifies it.
    pub why: Option<String>,
}

impl<T> Sourced<T> {
    pub(super) fn known(value: T, source: impl Into<String>, trusted: bool) -> Self {
        Self {
            value: Some(value),
            source: source.into(),
            trusted,
            why: None,
        }
    }

    pub(super) fn unknown(
        source: impl Into<String>,
        trusted: bool,
        why: impl Into<String>,
    ) -> Self {
        Self {
            value: None,
            source: source.into(),
            trusted,
            why: Some(why.into()),
        }
    }

    pub(super) fn with(mut self, why: impl Into<String>) -> Self {
        self.why = Some(why.into());
        self
    }
}

/// Where a world stands now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum WorldStatus {
    /// A chat the board runs is open in it.
    Live,
    /// A session outside the board — a terminal's — holds a run in it now.
    TuiLive,
    /// No session holds a run in it.
    Idle,
    /// Its directory is gone.
    Missing,
}

/// The latest thing that happened in a world, and what it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Activity {
    /// When (RFC 3339, UTC).
    pub at: String,
    /// What: `a session opened`, `log.md written`, …
    pub what: String,
}

/// A world's memory, counted as `mnemorphd memory status` counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Memory {
    /// Notes grounded in shared memory: the only ones other bodies recall.
    pub shared: u32,
    /// Personal notes grounded.
    pub grounded: u32,
    /// Personal notes proposed, not yet proved.
    pub proposed: u32,
    /// Notes the gate retired, kept dormant for the human to review.
    pub retired: u32,
    /// Note events whose author or proof does not resolve: the creature's writes stay frozen.
    pub unproven: u32,
}

/// The Dark Elf challenges that hold a landing, as `mnemorphd challenge list` lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ChallengeCounts {
    /// Landings whose challenge waits on a review not yet filed.
    pub pending: u32,
    /// Landings a finding blocks until it is fixed or the human waives it.
    pub blocked: u32,
    /// Landings whose findings wait on their verifier's verdict.
    pub verifying: u32,
    /// Open challenges with a speciality no living body bears.
    pub unstaffed: u32,
    /// Findings of medium or higher on the open challenges.
    pub findings: u32,
    /// Lines of the records that do not read: until they do, what they held is unknown.
    pub unreadable: u32,
}

/// The end gate's verdicts on a world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Gate {
    /// How many it gave.
    pub verdicts: u32,
    /// The last; `null` when it gave none yet.
    pub last: Option<GateVerdict>,
}

/// One verdict of the end gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct GateVerdict {
    /// When (RFC 3339, UTC).
    pub at: String,
    /// The body whose landing it judged.
    pub body: String,
    /// The verdict as `log.md` heads it: `PASS`, `FAIL, handed back`, …
    pub verdict: String,
    /// How many paths the landing changed.
    pub changed: u32,
}

/// One meter, as `mnemorphd meters --json` shows it: no report is no count, never a zero.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Meter {
    /// The meter, as the race law names it.
    pub meter: String,
    /// What succeeded; `null` with no report.
    pub successes: Option<u32>,
    /// Of how many; `null` with no report.
    pub n: Option<u32>,
    /// The point estimate.
    pub value: Option<f64>,
    /// The 95% Wilson interval's low end.
    pub lo: Option<f64>,
    /// Its high end.
    pub hi: Option<f64>,
}

/// A living creature's meters.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct CreatureMeters {
    /// The creature.
    pub creature: String,
    /// Its race, by name.
    pub race: String,
    /// Its meters, in the race's order.
    pub meters: Vec<Meter>,
}

/// What the latest session in the usage ledger spent.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Spend {
    /// The session.
    pub session: String,
    /// Its calls.
    pub calls: u32,
    /// US dollars over its priced calls; `null` when none was priced — never a made-up zero.
    pub usd: Option<f64>,
    /// Its calls on unpriced models: `usd` does not cover them.
    pub unpriced: u32,
    /// Its weighted tokens.
    #[ts(type = "number")]
    pub wt: u64,
}

/// `GET /api/w/<id>`: one world's overview.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BoardOverview {
    /// The world, as `GET /api/worlds` lists it.
    pub world: BoardWorld,
    /// Live, TUI-live, idle or missing.
    pub status: Sourced<WorldStatus>,
    /// The latest thing that happened in it.
    pub activity: Sourced<Activity>,
    /// Its memory standing.
    pub memory: Sourced<Memory>,
    /// Its open challenges and their findings.
    pub challenges: Sourced<ChallengeCounts>,
    /// The end gate's verdicts.
    pub gate: Sourced<Gate>,
    /// Every living creature's meters.
    pub meters: Sourced<Vec<CreatureMeters>>,
    /// The latest session's spend.
    pub spend: Sourced<Spend>,
    /// Gate asks waiting for the human in the board's chats.
    pub waiting: Sourced<u32>,
    /// How its changes reach the page: `pushed`, or `slowed` when the world holds more entries
    /// than the watcher's budget and is looked at less often.
    pub updates: Sourced<String>,
}

/// The board's chats in one world: how many, and the gate asks they keep waiting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Chats {
    /// The chats.
    pub open: u32,
    /// Their asks waiting for the human.
    pub asks: u32,
}

/// A count, as the protocol carries it.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// When `path` was last written, if it is there.
fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::symlink_metadata(path)
        .and_then(|m| m.modified())
        .ok()
}

/// `at` as the ledgers write a time.
fn stamp(at: SystemTime) -> Timestamp {
    let secs = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    Timestamp(secs)
}

/// The world's directory of run journals, as the CLI's `runs` reads it.
fn runs_dir(world: &Path) -> PathBuf {
    mnemorph_loop::resume::journals(world)
}

/// The newest run journal's write, among the session runs' and the bodies'.
fn newest_journal(world: &Path) -> Option<SystemTime> {
    std::fs::read_dir(runs_dir(world))
        .ok()?
        .take(snapshot::ENTRIES)
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

/// The status: missing, live in a board chat, held by a terminal session, or idle.
fn status(path: &Path, chats: Chats) -> Sourced<WorldStatus> {
    if !path.is_dir() {
        return Sourced::known(WorldStatus::Missing, "the world's directory", true)
            .with("the directory is gone; the world stays listed until it is forgotten");
    }
    if chats.open > 0 {
        return Sourced::known(WorldStatus::Live, "the board's chats", true)
            .with(format!("{} chat(s) open on the board", chats.open));
    }
    let held = mnemorph_loop::resume::live(path);
    let source = ".mnemorph/instruments/loop/<run>.lock";
    if let Some(run) = held.first() {
        let more = held.len() - 1;
        let why = if more == 0 {
            format!("run {run} is held by a live session")
        } else {
            format!("run {run} and {more} more are held by live sessions")
        };
        return Sourced::known(WorldStatus::TuiLive, source, false).with(why);
    }
    Sourced::known(WorldStatus::Idle, source, false).with("no live session holds a run")
}

/// The latest of: a session opening (the registry), the gate's last verdict, `log.md`, a run
/// journal and the usage ledger written.
fn activity(world: &BoardWorld, path: &Path, gate: Option<&GateVerdict>) -> Sourced<Activity> {
    let mut seen: Vec<(Timestamp, &str, &str, bool)> = Vec::new();
    if let Some(t) = world.last.as_deref().and_then(Timestamp::parse) {
        seen.push((t, "a session opened", "the worlds registry", true));
    }
    if let Some(t) = gate.and_then(|g| Timestamp::parse(&g.at)) {
        seen.push((
            t,
            "the end gate gave a verdict",
            "the gate's verdict journal",
            true,
        ));
    }
    if let Some(t) = modified(&path.join(LOG)) {
        seen.push((stamp(t), "log.md written", LOG, false));
    }
    if let Some(t) = newest_journal(path) {
        seen.push((
            stamp(t),
            "a run journal written",
            ".mnemorph/instruments/loop/",
            false,
        ));
    }
    if let Some(t) = modified(&path.join(USAGE)) {
        seen.push((stamp(t), "a call booked", USAGE, false));
    }
    // The latest; on a tie, the trusted source first.
    seen.sort_by(|a, b| b.0.cmp(&a.0).then(b.3.cmp(&a.3)));
    match seen.first() {
        Some(&(at, what, source, trusted)) => Sourced::known(
            Activity {
                at: at.to_string(),
                what: what.to_owned(),
            },
            source,
            trusted,
        ),
        None => Sourced::unknown(
            "the registry, the gate, log.md, the run journals, the usage ledger",
            false,
            "nothing has happened here that the board can read",
        ),
    }
}

/// The memory standing, counted as `mnemorphd memory status` counts it.
fn memory(snap: Result<&Snapshot, &str>) -> Sourced<Memory> {
    use mnemorph_loop::world::memory as m;
    const SOURCE: &str = ".mnemorph/memory/ (as `mnemorphd memory status` reads it)";
    let path = match snap {
        Ok(s) => s.path(),
        Err(why) => return Sourced::unknown(SOURCE, false, why),
    };
    let report = match m::status(path) {
        Ok(r) => r,
        Err(e) => return Sourced::unknown(SOURCE, false, format!("memory cannot be read: {e}")),
    };
    let s = m::standing(path);
    let unproven: usize = report.iter().map(|(_, found)| found.len()).sum();
    let mut names: Vec<String> = report.into_iter().map(|(n, _)| n).collect();
    for n in m::pathology::traced(path) {
        if !names.contains(&n) {
            names.push(n);
        }
    }
    let retired: usize = names
        .iter()
        .filter(|n| *n != SHARED)
        .map(|n| m::retired(path, n).map(|r| r.len()).unwrap_or_default())
        .sum();
    Sourced::known(
        Memory {
            shared: count(s.shared),
            grounded: count(s.grounded),
            proposed: count(s.proposed),
            retired: count(retired),
            unproven: count(unproven),
        },
        SOURCE,
        false,
    )
}

/// The open challenges, from the records under the human's state directory.
fn challenges(path: &Path) -> Sourced<ChallengeCounts> {
    const SOURCE: &str = "the challenge records under the state directory";
    let Some(records) = Challenges::of(path) else {
        return Sourced::unknown(
            SOURCE,
            true,
            "no state directory: the challenges cannot be read",
        );
    };
    let mut c = ChallengeCounts {
        pending: 0,
        blocked: 0,
        verifying: 0,
        unstaffed: 0,
        findings: 0,
        unreadable: count(records.unreadable().len()),
    };
    for r in records.records().values().filter(|r| r.status.open()) {
        match r.status {
            ChallengeStatus::Pending { .. } => c.pending += 1,
            ChallengeStatus::Blocked { .. } => c.blocked += 1,
            ChallengeStatus::Verifying { .. } => c.verifying += 1,
            ChallengeStatus::Cleared | ChallengeStatus::Waived { .. } => {}
        }
        if r.staff.iter().any(|(_, s)| matches!(s, Staff::Unstaffed)) {
            c.unstaffed += 1;
        }
        c.findings += count(r.findings.len());
    }
    Sourced::known(c, SOURCE, true)
}

/// The end gate's verdicts, from its chained journal under the human's state directory.
fn gate(path: &Path) -> Sourced<Gate> {
    const SOURCE: &str = "the gate's verdict journal under the state directory";
    if verdicts::verdicts_of(path).is_none() {
        return Sourced::unknown(
            SOURCE,
            true,
            "no state directory: the verdicts cannot be read",
        );
    }
    match verdicts::read(path) {
        Ok(rows) => Sourced::known(
            Gate {
                verdicts: count(rows.len()),
                last: rows.last().map(|r| GateVerdict {
                    at: r.ts.to_string(),
                    body: r.body.clone(),
                    verdict: r.verdict.clone(),
                    changed: count(r.changed.len()),
                }),
            },
            SOURCE,
            true,
        ),
        Err(e) => Sourced::unknown(SOURCE, true, format!("the verdicts cannot be read: {e}")),
    }
}

/// One meter, as `mnemorphd meters --json` shows it: no report — no fold, or a fold of none — is no
/// count, never a zero.
pub(super) fn meter(name: &str, rate: Option<meters::Rate>) -> Meter {
    let rate = rate.filter(|x| x.n > 0);
    let (lo, hi) = rate.and_then(|x| x.interval()).unzip();
    Meter {
        meter: name.to_owned(),
        successes: rate.map(|x| x.successes),
        n: rate.map(|x| x.n),
        value: rate.and_then(|x| x.value()),
        lo,
        hi,
    }
}

/// Every living creature's meters, as `mnemorphd meters --json` shows them.
pub(super) fn meters_of(
    path: &Path,
    snap: Result<&Snapshot, &str>,
) -> Sourced<Vec<CreatureMeters>> {
    const SOURCE: &str = "the roster's creatures (in the world) over the challenge records' folds";
    let copy = match snap {
        Ok(s) => s.path(),
        Err(why) => return Sourced::unknown(SOURCE, false, why),
    };
    // The roster from the copy; the folds from the records kept for the world's own path.
    let folds = mnemorph_loop::world::challenge::scored(path)
        .map(|(f, _)| f)
        .unwrap_or_default();
    let roster = mnemorph_loop::world::Roster::load(copy);
    let all = meters::of(&roster, &folds)
        .into_iter()
        .map(|m| CreatureMeters {
            creature: m.name,
            race: m.race.name().to_owned(),
            meters: m.readings.iter().map(|r| meter(r.meter, r.rate)).collect(),
        })
        .collect();
    Sourced::known(all, SOURCE, false)
        .with("only the Dark Elf's meters fold yet; shown with their count, never judged on")
}

/// The latest session's spend, from the usage ledger in the world.
fn spend(path: &Path) -> Sourced<Spend> {
    if !path.is_dir() {
        return Sourced::unknown(USAGE, false, "the directory is gone");
    }
    let (back, cut) = match tail(&path.join(USAGE), FILE_CAP) {
        Ok(Some(b)) => b,
        Ok(None) => return Sourced::unknown(USAGE, false, "no call is booked yet"),
        Err(e) => {
            return Sourced::unknown(USAGE, false, format!("the ledger cannot be read: {e}"));
        }
    };
    let Some(latest) = back.rows.last().map(|r| r.session.clone()) else {
        return Sourced::unknown(USAGE, false, "no call is booked yet");
    };
    let mut s = Spend {
        session: latest,
        calls: 0,
        usd: None,
        unpriced: 0,
        wt: 0,
    };
    for r in back.rows.iter().filter(|r| r.session == s.session) {
        s.calls += 1;
        s.wt += r.wt;
        match r.usd {
            Some(usd) => s.usd = Some(s.usd.unwrap_or_default() + usd),
            None => s.unpriced += 1,
        }
    }
    let mut why = Vec::new();
    if cut {
        why.push(format!(
            "read from the ledger's last {} MiB: an older part of this session is not counted",
            FILE_CAP >> 20
        ));
    }
    if !back.skipped.is_empty() {
        why.push(format!(
            "{} line(s) of the ledger do not read and are not counted",
            back.skipped.len()
        ));
    }
    let out = Sourced::known(s, USAGE, false);
    if why.is_empty() {
        out
    } else {
        out.with(why.join("; "))
    }
}

/// The rows of the usage ledger's last `cap` bytes, and whether older bytes were left unread: a
/// regular file only, opened as [`regular::open`] opens one. `None` when it is absent.
fn tail(
    path: &Path,
    cap: u64,
) -> Result<Option<(mnemorph_loop::world::ledger::ReadBack<UsageRow>, bool)>, String> {
    let Some(mut file) = regular::open(path).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    let from = len.saturating_sub(cap);
    file.seek(SeekFrom::Start(from))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(cap)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    // Cut mid-line: the first, partial line is not a row.
    let start = if from > 0 {
        bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |i| i + 1)
    } else {
        0
    };
    let back = parse_rows(&bytes[start..]).map_err(|e| e.to_string())?;
    Ok(Some((back, from > 0)))
}

/// `world`'s overview, with `chats` the board's own in it; `slowed` when the watcher found it past
/// its budget.
#[must_use]
pub fn overview(world: BoardWorld, chats: Chats, slowed: bool) -> BoardOverview {
    let path = PathBuf::from(&world.path);
    let snap = if path.is_dir() {
        snapshot::take(&path)
    } else {
        Err("the directory is gone".to_owned())
    };
    let snap = snap.as_ref().map_err(String::as_str);
    let gate = gate(&path);
    let last = gate.value.as_ref().and_then(|g| g.last.as_ref());
    BoardOverview {
        status: status(&path, chats),
        activity: activity(&world, &path, last),
        memory: memory(snap),
        challenges: challenges(&path),
        meters: meters_of(&path, snap),
        spend: spend(&path),
        waiting: Sourced::known(chats.asks, "the board's chats", true)
            .with("a terminal session's asks wait in its terminal"),
        updates: if slowed {
            Sourced::known("slowed".to_owned(), "the board's watcher", true).with(format!(
                "too many entries: updates slowed (over {} under the watched files; looked at \
                 every {} s)",
                super::watch::BUDGET,
                super::watch::SLOWED.as_secs()
            ))
        } else {
            Sourced::known("pushed".to_owned(), "the board's watcher", true)
        },
        gate,
        world,
    }
}
