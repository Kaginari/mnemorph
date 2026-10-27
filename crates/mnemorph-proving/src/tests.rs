//! The contract's fixtures: one passing record, one breaking record per rule, each rule shown to
//! be the one its fixture needs; hostile input; demotion from the wire to the count.

use std::collections::BTreeMap;

use mnemorph_wire::{Finding, Hole, Report};
use proptest::prelude::*;

use crate::finding::{check, sha256_hex as digest};
use crate::{
    CLAIM_BYTES, FindingRecord, Impact, Likelihood, Rule, Severity, Step, StepKind, Tree, Verdict,
    Witness, WitnessKind, World, accept, record_path, validate,
};

const SOURCE: &str = "crates/demo/src/read.rs";
const ENTRY: &str = "crates/demo/src/main.rs";
const POC: &str = ".mnemorph/dark-elf/dark-elf-harness/poc/read_path.rs";
const POC_BODY: &str = "#[test]\nfn escapes() { assert!(read_path(\"../x\").is_err()); }\n";

fn world() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([
        (
            SOURCE.to_owned(),
            b"pub fn read_path(p: &str) -> String { std::fs::read_to_string(p).unwrap() }\n"
                .to_vec(),
        ),
        (ENTRY.to_owned(), b"fn main() {}\n".to_vec()),
        (POC.to_owned(), POC_BODY.as_bytes().to_vec()),
    ])
}

/// The fingerprint and the record's path, derived again from the root cause.
fn seal(mut r: FindingRecord) -> FindingRecord {
    r.fingerprint = r.root().fingerprint().unwrap_or_default();
    r.rec = record_path(&r.speciality, &r.fingerprint);
    r
}

/// A record that keeps every rule.
fn good() -> FindingRecord {
    seal(FindingRecord {
        verdict: Verdict::Finding,
        fingerprint: String::new(),
        speciality: "dark-elf-harness".into(),
        class: "path-traversal".into(),
        file: SOURCE.into(),
        symbol: "read_path".into(),
        claim: "read_path follows `..` out of the world".into(),
        trace: vec![
            Step {
                kind: StepKind::Entry,
                at: format!("{ENTRY}:1"),
                note: "a path from a repository file".into(),
            },
            Step {
                kind: StepKind::Sink,
                at: format!("{SOURCE}:1"),
                note: "read without a check".into(),
            },
        ],
        witness: Some(Witness {
            kind: WitnessKind::Test,
            reference: POC.into(),
            sha256: digest(POC_BODY.as_bytes()),
        }),
        preconditions: vec!["the body reads an untrusted repository".into()],
        reachability: "a repository file names the path the tool reads".into(),
        likelihood: Some(Likelihood::Medium),
        likelihood_reason: "worlds are cloned from strangers".into(),
        impact: Some(Impact::Disclosure),
        impact_reason: "the test reads a file outside the world".into(),
        severity: Some(Severity::High),
        fix: "refuse `..` before the read".into(),
        invariant: "no read leaves the world".into(),
        rec: String::new(),
    })
}

/// One record per rule, breaking that rule and no other.
fn fixtures() -> Vec<(Rule, FindingRecord)> {
    let g = good;
    vec![
        (
            Rule::Nv,
            FindingRecord {
                verdict: Verdict::Nv,
                ..g()
            },
        ),
        (
            Rule::Required,
            FindingRecord {
                fix: " ".into(),
                ..g()
            },
        ),
        (
            Rule::Size,
            FindingRecord {
                claim: "a".repeat(CLAIM_BYTES + 1),
                ..g()
            },
        ),
        (
            Rule::Escaped,
            FindingRecord {
                claim: "read_path reads \u{202e}txt.exe".into(),
                ..g()
            },
        ),
        (
            Rule::ClassOwned,
            seal(FindingRecord {
                class: "prompt-injection".into(),
                ..g()
            }),
        ),
        (
            Rule::Fingerprint,
            FindingRecord {
                fingerprint: "0123456789".into(),
                rec: record_path("dark-elf-harness", "0123456789"),
                ..g()
            },
        ),
        (
            Rule::RecPath,
            FindingRecord {
                rec: record_path("dark-elf-agent", &g().fingerprint),
                ..g()
            },
        ),
        (Rule::FileExists, {
            let mut r = g();
            r.file = "crates/demo/src/gone.rs".into();
            r.trace[1].at = "crates/demo/src/gone.rs:1".into();
            seal(r)
        }),
        (
            Rule::SymbolPresent,
            seal(FindingRecord {
                symbol: "write_path".into(),
                ..g()
            }),
        ),
        (Rule::Trace, {
            let mut r = g();
            r.trace.reverse();
            r
        }),
        (
            Rule::Witness,
            FindingRecord {
                witness: None,
                ..g()
            },
        ),
        (Rule::WitnessHash, {
            let mut r = g();
            if let Some(w) = r.witness.as_mut() {
                w.sha256 = "0".repeat(64);
            }
            r
        }),
        (
            Rule::SeverityCap,
            FindingRecord {
                severity: Some(Severity::Critical),
                ..g()
            },
        ),
        (
            Rule::Preconditions,
            FindingRecord {
                preconditions: vec![],
                ..g()
            },
        ),
    ]
}

#[test]
fn the_good_record_passes() {
    let v = validate(&good(), &world()).expect("valid");
    assert_eq!(v.fingerprint(), good().fingerprint);
    assert_eq!(v.root(), &good().root());
    assert_eq!(v.record(), &good());
    assert_eq!(v.severity(), Severity::High);
}

#[test]
fn there_is_one_fixture_per_rule() {
    let rules: Vec<Rule> = fixtures().into_iter().map(|(r, _)| r).collect();
    assert_eq!(rules, Rule::ALL);
}

#[test]
fn each_fixture_breaks_its_rule_alone() {
    for (rule, bad) in fixtures() {
        assert_eq!(validate(&bad, &world()), Err(vec![rule]), "{rule}");
    }
}

#[test]
fn each_rule_is_the_one_its_fixture_needs() {
    // Switched off, a rule lets its fixture through: its fixture's test would fail.
    for (rule, bad) in fixtures() {
        assert!(check(&bad, &world(), Some(rule)).is_ok(), "{rule}");
        assert!(check(&good(), &world(), Some(rule)).is_ok(), "{rule}");
    }
}

#[test]
fn rule_names_are_unique_kebab_words() {
    let names: Vec<String> = Rule::ALL.iter().map(ToString::to_string).collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len());
    assert!(
        names
            .iter()
            .all(|n| n.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'))
    );
}

#[test]
fn severity_is_capped_by_the_demonstrated_impact() {
    let table = [
        (Impact::None, Severity::Informational),
        (Impact::Crash, Severity::Medium),
        (Impact::Exhaustion, Severity::Medium),
        (Impact::Disclosure, Severity::High),
        (Impact::Tamper, Severity::High),
        (Impact::Bypass, Severity::Critical),
        (Impact::Execution, Severity::Critical),
    ];
    let all = [
        Severity::Informational,
        Severity::Low,
        Severity::Medium,
        Severity::High,
        Severity::Critical,
    ];
    for (impact, cap) in table {
        assert_eq!(impact.cap(), cap);
        for severity in all {
            let r = FindingRecord {
                impact: Some(impact),
                severity: Some(severity),
                ..good()
            };
            let over = validate(&r, &world()).is_err_and(|e| e.contains(&Rule::SeverityCap));
            assert_eq!(over, severity > cap, "{impact:?} {}", severity.word());
        }
    }
}

#[test]
fn no_reachability_caps_at_low() {
    let unreached = |severity| FindingRecord {
        reachability: " ".into(),
        impact: Some(Impact::Execution),
        severity: Some(severity),
        ..good()
    };
    assert!(validate(&unreached(Severity::Low), &world()).is_ok());
    assert_eq!(
        validate(&unreached(Severity::Medium), &world()),
        Err(vec![Rule::SeverityCap, Rule::Preconditions])
    );
}

#[test]
fn below_medium_needs_no_preconditions() {
    let r = FindingRecord {
        preconditions: vec![],
        severity: Some(Severity::Low),
        ..good()
    };
    assert!(validate(&r, &world()).is_ok());
}

#[test]
fn a_record_missing_its_parts_breaks_required() {
    for r in [
        FindingRecord {
            trace: vec![],
            ..good()
        },
        FindingRecord {
            severity: None,
            ..good()
        },
        FindingRecord {
            impact: None,
            ..good()
        },
        FindingRecord {
            likelihood: None,
            ..good()
        },
    ] {
        assert_eq!(validate(&r, &world()), Err(vec![Rule::Required]), "{r:?}");
    }
}

#[test]
fn the_generalist_and_unknown_specialities_own_no_class() {
    for speciality in ["dark-elf", "dark-elf-harness-2", "orc-auth"] {
        let r = FindingRecord {
            speciality: speciality.into(),
            ..good()
        };
        assert!(
            validate(&r, &world()).is_err_and(|e| e.contains(&Rule::ClassOwned)),
            "{speciality}"
        );
    }
}

#[test]
fn a_symbol_is_matched_as_a_whole_word() {
    for (symbol, named) in [
        ("read_path", true),
        ("demo::read_path", true),
        ("Reader.read_path", true),
        ("read", false),
        ("path", false),
        ("::", false),
    ] {
        let r = seal(FindingRecord {
            symbol: symbol.into(),
            ..good()
        });
        let broke = validate(&r, &world()).is_err_and(|e| e.contains(&Rule::SymbolPresent));
        assert_eq!(broke, !named, "{symbol}");
    }
}

#[test]
fn a_trace_runs_entry_to_sink_on_safe_paths() {
    let step = |kind, at: &str| Step {
        kind,
        at: at.into(),
        note: String::new(),
    };
    let sink = format!("{SOURCE}:1");
    let entry = format!("{ENTRY}:1");
    let ok = [
        vec![step(StepKind::Sink, &sink)],
        vec![step(StepKind::Entry, &sink)],
        vec![
            step(StepKind::Entry, &entry),
            step(StepKind::Propagation, &entry),
            step(StepKind::Sink, &sink),
        ],
    ];
    let bad = [
        vec![step(StepKind::Propagation, &sink)],
        vec![step(StepKind::Entry, &entry), step(StepKind::Entry, &sink)],
        vec![
            step(StepKind::Entry, &entry),
            step(StepKind::Sink, &entry),
            step(StepKind::Sink, &sink),
        ],
        vec![step(StepKind::Entry, &entry), step(StepKind::Sink, &entry)],
        vec![
            step(StepKind::Entry, "../x.rs:1"),
            step(StepKind::Sink, &sink),
        ],
        vec![
            step(StepKind::Entry, &format!("{ENTRY}:0")),
            step(StepKind::Sink, &sink),
        ],
        vec![step(StepKind::Entry, ENTRY), step(StepKind::Sink, &sink)],
    ];
    for trace in ok {
        let r = FindingRecord { trace, ..good() };
        assert!(validate(&r, &world()).is_ok(), "{:?}", r.trace);
    }
    for trace in bad {
        let r = FindingRecord { trace, ..good() };
        assert_eq!(
            validate(&r, &world()),
            Err(vec![Rule::Trace]),
            "{:?}",
            r.trace
        );
    }
}

#[test]
fn a_witness_lies_in_its_own_proving_ground_or_is_a_hashed_command() {
    let command = "cargo test -p demo escapes";
    let poc = FindingRecord {
        witness: Some(Witness {
            kind: WitnessKind::PocCommand,
            reference: command.into(),
            sha256: digest(command.as_bytes()),
        }),
        ..good()
    };
    assert!(validate(&poc, &world()).is_ok());
    let mut elsewhere = world();
    let other = ".mnemorph/dark-elf/dark-elf-agent/poc/read_path.rs";
    elsewhere.insert(other.into(), POC_BODY.into());
    for (kind, reference, sha256) in [
        (WitnessKind::Test, other, digest(POC_BODY.as_bytes())),
        (
            WitnessKind::ScannerOutput,
            "crates/demo/src/read.rs",
            digest(b""),
        ),
        (WitnessKind::PocCommand, " ", digest(b" ")),
        (WitnessKind::Test, POC, "ABC".into()),
    ] {
        let r = FindingRecord {
            witness: Some(Witness {
                kind,
                reference: reference.into(),
                sha256,
            }),
            ..good()
        };
        assert!(
            validate(&r, &elsewhere).is_err_and(|e| e.contains(&Rule::Witness)),
            "{reference}"
        );
    }
    // A witness file that is not in the tree cannot be hashed.
    let mut gone = world();
    gone.remove(POC);
    assert_eq!(validate(&good(), &gone), Err(vec![Rule::WitnessHash]));
}

#[test]
fn a_record_reads_back_from_its_json() {
    let r = good();
    let back = FindingRecord::parse(&r.rec, r.to_json().as_bytes()).expect("a record");
    assert_eq!(back, r);
    // Unknown fields, a missing verdict, an oversized file: no record.
    let mut json: serde_json::Value = serde_json::from_str(&r.to_json()).expect("json");
    json["extra"] = serde_json::Value::Bool(true);
    assert!(FindingRecord::parse(&r.rec, json.to_string().as_bytes()).is_none());
    assert!(FindingRecord::parse(&r.rec, br#"{"claim":"x"}"#).is_none());
    let huge = format!(
        "{{\"verdict\":\"finding\",\"claim\":\"{}\"}}",
        "a".repeat(64 * 1024)
    );
    assert!(FindingRecord::parse(&r.rec, huge.as_bytes()).is_none());
    // An nv record parses — and never validates.
    let nv = br#"{"verdict":"nv","claim":"maybe"}"#;
    let nv = FindingRecord::parse("x.json", nv).expect("a record");
    assert!(validate(&nv, &world()).is_err_and(|e| e[0] == Rule::Nv));
}

// --- the tree on disk ---

#[test]
fn a_tree_reads_only_inside_its_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("world");
    std::fs::create_dir_all(root.join("src")).expect("mkdir");
    std::fs::write(root.join("src/a.rs"), "fn a() {}").expect("write");
    std::fs::write(dir.path().join("secret"), "s").expect("write");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join("secret"), root.join("src/link")).expect("ln");
        std::os::unix::fs::symlink(dir.path(), root.join("up")).expect("ln");
    }
    let tree = Tree::new(&root);
    assert_eq!(tree.read("src/a.rs").as_deref(), Some(&b"fn a() {}"[..]));
    assert_eq!(tree.read("./src\\a.rs").as_deref(), Some(&b"fn a() {}"[..]));
    for refused in [
        "../secret",
        "/etc/passwd",
        "src",
        "src/none.rs",
        "src/link",
        "up/secret",
    ] {
        assert!(tree.read(refused).is_none(), "{refused}");
    }
}

// --- demotion, from the wire to the count ---

fn finding_of(r: &FindingRecord) -> Finding {
    Finding {
        id: Some(r.fingerprint.clone()),
        at: Some(format!("{SOURCE}:1")),
        claim: r.claim.clone(),
        witness: Some(POC.into()),
        rec: Some(r.rec.clone()),
    }
}

fn with_records(records: &[&FindingRecord]) -> BTreeMap<String, Vec<u8>> {
    let mut w = world();
    for r in records {
        w.insert(r.rec.clone(), r.to_json().into_bytes());
    }
    w
}

#[test]
fn an_invalid_finding_is_demoted_with_its_rules_and_only_valid_ones_count() {
    let ok = good();
    let over = seal(FindingRecord {
        symbol: "std::fs::read_to_string".into(),
        severity: Some(Severity::Critical),
        ..good()
    });
    let nv = seal(FindingRecord {
        verdict: Verdict::Nv,
        class: "toctou".into(),
        ..good()
    });
    let w = with_records(&[&ok, &over, &nv]);
    let report = Report {
        status: "DONE".into(),
        findings: vec![finding_of(&ok), finding_of(&over), finding_of(&nv)],
        holes: vec![Hole::Nv {
            fingerprint: nv.fingerprint.clone(),
            blocker: "the race needs two writers".into(),
            plan: "run the PoC under a second writer".into(),
        }],
        ..Report::default()
    };
    // Through the wire and back, as the harness reads it.
    let read = Report::parse(&report.to_string()).expect("on the wire");
    let accepted = accept(read, &w);
    assert_eq!(accepted.valid().len(), 1);
    assert_eq!(accepted.valid()[0].record(), &ok);
    assert_eq!(accepted.report.proven().len(), 1);
    assert!(accepted.report.report.findings.is_empty(), "nothing found");
    let holes: Vec<String> = accepted
        .report
        .report
        .holes
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(matches!(accepted.report.report.holes[0], Hole::Nv { .. }));
    assert!(holes[1].starts_with(&format!(
        "invalid record (severity-cap): #{}",
        over.fingerprint
    )));
    assert!(holes[2].starts_with(&format!("invalid record (nv): #{}", nv.fingerprint)));
    // The demoted lines ride the wire as holes, not findings.
    let again = Report::parse(&accepted.report.report.to_string()).expect("on the wire");
    assert!(again.findings.is_empty(), "nothing found");
    assert_eq!(again.holes.len(), 3);
}

#[test]
fn a_missing_record_and_a_forged_id_are_demoted_too() {
    let ok = good();
    let w = with_records(&[&ok]);
    let missing = Finding {
        rec: Some(".mnemorph/dark-elf/dark-elf-harness/findings/none.json".into()),
        ..finding_of(&ok)
    };
    let forged = Finding {
        id: Some("0123456789".into()),
        ..finding_of(&ok)
    };
    let report = Report {
        status: "DONE".into(),
        findings: vec![missing, forged],
        ..Report::default()
    };
    let accepted = accept(report, &w);
    assert!(accepted.valid().is_empty(), "nothing counts");
    let holes: Vec<String> = accepted
        .report
        .report
        .holes
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(holes[0].starts_with("no record: "), "{holes:?}");
    assert!(holes[1].starts_with("forged id: "), "{holes:?}");
}

// --- hostile input ---

fn hostile() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        "[\\PC\u{1b}\u{202e}\u{200b}\n\r\t:/.\\\\@#-]{0,80}",
        Just("../../etc/passwd".to_owned()),
        Just("a".repeat(4096)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn any_bytes_parse_or_refuse_without_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = FindingRecord::parse("x.json", &bytes);
    }

    #[test]
    fn hostile_fields_never_pass_unsafe(
        claim in hostile(), symbol in hostile(), file in hostile(), reference in hostile(),
        reachability in hostile(), precondition in hostile(), at in hostile(),
    ) {
        let mut r = good();
        r.claim = claim;
        r.symbol = symbol;
        r.file = file;
        r.reachability = reachability;
        r.preconditions = vec![precondition];
        r.trace[0].at = at;
        if let Some(w) = r.witness.as_mut() { w.reference = reference; }
        let r = seal(r);
        if let Ok(v) = validate(&r, &world()) {
            let rec = v.record();
            let texts = [&rec.claim, &rec.symbol, &rec.file, &rec.reachability, &rec.preconditions[0]];
            prop_assert!(texts.iter().all(|t| !t.chars().any(char::is_control)));
            prop_assert!(rec.claim.len() <= CLAIM_BYTES);
            prop_assert!(world().read(&rec.file).is_some());
            prop_assert!(rec.severity.zip(rec.impact).is_some_and(|(s, i)| s <= i.cap()));
            let derived = rec.root().fingerprint();
            prop_assert_eq!(derived.as_deref(), Some(rec.fingerprint.as_str()));
        }
    }

    #[test]
    fn a_hostile_report_counts_nothing_without_a_valid_record(text in hostile(), id in "[0-9a-f]{10}") {
        let line = format!("@S DONE\n@F #{id} {SOURCE}:1 {} witness:{POC} rec:{}\n@E 1",
            text.replace(['\n', '\r'], " "), record_path("dark-elf-harness", &id));
        if let Some(report) = Report::parse(&line) {
            let accepted = accept(report, &world());
            prop_assert!(accepted.valid().is_empty());
            prop_assert!(accepted.report.report.findings.is_empty());
        }
    }
}
