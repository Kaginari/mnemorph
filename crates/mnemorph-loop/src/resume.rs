//! Resuming a session's run: which runs a world's journals hold that stopped and may be picked
//! up, and the hold a live session keeps on its run, so no second session takes it.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use mnemorph_ledger::{Journal, Timestamp};

use crate::beats::Beat;

/// Where a world's run journals live: `<run>.jsonl`, one per run, and `<run>.lock` beside each
/// session's.
#[must_use]
pub fn journals(world: &Path) -> PathBuf {
    world.join(".mnemorph/instruments/loop")
}

/// A session's run that stopped and may be resumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    /// The run's id.
    pub run: String,
    /// When its journal was last written.
    pub when: Timestamp,
    /// Why it stopped: the checkpoint's words, `interrupted`, or `cut mid-turn`.
    pub why: String,
    /// The last thing the human asked it.
    pub ask: String,
    /// The journal's time, for the order.
    at: SystemTime,
}

/// A session's run id is `r<milliseconds>-<process>`; a commissioned body's run is named for the
/// body, and is its dispatcher's to resume.
#[must_use]
pub fn is_session_run(id: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    id.strip_prefix('r')
        .and_then(|rest| rest.split_once('-'))
        .is_some_and(|(at, pid)| digits(at) && digits(pid))
}

/// Why a journal's run may be picked up, or why there is nothing to resume.
fn stop_of(beats: &[Beat]) -> Result<String, String> {
    let last_end = beats.iter().rposition(|b| matches!(b, Beat::End { .. }));
    let last_ask = beats.iter().rposition(|b| matches!(b, Beat::Ask { .. }));
    let Some(end) = last_end.filter(|e| last_ask.is_none_or(|a| a < *e)) else {
        return Ok("cut mid-turn".into());
    };
    let Beat::End { ending } = &beats[end] else {
        unreachable!("found as an End")
    };
    match ending.as_str() {
        "interrupted" => Ok(ending.clone()),
        e if e.starts_with("checkpoint") => {
            Ok(e.strip_prefix("checkpoint: ").unwrap_or(e).to_owned())
        }
        e => Err(format!("ended {e}: there is nothing to resume")),
    }
}

/// The last ask, on one line, cut short, every control, bidi and zero-width character escaped: a
/// journal from a cloned world may carry terminal escapes, and it is printed.
fn last_ask(beats: &[Beat]) -> String {
    let ask = beats
        .iter()
        .rev()
        .find_map(|b| match b {
            Beat::Ask { text } => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or_default();
    let line = crate::run::shown(&ask.split_whitespace().collect::<Vec<_>>().join(" "));
    match line.char_indices().nth(72) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line,
    }
}

/// A live session's hold on its run: while it lives — and the process with it — no other session
/// resumes the run. The operating system lets go of it when the process ends, however it ends.
/// When the session ends with nothing left to resume, the lock file goes, unlinked while still
/// held, so no live holder is ever raced.
#[derive(Debug)]
pub struct Hold {
    file: Option<File>,
    lock: PathBuf,
    journal: PathBuf,
}

impl Drop for Hold {
    fn drop(&mut self) {
        let done = match Journal::<Beat>::new(self.journal.clone()).read() {
            Ok(back) => {
                back.skipped.is_empty() && (back.rows.is_empty() || stop_of(&back.rows).is_err())
            }
            Err(_) => !self.journal.exists(),
        };
        if done {
            let _ = std::fs::remove_file(&self.lock);
        }
        // The lock is let go only after the file is gone.
        drop(self.file.take());
    }
}

/// How long a resume waits for a run's lock that a listing probes for a moment.
const PROBE_WAIT: std::time::Duration = std::time::Duration::from_millis(250);

/// Hold `run`.
///
/// # Errors
///
/// Another live session holds it, or its lock file cannot be made.
pub fn hold(world: &Path, run: &str) -> Result<Hold, String> {
    let dir = journals(world);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("{run}.lock"));
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    // A listing's probe holds the lock for an instant: a resume waits that out, never a session.
    let until = std::time::Instant::now() + PROBE_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => {
                return Ok(Hold {
                    file: Some(file),
                    journal: dir.join(format!("{run}.jsonl")),
                    lock: path,
                });
            }
            Err(TryLockError::WouldBlock) if std::time::Instant::now() < until => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(format!(
                    "run `{run}` is held by another live session: end it there first"
                ));
            }
            Err(TryLockError::Error(e)) => return Err(format!("{}: {e}", path.display())),
        }
    }
}

/// Whether a live session holds `run` now: a non-blocking probe, its shared lock let go at once —
/// a resume that meets it waits it out.
fn held(world: &Path, run: &str) -> bool {
    let path = journals(world).join(format!("{run}.lock"));
    File::open(&path).is_ok_and(|f| matches!(f.try_lock_shared(), Err(TryLockError::WouldBlock)))
}

/// The world's session runs a live session holds now, by id, sorted: each lock probed for an
/// instant, as a listing probes. Only a regular lock file is opened — the directory is the world's,
/// and a pipe planted in its place would hang the open.
#[must_use]
pub fn live(world: &Path) -> Vec<String> {
    let Ok(dir) = std::fs::read_dir(journals(world)) else {
        return Vec::new();
    };
    let mut out: Vec<String> = dir
        .filter_map(Result::ok)
        .filter_map(|e| {
            let run = e.file_name().to_str()?.strip_suffix(".lock")?.to_owned();
            let regular = std::fs::symlink_metadata(e.path()).is_ok_and(|m| m.is_file());
            (regular && is_session_run(&run) && held(world, &run)).then_some(run)
        })
        .collect();
    out.sort();
    out
}

/// The world's session runs that stopped and may be resumed, newest first: a checkpoint, an
/// interrupted turn, or a turn the process died in. A run a live session holds, one that answered
/// or failed, and a journal with a line that does not read are left out.
#[must_use]
pub fn stopped(world: &Path) -> Vec<Stopped> {
    let Ok(dir) = std::fs::read_dir(journals(world)) else {
        return Vec::new();
    };
    let mut out: Vec<Stopped> = dir
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            let run = path
                .file_name()?
                .to_str()?
                .strip_suffix(".jsonl")?
                .to_owned();
            if !is_session_run(&run) || held(world, &run) {
                return None;
            }
            let back = Journal::<Beat>::new(path.clone()).read().ok()?;
            if !back.skipped.is_empty() || back.rows.is_empty() {
                return None;
            }
            let why = stop_of(&back.rows).ok()?;
            let at = e.metadata().ok()?.modified().ok()?;
            let secs = at
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            Some(Stopped {
                ask: last_ask(&back.rows),
                why: crate::run::shown(&why),
                run,
                when: Timestamp(secs),
                at,
            })
        })
        .collect();
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.run.cmp(&a.run)));
    out
}

/// The run `which` names — its id, or `last` for the newest that stopped — held for this session,
/// with its journal's beats.
///
/// # Errors
///
/// No such run, its journal does not read whole, it answered or failed, or another live session
/// holds it.
pub fn take(world: &Path, which: &str) -> Result<(String, Hold, Vec<Beat>), String> {
    let dir = journals(world);
    let run = if which == "last" {
        stopped(world)
            .into_iter()
            .next()
            .map(|s| s.run)
            .ok_or_else(|| format!("no run in {} stopped to be resumed", dir.display()))?
    } else if is_session_run(which) {
        which.to_owned()
    } else {
        return Err(format!(
            "`{which}` names no session's run: give its id (`r…`, as `mnemorphd runs` lists it) or `last`"
        ));
    };
    let path = dir.join(format!("{run}.jsonl"));
    if !path.is_file() {
        return Err(format!("no run `{run}`: {} does not exist", path.display()));
    }
    let hold = hold(world, &run)?;
    let back = Journal::<Beat>::new(path.clone())
        .read()
        .map_err(|e| format!("run `{run}`: its journal cannot be read: {e}"))?;
    if !back.skipped.is_empty() {
        return Err(format!(
            "run `{run}`: its journal is corrupt — line(s) {:?} of {} do not read, so the \
             conversation cannot be rebuilt as it was",
            back.skipped,
            path.display()
        ));
    }
    if back.rows.is_empty() {
        return Err(format!("run `{run}`: its journal holds no beat"));
    }
    stop_of(&back.rows).map_err(|why| format!("run `{run}` {why}"))?;
    Ok((run, hold, back.rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_run_is_named_by_the_clock_and_the_process() {
        assert!(is_session_run("r1730000000000-42"));
        for no in ["r-1", "r12-", "rimuru-1", "slime-r1-2", "r1-2/x", "../r1-2"] {
            assert!(!is_session_run(no), "{no}");
        }
    }

    #[test]
    fn only_a_stop_is_resumed() {
        let ask = Beat::Ask { text: "go".into() };
        let end = |e: &str| Beat::End { ending: e.into() };
        assert_eq!(
            stop_of(&[ask.clone(), end("checkpoint: steps: 1 taken")]),
            Ok("steps: 1 taken".into())
        );
        assert_eq!(
            stop_of(&[ask.clone(), end("interrupted")]),
            Ok("interrupted".into())
        );
        assert_eq!(
            stop_of(std::slice::from_ref(&ask)),
            Ok("cut mid-turn".into())
        );
        assert_eq!(
            stop_of(&[ask.clone(), end("answered"), ask.clone()]),
            Ok("cut mid-turn".into()),
            "the process died in a later ask"
        );
        assert!(stop_of(&[ask.clone(), end("answered")]).is_err());
        assert!(stop_of(&[ask, end("failed: the provider")]).is_err());
    }

    #[test]
    fn a_held_run_reads_live_and_a_planted_pipe_is_never_opened() {
        let world = std::env::temp_dir().join(format!("mnemorph-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&world);
        assert_eq!(live(&world), Vec::<String>::new());
        let run = "r1730000000000-7";
        let hold = hold(&world, run).unwrap();
        // A body's run, and a lock no one holds, are not live.
        drop(super::hold(&world, "slime-x").unwrap());
        std::fs::write(journals(&world).join("r1-2.lock"), "").unwrap();
        #[cfg(unix)]
        assert!(
            std::process::Command::new("mkfifo")
                .arg(journals(&world).join("r3-4.lock"))
                .status()
                .is_ok_and(|s| s.success())
        );
        assert_eq!(live(&world), vec![run.to_owned()]);
        drop(hold);
        assert_eq!(live(&world), Vec::<String>::new());
        let _ = std::fs::remove_dir_all(&world);
    }
}
