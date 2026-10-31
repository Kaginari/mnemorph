//! A world's past chats, read-only, from its run journals: `GET /api/w/<id>/runs` lists the
//! session runs, `GET /api/w/<id>/runs/<run>` gives one's transcript as the protocol's events, so
//! the page draws a past chat as it draws a live one.
//!
//! The journals live inside the world, which a body can write: nothing read here grants anything,
//! and every read is bounded. A journal is opened only by a session run's id (`r<ms>-<pid>`, never
//! a path), only as a regular file through the world's shared reader (`world::regular`: a
//! symlink never followed, a pipe or a device never waited on, the handle checked) — and through a journals directory none of whose parts under the world is a symlink.
//! Each read stops at a size cap; the listing reads the newest runs only, under a total budget.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use mnemorph_loop::Beat;
use mnemorph_loop::resume::{is_session_run, journals};
use mnemorph_loop::world::regular::{self, Unread};
use serde::Serialize;
use serde_json::{Value, json};
use ts_rs::TS;

use crate::protocol::{Event, RESULT_BYTES};

/// The most runs a listing names, newest first.
pub const LISTED: usize = 100;

/// The most directory entries a listing looks at.
pub(crate) const ENTRIES: usize = 5000;

/// The largest journal a listing reads to summarise; a larger one is listed unsummarised.
pub const SUMMARY_BYTES: u64 = 4 << 20;

/// The bytes a listing reads in all.
const LISTING_BYTES: u64 = 64 << 20;

/// The largest journal a transcript reads.
pub const TRANSCRIPT_BYTES: u64 = 32 << 20;

/// The most events a transcript carries: the newest, as the live replay keeps.
pub const TRANSCRIPT_EVENTS: usize = 10_000;

/// The longest text one event of a transcript carries, beside a tool's result.
const TEXT_BYTES: usize = 256 << 10;

/// The longest last ask a listing shows, in characters.
const ASK_CHARS: usize = 120;

/// Where a past run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum RunState {
    /// A live session holds it now.
    Live,
    /// It stopped — a checkpoint, an interrupt, a turn cut short — and may be resumed.
    Stopped,
    /// It answered or failed: there is nothing to resume.
    Ended,
    /// Its journal was not summarised: too large, a line that does not read, or not a plain
    /// file. `why` says which.
    Unknown,
}

/// A session's run, as `GET /api/w/<id>/runs` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardRun {
    /// The run's id: `r<ms>-<pid>`.
    pub id: String,
    /// When its journal was last written (RFC 3339, UTC).
    pub when: String,
    /// Where it stands.
    pub state: RunState,
    /// Why: the stop's words, the ending, or why it was not summarised.
    pub why: String,
    /// The last thing the human asked it, on one line, cut short; empty when unknown.
    pub ask: String,
    /// The human's asks it holds.
    pub turns: u32,
}

/// What `GET /api/w/<id>/runs` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardRuns {
    /// The newest runs, newest first.
    pub runs: Vec<BoardRun>,
    /// Session runs past the listing's cap, not named.
    pub more: u32,
}

/// What `GET /api/w/<id>/runs/<run>` answers: the run's journal as the protocol's events.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BoardTranscript {
    /// The run.
    pub run: String,
    /// Where it stands.
    pub state: RunState,
    /// Each event as a live chat sends it: `{jsonrpc, method, params}`, `seq` from 1.
    pub events: Vec<Value>,
    /// Older events left out past [`TRANSCRIPT_EVENTS`].
    pub dropped: u32,
    /// The journal's lines that did not read, by number from 1.
    pub skipped: Vec<u32>,
}

/// Why a run cannot be read: a status and the words.
pub type Refused = (u16, String);

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The world's journals directory, if it is there and no part of it under the world is a
/// symlink.
pub(crate) fn journals_dir(world: &Path) -> Option<PathBuf> {
    let dir = journals(world);
    let rel = dir.strip_prefix(world).ok()?;
    let mut at = world.to_path_buf();
    for part in rel.components() {
        at.push(part);
        let meta = std::fs::symlink_metadata(&at).ok()?;
        if !meta.is_dir() {
            return None;
        }
    }
    Some(dir)
}

/// The bytes of the plain file at `path`, if it holds at most `cap`: the world's shared reader
/// (a link not followed, a pipe not waited on, the handle checked).
pub(crate) fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    match regular::read(path, cap) {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err("not read: it is gone".into()),
        Err(Unread::TooLarge(_)) => Err(format!("past {} MiB: not read whole", cap >> 20)),
        Err(e) => Err(format!("not read: {e}")),
    }
}

/// The journal's beats, and the numbers of the lines that do not read.
pub(crate) fn beats(bytes: &[u8]) -> (Vec<Beat>, Vec<u32>) {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for (i, line) in bytes.split(|&b| b == b'\n').enumerate() {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Beat>(line) {
            Ok(b) => out.push(b),
            Err(_) => skipped.push(count(i + 1)),
        }
    }
    (out, skipped)
}

/// Whether a live session holds `run` now: its lock probed for an instant, opened only as a
/// regular file.
fn held(dir: &Path, run: &str) -> bool {
    regular::open(&dir.join(format!("{run}.lock"))).is_ok_and(|f| {
        f.is_some_and(|f| matches!(f.try_lock_shared(), Err(std::fs::TryLockError::WouldBlock)))
    })
}

/// Where a run whose journal holds `beats` stands, and why — not knowing whether it is held.
fn standing(beats: &[Beat]) -> (RunState, String) {
    let last_end = beats.iter().rposition(|b| matches!(b, Beat::End { .. }));
    let last_ask = beats.iter().rposition(|b| matches!(b, Beat::Ask { .. }));
    let ended = last_end.filter(|e| last_ask.is_none_or(|a| a < *e));
    match ended.map(|e| &beats[e]) {
        Some(Beat::End { ending }) if ending == "interrupted" => {
            (RunState::Stopped, ending.clone())
        }
        Some(Beat::End { ending }) if ending.starts_with("checkpoint") => (
            RunState::Stopped,
            ending
                .strip_prefix("checkpoint: ")
                .unwrap_or(ending)
                .to_owned(),
        ),
        Some(Beat::End { ending }) => (RunState::Ended, ending.clone()),
        _ => (RunState::Stopped, "cut mid-turn".to_owned()),
    }
}

/// `s` on one line, cut to `chars` characters.
pub(crate) fn one_line(s: &str, chars: usize) -> String {
    let line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(chars) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line,
    }
}

/// RFC 3339 in UTC, to the second.
pub(crate) fn rfc3339(at: SystemTime) -> String {
    let secs = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 as a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The world's session runs, newest first: [`BoardRuns`].
#[must_use]
pub fn list(world: &Path) -> BoardRuns {
    let Some(dir) = journals_dir(world) else {
        return BoardRuns {
            runs: Vec::new(),
            more: 0,
        };
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return BoardRuns {
            runs: Vec::new(),
            more: 0,
        };
    };
    let mut found: Vec<(SystemTime, String)> = entries
        .take(ENTRIES)
        .filter_map(Result::ok)
        .filter_map(|e| {
            let run = e.file_name().to_str()?.strip_suffix(".jsonl")?.to_owned();
            if !is_session_run(&run) {
                return None;
            }
            let meta = std::fs::symlink_metadata(e.path()).ok()?;
            Some((meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), run))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    let more = count(found.len().saturating_sub(LISTED));
    let mut budget = LISTING_BYTES;
    let runs = found
        .into_iter()
        .take(LISTED)
        .map(|(at, id)| summary(&dir, id, at, &mut budget))
        .collect();
    BoardRuns { runs, more }
}

fn summary(dir: &Path, id: String, at: SystemTime, budget: &mut u64) -> BoardRun {
    let live = held(dir, &id);
    let unknown = |why: String| BoardRun {
        id: id.clone(),
        when: rfc3339(at),
        state: if live {
            RunState::Live
        } else {
            RunState::Unknown
        },
        why: mnemorph_loop::wire::shown(&why),
        ask: String::new(),
        turns: 0,
    };
    let cap = SUMMARY_BYTES.min(*budget);
    let bytes = match read_capped(&dir.join(format!("{id}.jsonl")), cap) {
        Ok(b) => b,
        Err(why) => return unknown(why),
    };
    *budget = budget.saturating_sub(bytes.len() as u64);
    let (beats, skipped) = beats(&bytes);
    if !skipped.is_empty() {
        return unknown(format!(
            "{} line(s) of its journal do not read",
            skipped.len()
        ));
    }
    let (state, why) = standing(&beats);
    let ask = beats
        .iter()
        .rev()
        .find_map(|b| match b {
            Beat::Ask { text } => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or_default();
    BoardRun {
        when: rfc3339(at),
        state: if live { RunState::Live } else { state },
        why: mnemorph_loop::wire::shown(&why),
        ask: mnemorph_loop::wire::shown(&one_line(ask, ASK_CHARS)),
        turns: count(
            beats
                .iter()
                .filter(|b| matches!(b, Beat::Ask { .. }))
                .count(),
        ),
        id,
    }
}

/// `s` cut to at most `max` bytes on a character boundary, and the bytes cut.
fn cut(s: &str, max: usize) -> (String, usize) {
    if s.len() <= max {
        return (s.to_owned(), 0);
    }
    let mut at = max;
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    (s[..at].to_owned(), s.len() - at)
}

/// The protocol's events a journal's beats stand for, in order. What the journal keeps for the
/// record alone — recall, the guts, the prefix, the model ridden, a compaction — is left out, as
/// a live chat never sends it.
#[must_use]
pub fn events(beats: &[Beat]) -> Vec<Event> {
    let text = |s: &str| cut(s, TEXT_BYTES).0;
    let mut out = Vec::new();
    for b in beats {
        match b {
            Beat::Ask { text: t } => out.push(Event::TurnStarted { text: text(t) }),
            Beat::Model { content, stop } => {
                let said: Vec<&str> = content
                    .iter()
                    .filter(|c| c["type"] == "text")
                    .filter_map(|c| c["text"].as_str())
                    .collect();
                if !said.is_empty() {
                    out.push(Event::Assistant {
                        text: text(&said.join("")),
                        stop: stop.clone(),
                    });
                }
                for c in content.iter().filter(|c| c["type"] == "tool_use") {
                    let (Some(id), Some(name)) = (c["id"].as_str(), c["name"].as_str()) else {
                        continue;
                    };
                    out.push(Event::ToolCall {
                        id: id.to_owned(),
                        name: name.to_owned(),
                        input: c.get("input").cloned().unwrap_or(Value::Null),
                    });
                }
            }
            Beat::Result {
                tool_use_id,
                content,
                is_error,
            } => {
                let (content, cut) = cut(content, RESULT_BYTES);
                out.push(Event::ToolResult {
                    id: tool_use_id.clone(),
                    content,
                    is_error: *is_error,
                    cut,
                });
            }
            Beat::Note { text: t } => out.push(Event::Note { text: text(t) }),
            Beat::Warning { text: t } => out.push(Event::Warning { text: text(t) }),
            Beat::Gate {
                changed,
                failures,
                unverified,
                verdict,
            } => out.push(Event::GateVerdict {
                passed: verdict == "PASS",
                verdict: verdict.clone(),
                changed: changed.clone(),
                failures: failures.clone(),
                unverified: unverified.clone(),
            }),
            Beat::End { ending } => out.push(Event::TurnEnded {
                ending: text(ending),
            }),
            _ => {}
        }
    }
    out
}

/// Run `run`'s transcript: [`BoardTranscript`].
///
/// # Errors
///
/// `run` names no session run (400), there is no such journal (404), or it is not a plain file
/// or past [`TRANSCRIPT_BYTES`] (422).
pub fn transcript(world: &Path, run: &str) -> Result<BoardTranscript, Refused> {
    if !is_session_run(run) {
        return Err((400, "a run is named by its id: r<ms>-<pid>".into()));
    }
    let Some(dir) = journals_dir(world) else {
        return Err((404, "the world keeps no run journals".into()));
    };
    let path = dir.join(format!("{run}.jsonl"));
    if std::fs::symlink_metadata(&path).is_err() {
        return Err((404, format!("no run `{run}` in this world")));
    }
    let bytes = read_capped(&path, TRANSCRIPT_BYTES).map_err(|why| (422, why))?;
    let (beats, skipped) = beats(&bytes);
    let (state, _) = standing(&beats);
    let state = if held(&dir, run) {
        RunState::Live
    } else {
        state
    };
    let all = events(&beats);
    let dropped = all.len().saturating_sub(TRANSCRIPT_EVENTS);
    let events = all
        .into_iter()
        .skip(dropped)
        .enumerate()
        .map(|(i, e)| {
            let mut params = serde_json::to_value(&e).unwrap_or(Value::Null);
            if let Some(o) = params.as_object_mut() {
                o.remove("type");
                o.insert("session".into(), json!(""));
                o.insert("run".into(), json!(run));
                o.insert("body".into(), json!(""));
                o.insert("seq".into(), json!(i + 1));
            }
            json!({"jsonrpc": "2.0", "method": format!("event/{}", e.kind()), "params": params})
        })
        .collect();
    Ok(BoardTranscript {
        run: run.to_owned(),
        state,
        events,
        dropped: count(dropped),
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_and_an_ending_read_as_the_resume_reads_them() {
        let ask = Beat::Ask { text: "go".into() };
        let end = |e: &str| Beat::End { ending: e.into() };
        assert_eq!(standing(std::slice::from_ref(&ask)).0, RunState::Stopped);
        assert_eq!(
            standing(&[ask.clone(), end("interrupted")]),
            (RunState::Stopped, "interrupted".into())
        );
        assert_eq!(
            standing(&[ask.clone(), end("checkpoint: the budget")]),
            (RunState::Stopped, "the budget".into())
        );
        assert_eq!(standing(&[ask.clone(), end("answered")]).0, RunState::Ended);
        assert_eq!(standing(&[end("answered"), ask]).0, RunState::Stopped);
    }

    #[test]
    fn a_cut_keeps_whole_characters() {
        assert_eq!(cut("héllo", 2), ("h".into(), 5));
        assert_eq!(cut("abc", 3), ("abc".into(), 0));
        assert_eq!(one_line("a\n b\tc", 2), "a …");
        assert_eq!(
            rfc3339(SystemTime::UNIX_EPOCH + std::time::Duration::from_hours(498_156)),
            "2026-10-30T12:00:00Z"
        );
        assert_eq!(rfc3339(SystemTime::UNIX_EPOCH), "1970-01-01T00:00:00Z");
    }
}
