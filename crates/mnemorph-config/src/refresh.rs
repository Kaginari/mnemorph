//! The standing reads, on a schedule: the forge's CI runs and the advisory mirror with its audit,
//! each only when it is due. `mnemorphd refresh` is one tick; the terminal app starts one after a
//! session and the board one an hour for each world. A mirror can be hundreds of megabytes, so
//! ticks are cheap and being due is what costs: the intervals are the human's
//! (`refresh.ci_minutes`, `refresh.scout_hours`), and 0 turns one off.
//!
//! When each last ran is kept beside the instruments. A body may write that file, so a time in the
//! future counts as never: it can bring a read forward, never put one off.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// When each read last ran, relative to the world.
pub const STAMPS: &str = ".mnemorph/instruments/refresh.json";

/// How often each read runs; 0 is never.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Every {
    /// The forge's CI runs, in minutes.
    pub ci_minutes: u64,
    /// The advisory mirror and its audit, in hours.
    pub scout_hours: u64,
}

/// The reads a tick may run, each given so a test can stand in for the forge and the network.
pub struct Jobs<'a> {
    /// Reads the forge's runs; a line of what it did.
    pub ci: &'a dyn Fn() -> Result<String, String>,
    /// Refreshes the mirror; a line of what it did.
    pub scout: &'a dyn Fn() -> Result<String, String>,
    /// Audits the lockfiles against it; a line of what it found.
    pub audit: &'a dyn Fn() -> Result<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stamps {
    #[serde(default)]
    ci: i64,
    #[serde(default)]
    scout: i64,
}

fn due(last: i64, now: i64, every_secs: u64) -> bool {
    every_secs > 0
        && (last > now || i64::try_from(every_secs).is_ok_and(|every| now - last >= every))
}

/// One tick over `world` at `now` (seconds): what is due runs, and each says one line. A read that
/// fails says why, and is stamped all the same, so a world with no forge is not asked every tick.
#[must_use]
pub fn tick(world: &Path, every: Every, now: i64, jobs: &Jobs<'_>) -> Vec<String> {
    let at = world.join(STAMPS);
    let mut stamps: Stamps = fs::read(&at)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut said = Vec::new();
    let say = |what: &str, got: Result<String, String>| match got {
        Ok(line) => format!("{what}: {line}"),
        Err(why) => format!("{what}: not refreshed — {why}"),
    };
    if due(stamps.ci, now, every.ci_minutes.saturating_mul(60)) {
        stamps.ci = now;
        said.push(say("ci", (jobs.ci)()));
    }
    if due(stamps.scout, now, every.scout_hours.saturating_mul(3600)) {
        stamps.scout = now;
        let mirrored = (jobs.scout)();
        let whole = mirrored.is_ok();
        said.push(say("scout", mirrored));
        if whole {
            said.push(say("audit", (jobs.audit)()));
        }
    }
    if !said.is_empty() {
        if let Some(dir) = at.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_vec(&stamps) {
            let _ = fs::remove_file(&at);
            let _ = fs::write(&at, text);
        }
    }
    said
}

/// [`tick`] with the harness's own reads, under a lock so two never run at once.
#[must_use]
pub fn tick_now(world: &Path, every: Every) -> Vec<String> {
    let dir = world.join(".mnemorph/instruments");
    let _ = fs::create_dir_all(&dir);
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("refresh.lock"));
    let Ok(lock) = lock else {
        return vec!["the instruments cannot be written: nothing refreshed".to_owned()];
    };
    if lock.try_lock().is_err() {
        return vec!["a refresh is running already".to_owned()];
    }
    let now = mnemorph_ledger::Timestamp::now().0;
    let ci = || crate::ci::refresh_now(world, None).map(|(r, n)| format!("{r}, {n} run(s) moved"));
    let scout = || {
        let (lines, whole) = crate::scout::refresh_now(world)?;
        let said = lines.join("; ");
        if whole { Ok(said) } else { Err(said) }
    };
    let audit = || {
        crate::scout::audit_now(world)
            .map(|a| format!("{} package(s) with advisories", a.advisories.len()))
    };
    tick(
        world,
        every,
        now,
        &Jobs {
            ci: &ci,
            scout: &scout,
            audit: &audit,
        },
    )
}
