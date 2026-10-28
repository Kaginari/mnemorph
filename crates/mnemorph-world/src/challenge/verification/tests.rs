use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;

use mnemorph_proving::coverage::Ran;
use mnemorph_proving::{
    FindingRecord, Impact, Likelihood, Severity, Step, StepKind, Verdict, Witness, WitnessKind,
    sha256_hex,
};

use super::super::{Challenges, Opening, Refusal, Row, Staff, Status, route};
use super::{Filing, Found, Guard, Pinned, Rerun, Ruling};
use crate::roster::Roster;

const FILE: &str = "crates/mnemorph-tools/src/files.rs";
const SOURCE: &str = "pub fn read_path(p: &str) -> String { todo!() }\n";
const POC: &str = ".mnemorph/dark-elf/dark-elf-harness/poc/read_path.rs";
const POC_BODY: &str = "#[test]\nfn escapes() {}\n";
const RERUN: &str = "cargo test --test read_path";
const OUTPUT: &str = "test escapes ... FAILED: read ../secret";
const FINDER: &str = "dark-elf-harness";
const AGENT: &str = "dark-elf-agent";

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "mnemorph-verification-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The landing's tree: the source and the finder's proof-of-concept.
fn tree() -> BTreeMap<String, Vec<u8>> {
    [
        (FILE.to_owned(), SOURCE.as_bytes().to_vec()),
        (POC.to_owned(), POC_BODY.as_bytes().to_vec()),
    ]
    .into_iter()
    .collect()
}

/// The verifier ran the witness and saw it fail.
fn ran() -> Ran {
    let mut r = Ran::default();
    r.record(RERUN, OUTPUT);
    r
}

/// A harness finding at `severity`, its witness the proof-of-concept test.
fn record(severity: Severity) -> FindingRecord {
    let fp = mnemorph_wire::fingerprint(FINDER, "path-traversal", FILE, "read_path").unwrap();
    FindingRecord {
        verdict: Verdict::Finding,
        fingerprint: fp.clone(),
        speciality: FINDER.into(),
        class: "path-traversal".into(),
        file: FILE.into(),
        symbol: "read_path".into(),
        claim: "read_path follows `..` out of the world".into(),
        trace: vec![Step {
            kind: StepKind::Sink,
            at: format!("{FILE}:1"),
            note: String::new(),
        }],
        witness: Some(Witness {
            kind: WitnessKind::Test,
            reference: POC.into(),
            sha256: sha256_hex(POC_BODY.as_bytes()),
            output: None,
            output_sha256: None,
        }),
        preconditions: vec!["an untrusted repository".into()],
        reachability: "a repository file names the path".into(),
        likelihood: Some(Likelihood::Medium),
        likelihood_reason: "worlds are cloned from strangers".into(),
        impact: Some(match severity {
            Severity::Critical => Impact::Bypass,
            Severity::High => Impact::Disclosure,
            _ => Impact::Crash,
        }),
        impact_reason: "the test reads a file outside the world".into(),
        severity: Some(severity),
        fix: "refuse `..`".into(),
        invariant: "no read leaves the world".into(),
        rec: mnemorph_proving::record_path(FINDER, &fp),
    }
}

fn found(severity: Severity) -> Found {
    let record = record(severity);
    Found {
        finder: FINDER.into(),
        rec: record.rec.clone(),
        record,
    }
}

/// A challenge on a harness landing by `orc-o`, its harness ledger left unfiled: the finding is
/// what is under test.
struct Case {
    dir: PathBuf,
    c: Challenges,
    id: String,
    fp: String,
}

impl Case {
    fn new(name: &str, severity: Severity) -> Case {
        Case::by(name, severity, "orc-o", &[])
    }

    fn by(name: &str, severity: Severity, author: &str, chain: &[&str]) -> Case {
        let dir = temp(name);
        let c = Challenges::at(&dir.join("challenges.jsonl"));
        let l = route(&Roster::default(), "run-1:1", &[FILE.to_owned()]);
        c.open_at(
            &l,
            &Opening {
                tree: "ab".repeat(32),
                by: author.to_owned(),
                chain: chain.iter().map(|s| (*s).to_owned()).collect(),
                ..Opening::default()
            },
        )
        .unwrap();
        let f = found(severity);
        let fp = f.fingerprint().to_owned();
        c.found(&l.id, f).unwrap();
        Case {
            dir,
            c,
            id: l.id,
            fp,
        }
    }

    fn sent(body: &str) -> Staff {
        Staff::Commissioned {
            body: body.to_owned(),
            by: "rimuru".to_owned(),
            wt: 3,
            waited: 0,
        }
    }

    fn assign(&self, body: &str) -> Result<(), Refusal> {
        self.c.assign(&self.id, &self.fp, Case::sent(body))
    }

    fn filing(&self, by: &str, ruling: Ruling) -> Filing {
        Filing {
            landing: self.id.clone(),
            fingerprint: self.fp.clone(),
            verifier: by.to_owned(),
            ruling,
        }
    }

    fn rule(&self, by: &str, ruling: Ruling) -> Result<Status, Refusal> {
        self.c.verdict(self.filing(by, ruling), &tree(), &ran())
    }

    fn finding(&self) -> super::Finding {
        self.c.record(&self.id).unwrap().findings.remove(0)
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn confirmed(severity: Severity) -> Ruling {
    Ruling::Confirmed {
        severity,
        reran: vec![Rerun {
            run: RERUN.into(),
            sha256: String::new(),
        }],
    }
}

fn refuted() -> Ruling {
    Ruling::Refuted {
        reason: "read_path is only reached through the grant check in files.rs".into(),
        files: vec![Pinned {
            path: FILE.into(),
            sha256: sha256_hex(SOURCE.as_bytes()),
        }],
        reran: Vec::new(),
    }
}

fn cannot() -> Ruling {
    Ruling::CannotVerify {
        blocker: "the proof-of-concept needs a network the sandbox denies".into(),
    }
}

fn unit(r: Result<Status, Refusal>) -> Result<(), Refusal> {
    r.map(|_| ())
}

fn blocked(s: Option<&Status>) -> bool {
    matches!(s, Some(Status::Blocked { .. }))
}

/// A confirmed high finding counts, keeps its severity, blocks the landing, and is a confirmed
/// outcome of its finder's speciality; the verdict keeps the verifier and the evidence hashes.
#[test]
fn a_confirmed_finding_counts_and_blocks() {
    let k = Case::new("confirmed", Severity::High);
    // Unverified, a high finding waits; it neither counts nor blocks.
    assert!(matches!(k.c.status(&k.id), Some(Status::Pending { .. })));
    assert!(!k.finding().counts() && !k.finding().blocks());
    k.assign(AGENT).unwrap();
    let Ok(Status::Blocked { findings }) = k.rule(AGENT, confirmed(Severity::High)) else {
        panic!("a confirmed high finding blocks");
    };
    assert_eq!(findings[0].fingerprint, k.fp);
    assert!(
        findings[0]
            .to_string()
            .contains("confirmed by dark-elf-agent")
    );
    let f = k.finding();
    assert!(f.counts() && f.blocks());
    let v = f.verdict.unwrap();
    assert_eq!((v.verifier.as_str(), v.speciality.as_str()), (AGENT, AGENT));
    assert_eq!(v.witness_sha256, sha256_hex(POC_BODY.as_bytes()));
    let Ruling::Confirmed { reran, .. } = &v.ruling else {
        panic!("confirmed");
    };
    // The harness stamped the output hash from the journal.
    assert_eq!(reran[0].sha256, sha256_hex(OUTPUT.as_bytes()));
    let rows = k.c.outcomes();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].confirmed);
    assert_eq!(rows[0].finder_speciality, FINDER);
}

/// A confirmed medium counts and does not block; a confirmation lowering a high to medium lifts
/// its block.
#[test]
fn a_confirmed_medium_counts_without_blocking() {
    let k = Case::new("medium", Severity::High);
    k.assign(AGENT).unwrap();
    assert!(matches!(
        k.rule(AGENT, confirmed(Severity::Medium)),
        Ok(Status::Pending { .. })
    ));
    let f = k.finding();
    assert!(f.counts() && !f.blocks());
    assert_eq!(f.severity(), Severity::Medium);
}

/// A refuted finding is demoted: it never counts nor blocks — not even a critical one, which
/// blocked while it was unverified — and it is a refuted outcome of its finder's speciality.
#[test]
fn a_refuted_finding_is_demoted() {
    let k = Case::new("refuted", Severity::Critical);
    assert!(
        blocked(k.c.status(&k.id).as_ref()),
        "an unverified critical blocks"
    );
    k.assign(AGENT).unwrap();
    assert!(matches!(
        k.rule(AGENT, refuted()),
        Ok(Status::Pending { .. })
    ));
    let f = k.finding();
    assert!(!f.counts() && !f.blocks());
    let rows = k.c.outcomes();
    assert!(!rows[0].confirmed);
    assert_eq!(rows[0].finder, FINDER);
}

/// `cannot-verify` never counts; a critical one blocks, for the human to decide; a high one does
/// not, and nothing waits on it.
#[test]
fn cannot_verify_blocks_only_a_critical() {
    let k = Case::new("cannot-critical", Severity::Critical);
    k.assign(AGENT).unwrap();
    let Ok(Status::Blocked { findings }) = k.rule(AGENT, cannot()) else {
        panic!("a critical that cannot be verified blocks");
    };
    assert!(findings[0].to_string().contains("the human decides"));
    assert!(!k.finding().counts());
    assert!(k.c.outcomes().is_empty(), "no outcome for a precision fold");
    let k = Case::new("cannot-high", Severity::High);
    k.assign(AGENT).unwrap();
    k.rule(AGENT, cannot()).unwrap();
    let f = k.finding();
    assert!(!f.counts() && !f.blocks() && !f.unverified());
}

/// With every ledger filed, an unverified high finding keeps the challenge verifying, never
/// cleared in silence.
#[test]
fn an_unverified_finding_keeps_the_challenge_verifying() {
    let dir = temp("verifying");
    let c = Challenges::at(&dir.join("c.jsonl"));
    // The generalist alone is routed a declared-surface-free landing: its empty ledger files.
    let l = route(
        &Roster::default(),
        "run-2:1",
        &[FILE.to_owned(), "Cargo.lock".to_owned()],
    );
    c.open(&l).unwrap();
    let f = found(Severity::High);
    let fp = f.fingerprint().to_owned();
    c.found(&l.id, f).unwrap();
    let mut open = c.hold().unwrap().all.remove(&l.id).unwrap();
    for r in &l.routed {
        open.filed.insert(
            r.speciality,
            mnemorph_proving::coverage::CoverageLedger {
                landing: l.id.clone(),
                speciality: r.speciality,
                entries: Vec::new(),
            },
        );
    }
    assert_eq!(open.status(), Status::Verifying { findings: vec![fp] });
    assert!(open.status().open());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A seal is proposed — once, only for a finding confirmed at high or critical, its witness
/// frozen by hash — and never applied: the landing stays as its verdict left it.
#[test]
fn a_seal_is_proposed_never_applied() {
    let k = Case::new("seal", Severity::High);
    let refused = |r: Result<_, Refusal>| matches!(r, Err(Refusal::Verification(g)) if g == [Guard::Sealable]);
    assert!(refused(k.c.propose(&k.id, &k.fp, "rimuru")), "unverified");
    k.assign(AGENT).unwrap();
    k.rule(AGENT, confirmed(Severity::High)).unwrap();
    let before = k.c.status(&k.id);
    let p = k.c.propose(&k.id, &k.fp, "rimuru").unwrap();
    assert_eq!(p.witness.sha256, sha256_hex(POC_BODY.as_bytes()));
    assert_eq!((p.verifier.as_str(), p.by.as_str()), (AGENT, "rimuru"));
    assert_eq!(k.finding().proposal, Some(p));
    assert_eq!(k.c.status(&k.id), before, "a proposal applies nothing");
    assert!(refused(k.c.propose(&k.id, &k.fp, "rimuru")), "once");
    // Confirmed at medium: no seal.
    let k = Case::new("seal-medium", Severity::High);
    k.assign(AGENT).unwrap();
    k.rule(AGENT, confirmed(Severity::Medium)).unwrap();
    assert!(refused(k.c.propose(&k.id, &k.fp, "rimuru")));
}

/// The verifier is never the finder, nor a body of its speciality, nor the landing's author or
/// a body above it; the generalist may verify.
#[test]
fn the_verifier_is_independent() {
    let k = Case::by(
        "independent",
        Severity::High,
        "slime-s",
        &["orc-o", "dark-elf-web"],
    );
    let independent = |r: Result<(), Refusal>| matches!(r, Err(Refusal::Verification(g)) if g == [Guard::Independent]);
    assert!(independent(k.assign(FINDER)), "the finder");
    assert!(independent(k.assign("dark-elf-web")), "above the author");
    assert!(independent(k.assign("orc-o")), "not a Dark Elf");
    let k = Case::by("author", Severity::High, AGENT, &[]);
    assert!(independent(k.assign(AGENT)), "the author");
    assert!(k.assign("dark-elf").is_ok(), "the generalist");
    // Queued or unstaffed carries no body to hold.
    assert!(k.c.assign(&k.id, &k.fp, Staff::Unstaffed).is_ok());
}

/// Each guard refuses its case alone, and with that guard switched off the same case is let
/// through: each test below fails without its guard.
#[test]
#[allow(clippy::too_many_lines)] // One case per guard, side by side.
fn each_guard_is_the_one_its_case_needs() {
    type Act = fn(&Case, Option<Guard>) -> Result<(), Refusal>;
    let cases: [(Guard, Severity, Act); 12] = [
        (Guard::Floor, Severity::High, |k, off| {
            unit(k.c.found_with(&k.id, found(Severity::Low), off))
        }),
        (Guard::Finder, Severity::High, |k, off| {
            let mut f = found(Severity::High);
            f.finder = AGENT.into();
            f.record.symbol = "other".into();
            f.record.fingerprint = "f".repeat(16);
            unit(k.c.found_with(&k.id, f, off))
        }),
        (Guard::Assigned, Severity::High, |k, off| {
            k.assign("dark-elf-web").unwrap();
            let f = k.filing(AGENT, confirmed(Severity::High));
            unit(k.c.verdict_with(f, &tree(), &ran(), off))
        }),
        (Guard::Independent, Severity::High, |k, off| {
            k.c.assign_with(&k.id, &k.fp, Case::sent(FINDER), off)
        }),
        (Guard::Once, Severity::High, |k, off| {
            k.assign(AGENT).unwrap();
            k.rule(AGENT, cannot()).unwrap();
            let f = k.filing(AGENT, confirmed(Severity::High));
            // A ruled finding takes no second verifier either.
            if off != Some(Guard::Once) {
                assert!(k.assign("dark-elf").is_err());
            }
            unit(k.c.verdict_with(f, &tree(), &ran(), off))
        }),
        (Guard::NoRaise, Severity::High, |k, off| {
            k.assign(AGENT).unwrap();
            let f = k.filing(AGENT, confirmed(Severity::Critical));
            unit(k.c.verdict_with(f, &tree(), &ran(), off))
        }),
        (Guard::Rederived, Severity::High, |k, off| {
            k.assign(AGENT).unwrap();
            let f = k.filing(AGENT, confirmed(Severity::High));
            unit(k.c.verdict_with(f, &tree(), &Ran::default(), off))
        }),
        (Guard::Reproduced, Severity::Medium, |k, off| {
            // A command's captured output: the re-run must print it again.
            let mut f = found(Severity::Medium);
            let out = ".mnemorph/dark-elf/dark-elf-harness/poc/out.txt";
            f.record.symbol = "read".into();
            f.record.fingerprint =
                mnemorph_wire::fingerprint(FINDER, "path-traversal", FILE, "read").unwrap();
            f.record.witness = Some(Witness {
                kind: WitnessKind::PocCommand,
                reference: RERUN.into(),
                sha256: sha256_hex(RERUN.as_bytes()),
                output: Some(out.into()),
                output_sha256: Some(sha256_hex(b"what it printed then")),
            });
            let fp = f.record.fingerprint.clone();
            k.c.found(&k.id, f).unwrap();
            k.c.assign(&k.id, &fp, Case::sent(AGENT)).unwrap();
            let mut t = tree();
            t.insert(out.into(), b"what it printed then".to_vec());
            let filing = Filing {
                fingerprint: fp,
                ..k.filing(AGENT, confirmed(Severity::Medium))
            };
            unit(k.c.verdict_with(filing, &t, &ran(), off))
        }),
        (Guard::Unchanged, Severity::High, |k, off| {
            k.assign(AGENT).unwrap();
            let mut t = tree();
            t.insert(POC.into(), b"#[test]\nfn passes() {}\n".to_vec());
            let f = k.filing(AGENT, confirmed(Severity::High));
            unit(k.c.verdict_with(f, &t, &ran(), off))
        }),
        (Guard::Reasoned, Severity::High, |k, off| {
            k.assign(AGENT).unwrap();
            let f = k.filing(
                AGENT,
                Ruling::CannotVerify {
                    blocker: "n/a".into(),
                },
            );
            unit(k.c.verdict_with(f, &tree(), &ran(), off))
        }),
        (Guard::Evidenced, Severity::High, |k, off| {
            k.assign(AGENT).unwrap();
            let Ruling::Refuted { reason, .. } = refuted() else {
                unreachable!()
            };
            let stale = Ruling::Refuted {
                reason,
                files: vec![Pinned {
                    path: FILE.into(),
                    sha256: "0".repeat(64),
                }],
                reran: Vec::new(),
            };
            unit(k.c.verdict_with(k.filing(AGENT, stale), &tree(), &ran(), off))
        }),
        (Guard::Sealable, Severity::High, |k, off| {
            k.c.propose_with(&k.id, &k.fp, "rimuru", off).map(|_| ())
        }),
    ];
    assert_eq!(cases.map(|c| c.0), Guard::ALL, "every guard has its case");
    for (n, (guard, severity, act)) in cases.into_iter().enumerate() {
        let k = Case::new(&format!("guard-{n}-on"), severity);
        assert_eq!(
            act(&k, None),
            Err(Refusal::Verification(vec![guard])),
            "{guard} refuses its case alone"
        );
        let k = Case::new(&format!("guard-{n}-off"), severity);
        assert_eq!(
            act(&k, Some(guard)),
            Ok(()),
            "{guard} off lets its case through"
        );
        assert_ne!(guard.asks(), "");
    }
    assert!(
        Refusal::Verification(vec![Guard::Assigned])
            .to_string()
            .contains("not assigned")
    );
}

/// A verification row written by anything but the harness counts only if it holds on read: a
/// verdict by a body never assigned, a verifier that is the finder, a seal for an unconfirmed
/// finding all count for nothing.
#[test]
fn the_fold_checks_every_verification_row_again() {
    let k = Case::new("fold", Severity::High);
    let line = |row: &Row| serde_json::to_string(row).unwrap();
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(k.dir.join("challenges.jsonl"))
        .unwrap();
    let forged = super::Verification {
        fingerprint: k.fp.clone(),
        verifier: AGENT.into(),
        speciality: AGENT.into(),
        finder: FINDER.into(),
        finder_speciality: FINDER.into(),
        witness_sha256: sha256_hex(POC_BODY.as_bytes()),
        output_sha256: String::new(),
        ruling: Ruling::Confirmed {
            severity: Severity::High,
            reran: vec![Rerun {
                run: RERUN.into(),
                sha256: sha256_hex(OUTPUT.as_bytes()),
            }],
        },
    };
    let rows = [
        Row::Assigned {
            landing: k.id.clone(),
            fingerprint: k.fp.clone(),
            staff: Case::sent(FINDER),
        },
        Row::Ruled {
            landing: k.id.clone(),
            verification: forged.clone(),
        },
        Row::Proposed {
            landing: k.id.clone(),
            proposal: super::Proposal {
                fingerprint: k.fp.clone(),
                speciality: FINDER.into(),
                file: FILE.into(),
                symbol: "read_path".into(),
                severity: Severity::High,
                witness: record(Severity::High).witness.unwrap(),
                verifier: AGENT.into(),
                by: "rimuru".into(),
            },
        },
        Row::Found {
            landing: k.id.clone(),
            found: Box::new({
                let mut low = found(Severity::Low);
                low.record.symbol = "other".into();
                low.record.fingerprint =
                    mnemorph_wire::fingerprint(FINDER, "path-traversal", FILE, "other").unwrap();
                low
            }),
        },
    ];
    for row in &rows {
        writeln!(f, "{}", line(row)).unwrap();
    }
    drop(f);
    let r = k.c.record(&k.id).unwrap();
    assert_eq!(r.findings.len(), 1, "the low finding is not kept");
    let finding = &r.findings[0];
    assert_eq!(finding.staff, None, "the finder never verifies");
    assert_eq!(
        finding.verdict, None,
        "an unassigned verdict counts for nothing"
    );
    assert_eq!(finding.proposal, None);
    // Assigned for real, the same verdict row now holds.
    k.assign(AGENT).unwrap();
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(k.dir.join("challenges.jsonl"))
        .unwrap();
    writeln!(
        f,
        "{}",
        line(&Row::Ruled {
            landing: k.id.clone(),
            verification: forged,
        })
    )
    .unwrap();
    drop(f);
    assert!(k.finding().counts());
}

/// Refusals for what is not there.
#[test]
fn no_such_landing_or_finding() {
    let k = Case::new("none", Severity::High);
    assert!(matches!(
        k.c.assign("nope", &k.fp, Staff::Unstaffed),
        Err(Refusal::NoSuchLanding(_))
    ));
    assert!(matches!(
        k.c.assign(&k.id, "nope", Staff::Unstaffed),
        Err(Refusal::NoSuchLanding(_))
    ));
    assert!(matches!(
        k.c.found("nope", found(Severity::High)),
        Err(Refusal::NoSuchLanding(_))
    ));
    let mut f = k.filing(AGENT, cannot());
    f.fingerprint = "nope".into();
    assert!(matches!(
        k.c.verdict(f, &tree(), &ran()),
        Err(Refusal::NoSuchLanding(_))
    ));
    assert!(matches!(
        k.c.propose(&k.id, "nope", "rimuru"),
        Err(Refusal::NoSuchLanding(_))
    ));
    // One fingerprint is kept once.
    k.c.found(&k.id, found(Severity::High)).unwrap();
    assert_eq!(k.c.record(&k.id).unwrap().findings.len(), 1);
    let missing = Challenges::at(&k.dir.join("absent/c.jsonl"));
    assert!(matches!(
        missing.verdict(k.filing(AGENT, cannot()), &tree(), &ran()),
        Err(Refusal::NoSuchLanding(_))
    ));
    for r in [confirmed(Severity::High), refuted(), cannot()] {
        assert_ne!(r.word(), "");
    }
}
