//! The Minds' witness: each Mind's adoption — its doc, the SHA-256 of its bytes when adopted, its
//! owner — and its retirement, kept by the harness outside the world at
//! `$XDG_STATE_HOME/mnemorph/minds/<world>-<fnv64a>.jsonl`, chained by hash
//! ([`mnemorph_ledger::chain`]) and anchored with the world's other chained journals
//! ([`crate::chains`]). Only the human's `mnemorphd mind adopt|retire|unretire` writes it, asked at
//! the terminal; a body's command is refused. Nothing read inside the world grants a Mind trust:
//! a body's write there is indistinguishable from the human's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mnemorph_ledger::chain;
use mnemorph_ledger::{Build, Journal, Timestamp};
use serde::{Deserialize, Serialize};

/// The human, as a Mind's owner.
pub const HUMAN: &str = "human";

/// What the human did to a Mind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Act {
    /// Adopted it, as its doc's bytes stand: it loads while they stay so.
    Adopt,
    /// Retired it — retired, or found poisoned: it no longer loads.
    Retire,
    /// Took its retirement back: it loads again while its bytes are the adopted ones.
    Unretire,
}

impl Act {
    /// The act's word: `adopt`, `retire`, `unretire`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Act::Adopt => "adopt",
            Act::Retire => "retire",
            Act::Unretire => "unretire",
        }
    }

    /// The act a command names.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Act> {
        [Act::Adopt, Act::Retire, Act::Unretire]
            .into_iter()
            .find(|a| a.word() == word)
    }
}

/// One row of the witness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// When the human did it.
    pub ts: Timestamp,
    /// What they did.
    pub act: Act,
    /// The Mind's name.
    pub mind: String,
    /// Its doc, relative to the world.
    pub doc: String,
    /// The SHA-256 of the doc's bytes then, in hex.
    pub sha256: String,
    /// Its owner — the human, or the creature that proposed it; given on adoption only.
    #[serde(default)]
    pub owner: Option<String>,
    /// The build that wrote the row.
    #[serde(default)]
    pub build: Build,
}

/// One Mind as the witness holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witnessed {
    /// Its doc when last adopted or retired.
    pub doc: String,
    /// The SHA-256 of its bytes when adopted; `None` when it was retired and never adopted.
    pub adopted: Option<String>,
    /// Its owner, named at adoption.
    pub owner: Option<String>,
    /// Retired, by the human.
    pub retired: bool,
}

/// Where `world`'s Minds' witness is kept: `$XDG_STATE_HOME/mnemorph/minds/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` with no home. The world is named by its real path, so
/// every spelling of it (`/var/…` and `/private/var/…` on macOS, a symlink) finds one witness.
#[must_use]
pub fn path_of(world: &Path) -> Option<PathBuf> {
    let real = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    crate::queue::state_file(&real, "minds")
}

/// The witness kept at `path`, chained.
#[must_use]
pub fn journal_at(path: impl Into<PathBuf>) -> Journal<Row> {
    Journal::chained(path)
}

/// Each Mind the witness at `path` holds, folded from its rows; the chain is walked first, held
/// to `anchor` when one was kept. A witness that does not walk whole is an error: a dropped
/// retirement is never read as none.
///
/// # Errors
///
/// The witness cannot be read, a link is broken, or a line does not parse.
pub fn fold_at(
    path: &Path,
    anchor: Option<&chain::Head>,
) -> Result<BTreeMap<String, Witnessed>, String> {
    let report = chain::verify_against(path, anchor)
        .map_err(|e| format!("the Minds' witness cannot be read: {e}"))?;
    if !report.holds() {
        return Err(format!("the Minds' witness does not verify: {report}"));
    }
    let back = journal_at(path)
        .read()
        .map_err(|e| format!("the Minds' witness cannot be read: {e}"))?;
    if let Some(line) = back.skipped.first() {
        return Err(format!(
            "the Minds' witness does not verify: line {line} does not read"
        ));
    }
    Ok(fold(&back.rows))
}

/// The witness's rows folded, per Mind.
#[must_use]
pub fn fold(rows: &[Row]) -> BTreeMap<String, Witnessed> {
    let mut out: BTreeMap<String, Witnessed> = BTreeMap::new();
    for r in rows {
        let w = out.entry(r.mind.clone()).or_insert_with(|| Witnessed {
            doc: r.doc.clone(),
            adopted: None,
            owner: None,
            retired: false,
        });
        match r.act {
            Act::Adopt => {
                w.doc.clone_from(&r.doc);
                w.adopted = Some(r.sha256.clone());
                w.owner.clone_from(&r.owner);
                w.retired = false;
            }
            Act::Retire => w.retired = true,
            Act::Unretire => w.retired = false,
        }
    }
    out
}

/// `row` appended to the witness at `path`, stamped with this binary's build. Refused inside a
/// body's command (`MNEMORPH_BODY` is set there): a Mind's provenance is the human's alone.
///
/// # Errors
///
/// A body asks, or the witness cannot be written.
pub fn record_at(path: &Path, mut row: Row) -> Result<(), String> {
    if let Some(body) = std::env::var_os("MNEMORPH_BODY") {
        return Err(format!(
            "{} runs this: a Mind is adopted, retired and unretired by the human alone",
            body.to_string_lossy()
        ));
    }
    row.build = Build::this();
    journal_at(path)
        .append(&row)
        .map_err(|e| format!("the Minds' witness cannot be written: {e}"))
}
