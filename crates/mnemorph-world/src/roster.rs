//! The roster: every creature the world's docs describe, read fresh when the docs change, and
//! held to the creatures' witness ([`crate::creatures`]). [`Roster::load`] is the trusted view
//! every privilege reads; [`Roster::cards`] is the cards as they stand, only ever shown.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::ErrorKind;
use std::path::Path;

use mnemorph_races::{Race, Speciality};

use crate::creatures::{Mismatch, Standing};
use crate::doc::{Creature, covers, parse};

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
    /// How each creature stands against the witness; empty for the cards alone.
    pub standing: BTreeMap<String, Standing>,
    /// Each field a card says otherwise than its row.
    pub mismatches: Vec<Mismatch>,
    /// Why the witness trusts no row, when it does not: no home, or a chain that does not walk.
    pub witness: Option<String>,
}

/// Names no creature may take: the human and the harness stand over every body.
const RESERVED: &[&str] = &[crate::queue::HUMAN, crate::queue::HARNESS];

/// Why a doc's name and race disagree on a Kijin post, if they do: the post is read from the
/// name (`kijin-forge`), so only a Kijin bears a `kijin-` name, and a Kijin's is `kijin-` and its
/// directory.
fn misnamed_kijin(c: &Creature, seat: &Path) -> Option<String> {
    let kijin = c.race.base() == Race::Kijin;
    if !kijin {
        return c.name.starts_with("kijin-").then(|| {
            format!(
                "`{}` is a kijin's name, but the doc's race is {}",
                c.name, c.race
            )
        });
    }
    let dir = seat.file_name()?.to_string_lossy();
    let want = format!("kijin-{dir}");
    (c.name != want).then(|| {
        format!(
            "a kijin's heading is `kijin-` and its directory: `{want}`, not `{}`",
            c.name
        )
    })
}

/// Why a doc's name and race disagree on a Dark Elf speciality, if they do: the speciality is
/// read from the name (`dark-elf-supply`), so only a Dark Elf, ascended or not, bears a
/// `dark-elf` name.
fn misnamed_dark_elf(c: &Creature) -> Option<String> {
    let named = c.name == "dark-elf" || c.name.starts_with("dark-elf-");
    (named && c.race.base() != Race::DarkElf).then(|| {
        format!(
            "`{}` is a dark elf's name, but the doc's race is {}",
            c.name, c.race
        )
    })
}

impl Roster {
    /// The trusted view of `world`'s creatures: each card held to the witness's row under the
    /// human's state directory ([`crate::creatures::trusted`]). Every privilege reads this.
    #[must_use]
    pub fn load(world: &Path) -> Roster {
        Roster::load_in(crate::queue::state_dir().as_deref(), world)
    }

    /// [`Roster::load`], the human's state directory at `state` (`None`: no home).
    #[must_use]
    pub fn load_in(state: Option<&Path>, world: &Path) -> Roster {
        crate::creatures::trusted(
            Roster::cards(world),
            &crate::creatures::witnessed_in(state, world),
        )
    }

    /// The trusted view of the cards copied at `copy` (a reader's snapshot of `world`), held to
    /// `world`'s own witness: the witness names the world by its real path, never the copy's.
    #[must_use]
    pub fn copied(copy: &Path, world: &Path) -> Roster {
        crate::creatures::trusted(Roster::cards(copy), &crate::creatures::witnessed(world))
    }

    /// The cards copied at `copy` (or `world` itself) as they stand, with the witness's word on
    /// each — its standing, the fields that differ, why no row is trusted — for what checks the
    /// docs' shape and shows it (`onto check`, the board's ontology): the shape is the cards',
    /// the `roster-mismatch` findings the witness's. Never a privilege's.
    #[must_use]
    pub fn shown(copy: &Path, world: &Path) -> Roster {
        let cards = Roster::cards(copy);
        let held = crate::creatures::trusted(cards.clone(), &crate::creatures::witnessed(world));
        Roster {
            standing: held
                .standing
                .into_iter()
                .filter(|(_, s)| *s != Standing::Stranger)
                .collect(),
            mismatches: held.mismatches,
            witness: held.witness,
            ..cards
        }
    }

    /// Whether `name` stands as a stranger: no row holds its card.
    #[must_use]
    pub fn stranger(&self, name: &str) -> bool {
        matches!(self.standing.get(name), Some(Standing::Stranger))
    }

    /// Read every `.mnemorph/<race>/<name>/README.md` of `world`, as the cards stand: a body can
    /// write a card, so this is only ever shown — every privilege reads [`Roster::load`].
    #[must_use]
    pub fn cards(world: &Path) -> Roster {
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
                        if let Some(why) =
                            misnamed_kijin(&c, &seat).or_else(|| misnamed_dark_elf(&c))
                        {
                            roster.problems.push(format!("{}: {why}", rel.display()));
                        } else if RESERVED.contains(&c.name.as_str()) {
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

    /// The Dark Elf specialities a landing's written paths are routed to: the compiled surfaces,
    /// and the Surfaces each living speciality's doc adds ([`mnemorph_races::route_with`]). Each
    /// path is normalised as ownership reads it (`./`, `//`, `..`, `\\`); one that does not
    /// normalise is matched as written.
    #[must_use]
    pub fn route<P: AsRef<str>>(&self, paths: &[P]) -> Vec<Speciality> {
        let paths: Vec<String> = paths
            .iter()
            .map(|p| crate::own::normal(p.as_ref()).unwrap_or_else(|| p.as_ref().to_owned()))
            .collect();
        mnemorph_races::route_with(&paths, |s, path| {
            self.living()
                .filter(|c| c.race.base() == Race::DarkElf)
                .filter(|c| Speciality::of(&c.name) == Some(s))
                .any(|c| c.surfaces.iter().any(|claim| covers(claim, path)))
        })
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
