//! Every creature's meters as `status` and `mnemorphd meters` show them: each with its count and
//! its 95% Wilson interval, `no reports` where its record holds none, or `unavailable` where a
//! journal it folds from cannot be trusted.
//!
//! The Dark Elf's meters fold from the challenge records, per speciality (`precision`,
//! `upheldRate`, `cannotVerifyRate`). Every other race's fold from its reports
//! ([`crate::reports`]): the usage witness, the gate verdicts, the challenge records and the Kijin
//! posts' witnesses, under the human's state directory. A journal that does not hold stops that
//! fold, and every meter it feeds reads unavailable — never stale, never zero. Showing only:
//! every decision reads the point estimate, and none is made here.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use mnemorph_ledger::SpecialityFold;
/// The races and their rates, as the meters' callers name them.
pub use mnemorph_races::{Race, Rate};

use crate::Roster;
use crate::reports::{Folded, Unavailable};

/// One meter: its name and its counts; `None` when the fold that reads it is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// The meter, as the race law names it.
    pub meter: &'static str,
    /// Its successes over its record — none of none reads `no reports` — or `None` when its fold
    /// is unavailable.
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
    /// Why its fold gave no reading, when it gave none.
    pub unavailable: Option<String>,
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

/// Every living creature's meters: a Dark Elf's from the challenge records' fold that bears its
/// name, every other race's from the report fold `reports`, by its name and its base race.
#[must_use]
pub fn of(
    roster: &Roster,
    folds: &BTreeMap<String, SpecialityFold>,
    reports: &Result<Folded, Unavailable>,
) -> Vec<Meters> {
    roster
        .living()
        .map(|c| {
            if c.race.base() == Race::DarkElf {
                return Meters {
                    name: c.name.clone(),
                    race: c.race,
                    readings: dark_elf(folds.get(&c.name)),
                    unavailable: None,
                };
            }
            let (inputs, unavailable) = match reports {
                Ok(f) => (Some(f.of(&c.name, c.race)), None),
                Err(e) => (None, Some(e.to_string())),
            };
            Meters {
                name: c.name.clone(),
                race: c.race,
                readings: of_race(c.race)
                    .iter()
                    .map(|&meter| Reading {
                        meter,
                        rate: inputs.as_ref().map(|i| i.rate(meter).unwrap_or_default()),
                    })
                    .collect(),
                unavailable,
            }
        })
        .collect()
}

/// [`of`] for `world`: its roster, its challenge records and its report fold, read and never
/// written.
#[must_use]
pub fn read(world: &Path) -> Vec<Meters> {
    let folds = crate::challenge::scored(world)
        .map(|(f, _)| f)
        .unwrap_or_default();
    of(&Roster::load(world), &folds, &crate::reports::fold(world))
}

impl fmt::Display for Meters {
    /// `glim (dark-elf): precision 0.90 (18/20, 95% 0.70–0.97) · …`,
    /// `sly (slime): no reports — landRate, gateRate, holeRate`, or
    /// `sly (slime): unavailable — the gate chain is broken at line 4: …`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}): ", self.name, self.race.name())?;
        if let Some(why) = &self.unavailable {
            return write!(f, "unavailable — {why}");
        }
        // A record that holds none of its meters says so once, naming them.
        if self
            .readings
            .iter()
            .all(|r| r.rate.is_none_or(|x| x.n == 0))
        {
            let names: Vec<&str> = self.readings.iter().map(|r| r.meter).collect();
            return write!(f, "no reports — {}", names.join(", "));
        }
        let shown: Vec<String> = self
            .readings
            .iter()
            .map(|r| match r.rate {
                Some(rate) => format!("{} {rate}", r.meter),
                None => format!("{} unavailable", r.meter),
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
        let shown: Vec<String> = of(&r, &folds, &Ok(Folded::default()))
            .iter()
            .map(ToString::to_string)
            .collect();
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
