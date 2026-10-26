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
