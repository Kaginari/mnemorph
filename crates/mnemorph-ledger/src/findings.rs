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
//! - **The build**: each row carries the build of the record that settled it ([`Build`]); a fold
//!   counts its findings per build, and [`fold_findings_by_build`] folds each build's rows on
//!   their own, so a rate earned under old rules is told apart. The scoring reads every build.
//!
//! A rise or a fall is only ever proposed here ([`Ascension`]): applying one is the human's.

use std::collections::BTreeMap;

use mnemorph_races::{
    Explanation, Kind, Meter, Op, Race, Rate, Recent, Recover, Scale, Signals, explain_fall,
    explain_rise, meter, rerise, rises,
};

use crate::Build;

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
    /// The order of the record that settled it there: a later record is greater. The challenge
    /// records number it by line, from 1, as their hash chain counts lines.
    pub at: u64,
    /// The build that wrote the record that settled it there; `unknown` for a record written
    /// before records were stamped.
    pub build: Build,
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
    /// Findings per build of the record that settled each: which rules the record was earned
    /// under. Shown, never scored apart.
    pub builds: BTreeMap<Build, u32>,
    /// The records' first and last row ([`FindingRow::at`]) of every finding folded: the window a
    /// rise reads.
    pub rows: Option<(u64, u64)>,
    /// The records' first and last row of [`SpecialityFold::recent`]: the
    /// window a fall reads.
    pub recent_rows: Option<(u64, u64)>,
    /// The records' first and last row of the last [`WINDOW`] findings filed: the window the
    /// `cannot-verify` bar reads.
    pub filed_rows: Option<(u64, u64)>,
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

/// A proposal and why: one line per bar ([`Explanation`]), each carrying the records' head digest
/// and the rows of the window it read, so the decision can be recomputed from the records.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposed {
    /// What is proposed.
    pub ascension: Ascension,
    /// Why: for a rise, every bar of it; for probation, each bar that failed.
    pub why: Vec<Explanation>,
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

    /// Precision as its counts, for its interval: upheld of upheld and refuted.
    #[must_use]
    pub fn precision_rate(&self) -> Rate {
        Rate::of(self.upheld, self.upheld + self.refuted)
    }

    /// The precision over [`SpecialityFold::recent`] as its counts.
    #[must_use]
    pub fn recent_rate(&self) -> Rate {
        let upheld = self.recent.iter().filter(|u| **u).count();
        Rate::of(
            u32::try_from(upheld).unwrap_or(u32::MAX),
            u32::try_from(self.recent.len()).unwrap_or(u32::MAX),
        )
    }

    /// The `cannot-verify` rate over the last ten findings as its counts.
    #[must_use]
    pub fn cannot_verify_counts(&self) -> Rate {
        Rate::of(self.last_cannot_verify, self.last_filed)
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
        self.proposed(form, None).map(|p| p.ascension)
    }

    /// [`SpecialityFold::proposal`], explained: each line names its bar, its reading and the
    /// rows of the window it read, with `head`, the records' chain head digest when the fold was
    /// read.
    #[must_use]
    pub fn proposed(&self, form: Race, head: Option<&str>) -> Option<Proposed> {
        match form {
            Race::DarkElf => rises(Race::ArchDarkElf, &self.signals()).then(|| Proposed {
                ascension: Ascension::Rise,
                // A rise reads the whole record.
                why: explain_rise(Race::ArchDarkElf, &self.signals())
                    .into_iter()
                    .map(|e| e.on_ledger(head, self.rows))
                    .collect(),
            }),
            Race::ArchDarkElf => {
                let mut why: Vec<Explanation> = explain_fall(Race::ArchDarkElf, &self.last())
                    .into_iter()
                    .map(|e| e.on_ledger(head, self.recent_rows))
                    .collect();
                why.extend(self.stalled().map(|e| e.on_ledger(head, self.filed_rows)));
                (!why.is_empty()).then_some(Proposed {
                    ascension: Ascension::Probation,
                    why,
                })
            }
            _ => None,
        }
    }

    /// The `cannot-verify` bar over a full last ten findings, when it is past: probation.
    fn stalled(&self) -> Option<Explanation> {
        let rate = self.cannot_verify_rate()?;
        let e = Explanation {
            kind: Kind::Probation,
            meter: "cannotVerifyRate",
            scale: Scale::Rate,
            reading: rate,
            op: Op::Gt,
            threshold: CANNOT_VERIFY_BAR,
            window: self.last_filed,
            unit: "findings",
            recover: Some(Recover {
                op: Op::Le,
                threshold: CANNOT_VERIFY_BAR,
                within: mnemorph_races::RECOVER_REPORTS,
                unit: "findings",
            }),
            rerise: rerise(Race::ArchDarkElf, "cannotVerifyRate"),
            head: None,
            from: None,
            to: None,
        };
        (self.last_filed as usize >= WINDOW && e.holds()).then_some(e)
    }
}

/// `rows` folded per speciality, each root cause counted once.
#[must_use]
pub fn fold_findings(rows: &[FindingRow]) -> BTreeMap<String, SpecialityFold> {
    fold_with(rows, true)
}

/// `rows` grouped by the build of the record that settled each, and each build's rows folded on
/// their own: a rate earned under one build's rules, apart from another's. A root cause settled
/// under two builds counts in both.
#[must_use]
pub fn fold_findings_by_build(
    rows: &[FindingRow],
) -> BTreeMap<Build, BTreeMap<String, SpecialityFold>> {
    let mut by: BTreeMap<&Build, Vec<FindingRow>> = BTreeMap::new();
    for r in rows {
        by.entry(&r.build).or_default().push(r.clone());
    }
    by.into_iter()
        .map(|(b, rows)| (b.clone(), fold_findings(&rows)))
        .collect()
}

/// The fold; `dedupe` off counts every landing's row: how a test shows the dedupe is needed.
fn fold_with(rows: &[FindingRow], dedupe: bool) -> BTreeMap<String, SpecialityFold> {
    // Per speciality, per root cause: its standing, the record that settled it and its build.
    let mut causes: BTreeMap<(&str, String), (Standing, u64, &Build)> = BTreeMap::new();
    for (n, r) in rows.iter().enumerate() {
        let key = if dedupe {
            r.fingerprint.clone()
        } else {
            format!("{}#{n}", r.fingerprint)
        };
        let now = (r.standing, r.at, &r.build);
        causes
            .entry((r.speciality.as_str(), key))
            .and_modify(|was| *was = stronger(*was, now))
            .or_insert(now);
    }
    let mut out: BTreeMap<String, SpecialityFold> = BTreeMap::new();
    let mut settled: BTreeMap<&str, Vec<(u64, bool)>> = BTreeMap::new();
    let mut filed: BTreeMap<&str, Vec<(u64, bool)>> = BTreeMap::new();
    for ((speciality, _), (standing, at, build)) in causes {
        filed
            .entry(speciality)
            .or_default()
            .push((at, standing == Standing::CannotVerify));
        let f = out.entry(speciality.to_owned()).or_default();
        f.findings += 1;
        *f.builds.entry(build.clone()).or_default() += 1;
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
            f.recent_rows = span(&s[from..]);
        }
    }
    for (speciality, mut s) in filed {
        s.sort_by_key(|(at, _)| *at);
        let last = &s[s.len().saturating_sub(WINDOW)..];
        if let Some(f) = out.get_mut(speciality) {
            f.rows = span(&s);
            f.last_filed = u32::try_from(last.len()).unwrap_or(u32::MAX);
            f.last_cannot_verify =
                u32::try_from(last.iter().filter(|(_, nv)| *nv).count()).unwrap_or(u32::MAX);
            f.filed_rows = span(last);
        }
    }
    out
}

/// The first and last row of a window sorted by row.
fn span<T>(window: &[(u64, T)]) -> Option<(u64, u64)> {
    Some((window.first()?.0, window.last()?.0))
}

/// Of two standings of one root cause, the one that stands: the human's latest word, else the
/// strongest verdict, the earlier record on a tie.
fn stronger<'a>(
    a: (Standing, u64, &'a Build),
    b: (Standing, u64, &'a Build),
) -> (Standing, u64, &'a Build) {
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
            build: Build::default(),
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

    /// A proposal carries one line per bar, with the head and the rows of the window it read;
    /// the rows inside that window, folded again, give the same reading and the same decision.
    #[test]
    fn a_proposal_is_explained_and_recomputes_from_its_window() {
        // Older confirmations, then ten settled: six refuted. Root cause `x` has rows on both
        // sides of the window's first: confirmed before it, refuted inside it — it stands
        // confirmed, before the window.
        let mut rows: Vec<FindingRow> = (0..5u64)
            .map(|i| row(&format!("old{i}"), "l0", Standing::Confirmed, 1 + i))
            .collect();
        rows.extend((0..10u64).map(|i| {
            let s = if i < 6 {
                Standing::Refuted
            } else {
                Standing::Confirmed
            };
            row(&format!("f{i}"), "l1", s, 20 + 2 * i)
        }));
        rows.push(row("x", "l0", Standing::Confirmed, 8));
        rows.push(row("x", "l1", Standing::Refuted, 25));
        let f = harness(&rows);
        let p = f.proposed(Race::ArchDarkElf, Some("ab12")).unwrap();
        assert_eq!(p.ascension, Ascension::Probation);
        assert_eq!(p.why.len(), 1);
        let e = &p.why[0];
        assert_eq!(
            e.to_string(),
            "probation: precision 0.40 < 0.70 (last 10 findings); recover: >= 0.70 within 5 \
             findings; re-rise: precision >= 0.85 over 20 findings; ledger head ab12, rows 20–38"
        );
        // Recomputed from the records up to the window's last row: the same reading, the same
        // decision. The rows inside the window alone would not do: `x`'s refutation there would
        // stand once its earlier confirmation is left out.
        let (from, to) = (e.from.unwrap(), e.to.unwrap());
        let upto: Vec<FindingRow> = rows.iter().filter(|r| r.at <= to).cloned().collect();
        let again = harness(&upto);
        assert_eq!(again.recent_precision(), Some(e.reading));
        assert!(e.op.holds(e.reading, e.threshold));
        assert_eq!(
            again.proposed(Race::ArchDarkElf, Some("ab12")),
            Some(p.clone())
        );
        let inside: Vec<FindingRow> = rows
            .iter()
            .filter(|r| (from..=to).contains(&r.at))
            .cloned()
            .collect();
        assert_ne!(
            harness(&inside).proposed(Race::ArchDarkElf, Some("ab12")),
            Some(p.clone())
        );
        assert_eq!(f.proposal(Race::ArchDarkElf), Some(Ascension::Probation));
        // A stalled record names the cannot-verify bar and the rows of the last ten filed.
        let stalled: Vec<FindingRow> = (0..10u64)
            .map(|i| {
                let s = if i < 4 {
                    Standing::Confirmed
                } else {
                    Standing::CannotVerify
                };
                row(&format!("c{i}"), "l1", s, 40 + i)
            })
            .collect();
        let p = harness(&stalled).proposed(Race::ArchDarkElf, None).unwrap();
        assert_eq!(
            p.why[0].to_string(),
            "probation: cannotVerifyRate 0.60 > 0.50 (last 10 findings); recover: <= 0.50 within \
             5 findings; re-rise: upheldRate >= 0.80 over 20 findings; ledger unchained, rows \
             40–49"
        );
        // A rise names every bar, over the whole record's rows.
        let rise: Vec<FindingRow> = (0..20u64)
            .map(|i| row(&format!("r{i}"), "l1", Standing::Confirmed, 3 + i))
            .collect();
        let p = harness(&rise)
            .proposed(Race::DarkElf, Some("cd34"))
            .unwrap();
        assert_eq!(p.ascension, Ascension::Rise);
        assert_eq!(p.why.len(), 3);
        assert!(p.why.iter().all(|e| (e.from, e.to) == (Some(3), Some(22))));
        assert!(p.why.iter().all(Explanation::holds));
        assert_eq!(harness(&rise).proposed(Race::Orc, None), None);
        assert_eq!(harness(&rise[..5]).proposed(Race::DarkElf, None), None);
    }

    /// Each row carries its build; old rows read as `unknown`. The fold counts findings per the
    /// build that settled each, and a fold by build keeps each build's rates apart — the
    /// whole-record score unchanged.
    #[test]
    fn a_fold_groups_by_build() {
        let new = Build {
            version: "0.2.0".into(),
            commit: "a1b2c3d4e5".into(),
        };
        let mut rows = vec![
            row("a", "l1", Standing::Refuted, 1),
            row("b", "l1", Standing::Refuted, 2),
        ];
        // Root cause `a` is settled again, under the new build; `c` and `d` are new.
        for (fp, at) in [("a", 3), ("c", 4), ("d", 5)] {
            rows.push(FindingRow {
                build: new.clone(),
                ..row(fp, "l2", Standing::Confirmed, at)
            });
        }
        let all = harness(&rows);
        assert_eq!((all.upheld, all.refuted), (3, 1));
        assert_eq!(all.builds[&Build::default()], 1);
        assert_eq!(all.builds[&new], 3);
        let by = fold_findings_by_build(&rows);
        assert_eq!(by.len(), 2);
        let old = &by[&Build::default()]["dark-elf-harness"];
        let now = &by[&new]["dark-elf-harness"];
        assert_eq!((old.precision(), now.precision()), (Some(0.0), Some(100.0)));
        // The counts behind each shown rate.
        assert_eq!(all.precision_rate(), Rate::of(3, 4));
        assert_eq!(all.recent_rate(), Rate::of(3, 4));
        assert_eq!(all.cannot_verify_counts(), Rate::of(0, 4));
        assert_eq!(all.meter().rate, Rate::of(3, 4));
    }
}
