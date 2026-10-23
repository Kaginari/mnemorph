//! How each race is measured, when it ascends and when it falls.
//!
//! Every input is a [`Signals`] fold the ledger computes; nothing here asks a model. Rates are
//! percentages, `0.0..=100.0`.

use crate::Race;

/// The readings a race is judged on, over a window. Rates are percentages.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Signals {
    /// Reports in the window.
    pub reports: u32,
    /// Findings filed (Dark Elves).
    pub findings: u32,
    /// Outward acts taken (Kijin).
    pub outward_acts: u32,
    /// Outward acts rolled back (Kijin).
    pub rollbacks: u32,
    /// Envelope acts the human corrected (Oni).
    pub envelope_corrections: u32,
    /// Passes this body held that a later fail overturned on a sealed territory (Orcs).
    pub overturned_on_sealed: u32,
    /// Landings that failed *tests intact* (Slimes).
    pub tests_intact_failures: u32,
    /// Human turns closed as done and not reopened (the throne).
    pub ask_done_rate: f64,
    /// Dispatches that landed (commanders).
    pub plan_land_rate: f64,
    /// Dispatches re-commissioned after a council warrant (commanders).
    pub reask_rate: f64,
    /// Runs that closed done, with no fail word and no failed gate.
    pub land_rate: f64,
    /// Verdicts held and not overturned (gate holders).
    pub gate_right: f64,
    /// 100 less the mean strain of the bodies it dispatched.
    pub slime_health: f64,
    /// First-try gate passes (authors).
    pub gate_rate: f64,
    /// Reports that carried a harness hole.
    pub hole_rate: f64,
    /// Outward acts that completed without rollback or human correction (Kijin).
    pub outward_clean_rate: f64,
    /// Verify lines green at the gate (self-gated authors).
    pub verify_green_rate: f64,
    /// Findings, linked by id, that a later landing fixed (Dark Elves).
    pub upheld_rate: f64,
    /// One less the share of findings the human dismissed on contest (Dark Elves).
    pub precision: f64,
}

/// Fewer reports than this: shown, never ranked.
const MIN_RECORD: u32 = 5;
/// Reports a rise needs; the same count of findings for a Dark Elf.
const RISE_RECORD: u32 = 20;
/// Outward acts an Oni's rise needs.
const RISE_OUTWARD: u32 = 30;
/// The recent record a rate-driven fall reads.
const FALL_RECORD: u32 = 10;
/// Reports a body on probation has to recover; past them without recovering, it falls.
pub const RECOVER_REPORTS: u32 = 5;
/// Runs a declined candidate waits before it is proposed again.
pub const DECLINE_COOLDOWN_RUNS: u32 = 10;

/// An ascended body's recent record: its last ten reports (ten findings for a Dark Elf), or,
/// on probation, the reports since probation began. A distinct type from the window's fold, so
/// the two are never swapped.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Recent(pub Signals);

/// A race's score, `0.0..=100.0`, by its race's compiled formula.
#[must_use]
pub fn score(race: Race, s: &Signals) -> f64 {
    let raw = match race.base() {
        Race::Elf => 0.6 * s.plan_land_rate - 0.2 * s.reask_rate + 0.2 * s.land_rate,
        Race::Orc => 0.5 * s.gate_right + 0.3 * s.slime_health + 0.2 * s.land_rate,
        Race::Slime => 0.6 * s.land_rate + 0.3 * s.gate_rate - 0.1 * s.hole_rate,
        Race::Kijin => 0.5 * s.outward_clean_rate + 0.3 * s.verify_green_rate + 0.2 * s.land_rate,
        Race::DarkElf => 0.7 * s.upheld_rate + 0.3 * s.precision,
        _ => s.ask_done_rate,
    };
    raw.clamp(0.0, 100.0)
}

/// Where a meter's reading falls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// At or above the ok bar.
    Ok,
    /// At or above the warn bar.
    Warn,
    /// Below the warn bar.
    Low,
    /// Too few reports to judge: shown, never ranked.
    Unranked,
}

/// One race's headline reading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meter {
    /// What it measures, as surfaces print it.
    pub label: &'static str,
    /// The reading, a percentage.
    pub value: f64,
    /// Its band.
    pub band: Band,
}

/// The race's meter over `s`.
#[must_use]
pub fn meter(race: Race, s: &Signals) -> Meter {
    let (label, value, ok, warn) = match race.base() {
        Race::Elf => ("Plans landed", s.plan_land_rate, 80.0, 50.0),
        Race::Orc => ("Gate right", s.gate_right, 90.0, 70.0),
        Race::Slime => ("Landed", s.land_rate, 80.0, 50.0),
        Race::Kijin => ("Outward clean", s.outward_clean_rate, 90.0, 70.0),
        Race::DarkElf => ("Upheld", s.upheld_rate, 80.0, 50.0),
        _ => ("Asks done", s.ask_done_rate, 80.0, 50.0),
    };
    let band = if s.reports < MIN_RECORD {
        Band::Unranked
    } else if value >= ok {
        Band::Ok
    } else if value >= warn {
        Band::Warn
    } else {
        Band::Low
    };
    Meter { label, value, band }
}

/// Whether a body of `risen`'s base race has earned `risen` over the window (the 28-day
/// fold). False when `risen` is not an ascended form.
#[must_use]
pub fn rises(risen: Race, s: &Signals) -> bool {
    match risen {
        Race::HighElf => {
            s.reports >= RISE_RECORD && s.plan_land_rate >= 85.0 && s.reask_rate <= 10.0
        }
        Race::HighOrc => s.reports >= RISE_RECORD && s.gate_right >= 90.0 && s.hole_rate <= 10.0,
        Race::NamedSlime => {
            s.reports >= RISE_RECORD
                && s.land_rate >= 90.0
                && s.gate_rate >= 90.0
                && s.hole_rate <= 5.0
        }
        Race::Oni => {
            s.outward_acts >= RISE_OUTWARD && s.outward_clean_rate >= 95.0 && s.rollbacks == 0
        }
        Race::ArchDarkElf => {
            s.findings >= RISE_RECORD && s.upheld_rate >= 80.0 && s.precision >= 85.0
        }
        _ => false,
    }
}

/// What the recent record says about an ascended body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallReading {
    /// It holds its form.
    Holds,
    /// A rate fell below the fall bar: probation, with five reports to recover.
    Probation,
    /// An event that ends the ascension at once.
    Immediate,
}

/// Reads an ascended body's recent record (its last ten reports, or findings for a Dark Elf).
/// Events fall at once; rates fall into probation, and only on a full recent record. The fall
/// bars sit well under the rise bars, so a body does not flap.
#[must_use]
pub fn falls(risen: Race, recent: &Recent) -> FallReading {
    let recent = &recent.0;
    let event = match risen {
        Race::HighOrc => recent.overturned_on_sealed > 0,
        Race::NamedSlime => recent.tests_intact_failures > 0,
        Race::Oni => recent.rollbacks > 0 || recent.envelope_corrections > 0,
        _ => false,
    };
    if event {
        return FallReading::Immediate;
    }
    let (record, below) = match risen {
        Race::HighElf => (recent.reports, recent.plan_land_rate < 70.0),
        Race::HighOrc => (recent.reports, recent.gate_right < 75.0),
        Race::NamedSlime => (recent.reports, recent.land_rate < 75.0),
        Race::ArchDarkElf => (recent.findings, recent.precision < 70.0),
        _ => (0, false),
    };
    if record >= FALL_RECORD && below {
        FallReading::Probation
    } else {
        FallReading::Holds
    }
}

/// Whether a body on probation has recovered: over the reports since probation began (at least
/// [`RECOVER_REPORTS`] of them, or findings for a Dark Elf), its rate is back at or above the fall
/// bar and no event fell it. `false` means it has not recovered yet; the caller ends probation
/// with a fall once [`RECOVER_REPORTS`] have passed.
#[must_use]
pub fn recovers(risen: Race, since: &Recent) -> bool {
    let s = &since.0;
    let (record, back) = match risen {
        Race::HighElf => (s.reports, s.plan_land_rate >= 70.0),
        Race::HighOrc => (s.reports, s.gate_right >= 75.0),
        Race::NamedSlime => (s.reports, s.land_rate >= 75.0),
        Race::ArchDarkElf => (s.findings, s.precision >= 70.0),
        _ => return false,
    };
    record >= RECOVER_REPORTS && back && falls(risen, since) != FallReading::Immediate
}
