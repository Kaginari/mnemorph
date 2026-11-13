//! The runs' health, for the metrics page: each session run of the window folded from its journal
//! as `mnemorphd runs --metrics` folds it — its turns, the time in the model and in tools, how much
//! it read again, the results cut short, the lines it wrote, whether a test ran after its last
//! code write — with its last gate verdict and how it ended, and the window's verdicts and
//! endings counted.
//!
//! The journals live inside the world, which a body can write, so the reading is not trusted,
//! and every read is bounded as the run listing's is: a session run's journal only, opened
//! through the world's shared reader, each cut at [`SUMMARY_BYTES`], all under one budget.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

use mnemorph_loop::Beat;
use mnemorph_loop::metrics::metrics;
use mnemorph_loop::resume::is_session_run;
use mnemorph_loop::world::ledger::Timestamp;
use mnemorph_loop::world::spend::Window;
use serde::Serialize;
use ts_rs::TS;

use super::overview::Sourced;
use super::runs::{
    ENTRIES, LISTED, RunState, SUMMARY_BYTES, beats, journals_dir, read_capped, rfc3339, standing,
};

/// The rows the page lists, newest first; every run of the window is counted.
pub const ROWS: usize = 30;

/// The bytes all the journals of one reading may take.
const BUDGET: u64 = 64 << 20;

/// One run, as the page's table shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunHealthRow {
    /// The world, by id.
    pub world: String,
    /// The run's id.
    pub run: String,
    /// When its journal was last written (RFC 3339, UTC).
    pub when: String,
    /// Where it stands.
    pub state: RunState,
    /// How it ended or why it stopped.
    pub why: String,
    /// Model turns.
    pub turns: u32,
    /// Tool calls answered.
    pub calls: u32,
    /// Seconds in the model, over the timed turns.
    pub model_secs: f64,
    /// Seconds in tools, over the timed calls.
    pub tool_secs: f64,
    /// The share of reads that read lines again, 0 to 1; `null` with no read.
    pub reread: Option<f64>,
    /// Tool results cut short.
    pub truncated: u32,
    /// Lines written in.
    pub lines_added: u32,
    /// Lines written out.
    pub lines_removed: u32,
    /// Whether a test ran after the last code write; `null` when it wrote no code.
    pub tested_after_write: Option<bool>,
    /// The last gate verdict: `PASS`, `FAIL`, `NOT_VERIFIED` or `UNCHECKED`; empty with none.
    pub verdict: String,
}

/// A count under a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Tally {
    /// What is counted.
    pub name: String,
    /// How many.
    pub count: u32,
}

/// The window's runs.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunHealth {
    /// The newest runs, at most [`ROWS`].
    pub rows: Vec<RunHealthRow>,
    /// The runs folded.
    pub runs: u32,
    /// Runs past the listing's caps, or whose journal did not read, left out.
    pub left_out: u32,
    /// The last gate verdict of each run, counted; `none` for a run that reached no gate.
    pub verdicts: Vec<Tally>,
    /// How each run ended or why it stopped, counted by its first words.
    pub endings: Vec<Tally>,
    /// Runs that wrote code, and of those, how many ran a test after the last write.
    pub wrote_code: u32,
    /// Of `wrote_code`, how many ran a test after.
    pub tested_after: u32,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn small(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[allow(
    clippy::cast_precision_loss,
    reason = "milliseconds of a run, far below 2^52"
)]
fn secs(ms: u64) -> f64 {
    ms as f64 / 1000.0
}

/// The last gate verdict among `beats`, as the page names it.
fn verdict(beats: &[Beat]) -> String {
    beats
        .iter()
        .rev()
        .find_map(|b| match b {
            Beat::Gate {
                verdict, unchecked, ..
            } => Some(if *unchecked {
                "UNCHECKED".to_owned()
            } else if verdict.is_empty() {
                "PASS".to_owned()
            } else {
                verdict.clone()
            }),
            _ => None,
        })
        .unwrap_or_default()
}

/// An ending's first words: `failed: the provider` counts as `failed`.
fn ending_word(why: &str) -> String {
    let head = why.split(':').next().unwrap_or(why).trim();
    if head.is_empty() {
        "unknown".to_owned()
    } else {
        super::runs::one_line(head, 40)
    }
}

/// A run found: when its journal was written, its world's id, its journals directory, its id.
type Found = (SystemTime, String, PathBuf, String);

/// The session runs of `worlds` written since `since` (seconds), newest first, at most
/// [`LISTED`] a world; and how many past that were left out.
fn listed(worlds: &[(String, PathBuf)], since: Option<i64>) -> (Vec<Found>, usize) {
    let mut found: Vec<Found> = Vec::new();
    let mut left_out = 0usize;
    for (id, path) in worlds {
        let Some(dir) = journals_dir(path) else {
            continue;
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut here: Vec<Found> = entries
            .take(ENTRIES)
            .filter_map(Result::ok)
            .filter_map(|e| {
                let run = e.file_name().to_str()?.strip_suffix(".jsonl")?.to_owned();
                if !is_session_run(&run) {
                    return None;
                }
                let at = std::fs::symlink_metadata(e.path())
                    .ok()?
                    .modified()
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let secs = at
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
                since
                    .is_none_or(|s| secs >= s)
                    .then(|| (at, id.clone(), dir.clone(), run))
            })
            .collect();
        here.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.3.cmp(&a.3)));
        left_out += here.len().saturating_sub(LISTED);
        here.truncate(LISTED);
        found.extend(here);
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.3.cmp(&a.3)));
    (found, left_out)
}

/// The session runs of `worlds` (`(id, path)`) written within `window` of `now`, folded.
#[must_use]
pub fn health(worlds: &[(String, PathBuf)], window: Window, now: Timestamp) -> Sourced<RunHealth> {
    let (found, mut left_out) = listed(worlds, window.since(now).map(|t| t.0));
    let mut budget = BUDGET;
    let mut rows = Vec::new();
    let mut verdicts: BTreeMap<String, u32> = BTreeMap::new();
    let mut endings: BTreeMap<String, u32> = BTreeMap::new();
    let (mut runs, mut wrote_code, mut tested_after) = (0u32, 0u32, 0u32);
    for (at, world, dir, run) in found {
        let bytes = match read_capped(&dir.join(format!("{run}.jsonl")), SUMMARY_BYTES.min(budget))
        {
            Ok(b) if budget > 0 => b,
            _ => {
                left_out += 1;
                continue;
            }
        };
        budget = budget.saturating_sub(bytes.len() as u64);
        let (beats, skipped) = beats(&bytes);
        if !skipped.is_empty() {
            left_out += 1;
            continue;
        }
        runs += 1;
        let m = metrics(&beats);
        let (state, why) = standing(&beats);
        let v = verdict(&beats);
        *verdicts
            .entry(if v.is_empty() {
                "none".into()
            } else {
                v.clone()
            })
            .or_default() += 1;
        *endings.entry(ending_word(&why)).or_default() += 1;
        if let Some(t) = m.tested_after_last_write {
            wrote_code += 1;
            tested_after += u32::from(t);
        }
        if rows.len() < ROWS {
            rows.push(RunHealthRow {
                world,
                run,
                when: rfc3339(at),
                state,
                why: mnemorph_loop::wire::shown(&super::runs::one_line(&why, 120)),
                turns: small(m.turns),
                calls: small(m.calls),
                model_secs: secs(m.model_ms),
                tool_secs: secs(m.tool_ms),
                reread: m.reread_ratio(),
                truncated: small(m.tools.values().map(|t| t.truncated).sum()),
                lines_added: small(m.lines_added),
                lines_removed: small(m.lines_removed),
                tested_after_write: m.tested_after_last_write,
                verdict: v,
            });
        }
    }
    let tally = |m: BTreeMap<String, u32>| {
        let mut v: Vec<Tally> = m
            .into_iter()
            .map(|(name, count)| Tally { name, count })
            .collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
        v
    };
    let value = RunHealth {
        rows,
        runs,
        left_out: count(left_out),
        verdicts: tally(verdicts),
        endings: tally(endings),
        wrote_code,
        tested_after,
    };
    Sourced::known(
        value,
        "the worlds' run journals, .mnemorph/runs (as `mnemorphd runs --metrics` folds them)",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn journal(world: &Path, run: &str, beats: &[Beat]) {
        let dir = mnemorph_loop::resume::journals(world);
        std::fs::create_dir_all(&dir).unwrap();
        let mut text = String::new();
        for b in beats {
            text.push_str(&serde_json::to_string(b).unwrap());
            text.push('\n');
        }
        std::fs::write(dir.join(format!("{run}.jsonl")), text).unwrap();
    }

    fn gate(verdict: &str, unchecked: bool) -> Beat {
        Beat::Gate {
            changed: vec!["a.rs".into()],
            failures: Vec::new(),
            unverified: Vec::new(),
            verdict: verdict.into(),
            unchecked,
            verified_by: Vec::new(),
            open: Vec::new(),
        }
    }

    #[test]
    fn the_windows_runs_are_folded_their_verdicts_and_endings_counted() {
        let w = tempfile::tempdir().unwrap();
        let ask = |t: &str| Beat::Ask { text: t.into() };
        let end = |e: &str| Beat::End { ending: e.into() };
        journal(
            w.path(),
            "r1000-1",
            &[ask("fix it"), gate("PASS", false), end("answered")],
        );
        journal(
            w.path(),
            "r2000-1",
            &[ask("again"), gate("FAIL", false), end("failed: the gate")],
        );
        journal(w.path(), "r3000-1", &[ask("cut"), gate("PASS", true)]);
        // Not a session run: never read.
        journal(w.path(), "notes", &[ask("x")]);
        let worlds = vec![("w1".to_owned(), w.path().to_path_buf())];
        let h = health(&worlds, Window::All, Timestamp(i64::MAX / 2))
            .value
            .unwrap();
        assert_eq!(h.runs, 3);
        assert_eq!(h.rows.len(), 3);
        assert!(h.rows.iter().all(|r| r.world == "w1"));
        let names = |t: &[Tally]| {
            t.iter()
                .map(|t| (t.name.clone(), t.count))
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(
            names(&h.verdicts),
            BTreeMap::from([
                ("FAIL".into(), 1),
                ("PASS".into(), 1),
                ("UNCHECKED".into(), 1)
            ])
        );
        assert_eq!(
            names(&h.endings),
            BTreeMap::from([
                ("answered".into(), 1),
                ("cut mid-turn".into(), 1),
                ("failed".into(), 1)
            ])
        );
        // Written long before the window: nothing is folded.
        let week = health(&worlds, Window::Week, Timestamp(i64::MAX / 2))
            .value
            .unwrap();
        assert_eq!((week.runs, week.rows.len()), (0, 0));
    }
}
