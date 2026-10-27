//! The Dark Elf's coverage ledger: what a speciality looked at on a landing, so that "no findings"
//! means something.
//!
//! For every surface the landing touched that the speciality is routed, and every attack class
//! the speciality owns, the ledger holds exactly one entry: **checked** (how, and the files and
//! commands that show it), **not applicable** (the fact that rules it out — shown by files for a
//! high-risk class), or **skipped** (why, and for a high-risk class the risk left open).
//! [`validate`] holds the ledger to its state table; each [`Rule`] is named, so a refused ledger
//! comes back with the rules it broke for the body to fix.
//!
//! Adapted from Cloudflare's security-audit-skill (`validate-coverage-ledger.cjs`, MIT; see
//! `THIRD-PARTY-NOTICES.md`): its strict state table and bounds, cut to Mnemorph's surfaces and
//! classes.

use std::collections::BTreeMap;
use std::fmt;

use mnemorph_races::{AttackClass, Speciality};
use serde::{Deserialize, Serialize};

use crate::World;
use crate::finding::sha256_hex;

/// The shortest a reason, a risk or a method may be, in characters, once its whitespace is folded.
pub const MIN_PROSE: usize = 20;
/// The most entries one ledger may hold.
pub const MAX_ENTRIES: usize = 512;
/// The longest any one text field may be, in bytes.
pub const MAX_TEXT: usize = 1024;
/// The most items any one list (files, symbols, commands) may hold.
pub const MAX_LIST: usize = 64;
/// The most breaches a refusal lists; it counts the rest.
pub const MAX_BREACHES: usize = 32;
/// The most pairs one normalised reason may stand for.
pub const MAX_SAME_REASON: usize = 2;
/// The most pairs one file, at one hash, may be the sole evidence of.
pub const MAX_SOLE_EVIDENCE: usize = 2;

/// Reasons that say nothing, as [`normalised`] reads them: refused whatever their length.
const BOILERPLATE: &[&str] = &[
    "na",
    "n a",
    "none",
    "nothing",
    "not relevant",
    "not applicable",
    "irrelevant",
    "no issues",
    "no issue",
    "no issues found",
    "no issues were found",
    "nothing found",
    "nothing to check",
    "nothing to report",
    "not needed",
    "not required",
    "does not apply",
    "does not apply here",
    "not applicable here",
    "not applicable to this change",
    "not relevant to this change",
    "not relevant here",
    "out of scope",
    "out of scope for this change",
    "same as above",
    "see above",
    "skipped",
    "skip",
    "todo",
    "tbd",
    "later",
    "no time",
    "ok",
    "fine",
    "looks fine",
    "looks good",
    "lgtm",
];

/// The classes whose break hands over code execution, a credential, the gate or the law: a skip
/// of one states the risk it leaves open, a ruling it out shows the files that do, and every
/// touched surface has one of its speciality's checked.
pub const HIGH_RISK: [AttackClass; 14] = [
    AttackClass::SandboxEscape,
    AttackClass::PathTraversal,
    AttackClass::CommandInjection,
    AttackClass::ApprovalBinding,
    AttackClass::MemorySafety,
    AttackClass::HookLaundering,
    AttackClass::WireForgery,
    AttackClass::KnownVulnerability,
    AttackClass::WorkflowInjection,
    AttackClass::SecretExposure,
    AttackClass::CredentialGrant,
    AttackClass::AuthBypass,
    AttackClass::UnsafeDeserialisation,
    AttackClass::LedgerIntegrity,
];

/// A surface a landing touched, as its routing found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    /// The compiled glob (`crates/mnemorph-wire/**`) or the world's declared claim (`src/auth/`).
    pub at: String,
    /// Whether the world declared it in a Dark Elf's doc, rather than the law compiling it.
    pub declared: bool,
}

impl Surface {
    /// Whether `path` (a plain repository path) lies on this surface: a compiled glob as the
    /// routing reads it; a declared claim as ownership does — a directory claim covers what lies
    /// under it, a file claim the file alone.
    #[must_use]
    pub fn holds(&self, path: &str) -> bool {
        if !self.declared {
            mnemorph_races::glob(&self.at, path)
        } else if self.at.ends_with('/') {
            path.starts_with(&self.at)
        } else {
            path == self.at
        }
    }
}

/// One speciality routed to a landing, and the surfaces of its that the landing touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routed {
    /// The speciality, by its creature name.
    #[serde(with = "by_name")]
    pub speciality: Speciality,
    /// Its surfaces the written paths touched, compiled ones first.
    pub surfaces: Vec<Surface>,
}

/// A landing under challenge, as the harness routed it at the gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Landing {
    /// The landing's id, as its challenge is opened under.
    pub id: String,
    /// The paths it wrote, normalised.
    pub written: Vec<String>,
    /// The specialities routed to it, in routing order.
    pub routed: Vec<Routed>,
}

impl Landing {
    /// The routing of `speciality` on this landing, if it is routed.
    #[must_use]
    pub fn routed(&self, speciality: Speciality) -> Option<&Routed> {
        self.routed.iter().find(|r| r.speciality == speciality)
    }

    /// Every (surface, class) pair `speciality` must cover on this landing, in table order. The
    /// generalist owns no class, so it owes none.
    #[must_use]
    pub fn required(&self, speciality: Speciality) -> Vec<(String, AttackClass)> {
        self.routed(speciality)
            .map(|r| {
                r.surfaces
                    .iter()
                    .flat_map(|s| speciality.classes().iter().map(|&c| (s.at.clone(), c)))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A file read: its path, the symbols looked at, and its SHA-256 as it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRead {
    /// A plain repository path.
    pub path: String,
    /// The functions, types, keys or packages looked at; at least one.
    #[serde(default)]
    pub symbols: Vec<String>,
    /// 64 hex digits: `sha256sum` of the file.
    pub sha256: String,
}

/// A command run, and the SHA-256 of its output as the body's run journal recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRun {
    /// The command, exactly as it was run through `bash`.
    pub run: String,
    /// 64 hex digits; left out, the harness stamps it from the recorded output ([`stamp`]).
    #[serde(default)]
    pub sha256: Option<String>,
}

/// What a check, or a ruling out, rests on.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// The files read.
    #[serde(default)]
    pub files: Vec<FileRead>,
    /// The commands run.
    #[serde(default)]
    pub commands: Vec<CommandRun>,
}

impl Evidence {
    fn is_empty(&self) -> bool {
        self.files.is_empty() && self.commands.is_empty()
    }
}

/// One pair's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum State {
    /// Looked at: how, and what shows it.
    Checked {
        /// How it was looked at.
        method: String,
        /// The files and commands that show it.
        evidence: Evidence,
    },
    /// Ruled out by a fact of the code.
    NotApplicable {
        /// The fact that rules it out.
        reason: String,
        /// The files that show it; required for a high-risk class.
        #[serde(default)]
        evidence: Evidence,
    },
    /// Not looked at.
    Skipped {
        /// Why not.
        reason: String,
        /// The risk left open; a high-risk class must state one.
        #[serde(default)]
        risk: Option<String>,
    },
}

impl State {
    fn evidence(&self) -> Option<&Evidence> {
        match self {
            State::Checked { evidence, .. } | State::NotApplicable { evidence, .. } => {
                Some(evidence)
            }
            State::Skipped { .. } => None,
        }
    }

    fn evidence_mut(&mut self) -> Option<&mut Evidence> {
        match self {
            State::Checked { evidence, .. } | State::NotApplicable { evidence, .. } => {
                Some(evidence)
            }
            State::Skipped { .. } => None,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            State::NotApplicable { reason, .. } | State::Skipped { reason, .. } => Some(reason),
            State::Checked { .. } => None,
        }
    }
}

/// One (surface, class) pair and its state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The surface, exactly as the landing's routing names it.
    pub surface: String,
    /// The attack class, by its playbook name (`wire-forgery`).
    pub class: String,
    /// Its state.
    pub state: State,
}

/// One speciality's coverage of one landing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageLedger {
    /// The landing it covers.
    pub landing: String,
    /// The speciality that filed it, by its creature name.
    #[serde(with = "by_name")]
    pub speciality: Speciality,
    /// One entry per required pair.
    pub entries: Vec<Entry>,
}

/// The commands a body ran, as its run journal recorded them: each command, trimmed, to the
/// SHA-256 of every output it gave.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ran {
    outputs: BTreeMap<String, Vec<String>>,
}

impl Ran {
    /// Records that `command` was run and printed `output`.
    pub fn record(&mut self, command: &str, output: &str) {
        self.outputs
            .entry(command.trim().to_owned())
            .or_default()
            .push(sha256_hex(output.as_bytes()));
    }

    /// Whether `command` was run, and — with `sha256` — gave that output.
    fn shows(&self, command: &str, sha256: Option<&str>) -> bool {
        self.outputs.get(command.trim()).is_some_and(|hashes| {
            sha256.is_none_or(|h| hashes.iter().any(|o| o.eq_ignore_ascii_case(h)))
        })
    }

    fn last(&self, command: &str) -> Option<&String> {
        self.outputs.get(command.trim()).and_then(|h| h.last())
    }
}

/// Stamps every command of `ledger` that names no hash with the hash of its last output in `ran`.
/// A command `ran` does not hold stays unstamped, and [`Rule::UnrunCommand`] refuses it.
pub fn stamp(ledger: &mut CoverageLedger, ran: &Ran) {
    for e in &mut ledger.entries {
        if let Some(evidence) = e.state.evidence_mut() {
            for c in &mut evidence.commands {
                if c.sha256.is_none() {
                    c.sha256 = ran.last(&c.run).cloned();
                }
            }
        }
    }
}

/// A rule of the state table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rule {
    /// The ledger is too big: too many entries, a text or a list over its cap.
    Oversize,
    /// A text holds a control or invisible character.
    UnreadableText,
    /// The speciality filing it is not routed to the landing.
    Unrouted,
    /// A required pair has no entry.
    MissingPair,
    /// A pair has more than one entry.
    DuplicatePair,
    /// An entry names a pair the landing does not require of this speciality.
    ExtraPair,
    /// A not-applicable or skipped reason is empty, boilerplate or under [`MIN_PROSE`].
    BoilerplateReason,
    /// One reason stands for more than [`MAX_SAME_REASON`] pairs.
    UniformReason,
    /// A high-risk class is skipped without a stated risk.
    UnstatedRisk,
    /// A high-risk class is ruled not applicable with no file to show it.
    UnprovenRuling,
    /// A touched surface has none of the speciality's high-risk classes checked.
    UncheckedSurface,
    /// A check names no method, or rests on no file and no command.
    HollowCheck,
    /// A cited file names no symbol.
    UnnamedSymbols,
    /// A cited file is outside the landing's surfaces for this speciality.
    OutsideEvidence,
    /// A file or command hash is not 64 hex digits.
    UnhashedEvidence,
    /// A file's hash is not the file as it stands in the landing's tree.
    StaleEvidence,
    /// One file at one hash is the sole evidence of more than [`MAX_SOLE_EVIDENCE`] pairs.
    ReusedEvidence,
    /// A command the body's run journal does not show run with that output.
    UnrunCommand,
}

impl Rule {
    /// Every rule, in the order they are checked.
    pub const ALL: [Rule; 18] = [
        Rule::Oversize,
        Rule::UnreadableText,
        Rule::Unrouted,
        Rule::MissingPair,
        Rule::DuplicatePair,
        Rule::ExtraPair,
        Rule::BoilerplateReason,
        Rule::UniformReason,
        Rule::UnstatedRisk,
        Rule::UnprovenRuling,
        Rule::UncheckedSurface,
        Rule::HollowCheck,
        Rule::UnnamedSymbols,
        Rule::OutsideEvidence,
        Rule::UnhashedEvidence,
        Rule::StaleEvidence,
        Rule::ReusedEvidence,
        Rule::UnrunCommand,
    ];

    /// The rule's name, as a refusal cites it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Rule::Oversize => "oversize",
            Rule::UnreadableText => "unreadable-text",
            Rule::Unrouted => "unrouted",
            Rule::MissingPair => "missing-pair",
            Rule::DuplicatePair => "duplicate-pair",
            Rule::ExtraPair => "extra-pair",
            Rule::BoilerplateReason => "boilerplate-reason",
            Rule::UniformReason => "uniform-reason",
            Rule::UnstatedRisk => "unstated-risk",
            Rule::UnprovenRuling => "unproven-ruling",
            Rule::UncheckedSurface => "unchecked-surface",
            Rule::HollowCheck => "hollow-check",
            Rule::UnnamedSymbols => "unnamed-symbols",
            Rule::OutsideEvidence => "outside-evidence",
            Rule::UnhashedEvidence => "unhashed-evidence",
            Rule::StaleEvidence => "stale-evidence",
            Rule::ReusedEvidence => "reused-evidence",
            Rule::UnrunCommand => "unrun-command",
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One broken rule: which, where (a pair, a surface, or `ledger`), and what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breach {
    /// The rule broken.
    pub rule: Rule,
    /// Where it was broken: `<surface> × <class>`, a surface, or `ledger`.
    pub at: String,
    /// What broke it.
    pub detail: String,
}

impl fmt::Display for Breach {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}: {}", self.rule, self.at, self.detail)
    }
}

/// A refused ledger: the first [`MAX_BREACHES`] breaches, and how many more there were.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The breaches listed.
    pub breaches: Vec<Breach>,
    /// The breaches not listed.
    pub more: usize,
}

impl Refused {
    /// The rules broken, each once, in table order.
    #[must_use]
    pub fn rules(&self) -> Vec<Rule> {
        let mut rules: Vec<Rule> = self.breaches.iter().map(|b| b.rule).collect();
        rules.sort();
        rules.dedup();
        rules
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in &self.breaches {
            writeln!(f, "- {b}")?;
        }
        if self.more > 0 {
            writeln!(f, "- and {} more", self.more)?;
        }
        Ok(())
    }
}

/// What a ledger's evidence is held to beyond itself: the landing's tree, for
/// [`Rule::StaleEvidence`], and the body's recorded commands, for [`Rule::UnrunCommand`]. Either
/// absent, its rule holds nothing to it — and a command must then carry its stamped hash.
#[derive(Clone, Copy, Default)]
pub struct Sources<'a> {
    /// The landing's tree.
    pub tree: Option<&'a dyn World>,
    /// The commands the filing body ran.
    pub ran: Option<&'a Ran>,
}

/// Holds `ledger` to every rule, with nothing beyond it: a stored ledger's check on read.
///
/// # Errors
///
/// The breaches, when any rule is broken.
pub fn validate(ledger: &CoverageLedger, landing: &Landing) -> Result<(), Refused> {
    refused(check(ledger, landing, &Rule::ALL, Sources::default()))
}

/// Holds `ledger` to every rule, its evidence held to `sources`: the check at filing.
///
/// # Errors
///
/// The breaches, when any rule is broken.
pub fn validate_in(
    ledger: &CoverageLedger,
    landing: &Landing,
    sources: Sources<'_>,
) -> Result<(), Refused> {
    refused(check(ledger, landing, &Rule::ALL, sources))
}

/// Holds `ledger` to `rules` only, its evidence held to `sources`.
#[must_use]
pub fn check(
    ledger: &CoverageLedger,
    landing: &Landing,
    rules: &[Rule],
    sources: Sources<'_>,
) -> Vec<Breach> {
    let mut c = Checker {
        rules,
        sources,
        out: Vec::new(),
    };
    // Bounded first: nothing past the cap is read.
    if ledger.entries.len() > MAX_ENTRIES {
        c.breach(
            Rule::Oversize,
            "ledger",
            format!(
                "{} entries, over the cap of {MAX_ENTRIES}",
                ledger.entries.len()
            ),
        );
        if c.on(Rule::Oversize) {
            return c.out;
        }
    }
    let entries = &ledger.entries[..ledger.entries.len().min(MAX_ENTRIES)];
    let spec = ledger.speciality;
    c.routing(ledger, landing);
    c.pairs(entries, &landing.required(spec));
    let surfaces: &[Surface] = landing.routed(spec).map_or(&[], |r| &r.surfaces);
    c.surfaces(entries, spec, surfaces);
    c.sameness(entries);
    for e in entries {
        c.entry(e, spec, surfaces);
    }
    c.out
}

/// The rules switched on, what evidence is held to, and the breaches found so far.
struct Checker<'a> {
    rules: &'a [Rule],
    sources: Sources<'a>,
    out: Vec<Breach>,
}

impl Checker<'_> {
    fn on(&self, rule: Rule) -> bool {
        self.rules.contains(&rule)
    }

    fn breach(&mut self, rule: Rule, at: &str, detail: String) {
        if self.on(rule) {
            self.out.push(Breach {
                rule,
                at: shown(at),
                detail,
            });
        }
    }

    /// [`Rule::Unrouted`]: the speciality is routed to the landing the ledger names.
    fn routing(&mut self, ledger: &CoverageLedger, landing: &Landing) {
        let spec = ledger.speciality;
        if landing.routed(spec).is_none() {
            self.breach(
                Rule::Unrouted,
                "ledger",
                format!("{spec} is not routed to landing {}", shown(&landing.id)),
            );
        }
        if ledger.landing != landing.id {
            self.breach(
                Rule::Unrouted,
                "ledger",
                format!(
                    "the ledger names landing {}, not {}",
                    shown(&ledger.landing),
                    shown(&landing.id)
                ),
            );
        }
    }

    /// Every required pair exactly once, and nothing else.
    fn pairs(&mut self, entries: &[Entry], required: &[(String, AttackClass)]) {
        let mut seen: BTreeMap<(&str, &str), usize> = BTreeMap::new();
        for e in entries {
            *seen.entry((&e.surface, &e.class)).or_default() += 1;
        }
        for (surface, class) in required {
            if !seen.contains_key(&(surface.as_str(), class.name())) {
                self.breach(
                    Rule::MissingPair,
                    &pair(surface, class.name()),
                    "no entry: every surface the landing touched × every class owned needs one"
                        .to_owned(),
                );
            }
        }
        for (&(surface, class), &n) in &seen {
            if n > 1 {
                self.breach(
                    Rule::DuplicatePair,
                    &pair(surface, class),
                    format!("{n} entries; a pair has exactly one"),
                );
            }
            if !required
                .iter()
                .any(|(s, c)| s == surface && c.name() == class)
            {
                self.breach(
                    Rule::ExtraPair,
                    &pair(surface, class),
                    "not a pair this landing requires of this speciality".to_owned(),
                );
            }
        }
    }

    /// [`Rule::UncheckedSurface`]: on every touched surface, one of the speciality's high-risk
    /// classes is checked.
    fn surfaces(&mut self, entries: &[Entry], spec: Speciality, surfaces: &[Surface]) {
        let high: Vec<&str> = spec
            .classes()
            .iter()
            .filter(|c| HIGH_RISK.contains(c))
            .map(|c| c.name())
            .collect();
        if high.is_empty() {
            return;
        }
        for s in surfaces {
            let checked = entries.iter().any(|e| {
                e.surface == s.at
                    && high.contains(&e.class.as_str())
                    && matches!(e.state, State::Checked { .. })
            });
            if !checked {
                self.breach(
                    Rule::UncheckedSurface,
                    &s.at,
                    format!(
                        "check at least one of {} here: rulings alone do not cover a surface",
                        high.join(", ")
                    ),
                );
            }
        }
    }

    /// [`Rule::UniformReason`] and [`Rule::ReusedEvidence`]: one reason, or one file at one hash,
    /// does not stand for many pairs.
    fn sameness(&mut self, entries: &[Entry]) {
        let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
        let mut sole: BTreeMap<(&str, String), usize> = BTreeMap::new();
        for e in entries {
            if let Some(reason) = e.state.reason() {
                let plain = normalised(reason);
                if !plain.is_empty() {
                    *reasons.entry(plain).or_default() += 1;
                }
            }
            if let Some(ev) = e.state.evidence()
                && let [file] = ev.files.as_slice()
                && ev.commands.is_empty()
            {
                *sole
                    .entry((file.path.as_str(), file.sha256.to_ascii_lowercase()))
                    .or_default() += 1;
            }
        }
        for (reason, n) in reasons {
            if n > MAX_SAME_REASON {
                self.breach(
                    Rule::UniformReason,
                    "ledger",
                    format!(
                        "`{}` stands for {n} pairs; each ruling states its own fact",
                        shown(&reason)
                    ),
                );
            }
        }
        for ((path, _), n) in sole {
            if n > MAX_SOLE_EVIDENCE {
                self.breach(
                    Rule::ReusedEvidence,
                    "ledger",
                    format!(
                        "{} is the sole evidence of {n} pairs, over {MAX_SOLE_EVIDENCE}",
                        shown(path)
                    ),
                );
            }
        }
    }

    /// One entry's texts and state.
    fn entry(&mut self, e: &Entry, spec: Speciality, surfaces: &[Surface]) {
        let at = pair(&e.surface, &e.class);
        for (field, text) in texts(e) {
            if text.len() > MAX_TEXT {
                self.breach(
                    Rule::Oversize,
                    &at,
                    format!("{field} is {} bytes, over {MAX_TEXT}", text.len()),
                );
            }
            if text.chars().any(unreadable) {
                self.breach(
                    Rule::UnreadableText,
                    &at,
                    format!("{field} holds a control or invisible character"),
                );
            }
        }
        let high = class_named(&e.class).is_some_and(|c| HIGH_RISK.contains(&c));
        match &e.state {
            State::NotApplicable { reason, evidence } => {
                if !substantive(reason) {
                    self.breach(Rule::BoilerplateReason, &at, said(reason));
                }
                if high && evidence.files.is_empty() {
                    self.breach(
                        Rule::UnprovenRuling,
                        &at,
                        format!(
                            "{} is high-risk: ruling it out shows the files and symbols that do",
                            shown(&e.class)
                        ),
                    );
                }
            }
            State::Skipped { reason, risk } => {
                if !substantive(reason) {
                    self.breach(Rule::BoilerplateReason, &at, said(reason));
                }
                if high && !risk.as_deref().is_some_and(substantive) {
                    self.breach(
                        Rule::UnstatedRisk,
                        &at,
                        format!(
                            "{} is high-risk: a skip states the risk it leaves open",
                            shown(&e.class)
                        ),
                    );
                }
            }
            State::Checked { method, evidence } => {
                if !substantive(method) {
                    self.breach(
                        Rule::HollowCheck,
                        &at,
                        format!("the method {}", said(method)),
                    );
                }
                if evidence.is_empty() {
                    self.breach(
                        Rule::HollowCheck,
                        &at,
                        "a check rests on at least one file read or command run".to_owned(),
                    );
                }
            }
        }
        if let Some(evidence) = e.state.evidence() {
            self.evidence(&at, evidence, spec, surfaces);
        }
    }

    /// The evidence of a check or a ruling: its lists capped, its files named, on the landing's
    /// surfaces and hashed as they stand, its commands in the body's journal.
    fn evidence(&mut self, at: &str, evidence: &Evidence, spec: Speciality, surfaces: &[Surface]) {
        let lists = [
            ("files", evidence.files.len()),
            ("commands", evidence.commands.len()),
        ]
        .into_iter()
        .chain(
            evidence
                .files
                .iter()
                .map(|f| ("a file's symbols", f.symbols.len())),
        );
        for (field, n) in lists {
            if n > MAX_LIST {
                self.breach(
                    Rule::Oversize,
                    at,
                    format!("{field}: {n} items, over {MAX_LIST}"),
                );
            }
        }
        for f in evidence.files.iter().take(MAX_LIST) {
            self.file(at, f, spec, surfaces);
        }
        for c in evidence.commands.iter().take(MAX_LIST) {
            let malformed = c.sha256.as_deref().is_some_and(|h| !is_sha256(h));
            let unstamped = c.sha256.is_none() && self.sources.ran.is_none();
            if malformed || unstamped {
                self.breach(
                    Rule::UnhashedEvidence,
                    at,
                    format!("{}: sha256 is not 64 hex digits", shown(&c.run)),
                );
            } else if let Some(ran) = self.sources.ran
                && !ran.shows(&c.run, c.sha256.as_deref())
            {
                self.breach(
                    Rule::UnrunCommand,
                    at,
                    format!(
                        "{}: your run journal shows no such command with that output; cite \
                         commands you ran through `bash`, exactly as run",
                        shown(&c.run)
                    ),
                );
            }
        }
    }

    fn file(&mut self, at: &str, f: &FileRead, spec: Speciality, surfaces: &[Surface]) {
        if f.symbols.iter().all(|s| s.trim().is_empty()) {
            self.breach(
                Rule::UnnamedSymbols,
                at,
                format!("{}: name the symbols looked at", shown(&f.path)),
            );
        }
        let path = mnemorph_wire::repo_path(&f.path);
        let inside = path
            .as_deref()
            .is_some_and(|p| surfaces.iter().any(|s| s.holds(p)));
        if !inside {
            self.breach(
                Rule::OutsideEvidence,
                at,
                format!(
                    "{} is not a file on the landing's surfaces for {spec}",
                    shown(&f.path)
                ),
            );
        }
        if !is_sha256(&f.sha256) {
            self.breach(
                Rule::UnhashedEvidence,
                at,
                format!("{}: sha256 is not 64 hex digits", shown(&f.path)),
            );
        } else if let (Some(tree), Some(p)) = (self.sources.tree, path.as_deref())
            && inside
            && !tree
                .read(p)
                .is_some_and(|bytes| sha256_hex(&bytes).eq_ignore_ascii_case(&f.sha256))
        {
            self.breach(
                Rule::StaleEvidence,
                at,
                format!("{}: not the file as it stands", shown(&f.path)),
            );
        }
    }
}

/// A refusal from breaches, or `Ok` when there are none.
fn refused(mut breaches: Vec<Breach>) -> Result<(), Refused> {
    if breaches.is_empty() {
        return Ok(());
    }
    let more = breaches.len().saturating_sub(MAX_BREACHES);
    breaches.truncate(MAX_BREACHES);
    Err(Refused { breaches, more })
}

/// The class a playbook name names.
#[must_use]
pub fn class_named(name: &str) -> Option<AttackClass> {
    AttackClass::ALL.into_iter().find(|c| c.name() == name)
}

/// Whether `text` says something: at least [`MIN_PROSE`] characters once its whitespace is
/// folded, and not one of the stock phrases that say nothing.
#[must_use]
pub fn substantive(text: &str) -> bool {
    let folded = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let plain = normalised(&folded);
    folded.chars().count() >= MIN_PROSE && !plain.is_empty() && !BOILERPLATE.contains(&&*plain)
}

/// Lower-cased, every run of characters that are not letters or digits read as one space,
/// trimmed: `N/A.` reads `n a`, `Not relevant!` reads `not relevant`.
fn normalised(text: &str) -> String {
    let lower = text.to_lowercase();
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 64 hex digits, either case.
fn is_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A control character (line breaks included) or one no reader can see.
fn unreadable(c: char) -> bool {
    c.is_control() || mnemorph_wire::invisible(c)
}

/// Every text field of an entry, named.
fn texts(e: &Entry) -> Vec<(&'static str, &str)> {
    let mut out = vec![("surface", e.surface.as_str()), ("class", e.class.as_str())];
    match &e.state {
        State::Checked { method, .. } => out.push(("method", method)),
        State::NotApplicable { reason, .. } => out.push(("reason", reason)),
        State::Skipped { reason, risk } => {
            out.push(("reason", reason));
            if let Some(risk) = risk {
                out.push(("risk", risk));
            }
        }
    }
    if let Some(evidence) = e.state.evidence() {
        for f in evidence.files.iter().take(MAX_LIST) {
            out.push(("a file's path", &f.path));
            out.push(("a file's sha256", &f.sha256));
            out.extend(
                f.symbols
                    .iter()
                    .take(MAX_LIST)
                    .map(|s| ("a symbol", s.as_str())),
            );
        }
        for c in evidence.commands.iter().take(MAX_LIST) {
            out.push(("a command", &c.run));
            if let Some(h) = &c.sha256 {
                out.push(("a command's sha256", h));
            }
        }
    }
    out
}

/// `<surface> × <class>`.
fn pair(surface: &str, class: &str) -> String {
    format!("{surface} × {class}")
}

/// What a refused text said, shown safely.
fn said(text: &str) -> String {
    format!(
        "{} says nothing: state the fact, in {MIN_PROSE} characters or more",
        if text.trim().is_empty() {
            "(empty)".to_owned()
        } else {
            format!("`{}`", shown(text))
        }
    )
}

/// A text from the ledger as a refusal shows it: unreadable characters as `?`, at most 80
/// characters.
fn shown(text: &str) -> String {
    let mut out: String = text
        .chars()
        .take(80)
        .map(|c| if unreadable(c) { '?' } else { c })
        .collect();
    if text.chars().nth(80).is_some() {
        out.push('…');
    }
    out
}

/// A speciality on disk and on the wire: its creature name.
mod by_name {
    use mnemorph_races::Speciality;
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    // Serde's `with` calls these with the field by reference.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(s: &Speciality, out: S) -> Result<S::Ok, S::Error> {
        out.serialize_str(s.creature())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Speciality, D::Error> {
        let name = String::deserialize(d)?;
        Speciality::of(&name).ok_or_else(|| D::Error::custom("no such Dark Elf speciality"))
    }
}

#[cfg(test)]
mod tests;
