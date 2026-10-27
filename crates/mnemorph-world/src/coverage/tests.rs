//! The state table, one fixture per rule: each fixture breaks its rule and no other, and passes
//! once that rule alone is switched off — so no rule is dead weight.

use super::*;
use proptest::prelude::*;

const REPORT: &str = "crates/mnemorph-wire/src/report.rs";
const WIRE: &str = "crates/mnemorph-wire/**";
const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const REASON: &str = "the landing touches no parser the class could reach";

fn landing() -> Landing {
    Landing {
        id: "run-1:3".to_owned(),
        written: vec![REPORT.to_owned()],
        routed: vec![Routed {
            speciality: Speciality::Agent,
            surfaces: vec![Surface {
                at: WIRE.to_owned(),
                declared: false,
            }],
        }],
    }
}

fn na(class: &str) -> Entry {
    Entry {
        surface: WIRE.to_owned(),
        class: class.to_owned(),
        state: State::NotApplicable {
            reason: REASON.to_owned(),
        },
    }
}

fn checked(path: &str, sha256: &str) -> State {
    State::Checked {
        method: "forged an @F through a claim and read the parser back".to_owned(),
        evidence: Evidence {
            files: vec![FileRead {
                path: path.to_owned(),
                symbols: vec!["Finding::parse".to_owned()],
                sha256: sha256.to_owned(),
            }],
            commands: vec![CommandRun {
                run: "cargo test -p mnemorph-wire forged".to_owned(),
                sha256: HASH.to_ascii_uppercase(),
            }],
        },
    }
}

/// A ledger that holds: the agent's six classes on the wire's surface, one checked, one skipped.
fn valid() -> CoverageLedger {
    let mut entries: Vec<Entry> = Speciality::Agent
        .classes()
        .iter()
        .map(|c| na(c.name()))
        .collect();
    entries[2].state = checked(REPORT, HASH);
    entries[4].state = State::Skipped {
        reason: "no MCP server is configured in this world yet".to_owned(),
        risk: None,
    };
    CoverageLedger {
        landing: "run-1:3".to_owned(),
        speciality: Speciality::Agent,
        entries,
    }
}

fn tree(path: &str) -> Option<String> {
    (path == REPORT).then(|| HASH.to_owned())
}

/// The fixture that breaks `rule`, and only it.
fn fixture(rule: Rule) -> CoverageLedger {
    let mut l = valid();
    match rule {
        Rule::Oversize => {
            l.entries[0].state = State::NotApplicable {
                reason: format!("{REASON} {}", "x".repeat(MAX_TEXT)),
            };
        }
        Rule::UnreadableText => {
            l.entries[0].state = State::NotApplicable {
                reason: format!("{REASON}\u{202E} reversed"),
            };
        }
        Rule::Unrouted => {
            l.speciality = Speciality::Web;
            l.entries.clear();
        }
        Rule::MissingPair => {
            l.entries.remove(0);
        }
        Rule::DuplicatePair => {
            let first = l.entries[0].clone();
            l.entries.push(first);
        }
        Rule::ExtraPair => l.entries.push(na("toctou")),
        Rule::BoilerplateReason => {
            l.entries[0].state = State::NotApplicable {
                reason: "Not relevant to this change.".to_owned(),
            };
        }
        Rule::UnstatedRisk => {
            // wire-forgery is high-risk.
            l.entries[2].state = State::Skipped {
                reason: "the parser is being rewritten in the next landing".to_owned(),
                risk: Some("  n/a  ".to_owned()),
            };
        }
        Rule::HollowCheck => {
            l.entries[2].state = State::Checked {
                method: "read the whole parser and its tests".to_owned(),
                evidence: Evidence::default(),
            };
        }
        Rule::OutsideEvidence => {
            l.entries[2].state = checked("crates/mnemorph-tools/src/bash.rs", HASH);
        }
        Rule::UnhashedEvidence => l.entries[2].state = checked(REPORT, "abc"),
        Rule::StaleEvidence => l.entries[2].state = checked(REPORT, &"0".repeat(64)),
    }
    l
}

fn rules_broken(l: &CoverageLedger, rules: &[Rule]) -> Vec<Rule> {
    let mut broken: Vec<Rule> = check(l, &landing(), rules, Some(&tree))
        .into_iter()
        .map(|b| b.rule)
        .collect();
    broken.dedup();
    broken
}

#[test]
fn a_full_ledger_holds() {
    assert_eq!(validate(&valid(), &landing()), Ok(()));
    assert_eq!(validate_in(&valid(), &landing(), &tree), Ok(()));
}

#[test]
fn each_rule_refuses_its_fixture_alone() {
    for rule in Rule::ALL {
        assert_eq!(
            rules_broken(&fixture(rule), &Rule::ALL),
            [rule],
            "{rule}: {:?}",
            check(&fixture(rule), &landing(), &Rule::ALL, Some(&tree))
        );
    }
}

/// Switch one rule off and its fixture passes: the rule alone stood between it and a pass.
#[test]
fn each_rule_switched_off_lets_its_fixture_through() {
    for rule in Rule::ALL {
        let others: Vec<Rule> = Rule::ALL.into_iter().filter(|&r| r != rule).collect();
        assert_eq!(rules_broken(&fixture(rule), &others), [], "{rule}");
    }
}

#[test]
fn rule_names_are_distinct_kebab_words() {
    let mut names: Vec<&str> = Rule::ALL.iter().map(|r| r.name()).collect();
    assert!(
        names
            .iter()
            .all(|n| n.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'))
    );
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Rule::ALL.len());
}

#[test]
fn a_refusal_names_its_rules_and_where() {
    let Err(r) = validate(&fixture(Rule::MissingPair), &landing()) else {
        panic!("a missing pair is refused");
    };
    assert_eq!(r.rules(), [Rule::MissingPair]);
    let text = r.to_string();
    assert!(
        text.contains("missing-pair at crates/mnemorph-wire/** × prompt-injection"),
        "{text}"
    );
}

#[test]
fn boilerplate_is_refused_whatever_its_case_or_punctuation() {
    for empty in [
        "",
        "   ",
        "n/a",
        "N/A.",
        "none",
        "Not relevant",
        "NOT APPLICABLE!!",
        "not applicable to this change",
        "Out of scope for this change.",
        "--------------------------",
        "short but real",
    ] {
        assert!(!substantive(empty), "{empty:?}");
    }
    assert!(substantive(REASON));
    // Whitespace does not pad a reason past the floor.
    assert!(!substantive(&format!("too{}short", " ".repeat(40))));
}

#[test]
fn a_skip_of_a_low_risk_class_needs_no_risk_and_a_high_risk_one_does() {
    for class in AttackClass::ALL {
        let mut l = valid();
        let at = l
            .entries
            .iter()
            .position(|e| e.class == class.name())
            .unwrap_or(0);
        if Speciality::Agent.classes().contains(&class) {
            l.entries[at].state = State::Skipped {
                reason: "the parser is being rewritten in the next landing".to_owned(),
                risk: None,
            };
            let broken = rules_broken(&l, &Rule::ALL);
            assert_eq!(
                broken.contains(&Rule::UnstatedRisk),
                HIGH_RISK.contains(&class),
                "{class}"
            );
            l.entries[at].state = State::Skipped {
                reason: "the parser is being rewritten in the next landing".to_owned(),
                risk: Some("a forged @F would read as a finding until then".to_owned()),
            };
            assert_eq!(rules_broken(&l, &Rule::ALL), [], "{class}");
        }
    }
}

#[test]
fn the_generalist_owes_an_empty_ledger() {
    let mut land = landing();
    land.routed.push(Routed {
        speciality: Speciality::General,
        surfaces: Vec::new(),
    });
    let mut l = CoverageLedger {
        landing: land.id.clone(),
        speciality: Speciality::General,
        entries: Vec::new(),
    };
    assert_eq!(validate(&l, &land), Ok(()));
    l.entries.push(na("wire-forgery"));
    assert_eq!(
        validate(&l, &land).map_err(|r| r.rules()),
        Err(vec![Rule::ExtraPair])
    );
}

#[test]
fn evidence_is_held_to_the_landing_s_surfaces_as_ownership_reads_paths() {
    for (path, inside) in [
        ("./crates/mnemorph-wire/src/report.rs", true),
        ("crates//mnemorph-wire/src/report.rs", true),
        ("crates/mnemorph-wire\\src\\report.rs", true),
        ("crates/mnemorph-tools/../mnemorph-wire/src/report.rs", true),
        ("crates/mnemorph-wire/../../etc/passwd", false),
        ("/home/me/.ssh/id_ed25519", false),
        ("../crates/mnemorph-wire/src/report.rs", false),
        ("crates/mnemorph-wire/", false),
        ("", false),
    ] {
        let mut l = valid();
        l.entries[2].state = checked(path, HASH);
        let broken = check(&l, &landing(), &[Rule::OutsideEvidence], None);
        assert_eq!(broken.is_empty(), inside, "{path}");
    }
}

#[test]
fn a_ledger_over_the_entry_cap_is_refused_unread() {
    let mut l = valid();
    l.entries = vec![na("wire-forgery"); MAX_ENTRIES + 1];
    let breaches = check(&l, &landing(), &Rule::ALL, None);
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].rule, Rule::Oversize);
    // With the cap off, only the first MAX_ENTRIES are read.
    let others: Vec<Rule> = Rule::ALL
        .into_iter()
        .filter(|&r| r != Rule::Oversize)
        .collect();
    let r = refused(check(&l, &landing(), &others, None)).unwrap_err();
    assert!(r.breaches.len() <= MAX_BREACHES);
    assert!(r.to_string().contains("duplicate-pair"));
}

#[test]
fn a_long_refusal_lists_its_first_breaches_and_counts_the_rest() {
    let mut l = valid();
    for e in &mut l.entries {
        e.surface = format!("nowhere/{}", e.class);
    }
    for n in 0..40 {
        l.entries.push(na(&format!("made-up-{n}")));
    }
    let r = validate(&l, &landing()).unwrap_err();
    assert_eq!(r.breaches.len(), MAX_BREACHES);
    assert!(r.more > 0);
    assert!(r.to_string().contains(&format!("and {} more", r.more)));
}

#[test]
fn every_list_is_capped() {
    let mut l = valid();
    let State::Checked { evidence, .. } = &mut l.entries[2].state else {
        unreachable!()
    };
    evidence.files[0].symbols = vec!["s".to_owned(); MAX_LIST + 1];
    evidence.commands = vec![evidence.commands[0].clone(); MAX_LIST + 1];
    let broken = check(&l, &landing(), &[Rule::Oversize], None);
    assert_eq!(broken.len(), 2, "{broken:?}");
}

#[test]
fn the_ledger_reads_from_json_and_refuses_unknown_fields() {
    let json = format!(
        r#"{{"landing":"run-1:3","speciality":"dark-elf-agent","entries":[
            {{"surface":"{WIRE}","class":"wire-forgery","state":{{"checked":{{"method":"m","evidence":{{"files":[{{"path":"{REPORT}","sha256":"{HASH}"}}]}}}}}}}},
            {{"surface":"{WIRE}","class":"mcp-identity","state":{{"skipped":{{"reason":"r"}}}}}},
            {{"surface":"{WIRE}","class":"toctou","state":{{"not_applicable":{{"reason":"r"}}}}}}
        ]}}"#
    );
    let l: CoverageLedger = serde_json::from_str(&json).unwrap();
    assert_eq!(l.entries.len(), 3);
    let back: CoverageLedger = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
    assert_eq!(back, l);
    for bad in [
        json.replace("\"dark-elf-agent\"", "\"dark-elf-agent-2\""),
        json.replace("\"method\"", "\"verdict\":1,\"method\""),
        json.replace("\"checked\"", "\"covered\""),
    ] {
        assert!(
            serde_json::from_str::<CoverageLedger>(&bad).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn shown_text_is_one_safe_line() {
    let s = shown(&format!("a\nb\u{1b}[2J\u{200B}{}", "x".repeat(200)));
    assert!(!s.chars().any(unreadable));
    assert!(s.ends_with('…'));
    assert_eq!(
        said(""),
        format!("(empty) says nothing: state the fact, in {MIN_PROSE} characters or more")
    );
}

fn hostile() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just(WIRE.to_owned()),
        Just("wire-forgery".to_owned()),
        Just(REPORT.to_owned()),
        Just(HASH.to_owned()),
        "\\PC{0,40}",
        "[a-z/*.\\\\ -]{0,30}",
        ".{0,60}",
        "[\u{0}-\u{1f}\u{202a}-\u{202e}\u{200b}a]{0,10}",
    ]
}

fn state() -> impl Strategy<Value = State> {
    prop_oneof![
        hostile().prop_map(|reason| State::NotApplicable { reason }),
        (hostile(), proptest::option::of(hostile()))
            .prop_map(|(reason, risk)| State::Skipped { reason, risk }),
        (
            hostile(),
            proptest::collection::vec(
                (
                    hostile(),
                    proptest::collection::vec(hostile(), 0..3),
                    hostile()
                ),
                0..3
            ),
            proptest::collection::vec((hostile(), hostile()), 0..3)
        )
            .prop_map(|(method, files, commands)| State::Checked {
                method,
                evidence: Evidence {
                    files: files
                        .into_iter()
                        .map(|(path, symbols, sha256)| FileRead {
                            path,
                            symbols,
                            sha256
                        })
                        .collect(),
                    commands: commands
                        .into_iter()
                        .map(|(run, sha256)| CommandRun { run, sha256 })
                        .collect(),
                },
            }),
    ]
}

fn entry() -> impl Strategy<Value = Entry> {
    (hostile(), hostile(), state()).prop_map(|(surface, class, state)| Entry {
        surface,
        class,
        state,
    })
}

proptest! {
    /// Whatever a body sends, the validator answers without panicking, every breach reads as one
    /// clean line, and the same ledger always gets the same answer.
    #[test]
    fn hostile_ledgers_get_clean_deterministic_answers(
        entries in proptest::collection::vec(entry(), 0..12),
        id in hostile(),
    ) {
        let l = CoverageLedger { landing: id, speciality: Speciality::Agent, entries };
        let first = check(&l, &landing(), &Rule::ALL, Some(&tree));
        prop_assert_eq!(&first, &check(&l, &landing(), &Rule::ALL, Some(&tree)));
        for b in &first {
            let line = b.to_string();
            prop_assert!(!line.chars().any(unreadable), "{:?}", line);
        }
    }

    /// A ledger holds only when every required pair is there exactly once and nothing else is.
    #[test]
    fn only_every_pair_exactly_once_holds(
        counts in proptest::collection::vec(0usize..3, 6),
        extra in proptest::option::of("[a-z-]{1,12}"),
    ) {
        let classes = Speciality::Agent.classes();
        let mut entries: Vec<Entry> = classes
            .iter()
            .zip(&counts)
            .flat_map(|(c, &n)| std::iter::repeat_n(na(c.name()), n))
            .collect();
        if let Some(e) = &extra {
            entries.push(na(e));
        }
        let l = CoverageLedger { landing: "run-1:3".to_owned(), speciality: Speciality::Agent, entries };
        // An extra entry is either a duplicate of a class's pair or a pair not required.
        let exact = counts.iter().all(|&n| n == 1) && extra.is_none();
        prop_assert_eq!(validate(&l, &landing()).is_ok(), exact);
    }

    /// Nothing under the floor says enough.
    #[test]
    fn short_prose_never_passes(text in "\\PC{0,19}") {
        prop_assert!(!substantive(&text));
    }
}
