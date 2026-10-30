//! How each race is measured, when it ascends and when it falls.
//!
//! Every input is a [`Signals`] fold the ledger computes; nothing here asks a model. Rates are
//! percentages, `0.0..=100.0`.
//!
//! A meter is shown with its count and its 95% Wilson interval ([`wilson`], [`Rate`]); the bars
//! that decide a rise or a fall read the point estimate, never the interval.

use std::fmt;

use crate::Race;
use crate::explain::{Explanation, Kind, Op, Recover, Rerise, Scale};

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
    /// Reports whose landing the gate passed: the harness's word, never the body's, so a body
    /// that says `@S FAIL` over a passing landing is not scored under one that says nothing.
    pub land_rate: f64,
    /// Verdicts held and not overturned (gate holders).
    pub gate_right: f64,
    /// 100 less the mean strain of the bodies it dispatched.
    pub slime_health: f64,
    /// First-try gate passes (authors).
    pub gate_rate: f64,
    /// Reports in which the harness found a hole — an edited or deleted verify line — never the
    /// body's own `@?`.
    pub hole_rate: f64,
    /// Outward acts that completed without rollback or human correction (Kijin).
    pub outward_clean_rate: f64,
    /// Verify lines green at the gate (self-gated authors).
    pub verify_green_rate: f64,
    /// Findings upheld — confirmed by their verifier or by the human — over findings filed, each
    /// root cause once (Dark Elves).
    pub upheld_rate: f64,
    /// Findings upheld over those upheld or refuted; `cannot-verify` is in neither (Dark Elves).
    pub precision: f64,
}

/// Fewer than this of the race's own unit (reports; findings for a Dark Elf, outward acts for a
/// Kijin): shown, never ranked.
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

/// The z of a two-sided 95% interval.
pub const Z95: f64 = 1.96;

/// The Wilson score interval of `successes` in `n` trials at `z`, as proportions `(lo, hi)`
/// within `0.0..=1.0`; `None` with no trial. Successes past `n` count as `n`. Zero successes
/// still give an upper bound over zero: no event seen is never no risk.
#[must_use]
pub fn wilson(successes: u32, n: u32, z: f64) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let n = f64::from(n);
    let p = f64::from(successes).min(n) / n;
    let z2 = z * z;
    let centre = p + z2 / (2.0 * n);
    let half = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    let scale = 1.0 + z2 / n;
    Some((
        ((centre - half) / scale).max(0.0),
        ((centre + half) / scale).min(1.0),
    ))
}

/// A rate as its counts: `successes` of `n`. Shown as its proportion, its counts and its 95%
/// Wilson interval — `0.90 (18/20, 95% 0.70–0.97)` — or `no reports` when `n` is zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rate {
    /// What succeeded.
    pub successes: u32,
    /// Of how many.
    pub n: u32,
}

impl Rate {
    /// `successes` of `n`.
    #[must_use]
    pub fn of(successes: u32, n: u32) -> Rate {
        Rate { successes, n }
    }

    /// The point estimate, a proportion; `None` with no trial.
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        (self.n > 0).then(|| f64::from(self.successes.min(self.n)) / f64::from(self.n))
    }

    /// The 95% Wilson interval; `None` with no trial.
    #[must_use]
    pub fn interval(&self) -> Option<(f64, f64)> {
        wilson(self.successes, self.n, Z95)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.value(), self.interval()) {
            (Some(p), Some((lo, hi))) => write!(
                f,
                "{p:.2} ({}/{}, 95% {lo:.2}–{hi:.2})",
                self.successes.min(self.n),
                self.n
            ),
            _ => f.write_str("no reports"),
        }
    }
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
    /// The reading as counts over the race's own unit (reports; findings for a Dark Elf,
    /// outward acts for a Kijin), for its interval: shown, never judged on.
    pub rate: Rate,
}

impl fmt::Display for Meter {
    /// `Upheld 0.85 (17/20, 95% 0.64–0.95)`, or `Upheld no reports`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.label, self.rate)
    }
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
    let record = match race.base() {
        Race::DarkElf => s.findings,
        Race::Kijin => s.outward_acts,
        _ => s.reports,
    };
    let band = if record < MIN_RECORD {
        Band::Unranked
    } else if value >= ok {
        Band::Ok
    } else if value >= warn {
        Band::Warn
    } else {
        Band::Low
    };
    // The successes the percentage stands for over the record: exact when the fold's rate is
    // over the same unit.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=record first"
    )]
    let successes = (value.clamp(0.0, 100.0) * f64::from(record) / 100.0).round() as u32;
    Meter {
        label,
        value,
        band,
        rate: Rate::of(successes, record),
    }
}

/// The unit a form's record is counted in: findings for a Dark Elf, outward acts for a Kijin,
/// reports for every other race.
fn unit(race: Race) -> &'static str {
    match race.base() {
        Race::DarkElf => "findings",
        Race::Kijin => "outward acts",
        _ => "reports",
    }
}

/// A form's record over `s`, in its [`unit`].
fn record(race: Race, s: &Signals) -> u32 {
    match race.base() {
        Race::DarkElf => s.findings,
        Race::Kijin => s.outward_acts,
        _ => s.reports,
    }
}

/// One bar: a meter, read off the signals, held against a threshold.
struct Bar {
    meter: &'static str,
    scale: Scale,
    read: fn(&Signals) -> f64,
    op: Op,
    threshold: f64,
}

impl Bar {
    fn rate(meter: &'static str, read: fn(&Signals) -> f64, op: Op, threshold: f64) -> Bar {
        Bar {
            meter,
            scale: Scale::Rate,
            read,
            op,
            threshold,
        }
    }

    fn count(meter: &'static str, read: fn(&Signals) -> f64, op: Op, threshold: f64) -> Bar {
        Bar {
            meter,
            scale: Scale::Count,
            read,
            op,
            threshold,
        }
    }

    /// The bar read over `s`, a record of `window`.
    fn explain(&self, kind: Kind, risen: Race, s: &Signals, window: u32) -> Explanation {
        Explanation {
            kind,
            meter: self.meter,
            scale: self.scale,
            reading: (self.read)(s),
            op: self.op,
            threshold: self.threshold,
            window,
            unit: unit(risen),
            recover: None,
            rerise: None,
            head: None,
            from: None,
            to: None,
        }
    }
}

/// The record a rise needs: outward acts for an Oni, reports or findings for the others.
fn rise_record(risen: Race) -> u32 {
    if risen == Race::Oni {
        RISE_OUTWARD
    } else {
        RISE_RECORD
    }
}

/// The bars of `risen`'s rise, its record first; none when `risen` is not an ascended form. An
/// event that would end the form at once bars the rise while it is in the window, so a body that
/// just rose never reads as falling.
fn rise_bars(risen: Race) -> Vec<Bar> {
    let at_least = |meter: &'static str, read: fn(&Signals) -> f64| {
        Bar::count(meter, read, Op::Ge, f64::from(rise_record(risen)))
    };
    let reports = || at_least("reports", |s| f64::from(s.reports));
    match risen {
        Race::HighElf => vec![
            reports(),
            Bar::rate("planLandRate", |s| s.plan_land_rate, Op::Ge, 85.0),
            Bar::rate("reaskRate", |s| s.reask_rate, Op::Le, 10.0),
        ],
        Race::HighOrc => vec![
            reports(),
            Bar::rate("gateRight", |s| s.gate_right, Op::Ge, 90.0),
            Bar::rate("holeRate", |s| s.hole_rate, Op::Le, 10.0),
            Bar::count(
                "overturnedOnSealed",
                |s| f64::from(s.overturned_on_sealed),
                Op::Le,
                0.0,
            ),
        ],
        Race::NamedSlime => vec![
            reports(),
            Bar::rate("landRate", |s| s.land_rate, Op::Ge, 90.0),
            Bar::rate("gateRate", |s| s.gate_rate, Op::Ge, 90.0),
            Bar::rate("holeRate", |s| s.hole_rate, Op::Le, 5.0),
            Bar::count(
                "testsIntactFailures",
                |s| f64::from(s.tests_intact_failures),
                Op::Le,
                0.0,
            ),
        ],
        Race::Oni => vec![
            at_least("outwardActs", |s| f64::from(s.outward_acts)),
            Bar::rate("outwardCleanRate", |s| s.outward_clean_rate, Op::Ge, 95.0),
            Bar::count("rollbacks", |s| f64::from(s.rollbacks), Op::Le, 0.0),
            Bar::count(
                "envelopeCorrections",
                |s| f64::from(s.envelope_corrections),
                Op::Le,
                0.0,
            ),
        ],
        Race::ArchDarkElf => vec![
            at_least("findings", |s| f64::from(s.findings)),
            Bar::rate("upheldRate", |s| s.upheld_rate, Op::Ge, 80.0),
            Bar::rate("precision", |s| s.precision, Op::Ge, 85.0),
        ],
        _ => Vec::new(),
    }
}

/// The rate whose fall bar starts `risen`'s probation; `None` for a form that falls only by event.
fn fall_bar(risen: Race) -> Option<Bar> {
    Some(match risen {
        Race::HighElf => Bar::rate("planLandRate", |s| s.plan_land_rate, Op::Lt, 70.0),
        Race::HighOrc => Bar::rate("gateRight", |s| s.gate_right, Op::Lt, 75.0),
        Race::NamedSlime => Bar::rate("landRate", |s| s.land_rate, Op::Lt, 75.0),
        Race::ArchDarkElf => Bar::rate("precision", |s| s.precision, Op::Lt, 70.0),
        _ => return None,
    })
}

/// The events that end `risen`'s ascension at once.
fn event_bars(risen: Race) -> Vec<Bar> {
    let any = |meter: &'static str, read: fn(&Signals) -> f64| Bar::count(meter, read, Op::Gt, 0.0);
    match risen {
        Race::HighOrc => vec![any("overturnedOnSealed", |s| {
            f64::from(s.overturned_on_sealed)
        })],
        Race::NamedSlime => vec![any("testsIntactFailures", |s| {
            f64::from(s.tests_intact_failures)
        })],
        Race::Oni => vec![
            any("rollbacks", |s| f64::from(s.rollbacks)),
            any("envelopeCorrections", |s| f64::from(s.envelope_corrections)),
        ],
        _ => Vec::new(),
    }
}

/// The rise bar a body of `risen` must earn again once it falls on `meter`: that meter's rise bar,
/// else the form's first rate bar. `None` for a form with no rise.
#[must_use]
pub fn rerise(risen: Race, meter: &str) -> Option<Rerise> {
    let bars = rise_bars(risen);
    let rates: Vec<&Bar> = bars.iter().filter(|b| b.scale == Scale::Rate).collect();
    let bar = rates
        .iter()
        .find(|b| b.meter == meter)
        .or_else(|| rates.first())?;
    Some(Rerise {
        meter: bar.meter,
        op: bar.op,
        threshold: bar.threshold,
        over: rise_record(risen),
        unit: unit(risen),
    })
}

/// Every bar of `risen`'s rise read over the window, held or not: a rise is proposed when each
/// holds ([`rises`]). Empty when `risen` is not an ascended form.
#[must_use]
pub fn explain_rise(risen: Race, s: &Signals) -> Vec<Explanation> {
    let window = record(risen, s);
    rise_bars(risen)
        .iter()
        .map(|b| b.explain(Kind::Rise, risen, s, window))
        .collect()
}

/// Whether a body of `risen`'s base race has earned `risen` over the window (the 28-day
/// fold): every bar of [`explain_rise`] holds. False when `risen` is not an ascended form.
#[must_use]
pub fn rises(risen: Race, s: &Signals) -> bool {
    let bars = explain_rise(risen, s);
    !bars.is_empty() && bars.iter().all(Explanation::holds)
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

/// The bars an ascended body's recent record fails, one line each: every event that ends the
/// ascension at once; else its rate under the fall bar on a full recent record, with the bar that
/// ends probation and the rise bar it would need again. Empty when it holds its form.
#[must_use]
pub fn explain_fall(risen: Race, recent: &Recent) -> Vec<Explanation> {
    let s = &recent.0;
    let window = record(risen, s);
    let events: Vec<Explanation> = event_bars(risen)
        .iter()
        .map(|b| b.explain(Kind::Fall, risen, s, window))
        .filter(Explanation::holds)
        .map(|e| Explanation {
            rerise: rerise(risen, e.meter),
            ..e
        })
        .collect();
    if !events.is_empty() {
        return events;
    }
    fall_bar(risen)
        .map(|b| b.explain(Kind::Probation, risen, s, window))
        .filter(|e| window >= FALL_RECORD && e.holds())
        .map(|e| Explanation {
            recover: Some(Recover {
                op: Op::Ge,
                threshold: e.threshold,
                within: RECOVER_REPORTS,
                unit: e.unit,
            }),
            rerise: rerise(risen, e.meter),
            ..e
        })
        .into_iter()
        .collect()
}

/// Reads an ascended body's recent record (its last ten reports, or findings for a Dark Elf).
/// Events fall at once; rates fall into probation, and only on a full recent record. The fall
/// bars sit well under the rise bars, so a body does not flap. The reading of
/// [`explain_fall`]'s lines.
#[must_use]
pub fn falls(risen: Race, recent: &Recent) -> FallReading {
    match explain_fall(risen, recent).first() {
        None => FallReading::Holds,
        Some(e) if e.kind == Kind::Fall => FallReading::Immediate,
        Some(_) => FallReading::Probation,
    }
}

/// Whether a body on probation has recovered: over the reports since probation began (at least
/// [`RECOVER_REPORTS`] of them, or findings for a Dark Elf), its rate is back at or above the fall
/// bar and no event fell it. `false` means it has not recovered yet; the caller ends probation
/// with a fall once [`RECOVER_REPORTS`] have passed.
#[must_use]
pub fn recovers(risen: Race, since: &Recent) -> bool {
    let Some(bar) = fall_bar(risen) else {
        return false;
    };
    let s = &since.0;
    record(risen, s) >= RECOVER_REPORTS
        && Op::Ge.holds((bar.read)(s), bar.threshold)
        && falls(risen, since) != FallReading::Immediate
}

/// The gate's word on a report's landing, as the fold reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateWord {
    /// Every check ran and held.
    Pass,
    /// A check ran and did not hold.
    Fail,
    /// Nothing failed, but a check gave no result.
    NotVerified,
}

/// The word a report opened with (`@S <word>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    /// `DONE` or `PASS`.
    Done,
    /// `FAIL`.
    Fail,
    /// `ESCALATE`.
    Escalate,
    /// Any other word.
    Other,
}

/// What one report came to: the body's own word and honest holes, and the harness's facts. The
/// fold ([`fold_reports`]) reads the harness's facts alone, so honest bad news — `@S FAIL`,
/// `@S ESCALATE`, an `@?` hole — never scores worse than silence over the same outcome. A
/// report's prose is not here at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// The word the report opened with: never read by the fold.
    pub said: Said,
    /// `@?` holes the body raised itself: never read by the fold.
    pub holes_said: u32,
    /// The gate's word on its landing; `None` when nothing landed.
    pub gate: Option<GateWord>,
    /// The gate passed on its first try.
    pub first_try: bool,
    /// Holes the harness found in the work: an edited or deleted verify line.
    pub holes_found: u32,
}

/// Reports folded into the signals they set: `reports`, `land_rate` (landings the gate passed),
/// `gate_rate` (landings the gate passed on the first try) and `hole_rate` (reports in which the
/// harness found a hole), each over every report. Every other signal is left at zero.
#[must_use]
pub fn fold_reports(outcomes: &[Outcome]) -> Signals {
    let n = u32::try_from(outcomes.len()).unwrap_or(u32::MAX);
    let pct = |pick: fn(&Outcome) -> bool| {
        let k = u32::try_from(outcomes.iter().filter(|o| pick(o)).count()).unwrap_or(u32::MAX);
        if n == 0 {
            0.0
        } else {
            f64::from(k) * 100.0 / f64::from(n)
        }
    };
    Signals {
        reports: n,
        land_rate: pct(|o| o.gate == Some(GateWord::Pass)),
        gate_rate: pct(|o| o.gate == Some(GateWord::Pass) && o.first_try),
        hole_rate: pct(|o| o.holes_found > 0),
        ..Signals::default()
    }
}
