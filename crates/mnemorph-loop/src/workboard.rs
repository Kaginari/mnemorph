//! The work board: `.mnemorph/tmp/board.md`, the one page the bodies of a session share. In
//! structured-coding and in collective-coding every commissioned body's brief carries it, and
//! what each body
//! found — the `@F` and `@W` lines of its answer — is posted back under its name, so a body
//! ordered later builds on what an earlier one read and does not start cold. The Summoner writes
//! the top of it: the map, and in the collective the brief, the list and the parts.
//!
//! It is one session's: a fresh session's first ask sets a board an earlier session left aside.
//! Not the web board (`mnemorphd board`), which is the human's page.

use std::io::Write as _;
use std::path::Path;

use crate::run::Ending;

/// The board, from the world's root.
pub const BOARD: &str = ".mnemorph/tmp/board.md";

/// Where a board an earlier session left is set aside, from the world's root.
pub const PREVIOUS: &str = ".mnemorph/tmp/board.prev.md";

/// The most of the board a brief carries, in bytes: its newest end when it is longer.
pub const BOARD_BYTES: usize = 24_000;

/// `ask` with the board before it, as a commissioned body is told it: the newest [`BOARD_BYTES`]
/// when the board is longer; `ask` as it stands when there is no board.
#[must_use]
pub fn with_board(world: &Path, ask: &str) -> String {
    let Ok(board) = std::fs::read_to_string(world.join(BOARD)) else {
        return ask.to_owned();
    };
    if board.trim().is_empty() {
        return ask.to_owned();
    }
    let mut from = board.len().saturating_sub(BOARD_BYTES);
    while !board.is_char_boundary(from) {
        from += 1;
    }
    format!(
        "The board ({BOARD}), as it stands now:\n\n{}\n\n---\n\nYour part:\n\n{ask}",
        &board[from..]
    )
}

/// The finding lines of a body's answer, posted to the board under its name; nothing when it did
/// not answer or found nothing. A board that cannot be written is left as it was: the report
/// still reaches its dispatcher.
pub fn post(world: &Path, body: &str, ending: &Ending) {
    let Ending::Answered(text) = ending else {
        return;
    };
    let found: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("@F ") || l.starts_with("@W "))
        .collect();
    if found.is_empty() {
        return;
    }
    let path = world.join(BOARD);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let entry = format!("\n## Found by {body}\n{}\n", found.join("\n"));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        // One write: two bodies posting at once do not interleave their lines.
        let _ = f.write_all(entry.as_bytes());
    }
}

/// Set a board an earlier session left aside, as [`PREVIOUS`], over any set aside before: its
/// findings are about a tree that may have moved since. Nothing when there is no board, or when
/// it is not a regular file.
pub fn set_aside(world: &Path) {
    let board = world.join(BOARD);
    if std::fs::symlink_metadata(&board).is_ok_and(|m| m.file_type().is_file()) {
        let _ = std::fs::rename(&board, world.join(PREVIOUS));
    }
}
