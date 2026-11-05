//! The strain journal: what the council and the doom checkpoint saw of each body, kept by the
//! harness outside the world — `$XDG_STATE_HOME/mnemorph/strain/<world>-<fnv64a>.jsonl`, chained by
//! hash and anchored with the world's other chained journals ([`crate::chains`]). The council's own
//! tally is in the world, where a body writes; this is the copy a body cannot, so what rests on a
//! creature's strain — evolution's trigger — reads here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mnemorph_ledger::chain;
use mnemorph_ledger::{Build, Journal, Timestamp};
use serde::{Deserialize, Serialize};

/// The state directory's kind for the strain journal: `strain/`.
pub const KIND: &str = "strain";

/// The strain at which a creature is stressed: the council's line, unless its keys say another.
pub const LINE: u64 = 5;

/// What was seen of a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "act", rename_all = "kebab-case")]
pub enum Act {
    /// A run added strain.
    Strain {
        /// How much.
        by: u64,
    },
    /// Its parent was warranted to answer as its manager.
    Warrant,
    /// A clean landing, or relief: it starts again.
    Clear,
    /// A run ended at the doom checkpoint: guts warned three windows running.
    Doom,
}

/// One row of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// When.
    pub ts: Timestamp,
    /// The body.
    pub body: String,
    /// What was seen.
    #[serde(flatten)]
    pub act: Act,
    /// The build that wrote it.
    #[serde(default)]
    pub build: Build,
}

/// A body's stress since it was last cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stress {
    /// Its strain.
    pub strain: u64,
    /// Whether a run of its ended at the doom checkpoint.
    pub doomed: bool,
}

/// Where `world`'s journal is kept under the state directory `state`. The world is named by its
/// real path, so a session and a survey that spell it differently find one journal.
#[must_use]
pub fn path_in(state: &Path, world: &Path) -> PathBuf {
    let real = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    crate::queue::state_file_in(state, &real, KIND)
}

/// The journal at `path`, chained.
#[must_use]
pub fn journal_at(path: impl Into<PathBuf>) -> Journal<Row> {
    Journal::chained(path)
}

/// `act` of `body` appended to `world`'s journal, stamped with this binary's build.
///
/// # Errors
///
/// No home, the state directory lies inside the world, or the row cannot be written.
pub fn note(world: &Path, body: &str, act: Act) -> Result<(), String> {
    let state = crate::queue::state_dir()
        .ok_or("no home: the strain journal is kept under the human's state directory")?;
    let at = path_in(&state, world);
    crate::pins::outside_as(world, &at, "the strain journal is")?;
    journal_at(at)
        .append(&Row {
            ts: Timestamp::now(),
            body: body.to_owned(),
            act,
            build: Build::this(),
        })
        .map_err(|e| format!("the strain journal: {e}"))
}

/// Each body's stress, from `rows` in order.
#[must_use]
pub fn fold(rows: &[Row]) -> BTreeMap<String, Stress> {
    let mut out: BTreeMap<String, Stress> = BTreeMap::new();
    for row in rows {
        let s = out.entry(row.body.clone()).or_default();
        match row.act {
            Act::Strain { by } => s.strain = s.strain.saturating_add(by),
            Act::Doom => s.doomed = true,
            Act::Clear => *s = Stress::default(),
            Act::Warrant => {}
        }
    }
    out
}

/// Each body's stress as the state directory `state` keeps it, the chain walked whole and held to
/// the head anchored for it. A journal not yet written is no stress.
///
/// # Errors
///
/// The journal cannot be read, a link is broken, or a line does not parse.
pub fn read_in(state: &Path, world: &Path) -> Result<BTreeMap<String, Stress>, String> {
    let path = path_in(state, world);
    let anchor = crate::chains::anchor_under(state, world, KIND);
    let report = chain::verify_against(&path, anchor.as_ref())
        .map_err(|e| format!("the strain journal cannot be read: {e}"))?;
    if !report.holds() {
        return Err(format!("the strain journal does not verify: {report}"));
    }
    let back = journal_at(&path)
        .read()
        .map_err(|e| format!("the strain journal cannot be read: {e}"))?;
    if let Some(line) = back.skipped.first() {
        return Err(format!(
            "the strain journal does not verify: line {line} does not read"
        ));
    }
    Ok(fold(&back.rows))
}
