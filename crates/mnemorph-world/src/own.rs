//! Ownership: one function, one owner per path, the first rule that matches.

use mnemorph_races::Race;

use crate::doc::{Creature, covers};
use crate::roster::Roster;

/// Who owns a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// The harness alone writes it: the shared queue, the instruments, memory, the ontology's
    /// events, the indexes, evolution's staging, the journal.
    Harness,
    /// The human alone changes it, as a diff they apply: the law and the world's configuration.
    Human,
    /// One creature, by name.
    Creature(String),
    /// Nobody: Rimuru routes its birth. A path outside the world is nobody's too.
    Unowned,
}

/// Harness soil, under `.mnemorph/`.
const HARNESS: &[&str] = &[
    ".mnemorph/shared/",
    ".mnemorph/instruments/",
    ".mnemorph/memory/",
    ".mnemorph/ontology/",
    ".mnemorph/index/",
    ".mnemorph/tmp/evolution/",
];

/// The human's files.
const HUMAN: &[&str] = &[
    ".mnemorph/law.md",
    ".mnemorph/config.toml",
    ".mnemorph/config.local.toml",
];

/// The throne's files: the plan and the dimension manifest.
const THRONE: &[&str] = &[".mnemorph/plan.md", ".mnemorph/dimension.toml"];

/// The compiled zone that holds the workspace: root files and `tools/`.
pub const WORKSPACE: &str = "slime-workspace";

/// A path as ownership reads it: relative to the world, `/`-separated, read segment by segment —
/// `.` and empty segments dropped, `..` taking the one before it. A directory keeps its trailing
/// `/`. `None` for a path that is absolute, climbs out of the world, or names nothing.
#[must_use]
pub fn normal(path: &str) -> Option<String> {
    let path = path.trim().replace('\\', "/");
    if path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    if parts.is_empty() {
        return None;
    }
    let mut out = parts.join("/");
    if path.ends_with('/') {
        out.push('/');
    }
    Some(out)
}

/// The owner of `path`: the first rule that matches, in this order — harness soil and the
/// human's files, scratch, a living creature's seat (the deepest), the compiled soils, the
/// longest Slime claim, the longest Orc claim, else nobody. Ties on one claim break by creature
/// name, so the answer stays the same while the overlap is reported.
#[must_use]
pub fn own(roster: &Roster, path: &str) -> Owner {
    let Some(path) = normal(path) else {
        return Owner::Unowned;
    };
    let path = path.as_str();
    if path == ".mnemorph/log.md" || HARNESS.iter().any(|h| path.starts_with(h)) {
        return Owner::Harness;
    }
    if HUMAN.contains(&path) {
        return Owner::Human;
    }
    if let Some(rest) = path.strip_prefix(".mnemorph/tmp/") {
        let body = rest.split('/').next().unwrap_or_default();
        return roster
            .creatures
            .get(body)
            .filter(|c| !c.retired)
            .map_or(Owner::Unowned, |c| Owner::Creature(c.name.clone()));
    }
    if let Some(c) = roster
        .living()
        .filter(|c| path.starts_with(&c.seat))
        .max_by(|a, b| a.seat.len().cmp(&b.seat.len()).then(b.name.cmp(&a.name)))
    {
        return Owner::Creature(c.name.clone());
    }
    if let Some(owner) = compiled(roster, path) {
        return owner;
    }
    longest(roster, path, Race::Slime, |c| &c.zone)
        .or_else(|| longest(roster, path, Race::Orc, |c| &c.domain))
        .unwrap_or(Owner::Unowned)
}

/// The living creature of `race` whose claim on `path` is the longest.
fn longest(
    roster: &Roster,
    path: &str,
    race: Race,
    ground: fn(&Creature) -> &[String],
) -> Option<Owner> {
    roster
        .living()
        .filter(|c| c.race.base() == race)
        .flat_map(|c| ground(c).iter().map(move |claim| (claim, c)))
        .filter(|(claim, _)| covers(claim, path))
        .max_by(|(p, a), (q, b)| p.len().cmp(&q.len()).then(b.name.cmp(&a.name)))
        .map(|(_, c)| Owner::Creature(c.name.clone()))
}

/// The first living creature of `race`, by name.
fn first_of(roster: &Roster, race: Race) -> Owner {
    roster
        .living()
        .find(|c| c.race.base() == race)
        .map_or(Owner::Unowned, |c| Owner::Creature(c.name.clone()))
}

/// The compiled soils: the throne's files, the proving ground, a Kijin's post, the workspace.
fn compiled(roster: &Roster, path: &str) -> Option<Owner> {
    if THRONE.contains(&path) {
        return Some(first_of(roster, Race::Rimuru));
    }
    // One proving ground, one keeper: the first living Dark Elf by name.
    if path.starts_with(".mnemorph/dark-elf/") {
        return Some(first_of(roster, Race::DarkElf));
    }
    if let Some(post) = longest(roster, path, Race::Kijin, |c| &c.post) {
        return Some(post);
    }
    // A root file — a path with no `/` — or `tools/`. The records' own directory is no file.
    let root_file = !path.contains('/') && path != ".mnemorph";
    if root_file || path.starts_with("tools/") {
        return Some(
            roster
                .creatures
                .get(WORKSPACE)
                .filter(|c| !c.retired)
                .map_or(Owner::Unowned, |c| Owner::Creature(c.name.clone())),
        );
    }
    None
}
