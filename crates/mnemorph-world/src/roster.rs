//! The roster: every creature the world's docs describe, read fresh when the docs change.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::ErrorKind;
use std::path::Path;

use mnemorph_races::Race;

use crate::doc::{Creature, parse};

/// The creatures of a world, by name, and the docs that could not be read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roster {
    /// Every creature, by name.
    pub creatures: BTreeMap<String, Creature>,
    /// Docs that could not be read, and why.
    pub problems: Vec<String>,
    /// The docs' fingerprint: when it moves, the roster is read again. In-process only — the
    /// hasher is not stable across Rust releases, so it is never written down.
    pub fingerprint: u64,
}

/// Names no creature may take: the human and the harness stand over every body.
const RESERVED: &[&str] = &[crate::queue::HUMAN, crate::queue::HARNESS];

impl Roster {
    /// Read every `.mnemorph/<race>/<name>/README.md` of `world`.
    #[must_use]
    pub fn load(world: &Path) -> Roster {
        let mut roster = Roster::default();
        let mut hasher = DefaultHasher::new();
        let records = world.join(".mnemorph");
        for race in Race::ALL {
            let dir = records.join(race.name());
            let seats = match fs::read_dir(&dir) {
                Ok(seats) => seats,
                Err(e) if e.kind() == ErrorKind::NotFound => continue,
                Err(e) => {
                    let why = format!(
                        "{}: the race's directory cannot be read: {e}",
                        dir.display()
                    );
                    why.hash(&mut hasher);
                    roster.problems.push(why);
                    continue;
                }
            };
            let mut seats: Vec<_> = seats
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            seats.sort();
            for seat in seats {
                let doc = seat.join("README.md");
                let rel = seat.strip_prefix(world).unwrap_or(&seat).to_path_buf();
                // No doc is no creature; a doc that cannot be read is reported, never guessed.
                let text = match fs::read_to_string(&doc) {
                    Ok(text) => text,
                    Err(e) if e.kind() == ErrorKind::NotFound => continue,
                    Err(e) => {
                        let why = format!("{}: the doc cannot be read: {e}", rel.display());
                        rel.hash(&mut hasher);
                        why.hash(&mut hasher);
                        // A doc that is not text still moves the fingerprint when it changes.
                        fs::read(&doc).unwrap_or_default().hash(&mut hasher);
                        roster.problems.push(why);
                        continue;
                    }
                };
                rel.hash(&mut hasher);
                text.hash(&mut hasher);
                match parse(&text, &rel) {
                    Ok(c) => {
                        if RESERVED.contains(&c.name.as_str()) {
                            roster.problems.push(format!(
                                "{}: `{}` is reserved: it names who stands over every body",
                                rel.display(),
                                c.name
                            ));
                        } else if let Some(other) = roster.creatures.get(&c.name) {
                            roster.problems.push(format!(
                                "{}: `{}` is already the creature at {}",
                                rel.display(),
                                c.name,
                                other.seat
                            ));
                        } else {
                            roster.creatures.insert(c.name.clone(), c);
                        }
                    }
                    Err(e) => roster.problems.push(e),
                }
            }
        }
        roster.fingerprint = hasher.finish();
        roster
    }

    /// The living creatures: retired ones own nothing and command no one.
    pub fn living(&self) -> impl Iterator<Item = &Creature> {
        self.creatures.values().filter(|c| !c.retired)
    }

    /// The chain above `name`, nearest first, stopping at the throne, a missing name or a loop.
    #[must_use]
    pub fn chain(&self, name: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut at = self.creatures.get(name).and_then(|c| c.reports_to.clone());
        while let Some(next) = at {
            if next == name || out.contains(&next) {
                break;
            }
            out.push(next.clone());
            at = self.creatures.get(&next).and_then(|c| c.reports_to.clone());
        }
        out
    }

    /// Whether `a` is above `b` in the chain of command (`above` is transitive).
    #[must_use]
    pub fn is_above(&self, a: &str, b: &str) -> bool {
        self.chain(b).iter().any(|n| n == a)
    }

    /// Every creature whose chain comes back to itself: the chain of command loops.
    #[must_use]
    pub fn loops(&self) -> Vec<String> {
        self.creatures
            .keys()
            .filter(|name| {
                let mut seen = vec![(*name).clone()];
                let mut at = self.creatures.get(*name).and_then(|c| c.reports_to.clone());
                while let Some(next) = at {
                    if seen.contains(&next) {
                        return next == **name;
                    }
                    seen.push(next.clone());
                    at = self.creatures.get(&next).and_then(|c| c.reports_to.clone());
                }
                false
            })
            .cloned()
            .collect()
    }
}
