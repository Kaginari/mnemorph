//! The write rule: a body's words land personal and proposed; only a route's proof grounds them,
//! and every proof is refused without the evidence its route needs.

use std::fs;

use mnemorph_memory::{Proof, Route, Status, Tier, fold, ground, propose, read};

#[test]
fn every_proof_is_refused_without_its_evidence() {
    assert!(
        Proof::checked("log.md: gate PASS", 0, 3).is_err(),
        "no verify line ran"
    );
    assert!(
        Proof::checked("log.md: gate PASS", 2, 0).is_err(),
        "nothing changed"
    );
    assert_eq!(
        Proof::checked("log.md: gate PASS", 1, 1).unwrap().route(),
        Route::Checked
    );
    assert!(Proof::accepted("verdict", false).is_err());
    let anchors = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert!(
        Proof::corroborated("slime-a", "slime-a", &anchors(&["x"]), &anchors(&["y"])).is_err(),
        "no self-corroboration"
    );
    assert!(
        Proof::corroborated(
            "slime-a",
            "slime-b",
            &anchors(&["src/x.rs"]),
            &anchors(&["src/x.rs"])
        )
        .is_err(),
        "one anchor is one piece of evidence"
    );
    assert!(Proof::corroborated("slime-a", "slime-b", &anchors(&[]), &anchors(&["y"])).is_err());
    assert_eq!(
        Proof::corroborated(
            "slime-a",
            "slime-b",
            &anchors(&["src/x.rs"]),
            &anchors(&["log.md#4"])
        )
        .unwrap()
        .route(),
        Route::Corroborated
    );
    assert_eq!(
        Proof::confirmed("the human, in session s1").route(),
        Route::Confirmed
    );
}

#[test]
fn a_claim_lands_proposed_and_a_route_grounds_it_in_both_tiers() {
    let w = std::env::temp_dir().join(format!("mnemorph-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    let claim = propose(
        &w,
        "slime-s",
        "the parser keeps interval fractions",
        vec!["law".into()],
        Some("src/parser.rs".into()),
    )
    .unwrap();
    let personal = Tier::Personal("slime-s".into());
    let mine = fold(&read(&w, &personal).unwrap().rows);
    let n = &mine[&claim.note];
    assert_eq!(
        (n.status, n.salience, n.confidence),
        (Status::Proposed, 0.6, 0.5)
    );
    assert!(
        fold(&read(&w, &Tier::Shared).unwrap().rows).is_empty(),
        "nothing unconfirmed is shared"
    );
    let proof = Proof::checked("log.md: slime-s gate PASS", 2, 3).unwrap();
    let grounded = ground(&w, claim, &proof).unwrap();
    let mine = fold(&read(&w, &personal).unwrap().rows);
    let shared = fold(&read(&w, &Tier::Shared).unwrap().rows);
    for n in [&mine[&grounded.note], &shared[&grounded.note]] {
        assert_eq!(n.status, Status::Grounded);
        assert!((n.confidence - 0.7).abs() < 1e-9, "a check grounds at 0.7");
        assert!(
            n.proof
                .as_deref()
                .unwrap()
                .starts_with("checked: log.md: slime-s gate PASS")
        );
    }
    assert_eq!(shared[&grounded.note].owner.as_deref(), Some("slime-s"));
    let _ = fs::remove_dir_all(&w);
}

/// A run-ending failure: one grounded revise note per failed act, linking each recalled anchor —
/// 0.9 when cited, 0.4 when merely recalled — never a dissenting one; shared when any is shared.
#[test]
fn a_run_ending_failure_revises_what_it_rode_on() {
    use mnemorph_memory::{Anchor, Polarity, revise};
    let w = std::env::temp_dir().join(format!("mnemorph-revise-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    let anchor = |id: &str, personal: bool, dissent: bool, cited: bool| Anchor {
        id: id.into(),
        personal,
        dissent,
        cited,
    };
    // Nothing recalled, or only dissent: nothing written.
    assert_eq!(
        revise(&w, "slime-s", &["x".into()], &[], "log.md v").unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        revise(
            &w,
            "slime-s",
            &["x".into()],
            &[anchor("c", false, true, false)],
            "v"
        )
        .unwrap(),
        Vec::<String>::new()
    );
    let made = revise(
        &w,
        "slime-s",
        &[
            "traits hold: `cargo test` failed".into(),
            "right author: `lib/x` is not yours".into(),
        ],
        &[
            anchor("cited", true, false, true),
            anchor("bystander", false, false, false),
            anchor("dissent", false, true, false),
        ],
        "log.md ### [t] slime-s — gate: FAIL",
    )
    .unwrap();
    assert_eq!(made.len(), 2, "one per failed act");
    let shared = fold(&read(&w, &Tier::Shared).unwrap().rows);
    for id in &made {
        let n = &shared[id];
        assert_eq!((n.polarity, n.status), (Polarity::Revise, Status::Grounded));
        assert!((n.salience - 1.0).abs() < 1e-9 && (n.confidence - 0.9).abs() < 1e-9);
        let links: Vec<(&str, f64)> = n
            .links
            .iter()
            .map(|l| (l.note.as_str(), l.weight))
            .collect();
        assert_eq!(
            links,
            [("cited", 0.9), ("bystander", 0.4)],
            "never the dissent"
        );
    }
    // Only personal anchors: the revise lands in the owner's personal tier.
    let personal = revise(
        &w,
        "slime-s",
        &["x".into()],
        &[anchor("mine", true, false, false)],
        "v",
    )
    .unwrap();
    let mine = fold(&read(&w, &Tier::Personal("slime-s".into())).unwrap().rows);
    assert!(mine.contains_key(&personal[0]));
    let _ = fs::remove_dir_all(&w);
}

/// One failed rule is one revise note, however many lines it failed by: eight foreign paths are
/// one `right author` failure.
#[test]
fn one_rule_failed_on_many_lines_is_one_revise_note() {
    use mnemorph_memory::{Anchor, revise};
    let w = std::env::temp_dir().join(format!("mnemorph-revise-rules-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    let acts: Vec<String> = (0..8)
        .map(|i| format!("right author: `lib/x{i}.rs` is slime-b's, not slime-a's\nmore detail"))
        .collect();
    let made = revise(
        &w,
        "slime-a",
        &acts,
        &[Anchor {
            id: "n".into(),
            personal: false,
            dissent: false,
            cited: false,
        }],
        "log.md v",
    )
    .unwrap();
    assert_eq!(made.len(), 1);
    let n = &fold(&read(&w, &Tier::Shared).unwrap().rows)[&made[0]];
    assert!(
        n.text.starts_with("right author: `lib/x0.rs`"),
        "{}",
        n.text
    );
    assert!(
        !n.text.contains("more detail"),
        "only each line's first line"
    );
    let _ = fs::remove_dir_all(&w);
}

/// What two run-ending failures cited is retired: dormant, kept, no longer recalled. One failure,
/// or a mere recall, retires nothing.
#[test]
fn a_note_cited_by_two_failures_is_retired() {
    use mnemorph_memory::{Anchor, retire, revise};
    let w = std::env::temp_dir().join(format!("mnemorph-retire-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    let cited = |id: &str, cited: bool| Anchor {
        id: id.into(),
        personal: false,
        dissent: false,
        cited,
    };
    let bad = ground(
        &w,
        propose(&w, "slime-s", "skip the fixture", vec![], None).unwrap(),
        &Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    let bystander = ground(
        &w,
        propose(&w, "slime-s", "the lexer is pure", vec![], None).unwrap(),
        &Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    let anchors = [cited(&bad, true), cited(&bystander, false)];
    revise(
        &w,
        "slime-s",
        &["traits hold: broke".into()],
        &anchors,
        "log.md one",
    )
    .unwrap();
    assert_eq!(
        retire(&w, "slime-s", &anchors, "log.md one").unwrap(),
        Vec::<String>::new()
    );
    revise(
        &w,
        "slime-s",
        &["traits hold: broke again".into()],
        &anchors,
        "log.md two",
    )
    .unwrap();
    assert_eq!(
        retire(&w, "slime-s", &anchors, "log.md two").unwrap(),
        vec![bad.clone()]
    );
    let notes = fold(&read(&w, &Tier::Shared).unwrap().rows);
    assert_eq!(notes[&bad].status, Status::Dormant);
    assert_eq!(
        notes[&bystander].status,
        Status::Grounded,
        "merely recalled"
    );
    assert!(
        mnemorph_memory::findings(&w, "slime-s").unwrap().is_empty(),
        "the retirement resolves"
    );
    let _ = fs::remove_dir_all(&w);
}

/// A personal note's strikes are counted wherever `revise` wrote them, and a note the human woke
/// starts clean: one more failure does not retire it again.
#[test]
fn strikes_count_across_tiers_and_restart_after_a_waking() {
    use mnemorph_memory::{Anchor, retire, revise};
    let w = std::env::temp_dir().join(format!("mnemorph-strikes-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    // A personal note, cited, recalled beside a shared one: the revise notes land shared.
    let mine = propose(&w, "slime-s", "skip the fixture", vec![], None)
        .unwrap()
        .note;
    let shared = ground(
        &w,
        propose(&w, "slime-o", "the lexer is pure", vec![], None).unwrap(),
        &Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    let anchors = [
        Anchor {
            id: mine.clone(),
            personal: true,
            dissent: false,
            cited: true,
        },
        Anchor {
            id: shared,
            personal: false,
            dissent: false,
            cited: false,
        },
    ];
    for v in ["log.md one", "log.md two"] {
        revise(&w, "slime-s", &["traits hold: broke".into()], &anchors, v).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    assert_eq!(
        retire(&w, "slime-s", &anchors, "log.md two").unwrap(),
        vec![mine.clone()]
    );
    // The human wakes it: a grounded status, by hand.
    let personal = Tier::Personal("slime-s".into());
    let wake = format!(
        r#"{{"id":"wake","time":{},"note":"{mine}","by":"human","kind":"status","status":"grounded"}}"#,
        mnemorph_memory::store::now_ms()
    );
    let path = w.join(personal.file());
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str(&wake);
    text.push('\n');
    fs::write(&path, text).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(3));
    revise(
        &w,
        "slime-s",
        &["traits hold: once more".into()],
        &anchors,
        "log.md three",
    )
    .unwrap();
    assert_eq!(
        retire(&w, "slime-s", &anchors, "log.md three").unwrap(),
        Vec::<String>::new(),
        "only failures since the waking count"
    );
    let _ = fs::remove_dir_all(&w);
}
