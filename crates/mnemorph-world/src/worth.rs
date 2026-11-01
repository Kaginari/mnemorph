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
    /// Where the verdicts were read from.
    source: Source,
}

/// Where a world's gate verdicts were read from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Source {
    /// The chained verdict journal under the human's state directory, its chain holding.
    #[default]
    Journal,
    /// `log.md`'s gate entries: the world keeps no verdict journal yet.
    Log,
    /// The verdict journal, whose chain does not hold or cannot be read: no record is trusted.
    Unsound(String),
}

impl Records {
    /// Read `world`'s usage ledger and its gate verdicts: from the chained verdict journal kept
    /// outside the world, held to its newest anchor, when there is one; else from `log.md`'s
    /// entries by the gate alone, and [`Records::note`] says so.
    #[must_use]
    pub fn load(world: &Path) -> Records {
        let journal = crate::verdicts::verdicts_of(world).filter(|p| p.exists());
        let anchor = journal
            .is_some()
            .then(|| crate::chains::anchor_of(world, "gate"))
            .flatten();
        Records::load_at(world, journal.as_deref(), anchor.as_ref())
    }

    /// [`Records::load`], the verdicts read from the journal at `verdicts` held to `anchor`, or
    /// from `log.md` with no journal.
    #[must_use]
    pub fn load_at(
        world: &Path,
        verdicts: Option<&Path>,
        anchor: Option<&mnemorph_ledger::Head>,
    ) -> Records {
        let mut records = Records::from_log(world);
        let Some(path) = verdicts else {
            records.source = Source::Log;
            return records;
        };
        records.verdicts = Vec::new();
        let unsound = match mnemorph_ledger::chain::verify_against(path, anchor) {
            Ok(r) if r.holds() => None,
            Ok(r) => Some(r.to_string()),
            Err(e) => Some(format!("unreadable: {e}")),
        };
        if let Some(why) = unsound {
            records.source = Source::Unsound(why);
            return records;
        }
        match crate::verdicts::journal_at(path).read() {
            Ok(back) => {
                records.verdicts = back
                    .rows
                    .into_iter()
                    .map(|r| (r.body, r.verdict, r.changed))
                    .collect();
                records.source = Source::Journal;
            }
            Err(e) => records.source = Source::Unsound(format!("unreadable: {e}")),
        }
        records
    }

    /// Where the verdicts were read from.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Why no body's record can be trusted: the verdict journal does not verify.
    #[must_use]
    pub fn unsound(&self) -> Option<&str> {
        match &self.source {
            Source::Unsound(why) => Some(why),
            _ => None,
        }
    }

    /// A line for whoever reads the cards when the verdicts are not the chained journal's.
    #[must_use]
    pub fn note(&self) -> Option<String> {
        match &self.source {
            Source::Journal => None,
            Source::Log => Some(
                "the gate's verdicts are read from log.md: this world keeps no verdict journal yet"
                    .to_owned(),
            ),
            Source::Unsound(why) => Some(format!(
                "the gate's verdict journal does not verify ({why}): no card is trusted"
            )),
        }
    }

    /// The usage ledger, and the verdicts of `log.md`'s entries by the gate.
    fn from_log(world: &Path) -> Records {
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
        Records {
            spent,
            verdicts,
            source: Source::Log,
        }
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
            // A check that gave no result is no pass: it counts as a failure, so not verifying
            // never buys a body worth — unless it never ran, which is the machine's doing
            // ("NOT_VERIFIED, unrun"), and counts neither way. A pass no verify line ran on
            // ("PASS, unchecked") is never counted as verified: it counts neither way either.
            match verdict.as_str() {
                "PASS" => card.passes += 1,
                "PASS, unchecked" => {}
                "FAIL" | "NOT_VERIFIED" => card.fails += 1,
                v if v.ends_with(", handed back") => card.handed_back += 1,
                _ => {}
            }
        }
        card
    }
}

impl Records {
    /// The files `name` is stuck on: its last three verdicts all failed, and these paths were in
    /// every one of them. Empty when it is not stuck — or when the verdicts are unsound: a caller
    /// asks [`Records::unsound`] first, and trusts no body then.
    #[must_use]
    pub fn stuck(&self, name: &str) -> Vec<String> {
        let last: Vec<&(String, String, Vec<String>)> = self
            .verdicts
            .iter()
            .rev()
            .filter(|(b, v, _)| {
                b == name && !v.ends_with(", handed back") && !v.ends_with(", unrun")
            })
            .take(STUCK_AFTER)
            .collect();
        let failed = |v: &str| v == "FAIL" || v == "NOT_VERIFIED";
        if last.len() < STUCK_AFTER || last.iter().any(|(_, v, _)| !failed(v)) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::{Entry, append_entry};
    use crate::verdicts::{VerdictRow, journal_at};
    use mnemorph_ledger::{Build, Timestamp, chain};

    fn logged(world: &Path, verdict: &str, path: &str) {
        append_entry(
            world,
            &Entry {
                body: "slime-s".into(),
                title: format!("gate: {verdict}"),
                lines: vec![
                    ("Changed".into(), format!("`{path}`")),
                    ("By".into(), "the end gate".into()),
                ],
            },
        )
        .unwrap();
    }

    fn kept(j: &Journal<VerdictRow>, verdict: &str, path: &str) {
        j.append(&VerdictRow {
            ts: Timestamp::now(),
            body: "slime-s".into(),
            verdict: verdict.into(),
            changed: vec![path.into()],
            failures: Vec::new(),
            unverified: Vec::new(),
            unchecked: false,
            verified_by: Vec::new(),
            entry: None,
            build: Build::this(),
            run: None,
        })
        .unwrap();
    }

    /// The chained verdict journal is the source when the world keeps one; `log.md` only when
    /// it keeps none, and the note says so; a journal that does not verify trusts no body.
    #[test]
    fn the_verdict_journal_is_the_source_and_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        for _ in 0..STUCK_AFTER {
            logged(&world, "FAIL", "src/a.rs");
        }
        let old = Records::load_at(&world, None, None);
        assert_eq!(old.source(), &Source::Log);
        assert_eq!(old.stuck("slime-s"), ["src/a.rs"]);
        assert!(old.note().is_some_and(|n| n.contains("read from log.md")));

        // A body that forges passes into log.md moves nothing once the journal is the source.
        let path = dir.path().join("state/gate.jsonl");
        let j = journal_at(&path);
        for _ in 0..STUCK_AFTER {
            kept(&j, "FAIL", "src/b.rs");
            logged(&world, "PASS", "src/b.rs");
        }
        let now = Records::load_at(&world, Some(&path), None);
        assert_eq!(now.source(), &Source::Journal);
        assert_eq!(now.note(), None);
        assert_eq!(now.stuck("slime-s"), ["src/b.rs"]);
        let head = chain::verify(&path).unwrap().head.unwrap();

        // An edit: unsound, reported, no body trusted.
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replacen("FAIL", "PASS", 1)).unwrap();
        let edited = Records::load_at(&world, Some(&path), Some(&head));
        assert!(
            edited
                .unsound()
                .is_some_and(|w| w.contains("broken at line 1"))
        );
        assert!(edited.note().is_some_and(|n| n.contains("does not verify")));

        // A dropped tail, found against the anchor.
        let first: String = text.lines().next().unwrap().to_owned() + "\n";
        std::fs::write(&path, first).unwrap();
        let dropped = Records::load_at(&world, Some(&path), Some(&head));
        assert!(dropped.unsound().is_some(), "{:?}", dropped.source());
    }

    /// A pass no verify line ran on is never counted as verified: it is no pass on the card.
    #[test]
    fn an_unchecked_pass_counts_neither_way() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w");
        std::fs::create_dir_all(world.join(".mnemorph/slime/s")).unwrap();
        std::fs::write(
            world.join(".mnemorph/slime/s/README.md"),
            "# slime-s\n\n- **Race:** slime\n- **Territory:** src/\n",
        )
        .unwrap();
        let path = dir.path().join("state/gate.jsonl");
        let j = journal_at(&path);
        kept(&j, "PASS, unchecked", "src/a.rs");
        kept(&j, "PASS", "src/b.rs");
        let roster = Roster::cards(&world);
        let card = Records::load_at(&world, Some(&path), None).card(&roster, "slime-s");
        assert_eq!((card.passes, card.fails, card.handed_back), (1, 0, 0));
    }
}
