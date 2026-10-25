//! What a body is worth to the one that funds it: its card — its record and the record of every
//! body below it — and the ceiling a grant to it may reach.

use std::path::Path;

use mnemorph_ledger::{Journal, UsageRow};
use mnemorph_races::Race;

use crate::log::LOG;
use crate::roster::Roster;

/// Where the usage ledger lives in a world.
pub const USAGE: &str = ".mnemorph/instruments/usage.jsonl";

/// One body's record, counted with everything below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// The body.
    pub body: String,
    /// Weighted tokens spent by it and the bodies below it.
    pub spent_wt: u64,
    /// Gate passes on a landing.
    pub passes: u32,
    /// Gate failures that ended a run.
    pub fails: u32,
}

impl Card {
    /// Its worth, between 0.5 and 1: `0.5 + 0.5 × (passes + 1) / (passes + fails + 2)`, in
    /// thousandths. No record reads 750.
    #[must_use]
    pub fn worth_milli(&self) -> u64 {
        let (p, f) = (u64::from(self.passes), u64::from(self.fails));
        500 + 500 * (p + 1) / (p + f + 2)
    }

    /// The card as a dispatching body reads it.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "{}: worth 0.{:03} — {} gate passes, {} fails, {} wt spent (with the bodies below it)",
            self.body,
            self.worth_milli().min(999),
            self.passes,
            self.fails,
            self.spent_wt
        )
    }
}

/// `name`'s card in `world`: its own record and every living body's below it.
#[must_use]
pub fn card(world: &Path, roster: &Roster, name: &str) -> Card {
    let below: Vec<&str> = roster
        .living()
        .filter(|c| c.name == name || roster.chain(&c.name).iter().any(|n| n == name))
        .map(|c| c.name.as_str())
        .collect();
    let spent_wt = Journal::<UsageRow>::new(world.join(USAGE))
        .read()
        .map(|r| r.rows)
        .unwrap_or_default()
        .iter()
        .filter(|row| below.contains(&row.body.as_str()))
        .map(|row| row.wt)
        .sum();
    let (mut passes, mut fails) = (0, 0);
    let journal = std::fs::read_to_string(world.join(LOG)).unwrap_or_default();
    for line in journal.lines().filter(|l| l.starts_with("### [")) {
        let Some((_, rest)) = line.split_once("] ") else {
            continue;
        };
        let Some((body, verdict)) = rest.split_once(" — gate: ") else {
            continue;
        };
        if !below.contains(&body) {
            continue;
        }
        // A failure handed back to be fixed is not yet a verdict on the run.
        match verdict.trim() {
            "PASS" => passes += 1,
            "FAIL" => fails += 1,
            _ => {}
        }
    }
    Card {
        body: name.to_owned(),
        spent_wt,
        passes,
        fails,
    }
}

/// A race's compiled maximum share of what its parent has left, in percent.
#[must_use]
pub fn share_pct(race: Race) -> u64 {
    match race.base() {
        Race::Elf | Race::Orc => 60,
        Race::Slime => 70,
        Race::Kijin => 30,
        Race::DarkElf => 40,
        _ => 50,
    }
}

/// The most a grant to a body of `race` and `card` may reach out of `left`: the giver keeps 10%,
/// the race takes at most its share of the rest, scaled by the body's worth.
#[must_use]
pub fn ceiling(left: u64, race: Race, card: &Card) -> u64 {
    let wide = u128::from(left) * 9 * u128::from(share_pct(race)) * u128::from(card.worth_milli())
        / (10 * 100 * 1000);
    u64::try_from(wide).unwrap_or(u64::MAX)
}
