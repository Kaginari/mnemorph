//! Envelopes: the budget a body holds, carved down the chain, spent, and returned.
//!
//! Only cost is carved — in weighted tokens. Steps and the clock pass down whole, per race
//! (`Race::steps`), and never appear here.

use std::fmt;

use tempest_races::Race;

/// The share of what a parent has left that it keeps for itself when it carves.
pub const PARENT_RESERVE_PCT: u64 = 10;
/// The share of a session envelope held for the automatic Dark Elf challenges.
pub const CHALLENGE_RESERVE_PCT: u64 = 10;
/// How many full-size challenges the reserve funds: each one takes this fraction of it.
const CHALLENGES_PER_RESERVE: u64 = 4;

/// One item of a parent's plan: the race that will take it, and its declared weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    /// The race of the body the item goes to.
    pub race: Race,
    /// Its weight among the plan's items (equal weights split evenly; 0 gets nothing).
    pub weight: u32,
}

/// Where a slice came from, so its unspent part goes back to the same place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Plan,
    Challenge,
}

/// A slice carved for one child: the size of the child's envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a slice must be settled, or its budget stays out"]
pub struct Slice {
    /// The child's race.
    pub race: Race,
    /// Weighted tokens the child may spend.
    pub of: u64,
    source: Source,
}

/// The next call does not fit: `@? budget: need ~<need> wt, have <have> wt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetShort {
    /// Weighted tokens the call is estimated to need.
    pub need: u64,
    /// Weighted tokens left.
    pub have: u64,
}

impl fmt::Display for BudgetShort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "budget: need ~{} wt, have {} wt", self.need, self.have)
    }
}

/// A body's budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    of: u64,
    spent: u64,
    out: u64,
    challenge: u64,
}

impl Envelope {
    /// The session's envelope: the whole budget, with the challenge reserve held back.
    #[must_use]
    pub fn session(of: u64) -> Envelope {
        Envelope {
            of,
            spent: 0,
            out: 0,
            challenge: pct(of, CHALLENGE_RESERVE_PCT),
        }
    }

    /// A child's envelope, from the slice its parent carved.
    #[must_use]
    pub fn child(slice: Slice) -> Envelope {
        Envelope {
            of: slice.of,
            spent: 0,
            out: 0,
            challenge: 0,
        }
    }

    /// The envelope's size.
    #[must_use]
    pub fn of(&self) -> u64 {
        self.of
    }

    /// Weighted tokens spent, by this body and every child it settled.
    #[must_use]
    pub fn spent(&self) -> u64 {
        self.spent
    }

    /// Weighted tokens this body may still spend or carve.
    #[must_use]
    pub fn left(&self) -> u64 {
        self.of
            .saturating_sub(self.spent + self.out + self.challenge)
    }

    /// Whether real spend went past the envelope (an estimate can be wrong; spend is never
    /// refused after the fact).
    #[must_use]
    pub fn overspent(&self) -> bool {
        self.spent + self.out + self.challenge > self.of
    }

    /// Checks a call's estimate before it is made.
    ///
    /// # Errors
    ///
    /// The estimate does not fit what is left.
    pub fn preflight(&self, estimate: u64) -> Result<(), BudgetShort> {
        let have = self.left();
        if estimate <= have {
            Ok(())
        } else {
            Err(BudgetShort {
                need: estimate,
                have,
            })
        }
    }

    /// Records what a call really cost.
    pub fn record(&mut self, wt: u64) {
        self.spent += wt;
    }

    /// Carves slices for a plan's items. The parent keeps [`PARENT_RESERVE_PCT`] of what it has
    /// left; the rest is split by weight, and no slice exceeds its race's maximum share of what
    /// the parent had left. The slices together never exceed what the parent holds.
    pub fn carve(&mut self, items: &[Item]) -> Vec<Slice> {
        let left = self.left();
        let available = left - pct(left, PARENT_RESERVE_PCT);
        let total: u64 = items.iter().map(|i| u64::from(i.weight)).sum();
        let slices: Vec<Slice> = items
            .iter()
            .map(|i| {
                let by_weight = if total == 0 {
                    0
                } else {
                    u64::try_from(u128::from(available) * u128::from(i.weight) / u128::from(total))
                        .unwrap_or(u64::MAX)
                };
                let ceiling = pct(left, u64::from(i.race.max_share()));
                Slice {
                    race: i.race,
                    of: by_weight.min(ceiling),
                    source: Source::Plan,
                }
            })
            .collect();
        self.out += slices.iter().map(|s| s.of).sum::<u64>();
        slices
    }

    /// Carves a slice for an automatic Dark Elf challenge from the session's reserve: a quarter
    /// of the reserve's full size, or what remains of it. `None` once the reserve is spent.
    pub fn challenge(&mut self) -> Option<Slice> {
        let size = pct(self.of, CHALLENGE_RESERVE_PCT) / CHALLENGES_PER_RESERVE;
        let of = size.max(1).min(self.challenge);
        if of == 0 {
            return None;
        }
        self.challenge -= of;
        self.out += of;
        Some(Slice {
            race: Race::DarkElf,
            of,
            source: Source::Challenge,
        })
    }

    /// Settles a child's slice when it reports: what the child spent counts as spent here, and
    /// what it did not spend comes back — a challenge slice's to the reserve.
    pub fn settle(&mut self, slice: Slice, child_spent: u64) {
        self.out -= slice.of;
        self.spent += child_spent;
        if slice.source == Source::Challenge {
            self.challenge += slice.of.saturating_sub(child_spent);
        }
    }
}

/// `pct`% of `n`, rounded down.
fn pct(n: u64, pct: u64) -> u64 {
    u64::try_from(u128::from(n) * u128::from(pct) / 100).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_holds_the_challenge_reserve() {
        let s = Envelope::session(100_000);
        assert_eq!((s.of(), s.left(), s.spent()), (100_000, 90_000, 0));
        assert_eq!(s.preflight(90_000), Ok(()));
        assert_eq!(
            s.preflight(90_001),
            Err(BudgetShort {
                need: 90_001,
                have: 90_000
            })
        );
        assert_eq!(
            BudgetShort { need: 5, have: 3 }.to_string(),
            "budget: need ~5 wt, have 3 wt"
        );
    }

    #[test]
    fn carving_keeps_a_reserve_and_honours_ceilings() {
        let mut e = Envelope::session(100_000);
        let slices = e.carve(&[
            Item {
                race: Race::Elf,
                weight: 1,
            },
            Item {
                race: Race::Elf,
                weight: 1,
            },
        ]);
        // 90 000 left, 81 000 to carve, 40 500 each; the Elf ceiling is 60% of 90 000.
        assert_eq!(
            slices.iter().map(|s| s.of).collect::<Vec<_>>(),
            [40_500, 40_500]
        );
        assert_eq!(e.left(), 9_000);
        // One heavy Kijin item is held to its 30% ceiling.
        let mut e = Envelope::session(100_000);
        let s = e.carve(&[Item {
            race: Race::Oni,
            weight: 5,
        }]);
        assert_eq!(s[0].of, 27_000);
        assert_eq!(
            e.carve(&[Item {
                race: Race::Slime,
                weight: 0
            }])[0]
                .of,
            0
        );
        assert_eq!(e.carve(&[]), []);
    }

    #[test]
    fn settling_returns_what_was_not_spent() {
        let mut e = Envelope::session(100_000);
        let s = e
            .carve(&[Item {
                race: Race::Orc,
                weight: 1,
            }])
            .remove(0);
        let mut child = Envelope::child(s);
        child.record(10_000);
        e.settle(s, child.spent());
        assert_eq!(e.spent(), 10_000);
        assert_eq!(e.left(), 80_000);
        e.record(85_000);
        assert!(
            e.overspent() && e.left() == 0,
            "real spend is recorded even past the line"
        );
    }

    #[test]
    fn challenges_draw_on_their_own_reserve() {
        let mut e = Envelope::session(100_000);
        let before = e.left();
        let slices: Vec<_> = std::iter::from_fn(|| e.challenge()).collect();
        assert_eq!(slices.len(), 4);
        assert!(
            slices
                .iter()
                .all(|s| s.of == 2_500 && s.race == Race::DarkElf)
        );
        assert_eq!(
            e.left(),
            before,
            "a challenge never takes from the work's budget"
        );
        e.settle(slices[0], 500);
        let again = e
            .challenge()
            .expect("unspent challenge budget returns to the reserve");
        assert_eq!(again.of, 2_000);
        assert_eq!(Envelope::session(0).challenge(), None);
    }
}
