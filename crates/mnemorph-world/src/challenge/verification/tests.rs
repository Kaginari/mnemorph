use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;

use mnemorph_proving::coverage::Ran;
use mnemorph_proving::{
    FindingRecord, Impact, Likelihood, Severity, Step, StepKind, Verdict, Witness, WitnessKind,
    sha256_hex,
};

use super::super::{Build, Challenges, Opening, Refusal, Row, Staff, Status, route};
use super::{Decision, Filing, Found, Guard, Pinned, Rerun, Ruling};
use crate::roster::Roster;

const FILE: &str = "crates/mnemorph-tools/src/files.rs";
const SOURCE: &str = "pub fn read_path(p: &str) -> String { todo!() }\n";
const POC: &str = ".mnemorph/dark-elf/dark-elf-harness/poc/read_path.rs";
const POC_BODY: &str = "#[test]\nfn escapes() {}\n";
const RERUN: &str = "rustc --test .mnemorph/dark-elf/dark-elf-harness/poc/read_path.rs";
const TREE: &str = "abababababababababababababababababababababababababababababababab";
const LOWER: &str = "the read stays inside files the grant already shows";
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
    r.record_exit(RERUN, OUTPUT, true);
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
            tree: TREE.to_owned(),
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
    rerun(severity, RERUN)
}

/// A confirmation citing `command`, with the reason a lowered severity needs.
fn rerun(severity: Severity, command: &str) -> Ruling {
    Ruling::Confirmed {
        severity,
        reason: LOWER.into(),
        reran: vec![Rerun {
            run: command.into(),
            sha256: String::new(),
            failed: false,
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

/// A refuted finding is demoted: it never counts nor blocks, and it is a refuted outcome of its
/// finder's speciality.
#[test]
fn a_refuted_finding_is_demoted() {
    let k = Case::new("refuted", Severity::High);
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

/// One verifier never lifts a critical's block: refuted, or confirmed below high, it still
/// blocks, said as the human's to decide, until the human waives it.
#[test]
fn a_critical_stays_blocked_whatever_one_verifier_says() {
    for (n, ruling) in [refuted(), confirmed(Severity::Medium)]
        .into_iter()
        .enumerate()
    {
        let k = Case::new(&format!("critical-held-{n}"), Severity::Critical);
        assert!(
            blocked(k.c.status(&k.id).as_ref()),
            "an unverified critical blocks"
        );
        k.assign(AGENT).unwrap();
        let word = ruling.word();
        let Ok(Status::Blocked { findings }) = k.rule(AGENT, ruling) else {
            panic!("a {word} critical still blocks");
        };
        let said = findings[0].to_string();
        assert!(
            said.contains("critical") && said.contains("the human decides"),
            "{said}"
        );
        assert!(k.finding().blocks());
        let waiver =
            super::super::Waiver::human("the human read the refutation and accepts it").unwrap();
        assert!(matches!(
            k.c.waive(&k.id, &waiver),
            Ok(Status::Waived { .. })
        ));
    }
}

/// A confirmation that lowers the severity says why; one that keeps it needs no reason.
#[test]
fn a_lowered_severity_needs_its_reason() {
    let k = Case::new("lowered", Severity::High);
    k.assign(AGENT).unwrap();
    let bare = Ruling::Confirmed {
        severity: Severity::Medium,
        reason: String::new(),
        reran: vec![Rerun {
            run: RERUN.into(),
            sha256: String::new(),
            failed: false,
        }],
    };
    assert_eq!(
        k.rule(AGENT, bare),
        Err(Refusal::Verification(vec![Guard::Reasoned]))
    );
    let kept = Ruling::Confirmed {
        severity: Severity::High,
        reason: String::new(),
        reran: vec![Rerun {
            run: RERUN.into(),
            sha256: String::new(),
            failed: false,
        }],
    };
    assert!(k.rule(AGENT, kept).is_ok());
}

/// A re-run confirms only when it re-runs the witness: `echo ok` never does; `cat` of a test
/// witness reads it but does not run it to its failure; a command witness is re-run only by its
/// own command, whatever else prints its file.
#[test]
fn only_a_re_run_of_the_witness_confirms() {
    let rederived = |r: Result<Status, Refusal>| matches!(r, Err(Refusal::Verification(g)) if g.contains(&Guard::Rederived));
    let mut journal = ran();
    journal.record("echo ok", "ok");
    let cat = format!("cat {POC}");
    journal.record(&cat, POC_BODY);
    let k = Case::new("echo", Severity::High);
    k.assign(AGENT).unwrap();
    for command in ["echo ok", cat.as_str()] {
        let f = k.filing(AGENT, rerun(Severity::High, command));
        assert!(rederived(k.c.verdict(f, &tree(), &journal)), "{command}");
    }
    // Written into the records, a `cat` that did not fail counts for nothing on read either.
    let forged = super::Verification {
        fingerprint: k.fp.clone(),
        verifier: AGENT.into(),
        speciality: AGENT.into(),
        finder: FINDER.into(),
        finder_speciality: FINDER.into(),
        tree: TREE.into(),
        witness_sha256: sha256_hex(POC_BODY.as_bytes()),
        output_sha256: String::new(),
        ruling: Ruling::Confirmed {
            severity: Severity::High,
            reason: String::new(),
            reran: vec![Rerun {
                run: cat.clone(),
                sha256: sha256_hex(POC_BODY.as_bytes()),
                failed: false,
            }],
        },
    };
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(k.dir.join("challenges.jsonl"))
        .unwrap();
    let row = Row::Ruled {
        landing: k.id.clone(),
        verification: forged,
        build: Build::default(),
    };
    writeln!(file, "{}", serde_json::to_string(&row).unwrap()).unwrap();
    drop(file);
    assert!(
        k.finding().unverified(),
        "a re-run that did not fail rederives nothing"
    );
    // A command witness: `cat` of its output file is no re-run of it.
    let k = Case::new("poc-cat", Severity::Medium);
    let out = ".mnemorph/dark-elf/dark-elf-harness/poc/out.txt";
    let mut f = found(Severity::Medium);
    f.record.symbol = "read".into();
    f.record.fingerprint =
        mnemorph_wire::fingerprint(FINDER, "path-traversal", FILE, "read").unwrap();
    f.record.witness = Some(Witness {
        kind: WitnessKind::PocCommand,
        reference: "sh poc.sh".into(),
        sha256: sha256_hex(b"sh poc.sh"),
        output: Some(out.into()),
        output_sha256: Some(sha256_hex(OUTPUT.as_bytes())),
    });
    let fp = f.record.fingerprint.clone();
    k.c.found(&k.id, f).unwrap();
    k.c.assign(&k.id, &fp, Case::sent(AGENT)).unwrap();
    let mut t = tree();
    t.insert(out.into(), OUTPUT.as_bytes().to_vec());
    let cat = format!("cat {out}");
    let mut journal = ran();
    journal.record(&cat, OUTPUT);
    journal.record("sh poc.sh", OUTPUT);
    let filing = |command: &str| Filing {
        fingerprint: fp.clone(),
        ..k.filing(AGENT, rerun(Severity::Medium, command))
    };
    let cat_refused = k.c.verdict(filing(&cat), &t, &journal);
    assert!(rederived(cat_refused.clone()), "{cat_refused:?}");
    assert!(k.c.verdict(filing("sh poc.sh"), &t, &journal).is_ok());
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
    let cases: [(Guard, Severity, Act); 13] = [
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
        (Guard::Current, Severity::High, |k, off| {
            k.assign(AGENT).unwrap();
            let f = Filing {
                tree: "f00d".into(),
                ..k.filing(AGENT, confirmed(Severity::High))
            };
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
        tree: TREE.into(),
        witness_sha256: sha256_hex(POC_BODY.as_bytes()),
        output_sha256: String::new(),
        ruling: Ruling::Confirmed {
            severity: Severity::High,
            reason: String::new(),
            reran: vec![Rerun {
                run: RERUN.into(),
                sha256: sha256_hex(OUTPUT.as_bytes()),
                failed: true,
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
            build: Build::default(),
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
            build: Build::default(),
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
            build: Build::default(),
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

/// A finding of the harness on `symbol`: its own root cause.
fn found_on(symbol: &str, severity: Severity) -> Found {
    let mut f = found(severity);
    f.record.symbol = symbol.into();
    f.record.fingerprint =
        mnemorph_wire::fingerprint(FINDER, "path-traversal", FILE, symbol).unwrap();
    f.record.rec = mnemorph_proving::record_path(FINDER, &f.record.fingerprint);
    f
}

/// Opens landing `id` on `c`, by `orc-o`, on the test tree.
fn open_landing(c: &Challenges, id: &str) {
    let l = route(&Roster::default(), id, &[FILE.to_owned()]);
    c.open_at(
        &l,
        &Opening {
            tree: TREE.into(),
            by: "orc-o".into(),
            ..Opening::default()
        },
    )
    .unwrap();
}

/// Keeps `f` on `landing` and has the agent rule it.
fn found_and_ruled(c: &Challenges, landing: &str, f: Found, ruling: Option<Ruling>) -> String {
    let fp = f.fingerprint().to_owned();
    c.found(landing, f).unwrap();
    if let Some(ruling) = ruling {
        c.assign(landing, &fp, Case::sent(AGENT)).unwrap();
        let filing = Filing {
            landing: landing.into(),
            fingerprint: fp.clone(),
            verifier: AGENT.into(),
            tree: TREE.into(),
            ruling,
        };
        c.verdict(filing, &tree(), &ran()).unwrap();
    }
    fp
}

fn harness_of(c: &Challenges) -> mnemorph_ledger::SpecialityFold {
    mnemorph_ledger::fold_findings(&c.finding_rows())
        .remove(FINDER)
        .unwrap_or_default()
}

/// A confirmed, a refuted, a `cannot-verify` and an unverified finding fold into their
/// speciality's ledger: precision is upheld over upheld and refuted, `cannot-verify` in neither.
#[test]
fn the_ledger_folds_the_verdicts() {
    let dir = temp("ledger-fold");
    let c = Challenges::at(&dir.join("challenges.jsonl"));
    open_landing(&c, "run-1:1");
    found_and_ruled(
        &c,
        "run-1:1",
        found_on("a", Severity::High),
        Some(confirmed(Severity::High)),
    );
    found_and_ruled(
        &c,
        "run-1:1",
        found_on("b", Severity::High),
        Some(refuted()),
    );
    found_and_ruled(&c, "run-1:1", found_on("c", Severity::High), Some(cannot()));
    found_and_ruled(&c, "run-1:1", found_on("d", Severity::Medium), None);
    let f = harness_of(&c);
    assert_eq!(
        (
            f.findings,
            f.upheld,
            f.refuted,
            f.cannot_verify,
            f.unverified
        ),
        (4, 1, 1, 1, 1)
    );
    assert_eq!(f.precision(), Some(50.0));
    assert_eq!(f.upheld_rate(), Some(25.0));
    // The window is in the order the verdicts were filed.
    assert_eq!(f.recent, vec![true, false]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every row that settles a finding carries the build that wrote it, the one `status` prints; a
/// row written before rows were stamped reads as `unknown`, and a fold by build keeps the two
/// apart.
#[test]
fn settling_rows_carry_their_build() {
    let dir = temp("ledger-build");
    let file = dir.join("challenges.jsonl");
    let c = Challenges::at(&file);
    open_landing(&c, "run-1:1");
    let fp = found_and_ruled(
        &c,
        "run-1:1",
        found_on("a", Severity::High),
        Some(confirmed(Severity::High)),
    );
    found_and_ruled(&c, "run-1:1", found_on("b", Severity::Medium), None);
    let this = Build::this();
    let rows = c.finding_rows();
    assert!(rows.iter().all(|r| r.build == this), "{rows:?}");
    let text = std::fs::read_to_string(&file).unwrap();
    let stamped: Vec<&str> = text
        .lines()
        .filter(|l| l.contains(r#""row":"found""#) || l.contains(r#""row":"ruled""#))
        .collect();
    assert_eq!(stamped.len(), 3);
    for l in &stamped {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert_eq!(v["build"]["version"], this.version.as_str());
        assert_eq!(v["build"]["commit"], this.commit.as_str());
    }
    // The same records as an older build wrote them, with no stamp: `unknown`.
    let mut old = String::new();
    for l in text.lines() {
        let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
        if let Some(o) = v.as_object_mut() {
            o.remove("build");
        }
        old.push_str(&v.to_string());
        old.push('\n');
    }
    std::fs::write(&file, old).unwrap();
    let rows = c.finding_rows();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.build == Build::default()));
    assert_eq!(harness_of(&c).upheld, 1, "an unstamped row still folds");
    // A later decision under this build settles `a` again: the fold tells the builds apart.
    let reason = "the fix proved the read escaped the grant";
    c.decide("run-1:1", &fp, &Decision::human(true, reason).unwrap())
        .unwrap();
    let f = harness_of(&c);
    assert_eq!(f.builds[&this], 1);
    assert_eq!(f.builds[&Build::default()], 1);
    let by = super::super::fold_findings_by_build(&c.finding_rows());
    assert_eq!(by[&this][FINDER].upheld, 1);
    assert_eq!(by[&Build::default()][FINDER].unverified, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A critical finding blocks whatever line follows it: a torn row after it changes nothing; the
/// critical's own row, unreadable to this binary, holds the landing pending on it — never
/// cleared — and with a field this binary does not know, it reads and blocks.
#[test]
fn a_critical_blocks_through_an_unreadable_row() {
    let dir = temp("unreadable-critical");
    let file = dir.join("challenges.jsonl");
    let c = Challenges::at(&file);
    open_landing(&c, "run-1:1");
    let blocked = |c: &Challenges| matches!(c.status("run-1:1"), Some(Status::Blocked { .. }));
    assert!(matches!(
        c.found("run-1:1", found(Severity::Critical)),
        Ok(Status::Blocked { .. })
    ));
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        format!("{text}{{\"row\":\"found\",\"landing\":\"run-1:\n"),
    )
    .unwrap();
    assert!(blocked(&c), "a torn line after a critical still blocks");
    // The critical's row as a newer binary wrote it, of a kind this one does not know.
    let n = u64::try_from(text.lines().count()).unwrap();
    let newer = text.replace(r#""row":"found""#, r#""row":"found-v2""#);
    std::fs::write(&file, &newer).unwrap();
    assert!(matches!(
        c.status("run-1:1"),
        Some(Status::Pending { ref unreadable, .. }) if unreadable == &[n]
    ));
    // With a field this binary does not know, it reads, and blocks.
    let extra = text.replace(r#""row":"found","#, r#""row":"found","future":1,"#);
    assert_ne!(extra, text);
    std::fs::write(&file, &extra).unwrap();
    assert!(blocked(&c));
    let _ = std::fs::remove_dir_all(&dir);
}

/// One root cause found on two landings — refuted on the first, confirmed on the second after
/// its line moved — counts once, upheld: the fingerprint is line-free.
#[test]
fn the_ledger_counts_a_root_cause_once_across_landings() {
    let dir = temp("ledger-dedupe");
    let c = Challenges::at(&dir.join("challenges.jsonl"));
    open_landing(&c, "run-1:1");
    open_landing(&c, "run-2:1");
    let first = found_and_ruled(
        &c,
        "run-1:1",
        found_on("a", Severity::High),
        Some(refuted()),
    );
    let mut moved = found_on("a", Severity::High);
    moved.record.trace[0].at = format!("{FILE}:7");
    let second = found_and_ruled(&c, "run-2:1", moved, Some(confirmed(Severity::High)));
    assert_eq!(first, second);
    assert_eq!(c.finding_rows().len(), 2, "each landing keeps its row");
    let f = harness_of(&c);
    assert_eq!((f.findings, f.upheld, f.refuted), (1, 1, 0));
    assert_eq!(f.precision(), Some(100.0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The human decides: a confirmed finding dismissed as a false positive counts refuted; a
/// critical its verifier refuted is neither until the human upholds it. Only the human's
/// reasoned decision, bound to the opened tree, is kept.
#[test]
fn the_human_decides_what_a_verifier_cannot() {
    let dir = temp("ledger-human");
    let c = Challenges::at(&dir.join("challenges.jsonl"));
    open_landing(&c, "run-1:1");
    let fp = found_and_ruled(
        &c,
        "run-1:1",
        found_on("a", Severity::High),
        Some(confirmed(Severity::High)),
    );
    let crit = found_and_ruled(
        &c,
        "run-1:1",
        found_on("b", Severity::Critical),
        Some(refuted()),
    );
    let f = harness_of(&c);
    assert_eq!((f.upheld, f.refuted, f.held), (1, 0, 1));
    let reason = "the grant check refuses the path before any read";
    c.decide("run-1:1", &fp, &Decision::human(false, reason).unwrap())
        .unwrap();
    let held = "the proof-of-concept escapes once the grant is widened";
    c.decide("run-1:1", &crit, &Decision::human(true, held).unwrap())
        .unwrap();
    let f = harness_of(&c);
    assert_eq!((f.upheld, f.refuted, f.held), (1, 1, 0));
    let r = c.record("run-1:1").unwrap();
    let d = r.findings[0].decision.as_ref().unwrap();
    assert!(!d.upheld() && d.reason() == reason);
    assert!(r.status.open(), "a decision lifts no block");
    // Refused: no reason worth the name, no such finding, no such landing.
    assert!(matches!(
        Decision::human(true, "n/a"),
        Err(Refusal::Unreasoned)
    ));
    let ok = Decision::human(true, held).unwrap();
    assert!(matches!(
        c.decide("run-1:1", "nope", &ok),
        Err(Refusal::NoSuchLanding(_))
    ));
    assert!(matches!(
        c.decide("nope", &fp, &ok),
        Err(Refusal::NoSuchLanding(_))
    ));
    let mut bare = ok.clone();
    bare.reason = "short".into();
    assert!(matches!(
        c.decide("run-1:1", &fp, &bare),
        Err(Refusal::Unreasoned)
    ));
    // Forged decisions: not the human's, or bound to another tree, count for nothing.
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("challenges.jsonl"))
        .unwrap();
    let mut body = ok.clone();
    body.by = "dark-elf-harness".into();
    body.tree = TREE.into();
    let mut other = ok;
    other.tree = "cd".repeat(32);
    for decision in [body, other] {
        let row = Row::Decided {
            landing: "run-1:1".into(),
            fingerprint: fp.clone(),
            decision,
            build: Build::default(),
        };
        writeln!(f, "{}", serde_json::to_string(&row).unwrap()).unwrap();
    }
    drop(f);
    let f = harness_of(&c);
    assert_eq!((f.upheld, f.refuted), (1, 1), "the forged rows are ignored");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The ledger is folded from the records outside the world alone: challenge rows and finding
/// records forged inside the world are never read, and records kept inside it are refused. With
/// the source check off, the forged rows would count: the case needs it.
#[test]
fn the_ledger_reads_only_the_records_outside_the_world() {
    let dir = temp("ledger-source");
    let world = dir.join("world");
    let state = dir.join("state/challenges.jsonl");
    // A body forges a challenge with a confirmed finding inside the world.
    let inside = world.join(".mnemorph/challenges.jsonl");
    std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
    let forged = Challenges::at(&inside);
    open_landing(&forged, "run-1:1");
    found_and_ruled(
        &forged,
        "run-1:1",
        found_on("a", Severity::High),
        Some(confirmed(Severity::High)),
    );
    let rec = world.join(found_on("a", Severity::High).rec);
    std::fs::create_dir_all(rec.parent().unwrap()).unwrap();
    std::fs::write(&rec, serde_json::to_vec(&record(Severity::High)).unwrap()).unwrap();
    // The real records, outside: one refuted finding.
    let real = Challenges::at(&state);
    open_landing(&real, "run-1:1");
    found_and_ruled(
        &real,
        "run-1:1",
        found_on("b", Severity::High),
        Some(refuted()),
    );
    let folded = super::super::scores_at(&world, &state, true).unwrap();
    assert_eq!((folded[FINDER].findings, folded[FINDER].upheld), (1, 0));
    assert!(matches!(
        super::super::scores_at(&world, &inside, true),
        Err(Refusal::InWorld(_))
    ));
    // A records path not there yet inside the world is refused too.
    assert!(matches!(
        super::super::scores_at(&world, &world.join("new/c.jsonl"), true),
        Err(Refusal::InWorld(_))
    ));
    let unchecked = super::super::scores_at(&world, &inside, false).unwrap();
    assert_eq!(
        unchecked[FINDER].upheld, 1,
        "without the check the forgery counts"
    );
    assert!(
        Refusal::InWorld(inside)
            .to_string()
            .contains("inside the world")
    );
    assert!(Refusal::NoState.to_string().contains("no state directory"));
    // A world's own records, wherever the state directory is, fold or are refused — never panic.
    let _ = super::super::scores(&world);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A proposal names the records' chain head and the rows of the window it read: the chain walked
/// again holds at that head, and the rows inside the window, folded again, give the same lines.
#[test]
fn a_proposal_recomputes_from_the_chained_records() {
    let dir = temp("explained");
    let file = dir.join("challenges.jsonl");
    let c = Challenges::at(&file);
    open_landing(&c, "run-1:1");
    for i in 0..10 {
        let ruling = if i < 6 {
            refuted()
        } else {
            confirmed(Severity::High)
        };
        found_and_ruled(
            &c,
            "run-1:1",
            found_on(&format!("f{i}"), Severity::High),
            Some(ruling),
        );
    }
    let chain = c.chain().unwrap();
    assert!(chain.holds(), "{chain}");
    let head = chain.head.unwrap().digest;
    let p = harness_of(&c)
        .proposed(mnemorph_races::Race::ArchDarkElf, Some(&head))
        .unwrap();
    let e = &p.why[0];
    assert_eq!(e.head.as_deref(), Some(head.as_str()));
    assert!(
        e.to_string()
            .starts_with("probation: precision 0.40 < 0.70"),
        "{e}"
    );
    // The window's rows are the records' own lines, from 1: each a verdict that settled a finding.
    let text = std::fs::read_to_string(&file).unwrap();
    let (from, to) = (e.from.unwrap(), e.to.unwrap());
    for at in [from, to] {
        let line = text.lines().nth(usize::try_from(at - 1).unwrap()).unwrap();
        assert!(line.contains(r#""row":"ruled""#), "{line}");
    }
    let window: Vec<_> = c
        .finding_rows()
        .into_iter()
        .filter(|r| (from..=to).contains(&r.at))
        .collect();
    let again = mnemorph_ledger::fold_findings(&window)
        .remove(FINDER)
        .unwrap();
    assert_eq!(
        again.proposed(mnemorph_races::Race::ArchDarkElf, Some(&head)),
        Some(p.clone())
    );
    let _ = std::fs::remove_dir_all(&dir);
}
