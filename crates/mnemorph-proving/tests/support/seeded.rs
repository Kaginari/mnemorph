//! The seeded fixture, read for tests: the answer key (`fixtures/seeded.toml`), the world it keys
//! (`fixtures/seeded/`), and what a Dark Elf that found each plant would file — its record, its
//! witness, its `@F` line and its coverage ledger — built from the key alone, with no model.
//!
//! Shared by `mnemorph-proving`'s and `mnemorph-loop`'s seeded tests, so both read one key.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mnemorph_proving::coverage::{
    CoverageLedger, Entry, Evidence, FileRead, HIGH_RISK, Landing, Routed, State, Surface,
};
use mnemorph_proving::{
    FindingRecord, Impact, Likelihood, Severity, Step, StepKind, Verdict, Witness, WitnessKind,
    record_path,
};
use mnemorph_races::{AttackClass, Speciality};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

/// The answer key.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Each planted flaw.
    pub plant: Vec<Plant>,
    /// Each look-alike that must not be flagged.
    pub control: Vec<Control>,
}

/// One planted flaw: the minimal finding that names it, and what a report of it states.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plant {
    /// The plant's id.
    pub id: String,
    /// The owning speciality's creature name.
    pub speciality: String,
    /// The attack class.
    pub class: String,
    /// The root cause's file, world-relative.
    pub file: String,
    /// The root cause's symbol.
    pub symbol: String,
    /// The severity a witness demonstrates.
    pub severity: Severity,
    /// The impact a witness demonstrates.
    pub impact: Impact,
    /// The claim, one line.
    pub claim: String,
    /// How an attacker reaches it.
    pub reachability: String,
    /// What must hold.
    pub precondition: String,
    /// The fix.
    pub fix: String,
    /// The invariant the fix restores.
    pub invariant: String,
}

/// One look-alike: "Not a finding" by its class's playbook.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    /// The control's id.
    pub id: String,
    /// The speciality whose class it resembles.
    pub speciality: String,
    /// The class it resembles.
    pub class: String,
    /// Its file, world-relative.
    pub file: String,
    /// The symbol that shows it safe.
    pub symbol: String,
    /// The plant it imitates.
    pub imitates: String,
    /// Whether it names the imitated plant's symbol too.
    #[serde(default)]
    pub same_symbol: bool,
    /// The fact that rules it out.
    pub reason: String,
}

impl Plant {
    /// Its speciality.
    pub fn speciality(&self) -> Speciality {
        Speciality::of(&self.speciality).expect("a speciality")
    }

    /// Its class.
    pub fn class(&self) -> AttackClass {
        class(&self.class)
    }
}

impl Control {
    /// Its speciality.
    pub fn speciality(&self) -> Speciality {
        Speciality::of(&self.speciality).expect("a speciality")
    }

    /// Its class.
    pub fn class(&self) -> AttackClass {
        class(&self.class)
    }
}

fn class(name: &str) -> AttackClass {
    AttackClass::ALL
        .into_iter()
        .find(|c| c.name() == name)
        .unwrap_or_else(|| panic!("no class {name}"))
}

/// The fixtures folder: `crates/mnemorph-proving/fixtures`, from any crate beside it.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../mnemorph-proving/fixtures")
}

/// The seeded world on disk.
pub fn world_dir() -> PathBuf {
    fixtures().join("seeded")
}

/// The answer key, read.
pub fn manifest() -> Manifest {
    let text = std::fs::read_to_string(fixtures().join("seeded.toml")).expect("seeded.toml");
    toml::from_str(&text).expect("seeded.toml parses")
}

/// The seeded world in memory: every file under `seeded/`, by its world-relative path.
pub fn tree() -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries: Vec<_> = std::fs::read_dir(at)
            .expect("read_dir")
            .map(|e| e.expect("entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .expect("under root")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).expect("read"));
            }
        }
    }
    let root = world_dir();
    let mut out = BTreeMap::new();
    walk(&root, &root, &mut out);
    out
}

/// Lowercase hex SHA-256.
pub fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Whether `text` names `word` whole, as the finding validator reads a symbol.
pub fn names(text: &str, word: &str) -> bool {
    let w = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(w)
            && !text[i + word.len()..].chars().next().is_some_and(w)
    })
}

/// The first line (from 1) of `text` that names `symbol`.
pub fn line_of(text: &str, symbol: &str) -> Option<usize> {
    text.lines().position(|l| names(l, symbol)).map(|i| i + 1)
}

/// The plant's witness in its speciality's proving ground.
pub fn poc_path(p: &Plant) -> String {
    format!(".mnemorph/dark-elf/{}/poc/{}.md", p.speciality, p.id)
}

/// The witness's text: what a proof of the plant shows.
pub fn poc_body(p: &Plant) -> String {
    format!(
        "# Witness: {}\n\nClass {} at {}::{}.\n\n{}\n",
        p.id, p.class, p.file, p.symbol, p.claim
    )
}

/// The record a Dark Elf that found `p` would file, its fingerprint and path derived from the
/// root cause as the harness derives them; `text` is the plant file's content, whose first line
/// naming the symbol is the sink (its first line when none does).
pub fn record(p: &Plant, text: &str) -> FindingRecord {
    let fingerprint = mnemorph_wire::fingerprint(&p.speciality, &p.class, &p.file, &p.symbol)
        .expect("a fingerprint");
    let line = line_of(text, &p.symbol).unwrap_or(1);
    FindingRecord {
        verdict: Verdict::Finding,
        rec: record_path(&p.speciality, &fingerprint),
        fingerprint,
        speciality: p.speciality.clone(),
        class: p.class.clone(),
        file: p.file.clone(),
        symbol: p.symbol.clone(),
        claim: p.claim.clone(),
        trace: vec![
            Step {
                kind: StepKind::Entry,
                at: format!("{}:1", p.file),
                note: "where the untrusted input arrives".into(),
            },
            Step {
                kind: StepKind::Sink,
                at: format!("{}:{line}", p.file),
                note: format!("{} does the harm", p.symbol),
            },
        ],
        witness: Some(Witness {
            kind: WitnessKind::Test,
            reference: poc_path(p),
            sha256: sha(poc_body(p).as_bytes()),
            output: None,
            output_sha256: None,
        }),
        preconditions: vec![p.precondition.clone()],
        reachability: p.reachability.clone(),
        likelihood: Some(Likelihood::Medium),
        likelihood_reason: "the seeded fixture meets the precondition by default".into(),
        impact: Some(p.impact),
        impact_reason: "the witness demonstrates it on the seeded world".into(),
        severity: Some(p.severity),
        fix: p.fix.clone(),
        invariant: p.invariant.clone(),
    }
}

/// `p`'s `@F` line, pointing at `r`, its record.
pub fn finding_line(p: &Plant, r: &FindingRecord) -> String {
    let line = r
        .trace
        .last()
        .map_or("1", |s| s.at.rsplit_once(':').map_or("1", |(_, l)| l));
    format!(
        "@F #{} {}:{line} {} witness:{} rec:{}",
        r.fingerprint,
        p.file,
        p.claim,
        poc_path(p),
        r.rec
    )
}

/// Writes `p`'s witness and record into `tree`, as the reviewer leaves its proving ground.
pub fn prove(tree: &mut BTreeMap<String, Vec<u8>>, p: &Plant) -> FindingRecord {
    let text = String::from_utf8(tree[&p.file].clone()).expect("utf-8");
    let r = record(p, &text);
    tree.insert(poc_path(p), poc_body(p).into_bytes());
    tree.insert(r.rec.clone(), r.to_json().into_bytes());
    r
}

/// The landing `id` that wrote `written`, routed by the compiled surfaces alone.
pub fn landing(id: &str, written: &[String]) -> Landing {
    let routed = mnemorph_races::route(written)
        .into_iter()
        .map(|s| Routed {
            speciality: s,
            surfaces: s
                .surfaces()
                .iter()
                .filter(|g| written.iter().any(|p| mnemorph_races::glob(g, p)))
                .map(|g| Surface {
                    at: (*g).to_owned(),
                    declared: false,
                })
                .collect(),
        })
        .collect();
    Landing {
        id: id.to_owned(),
        written: written.to_vec(),
        routed,
    }
}

/// What a ledger says of one written file: the classes it shows found, and the classes it shows
/// ruled out with the fact that does.
pub struct Looked<'a> {
    /// The file.
    pub file: &'a str,
    /// Its bytes in the landing's tree.
    pub bytes: &'a [u8],
    /// The symbol looked at.
    pub symbol: &'a str,
    /// The classes checked on it: a plant's.
    pub checked: Vec<AttackClass>,
    /// The classes ruled out on it, with their facts: a control's.
    pub ruled_out: Vec<(AttackClass, &'a str)>,
}

/// `s`'s coverage ledger of `landing`, from what it `looked` at: every required pair, each in a
/// state the table takes. A class shown found or ruled out on a file of a surface is `checked` or
/// `not_applicable` there with that file as its evidence; on a surface with no high-risk class
/// checked yet, the first high-risk class `s` owns is checked on the surface's file; every other
/// pair is skipped for its own reason, with its own risk when it is high-risk.
pub fn ledger(landing: &Landing, s: Speciality, looked: &[Looked<'_>]) -> CoverageLedger {
    let routed = landing.routed(s).expect("routed");
    let mut entries = Vec::new();
    for surface in &routed.surfaces {
        let on: Vec<&Looked<'_>> = looked.iter().filter(|l| surface.holds(l.file)).collect();
        let read = |l: &Looked<'_>| Evidence {
            files: vec![FileRead {
                path: l.file.to_owned(),
                symbols: vec![l.symbol.to_owned()],
                sha256: sha(l.bytes),
            }],
            commands: Vec::new(),
        };
        let found = |c: AttackClass| on.iter().find(|l| l.checked.contains(&c));
        let ruled = |c: AttackClass| {
            on.iter().find_map(|l| {
                l.ruled_out
                    .iter()
                    .find(|(x, _)| *x == c)
                    .map(|(_, r)| (*l, *r))
            })
        };
        let high_checked = s
            .classes()
            .iter()
            .any(|c| HIGH_RISK.contains(c) && found(*c).is_some());
        // The class checked to keep the surface from going unchecked, when none of its findings is
        // high-risk.
        let keeper = (!high_checked)
            .then(|| {
                s.classes()
                    .iter()
                    .copied()
                    .find(|c| HIGH_RISK.contains(c) && ruled(*c).is_none())
            })
            .flatten();
        for &c in s.classes() {
            let state = if let Some(l) = found(c) {
                State::Checked {
                    method: format!("read {} and traced {} to its sink", l.file, l.symbol),
                    evidence: read(l),
                }
            } else if let Some((l, reason)) = ruled(c) {
                State::NotApplicable {
                    reason: reason.to_owned(),
                    evidence: read(l),
                }
            } else if Some(c) == keeper
                && let Some(l) = on.first()
            {
                State::Checked {
                    method: format!("read {} for {c} and found nothing to reach it", l.file),
                    evidence: read(l),
                }
            } else {
                State::Skipped {
                    reason: format!("the seeded landing on {} plants no {c} here", surface.at),
                    risk: HIGH_RISK
                        .contains(&c)
                        .then(|| format!("an unseeded {c} on {} would go unseen", surface.at)),
                }
            };
            entries.push(Entry {
                surface: surface.at.clone(),
                class: c.name().to_owned(),
                state,
            });
        }
    }
    CoverageLedger {
        landing: landing.id.clone(),
        speciality: s,
        entries,
    }
}

/// What `s` looked at on a landing of `written` over `tree`: each written plant of its classes
/// checked, each written control of its classes ruled out, and every other written file it is
/// routed as read for nothing.
pub fn looked<'a>(
    m: &'a Manifest,
    tree: &'a BTreeMap<String, Vec<u8>>,
    s: Speciality,
    written: &'a [String],
) -> Vec<Looked<'a>> {
    written
        .iter()
        .filter(|f| s.touches(f))
        .map(|f| {
            let plant = m.plant.iter().find(|p| &p.file == f && p.speciality() == s);
            let control = m
                .control
                .iter()
                .find(|c| &c.file == f && c.speciality() == s);
            let other_plant = m.plant.iter().find(|p| &p.file == f);
            let symbol = plant
                .map(|p| p.symbol.as_str())
                .or(control.map(|c| c.symbol.as_str()))
                .or(other_plant.map(|p| p.symbol.as_str()))
                .unwrap_or("main");
            Looked {
                file: f,
                bytes: &tree[f.as_str()],
                symbol,
                checked: plant.map(Plant::class).into_iter().collect(),
                ruled_out: control
                    .map(|c| (c.class(), c.reason.as_str()))
                    .into_iter()
                    .collect(),
            }
        })
        .collect()
}
