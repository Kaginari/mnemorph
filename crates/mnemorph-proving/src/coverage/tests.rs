//! The state table, one fixture per rule: each fixture breaks its rule and no other, and passes
//! once that rule alone is switched off — so no rule is dead weight.

use std::collections::BTreeMap;

use super::*;
use proptest::prelude::*;

const REPORT: &str = "crates/mnemorph-wire/src/report.rs";
const WIRE: &str = "crates/mnemorph-wire/**";
const LOCK: &str = "Cargo.lock";
const LOCKS: &str = "**/Cargo.lock";
/// The SHA-256 of `abc`: every file of the fixtures' tree, and every recorded output.
const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const TEST: &str = "cargo test -p mnemorph-wire forged";
const AUDIT: &str = "cargo audit";

fn tree() -> BTreeMap<String, Vec<u8>> {
    [(REPORT, b"abc"), (LOCK, b"abc")]
        .into_iter()
        .map(|(p, b)| (p.to_owned(), b.to_vec()))
        .collect()
}

fn ran() -> Ran {
    let mut r = Ran::default();
    r.record(TEST, "abc");
    r.record(AUDIT, "abc");
    r
}

fn landing_of(spec: Speciality, surface: &str, written: &str) -> Landing {
    Landing {
        id: "run-1:3".to_owned(),
        written: vec![written.to_owned()],
        routed: vec![Routed {
            speciality: spec,
            surfaces: vec![Surface {
                at: surface.to_owned(),
                declared: false,
            }],
        }],
    }
}

fn landing() -> Landing {
    landing_of(Speciality::Agent, WIRE, REPORT)
}

fn file(path: &str, sha256: &str) -> FileRead {
    FileRead {
        path: path.to_owned(),
        symbols: vec!["Finding::parse".to_owned()],
        sha256: sha256.to_owned(),
    }
}

fn checked(path: &str, sha256: &str, run: &str) -> State {
    State::Checked {
        method: "forged an @F through a claim and read the parser back".to_owned(),
        evidence: Evidence {
            files: vec![file(path, sha256)],
            commands: vec![CommandRun {
                run: run.to_owned(),
                sha256: Some(HASH.to_ascii_uppercase()),
            }],
        },
    }
}

fn na(surface: &str, class: &str, n: usize) -> Entry {
    Entry {
        surface: surface.to_owned(),
        class: class.to_owned(),
        state: State::NotApplicable {
            reason: format!("fact {n}: the landing adds no reader this class could reach"),
            evidence: Evidence::default(),
        },
    }
}

/// A ledger of `spec` on `surface`: every class not applicable, each for its own reason.
fn ruled(spec: Speciality, surface: &str) -> CoverageLedger {
    CoverageLedger {
        landing: "run-1:3".to_owned(),
        speciality: spec,
        entries: spec
            .classes()
            .iter()
            .enumerate()
            .map(|(n, c)| na(surface, c.name(), n))
            .collect(),
    }
}

/// The agent's six classes on the wire's surface: wire-forgery checked, mcp-identity skipped.
fn valid() -> CoverageLedger {
    let mut l = ruled(Speciality::Agent, WIRE);
    l.entries[2].state = checked(REPORT, HASH, TEST);
    l.entries[4].state = State::Skipped {
        reason: "no MCP server is configured in this world yet".to_owned(),
        risk: None,
    };
    l
}

/// The supply's five classes on the lockfile: known-vulnerability checked, workflow-injection
/// (high-risk) ruled out with the file that shows it.
fn supply() -> CoverageLedger {
    let mut l = ruled(Speciality::Supply, LOCKS);
    l.entries[0].state = checked(LOCK, HASH, AUDIT);
    l.entries[2].state = State::NotApplicable {
        reason: "the landing changes no workflow; the lockfile has no action pins".to_owned(),
        evidence: Evidence {
            files: vec![file(LOCK, HASH)],
            commands: Vec::new(),
        },
    };
    l
}

/// A ruling with no evidence.
fn ruling(reason: &str) -> State {
    State::NotApplicable {
        reason: reason.to_owned(),
        evidence: Evidence::default(),
    }
}

/// The fixture that breaks `rule`, and only it, with its landing.
fn fixture(rule: Rule) -> (CoverageLedger, Landing) {
    let mut l = valid();
    let mut land = landing();
    match rule {
        Rule::Oversize => {
            l.entries[0].state = ruling(&format!("a long fact {}", "x".repeat(MAX_TEXT)));
        }
        Rule::UnreadableText => {
            l.entries[0].state = ruling("the landing adds no reader\u{202E} reversed");
        }
        Rule::Unrouted => {
            l.speciality = Speciality::Web;
            l.entries.clear();
        }
        Rule::MissingPair => {
            l.entries.remove(0);
        }
        Rule::DuplicatePair => {
            let mut first = l.entries[0].clone();
            first.state = ruling("a second fact for the same pair, stated again");
            l.entries.push(first);
        }
        Rule::ExtraPair => l.entries.push(na(WIRE, "toctou", 9)),
        Rule::BoilerplateReason => {
            l.entries[0].state = ruling("Not relevant to this change.");
        }
        Rule::UniformReason => {
            for n in [0, 1, 3] {
                l.entries[n].state = ruling("The landing adds no reader this class could reach.");
            }
        }
        // The agent owns one high-risk class, which its surface needs checked: the supply's
        // second one is where a skip or a ruling of a high-risk class can stand alone.
        Rule::UnstatedRisk => {
            land = landing_of(Speciality::Supply, LOCKS, LOCK);
            l = supply();
            l.entries[2].state = State::Skipped {
                reason: "the workflow files are being rewritten in the next landing".to_owned(),
                risk: Some("  n/a  ".to_owned()),
            };
        }
        Rule::UnprovenRuling => {
            land = landing_of(Speciality::Supply, LOCKS, LOCK);
            l = supply();
            l.entries[2].state =
                ruling("the landing changes no workflow; the lockfile has no action pins");
        }
        Rule::UncheckedSurface => {
            l.entries[2].state = State::NotApplicable {
                reason: "the landing changes only the courier's hash, read by no parser".to_owned(),
                evidence: Evidence {
                    files: vec![file(REPORT, HASH)],
                    commands: Vec::new(),
                },
            };
        }
        Rule::HollowCheck => {
            l.entries[2].state = State::Checked {
                method: "read the whole parser and its tests".to_owned(),
                evidence: Evidence::default(),
            };
        }
        Rule::UnnamedSymbols => {
            let mut c = checked(REPORT, HASH, TEST);
            if let State::Checked { evidence, .. } = &mut c {
                evidence.files[0].symbols = vec![" ".to_owned()];
            }
            l.entries[2].state = c;
        }
        Rule::OutsideEvidence => {
            l.entries[2].state = checked("crates/mnemorph-tools/src/bash.rs", HASH, TEST);
        }
        Rule::UnhashedEvidence => l.entries[2].state = checked(REPORT, "abc", TEST),
        Rule::StaleEvidence => l.entries[2].state = checked(REPORT, &"0".repeat(64), TEST),
        Rule::ReusedEvidence => {
            for n in [0, 1, 3] {
                l.entries[n].state = State::Checked {
                    method: format!("read the parser for class {n} and its tests whole"),
                    evidence: Evidence {
                        files: vec![file(REPORT, HASH)],
                        commands: Vec::new(),
                    },
                };
            }
        }
        Rule::UnrunCommand => {
            l.entries[2].state = checked(REPORT, HASH, "cargo test -p mnemorph-wire --all");
        }
    }
    (l, land)
}

fn rules_broken(l: &CoverageLedger, land: &Landing, rules: &[Rule]) -> Vec<Rule> {
    let (t, r) = (tree(), ran());
    let sources = Sources {
        tree: Some(&t),
        ran: Some(&r),
    };
    let mut broken: Vec<Rule> = check(l, land, rules, sources)
        .into_iter()
        .map(|b| b.rule)
        .collect();
    broken.sort();
    broken.dedup();
    broken
}

#[test]
fn a_full_ledger_holds() {
    let (t, r) = (tree(), ran());
    for (l, land) in [
        (valid(), landing()),
        (supply(), landing_of(Speciality::Supply, LOCKS, LOCK)),
    ] {
        assert_eq!(validate(&l, &land), Ok(()));
        let sources = Sources {
            tree: Some(&t),
            ran: Some(&r),
        };
        assert_eq!(validate_in(&l, &land, sources), Ok(()));
    }
}

#[test]
fn each_rule_refuses_its_fixture_alone() {
    for rule in Rule::ALL {
        let (l, land) = fixture(rule);
        assert_eq!(rules_broken(&l, &land, &Rule::ALL), [rule], "{rule}");
    }
}

/// Switch one rule off and its fixture passes: the rule alone stood between it and a pass.
#[test]
fn each_rule_switched_off_lets_its_fixture_through() {
    for rule in Rule::ALL {
        let (l, land) = fixture(rule);
        let others: Vec<Rule> = Rule::ALL.into_iter().filter(|&r| r != rule).collect();
        assert_eq!(rules_broken(&l, &land, &others), [], "{rule}");
    }
}

/// The hollow pass: every pair ruled out by one copied reason clears nothing.
#[test]
fn a_ledger_of_one_copied_ruling_is_refused() {
    let mut l = ruled(Speciality::Agent, WIRE);
    for e in &mut l.entries {
        e.state = State::NotApplicable {
            reason: "the landing only renames a test helper; no input reaches it".to_owned(),
            evidence: Evidence::default(),
        };
    }
    assert_eq!(
        rules_broken(&l, &landing(), &Rule::ALL),
        [
            Rule::UniformReason,
            Rule::UnprovenRuling,
            Rule::UncheckedSurface
        ]
    );
    // Each ruling with its own fact still leaves the surface unchecked.
    let l = ruled(Speciality::Agent, WIRE);
    assert_eq!(
        rules_broken(&l, &landing(), &Rule::ALL),
        [Rule::UnprovenRuling, Rule::UncheckedSurface]
    );
}

#[test]
fn every_hunter_owns_a_high_risk_class() {
    for s in Speciality::HUNTERS {
        assert!(s.classes().iter().any(|c| HIGH_RISK.contains(c)), "{s}");
    }
}

#[test]
fn a_command_is_stamped_from_the_journal_or_refused() {
    let mut l = valid();
    if let State::Checked { evidence, .. } = &mut l.entries[2].state {
        evidence.commands[0].sha256 = None;
        evidence.commands.push(CommandRun {
            run: "echo forged".to_owned(),
            sha256: None,
        });
    }
    // Unstamped, a stored ledger holds no hash for its commands.
    assert_eq!(
        validate(&l, &landing()).map_err(|r| r.rules()),
        Err(vec![Rule::UnhashedEvidence])
    );
    stamp(&mut l, &ran());
    let State::Checked { evidence, .. } = &l.entries[2].state else {
        unreachable!()
    };
    assert_eq!(evidence.commands[0].sha256.as_deref(), Some(HASH));
    assert_eq!(
        evidence.commands[1].sha256, None,
        "never run, never stamped"
    );
    assert_eq!(
        rules_broken(&l, &landing(), &Rule::ALL),
        [Rule::UnrunCommand]
    );
    // Run, but with another output.
    let mut other = Ran::default();
    other.record(TEST, "something else");
    let sources = Sources {
        tree: None,
        ran: Some(&other),
    };
    let broken = check(&valid(), &landing(), &[Rule::UnrunCommand], sources);
    assert_eq!(broken.len(), 1, "{broken:?}");
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
    let (l, land) = fixture(Rule::MissingPair);
    let Err(r) = validate(&l, &land) else {
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
    assert!(substantive(
        "the landing adds no reader this class could reach"
    ));
    assert!(!substantive(&format!("too{}short", " ".repeat(40))));
}

#[test]
fn a_skip_of_a_low_risk_class_needs_no_risk() {
    let mut l = valid();
    l.entries[0].state = State::Skipped {
        reason: "the prompts are being rewritten in the next landing".to_owned(),
        risk: None,
    };
    assert_eq!(rules_broken(&l, &landing(), &Rule::ALL), []);
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
    l.entries.push(na(WIRE, "frame-confusion", 0));
    assert_eq!(
        validate(&l, &land).map_err(|r| r.rules()),
        Err(vec![Rule::ExtraPair])
    );
}

#[test]
fn evidence_is_a_plain_repository_path_on_the_surfaces() {
    for (path, inside) in [
        ("./crates/mnemorph-wire/src/report.rs", true),
        ("crates/mnemorph-wire\\src\\report.rs", true),
        ("crates//mnemorph-wire/src/report.rs", false),
        (
            "crates/mnemorph-tools/../mnemorph-wire/src/report.rs",
            false,
        ),
        ("crates/mnemorph-wire/../../etc/passwd", false),
        ("/home/me/.ssh/id_ed25519", false),
        ("../crates/mnemorph-wire/src/report.rs", false),
        ("crates/mnemorph-wire/", false),
        ("", false),
    ] {
        let mut l = valid();
        l.entries[2].state = checked(path, HASH, TEST);
        let broken = check(&l, &landing(), &[Rule::OutsideEvidence], Sources::default());
        assert_eq!(broken.is_empty(), inside, "{path}");
    }
}

#[test]
fn a_declared_surface_holds_as_ownership_reads_it() {
    let dir = Surface {
        at: "src/auth/".to_owned(),
        declared: true,
    };
    let one = Surface {
        at: "auth.py".to_owned(),
        declared: true,
    };
    assert!(dir.holds("src/auth/token.rs"));
    assert!(!dir.holds("src/authz.rs"));
    assert!(one.holds("auth.py"));
    assert!(!one.holds("auth.pyc"));
}

#[test]
fn a_ledger_over_the_entry_cap_is_refused_unread() {
    let mut l = valid();
    l.entries = vec![na(WIRE, "wire-forgery", 0); MAX_ENTRIES + 1];
    let breaches = check(&l, &landing(), &Rule::ALL, Sources::default());
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].rule, Rule::Oversize);
    let others: Vec<Rule> = Rule::ALL
        .into_iter()
        .filter(|&r| r != Rule::Oversize)
        .collect();
    let r = refused(check(&l, &landing(), &others, Sources::default())).unwrap_err();
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
        l.entries.push(na(WIRE, &format!("made-up-{n}"), n));
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
    let broken = check(&l, &landing(), &[Rule::Oversize], Sources::default());
    assert_eq!(broken.len(), 2, "{broken:?}");
}

#[test]
fn the_ledger_reads_from_json_and_refuses_unknown_fields() {
    let json = format!(
        r#"{{"landing":"run-1:3","speciality":"dark-elf-agent","entries":[
            {{"surface":"{WIRE}","class":"wire-forgery","state":{{"checked":{{"method":"m","evidence":{{"files":[{{"path":"{REPORT}","symbols":["x"],"sha256":"{HASH}"}}],"commands":[{{"run":"{TEST}"}}]}}}}}}}},
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
        Just(TEST.to_owned()),
        "\\PC{0,40}",
        "[a-z/*.\\\\ -]{0,30}",
        ".{0,60}",
        "[\u{0}-\u{1f}\u{202a}-\u{202e}\u{200b}a]{0,10}",
    ]
}

fn evidence() -> impl Strategy<Value = Evidence> {
    (
        proptest::collection::vec(
            (
                hostile(),
                proptest::collection::vec(hostile(), 0..3),
                hostile(),
            ),
            0..3,
        ),
        proptest::collection::vec((hostile(), proptest::option::of(hostile())), 0..3),
    )
        .prop_map(|(files, commands)| Evidence {
            files: files
                .into_iter()
                .map(|(path, symbols, sha256)| FileRead {
                    path,
                    symbols,
                    sha256,
                })
                .collect(),
            commands: commands
                .into_iter()
                .map(|(run, sha256)| CommandRun { run, sha256 })
                .collect(),
        })
}

fn state() -> impl Strategy<Value = State> {
    prop_oneof![
        (hostile(), evidence())
            .prop_map(|(reason, evidence)| State::NotApplicable { reason, evidence }),
        (hostile(), proptest::option::of(hostile()))
            .prop_map(|(reason, risk)| State::Skipped { reason, risk }),
        (hostile(), evidence()).prop_map(|(method, evidence)| State::Checked { method, evidence }),
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
        let (t, r) = (tree(), ran());
        let sources = Sources { tree: Some(&t), ran: Some(&r) };
        let first = check(&l, &landing(), &Rule::ALL, sources);
        prop_assert_eq!(&first, &check(&l, &landing(), &Rule::ALL, sources));
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
        let v = valid();
        let mut entries: Vec<Entry> = v
            .entries
            .iter()
            .zip(&counts)
            .enumerate()
            .flat_map(|(n, (e, &k))| {
                (0..k).map(move |i| {
                    let mut e = e.clone();
                    if let State::NotApplicable { reason, .. } = &mut e.state {
                        *reason = format!("fact {n}.{i}: the landing adds no reader here");
                    }
                    e
                })
            })
            .collect();
        if let Some(x) = &extra {
            entries.push(na(WIRE, x, 99));
        }
        let l = CoverageLedger { entries, ..v };
        let exact = counts.iter().all(|&n| n == 1) && extra.is_none();
        prop_assert_eq!(validate(&l, &landing()).is_ok(), exact);
    }

    /// Nothing under the floor says enough.
    #[test]
    fn short_prose_never_passes(text in "\\PC{0,19}") {
        prop_assert!(!substantive(&text));
    }
}
