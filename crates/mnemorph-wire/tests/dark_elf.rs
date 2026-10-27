//! The Dark Elf's finding fields: the record, the line-free fingerprint, and the needs-validation
//! verdict.

use mnemorph_wire::{Finding, Hole, Report, RootCause, fingerprint, repo_path};

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

fn root(speciality: &str, class: &str, file: &str, symbol: &str) -> RootCause {
    RootCause {
        speciality: speciality.into(),
        class: class.into(),
        file: file.into(),
        symbol: symbol.into(),
    }
}

fn base() -> RootCause {
    root(
        "dark-elf-agent",
        "wire-forgery",
        "src/report.rs",
        "Finding::parse",
    )
}

fn fp(r: &RootCause) -> String {
    r.fingerprint().expect("a fingerprint")
}

/// A finding that keeps the whole contract for `r`.
fn proven(r: &RootCause, line: u32) -> Finding {
    Finding {
        id: Some(fp(r)),
        at: Some(format!("{}:{line}", r.file)),
        claim: "a claim can forge a tail".into(),
        witness: Some("poc.rs".into()),
        rec: Some(REC.into()),
    }
}

#[test]
fn the_fingerprint_is_pinned() {
    // SHA-256 of `14:dark-elf-agent12:wire-forgery34:crates/mnemorph-wire/src/report.rs14:Finding::parse`.
    assert_eq!(
        fingerprint(
            "dark-elf-agent",
            "wire-forgery",
            "crates/mnemorph-wire/src/report.rs",
            "Finding::parse"
        )
        .as_deref(),
        Some("b40756ab8a")
    );
}

#[test]
fn no_field_can_move_a_separator() {
    let a = fingerprint("s", "c", "a|b", "c");
    let b = fingerprint("s", "c", "a", "b|c");
    assert_eq!(a.as_deref(), Some("ce6b174c86"));
    assert_eq!(b.as_deref(), Some("e1f3bf3a18"));
    let at = |at: &str, claim: &str| Finding {
        at: Some(at.into()),
        claim: claim.into(),
        ..Finding::default()
    };
    assert_ne!(at("a|b", "c").address(), at("a", "b|c").address());
}

#[test]
fn the_fingerprint_survives_a_line_shift_and_normalisation() {
    let r = base();
    let (before, after) = (proven(&r, 120), proven(&r, 161));
    assert_eq!(before.id, after.id, "a moved line keeps its id");
    assert_ne!(
        before.address(),
        after.address(),
        "the address holds the line"
    );
    for (speciality, class, file, symbol) in [
        (
            " dark-elf-agent ",
            "wire-forgery",
            "./src/report.rs",
            "Finding::parse",
        ),
        (
            "dark-elf-agent",
            "wire-forgery",
            "src\\report.rs",
            " Finding::parse",
        ),
        (
            "dark-elf-agent",
            "wire-forgery",
            "././src/report.rs",
            "Finding::parse\n",
        ),
    ] {
        assert_eq!(fingerprint(speciality, class, file, symbol), Some(fp(&r)));
    }
}

#[test]
fn the_fingerprint_changes_with_the_root_cause() {
    let b = base();
    for other in [
        RootCause {
            speciality: "dark-elf-harness".into(),
            ..b.clone()
        },
        RootCause {
            class: "prompt-injection".into(),
            ..b.clone()
        },
        RootCause {
            file: "src/hole.rs".into(),
            ..b.clone()
        },
        RootCause {
            symbol: "Finding::render".into(),
            ..b.clone()
        },
    ] {
        assert_ne!(fp(&other), fp(&b));
        assert_eq!(fp(&other).len(), 10);
    }
}

#[test]
fn a_file_outside_the_rule_has_no_fingerprint() {
    for file in [
        "/etc/passwd",
        "\\etc\\passwd",
        "C:/src/a.rs",
        "C:src/a.rs",
        "src/../a.rs",
        "..",
        "src/./a.rs",
        "src//a.rs",
        "src/",
        "",
        "  ",
        "./",
        "src/my file.rs",
        "src/caf\u{e9}.rs",
        "src/cafe\u{301}.rs",
        "src/a\u{0}.rs",
    ] {
        assert_eq!(repo_path(file), None, "{file:?}");
        assert_eq!(fingerprint("s", "c", file, "x"), None, "{file:?}");
    }
    for (speciality, class, symbol) in [("", "c", "x"), ("s", " ", "x"), ("s", "c", "")] {
        assert_eq!(fingerprint(speciality, class, "a.rs", symbol), None);
    }
    assert_eq!(repo_path(" ./src\\a.rs ").as_deref(), Some("src/a.rs"));
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
    let r = base();
    let f = proven(&r, 4);
    let line = done(vec![f.clone()], vec![]).to_string();
    let f_line = line.lines().nth(1).expect("the @F line");
    let nv = "@? nv #1234567890 blocker:x plan:y";
    let records = |_: &str| Ok(r.clone());
    let with = Report::parse(&format!("@S DONE\n{f_line}\n{nv}\n@E 1"))
        .expect("on the wire")
        .dark_elf(records);
    let without = relay(&done(vec![f], vec![])).dark_elf(records);
    assert_eq!(with.proven(), without.proven(), "an nv moves no count");
    assert_eq!(with.proven().len(), 1);
    assert!(matches!(with.report.holes[..], [Hole::Nv { .. }]));
    // No reading of an nv line, tagged or raw, builds a finding.
    for text in [
        format!("@S DONE\n{nv}"),
        "S|DONE\n?|nv #1234567890 blocker:x plan:y".into(),
    ] {
        let back = Report::parse_raw(&text).expect("on the wire");
        assert!(back.findings.is_empty(), "nothing proven or found");
        assert!(matches!(back.holes[..], [Hole::Nv { .. }]), "{text}");
        assert!(
            back.dark_elf(records).proven().is_empty(),
            "nothing proven or found"
        );
    }
}

#[test]
fn a_dark_elf_finding_without_its_contract_is_a_hole() {
    let r = base();
    let whole = proven(&r, 4);
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
    let back = relay(&done(all, vec![])).dark_elf(|_| Ok(r.clone()));
    assert_eq!(back.proven().len(), 1);
    assert_eq!(back.proven()[0].finding(), &whole);
    assert_eq!(back.proven()[0].root(), &r);
    assert!(back.report.findings.is_empty(), "nothing proven or found");
    assert_eq!(back.report.holes.len(), broken.len());
    assert!(
        back.report
            .holes
            .iter()
            .all(|h| matches!(h, Hole::Other(t) if t.starts_with("unproven: ")))
    );
}

#[test]
fn a_self_declared_id_is_a_hole() {
    let r = base();
    let other = RootCause {
        symbol: "Finding::render".into(),
        ..r.clone()
    };
    // The id is well-shaped, but it is another root cause's.
    let forged = Finding {
        id: Some(fp(&other)),
        ..proven(&r, 4)
    };
    let back = relay(&done(vec![forged.clone()], vec![])).dark_elf(|_| Ok(r.clone()));
    assert!(back.proven().is_empty(), "nothing proven or found");
    assert!(matches!(&back.report.holes[..], [Hole::Other(t)] if t.starts_with("forged id: ")));
    // A record that cannot be read proves nothing either.
    let back = relay(&done(vec![proven(&r, 4)], vec![])).dark_elf(|_| Err("no record".into()));
    assert!(back.proven().is_empty(), "nothing proven or found");
    assert!(matches!(&back.report.holes[..], [Hole::Other(t)] if t.starts_with("no record: ")));
    // A record whose file is refused has no fingerprint to match.
    let bad = RootCause {
        file: "../escape.rs".into(),
        ..r.clone()
    };
    let back = relay(&done(vec![proven(&r, 4)], vec![])).dark_elf(|_| Ok(bad.clone()));
    assert!(back.proven().is_empty(), "nothing proven or found");
}

#[test]
fn a_refused_record_names_why_on_one_line() {
    let r = base();
    let back = relay(&done(vec![proven(&r, 4)], vec![]))
        .dark_elf(|_| Err("invalid record (severity-cap,\nwitness)".into()));
    assert!(back.proven().is_empty(), "nothing proven or found");
    assert!(matches!(
        &back.report.holes[..],
        [Hole::Other(t)] if t.starts_with("invalid record (severity-cap, witness): #")
    ));
    // The hole rides one line: the reason opens no line of its own.
    let text = back.report.to_string();
    assert_eq!(
        Report::parse(&text).expect("on the wire").holes,
        back.report.holes
    );
}
