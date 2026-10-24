//! The graph: the docs' fields as triples, closed under the ontology's rules to their least fixed
//! point. Derived, never written back — the docs are the truth.

use std::collections::BTreeSet;

use mnemorph_races::Race;

use crate::roster::Roster;

/// A bond between two nodes of the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bond {
    /// A creature is of a race; an ascended race is of its base race; a base race is a creature.
    IsA,
    /// Above in the chain of command: transitive.
    Above,
    /// The inverse of [`Bond::Above`].
    Below,
    /// A Slime's truth goes to its Orc: a sub-bond of `Above`.
    Truth,
    /// An Orc's verdict goes to its Elf: a sub-bond of `Above`.
    Verdict,
    /// A report reaches the throne: a sub-bond of `Above`.
    Reports,
    /// A creature owns a path: a Slime's zone.
    Owns,
    /// An Orc routes and judges a path: its domain.
    Routes,
    /// A Kijin's outward acts own a path: its post.
    Posts,
    /// A creature wears a Mind.
    Wears,
    /// The inverse of [`Bond::Wears`].
    WornBy,
}

/// One triple: subject, bond, object.
pub type Triple = (String, Bond, String);

/// The class every race is, at the top of the hierarchy.
pub const CREATURE: &str = "Creature";

/// The closed graph.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph {
    /// Every triple, asserted and inferred.
    pub triples: BTreeSet<Triple>,
}

impl Graph {
    /// The graph of `roster`'s living creatures, closed to its fixed point.
    #[must_use]
    pub fn of(roster: &Roster) -> Graph {
        let mut t: BTreeSet<Triple> = BTreeSet::new();
        let race = |r: Race| r.name().to_owned();
        for r in Race::ALL {
            let base = r.base();
            if base == r {
                t.insert((race(r), Bond::IsA, CREATURE.into()));
            } else {
                t.insert((race(r), Bond::IsA, race(base)));
            }
        }
        for c in roster.living() {
            t.insert((c.name.clone(), Bond::IsA, race(c.race)));
            if let Some(up) = &c.reports_to
                && let Some(parent) = roster.creatures.get(up).filter(|p| !p.retired)
            {
                let sub = match (c.race.base(), parent.race.base()) {
                    (_, Race::Rimuru) => Some(Bond::Reports),
                    (Race::Slime, Race::Orc) => Some(Bond::Truth),
                    (Race::Orc, Race::Elf) => Some(Bond::Verdict),
                    _ => None,
                };
                // `a Above b`: a is above b.
                if let Some(sub) = sub {
                    t.insert((parent.name.clone(), sub, c.name.clone()));
                }
                t.insert((parent.name.clone(), Bond::Above, c.name.clone()));
            }
            for (bond, claims) in [
                (Bond::Owns, &c.zone),
                (Bond::Routes, &c.domain),
                (Bond::Posts, &c.post),
            ] {
                for claim in claims {
                    t.insert((c.name.clone(), bond, claim.clone()));
                }
            }
            for mind in &c.minds {
                t.insert((c.name.clone(), Bond::Wears, mind.clone()));
            }
        }
        Graph { triples: close(t) }
    }

    /// Whether the graph holds `subject bond object`.
    #[must_use]
    pub fn has(&self, subject: &str, bond: Bond, object: &str) -> bool {
        self.triples
            .contains(&(subject.to_owned(), bond, object.to_owned()))
    }

    /// Every object of `subject bond _`.
    pub fn objects<'a>(&'a self, subject: &'a str, bond: Bond) -> impl Iterator<Item = &'a str> {
        self.triples
            .iter()
            .filter(move |(s, b, _)| s == subject && *b == bond)
            .map(|(_, _, o)| o.as_str())
    }
}

/// Apply the rules until nothing new appears: subclass closure, sub-bonds into `Above`, the
/// transitivity of `Above` and `IsA`, and the inverses. The rules only add triples over a finite
/// set of nodes, so the fixed point is unique and reached.
fn close(mut t: BTreeSet<Triple>) -> BTreeSet<Triple> {
    loop {
        let mut new: Vec<Triple> = Vec::new();
        for (s, b, o) in &t {
            match b {
                Bond::Truth | Bond::Verdict | Bond::Reports => {
                    new.push((s.clone(), Bond::Above, o.clone()));
                }
                Bond::Above => new.push((o.clone(), Bond::Below, s.clone())),
                Bond::Wears => new.push((o.clone(), Bond::WornBy, s.clone())),
                _ => {}
            }
            if matches!(b, Bond::Above | Bond::IsA) {
                // The set is ordered by subject, then bond: `o b _` is one range, not a scan.
                let from = (o.clone(), *b, String::new());
                for (_, _, o2) in t.range(from..).take_while(|(s2, b2, _)| s2 == o && b2 == b) {
                    new.push((s.clone(), *b, o2.clone()));
                }
            }
        }
        let before = t.len();
        t.extend(new);
        if t.len() == before {
            return t;
        }
    }
}
