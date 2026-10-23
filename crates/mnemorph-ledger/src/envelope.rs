//! Envelopes: the budget a body holds, carved down the chain, spent, and returned.
//!
//! Only cost is carved — in weighted tokens. Steps and the clock pass down whole, per race
//! (`Race::steps`), and never appear here.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mnemorph_races::Race;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Source {
    Plan,
    Challenge,
}

/// A slice carved for one child: the size of the child's envelope. It can be settled once, by
/// the envelope that carved it — it is neither `Copy` nor `Clone`, and it carries its issuer.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[must_use = "a slice must be settled, or its budget stays out"]
pub struct Slice {
    /// The child's race, by name.
    race: String,
    /// Weighted tokens the child may spend.
    of: u64,
    source: Source,
    issuer: u64,
    number: u64,
}

impl Slice {
    /// Weighted tokens the child may spend.
    #[must_use]
    pub fn of(&self) -> u64 {
        self.of
    }

    /// The child's race.
    #[must_use]
    pub fn race(&self) -> Race {
        Race::from_name(&self.race).unwrap_or(Race::Slime)
    }
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

/// A slice refused at settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleError {
    /// Another envelope carved it.
    NotMine,
    /// It was already settled.
    AlreadySettled,
}

impl fmt::Display for SettleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SettleError::NotMine => "this slice was carved by another envelope",
            SettleError::AlreadySettled => "this slice was already settled",
        })
    }
}

impl std::error::Error for SettleError {}

/// A body's budget. It serialises whole, open slices included, so a checkpointed run resumes
/// with the same books.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    id: u64,
    of: u64,
    spent: u64,
    out: u64,
    challenge: u64,
    next_slice: u64,
    open: BTreeSet<u64>,
}

/// A process-unique, restart-resistant envelope id: the clock's nanoseconds mixed with a counter.
fn new_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    #[allow(clippy::cast_possible_truncation)]
    let nanos = nanos as u64;
    nanos
        ^ COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

impl Envelope {
    fn with(of: u64, challenge: u64) -> Envelope {
        Envelope {
            id: new_id(),
            of,
            spent: 0,
            out: 0,
            challenge,
            next_slice: 0,
            open: BTreeSet::new(),
        }
    }

    /// The session's envelope: the whole budget, with the challenge reserve held back.
    #[must_use]
    pub fn session(of: u64) -> Envelope {
        Envelope::with(of, pct(of, CHALLENGE_RESERVE_PCT))
    }

    /// A child's envelope, from the slice its parent carved. The parent keeps the slice to
    /// settle when the child reports.
    #[must_use]
    pub fn child(slice: &Slice) -> Envelope {
        Envelope::with(slice.of, 0)
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

    fn committed(&self) -> u64 {
        self.spent
            .saturating_add(self.out)
            .saturating_add(self.challenge)
    }

    /// Weighted tokens this body may still spend or carve.
    #[must_use]
    pub fn left(&self) -> u64 {
        self.of.saturating_sub(self.committed())
    }

    /// Whether real spend went past the envelope (an estimate can be wrong; spend is never
    /// refused after the fact).
    #[must_use]
    pub fn overspent(&self) -> bool {
        self.committed() > self.of
    }

    /// Checks a call's estimate (see [`crate::estimate`]) before it is made.
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
        self.spent = self.spent.saturating_add(wt);
    }

    fn issue(&mut self, race: Race, of: u64, source: Source) -> Slice {
        let number = self.next_slice;
        self.next_slice += 1;
        self.open.insert(number);
        self.out = self.out.saturating_add(of);
        Slice {
            race: race.name().to_owned(),
            of,
            source,
            issuer: self.id,
            number,
        }
    }

    /// Carves slices for a plan's items. The parent keeps [`PARENT_RESERVE_PCT`] of what it has
    /// left; the rest is split by weight, and no slice exceeds its race's maximum share of what
    /// the parent has left when it carves. The slices together never exceed what it holds.
    pub fn carve(&mut self, items: &[Item]) -> Vec<Slice> {
        let left = self.left();
        let available = left - pct(left, PARENT_RESERVE_PCT);
        let total: u64 = items.iter().map(|i| u64::from(i.weight)).sum();
        items
            .iter()
            .map(|i| {
                let by_weight = if total == 0 {
                    0
                } else {
                    u64::try_from(u128::from(available) * u128::from(i.weight) / u128::from(total))
                        .unwrap_or(u64::MAX)
                };
                let of = by_weight.min(pct(left, u64::from(i.race.max_share())));
                self.issue(i.race, of, Source::Plan)
            })
            .collect()
    }

    /// Carves a slice for an automatic Dark Elf challenge from the session's reserve: a quarter
    /// of the reserve's full size, or what remains of it. `None` once the reserve is spent, and
    /// always on a child's envelope, which holds no reserve.
    pub fn challenge(&mut self) -> Option<Slice> {
        let size = pct(self.of, CHALLENGE_RESERVE_PCT) / CHALLENGES_PER_RESERVE;
        let of = size.max(1).min(self.challenge);
        if of == 0 {
            return None;
        }
        self.challenge -= of;
        Some(self.issue(Race::DarkElf, of, Source::Challenge))
    }

    /// Grows an open slice by `extra` from what this envelope has left — the parent's side of an
    /// `@INSTRUCT` with a new budget. The child's side is [`Envelope::grant`].
    ///
    /// # Errors
    ///
    /// `extra` does not fit what is left, or the slice is not this envelope's open slice.
    pub fn top_up(&mut self, slice: &mut Slice, extra: u64) -> Result<(), TopUpError> {
        self.check(slice).map_err(TopUpError::Slice)?;
        self.preflight(extra).map_err(TopUpError::Budget)?;
        slice.of += extra;
        self.out += extra;
        Ok(())
    }

    /// Grows this envelope by `extra` — the child's side of a top-up.
    pub fn grant(&mut self, extra: u64) {
        self.of = self.of.saturating_add(extra);
    }

    fn check(&self, slice: &Slice) -> Result<(), SettleError> {
        if slice.issuer != self.id {
            Err(SettleError::NotMine)
        } else if !self.open.contains(&slice.number) {
            Err(SettleError::AlreadySettled)
        } else {
            Ok(())
        }
    }

    /// Settles a child's slice when it reports: what the child spent counts as spent here, and
    /// what it did not spend comes back — a challenge slice's to the reserve.
    ///
    /// # Errors
    ///
    /// The slice was carved by another envelope, or was already settled; the books are
    /// untouched.
    // By value on purpose: a settled slice leaves the caller's hands.
    #[allow(clippy::needless_pass_by_value)]
    pub fn settle(&mut self, slice: Slice, child_spent: u64) -> Result<(), SettleError> {
        self.check(&slice)?;
        self.open.remove(&slice.number);
        self.out = self.out.saturating_sub(slice.of);
        self.spent = self.spent.saturating_add(child_spent);
        if slice.source == Source::Challenge {
            self.challenge += slice.of.saturating_sub(child_spent);
        }
        Ok(())
    }
}

/// A top-up refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopUpError {
    /// The slice is not this envelope's to grow.
    Slice(SettleError),
    /// The parent cannot spare it.
    Budget(BudgetShort),
}

impl fmt::Display for TopUpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TopUpError::Slice(e) => e.fmt(f),
            TopUpError::Budget(b) => b.fmt(f),
        }
    }
}

impl std::error::Error for TopUpError {}

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
            slices.iter().map(Slice::of).collect::<Vec<_>>(),
            [40_500, 40_500]
        );
        assert_eq!(slices[0].race(), Race::Elf);
        assert_eq!(e.left(), 9_000);
        let mut e = Envelope::session(100_000);
        assert_eq!(
            e.carve(&[Item {
                race: Race::Oni,
                weight: 5
            }])[0]
                .of(),
            27_000,
            "Kijin ceiling 30%"
        );
        assert_eq!(
            e.carve(&[Item {
                race: Race::Slime,
                weight: 0
            }])[0]
                .of(),
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
        let mut child = Envelope::child(&s);
        child.record(10_000);
        e.settle(s, child.spent()).unwrap();
        assert_eq!((e.spent(), e.left()), (10_000, 80_000));
        e.record(85_000);
        assert!(
            e.overspent() && e.left() == 0,
            "real spend is recorded even past the line"
        );
        e.record(u64::MAX);
        assert_eq!(e.spent(), u64::MAX, "saturates, never wraps");
    }

    #[test]
    fn a_slice_settles_once_and_only_at_home() {
        let mut a = Envelope::session(100_000);
        let mut b = Envelope::session(100_000);
        let from_b = b
            .carve(&[Item {
                race: Race::Elf,
                weight: 1,
            }])
            .remove(0);
        assert_eq!(a.settle(from_b, 1), Err(SettleError::NotMine));
        let s = a
            .carve(&[Item {
                race: Race::Elf,
                weight: 1,
            }])
            .remove(0);
        // A duplicate can only come back through a copy of the books, e.g. a replayed report.
        let replay: Slice = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
        let before = a.clone();
        a.settle(s, 100).unwrap();
        let after = a.clone();
        assert_eq!(a.settle(replay, 100), Err(SettleError::AlreadySettled));
        assert_eq!(a, after, "a refused settlement leaves the books alone");
        assert_ne!(before, after);
        assert_eq!(
            SettleError::NotMine.to_string(),
            "this slice was carved by another envelope"
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
                .all(|s| s.of() == 2_500 && s.race() == Race::DarkElf)
        );
        assert_eq!(
            e.left(),
            before,
            "a challenge never takes from the work's budget"
        );
        let mut slices = slices.into_iter();
        e.settle(slices.next().unwrap(), 500).unwrap();
        assert_eq!(
            e.challenge().map(|s| s.of()),
            Some(2_000),
            "unspent returns to the reserve"
        );
        e.settle(slices.next().unwrap(), 9_999).unwrap();
        assert_eq!(
            e.challenge(),
            None,
            "an overspent challenge returns nothing"
        );
        assert_eq!(Envelope::session(0).challenge(), None);
        let mut parent = Envelope::session(10_000);
        let s = parent
            .carve(&[Item {
                race: Race::Elf,
                weight: 1,
            }])
            .remove(0);
        assert_eq!(
            Envelope::child(&s).challenge(),
            None,
            "a child holds no reserve"
        );
    }

    #[test]
    fn a_top_up_grows_both_sides() {
        let mut p = Envelope::session(100_000);
        let mut s = p
            .carve(&[Item {
                race: Race::Orc,
                weight: 1,
            }])
            .remove(0);
        let mut c = Envelope::child(&s);
        let left = p.left();
        p.top_up(&mut s, 1_000).unwrap();
        c.grant(1_000);
        assert_eq!((p.left(), s.of(), c.of()), (left - 1_000, 55_000, 55_000));
        assert_eq!(
            p.top_up(&mut s, u64::MAX),
            Err(TopUpError::Budget(BudgetShort {
                need: u64::MAX,
                have: left - 1_000
            }))
        );
        let mut other = Envelope::session(1_000);
        assert_eq!(
            other.top_up(&mut s, 1),
            Err(TopUpError::Slice(SettleError::NotMine))
        );
        assert_ne!(
            TopUpError::Slice(SettleError::AlreadySettled).to_string(),
            ""
        );
        p.settle(s, 0).unwrap();
        assert_eq!(p.left(), 90_000);
    }

    #[test]
    fn the_books_survive_a_checkpoint() {
        let mut e = Envelope::session(50_000);
        let s = e
            .carve(&[Item {
                race: Race::Kijin,
                weight: 1,
            }])
            .remove(0);
        let _c = e.challenge().unwrap();
        e.record(300);
        let saved = serde_json::to_string(&(&e, &s)).unwrap();
        let (mut back, slice): (Envelope, Slice) = serde_json::from_str(&saved).unwrap();
        assert_eq!(back, e);
        back.settle(slice, 10).unwrap();
        assert_eq!(back.spent(), 310);
    }
}
