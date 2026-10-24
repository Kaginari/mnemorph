//! Ownership: one function, one owner per path, the first rule that matches.

use mnemorph_races::Race;

use crate::doc::Creature;
use crate::roster::Roster;

/// Who owns a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// The harness alone writes it: the shared queue, the instruments, memory, the ontology's
    /// events, the indexes, the journal.
    Harness,
    /// The human alone changes it, as a diff they apply: the law.
    Human,
    /// One creature, by name.
    Creature(String),
    /// Nobody: Rimuru routes its birth.
    Unowned,
}

/// Harness soil, under `.mnemorph/`.
const HARNESS: &[&str] = &[
    ".mnemorph/shared/",
    ".mnemorph/instruments/",
    ".mnemorph/memory/",
    ".mnemorph/ontology/",
    ".mnemorph/index/",
];

/// The compiled zone that holds the workspace: root files and `tools/`.
pub const WORKSPACE: &str = "slime-workspace";

/// A path as ownership reads it: relative to the world, `/`-separated, no `./`.
#[must_use]
pub fn normal(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

/// The owner of `path`: the first rule that matches, in this order — harness soil, the law,
/// scratch, a living creature's seat (the deepest), the compiled soils, the longest Slime zone,
/// the longest Orc domain, else nobody. Ties on one prefix break by creature name, so the answer
/// stays the same while the overlap is reported.
#[must_use]
pub fn own(roster: &Roster, path: &str) -> Owner {
    let path = normal(path);
    if path == ".mnemorph/log.md" || HARNESS.iter().any(|h| path.starts_with(h)) {
        return Owner::Harness;
    }
    if path == ".mnemorph/law.md" {
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
    if let Some(owner) = compiled(roster, &path) {
        return owner;
    }
    let longest = |ground: fn(&Creature) -> &[String], race: Race| {
        roster
            .living()
            .filter(|c| c.race.base() == race)
            .flat_map(|c| ground(c).iter().map(move |p| (p, c)))
            .filter(|(p, _)| !p.is_empty() && path.starts_with(p.as_str()))
            .max_by(|(p, a), (q, b)| p.len().cmp(&q.len()).then(b.name.cmp(&a.name)))
            .map(|(_, c)| Owner::Creature(c.name.clone()))
    };
    longest(|c| &c.zone, Race::Slime)
        .or_else(|| longest(|c| &c.domain, Race::Orc))
        .unwrap_or(Owner::Unowned)
}

/// The compiled soils: the workspace, a Kijin's post, the proving ground, the plan and the
/// dimension manifest.
fn compiled(roster: &Roster, path: &str) -> Option<Owner> {
    let named = |name: &str| {
        roster
            .creatures
            .get(name)
            .filter(|c| !c.retired)
            .map(|c| Owner::Creature(c.name.clone()))
    };
    let first_of = |race: Race| {
        roster
            .living()
            .find(|c| c.race.base() == race)
            .map(|c| Owner::Creature(c.name.clone()))
    };
    if path == ".mnemorph/dimension.toml" || path.starts_with(".mnemorph/plan") {
        return Some(Owner::Creature("rimuru".into()));
    }
    if path.starts_with(".mnemorph/dark-elf/") {
        return first_of(Race::DarkElf).or(Some(Owner::Unowned));
    }
    if let Some((_, c)) = roster
        .living()
        .filter(|c| c.race.base() == Race::Kijin)
        .flat_map(|c| c.post.iter().map(move |p| (p, c)))
        .filter(|(p, _)| !p.is_empty() && path.starts_with(p.as_str()))
        .max_by(|(p, a), (q, b)| p.len().cmp(&q.len()).then(b.name.cmp(&a.name)))
    {
        return Some(Owner::Creature(c.name.clone()));
    }
    let root_file = !path.contains('/');
    if root_file || path.starts_with("tools/") {
        return named(WORKSPACE).or(Some(Owner::Unowned));
    }
    None
}
