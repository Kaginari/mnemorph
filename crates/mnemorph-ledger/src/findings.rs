//! The Dark Elf's value ledger: its findings folded per speciality, from the challenge records the
//! harness keeps outside the world, into the [`Signals`] the race law judges.
//!
//! - **One root cause counts once.** Rows are deduplicated by fingerprint within a speciality,
//!   across landings: the fingerprint is line-free, so a finding filed again after its line moved,
//!   or confirmed on a later landing, is the same finding.
//! - **Its standing** is the human's latest decision on it when there is one; else the strongest
//!   verdict any landing holds: confirmed, then a critical held for the human, then refuted, then
//!   `cannot-verify`, then none.
//! - **Upheld** is confirmed by its verifier, or confirmed by the human (a later fix proves it the
//!   same way); **refuted** is refuted by its verifier, or dismissed by the human as a false
//!   positive. A critical its verifier refuted is the human's to decide, and is neither until
//!   decided; `cannot-verify` and an unverified finding are neither.
//! - **Precision** is `upheld ÷ (upheld + refuted)`; the **upheld rate** is `upheld ÷ findings`.
//! - **The last ten**: the ten latest findings that are upheld or refuted, by the record that
//!   settled each, are what a fall is read on ([`Recent`]).
//!
//! A rise or a fall is only ever proposed here ([`Ascension`]): applying one is the human's.

use std::collections::BTreeMap;

use mnemorph_races::{FallReading, Meter, Race, Recent, Signals, falls, meter, rises};

/// How many settled findings a fall reads.
pub const WINDOW: usize = 10;

/// The share of the last [`WINDOW`] findings, settled or not, past which an Arch Dark Elf's
/// `cannot-verify` rate proposes probation: findings no verifier can settle never reach the
/// precision a fall reads, so a body could stall its fall by filing them.
pub const CANNOT_VERIFY_BAR: f64 = 50.0;

/// Where one finding stands on one landing, as its challenge records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Kept, no verdict yet.
    Unverified,
    /// Its verifier could not verify it.
    CannotVerify,
    /// Its verifier refuted it.
    Refuted,
    /// A critical its verifier refuted, held for the human: neither upheld nor refuted until the
    /// human decides.
    Held,
    /// Its verifier confirmed it.
    Confirmed,
    /// The human dismissed it as a false positive.
    Dismissed,
    /// The human confirmed it.
    Upheld,
}

impl Standing {
    /// The human's own word: it outranks every verdict, and the latest stands.
    fn human(self) -> bool {
        matches!(self, Standing::Dismissed | Standing::Upheld)
    }

    /// How strongly a verifier's verdict settles it, across landings.
    fn rank(self) -> u8 {
        match self {
            Standing::Unverified => 0,
            Standing::CannotVerify => 1,
            Standing::Refuted => 2,
            Standing::Held => 3,
            Standing::Confirmed | Standing::Dismissed | Standing::Upheld => 4,
        }
    }

    /// `Some(true)` upheld, `Some(false)` refuted, `None` neither.
    fn settled(self) -> Option<bool> {
        match self {
            Standing::Confirmed | Standing::Upheld => Some(true),
            Standing::Refuted | Standing::Dismissed => Some(false),
            Standing::Unverified | Standing::CannotVerify | Standing::Held => None,
        }
    }
}

/// One finding of one landing, as the challenge records hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRow {
    /// The finder's speciality, by its creature name.
    pub speciality: String,
    /// The root cause's fingerprint: line-free, so it links one finding across landings.
    pub fingerprint: String,
    /// The landing it was found on.
    pub landing: String,
    /// Where it stands there.
    pub standing: Standing,
    /// The order of the record that settled it there: a later record is greater.
    pub at: u64,
}

/// One speciality's findings, folded.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpecialityFold {
    /// Findings filed: distinct root causes.
    pub findings: u32,
    /// Upheld: confirmed by a verifier or by the human.
    pub upheld: u32,
    /// Refuted by a verifier, or dismissed by the human.
    pub refuted: u32,
    /// Its verifier could not verify it.
    pub cannot_verify: u32,
    /// A critical refuted by its verifier, waiting on the human.
    pub held: u32,
    /// No verdict yet.
    pub unverified: u32,
    /// The latest settled findings, oldest first, at most [`WINDOW`]: `true` upheld.
    pub recent: Vec<bool>,
    /// The latest findings, settled or not, at most [`WINDOW`]: how many.
    pub last_filed: u32,
    /// Of those, how many its verifier could not verify.
    pub last_cannot_verify: u32,
}

/// What the fold proposes to the human for a speciality. Nothing here applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ascension {
    /// The Dark Elf has met the Arch Dark Elf's bars.
    Rise,
    /// The Arch Dark Elf's precision over its last ten settled fell under the fall bar, or its
    /// `cannot-verify` rate over its last ten findings rose past [`CANNOT_VERIFY_BAR`]: probation.
    Probation,
}

#[allow(clippy::cast_precision_loss)]
fn pct(part: u32, whole: u32) -> Option<f64> {
    (whole > 0).then(|| f64::from(part) * 100.0 / f64::from(whole))
}

impl SpecialityFold {
    /// `upheld ÷ (upheld + refuted)`, a percentage; `None` with nothing settled.
    #[must_use]
    pub fn precision(&self) -> Option<f64> {
        pct(self.upheld, self.upheld + self.refuted)
    }

    /// `upheld ÷ findings`, a percentage; `None` with no finding.
    #[must_use]
    pub fn upheld_rate(&self) -> Option<f64> {
        pct(self.upheld, self.findings)
    }

    /// The precision over [`SpecialityFold::recent`]; `None` when it is empty.
    #[must_use]
    pub fn recent_precision(&self) -> Option<f64> {
        let upheld = self.recent.iter().filter(|u| **u).count();
        pct(
            u32::try_from(upheld).unwrap_or(u32::MAX),
            u32::try_from(self.recent.len()).unwrap_or(u32::MAX),
        )
    }

    /// `cannot-verify ÷ findings` over the last ten findings, settled or not, a percentage;
    /// `None` with no finding.
    #[must_use]
    pub fn cannot_verify_rate(&self) -> Option<f64> {
        pct(self.last_cannot_verify, self.last_filed)
    }

    /// The readings the race law judges over the whole record.
    #[must_use]
    pub fn signals(&self) -> Signals {
        Signals {
            findings: self.findings,
            upheld_rate: self.upheld_rate().unwrap_or(0.0),
            precision: self.precision().unwrap_or(0.0),
            ..Signals::default()
        }
    }

    /// The last ten settled findings, as a fall reads them.
    #[must_use]
    pub fn last(&self) -> Recent {
        Recent(Signals {
            findings: u32::try_from(self.recent.len()).unwrap_or(u32::MAX),
            precision: self.recent_precision().unwrap_or(0.0),
            ..Signals::default()
        })
    }

    /// The race's meter over the whole record.
    #[must_use]
    pub fn meter(&self) -> Meter {
        meter(Race::DarkElf, &self.signals())
    }

    /// What to propose to the human for a body of `form`: a Dark Elf at the rise bars rises; an
    /// Arch Dark Elf under the fall bar on a full last ten settled, or past the `cannot-verify`
    /// bar on a full last ten findings, goes on probation.
    #[must_use]
    pub fn proposal(&self, form: Race) -> Option<Ascension> {
        let stalled = self.last_filed as usize >= WINDOW
            && self
                .cannot_verify_rate()
                .is_some_and(|r| r > CANNOT_VERIFY_BAR);
        match form {
            Race::DarkElf => rises(Race::ArchDarkElf, &self.signals()).then_some(Ascension::Rise),
            Race::ArchDarkElf => (stalled
                || falls(Race::ArchDarkElf, &self.last()) != FallReading::Holds)
                .then_some(Ascension::Probation),
            _ => None,
        }
    }
}

/// `rows` folded per speciality, each root cause counted once.
#[must_use]
pub fn fold_findings(rows: &[FindingRow]) -> BTreeMap<String, SpecialityFold> {
    fold_with(rows, true)
}

/// The fold; `dedupe` off counts every landing's row: how a test shows the dedupe is needed.
fn fold_with(rows: &[FindingRow], dedupe: bool) -> BTreeMap<String, SpecialityFold> {
    // Per speciality, per root cause: its standing and the record that settled it.
    let mut causes: BTreeMap<(&str, String), (Standing, u64)> = BTreeMap::new();
    for (n, r) in rows.iter().enumerate() {
        let key = if dedupe {
            r.fingerprint.clone()
        } else {
            format!("{}#{n}", r.fingerprint)
        };
        let now = (r.standing, r.at);
        causes
            .entry((r.speciality.as_str(), key))
            .and_modify(|was| *was = stronger(*was, now))
            .or_insert(now);
    }
    let mut out: BTreeMap<String, SpecialityFold> = BTreeMap::new();
    let mut settled: BTreeMap<&str, Vec<(u64, bool)>> = BTreeMap::new();
    let mut filed: BTreeMap<&str, Vec<(u64, bool)>> = BTreeMap::new();
    for ((speciality, _), (standing, at)) in causes {
        filed
            .entry(speciality)
            .or_default()
            .push((at, standing == Standing::CannotVerify));
        let f = out.entry(speciality.to_owned()).or_default();
        f.findings += 1;
        match standing {
            Standing::Confirmed | Standing::Upheld => f.upheld += 1,
            Standing::Refuted | Standing::Dismissed => f.refuted += 1,
            Standing::CannotVerify => f.cannot_verify += 1,
            Standing::Held => f.held += 1,
            Standing::Unverified => f.unverified += 1,
        }
        if let Some(upheld) = standing.settled() {
            settled.entry(speciality).or_default().push((at, upheld));
        }
    }
    for (speciality, mut s) in settled {
        s.sort_by_key(|(at, _)| *at);
        let from = s.len().saturating_sub(WINDOW);
        if let Some(f) = out.get_mut(speciality) {
            f.recent = s[from..].iter().map(|(_, u)| *u).collect();
        }
    }
    for (speciality, mut s) in filed {
        s.sort_by_key(|(at, _)| *at);
        let last = &s[s.len().saturating_sub(WINDOW)..];
        if let Some(f) = out.get_mut(speciality) {
            f.last_filed = u32::try_from(last.len()).unwrap_or(u32::MAX);
            f.last_cannot_verify =
                u32::try_from(last.iter().filter(|(_, nv)| *nv).count()).unwrap_or(u32::MAX);
        }
    }
    out
}

/// Of two standings of one root cause, the one that stands: the human's latest word, else the
/// strongest verdict, the earlier record on a tie.
fn stronger(a: (Standing, u64), b: (Standing, u64)) -> (Standing, u64) {
    match (a.0.human(), b.0.human()) {
        (true, true) => {
            if b.1 > a.1 {
                b
            } else {
                a
            }
        }
        (true, false) => a,
        (false, true) => b,
        (false, false) => {
            if b.0.rank() > a.0.rank() {
                b
            } else {
                a
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(fp: &str, landing: &str, standing: Standing, at: u64) -> FindingRow {
        FindingRow {
            speciality: "dark-elf-harness".into(),
            fingerprint: fp.into(),
            landing: landing.into(),
            standing,
            at,
        }
    }

    fn harness(rows: &[FindingRow]) -> SpecialityFold {
        fold_findings(rows)
            .remove("dark-elf-harness")
            .unwrap_or_default()
    }

    /// Confirmed, refuted and cannot-verify: precision is upheld over upheld and refuted, and
    /// cannot-verify is in neither; the upheld rate is over every finding.
    #[test]
    fn precision_leaves_cannot_verify_out() {
        let f = harness(&[
            row("a", "l1", Standing::Confirmed, 1),
            row("b", "l1", Standing::Refuted, 2),
            row("c", "l1", Standing::CannotVerify, 3),
        ]);
        assert_eq!(
            (f.findings, f.upheld, f.refuted, f.cannot_verify),
            (3, 1, 1, 1)
        );
        assert_eq!(f.precision(), Some(50.0));
        let rate = f.upheld_rate().unwrap();
        assert!((rate - 100.0 / 3.0).abs() < 1e-9);
        assert_eq!(f.recent, vec![true, false]);
        let s = f.signals();
        assert_eq!((s.findings, s.precision), (3, 50.0));
        assert_eq!(harness(&[]).precision(), None);
        assert_eq!(harness(&[]).recent_precision(), None);
    }

    /// One root cause, refuted on one landing and confirmed on a later one after its line moved,
    /// counts once, upheld. With the dedupe off it counts twice: the case needs the dedupe.
    #[test]
    fn one_root_cause_counts_once_across_landings() {
        let rows = [
            row("a", "l1", Standing::Refuted, 1),
            row("a", "l2", Standing::Confirmed, 2),
            row("a", "l3", Standing::Unverified, 3),
        ];
        let once = |f: &SpecialityFold| f.findings == 1 && f.upheld == 1 && f.refuted == 0;
        assert!(once(&harness(&rows)));
        let off = fold_with(&rows, false).remove("dark-elf-harness").unwrap();
        assert!(
            !once(&off),
            "without the dedupe the root cause counts thrice"
        );
        // Two specialities keep their own records of one fingerprint.
        let mut other = row("a", "l1", Standing::Refuted, 4);
        other.speciality = "dark-elf-agent".into();
        let all = fold_findings(&[row("a", "l1", Standing::Confirmed, 1), other]);
        assert_eq!(all["dark-elf-agent"].refuted, 1);
        assert_eq!(all["dark-elf-harness"].upheld, 1);
    }

    /// The human's word outranks every verdict, the latest standing; a held critical is neither
    /// until it is decided.
    #[test]
    fn the_human_decides_held_and_waived_findings() {
        let f = harness(&[row("a", "l1", Standing::Held, 1)]);
        assert_eq!((f.findings, f.held, f.upheld, f.refuted), (1, 1, 0, 0));
        assert_eq!(f.precision(), None);
        // Confirmed by the verifier, dismissed by the human as a false positive: refuted.
        let f = harness(&[
            row("a", "l1", Standing::Confirmed, 1),
            row("a", "l1", Standing::Dismissed, 2),
            row("a", "l2", Standing::Confirmed, 3),
        ]);
        assert_eq!((f.upheld, f.refuted), (0, 1));
        // The human's later word stands over an earlier one.
        let f = harness(&[
            row("a", "l1", Standing::Upheld, 5),
            row("a", "l2", Standing::Dismissed, 2),
        ]);
        assert_eq!((f.upheld, f.refuted), (1, 0));
        let f = harness(&[
            row("a", "l1", Standing::Dismissed, 2),
            row("a", "l2", Standing::Upheld, 5),
        ]);
        assert_eq!((f.upheld, f.refuted), (1, 0));
        // A held critical over a refutation elsewhere waits on the human.
        let f = harness(&[
            row("a", "l1", Standing::Refuted, 1),
            row("a", "l2", Standing::Held, 2),
            row("b", "l2", Standing::Unverified, 2),
        ]);
        assert_eq!((f.held, f.refuted, f.unverified), (1, 0, 1));
    }

    /// The last ten are the latest settled, by the record that settled them; a fall is proposed
    /// under 70 only on a full ten.
    #[test]
    fn a_fall_needs_a_full_last_ten() {
        // Nine settled, six refuted: precision 33 — not a full record, it holds.
        let mut rows: Vec<FindingRow> = (0..9u64)
            .map(|i| {
                let s = if i < 6 {
                    Standing::Refuted
                } else {
                    Standing::Confirmed
                };
                row(&format!("f{i}"), "l1", s, 100 + i)
            })
            .collect();
        assert_eq!(harness(&rows).proposal(Race::ArchDarkElf), None);
        // A tenth: a full record under the bar.
        rows.push(row("f9", "l1", Standing::Refuted, 109));
        let f = harness(&rows);
        assert_eq!(f.recent.len(), WINDOW);
        assert_eq!(f.proposal(Race::ArchDarkElf), Some(Ascension::Probation));
        // Older settled findings fall out of the window: ten fresh confirmations hold the form.
        for i in 0..10u64 {
            rows.push(row(&format!("g{i}"), "l2", Standing::Confirmed, 200 + i));
        }
        let f = harness(&rows);
        assert_eq!(f.recent, vec![true; WINDOW]);
        assert_eq!(f.recent_precision(), Some(100.0));
        assert_eq!(f.proposal(Race::ArchDarkElf), None);
        // Seven of ten upheld is at the bar: it holds.
        let mut seven: Vec<FindingRow> = (0..10u64)
            .map(|i| {
                let s = if i < 7 {
                    Standing::Confirmed
                } else {
                    Standing::Refuted
                };
                row(&format!("h{i}"), "l1", s, i)
            })
            .collect();
        assert_eq!(harness(&seven).proposal(Race::ArchDarkElf), None);
        seven[0].standing = Standing::Refuted;
        assert_eq!(
            harness(&seven).proposal(Race::ArchDarkElf),
            Some(Ascension::Probation)
        );
    }

    /// An Arch Dark Elf cannot stall its fall with findings no verifier can settle: past half
    /// `cannot-verify` over its last ten findings, probation is proposed — only on a full ten.
    #[test]
    fn a_cannot_verify_heavy_record_proposes_probation() {
        // Four settled, all upheld; six cannot-verify: the precision window is not full.
        let mut rows: Vec<FindingRow> = (0..10u64)
            .map(|i| {
                let s = if i < 4 {
                    Standing::Confirmed
                } else {
                    Standing::CannotVerify
                };
                row(&format!("f{i}"), "l1", s, i)
            })
            .collect();
        let f = harness(&rows);
        assert_eq!((f.last_filed, f.last_cannot_verify), (10, 6));
        assert_eq!(f.cannot_verify_rate(), Some(60.0));
        assert_eq!(
            f.last().0.findings,
            4,
            "the settled window alone would hold"
        );
        assert_eq!(f.proposal(Race::ArchDarkElf), Some(Ascension::Probation));
        // Half is at the bar: it holds.
        rows[4].standing = Standing::Confirmed;
        assert_eq!(harness(&rows).proposal(Race::ArchDarkElf), None);
        // Nine findings, six of them cannot-verify: not a full ten, it holds.
        rows[4].standing = Standing::CannotVerify;
        rows.remove(0);
        assert_eq!(
            harness(&rows).cannot_verify_rate().map(f64::round),
            Some(67.0)
        );
        assert_eq!(harness(&rows).proposal(Race::ArchDarkElf), None);
        // Older findings fall out of the last ten: ten fresh confirmations hold the form.
        for i in 0..10u64 {
            rows.push(row(&format!("g{i}"), "l2", Standing::Confirmed, 100 + i));
        }
        let f = harness(&rows);
        assert_eq!((f.last_filed, f.last_cannot_verify), (10, 0));
        assert_eq!(f.proposal(Race::ArchDarkElf), None);
        assert_eq!(harness(&[]).cannot_verify_rate(), None);
    }

    /// A rise is proposed at the bars — twenty findings, upheld 80, precision 85 — never short.
    #[test]
    fn a_rise_is_proposed_at_the_bars() {
        // 20 findings: 17 upheld, 3 refuted — upheld 85, precision 85.
        let mut rows: Vec<FindingRow> = (0..20u64)
            .map(|i| {
                let s = if i < 17 {
                    Standing::Confirmed
                } else {
                    Standing::Refuted
                };
                row(&format!("f{i}"), "l1", s, i)
            })
            .collect();
        let f = harness(&rows);
        assert_eq!(f.proposal(Race::DarkElf), Some(Ascension::Rise));
        assert_eq!(f.meter().label, "Upheld");
        // An Arch Dark Elf is not proposed a rise; another race nothing.
        assert_eq!(f.proposal(Race::ArchDarkElf), None);
        assert_eq!(f.proposal(Race::Orc), None);
        // One refuted more: precision 80, under its bar.
        rows[0].standing = Standing::Refuted;
        assert_eq!(harness(&rows).proposal(Race::DarkElf), None);
        // Nineteen: under the record.
        rows[0].standing = Standing::Confirmed;
        rows.pop();
        assert_eq!(harness(&rows).proposal(Race::DarkElf), None);
    }
}
