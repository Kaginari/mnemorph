//! A world's timeline, as `GET /api/w/<id>/timeline` shows it: the commissions its bodies ran —
//! each a run journal under `.mnemorph/instruments/loop/`, named `<body>-<hex nanos>-<n>.jsonl` by
//! the commissioner — and the end gate's verdicts, from the harness's chained journal under the
//! human's state directory. The journals are read inside the world, so a commission is shown and
//! marked untrusted; a verdict is the harness's own, marked trusted.
//!
//! Journals open only as regular files (a link, a pipe or a device is refused), each capped, all
//! of them within one budget; the newest are read first.

use std::path::Path;
use std::time::SystemTime;

use mnemorph_loop::Beat;
use mnemorph_loop::resume::is_session_run;
use mnemorph_loop::world::ledger::Timestamp;
use mnemorph_loop::world::verdicts;
use serde::Serialize;
use ts_rs::TS;

use super::runs::{beats, journals_dir, one_line, read_capped};

/// The most body journals one read looks at, the newest first.
pub const JOURNALS: usize = 500;

/// The most bytes one body journal may hold to be read.
pub const JOURNAL_BYTES: u64 = 4 << 20;

/// The most bytes the journals of one read hold in all.
pub const BUDGET_BYTES: u64 = 64 << 20;

/// The most characters of a commission's ask shown.
pub const ASK_CHARS: usize = 240;

/// The most entries one page holds, and the default.
pub const PAGE_MAX: usize = 500;
const PAGE: usize = 50;

/// What an entry of the timeline is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum TimelineKind {
    /// A body ran an order: its run journal.
    Commission,
    /// The end gate judged a landing: its verdict journal.
    Verdict,
}

/// Where a commission's run stands, as its journal reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum CommissionState {
    /// Its last ask has an end.
    Ended,
    /// Its last ask has no end yet.
    Open,
    /// Its journal could not be read: `why` says so.
    Unknown,
}

/// One entry of the timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TimelineEntry {
    /// A commission or a verdict.
    pub kind: TimelineKind,
    /// When (RFC 3339, UTC): a commission's start, read from its journal's name; a verdict's time.
    pub at: String,
    /// The body that ran the order, or whose landing was judged.
    pub body: String,
    /// A commission's run id (its journal's name); `null` for a verdict.
    pub run: Option<String>,
    /// A verdict's number in the gate's journal, from 1 (what a settle cites); `null` for a
    /// commission.
    pub n: Option<u32>,
    /// A commission's order, on one line and cut; `null` for a verdict or an unread journal.
    pub ask: Option<String>,
    /// A commission's state.
    pub state: Option<CommissionState>,
    /// How its last turn ended (`answered`, `interrupted`, …), when it did.
    pub ending: Option<String>,
    /// The gate's word: a verdict's, or the last one in a commission's journal.
    pub verdict: Option<String>,
    /// How many paths the judged landing changed.
    pub changed: u32,
    /// The checks that failed, for a verdict.
    pub failures: Vec<String>,
    /// Why a journal was not read, or what qualifies the entry.
    pub why: Option<String>,
    /// Where it was read.
    pub source: String,
    /// Whether that source is the harness's own, under the human's state directory.
    pub trusted: bool,
}

/// `GET /api/w/<id>/timeline`: a page of entries, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardTimeline {
    /// The page.
    pub entries: Vec<TimelineEntry>,
    /// Entries matching the filter.
    #[ts(type = "number")]
    pub total: u64,
    /// Where the page starts.
    #[ts(type = "number")]
    pub offset: u64,
    /// The page's size.
    #[ts(type = "number")]
    pub limit: u64,
    /// Every body named in the timeline, for the filter.
    pub bodies: Vec<String>,
    /// Commissions counted, of every body.
    pub commissions: u32,
    /// Verdicts counted, of every body.
    pub verdicts: u32,
    /// Body journals past [`JOURNALS`], not looked at.
    pub older: u32,
    /// Why the verdicts are unknown, when they are.
    pub verdicts_why: Option<String>,
}

/// The timeline's filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Only this kind.
    pub kind: Option<TimelineKind>,
    /// Only this body.
    pub body: Option<String>,
    /// Where the page starts.
    pub offset: usize,
    /// The page's size.
    pub limit: usize,
}

impl Filter {
    /// The filter a query string asks for.
    ///
    /// # Errors
    ///
    /// A key it does not know, or a value that does not read.
    pub fn parse(query: Option<&str>) -> Result<Self, String> {
        let mut f = Filter {
            limit: PAGE,
            ..Filter::default()
        };
        for (k, v) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
            match &*k {
                "kind" => {
                    f.kind = Some(match &*v {
                        "commission" => TimelineKind::Commission,
                        "verdict" => TimelineKind::Verdict,
                        _ => return Err("`kind` is commission or verdict".into()),
                    });
                }
                "body" if !v.is_empty() && v.len() <= 128 => f.body = Some(v.into_owned()),
                "body" => return Err("`body` names a body: 1 to 128 bytes".into()),
                "offset" => f.offset = v.parse().map_err(|_| "`offset` is a count")?,
                "limit" => {
                    f.limit = v
                        .parse::<usize>()
                        .map_err(|_| "`limit` is a count")?
                        .clamp(1, PAGE_MAX);
                }
                other => return Err(format!("the timeline has no filter `{other}`")),
            }
        }
        Ok(f)
    }
}

/// One commission, as its journal reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commission {
    /// The body.
    pub body: String,
    /// The run id: its journal's name.
    pub run: String,
    /// When it started, from the name.
    pub at: Timestamp,
    /// The order, on one line and cut.
    pub ask: Option<String>,
    /// Ended, open or unknown.
    pub state: CommissionState,
    /// How its last turn ended.
    pub ending: Option<String>,
    /// Its last gate verdict.
    pub verdict: Option<String>,
    /// How many paths that verdict's landing changed.
    pub changed: u32,
    /// Why it was not read.
    pub why: Option<String>,
    /// When its journal was last written.
    pub written: Option<SystemTime>,
}

/// The body and start time a commission's run id names: `<body>-<hex nanos>-<n>`. `None` for a
/// session run's id, or any other name.
#[must_use]
pub fn body_run(run: &str) -> Option<(String, Timestamp)> {
    if is_session_run(run) {
        return None;
    }
    let mut parts = run.rsplitn(3, '-');
    let n = parts.next()?;
    let hex = parts.next()?;
    let body = parts.next()?;
    let plain = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.');
    if n.is_empty()
        || !n.bytes().all(|b| b.is_ascii_digit())
        || hex.is_empty()
        || hex.len() > 32
        || !hex.bytes().all(|b| b.is_ascii_hexdigit())
        || body.is_empty()
        || body.len() > 128
        || !body.bytes().all(plain)
    {
        return None;
    }
    let nanos = u128::from_str_radix(hex, 16).ok()?;
    let secs = i64::try_from(nanos / 1_000_000_000).ok()?;
    Some((body.to_owned(), Timestamp(secs)))
}

/// The word of a gate beat, as the protocol reads it.
fn word(failures: &[String], unverified: &[String], verdict: &str) -> String {
    if !verdict.is_empty() {
        verdict.to_owned()
    } else if !failures.is_empty() {
        "FAIL".into()
    } else if !unverified.is_empty() {
        "NOT_VERIFIED".into()
    } else {
        "PASS".into()
    }
}

/// A commission read from its beats.
fn read_beats(c: &mut Commission, beats: &[Beat]) {
    c.ask = beats.iter().find_map(|b| match b {
        Beat::Ask { text } => Some(one_line(text, ASK_CHARS)),
        _ => None,
    });
    let last_ask = beats.iter().rposition(|b| matches!(b, Beat::Ask { .. }));
    let last_end = beats.iter().rposition(|b| matches!(b, Beat::End { .. }));
    match (last_ask, last_end) {
        (a, Some(e)) if a.is_none_or(|a| a < e) => {
            c.state = CommissionState::Ended;
            if let Beat::End { ending } = &beats[e] {
                c.ending = Some(one_line(ending, ASK_CHARS));
            }
        }
        _ => c.state = CommissionState::Open,
    }
    if let Some(Beat::Gate {
        changed,
        failures,
        unverified,
        verdict,
    }) = beats.iter().rev().find(|b| matches!(b, Beat::Gate { .. }))
    {
        c.verdict = Some(word(failures, unverified, verdict));
        c.changed = u32::try_from(changed.len()).unwrap_or(u32::MAX);
    }
}

/// Every commission of `world` the journals hold, newest first, and how many older journals were
/// left unread past [`JOURNALS`].
#[must_use]
pub fn commissions(world: &Path) -> (Vec<Commission>, u32) {
    let Some(dir) = journals_dir(world) else {
        return (Vec::new(), 0);
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (Vec::new(), 0);
    };
    let mut found: Vec<(Timestamp, String, String)> = entries
        .take(super::runs::ENTRIES)
        .filter_map(Result::ok)
        .filter_map(|e| {
            let run = e.file_name().to_str()?.strip_suffix(".jsonl")?.to_owned();
            let (body, at) = body_run(&run)?;
            Some((at, run, body))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    let older = u32::try_from(found.len().saturating_sub(JOURNALS)).unwrap_or(u32::MAX);
    let mut budget = BUDGET_BYTES;
    let out = found
        .into_iter()
        .take(JOURNALS)
        .map(|(at, run, body)| {
            let path = dir.join(format!("{run}.jsonl"));
            let mut c = Commission {
                body,
                run,
                at,
                ask: None,
                state: CommissionState::Unknown,
                ending: None,
                verdict: None,
                changed: 0,
                why: None,
                written: std::fs::symlink_metadata(&path)
                    .and_then(|m| m.modified())
                    .ok(),
            };
            if budget == 0 {
                c.why = Some(format!(
                    "not read: the journals read hold over {} MiB",
                    BUDGET_BYTES >> 20
                ));
                return c;
            }
            match read_capped(&path, JOURNAL_BYTES.min(budget)) {
                Ok(bytes) => {
                    budget = budget.saturating_sub(bytes.len() as u64);
                    let (beats, skipped) = beats(&bytes);
                    read_beats(&mut c, &beats);
                    if !skipped.is_empty() {
                        c.why = Some(format!("{} line(s) do not read", skipped.len()));
                    }
                }
                Err(e) => c.why = Some(e),
            }
            c
        })
        .collect();
    (out, older)
}

/// The entry a commission is.
fn of_commission(c: &Commission) -> TimelineEntry {
    TimelineEntry {
        kind: TimelineKind::Commission,
        at: c.at.to_string(),
        body: c.body.clone(),
        run: Some(c.run.clone()),
        n: None,
        ask: c.ask.clone(),
        state: Some(c.state),
        ending: c.ending.clone(),
        verdict: c.verdict.clone(),
        changed: c.changed,
        failures: Vec::new(),
        why: c.why.clone(),
        source: format!(".mnemorph/instruments/loop/{}.jsonl", c.run),
        trusted: false,
    }
}

/// The most failures one verdict entry lists.
const FAILURES: usize = 20;

/// Every entry of `world`'s timeline, newest first; on one time, a verdict before a commission.
/// Also the commissions read, how many journals were left, and why the verdicts are unknown.
#[must_use]
pub fn entries(world: &Path) -> (Vec<TimelineEntry>, u32, Option<String>) {
    let (commissions, older) = commissions(world);
    let (out, why) = entries_with(world, &commissions);
    (out, older, why)
}

/// [`entries`], over `commissions` already read; and why the verdicts are unknown.
#[must_use]
pub fn entries_with(
    world: &Path,
    commissions: &[Commission],
) -> (Vec<TimelineEntry>, Option<String>) {
    let mut out: Vec<(i64, TimelineEntry)> = commissions
        .iter()
        .map(|c| (c.at.0, of_commission(c)))
        .collect();
    let mut why = None;
    if verdicts::verdicts_of(world).is_none() {
        why = Some("no state directory: the verdicts cannot be read".to_owned());
    } else {
        match verdicts::read(world) {
            Ok(rows) => {
                for (i, r) in rows.iter().enumerate() {
                    out.push((
                        r.ts.0,
                        TimelineEntry {
                            kind: TimelineKind::Verdict,
                            at: r.ts.to_string(),
                            body: r.body.clone(),
                            run: None,
                            n: Some(u32::try_from(i + 1).unwrap_or(u32::MAX)),
                            ask: None,
                            state: None,
                            ending: None,
                            verdict: Some(r.verdict.clone()),
                            changed: u32::try_from(r.changed.len()).unwrap_or(u32::MAX),
                            failures: r.failures.iter().take(FAILURES).cloned().collect(),
                            why: r.entry.clone(),
                            source: "the gate's verdict journal under the state directory"
                                .to_owned(),
                            trusted: true,
                        },
                    ));
                }
            }
            Err(e) => why = Some(format!("the verdicts cannot be read: {e}")),
        }
    }
    out.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| {
                (b.1.kind == TimelineKind::Verdict).cmp(&(a.1.kind == TimelineKind::Verdict))
            })
            .then_with(|| b.1.n.cmp(&a.1.n))
            .then_with(|| b.1.run.cmp(&a.1.run))
    });
    (out.into_iter().map(|(_, e)| e).collect(), why)
}

/// `world`'s timeline, filtered and paged.
#[must_use]
pub fn timeline(world: &Path, filter: &Filter) -> BoardTimeline {
    let (all, older, verdicts_why) = entries(world);
    let mut bodies: Vec<String> = all.iter().map(|e| e.body.clone()).collect();
    bodies.sort();
    bodies.dedup();
    let commissions = all
        .iter()
        .filter(|e| e.kind == TimelineKind::Commission)
        .count();
    let verdicts = all.len() - commissions;
    let matching: Vec<TimelineEntry> = all
        .into_iter()
        .filter(|e| filter.kind.is_none_or(|k| e.kind == k))
        .filter(|e| filter.body.as_ref().is_none_or(|b| &e.body == b))
        .collect();
    let limit = filter.limit.clamp(1, PAGE_MAX);
    BoardTimeline {
        total: matching.len() as u64,
        entries: matching
            .into_iter()
            .skip(filter.offset)
            .take(limit)
            .collect(),
        offset: filter.offset as u64,
        limit: limit as u64,
        bodies,
        commissions: u32::try_from(commissions).unwrap_or(u32::MAX),
        verdicts: u32::try_from(verdicts).unwrap_or(u32::MAX),
        older,
        verdicts_why,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_run_names_its_body_and_start() {
        let (body, at) = body_run("slime-wire-186f0a2b3c4d5e6f-3").unwrap();
        assert_eq!(body, "slime-wire");
        assert_eq!(
            at.0,
            i64::try_from(0x186f_0a2b_3c4d_5e6f_u64 / 1_000_000_000).unwrap()
        );
        for no in [
            "r1793361000000-41",
            "slime-wire",
            "slime-wire-xyz-1",
            "-ab-1",
            "slime-ab-",
            "../x-ab-1",
            "a b-ab-1",
        ] {
            assert_eq!(body_run(no), None, "{no}");
        }
    }

    #[test]
    fn the_filter_reads_and_refuses() {
        let f = Filter::parse(Some("kind=verdict&body=orc-o&offset=3&limit=9999")).unwrap();
        assert_eq!(f.kind, Some(TimelineKind::Verdict));
        assert_eq!(f.body.as_deref(), Some("orc-o"));
        assert_eq!((f.offset, f.limit), (3, PAGE_MAX));
        assert!(Filter::parse(Some("kind=act")).is_err());
        assert!(Filter::parse(Some("who=x")).is_err());
        assert!(Filter::parse(Some("offset=-1")).is_err());
        assert_eq!(Filter::parse(None).unwrap().limit, PAGE);
    }
}
