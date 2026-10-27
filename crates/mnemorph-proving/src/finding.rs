//! The finding record and its validator.

use std::fmt::{self, Write as _};

use mnemorph_races::{AttackClass, Speciality};
use mnemorph_wire::{RootCause, repo_path};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::world::World;

/// The most bytes a record's file may hold: a longer one is no record.
pub const RECORD_BYTES: usize = 64 * 1024;
/// The most bytes of a claim.
pub const CLAIM_BYTES: usize = 512;
/// The most bytes of a one-line field: the root cause's four fields, a trace step's location and
/// note, a precondition, the witness's reference.
pub const FIELD_BYTES: usize = 512;
/// The most bytes of a prose field: the reachability, the reasons, the fix and its invariant.
pub const TEXT_BYTES: usize = 2048;
/// The most preconditions a record lists.
pub const LIST_ITEMS: usize = 16;
/// The most steps a trace holds.
pub const TRACE_STEPS: usize = 32;

/// What a record claims to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// A finding: it may count once it validates.
    Finding,
    /// Needs validation: a suspicion with its blocker and plan, never a finding.
    Nv,
}

/// A finding's overall severity, lowest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Worth knowing; no harm shown.
    Informational,
    /// Little harm, or none reachable.
    Low,
    /// Harm bounded to availability, or needing unusual conditions.
    Medium,
    /// Data read or changed across a boundary.
    High,
    /// A boundary bypassed, or code run, by a lower-trust principal.
    Critical,
}

impl Severity {
    /// Its word in a record.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Severity::Informational => "informational",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
}

/// The impact the witness demonstrates — not the worst it could imagine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Impact {
    /// No failure shown beyond a deviation.
    None,
    /// A panic, an abort or a hang of one process.
    Crash,
    /// A budget, a cap or a resource run out.
    Exhaustion,
    /// Data read across a boundary: a secret, another world's memory, a file outside the grant.
    Disclosure,
    /// State changed across a boundary: a forged wire line, a journal or ledger rewritten.
    Tamper,
    /// A boundary bypassed: the sandbox, the gate, an approval bound to another act.
    Bypass,
    /// Code of the attacker's choosing run.
    Execution,
}

impl Impact {
    /// The highest severity this impact can carry, when its reachability is shown.
    #[must_use]
    pub fn cap(self) -> Severity {
        match self {
            Impact::None => Severity::Informational,
            Impact::Crash | Impact::Exhaustion => Severity::Medium,
            Impact::Disclosure | Impact::Tamper => Severity::High,
            Impact::Bypass | Impact::Execution => Severity::Critical,
        }
    }
}

/// How likely an attacker meets the preconditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Likelihood {
    /// Rare, or needing a privileged foothold.
    Low,
    /// Plausible in ordinary use.
    Medium,
    /// Met by default.
    High,
}

/// What the witness is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WitnessKind {
    /// A proof-of-concept test in the proving ground; its reference is the test's file.
    Test,
    /// A proof-of-concept command; its reference is the command line.
    PocCommand,
    /// A scanner's output in the proving ground; its reference is the output's file.
    ScannerOutput,
}

/// The witness: what shows the failure, and the hash that pins it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Witness {
    /// What it is.
    pub kind: WitnessKind,
    /// The test's or the output's file in the proving ground, or the command line.
    pub reference: String,
    /// The SHA-256 of the file's bytes, or of the command line, in lowercase hex.
    pub sha256: String,
    /// For a `poc-command`: the file in the proving ground holding the output it printed when
    /// run — what the harness hashes, since a command's own text proves nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// The SHA-256 of that output file's bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_sha256: Option<String>,
}

impl Witness {
    /// Whether the harness can re-read a file that shows the failure: a test, a scanner's
    /// output, or a command's captured output. A bare command line is only a claim.
    #[must_use]
    pub fn on_file(&self) -> bool {
        match self.kind {
            WitnessKind::Test | WitnessKind::ScannerOutput => true,
            WitnessKind::PocCommand => self.output.is_some(),
        }
    }
}

/// Where a trace step stands between the entry and the sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepKind {
    /// Where the attacker's input enters.
    Entry,
    /// Where it travels.
    Propagation,
    /// Where it does harm: the root cause's file.
    Sink,
}

/// One step of the trace from entry to sink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Its place in the trace.
    pub kind: StepKind,
    /// `file:line`.
    pub at: String,
    /// What happens there.
    #[serde(default)]
    pub note: String,
}

/// A Dark Elf's finding record: `.mnemorph/dark-elf/<speciality>/findings/<fingerprint>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindingRecord {
    /// A finding, or one that needs validation.
    pub verdict: Verdict,
    /// The root-cause fingerprint the record states; the harness recomputes it.
    #[serde(default)]
    pub fingerprint: String,
    /// The speciality's full name (`dark-elf-harness`).
    #[serde(default)]
    pub speciality: String,
    /// The attack class, one the speciality owns.
    #[serde(default)]
    pub class: String,
    /// The root cause's file, repository-relative.
    #[serde(default)]
    pub file: String,
    /// The root cause's symbol, as the code names it.
    #[serde(default)]
    pub symbol: String,
    /// The claim, one line.
    #[serde(default)]
    pub claim: String,
    /// From entry to sink, a `file:line` at each step.
    #[serde(default)]
    pub trace: Vec<Step>,
    /// What shows the failure.
    #[serde(default)]
    pub witness: Option<Witness>,
    /// What must hold for the attack to work.
    #[serde(default)]
    pub preconditions: Vec<String>,
    /// How an attacker reaches it; empty when not shown.
    #[serde(default)]
    pub reachability: String,
    /// How likely the preconditions are met.
    #[serde(default)]
    pub likelihood: Option<Likelihood>,
    /// Why.
    #[serde(default)]
    pub likelihood_reason: String,
    /// The impact the witness demonstrates.
    #[serde(default)]
    pub impact: Option<Impact>,
    /// Why.
    #[serde(default)]
    pub impact_reason: String,
    /// The overall severity, capped by the impact.
    #[serde(default)]
    pub severity: Option<Severity>,
    /// The fix.
    #[serde(default)]
    pub fix: String,
    /// The invariant the fix restores.
    #[serde(default)]
    pub invariant: String,
    /// The path the record was read from: the `@F`'s `rec:`. Not in the file.
    #[serde(skip)]
    pub rec: String,
}

impl FindingRecord {
    /// Reads a record from its file's bytes; `rec` is the path it was read from. `None` when the
    /// bytes are over [`RECORD_BYTES`] or are not a record.
    #[must_use]
    pub fn parse(rec: &str, bytes: &[u8]) -> Option<Self> {
        if bytes.len() > RECORD_BYTES {
            return None;
        }
        let mut record: FindingRecord = serde_json::from_slice(bytes).ok()?;
        rec.clone_into(&mut record.rec);
        Some(record)
    }

    /// The record as its file holds it.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// The root cause it states.
    #[must_use]
    pub fn root(&self) -> RootCause {
        RootCause {
            speciality: self.speciality.clone(),
            class: self.class.clone(),
            file: self.file.clone(),
            symbol: self.symbol.clone(),
        }
    }

    fn one_liners(&self) -> impl Iterator<Item = &str> {
        let steps = self
            .trace
            .iter()
            .flat_map(|s| [s.at.as_str(), s.note.as_str()]);
        let witness = self
            .witness
            .iter()
            .flat_map(|w| [w.reference.as_str(), w.sha256.as_str()]);
        [
            self.fingerprint.as_str(),
            &self.speciality,
            &self.class,
            &self.file,
            &self.symbol,
        ]
        .into_iter()
        .chain(steps)
        .chain(witness)
        .chain(self.preconditions.iter().map(String::as_str))
    }

    fn prose(&self) -> [&str; 5] {
        [
            &self.reachability,
            &self.likelihood_reason,
            &self.impact_reason,
            &self.fix,
            &self.invariant,
        ]
    }
}

/// Where a record lives: `.mnemorph/dark-elf/<speciality>/findings/<fingerprint>.json`.
#[must_use]
pub fn record_path(speciality: &str, fingerprint: &str) -> String {
    format!(".mnemorph/dark-elf/{speciality}/findings/{fingerprint}.json")
}

/// One rule of the finding contract. A record that breaks any is read back as `@?`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rule {
    /// A needs-validation record is never a finding.
    Nv,
    /// The root cause's four fields, the claim, a trace, the likelihood, the impact (each with
    /// its reason), the severity, the fix and its invariant are all present.
    Required,
    /// Every field within its size cap.
    Size,
    /// No field holds a control character or an invisible format character (a bidi override, a
    /// zero-width space): text a terminal or a reader could be fooled by.
    Escaped,
    /// The class is one the speciality owns in the compiled table.
    ClassOwned,
    /// The stated fingerprint is the one derived from the root cause.
    Fingerprint,
    /// The record was read from its own path.
    RecPath,
    /// The root cause's file exists in the landing's tree.
    FileExists,
    /// The symbol is named in that file.
    SymbolPresent,
    /// The trace runs entry, propagation, sink, each at a safe `file:line`, the sink in the root
    /// cause's file.
    Trace,
    /// Every trace step's file is in the landing's tree, and its line within the file.
    TraceLocated,
    /// A witness: a test or scanner output in the speciality's proving ground, or a command, with
    /// a SHA-256.
    Witness,
    /// The witness's hash is its content's, and a captured output's hash its file's.
    WitnessHash,
    /// From high up, the witness is a file the harness re-reads: a test, a scanner's output, or
    /// a command's captured output — a bare command caps the severity at medium.
    WitnessStrength,
    /// The severity is at most the cap of the demonstrated impact, and at most low when no
    /// reachability is shown.
    SeverityCap,
    /// From medium up, the preconditions and the reachability are stated.
    Preconditions,
}

impl Rule {
    /// Every rule, in the order they are checked and reported.
    pub const ALL: [Rule; 16] = [
        Rule::Nv,
        Rule::Required,
        Rule::Size,
        Rule::Escaped,
        Rule::ClassOwned,
        Rule::Fingerprint,
        Rule::RecPath,
        Rule::FileExists,
        Rule::SymbolPresent,
        Rule::Trace,
        Rule::TraceLocated,
        Rule::Witness,
        Rule::WitnessHash,
        Rule::WitnessStrength,
        Rule::SeverityCap,
        Rule::Preconditions,
    ];

    /// Its name, as a demoted finding's hole names it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Rule::Nv => "nv",
            Rule::Required => "required",
            Rule::Size => "size",
            Rule::Escaped => "escaped",
            Rule::ClassOwned => "class-owned",
            Rule::Fingerprint => "fingerprint",
            Rule::RecPath => "rec-path",
            Rule::FileExists => "file-exists",
            Rule::SymbolPresent => "symbol-present",
            Rule::Trace => "trace",
            Rule::TraceLocated => "trace-located",
            Rule::Witness => "witness",
            Rule::WitnessHash => "witness-hash",
            Rule::WitnessStrength => "witness-strength",
            Rule::SeverityCap => "severity-cap",
            Rule::Preconditions => "preconditions",
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A record that passed every rule: the only finding a count, a score or a seal takes. Only
/// [`validate`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Valid {
    record: FindingRecord,
    root: RootCause,
}

impl Valid {
    /// The record.
    #[must_use]
    pub fn record(&self) -> &FindingRecord {
        &self.record
    }

    /// Its root cause.
    #[must_use]
    pub fn root(&self) -> &RootCause {
        &self.root
    }

    /// Its fingerprint, the one derived from its root cause.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.record.fingerprint
    }

    /// Its severity, within the cap of its impact.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.record.severity.unwrap_or(Severity::Informational)
    }
}

/// Holds `record` to the finding contract against `world`, the landing's tree: a [`Valid`], or
/// every rule it breaks, in [`Rule::ALL`]'s order.
///
/// # Errors
///
/// The rules the record breaks.
pub fn validate(record: &FindingRecord, world: &(impl World + ?Sized)) -> Result<Valid, Vec<Rule>> {
    check(record, world, None)
}

/// [`validate`] with one rule switched off: how a test shows that rule is the one its fixture
/// needs.
pub(crate) fn check(
    record: &FindingRecord,
    world: &(impl World + ?Sized),
    off: Option<Rule>,
) -> Result<Valid, Vec<Rule>> {
    let r = record;
    let source = world.read(&r.file);
    let broken: Vec<Rule> = Rule::ALL
        .into_iter()
        .filter(|&rule| Some(rule) != off)
        .filter(|&rule| match rule {
            Rule::Nv => r.verdict == Verdict::Nv,
            Rule::Required => !required(r),
            Rule::Size => !sized(r),
            Rule::Escaped => r
                .one_liners()
                .chain(r.prose())
                .chain([r.claim.as_str()])
                .any(hidden),
            Rule::ClassOwned => !owned(&r.speciality, &r.class),
            Rule::Fingerprint => r.root().fingerprint().as_deref() != Some(r.fingerprint.as_str()),
            Rule::RecPath => repo_path(&r.rec) != Some(record_path(&r.speciality, &r.fingerprint)),
            Rule::FileExists => source.is_none(),
            Rule::SymbolPresent => source
                .as_deref()
                .is_some_and(|s| !names(&String::from_utf8_lossy(s), &r.symbol)),
            Rule::Trace => !traced(r),
            Rule::TraceLocated => !trace_located(r, world, source.is_some()),
            Rule::Witness => !witnessed(r),
            Rule::WitnessHash => r.witness.as_ref().is_some_and(|w| !hashed(w, world)),
            Rule::WitnessStrength => {
                r.severity.is_some_and(|s| s >= Severity::High)
                    && r.witness.as_ref().is_some_and(|w| !w.on_file())
            }
            Rule::SeverityCap => r
                .severity
                .zip(r.impact)
                .is_some_and(|(s, i)| s > cap(i, &r.reachability)),
            Rule::Preconditions => {
                r.severity.is_some_and(|s| s >= Severity::Medium)
                    && (blank(&r.reachability) || r.preconditions.iter().all(|p| blank(p)))
            }
        })
        .collect();
    if broken.is_empty() {
        Ok(Valid {
            record: record.clone(),
            root: record.root(),
        })
    } else {
        Err(broken)
    }
}

/// The highest severity a finding may carry: its impact's cap, and at most low when no
/// reachability is shown.
fn cap(impact: Impact, reachability: &str) -> Severity {
    if blank(reachability) {
        impact.cap().min(Severity::Low)
    } else {
        impact.cap()
    }
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn required(r: &FindingRecord) -> bool {
    let texts = [
        &r.speciality,
        &r.class,
        &r.file,
        &r.symbol,
        &r.claim,
        &r.likelihood_reason,
        &r.impact_reason,
        &r.fix,
        &r.invariant,
    ];
    texts.iter().all(|t| !blank(t))
        && !r.trace.is_empty()
        && r.likelihood.is_some()
        && r.impact.is_some()
        && r.severity.is_some()
}

fn sized(r: &FindingRecord) -> bool {
    r.claim.len() <= CLAIM_BYTES
        && r.one_liners().all(|t| t.len() <= FIELD_BYTES)
        && r.prose().iter().all(|t| t.len() <= TEXT_BYTES)
        && r.preconditions.len() <= LIST_ITEMS
        && r.trace.len() <= TRACE_STEPS
}

/// A character that may not ride in a record: a control, or an invisible format character that
/// can reorder or hide text.
fn unsafe_char(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{00AD}' | '\u{061C}' | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{E0000}'..='\u{E007F}')
}

fn hidden(text: &str) -> bool {
    text.chars().any(unsafe_char)
}

fn owned(speciality: &str, class: &str) -> bool {
    let Some(speciality) = Speciality::of(speciality) else {
        return false;
    };
    AttackClass::ALL
        .into_iter()
        .find(|c| c.name() == class)
        .is_some_and(|c| speciality.classes().contains(&c))
}

/// Whether `text` names `symbol`: its last segment (after `::` or `.`) as a whole word.
fn names(text: &str, symbol: &str) -> bool {
    let last = symbol.rsplit([':', '.']).next().unwrap_or(symbol).trim();
    if last.is_empty() {
        return false;
    }
    let word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(last).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + last.len()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

/// The file of a `file:line`, when it is a safe repository path and a line from 1.
fn located(at: &str) -> Option<(String, usize)> {
    let (file, line) = at.rsplit_once(':')?;
    let line: usize = line.parse().ok()?;
    if line == 0 {
        return None;
    }
    Some((repo_path(file)?, line))
}

/// Every step's file in the tree and its line within it. A step in the root cause's file is
/// left to `file-exists` when that file is missing, so one gap breaks one rule.
fn trace_located(r: &FindingRecord, world: &(impl World + ?Sized), source: bool) -> bool {
    let root = repo_path(&r.file);
    r.trace.iter().all(|s| {
        let Some((file, line)) = located(&s.at) else {
            // A malformed step breaks `trace`.
            return true;
        };
        if !source && Some(&file) == root.as_ref() {
            return true;
        }
        world
            .read(&file)
            .is_some_and(|b| line <= String::from_utf8_lossy(&b).lines().count())
    })
}

fn traced(r: &FindingRecord) -> bool {
    let Some(last) = r.trace.last() else {
        // An empty trace breaks `required`, not this rule.
        return true;
    };
    let n = r.trace.len();
    let ordered = r.trace.iter().enumerate().all(|(i, s)| match (i, s.kind) {
        (0, StepKind::Entry) => true,
        (0, StepKind::Sink) => n == 1,
        (i, StepKind::Sink) => i == n - 1,
        (i, StepKind::Propagation) => i > 0 && i < n - 1,
        _ => false,
    });
    let safe = r.trace.iter().all(|s| located(&s.at).is_some());
    let sink = located(&last.at).is_some_and(|(f, _)| Some(f) == repo_path(&r.file));
    ordered && safe && sink
}

fn witnessed(r: &FindingRecord) -> bool {
    let Some(w) = &r.witness else {
        return false;
    };
    let ground = format!(".mnemorph/dark-elf/{}/", r.speciality);
    let grounded = |path: &str| repo_path(path).is_some_and(|p| p.starts_with(&ground));
    let placed = match w.kind {
        WitnessKind::PocCommand => !blank(&w.reference),
        WitnessKind::Test | WitnessKind::ScannerOutput => grounded(&w.reference),
    };
    let output = match (&w.output, &w.output_sha256) {
        (None, None) => true,
        (Some(path), Some(sha)) => {
            w.kind == WitnessKind::PocCommand && grounded(path) && is_sha256(sha)
        }
        _ => false,
    };
    is_sha256(&w.sha256) && placed && output
}

fn is_sha256(hex: &str) -> bool {
    hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        })
}

fn hashed(w: &Witness, world: &(impl World + ?Sized)) -> bool {
    let content = match w.kind {
        WitnessKind::PocCommand => Some(w.reference.as_bytes().to_vec()),
        WitnessKind::Test | WitnessKind::ScannerOutput => world.read(&w.reference),
    };
    let output = match (&w.output, &w.output_sha256) {
        (Some(path), Some(sha)) => world.read(path).is_some_and(|c| &sha256_hex(&c) == sha),
        (None, None) => true,
        _ => false,
    };
    content.is_some_and(|c| sha256_hex(&c) == w.sha256) && output
}
