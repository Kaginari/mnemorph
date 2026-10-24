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

/// The proving ground: one ground, one keeper.
const PROVING_GROUND: &str = ".mnemorph/dark-elf/";

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

/// Which rule gave a path its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// The path is absolute, climbs out of the world, or names nothing.
    Outside,
    /// Harness soil (rule 1).
    Harness,
    /// The human's files (rule 1).
    Human,
    /// A body's scratch, `.mnemorph/tmp/<body>/` (rule 2).
    Scratch,
    /// A creature's seat (rule 3).
    Seat,
    /// The throne's files: the plan and the dimension manifest (rule 4).
    Throne,
    /// The proving ground (rule 4).
    ProvingGround,
    /// A Kijin's post (rule 4).
    Post,
    /// The workspace: root files and `tools/` (rule 4).
    Workspace,
    /// A Slime's zone (rule 5).
    Zone,
    /// An Orc's domain (rule 6).
    Domain,
    /// No rule matched (rule 7).
    Unclaimed,
}

impl Rule {
    /// The rule, in words.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Rule::Outside => "outside the world",
            Rule::Harness => "harness soil",
            Rule::Human => "the human's file",
            Rule::Scratch => "scratch",
            Rule::Seat => "seat",
            Rule::Throne => "the throne's file",
            Rule::ProvingGround => "the proving ground",
            Rule::Post => "post",
            Rule::Workspace => "the workspace",
            Rule::Zone => "zone",
            Rule::Domain => "domain",
            Rule::Unclaimed => "unclaimed: Rimuru routes its birth",
        }
    }
}

/// A path's owner, the rule that gave it, and the claim that matched — what `onto owner` prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ownership {
    /// The owner.
    pub owner: Owner,
    /// The rule.
    pub rule: Rule,
    /// The claim, seat or soil that matched, when one did.
    pub claim: Option<String>,
}

/// The owner of `path`: the first rule that matches, in this order — harness soil and the
/// human's files, scratch, a living creature's seat (the deepest), the compiled soils, the
/// longest Slime claim, the longest Orc claim, else nobody. Ties on one claim break by creature
/// name, so the answer stays the same while the overlap is reported.
#[must_use]
pub fn own(roster: &Roster, path: &str) -> Owner {
    explain(roster, path).owner
}

/// [`own`], with the rule and the claim that decided it.
#[must_use]
pub fn explain(roster: &Roster, path: &str) -> Ownership {
    let found = |owner: Owner, rule: Rule, claim: Option<&str>| Ownership {
        owner,
        rule,
        claim: claim.map(str::to_owned),
    };
    let Some(path) = normal(path) else {
        return found(Owner::Unowned, Rule::Outside, None);
    };
    let path = path.as_str();
    if path == ".mnemorph/log.md" {
        return found(Owner::Harness, Rule::Harness, Some(path));
    }
    if let Some(h) = HARNESS.iter().find(|h| path.starts_with(*h)) {
        return found(Owner::Harness, Rule::Harness, Some(h));
    }
    if HUMAN.contains(&path) {
        return found(Owner::Human, Rule::Human, Some(path));
    }
    if let Some(rest) = path.strip_prefix(".mnemorph/tmp/") {
        let body = rest.split('/').next().unwrap_or_default();
        let owner = living(roster, body);
        return found(
            owner,
            Rule::Scratch,
            Some(&format!(".mnemorph/tmp/{body}/")),
        );
    }
    if let Some(c) = roster
        .living()
        .filter(|c| path.starts_with(&c.seat))
        .max_by(|a, b| a.seat.len().cmp(&b.seat.len()).then(b.name.cmp(&a.name)))
    {
        return found(Owner::Creature(c.name.clone()), Rule::Seat, Some(&c.seat));
    }
    if let Some(owned) = compiled(roster, path) {
        return owned;
    }
    if let Some((claim, c)) = longest(roster, path, Race::Slime, |c| &c.zone) {
        return found(Owner::Creature(c.into()), Rule::Zone, Some(claim));
    }
    if let Some((claim, c)) = longest(roster, path, Race::Orc, |c| &c.domain) {
        return found(Owner::Creature(c.into()), Rule::Domain, Some(claim));
    }
    found(Owner::Unowned, Rule::Unclaimed, None)
}

/// The creature named `name`, if it lives; else nobody.
fn living(roster: &Roster, name: &str) -> Owner {
    roster
        .creatures
        .get(name)
        .filter(|c| !c.retired)
        .map_or(Owner::Unowned, |c| Owner::Creature(c.name.clone()))
}

/// The longest claim of a living creature of `race` on `path`, and the creature.
fn longest<'a>(
    roster: &'a Roster,
    path: &str,
    race: Race,
    ground: fn(&Creature) -> &[String],
) -> Option<(&'a str, &'a str)> {
    roster
        .living()
        .filter(|c| c.race.base() == race)
        .flat_map(|c| ground(c).iter().map(move |claim| (claim, c)))
        .filter(|(claim, _)| covers(claim, path))
        .max_by(|(p, a), (q, b)| p.len().cmp(&q.len()).then(b.name.cmp(&a.name)))
        .map(|(claim, c)| (claim.as_str(), c.name.as_str()))
}

/// The first living creature of `race`, by name.
fn first_of(roster: &Roster, race: Race) -> Owner {
    roster
        .living()
        .find(|c| c.race.base() == race)
        .map_or(Owner::Unowned, |c| Owner::Creature(c.name.clone()))
}

/// The compiled soils: the throne's files, the proving ground, a Kijin's post, the workspace.
fn compiled(roster: &Roster, path: &str) -> Option<Ownership> {
    let found = |owner: Owner, rule: Rule, claim: &str| {
        Some(Ownership {
            owner,
            rule,
            claim: Some(claim.to_owned()),
        })
    };
    if THRONE.contains(&path) {
        return found(first_of(roster, Race::Rimuru), Rule::Throne, path);
    }
    // One proving ground, one keeper: the first living Dark Elf by name.
    if path.starts_with(PROVING_GROUND) {
        return found(
            first_of(roster, Race::DarkElf),
            Rule::ProvingGround,
            PROVING_GROUND,
        );
    }
    if let Some((claim, c)) = longest(roster, path, Race::Kijin, |c| &c.post) {
        return found(Owner::Creature(c.into()), Rule::Post, claim);
    }
    // A root file — a path with no `/` — or `tools/`. The records' own directory is no file.
    let root_file = !path.contains('/') && path != ".mnemorph";
    if root_file || path.starts_with("tools/") {
        let claim = if root_file { path } else { "tools/" };
        return found(living(roster, WORKSPACE), Rule::Workspace, claim);
    }
    None
}
