//! What a body is worth to the one that funds it: its card — its record and the record of every
//! body below it — and the ceiling a grant to it may reach.

use std::path::Path;

use mnemorph_ledger::{Journal, UsageRow};
use mnemorph_races::Race;

use crate::log::LOG;
use crate::roster::Roster;

/// Where the usage ledger lives in a world.
pub const USAGE: &str = ".mnemorph/instruments/usage.jsonl";

/// What the gate writes under each verdict it journals: only its own entries count on a card.
pub const BY_THE_GATE: &str = "- **By:** the end gate";

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
    /// Gate failures handed back to be fixed: half a failure each.
    pub handed_back: u32,
}

impl Card {
    /// Its worth, between 0.5 and 1, in thousandths: `0.5 + 0.5 × (passes + 1) / (passes +
    /// fails + handed_back / 2 + 2)`. No record reads 750.
    #[must_use]
    pub fn worth_milli(&self) -> u64 {
        let (p, f, h) = (
            u64::from(self.passes),
            u64::from(self.fails),
            u64::from(self.handed_back),
        );
        500 + 500 * (2 * p + 2) / (2 * p + 2 * f + h + 4)
    }

    /// The card as a dispatching body reads it; spend is rounded to the thousand, so a card
    /// changes only when the record does.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "{}: worth 0.{:03} — {} gate passes, {} fails, {} handed back, ~{}k wt spent (with the bodies below it)",
            self.body,
            self.worth_milli(),
            self.passes,
            self.fails,
            self.handed_back,
            self.spent_wt.div_ceil(1000)
        )
    }
}

/// A world's records, read once: who spent what, and the gate's verdicts.
#[derive(Debug, Default)]
pub struct Records {
    spent: Vec<(String, u64)>,
    /// The gate's verdicts in order: the body, the verdict, the paths it changed.
    verdicts: Vec<(String, String, Vec<String>)>,
}

impl Records {
    /// Read `world`'s usage ledger and journal. Only the gate's own journal entries count.
    #[must_use]
    pub fn load(world: &Path) -> Records {
        let spent = Journal::<UsageRow>::new(world.join(USAGE))
            .read()
            .map(|r| r.rows)
            .unwrap_or_default()
            .into_iter()
            .map(|row| (row.body, row.wt))
            .collect();
        let journal = std::fs::read_to_string(world.join(LOG)).unwrap_or_default();
        let mut verdicts = Vec::new();
        for entry in journal.split("\n---\n") {
            let mut lines = entry.lines().skip_while(|l| !l.starts_with("### ["));
            let Some(head) = lines.next() else {
                continue;
            };
            let rest: Vec<&str> = lines.collect();
            if !rest.contains(&BY_THE_GATE) {
                continue;
            }
            let changed: Vec<String> = rest
                .iter()
                .find_map(|l| l.strip_prefix("- **Changed:** "))
                .and_then(|c| c.strip_prefix('`').and_then(|c| c.strip_suffix('`')))
                // Split on the backticks' own boundaries: a path may hold ", " itself.
                .map(|c| c.split("`, `").map(str::to_owned).collect())
                .unwrap_or_default();
            let Some((_, rest)) = head.split_once("] ") else {
                continue;
            };
            if let Some((body, verdict)) = rest.split_once(" — gate: ") {
                verdicts.push((body.to_owned(), verdict.trim().to_owned(), changed));
            }
        }
        Records { spent, verdicts }
    }

    /// `name`'s card: its own record and every living body's below it.
    #[must_use]
    pub fn card(&self, roster: &Roster, name: &str) -> Card {
        let below: Vec<&str> = roster
            .living()
            .filter(|c| c.name == name || roster.chain(&c.name).iter().any(|n| n == name))
            .map(|c| c.name.as_str())
            .collect();
        let mine = |body: &str| below.contains(&body);
        let spent_wt = self
            .spent
            .iter()
            .filter(|(b, _)| mine(b))
            .map(|(_, wt)| wt)
            .sum();
        let mut card = Card {
            body: name.to_owned(),
            spent_wt,
            passes: 0,
            fails: 0,
            handed_back: 0,
        };
        for (_, verdict, _) in self.verdicts.iter().filter(|(b, _, _)| mine(b)) {
            match verdict.as_str() {
                "PASS" => card.passes += 1,
                "FAIL" => card.fails += 1,
                "FAIL, handed back" => card.handed_back += 1,
                _ => {}
            }
        }
        card
    }
}

impl Records {
    /// The files `name` is stuck on: its last three verdicts all failed, and these paths were in
    /// every one of them. Empty when it is not stuck.
    #[must_use]
    pub fn stuck(&self, name: &str) -> Vec<String> {
        let last: Vec<&(String, String, Vec<String>)> = self
            .verdicts
            .iter()
            .rev()
            .filter(|(b, v, _)| b == name && v != "FAIL, handed back")
            .take(STUCK_AFTER)
            .collect();
        if last.len() < STUCK_AFTER || last.iter().any(|(_, v, _)| v != "FAIL") {
            return Vec::new();
        }
        let mut shared = last[0].2.clone();
        shared.retain(|p| last.iter().all(|(_, _, c)| c.contains(p)));
        shared
    }
}

/// Whether an order's text names `path`: a word of it, its quotes and punctuation trimmed, a
/// `:line` suffix and a leading `./` dropped, is the path itself.
#[must_use]
pub fn names(text: &str, path: &str) -> bool {
    text.split_whitespace().any(|word| {
        // Quotes and brackets go at both ends, punctuation only at the end: `./src` keeps its dot.
        let mut w = word
            .trim_start_matches(|c: char| "`'\"([".contains(c))
            .trim_end_matches(|c: char| "`'\",.;)]".contains(c));
        while let Some((head, tail)) = w.rsplit_once(':') {
            if tail.is_empty() || tail.bytes().all(|b| b.is_ascii_digit()) {
                w = head;
            } else {
                break;
            }
        }
        w.strip_prefix("./").unwrap_or(w) == path
    })
}

/// How many failed runs in a row on the same files make a body stuck on them.
pub const STUCK_AFTER: usize = 3;

/// `name`'s card in `world`, read fresh.
#[must_use]
pub fn card(world: &Path, roster: &Roster, name: &str) -> Card {
    Records::load(world).card(roster, name)
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
