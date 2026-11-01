//! The report ledger: every creature's reports, folded from the harness's trusted journals into
//! the rows the race law reads, and each creature's inputs folded from those rows.
//!
//! A row is one report in its race's own unit:
//!
//! - **`run`**: one run of a body, as the usage witness names it — the gate's last word on its
//!   landing, a first-try pass, the holes the harness found (*tests intact*), whether its verify
//!   lines were green; and, for a run that commissioned others, its dispatches, those that landed,
//!   those re-commissioned, the passes the gate held on them and those a Dark Elf's upheld finding
//!   overturned;
//! - **`ask`**: one landing of the throne the gate judged, and whether it was done — passed, and
//!   not reopened by any of the throne's next three landings rewriting one of its paths;
//! - **`act`**: one outward act of a Kijin post that reached an outcome, clean or not, rolled back
//!   or not.
//!
//! The rows are a function of the journals alone: each row carries the build of the journal row
//! that settled it (`unknown` when that journal stamps none), never the build that folded it, so
//! the same journals fold to the same bytes under any binary ([`ledger_bytes`]). The ledger is
//! chained from its genesis ([`crate::chain`]).
//!
//! An input no trusted journal carries yet is `None`: unknown, never a zero.

use std::collections::BTreeMap;
use std::io;

use mnemorph_races::{GateWord, Outcome, Race, Rate, Said, Signals, fold_reports};
use serde::{Deserialize, Serialize};

use crate::Build;
use crate::chain::Walk;

/// The gate's last word on a run's landing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Gate {
    /// Every check ran and held.
    Pass,
    /// A check ran and did not hold.
    Fail,
    /// Nothing failed, but a check gave no result.
    NotVerified,
}

impl Gate {
    /// The word as the race law reads it.
    #[must_use]
    pub fn word(self) -> GateWord {
        match self {
            Gate::Pass => GateWord::Pass,
            Gate::Fail => GateWord::Fail,
            Gate::NotVerified => GateWord::NotVerified,
        }
    }
}

/// One run of a body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    /// The creature.
    pub creature: String,
    /// Its race, as the usage witness names it.
    pub race: String,
    /// The run.
    pub run: String,
    /// The build of the gate verdict that settled it; `unknown` with none.
    pub build: Build,
    /// The gate's last word on its landing; `None` when nothing landed.
    pub gate: Option<Gate>,
    /// The gate passed on its first try.
    pub first_try: bool,
    /// Holes the harness found: *tests intact* failures, over every verdict of the run.
    pub holes: u32,
    /// Its verify lines green at the gate: no `traits hold` failure and no unverified line;
    /// `None` when no gate judged it.
    pub verify_green: Option<bool>,
    /// Runs it commissioned.
    pub dispatches: u32,
    /// Of those, the ones that landed: the gate passed a landing under them and failed none.
    pub landed: u32,
    /// Of those, the ones that commissioned a body this run had already commissioned.
    pub reasks: u32,
    /// Of those, the ones whose landing the gate passed.
    pub passes: u32,
    /// Of those passes, the ones a Dark Elf's upheld finding overturned.
    pub overturned: u32,
}

/// One landing of the throne the gate judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskReport {
    /// The creature.
    pub creature: String,
    /// Its race.
    pub race: String,
    /// The verdict's place among the gate's verdicts, from 1.
    pub verdict: usize,
    /// The verdict's build.
    pub build: Build,
    /// Passed, and none of the throne's next three landings rewrote one of its paths.
    pub done: bool,
}

/// One outward act of a Kijin post that reached an outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActReport {
    /// The post's creature.
    pub creature: String,
    /// Its race.
    pub race: String,
    /// The act's receipt.
    pub act: String,
    /// The post journal stamps no build: `unknown`.
    pub build: Build,
    /// Completed without rollback or the human's look.
    pub clean: bool,
    /// Put back by an undo.
    pub rolled_back: bool,
}

/// One row of the report ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReportRow {
    /// A run.
    Run(RunReport),
    /// A throne's judged landing.
    Ask(AskReport),
    /// A Kijin's outward act.
    Act(ActReport),
}

impl ReportRow {
    /// The creature and race it counts for.
    #[must_use]
    pub fn key(&self) -> (&str, &str) {
        match self {
            ReportRow::Run(r) => (&r.creature, &r.race),
            ReportRow::Ask(r) => (&r.creature, &r.race),
            ReportRow::Act(r) => (&r.creature, &r.race),
        }
    }
}

/// One creature's inputs to the race law, each as counts over its own unit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Inputs {
    /// Its runs: the record a meter is ranked on.
    pub reports: u32,
    /// Runs whose landing the gate passed, over runs.
    pub land: Rate,
    /// Runs whose landing passed on the first try, over runs.
    pub gate: Rate,
    /// Runs in which the harness found a hole, over runs.
    pub holes: Rate,
    /// Runs whose verify lines were green, over runs the gate judged.
    pub verify_green: Rate,
    /// Dispatches that landed, over dispatches.
    pub plan_land: Rate,
    /// Dispatches re-commissioned, over dispatches.
    pub reask: Rate,
    /// Passes on its dispatches not overturned, over those passes.
    pub gate_right: Rate,
    /// Outward acts clean, over outward acts.
    pub outward_clean: Rate,
    /// Outward acts rolled back.
    pub rollbacks: u32,
    /// The throne's landings done, over its judged landings.
    pub ask_done: Rate,
    /// The outcomes the law's own report fold reads, in order.
    pub outcomes: Vec<Outcome>,
    /// 100 less the mean strain of the bodies it dispatched: the strain is journaled only inside
    /// the world, so no trusted journal carries it yet — unknown.
    pub slime_health: Option<f64>,
    /// Passes overturned on a sealed territory: no trusted journal marks a seal on a landing yet
    /// — unknown.
    pub overturned_on_sealed: Option<u32>,
    /// Envelope acts the human corrected: the post journal does not say which act an envelope
    /// covered — unknown.
    pub envelope_corrections: Option<u32>,
}

impl Inputs {
    /// The input the race law names `meter`, as its counts; `None` for a name the fold does not
    /// read.
    #[must_use]
    pub fn rate(&self, meter: &str) -> Option<Rate> {
        Some(match meter {
            "landRate" => self.land,
            "gateRate" => self.gate,
            "holeRate" => self.holes,
            "verifyGreenRate" => self.verify_green,
            "planLandRate" => self.plan_land,
            "reaskRate" => self.reask,
            "gateRight" => self.gate_right,
            "outwardCleanRate" => self.outward_clean,
            "askDoneRate" => self.ask_done,
            _ => return None,
        })
    }

    /// The signals the compiled law reads for `race`, when every input it reads is known: the
    /// rates of the run reports through the law's own fold ([`fold_reports`]). `None` names the
    /// first input no trusted journal carries.
    ///
    /// # Errors
    ///
    /// An input `race`'s score or bars read is unknown: its law name.
    pub fn signals(&self, race: Race) -> Result<Signals, &'static str> {
        let pct = |r: Rate| r.value().map_or(0.0, |p| p * 100.0);
        let mut s = fold_reports(&self.outcomes);
        s.tests_intact_failures = count(self.outcomes.iter().filter(|o| o.holes_found > 0).count());
        s.verify_green_rate = pct(self.verify_green);
        s.plan_land_rate = pct(self.plan_land);
        s.reask_rate = pct(self.reask);
        s.gate_right = pct(self.gate_right);
        s.outward_acts = self.outward_clean.n;
        s.outward_clean_rate = pct(self.outward_clean);
        s.rollbacks = self.rollbacks;
        s.ask_done_rate = pct(self.ask_done);
        match race.base() {
            Race::Orc => {
                s.slime_health = self.slime_health.ok_or("slimeHealth")?;
                s.overturned_on_sealed = self.overturned_on_sealed.ok_or("overturnedOnSealed")?;
            }
            Race::Kijin => {
                s.envelope_corrections = self.envelope_corrections.ok_or("envelopeCorrections")?;
            }
            Race::DarkElf => return Err("findings"),
            _ => {}
        }
        Ok(s)
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn add(r: &mut Rate, success: bool) {
    r.n = r.n.saturating_add(1);
    if success {
        r.successes = r.successes.saturating_add(1);
    }
}

fn add_many(r: &mut Rate, successes: u32, n: u32) {
    r.successes = r.successes.saturating_add(successes.min(n));
    r.n = r.n.saturating_add(n);
}

/// Each creature's inputs over `rows`, by creature and race.
#[must_use]
pub fn fold(rows: &[ReportRow]) -> BTreeMap<(String, String), Inputs> {
    let mut out: BTreeMap<(String, String), Inputs> = BTreeMap::new();
    for row in rows {
        let (c, r) = row.key();
        let i = out.entry((c.to_owned(), r.to_owned())).or_default();
        match row {
            ReportRow::Run(run) => {
                let outcome = Outcome {
                    // The fold never reads the body's own word or holes.
                    said: Said::Other,
                    holes_said: 0,
                    gate: run.gate.map(Gate::word),
                    first_try: run.first_try,
                    holes_found: run.holes,
                };
                add(&mut i.land, run.gate == Some(Gate::Pass));
                add(&mut i.gate, run.gate == Some(Gate::Pass) && run.first_try);
                add(&mut i.holes, run.holes > 0);
                if let Some(green) = run.verify_green {
                    add(&mut i.verify_green, green);
                }
                add_many(&mut i.plan_land, run.landed, run.dispatches);
                add_many(&mut i.reask, run.reasks, run.dispatches);
                add_many(
                    &mut i.gate_right,
                    run.passes.saturating_sub(run.overturned),
                    run.passes,
                );
                i.outcomes.push(outcome);
            }
            ReportRow::Ask(ask) => add(&mut i.ask_done, ask.done),
            ReportRow::Act(act) => {
                add(&mut i.outward_clean, act.clean);
                if act.rolled_back {
                    i.rollbacks = i.rollbacks.saturating_add(1);
                }
            }
        }
    }
    for i in out.values_mut() {
        i.reports = count(i.outcomes.len());
    }
    out
}

/// The ledger's bytes for `rows`: each row one line, chained from its genesis. The same rows give
/// the same bytes.
///
/// # Errors
///
/// A row does not serialise.
pub fn ledger_bytes(rows: &[ReportRow]) -> io::Result<Vec<u8>> {
    let mut walk = Walk::new();
    let mut out = Vec::new();
    for row in rows {
        let body = serde_json::to_vec(row).map_err(io::Error::other)?;
        let (bytes, next) = walk.seal(&body)?;
        out.extend_from_slice(&bytes);
        walk = next;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(creature: &str, race: &str, gate: Option<Gate>, first_try: bool) -> ReportRow {
        ReportRow::Run(RunReport {
            creature: creature.into(),
            race: race.into(),
            run: format!("{creature}-r"),
            build: Build::default(),
            gate,
            first_try,
            holes: 0,
            verify_green: gate.map(|g| g == Gate::Pass),
            dispatches: 0,
            landed: 0,
            reasks: 0,
            passes: 0,
            overturned: 0,
        })
    }

    #[test]
    fn the_rates_agree_with_the_laws_own_fold() {
        let mut rows = vec![
            run("sly", "slime", Some(Gate::Pass), true),
            run("sly", "slime", Some(Gate::Pass), false),
            run("sly", "slime", Some(Gate::Fail), false),
            run("sly", "slime", None, false),
        ];
        if let ReportRow::Run(r) = &mut rows[2] {
            r.holes = 1;
        }
        let f = fold(&rows);
        let i = &f[&("sly".to_owned(), "slime".to_owned())];
        assert_eq!(i.reports, 4);
        assert_eq!(i.land, Rate::of(2, 4));
        assert_eq!(i.gate, Rate::of(1, 4));
        assert_eq!(i.holes, Rate::of(1, 4));
        assert_eq!(i.verify_green, Rate::of(2, 3));
        let s = i.signals(Race::Slime).unwrap();
        let law = fold_reports(&i.outcomes);
        assert!((s.land_rate - 50.0).abs() < 1e-9 && (law.land_rate - 50.0).abs() < 1e-9);
        assert!((s.gate_rate - 25.0).abs() < 1e-9);
        assert!((s.hole_rate - 25.0).abs() < 1e-9);
        assert_eq!(s.tests_intact_failures, 1);
    }

    #[test]
    fn an_input_no_journal_carries_is_unknown_never_zero() {
        let f = fold(&[run("gor", "orc", Some(Gate::Pass), true)]);
        let i = &f[&("gor".to_owned(), "orc".to_owned())];
        assert_eq!(i.signals(Race::Orc), Err("slimeHealth"));
        assert_eq!(i.signals(Race::Kijin), Err("envelopeCorrections"));
        assert!(i.signals(Race::Elf).is_ok());
        assert_eq!(i.rate("planLandRate"), Some(Rate::of(0, 0)));
        assert_eq!(i.rate("slimeHealth"), None);
    }

    #[test]
    fn the_ledger_is_chained_from_its_genesis_and_the_same_rows_give_the_same_bytes() {
        let rows = vec![
            run("sly", "slime", Some(Gate::Pass), true),
            ReportRow::Act(ActReport {
                creature: "kijin-forge".into(),
                race: "kijin".into(),
                act: "a1".into(),
                build: Build::default(),
                clean: false,
                rolled_back: true,
            }),
        ];
        let a = ledger_bytes(&rows).unwrap();
        assert_eq!(a, ledger_bytes(&rows).unwrap());
        let mut walk = Walk::new();
        walk.feed(&a);
        let report = walk.report();
        assert!(report.holds(), "{report}");
        assert_eq!((report.rows, report.genesis.map(|g| g.line)), (2, Some(1)));
        let text = String::from_utf8(a).unwrap();
        let first = text.lines().next().unwrap();
        assert!(
            first.contains(r#""kind":"run","creature":"sly""#),
            "{first}"
        );
        assert!(first.contains(r#""build":{"version":"unknown","commit":"unknown"}"#));
        let back: Vec<ReportRow> = crate::parse_rows(text.as_bytes()).unwrap().rows;
        assert_eq!(back, rows);
    }
}
