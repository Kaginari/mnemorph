//! Every creature's meters as `status` and `mnemorphd meters` show them: each with its count and
//! its 95% Wilson interval, read from a fold that exists, or `no reports` where none does.
//!
//! Only the Dark Elf's meters are live: they fold from the challenge records, per speciality
//! (`precision`, `upheldRate`, `cannotVerifyRate`). Every other race's meters fold from its
//! reports, and the general report fold over the journal is M11's: until it exists they read
//! `no reports`, never a number. Showing only: every decision reads the point estimate.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use mnemorph_ledger::SpecialityFold;
/// The races and their rates, as the meters' callers name them.
pub use mnemorph_races::{Race, Rate};

use crate::Roster;

/// One meter: its name and its counts; `None` when no fold reads it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// The meter, as the race law names it.
    pub meter: &'static str,
    /// Its successes over its record; `None` when no fold reads it yet.
    pub rate: Option<Rate>,
}

/// A creature's meters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meters {
    /// The creature.
    pub name: String,
    /// Its race.
    pub race: Race,
    /// Its meters, in the race's order.
    pub readings: Vec<Reading>,
}

/// The meters a race is read on, headline first.
#[must_use]
pub fn of_race(race: Race) -> &'static [&'static str] {
    match race.base() {
        Race::Elf => &["planLandRate", "reaskRate", "landRate"],
        Race::Orc => &["gateRight", "holeRate", "landRate"],
        Race::Slime => &["landRate", "gateRate", "holeRate"],
        Race::Kijin => &["outwardCleanRate", "verifyGreenRate", "landRate"],
        Race::DarkElf => &["precision", "upheldRate", "cannotVerifyRate"],
        _ => &["askDoneRate"],
    }
}

/// The Dark Elf's meters over its speciality's fold.
fn dark_elf(f: Option<&SpecialityFold>) -> Vec<Reading> {
    let f = f.cloned().unwrap_or_default();
    vec![
        Reading {
            meter: "precision",
            rate: Some(f.precision_rate()),
        },
        Reading {
            meter: "upheldRate",
            rate: Some(Rate::of(f.upheld, f.findings)),
        },
        Reading {
            meter: "cannotVerifyRate",
            rate: Some(f.cannot_verify_counts()),
        },
    ]
}

/// Every living creature's meters over the challenge records' folds, by speciality: a Dark Elf's
/// from the fold that bears its name, every other race's `no reports`.
#[must_use]
pub fn of(roster: &Roster, folds: &BTreeMap<String, SpecialityFold>) -> Vec<Meters> {
    roster
        .living()
        .map(|c| Meters {
            name: c.name.clone(),
            race: c.race,
            readings: if c.race.base() == Race::DarkElf {
                dark_elf(folds.get(&c.name))
            } else {
                of_race(c.race)
                    .iter()
                    .map(|&meter| Reading { meter, rate: None })
                    .collect()
            },
        })
        .collect()
}

/// [`of`] for `world`: its roster and its challenge records, when it has a home for them.
#[must_use]
pub fn read(world: &Path) -> Vec<Meters> {
    let folds = crate::challenge::scored(world)
        .map(|(f, _)| f)
        .unwrap_or_default();
    of(&Roster::load(world), &folds)
}

impl fmt::Display for Meters {
    /// `glim (dark-elf): precision 0.90 (18/20, 95% 0.70–0.97) · …`, or
    /// `sly (slime): no reports — landRate, gateRate, holeRate`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}): ", self.name, self.race.name())?;
        // A fold that reads none of its meters yet says so once, naming them.
        if self.readings.iter().all(|r| r.rate.is_none()) {
            let names: Vec<&str> = self.readings.iter().map(|r| r.meter).collect();
            return write!(f, "no reports — {}", names.join(", "));
        }
        let shown: Vec<String> = self
            .readings
            .iter()
            .map(|r| match r.rate {
                Some(rate) => format!("{} {rate}", r.meter),
                None => format!("{} no reports", r.meter),
            })
            .collect();
        f.write_str(&shown.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster(docs: &[(&str, &str)]) -> (tempfile::TempDir, Roster) {
        let dir = tempfile::tempdir().unwrap();
        for (seat, doc) in docs {
            let at = dir.path().join(".mnemorph").join(seat);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("README.md"), doc).unwrap();
        }
        let r = Roster::load(dir.path());
        (dir, r)
    }

    #[test]
    fn a_live_meter_shows_its_count_and_interval_and_the_rest_say_no_reports() {
        let (_d, r) = roster(&[
            (
                "dark-elf/glim",
                "# glim\n\n- **Race:** dark-elf\n- **Speciality:** law\n",
            ),
            ("slime/sly", "# sly\n\n- **Race:** slime\n"),
        ]);
        let mut folds = BTreeMap::new();
        folds.insert(
            "glim".to_owned(),
            SpecialityFold {
                findings: 20,
                upheld: 18,
                refuted: 2,
                ..SpecialityFold::default()
            },
        );
        let shown: Vec<String> = of(&r, &folds).iter().map(ToString::to_string).collect();
        assert!(
            shown.contains(
                &"glim (dark-elf): precision 0.90 (18/20, 95% 0.70–0.97) · upheldRate 0.90 \
                  (18/20, 95% 0.70–0.97) · cannotVerifyRate no reports"
                    .to_owned()
            ),
            "{shown:?}"
        );
        assert!(
            shown.contains(&"sly (slime): no reports — landRate, gateRate, holeRate".to_owned()),
            "{shown:?}"
        );
    }
}
