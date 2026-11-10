//! The collective: the Summoner's members are equals — the same mind in different bodies, each
//! with its own context — not ranks. Only the Summoner summons; no member orders another. What
//! they share is the board (`.mnemorph/tmp/board.md`): the Summoner writes the brief, the map and
//! the parts on it, every member's brief carries it, and every member's findings are posted back
//! to it, so no member starts cold. On with the human's `collective.on`; off, the chain's law
//! holds as it was.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

use mnemorph_races::Race;
use mnemorph_world::Roster;

use crate::run::Ending;

/// The board, from the world's root.
pub const BOARD: &str = ".mnemorph/tmp/board.md";

/// The most of the board a brief carries, in bytes: its newest end when it is longer.
pub const BOARD_BYTES: usize = 24_000;

/// The throne's name: the only body that summons in the collective.
const THRONE: &str = "summoner";

/// The members: every living body that answers to the throne, with what it writes.
#[must_use]
pub fn members(roster: &Roster) -> Vec<(String, Race, Vec<String>)> {
    roster
        .living()
        .filter(|c| c.reports_to.as_deref() == Some(THRONE))
        .map(|c| (c.name.clone(), c.race, c.zone.clone()))
        .collect()
}

/// What the Summoner is told of the collective, its members listed.
#[must_use]
pub fn prompt(members: &[(String, Race, Vec<String>)]) -> String {
    let mut list = String::new();
    for (name, race, zone) in members {
        let _ = write!(list, "\n- {name} ({}", race.name());
        if !zone.is_empty() {
            let _ = write!(list, "; writes {}", zone.join(", "));
        }
        list.push(')');
    }
    format!(
        "\n\nYou are not alone here: this world is a collective. Its members are the same mind as \
you, each in a different body with its own clean context and its own zone; none orders another, \
and only you summon them, with the commission tool. The members:{list}\n\n\
How the collective works, in this order:\n\
1. Restructure first. Read only to map, never to fix: where each part of the ask lives, which \
files and lines, which zone. Then, before you change any code, write the board, `{BOARD}`, with \
the write tool; in the collective the board is your plan, in place of .mnemorph/tmp/plan.md. It \
holds `## Brief` (the ask in your words: the goal, what counts as done, each behaviour with its \
exact expected output), `## Map` (for each part, the files and line spans that matter and the \
traps you saw) and `## Parts` (one line per member you summon: `- <member>: <its share>`).\n\
2. Summon by need. An ask with two or more separate problems, or with changes in more than one \
zone, has more than one part: give each member whose zone it touches its share, all in one turn — \
commissions to different members in one turn run at once. A member writes only its own zone, so \
a change across zones is split by zone, each member's brief naming what the others make that it \
uses. One member, one brief: give it all of its share at once. You write no code yourself: the code is the members', and a write outside `{SUMMONER_SOIL}` is refused. Every brief carries the board, and the members' findings are posted back to it; \
name in `verified` the spans each member needs. A tester gets the brief and the parts, never the \
code being written: it writes, from the ask alone, the tests that prove each part.\n\
3. Judge, never redo. When the members have reported, run the tests the ask names and the \
code's own tests. A share that fails goes back to its member with the failure, at most twice; \
then give it to another member with the board's findings. A share that passes you do not touch."
    )
}

/// Where the Summoner writes in the collective: its scratch, the board's home. The code is the
/// members'.
pub const SUMMONER_SOIL: &str = ".mnemorph/tmp";

/// The Summoner's tools' context in the collective: it writes only [`SUMMONER_SOIL`], and a write
/// anywhere else is refused naming the member whose zone holds it.
#[must_use]
pub fn summoner_context(cx: mnemorph_tools::Context, roster: Roster) -> mnemorph_tools::Context {
    cx.with_soil(&[SUMMONER_SOIL], &[] as &[&str], false)
        .with_owner_of(std::sync::Arc::new(move |path: &str| keeper(&roster, path)))
}

/// Who keeps `path` in the collective, as a refusal says it.
#[must_use]
pub fn keeper(roster: &Roster, path: &str) -> String {
    match mnemorph_world::own(roster, path) {
        mnemorph_world::Owner::Creature(c) if c != THRONE => format!(
            "it is {c}'s zone: in the collective the code is the members' — put the change in \
             {c}'s share on the board and commission {c}"
        ),
        _ => "in the collective the code is the members': give it to the member whose zone is \
              nearest, on the board, and commission it"
            .to_owned(),
    }
}

/// What a member reads first of its race, in the collective.
#[must_use]
pub fn card(race: Race) -> &'static str {
    match race.base() {
        Race::DarkElf => {
            "You are a member of a collective: the same mind as the others, in a different body. \
Your part is the challenge: find what is wrong with what landed, with evidence."
        }
        Race::Kijin => {
            "You are a member of a collective: the same mind as the others, in a different body. \
Your part is the outward acts of your post — what leaves the machine — through the outward tool."
        }
        _ => {
            "You are a member of a collective: the same mind as every other member, in a different \
body, each with its own part of the work. No one orders you and you order no one: the Summoner \
split the ask, and the part in your brief is yours. Your brief carries the board — the shared \
brief, the map, and what the other members found: build on it, and do not read again what it \
already shows. Change only what your part needs. When your part needs a change in another \
member's part, do not make it: say it on an @F line naming the file, the line and the change. When \
you answer, give one @F line for each thing another member would need: a path with its lines, a \
name, a trap."
        }
    }
}

/// Whether `parent` may commission `to` in the collective: the Summoner may summon any living
/// member but itself; no member orders another.
///
/// # Errors
///
/// The reason, with the lawful next act.
pub fn lawful(roster: &Roster, parent: &str, to: &str) -> Result<(), String> {
    if parent != THRONE {
        return Err(format!(
            "refused: in the collective only the Summoner summons — say what {to} should do on an \
             @F line in your answer, and the Summoner decides"
        ));
    }
    match roster.creatures.get(to).filter(|c| !c.retired) {
        Some(c) if c.name != THRONE => Ok(()),
        _ => Err(format!("`{to}` is no living member of the collective")),
    }
}

/// `ask` with the board before it, as a member is told it: the newest [`BOARD_BYTES`] when the
/// board is longer; `ask` as it stands when there is no board.
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

/// The finding lines of a member's answer, posted to the board under its name; nothing when it
/// did not answer or found nothing. A board that cannot be written is left as it was: the report
/// still reaches the Summoner.
pub fn post(world: &Path, member: &str, ending: &Ending) {
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
    let entry = format!("\n## Found by {member}\n{}\n", found.join("\n"));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        // One write: two members posting at once do not interleave their lines.
        let _ = f.write_all(entry.as_bytes());
    }
}
