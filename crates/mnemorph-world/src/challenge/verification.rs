//! Independent verification of a challenge's findings: before a finding counts, blocks or is
//! proposed as a seal, a fresh verifier — a Dark Elf of another speciality than its finder, never
//! the finder, never the landing's author nor a body above it — is given the record alone and
//! tries to refute it.
//!
//! - A valid finding of [`FLOOR`] or higher is kept as [`Found`], awaiting its verdict. Below the
//!   floor it is not verified, and never counts, blocks or seals.
//! - Its verifier is recorded by [`Challenges::assign`]; only that body files its verdict.
//! - The verdict ([`Ruling`]) is `confirmed`, with the verifier's own re-run of the witness
//!   (the severity may be lowered, never raised); `refuted`, with a reason and its evidence; or
//!   `cannot-verify`, with the blocker. Each [`Guard`] a filing breaks is named in the refusal.
//! - **Counting:** only a confirmed finding counts. **Blocking:** a critical finding blocks the
//!   landing whatever one verifier says — refuted, lowered, unverified or `cannot-verify` alike,
//!   for the human to settle by the fix or a waiver — and a high one once confirmed at high.
//! - **Seals:** a finding confirmed at high or critical may be proposed as a seal, with its
//!   witness frozen by hash ([`Challenges::propose`]); nothing here applies one — a seal is the
//!   human's.
//! - Every confirmed and refuted verdict is a row a precision fold reads per finder speciality
//!   ([`Challenges::outcomes`]); every finding, with its verdict and the human's decision on it
//!   ([`Decision`]), is a row of its speciality's value ledger ([`Challenges::finding_rows`]).

use std::fmt;

use mnemorph_proving::coverage::Ran;
use mnemorph_proving::{FindingRecord, Severity, Witness, WitnessKind, World, sha256_hex};
use mnemorph_races::Speciality;
use serde::{Deserialize, Serialize};

use super::{Blocker, Challenges, HUMAN, Open, Opening, Refusal, Row, Staff, Status, reasoned};
use mnemorph_ledger::{FindingRow, Standing};

/// The lowest severity a finding is verified at. Medium is where the contract first asks a
/// finding's preconditions and reachability — what a verifier can try to refute — and where it
/// starts to weigh: a confirmed medium counts, a high or critical blocks. A low or informational
/// finding is never verified, and so never counts, blocks or seals: verifying it would spend the
/// challenge reserve on what cannot move a landing.
pub const FLOOR: Severity = Severity::Medium;

/// The most re-runs, or files, one verdict cites.
pub const MAX_EVIDENCE: usize = 16;

/// A valid finding of [`FLOOR`] or higher, kept with its challenge while it waits on its verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Found {
    /// The body whose report it was: never its verifier.
    pub finder: String,
    /// Where its record was read: the `@F`'s `rec:`.
    pub rec: String,
    /// The record, as it validated on the landing's tree.
    pub record: FindingRecord,
}

impl Found {
    /// Its root-cause fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.record.fingerprint
    }

    /// Its speciality: the finder's.
    #[must_use]
    pub fn speciality(&self) -> Option<Speciality> {
        Speciality::of(&self.record.speciality)
    }

    /// The severity it was found at.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.record.severity.unwrap_or(Severity::Informational)
    }
}

/// A command the verifier ran, as its own run journal shows it: the hash is of the output the
/// harness read there, never one the verifier wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rerun {
    /// The command, exactly as run.
    pub run: String,
    /// The SHA-256 of its output, stamped from the journal; empty when the journal holds none.
    #[serde(default)]
    pub sha256: String,
    /// Whether that run failed — a non-zero exit, a signal, a timeout — stamped from the journal.
    #[serde(default, skip_serializing_if = "is_false")]
    pub failed: bool,
}

// Serde's `skip_serializing_if` passes the field by reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !*b
}

/// A file the verifier read, at the hash it stands at in the landing's tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pinned {
    /// Its repository path.
    pub path: String,
    /// Its SHA-256.
    pub sha256: String,
}

/// A verifier's verdict on one finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Ruling {
    /// Re-derived: the finding counts.
    Confirmed {
        /// The severity as it stands: the finder's, or lower — never higher.
        severity: Severity,
        /// Why it is lower than the finder's: required when it is.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        reason: String,
        /// The verifier's own re-run of the witness.
        reran: Vec<Rerun>,
    },
    /// Refuted: the finding is demoted to `@?` with the reason.
    Refuted {
        /// Why it does not hold.
        reason: String,
        /// The files that show it.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<Pinned>,
        /// The commands that show it.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        reran: Vec<Rerun>,
    },
    /// Neither: it stays pending verification and never counts.
    CannotVerify {
        /// What stopped the verifier.
        blocker: String,
    },
}

impl Ruling {
    /// Its word: `confirmed`, `refuted`, `cannot-verify`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Ruling::Confirmed { .. } => "confirmed",
            Ruling::Refuted { .. } => "refuted",
            Ruling::CannotVerify { .. } => "cannot-verify",
        }
    }
}

/// A filed verdict, as the challenge keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    /// The finding's fingerprint.
    pub fingerprint: String,
    /// The verifier, by body.
    pub verifier: String,
    /// The verifier's speciality, by its creature name.
    pub speciality: String,
    /// The finder, by body.
    pub finder: String,
    /// The finder's speciality, by its creature name: what a precision fold reads per speciality.
    pub finder_speciality: String,
    /// The landing's tree the verdict was filed on: the one its challenge was opened on.
    #[serde(default)]
    pub tree: String,
    /// The witness's SHA-256 when it was verified.
    pub witness_sha256: String,
    /// The witness's captured output's SHA-256, when it has one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub output_sha256: String,
    /// The verdict.
    pub ruling: Ruling,
}

/// A proposed seal: a finding confirmed at high or critical, its witness frozen by hash. It is
/// the human's to confirm; nothing in the harness applies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// The finding's fingerprint.
    pub fingerprint: String,
    /// The finder's speciality.
    pub speciality: String,
    /// The root cause's file: the territory the seal would hold.
    pub file: String,
    /// The root cause's symbol.
    pub symbol: String,
    /// The severity as confirmed.
    pub severity: Severity,
    /// The witness, frozen: its hash is what the finding is judged by from now on.
    pub witness: Witness,
    /// Who confirmed it.
    pub verifier: String,
    /// Who proposed it to the human: the throne.
    pub by: String,
}

/// A finding of the challenge and where its verification stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The finding.
    pub found: Found,
    /// Its verifier's staffing, once planned: the latest stands.
    pub staff: Option<Staff>,
    /// Its verdict, once filed.
    pub verdict: Option<Verification>,
    /// Its seal proposal, once made.
    pub proposal: Option<Proposal>,
    /// The human's decision on it, once given: the latest stands.
    pub decision: Option<Decision>,
}

impl Finding {
    pub(super) fn new(found: Found) -> Finding {
        Finding {
            found,
            staff: None,
            verdict: None,
            proposal: None,
            decision: None,
        }
    }

    /// Where it stands in its speciality's value ledger: the human's decision when given, else
    /// its verdict — a critical its verifier refuted is held for the human, neither upheld nor
    /// refuted until decided.
    #[must_use]
    pub fn standing(&self) -> Standing {
        if let Some(d) = &self.decision {
            return if d.upheld {
                Standing::Upheld
            } else {
                Standing::Dismissed
            };
        }
        match self.verdict.as_ref().map(|v| &v.ruling) {
            None => Standing::Unverified,
            Some(Ruling::Confirmed { .. }) => Standing::Confirmed,
            Some(Ruling::CannotVerify { .. }) => Standing::CannotVerify,
            Some(Ruling::Refuted { .. }) if self.found.severity() == Severity::Critical => {
                Standing::Held
            }
            Some(Ruling::Refuted { .. }) => Standing::Refuted,
        }
    }

    /// Its fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        self.found.fingerprint()
    }

    /// The severity as it stands: the confirmed one, else as found.
    #[must_use]
    pub fn severity(&self) -> Severity {
        match self.verdict.as_ref().map(|v| &v.ruling) {
            Some(Ruling::Confirmed { severity, .. }) => *severity,
            _ => self.found.severity(),
        }
    }

    /// Whether it counts: only once confirmed.
    #[must_use]
    pub fn counts(&self) -> bool {
        matches!(
            self.verdict.as_ref().map(|v| &v.ruling),
            Some(Ruling::Confirmed { .. })
        )
    }

    /// Whether it blocks the landing: a critical finding always — whatever one verifier says, it
    /// is the human's to settle, by the fix or a waiver — and any other once confirmed at high.
    #[must_use]
    pub fn blocks(&self) -> bool {
        self.found.severity() == Severity::Critical
            || matches!(
                self.verdict.as_ref().map(|v| &v.ruling),
                Some(Ruling::Confirmed { severity, .. }) if *severity >= Severity::High
            )
    }

    /// Whether it still waits on a verdict.
    #[must_use]
    pub fn unverified(&self) -> bool {
        self.verdict.is_none()
    }

    /// It as a blocker, with where its verification stands.
    pub(super) fn blocker(&self) -> Blocker {
        let held = self.severity() < Severity::High;
        let standing = match &self.verdict {
            Some(v) => match &v.ruling {
                Ruling::Confirmed { severity, .. } if held => format!(
                    "critical, confirmed by {} at {}: the human decides",
                    v.verifier,
                    severity.word()
                ),
                Ruling::Confirmed { .. } => format!("confirmed by {}", v.verifier),
                Ruling::CannotVerify { blocker } => {
                    format!("critical, cannot be verified ({blocker}): the human decides")
                }
                Ruling::Refuted { reason, .. } => format!(
                    "critical, refuted by {}: {reason} — the human decides",
                    v.verifier
                ),
            },
            None => "critical, unverified: the human decides".to_owned(),
        };
        // A critical held for the human reads as found; a confirmed one at its confirmed severity.
        let severity = if held {
            self.found.severity()
        } else {
            self.severity()
        };
        Blocker {
            speciality: self.found.record.speciality.clone(),
            fingerprint: self.fingerprint().to_owned(),
            severity: severity.word().to_owned(),
            claim: self.found.record.claim.clone(),
            standing,
        }
    }
}

/// The human's decision on one finding, for its speciality's value ledger: it holds — confirmed,
/// or proven by its fix — and counts upheld, or it is a false positive and counts refuted. It lifts
/// no block: a waiver or the fix does that. Only [`Decision::human`] makes one, and the records
/// keep it only with a reason that says something, bound to the tree the challenge was opened on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    by: String,
    upheld: bool,
    reason: String,
    #[serde(default)]
    tree: String,
}

impl Decision {
    /// The human's decision: `upheld`, or a false positive. Refused inside a body's command
    /// (`MNEMORPH_BODY` is set there): a body never scores a finding.
    ///
    /// # Errors
    ///
    /// A body asks, or the reason says nothing (under 20 characters, a stock phrase, an
    /// unreadable character, over 1 KiB).
    pub fn human(upheld: bool, reason: &str) -> Result<Decision, Refusal> {
        if let Some(body) = std::env::var_os("MNEMORPH_BODY") {
            return Err(Refusal::NotHuman(body.to_string_lossy().into_owned()));
        }
        let reason = reason.trim();
        if !reasoned(reason) {
            return Err(Refusal::Unreasoned);
        }
        Ok(Decision {
            by: HUMAN.to_owned(),
            upheld,
            reason: reason.to_owned(),
            tree: String::new(),
        })
    }

    /// Whether the human upheld it; else it is a false positive.
    #[must_use]
    pub fn upheld(&self) -> bool {
        self.upheld
    }

    /// Why.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Whether the records keep it for a challenge opened on `tree`.
    fn holds(&self, tree: &str) -> bool {
        self.by == HUMAN && reasoned(&self.reason) && self.tree == tree
    }
}

/// One confirmed or refuted verdict: the row a precision fold reads, per finder speciality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The landing.
    pub landing: String,
    /// The finding's fingerprint.
    pub fingerprint: String,
    /// The finder's speciality, by its creature name.
    pub finder_speciality: String,
    /// The finder, by body.
    pub finder: String,
    /// The verifier, by body.
    pub verifier: String,
    /// Whether it was confirmed; else refuted.
    pub confirmed: bool,
}

/// A rule of verification. A filing that breaks any is refused, every broken rule named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Guard {
    /// A finding is kept for verification only at [`FLOOR`] or higher.
    Floor,
    /// A finding is its finder's own speciality's.
    Finder,
    /// The verdict is filed by the body assigned to verify that finding.
    Assigned,
    /// The verifier is a Dark Elf of another speciality than the finder, never the finder, never
    /// the landing's author nor a body above it.
    Independent,
    /// A finding takes one verdict.
    Once,
    /// The verdict is filed on the landing's tree as its challenge was opened on.
    Current,
    /// A confirmation never raises the severity.
    NoRaise,
    /// A confirmation cites the verifier's own re-run of the witness, shown in its journal: a
    /// command witness's own command, or a command naming a test's or scanner output's file that
    /// failed as the witness shows.
    Rederived,
    /// A witness with captured output is confirmed only by a re-run that printed that output.
    Reproduced,
    /// A confirmed witness is the one found: its file still hashes as the record says.
    Unchanged,
    /// A refutation says why, a `cannot-verify` its blocker, and a confirmation that lowers the
    /// severity why: at least 20 characters, not a stock phrase.
    Reasoned,
    /// A refutation shows its evidence; every cited file stands at its hash in the tree, every
    /// command in the verifier's journal; no more than [`MAX_EVIDENCE`] of each.
    Evidenced,
    /// A seal is proposed only for a finding confirmed at high or critical, once.
    Sealable,
}

impl Guard {
    /// Every guard, in the order they are reported.
    pub const ALL: [Guard; 13] = [
        Guard::Floor,
        Guard::Finder,
        Guard::Assigned,
        Guard::Independent,
        Guard::Once,
        Guard::Current,
        Guard::NoRaise,
        Guard::Rederived,
        Guard::Reproduced,
        Guard::Unchanged,
        Guard::Reasoned,
        Guard::Evidenced,
        Guard::Sealable,
    ];

    /// Its name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Guard::Floor => "floor",
            Guard::Finder => "finder",
            Guard::Assigned => "assigned",
            Guard::Independent => "independent",
            Guard::Once => "once",
            Guard::Current => "current",
            Guard::NoRaise => "no-raise",
            Guard::Rederived => "rederived",
            Guard::Reproduced => "reproduced",
            Guard::Unchanged => "unchanged",
            Guard::Reasoned => "reasoned",
            Guard::Evidenced => "evidenced",
            Guard::Sealable => "sealable",
        }
    }

    /// What it asks, said to the body refused.
    #[must_use]
    pub fn asks(self) -> &'static str {
        match self {
            Guard::Floor => "only a finding of medium or higher is verified",
            Guard::Finder => "a finding is its finder's own speciality's",
            Guard::Assigned => "you were not assigned to verify this finding",
            Guard::Independent => {
                "a verifier is another speciality than the finder, never the finder, never the \
                 landing's author nor a body above it"
            }
            Guard::Once => "this finding has its verdict already",
            Guard::Current => {
                "the landing's tree is no longer the one its challenge was opened on: the verdict \
                 would judge other code"
            }
            Guard::NoRaise => "a confirmation may lower the severity, never raise it",
            Guard::Rederived => {
                "a confirmation cites a command you ran through `bash` that re-runs the witness: a \
                 command witness exactly as recorded, or a command naming the test's or the \
                 scanner output's file that fails as the witness shows"
            }
            Guard::Reproduced => {
                "the witness captured its output: a confirmation re-runs its command to that same \
                 output"
            }
            Guard::Unchanged => {
                "the witness no longer hashes as the record says: what was found is not what is \
                 there"
            }
            Guard::Reasoned => {
                "a refutation says why, a `cannot-verify` its blocker, and a lowered severity its \
                 reason: at least 20 characters, not a stock phrase"
            }
            Guard::Evidenced => {
                "a refutation cites a file at its hash or a command you ran; every cited file \
                 stands at its hash and every command is in your journal, at most 16 of each"
            }
            Guard::Sealable => {
                "a seal is proposed once, for a finding confirmed at high or critical"
            }
        }
    }
}

impl fmt::Display for Guard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Where a filing's evidence is checked: the landing's tree and the verifier's journal; `None` on
/// a read of the records, which re-checks only what the rows carry.
pub(super) type Sources<'a> = Option<(&'a dyn World, &'a Ran)>;

/// The guards `found` breaks as a finding of `open`, but `off`.
pub(super) fn found_breaks(found: &Found, off: Option<Guard>) -> Vec<Guard> {
    let mut broken = Vec::new();
    if found.severity() < FLOOR {
        broken.push(Guard::Floor);
    }
    let finder = Speciality::of(&found.finder);
    if finder.is_none() || finder != found.speciality() {
        broken.push(Guard::Finder);
    }
    broken.retain(|g| Some(*g) != off);
    broken
}

/// Whether `body` may verify `finding` on a landing opened by `opening`: a Dark Elf speciality
/// other than the finder's, not the finder, not the author nor a body above it.
pub(super) fn independent(finding: &Finding, opening: &Opening, body: &str) -> bool {
    let Some(theirs) = Speciality::of(body) else {
        return false;
    };
    body != finding.found.finder
        && Some(theirs) != finding.found.speciality()
        && !opening.holds_author(body)
}

/// The guards a staffing of a verifier breaks, but `off`.
pub(super) fn assign_breaks(
    finding: &Finding,
    opening: &Opening,
    staff: &Staff,
    off: Option<Guard>,
) -> Vec<Guard> {
    let mut broken = Vec::new();
    if let Staff::Commissioned { body, .. } = staff
        && !independent(finding, opening, body)
    {
        broken.push(Guard::Independent);
    }
    // Once ruled, a finding takes no other verifier: only its own verifier's spend is booked.
    let own = matches!(
        (staff, &finding.verdict),
        (Staff::Commissioned { body, .. }, Some(v)) if *body == v.verifier
    );
    if finding.verdict.is_some() && !own {
        broken.push(Guard::Once);
    }
    broken.retain(|g| Some(*g) != off);
    broken
}

/// The guards `v` breaks as the verdict on `finding`, but `off`; its evidence held to `sources`
/// when given.
pub(super) fn verdict_breaks(
    finding: &Finding,
    opening: &Opening,
    v: &Verification,
    sources: Sources<'_>,
    off: Option<Guard>,
) -> Vec<Guard> {
    let record = &finding.found.record;
    let assigned = matches!(
        &finding.staff,
        Some(Staff::Commissioned { body, .. }) if *body == v.verifier
    );
    let mut broken = Vec::new();
    let mut flag = |g: Guard, broke: bool| {
        if broke && Some(g) != off && !broken.contains(&g) {
            broken.push(g);
        }
    };
    flag(Guard::Assigned, !assigned);
    flag(
        Guard::Independent,
        !independent(finding, opening, &v.verifier)
            || Speciality::of(&v.verifier).map(Speciality::creature) != Some(v.speciality.as_str()),
    );
    flag(Guard::Once, finding.verdict.is_some());
    flag(Guard::Current, v.tree != opening.current_tree());
    let shown = |r: &Rerun| match sources {
        Some((_, ran)) => !r.sha256.is_empty() && ran.shows(&r.run, Some(&r.sha256)),
        None => is_sha256(&r.sha256),
    };
    match &v.ruling {
        Ruling::Confirmed {
            severity,
            reason,
            reran,
        } => {
            flag(Guard::NoRaise, *severity > finding.found.severity());
            flag(
                Guard::Reasoned,
                *severity < finding.found.severity() && !reasoned(reason),
            );
            let witness = record.witness.as_ref();
            let rederives =
                |r: &Rerun| witness.is_some_and(|w| shown(r) && rederives(w, r, sources));
            flag(
                Guard::Rederived,
                reran.is_empty()
                    || reran.len() > MAX_EVIDENCE
                    || !reran.iter().all(shown)
                    || !reran.iter().any(rederives),
            );
            // A command's captured output is reproduced only by its own command printing it again.
            let captured = witness
                .filter(|w| w.kind == WitnessKind::PocCommand)
                .and_then(|w| Some((w.reference.trim(), w.output_sha256.as_deref()?)));
            flag(
                Guard::Reproduced,
                captured.is_some_and(|(command, out)| {
                    !reran
                        .iter()
                        .any(|r| r.run.trim() == command && r.sha256 == out)
                }),
            );
            flag(Guard::Unchanged, !unchanged(witness, v, sources));
        }
        Ruling::Refuted {
            reason,
            files,
            reran,
        } => {
            flag(Guard::Reasoned, !reasoned(reason));
            let pinned = |p: &Pinned| match sources {
                Some((tree, _)) => tree
                    .read(&p.path)
                    .is_some_and(|b| sha256_hex(&b).eq_ignore_ascii_case(&p.sha256)),
                None => is_sha256(&p.sha256),
            };
            flag(
                Guard::Evidenced,
                (files.is_empty() && reran.is_empty())
                    || files.len() > MAX_EVIDENCE
                    || reran.len() > MAX_EVIDENCE
                    || !files.iter().all(pinned)
                    || !reran.iter().all(shown),
            );
        }
        Ruling::CannotVerify { blocker } => flag(Guard::Reasoned, !reasoned(blocker)),
    }
    broken
}

/// Whether `r` re-runs witness `w`: a command witness's own command; for a test or a scanner's
/// output, a command naming its file that failed — the failure the witness shows — as the
/// verifier's journal recorded it.
fn rederives(w: &Witness, r: &Rerun, sources: Sources<'_>) -> bool {
    match w.kind {
        WitnessKind::PocCommand => r.run.trim() == w.reference.trim(),
        WitnessKind::Test | WitnessKind::ScannerOutput => {
            names_path(&r.run, &w.reference)
                && r.failed
                && sources.is_none_or(|(_, ran)| ran.shows_failed(&r.run, &r.sha256))
        }
    }
}

/// Whether `command` names `path` as one of its words, bare, `./`-prefixed or quoted.
fn names_path(command: &str, path: &str) -> bool {
    let path = path.trim_start_matches("./");
    !path.is_empty()
        && command.split_whitespace().any(|word| {
            let word = word.trim_matches(|c| matches!(c, '\'' | '"' | ';' | '(' | ')'));
            word.trim_start_matches("./") == path
        })
}

/// Whether the witness `v` confirms is the one found: the hashes it carries are the record's,
/// and — against the tree — its files still hash so.
fn unchanged(witness: Option<&Witness>, v: &Verification, sources: Sources<'_>) -> bool {
    let Some(w) = witness else {
        return false;
    };
    let carried = v.witness_sha256 == w.sha256
        && v.output_sha256 == w.output_sha256.clone().unwrap_or_default();
    let Some((tree, _)) = sources else {
        return carried;
    };
    let at = |path: &str, sha: &str| tree.read(path).is_some_and(|b| sha256_hex(&b) == sha);
    let content = match w.kind {
        WitnessKind::PocCommand => sha256_hex(w.reference.as_bytes()) == w.sha256,
        WitnessKind::Test | WitnessKind::ScannerOutput => at(&w.reference, &w.sha256),
    };
    let output = match (&w.output, &w.output_sha256) {
        (Some(path), Some(sha)) => at(path, sha),
        _ => true,
    };
    carried && content && output
}

/// The guards a seal proposal on `finding` breaks, but `off`.
pub(super) fn propose_breaks(finding: &Finding, off: Option<Guard>) -> Vec<Guard> {
    let sealable = finding.counts()
        && finding.severity() >= Severity::High
        && finding.proposal.is_none()
        && finding.found.record.witness.is_some();
    if sealable || off == Some(Guard::Sealable) {
        Vec::new()
    } else {
        vec![Guard::Sealable]
    }
}

fn is_sha256(hex: &str) -> bool {
    hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// What a verifier files: the verdict, its commands' hashes left for the harness to stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filing {
    /// The landing.
    pub landing: String,
    /// The finding's fingerprint.
    pub fingerprint: String,
    /// The verifier, by body: the one whose run files it.
    pub verifier: String,
    /// The landing's tree as the harness hashed it at the filing.
    pub tree: String,
    /// The verdict.
    pub ruling: Ruling,
}

impl Open {
    pub(super) fn finding(&self, fingerprint: &str) -> Option<&Finding> {
        self.findings
            .iter()
            .find(|f| f.fingerprint() == fingerprint)
    }

    fn finding_mut(&mut self, fingerprint: &str) -> Option<&mut Finding> {
        self.findings
            .iter_mut()
            .find(|f| f.fingerprint() == fingerprint)
    }

    /// The findings that block, as blockers.
    pub(super) fn blocking(&self) -> Vec<Blocker> {
        self.findings
            .iter()
            .filter(|f| f.blocks())
            .map(Finding::blocker)
            .collect()
    }

    /// The findings still waiting on a verdict, by fingerprint.
    pub(super) fn verifying(&self) -> Vec<String> {
        self.findings
            .iter()
            .filter(|f| f.unverified())
            .map(|f| f.fingerprint().to_owned())
            .collect()
    }

    pub(super) fn fold_found(&mut self, found: Found) -> bool {
        let kept =
            self.finding(found.fingerprint()).is_none() && found_breaks(&found, None).is_empty();
        if kept {
            self.findings.push(Finding::new(found));
        }
        kept
    }

    pub(super) fn fold_assigned(&mut self, fingerprint: &str, staff: Staff) {
        let opening = self.opening.clone();
        if let Some(f) = self.finding_mut(fingerprint)
            && assign_breaks(f, &opening, &staff, None).is_empty()
        {
            f.staff = Some(staff);
        }
    }

    pub(super) fn fold_ruled(&mut self, v: Verification) -> bool {
        let opening = self.opening.clone();
        if let Some(f) = self.finding_mut(&v.fingerprint)
            && v.finder == f.found.finder
            && v.finder_speciality == f.found.record.speciality
            && verdict_breaks(f, &opening, &v, None, None).is_empty()
        {
            f.verdict = Some(v);
            return true;
        }
        false
    }

    pub(super) fn fold_decided(&mut self, fingerprint: &str, d: Decision) -> bool {
        let tree = self.opening.tree.clone();
        if let Some(f) = self.finding_mut(fingerprint)
            && d.holds(&tree)
        {
            f.decision = Some(d);
            return true;
        }
        false
    }

    pub(super) fn fold_proposed(&mut self, p: Proposal) {
        if let Some(f) = self.finding_mut(&p.fingerprint)
            && propose_breaks(f, None).is_empty()
            && Some(&p.witness) == f.found.record.witness.as_ref()
        {
            f.proposal = Some(p);
        }
    }
}

/// `off` switches one guard off: how a test shows that guard is the one its case needs.
pub(super) type Off = Option<Guard>;

impl Challenges {
    /// Keeps `found`, a valid finding of a reviewer's report on `landing`, until its verdict. One
    /// fingerprint is kept once.
    ///
    /// # Errors
    ///
    /// No such open landing; a finding below [`FLOOR`], or not its finder's speciality's; records
    /// that cannot be kept.
    pub fn found(&self, landing: &str, found: Found) -> Result<Status, Refusal> {
        self.found_with(landing, found, None)
    }

    pub(super) fn found_with(
        &self,
        landing: &str,
        found: Found,
        off: Off,
    ) -> Result<Status, Refusal> {
        let mut held = self.hold()?;
        let open = held
            .all
            .get_mut(landing)
            .ok_or_else(|| Refusal::NoSuchLanding(landing.to_owned()))?;
        let broken = found_breaks(&found, off);
        if !broken.is_empty() {
            return Err(Refusal::Verification(broken));
        }
        if open.finding(found.fingerprint()).is_some() {
            return Ok(open.status());
        }
        open.findings.push(Finding::new(found.clone()));
        let status = open.status();
        held.append(&Row::Found {
            landing: landing.to_owned(),
            found: Box::new(found),
        })?;
        Ok(status)
    }

    /// Records how the verification of `fingerprint` on `landing` is staffed; the latest row
    /// stands. A verifier commissioned is held independent of the finder and the author.
    ///
    /// # Errors
    ///
    /// No such open landing or finding; a verifier that is not independent; a finding already
    /// ruled; records that cannot be kept.
    pub fn assign(&self, landing: &str, fingerprint: &str, staff: Staff) -> Result<(), Refusal> {
        self.assign_with(landing, fingerprint, staff, None)
    }

    pub(super) fn assign_with(
        &self,
        landing: &str,
        fingerprint: &str,
        staff: Staff,
        off: Off,
    ) -> Result<(), Refusal> {
        let mut held = self.hold()?;
        let open = held
            .all
            .get(landing)
            .ok_or_else(|| Refusal::NoSuchLanding(landing.to_owned()))?;
        let finding = open
            .finding(fingerprint)
            .ok_or_else(|| Refusal::NoSuchLanding(format!("{landing} #{fingerprint}")))?;
        let broken = assign_breaks(finding, &open.opening, &staff, off);
        if !broken.is_empty() {
            return Err(Refusal::Verification(broken));
        }
        held.append(&Row::Assigned {
            landing: landing.to_owned(),
            fingerprint: fingerprint.to_owned(),
            staff,
        })
    }

    /// Files a verifier's verdict. Its commands are stamped from `ran`, the verifier's own
    /// recorded commands, and its evidence held to `tree`, the landing's tree, and to `ran`.
    ///
    /// # Errors
    ///
    /// No such open landing or finding; a filing that breaks a [`Guard`] — every one named;
    /// records that cannot be kept.
    pub fn verdict(&self, filing: Filing, tree: &dyn World, ran: &Ran) -> Result<Status, Refusal> {
        self.verdict_with(filing, tree, ran, None)
    }

    pub(super) fn verdict_with(
        &self,
        filing: Filing,
        tree: &dyn World,
        ran: &Ran,
        off: Off,
    ) -> Result<Status, Refusal> {
        if !self.path.exists() {
            return Err(Refusal::NoSuchLanding(filing.landing));
        }
        let mut held = self.hold()?;
        let open = held
            .all
            .get(&filing.landing)
            .ok_or_else(|| Refusal::NoSuchLanding(filing.landing.clone()))?;
        let finding = open.finding(&filing.fingerprint).ok_or_else(|| {
            Refusal::NoSuchLanding(format!("{} #{}", filing.landing, filing.fingerprint))
        })?;
        let mut ruling = filing.ruling;
        if let Ruling::Confirmed { reran, .. } | Ruling::Refuted { reran, .. } = &mut ruling {
            for r in reran.iter_mut() {
                r.sha256 = ran.last(&r.run).cloned().unwrap_or_default();
                r.failed = ran.last_failed(&r.run).unwrap_or(false);
            }
        }
        let witness = finding.found.record.witness.as_ref();
        let v = Verification {
            fingerprint: filing.fingerprint.clone(),
            verifier: filing.verifier.clone(),
            speciality: Speciality::of(&filing.verifier)
                .map(|s| s.creature().to_owned())
                .unwrap_or_default(),
            finder: finding.found.finder.clone(),
            finder_speciality: finding.found.record.speciality.clone(),
            tree: filing.tree.clone(),
            witness_sha256: witness.map(|w| w.sha256.clone()).unwrap_or_default(),
            output_sha256: witness
                .and_then(|w| w.output_sha256.clone())
                .unwrap_or_default(),
            ruling,
        };
        let broken = verdict_breaks(finding, &open.opening, &v, Some((tree, ran)), off);
        if !broken.is_empty() {
            return Err(Refusal::Verification(broken));
        }
        held.append(&Row::Ruled {
            landing: filing.landing.clone(),
            verification: v.clone(),
        })?;
        let open = held
            .all
            .get_mut(&filing.landing)
            .ok_or_else(|| Refusal::NoSuchLanding(filing.landing.clone()))?;
        if let Some(f) = open.finding_mut(&filing.fingerprint) {
            f.verdict = Some(v);
        }
        Ok(open.status())
    }

    /// Proposes a seal on a finding confirmed at high or critical, by `by` (the throne), its
    /// witness frozen by hash. The proposal waits for the human: nothing here applies a seal.
    ///
    /// # Errors
    ///
    /// No such open landing or finding; one not confirmed at high or critical, or proposed
    /// already; records that cannot be kept.
    pub fn propose(&self, landing: &str, fingerprint: &str, by: &str) -> Result<Proposal, Refusal> {
        self.propose_with(landing, fingerprint, by, None)
    }

    pub(super) fn propose_with(
        &self,
        landing: &str,
        fingerprint: &str,
        by: &str,
        off: Off,
    ) -> Result<Proposal, Refusal> {
        let mut held = self.hold()?;
        let open = held
            .all
            .get(landing)
            .ok_or_else(|| Refusal::NoSuchLanding(landing.to_owned()))?;
        let f = open
            .finding(fingerprint)
            .ok_or_else(|| Refusal::NoSuchLanding(format!("{landing} #{fingerprint}")))?;
        let broken = propose_breaks(f, off);
        if !broken.is_empty() {
            return Err(Refusal::Verification(broken));
        }
        let record = &f.found.record;
        let p = Proposal {
            fingerprint: fingerprint.to_owned(),
            speciality: record.speciality.clone(),
            file: record.file.clone(),
            symbol: record.symbol.clone(),
            severity: f.severity(),
            witness: record
                .witness
                .clone()
                .ok_or_else(|| Refusal::Verification(vec![Guard::Sealable]))?,
            verifier: f
                .verdict
                .as_ref()
                .map(|v| v.verifier.clone())
                .unwrap_or_default(),
            by: by.to_owned(),
        };
        held.append(&Row::Proposed {
            landing: landing.to_owned(),
            proposal: p.clone(),
        })?;
        Ok(p)
    }

    /// Records the human's decision on `fingerprint`, a finding of `landing`, bound to the tree its
    /// challenge was opened on; the latest stands. It lifts no block.
    ///
    /// # Errors
    ///
    /// No such open landing or finding, or records that cannot be kept.
    pub fn decide(
        &self,
        landing: &str,
        fingerprint: &str,
        decision: &Decision,
    ) -> Result<(), Refusal> {
        let mut held = self.hold()?;
        let open = held
            .all
            .get(landing)
            .ok_or_else(|| Refusal::NoSuchLanding(landing.to_owned()))?;
        if open.finding(fingerprint).is_none() {
            return Err(Refusal::NoSuchLanding(format!("{landing} #{fingerprint}")));
        }
        let mut decision = decision.clone();
        decision.tree.clone_from(&open.opening.tree);
        if !decision.holds(&open.opening.tree) {
            return Err(Refusal::Unreasoned);
        }
        held.append(&Row::Decided {
            landing: landing.to_owned(),
            fingerprint: fingerprint.to_owned(),
            decision,
        })
    }

    /// Every finding the records keep, each landing's with where it stands and the order of the
    /// record that settled it: the rows the Dark Elf's value ledger folds per speciality
    /// ([`mnemorph_ledger::fold_findings`]).
    #[must_use]
    pub fn finding_rows(&self) -> Vec<FindingRow> {
        self.read()
            .into_iter()
            .flat_map(|(id, open)| {
                let at = open.at;
                open.findings.into_iter().map(move |f| FindingRow {
                    speciality: f.found.record.speciality.clone(),
                    fingerprint: f.fingerprint().to_owned(),
                    landing: id.clone(),
                    standing: f.standing(),
                    at: at.get(f.fingerprint()).copied().unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Every confirmed and refuted verdict the records keep, landing by landing: the rows a
    /// precision fold reads per finder speciality.
    #[must_use]
    pub fn outcomes(&self) -> Vec<Outcome> {
        self.read()
            .into_iter()
            .flat_map(|(id, open)| {
                open.findings
                    .into_iter()
                    .filter_map(|f| f.verdict)
                    .filter_map(move |v| {
                        let confirmed = match v.ruling {
                            Ruling::Confirmed { .. } => true,
                            Ruling::Refuted { .. } => false,
                            Ruling::CannotVerify { .. } => return None,
                        };
                        Some(Outcome {
                            landing: id.clone(),
                            fingerprint: v.fingerprint,
                            finder_speciality: v.finder_speciality,
                            finder: v.finder,
                            verifier: v.verifier,
                            confirmed,
                        })
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
