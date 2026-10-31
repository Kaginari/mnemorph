//! The human's answers to the gate given on the board, kept by the harness outside the world:
//! `$XDG_STATE_HOME/mnemorph/answers/<world>-<fnv64a>.jsonl`, beside the end gate's verdicts
//! ([`crate::verdicts`]). One row per answer the board took or refused — the chat, the ask, the
//! tab's connection, the digest of the act as it was shown and the one the answer named, and how
//! long after the ask and after the card was first seen the answer came. Written by the board
//! alone, chained by hash ([`mnemorph_ledger::chain`]) and anchored with the world's other chained
//! journals ([`crate::chains`]); each row carries the build that wrote it.

use std::io;
use std::path::{Path, PathBuf};

use mnemorph_ledger::{Build, Journal, Timestamp};
use serde::{Deserialize, Serialize};

/// The state directory's kind for the answers: `answers/`.
pub const KIND: &str = "answers";

/// One answer to the gate, given on the board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerRow {
    /// When it came.
    pub ts: Timestamp,
    /// The board's chat.
    pub chat: String,
    /// The ask it answers: the core's request id (`gate-<n>`).
    pub ask: String,
    /// The tab's connection on the chat: a number the bridge gives each, in order.
    pub conn: u64,
    /// The body that asked, as shown.
    pub body: String,
    /// The act's class, as shown.
    pub class: String,
    /// The act as the card showed it, cut to its first 4 KiB.
    pub act: String,
    /// The SHA-256 of the act as shown, as the board sent it.
    pub shown_digest: String,
    /// The digest the answer named; `None` when it named none.
    #[serde(default)]
    pub named_digest: Option<String>,
    /// Whether the answer allowed the act.
    pub allow: bool,
    /// `answered` when the answer reached the core, else `refused: <why>`.
    pub outcome: String,
    /// The milliseconds from the ask to the answer.
    pub asked_ms: u64,
    /// The milliseconds from this tab's render acknowledgement to the answer; `None` when it
    /// sent none.
    #[serde(default)]
    pub seen_ms: Option<u64>,
    /// The build that took it.
    #[serde(default)]
    pub build: Build,
}

/// Where `world`'s board answers are kept: `$XDG_STATE_HOME/mnemorph/answers/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` when there is no home.
#[must_use]
pub fn answers_of(world: &Path) -> Option<PathBuf> {
    crate::queue::state_file(world, KIND)
}

/// [`answers_of`], the human's state directory at `state`.
#[must_use]
pub fn answers_in(state: &Path, world: &Path) -> PathBuf {
    crate::queue::state_file_in(state, world, KIND)
}

/// The answers kept at `path`, chained.
#[must_use]
pub fn journal_at(path: impl Into<PathBuf>) -> Journal<AnswerRow> {
    Journal::chained(path)
}

/// Appends `row` to `journal`, stamped with this binary's build.
///
/// # Errors
///
/// The row cannot be written.
pub fn record(journal: &Journal<AnswerRow>, mut row: AnswerRow) -> io::Result<()> {
    row.build = Build::this();
    journal.append(&row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answers_live_outside_the_world_and_are_anchored_with_the_others() {
        let world = Path::new("/srv/worlds/jura");
        let Some(at) = answers_of(world) else {
            return;
        };
        assert!(!at.starts_with(world), "{}", at.display());
        assert!(at.to_string_lossy().contains("mnemorph/answers/jura-"));
        assert!(journal_at(&at).is_chained());
        assert!(
            crate::chains::journals(world)
                .iter()
                .any(|j| j.name == "answers" && j.path == at)
        );
    }

    #[test]
    fn a_row_is_stamped_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let j = journal_at(answers_in(dir.path(), Path::new("/w")));
        let row = AnswerRow {
            ts: Timestamp::now(),
            chat: "c".into(),
            ask: "gate-1".into(),
            conn: 2,
            body: "rimuru".into(),
            class: "write".into(),
            act: "rm -rf build".into(),
            shown_digest: "d".into(),
            named_digest: Some("d".into()),
            allow: true,
            outcome: "answered".into(),
            asked_ms: 1200,
            seen_ms: Some(900),
            build: Build::default(),
        };
        record(&j, row.clone()).unwrap();
        let back = j.read().unwrap().rows;
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].build, Build::this());
        assert_eq!(back[0].ask, row.ask);
        assert!(j.verify().unwrap().broken.is_none());
    }
}
