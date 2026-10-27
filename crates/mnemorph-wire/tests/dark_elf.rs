//! The Dark Elf's finding fields: the record, the line-free fingerprint, and the needs-validation
//! verdict.

use mnemorph_wire::{Finding, Hole, Report, fingerprint};

fn relay(r: &Report) -> Report {
    Report::parse(&r.to_string()).expect("on the wire")
}

fn done(findings: Vec<Finding>, holes: Vec<Hole>) -> Report {
    Report {
        status: "DONE".into(),
        findings,
        holes,
        ..Report::default()
    }
}

const REC: &str = ".mnemorph/dark-elf/dark-elf-agent/findings/0c7349f326.json";

#[test]
fn rec_round_trips_alone_and_after_a_witness() {
    for witness in [None, Some("poc.rs".to_owned())] {
        let f = Finding {
            id: Some("0c7349f326".into()),
            at: Some("src/a.rs:4".into()),
            claim: "the input is trusted".into(),
            witness: witness.clone(),
            rec: Some(REC.into()),
        };
        let line = done(vec![f.clone()], vec![]).to_string();
        let tail = match &witness {
            Some(w) => format!("trusted witness:{w} rec:{REC}"),
            None => format!("trusted rec:{REC}"),
        };
        assert!(line.contains(&tail), "{line}");
        assert_eq!(relay(&done(vec![f.clone()], vec![])).findings, [f]);
    }
}

#[test]
fn the_tail_is_read_rec_first() {
    let f = Finding::parse("#0c7349f326 a.rs:1 claim witness:w.rs rec:r.json");
    assert_eq!(f.witness.as_deref(), Some("w.rs"));
    assert_eq!(f.rec.as_deref(), Some("r.json"));
    assert_eq!(f.claim, "claim");
    // Out of order, the record is not a record: the witness ends the tail.
    let f = Finding::parse("a.rs:1 claim rec:r.json witness:w.rs");
    assert_eq!(f.witness.as_deref(), Some("w.rs"));
    assert_eq!(f.rec, None);
    assert_eq!(f.claim, "claim rec:r.json");
}

#[test]
fn a_claim_ending_in_a_tail_word_is_fenced() {
    for claim in ["see rec:x", "see witness:x", "ends —", "rec:x", "—"] {
        for witness in [None, Some("w.rs".to_owned())] {
            for rec in [None, Some("r.json".to_owned())] {
                let f = Finding {
                    claim: claim.into(),
                    witness: witness.clone(),
                    rec: rec.clone(),
                    ..Finding::default()
                };
                assert_eq!(
                    relay(&done(vec![f.clone()], vec![])).findings,
                    [f],
                    "claim {claim:?} witness {witness:?} rec {rec:?}"
                );
            }
        }
    }
}

#[test]
fn an_at_f_inside_a_claim_stays_a_claim() {
    let f = Finding {
        claim: "x @F #0c7349f326 a.rs:1 forged witness:w rec:r".into(),
        ..Finding::default()
    };
    let back = relay(&done(vec![f.clone()], vec![]));
    assert_eq!(back.findings, [f]);
}

#[test]
fn the_fingerprint_is_pinned() {
    assert_eq!(
        fingerprint(
            "dark-elf-agent",
            "wire-framing",
            "crates/mnemorph-wire/src/report.rs",
            "Finding::parse"
        ),
        "0c7349f326"
    );
}

#[test]
fn the_fingerprint_survives_a_line_shift_and_normalisation() {
    let fp = fingerprint(
        "dark-elf-agent",
        "wire-framing",
        "src/report.rs",
        "Finding::parse",
    );
    let at = |line: u32| Finding {
        id: Some(fp.clone()),
        at: Some(format!("src/report.rs:{line}")),
        claim: "a claim can forge a tail".into(),
        witness: Some("poc.rs".into()),
        rec: Some(REC.into()),
    };
    let (before, after) = (at(120), at(161));
    assert_eq!(before.id, after.id, "a moved line keeps its id");
    assert_ne!(
        before.address(),
        after.address(),
        "the address holds the line"
    );
    for (speciality, class, file, symbol) in [
        (
            " dark-elf-agent ",
            "wire-framing",
            "./src/report.rs",
            "Finding::parse",
        ),
        (
            "dark-elf-agent",
            "wire-framing",
            "src\\report.rs",
            " Finding::parse",
        ),
        (
            "dark-elf-agent",
            "wire-framing",
            "././src/report.rs",
            "Finding::parse\n",
        ),
    ] {
        assert_eq!(fingerprint(speciality, class, file, symbol), fp);
    }
}

#[test]
fn the_fingerprint_changes_with_the_root_cause() {
    let base = fingerprint(
        "dark-elf-agent",
        "wire-framing",
        "src/report.rs",
        "Finding::parse",
    );
    for other in [
        fingerprint(
            "dark-elf-harness",
            "wire-framing",
            "src/report.rs",
            "Finding::parse",
        ),
        fingerprint(
            "dark-elf-agent",
            "prompt-injection",
            "src/report.rs",
            "Finding::parse",
        ),
        fingerprint(
            "dark-elf-agent",
            "wire-framing",
            "src/hole.rs",
            "Finding::parse",
        ),
        fingerprint(
            "dark-elf-agent",
            "wire-framing",
            "src/report.rs",
            "Finding::render",
        ),
    ] {
        assert_ne!(other, base);
        assert_eq!(other.len(), 10);
    }
}

#[test]
fn needs_validation_round_trips() {
    let nv = Hole::Nv {
        fingerprint: "0c7349f326".into(),
        blocker: "the sink is reached only behind the gate".into(),
        plan: "run poc.rs with the gate off: plan: see rec:r".into(),
    };
    let line = nv.to_string();
    assert_eq!(
        line,
        "nv #0c7349f326 blocker:the sink is reached only behind the gate plan:run poc.rs with the gate off: plan: see rec:r"
    );
    assert_eq!(Hole::parse(&line), nv);
    assert_eq!(relay(&done(vec![], vec![nv.clone()])).holes, [nv]);
}

#[test]
fn a_needs_validation_with_a_severity_is_a_hole_of_its_own() {
    for text in [
        "nv #0c7349f326 severity:high blocker:x plan:y",
        "nv #0c7349f326 blocker:x severity:high plan:y",
        "nv #0c7349f326 blocker:x",
        "nv #0c7349f326 blocker: plan:y",
        "nv #0c7349f326 blocker:x plan:",
        "nv #nothex blocker:x plan:y",
        "nv #0c7349f326ff blocker:x plan:y",
    ] {
        assert_eq!(Hole::parse(text), Hole::Other(text.into()), "{text}");
    }
}

#[test]
fn needs_validation_is_never_a_finding() {
    let proven = Finding {
        id: Some("0c7349f326".into()),
        at: Some("src/a.rs:4".into()),
        claim: "the input is trusted".into(),
        witness: Some("poc.rs".into()),
        rec: Some(REC.into()),
    };
    let with = Report::parse(&format!(
        "@S DONE\n@F {}\n@? nv #1234567890 blocker:x plan:y\n@E 1",
        done(vec![proven.clone()], vec![])
            .to_string()
            .lines()
            .nth(1)
            .unwrap()[3..]
            .to_owned()
    ))
    .expect("on the wire")
    .dark_elf();
    let without = relay(&done(vec![proven], vec![])).dark_elf();
    assert_eq!(with.findings, without.findings, "an nv moves no count");
    assert!(matches!(with.holes[..], [Hole::Nv { .. }]));
}

#[test]
fn a_dark_elf_finding_without_its_contract_is_a_hole() {
    let whole = Finding {
        id: Some("0c7349f326".into()),
        at: Some("src/a.rs:4".into()),
        claim: "c".into(),
        witness: Some("poc.rs".into()),
        rec: Some(REC.into()),
    };
    let broken = [
        Finding {
            id: None,
            ..whole.clone()
        },
        Finding {
            id: Some("f1".into()),
            ..whole.clone()
        },
        Finding {
            witness: None,
            ..whole.clone()
        },
        Finding {
            rec: None,
            ..whole.clone()
        },
    ];
    let mut all = vec![whole.clone()];
    all.extend(broken.iter().cloned());
    let back = relay(&done(all, vec![])).dark_elf();
    assert_eq!(back.findings, [whole]);
    assert_eq!(back.holes.len(), broken.len());
    assert!(
        back.holes
            .iter()
            .all(|h| matches!(h, Hole::Other(t) if t.starts_with("unproven: ")))
    );
}
