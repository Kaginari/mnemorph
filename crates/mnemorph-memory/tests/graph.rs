//! The claim graph: fixed ids and a chain of versions, review-on-invalidate through what a claim
//! rests on — transitive, and ending on a cycle — and a strength that never lets an unproven claim
//! outrank a grounded one.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_memory::{
    Change, Class, Dependency, Moved, Note, Polarity, Proof, Relation, Status, Tier, Weights,
    amend, fold, ground, invalidate, landed, lines, propose, propose_on, propose_resting, read,
    recall, strengths,
};

const DAY: u64 = 86_400_000;

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-graph-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join("src")).unwrap();
    w
}

fn note_of(w: &Path, tier: &Tier, id: &str) -> Note {
    fold(&read(w, tier).unwrap().rows).remove(id).unwrap()
}

fn reviews(w: &Path, tier: &Tier) -> usize {
    read(w, tier)
        .unwrap()
        .rows
        .iter()
        .filter(|e| matches!(e.change, Change::Review { .. }))
        .count()
}

fn on(id: &str) -> Dependency {
    Dependency::Claim { id: id.into() }
}

/// A claim's id is fixed whatever its text becomes: each correction is the next version, the
/// earlier ones kept, the line that said the first never rewritten — and a correction waits for a
/// route of its own.
#[test]
fn ids_are_stable_across_edits_and_versions_chain() {
    let w = world("versions");
    let claim = propose(&w, "slime-s", "the parser drops collations", vec![], None).unwrap();
    let id = claim.note.clone();
    ground(&w, claim, &Proof::confirmed("the human")).unwrap();
    let first_line = fs::read_to_string(w.join(Tier::Personal("slime-s".into()).file())).unwrap();
    let (v2, _) = amend(
        &w,
        "slime-s",
        &id,
        "the parser drops collations on ALTER",
        vec![],
    )
    .unwrap();
    let (v3, _) = amend(
        &w,
        "slime-s",
        &id,
        "the parser keeps collations since 0.4",
        vec![],
    )
    .unwrap();
    assert_eq!((v2, v3), (2, 3));
    for tier in [Tier::Personal("slime-s".into()), Tier::Shared] {
        let n = note_of(&w, &tier, &id);
        assert_eq!(n.id, id, "the id never moves");
        assert_eq!(n.version, 3);
        let chain: Vec<(u32, &str)> = n.versions.iter().map(|v| (v.n, v.text.as_str())).collect();
        assert_eq!(
            chain,
            vec![
                (1, "the parser drops collations"),
                (2, "the parser drops collations on ALTER"),
                (3, "the parser keeps collations since 0.4"),
            ]
        );
        assert_eq!(n.text, "the parser keeps collations since 0.4");
        assert_eq!(n.status, Status::Proposed, "a correction is grounded anew");
        assert_eq!(n.groundings, Vec::<String>::new());
    }
    let now = fs::read_to_string(w.join(Tier::Personal("slime-s".into()).file())).unwrap();
    assert!(
        now.starts_with(&first_line),
        "append-only: nothing rewritten"
    );
    assert!(amend(&w, "slime-other", &id, "not mine", vec![]).is_err());
    let _ = fs::remove_dir_all(&w);
}

/// A landing that changes a file marks the claims resting on it, and everything resting on those,
/// each once: a cycle among them ends. A claim that recorded the new bytes stands, and a route that
/// grounds a marked claim again answers its review.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one story: a chain, a cycle, a landing, a re-grounding"
)]
fn a_changed_file_marks_its_dependents_transitively_and_a_cycle_ends() {
    let w = world("landing");
    fs::write(w.join("src/lexer.rs"), "fn lex() {}").unwrap();
    fs::write(w.join("src/other.rs"), "fn other() {}").unwrap();
    let base = propose_on(
        &w,
        "slime-s",
        "src/lexer.rs splits on whitespace",
        vec![],
        None,
        vec!["src/lexer.rs".into()],
    )
    .unwrap();
    let base_id = base.note.clone();
    assert!(
        matches!(&base.depends_on[..], [Dependency::File { path, .. }] if path == "src/lexer.rs"),
        "the evidence's file is recorded with its hash: {:?}",
        base.depends_on
    );
    ground(&w, base, &Proof::confirmed("the human")).unwrap();
    // c3 rests on c2, c2 on the base and on c3: a cycle downstream.
    let c3_id = "pending";
    let c2 = propose_resting(
        &w,
        "slime-t",
        "tokens never hold spaces",
        vec![],
        None,
        vec![],
        vec![on(&base_id), on(c3_id)],
    )
    .unwrap();
    let c3 = propose_resting(
        &w,
        "slime-u",
        "the formatter may split on spaces",
        vec![],
        None,
        vec![],
        vec![on(&c2.note)],
    )
    .unwrap();
    // Close the cycle: c2's next version rests on c3 by its real id.
    amend(
        &w,
        "slime-t",
        &c2.note,
        "tokens never hold spaces",
        vec![on(&base_id), on(&c3.note)],
    )
    .unwrap();
    // That correction superseded what c3 rests on: c3 waits under review. The human grounds it,
    // so the landing below is what marks it again.
    assert!(
        note_of(&w, &Tier::Personal("slime-u".into()), &c3.note)
            .review
            .as_deref()
            .is_some_and(|c| c.contains("superseded by version 2"))
    );
    let c3_again = mnemorph_memory::proposed(&w, "slime-u", &c3.note)
        .unwrap()
        .unwrap();
    ground(&w, c3_again, &Proof::confirmed("the human")).unwrap();
    assert!(note_of(&w, &Tier::Shared, &c3.note).review.is_none());
    let apart = propose_on(
        &w,
        "slime-s",
        "src/other.rs is unrelated",
        vec![],
        None,
        vec!["src/other.rs".into()],
    )
    .unwrap();

    // Nothing changed yet: a landing over the same bytes marks nothing.
    assert_eq!(
        landed(&w, &["src/lexer.rs".into()], "log.md #1").unwrap(),
        Vec::<String>::new(),
    );
    fs::write(w.join("src/lexer.rs"), "fn lex() { split() }").unwrap();
    let marked = landed(&w, &["src/lexer.rs".into()], "log.md #2").unwrap();
    assert_eq!(
        marked,
        vec![base_id.clone(), c2.note.clone(), c3.note.clone()],
        "the base, then downstream, each once"
    );
    let base_now = note_of(&w, &Tier::Shared, &base_id);
    assert_eq!(base_now.status, Status::Grounded, "marked, never retired");
    assert!(
        base_now
            .review
            .as_deref()
            .is_some_and(|c| c.contains("src/lexer.rs") && c.contains("log.md #2")),
        "{:?}",
        base_now.review
    );
    assert!(
        note_of(&w, &Tier::Personal("slime-t".into()), &c2.note)
            .review
            .is_some()
    );
    assert!(
        note_of(&w, &Tier::Personal("slime-u".into()), &c3.note)
            .review
            .as_deref()
            .is_some_and(|c| c.contains(&c2.note) && c.contains("which needs review")),
        "transitively, through c2"
    );
    assert!(
        note_of(&w, &Tier::Personal("slime-s".into()), &apart.note)
            .review
            .is_none()
    );
    // Marked once: a second landing of the same change writes no second review.
    let before = reviews(&w, &Tier::Shared);
    landed(&w, &["src/lexer.rs".into()], "log.md #3").unwrap();
    assert_eq!(reviews(&w, &Tier::Shared), before);
    // A route grounds it again: the review is answered, and the new bytes recorded.
    let again = mnemorph_memory::proposed(&w, "slime-s", &base_id)
        .unwrap()
        .expect("a claim under review waits for a route");
    ground(&w, again, &Proof::confirmed("the human, again")).unwrap();
    assert!(note_of(&w, &Tier::Shared, &base_id).review.is_none());
    assert_eq!(
        landed(&w, &["src/lexer.rs".into()], "log.md #4").unwrap(),
        Vec::<String>::new(),
        "the grounding saw these bytes"
    );
    let _ = fs::remove_dir_all(&w);
}

/// A claim retired by the failures laid at its door no longer holds up what rests on it: its
/// dependents need review, and the retirement says which.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one story: a doubt, a re-grounding, a retirement"
)]
fn a_retired_dependency_marks_its_dependents() {
    use mnemorph_memory::{Anchor, retire, revise};
    let w = world("retired");
    let bad = ground(
        &w,
        propose(
            &w,
            "slime-s",
            "skip the fixture tests/fixtures/utf8.sql",
            vec![],
            None,
        )
        .unwrap(),
        &Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    let rests = propose_resting(
        &w,
        "slime-t",
        "the utf8 suite needs no fixture",
        vec![],
        None,
        vec![],
        vec![on(&bad)],
    )
    .unwrap();
    let far = propose_resting(
        &w,
        "slime-u",
        "the utf8 suite runs in a second",
        vec![],
        None,
        vec![],
        vec![on(&rests.note)],
    )
    .unwrap();
    let anchors = [Anchor {
        id: bad.clone(),
        personal: false,
        dissent: false,
        cited: true,
    }];
    let mut seen = Vec::new();
    for v in ["log.md one", "log.md two"] {
        revise(
            &w,
            "slime-s",
            &["traits hold: tests/fixtures/utf8.sql::round_trip failed".into()],
            &anchors,
            v,
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        seen.push(retire(&w, "slime-s", &anchors, v).unwrap());
        if seen.len() == 1 {
            // The first failure laid at its door doubts it: what rests on it directly is marked,
            // and no further — one failure never flags a whole subgraph.
            let n = note_of(&w, &Tier::Personal("slime-t".into()), &rests.note);
            assert!(
                n.review
                    .as_deref()
                    .is_some_and(|c| c.contains(&bad) && c.contains("doubted by a failure")),
                "{:?}",
                n.review
            );
            assert_eq!(seen[0].reviewed, vec![rests.note.clone()]);
            assert!(
                note_of(&w, &Tier::Personal("slime-u".into()), &far.note)
                    .review
                    .is_none(),
                "a doubt reaches no further"
            );
            // The human grounds it again; the retirement marks it anew.
            let again = mnemorph_memory::proposed(&w, "slime-t", &rests.note)
                .unwrap()
                .unwrap();
            ground(&w, again, &Proof::confirmed("the human")).unwrap();
        }
    }
    assert_eq!(seen[0].doubted, vec![bad.clone()]);
    assert_eq!(seen[1].retired, vec![bad.clone()]);
    assert_eq!(
        seen[1].reviewed,
        vec![rests.note.clone(), far.note.clone()],
        "a retirement reaches downstream"
    );
    let n = note_of(&w, &Tier::Personal("slime-t".into()), &rests.note);
    assert!(
        n.review
            .as_deref()
            .is_some_and(|c| c.contains(&bad) && c.contains("retired")),
        "{:?}",
        n.review
    );
    assert_eq!(
        n.status,
        Status::Grounded,
        "marked, neither deleted nor retired"
    );
    assert!(
        mnemorph_memory::findings(&w, "slime-t").unwrap().is_empty(),
        "a review claims no standing: it resolves"
    );
    let _ = fs::remove_dir_all(&w);
}

/// A fact newly contested — one body's counter can do it — marks only the claims resting on it
/// directly; a creature rebirthed or retired marks downstream too.
#[test]
fn a_moved_fact_marks_the_claims_resting_on_it() {
    let w = world("fact");
    let c = propose_resting(
        &w,
        "slime-s",
        "the API caps at 100 a minute",
        vec![],
        None,
        vec![],
        vec![Dependency::Fact {
            id: "abc123".into(),
        }],
    )
    .unwrap();
    let below = |owner: &str, id: &str| {
        propose_resting(
            &w,
            owner,
            "it rests further",
            vec![],
            None,
            vec![],
            vec![on(id)],
        )
        .unwrap()
        .note
    };
    let c_far = below("slime-v", &c.note);
    assert_eq!(
        invalidate(&w, Moved::Fact("other"), "contested").unwrap(),
        Vec::<String>::new(),
    );
    assert_eq!(
        invalidate(&w, Moved::Fact("abc123"), "fact #abc123 is contested").unwrap(),
        vec![c.note],
        "direct only"
    );
    assert!(
        note_of(&w, &Tier::Personal("slime-v".into()), &c_far)
            .review
            .is_none()
    );
    let d = propose_resting(
        &w,
        "slime-t",
        "slime-s owns the lexer",
        vec![],
        None,
        vec![],
        vec![Dependency::Creature {
            name: "slime-s".into(),
        }],
    )
    .unwrap();
    let d_far = below("slime-w", &d.note);
    assert_eq!(
        invalidate(&w, Moved::Creature("slime-s"), "slime-s was rebirthed").unwrap(),
        vec![d.note, d_far],
        "downstream"
    );
    let _ = fs::remove_dir_all(&w);
}

fn grounded(id: &str, proof: &str, confidence: f64, groundings: usize, age_days: u64) -> Note {
    Note {
        id: id.into(),
        text: "the tokenizer keeps interval fractions whole".into(),
        polarity: Polarity::Support,
        salience: 0.6,
        confidence,
        status: Status::Grounded,
        proof: Some(proof.into()),
        born: 1000 * DAY - age_days * DAY,
        grounded_at: Some(1000 * DAY - age_days * DAY),
        groundings: (0..groundings).map(|i| format!("{proof} #{i}")).collect(),
        version: 1,
        ..Note::default()
    }
}

/// Notes about other things: a memory holds more than the question, so its words are rare.
fn others() -> Vec<(Note, Relation)> {
    (0..12)
        .map(|i| {
            let mut n = grounded(&format!("other{i}"), "checked: v", 0.7, 1, 3);
            n.text = format!("the optimizer pushes predicate {i} down through joins");
            (n, Relation::Shared)
        })
        .collect()
}

/// A small deterministic generator: the property is checked over many draws, the same each run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        #[allow(clippy::cast_precision_loss)]
        let x = (self.0 >> 11) as f64 / (1u64 << 53) as f64;
        x
    }
}

/// With equal relevance — the same words, relation, salience, polarity and age — a grounded claim
/// in good health outranks an unproven one, whatever its route, its corroborations and either's
/// confidence.
#[test]
fn a_grounded_claim_always_outranks_an_unproven_one_of_equal_relevance() {
    let routes = [
        "confirmed: the human",
        "checked: log.md #1",
        "corroborated: slime-x: a.rs",
        "accepted: log.md #2",
    ];
    let mut g = Lcg(7);
    let now = 1000 * DAY;
    for i in 0..500 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (route, count, age) = (
            routes[(g.next() * 4.0) as usize],
            1 + (g.next() * 4.0) as usize,
            (g.next() * 900.0) as u64,
        );
        let proven = grounded("g", route, 0.3 + 0.65 * g.next(), count, age);
        let mut unproven = proven.clone();
        unproven.id = "u".into();
        unproven.status = Status::Proposed;
        unproven.proof = None;
        unproven.groundings.clear();
        unproven.grounded_at = None;
        unproven.confidence = 0.3 + 0.65 * g.next();
        let mut notes = vec![(proven, Relation::Own), (unproven, Relation::Own)];
        notes.extend(others());
        let r = recall(
            "tokenizer interval fractions",
            &notes,
            now,
            6,
            &Weights::default(),
        );
        assert_eq!(r.anchors.len(), 2, "draw {i}");
        assert_eq!(r.anchors[0].note.id, "g", "draw {i}: {:?}", r.anchors);
        assert!(r.anchors[0].strength.value >= 0.5);
        assert!(r.anchors[0].strength.explain().starts_with("observed"));
        assert_eq!(r.anchors[1].strength.value, 0.0);
    }
}

/// A weaker source never yields a stronger claim: grounded on a hypothesis, a claim is one; on a
/// grounded claim, derived and capped by it.
#[test]
fn a_claim_is_never_stronger_than_what_it_rests_on() {
    let now = 1000 * DAY;
    let guess = Note {
        id: "guess".into(),
        status: Status::Proposed,
        ..Note::default()
    };
    let mut on_guess = grounded("a", "confirmed: the human", 0.7, 1, 0);
    on_guess.depends_on = vec![on("guess")];
    let base = grounded("base", "accepted: log.md #1", 0.6, 1, 400);
    let mut derived = grounded("d", "confirmed: the human", 0.7, 3, 0);
    derived.depends_on = vec![on("base"), on("out-of-sight")];
    let s = strengths([&guess, &on_guess, &base, &derived], now);
    assert_eq!(s["a"].class, Class::Hypothesis);
    assert_eq!(s["a"].value, 0.0);
    assert_eq!(s["d"].class, Class::Derived);
    assert!((s["d"].value - s["base"].value).abs() < 1e-12, "capped");
    assert_eq!(s["d"].capped_by.as_deref(), Some("base"));
    assert!((s["d"].health - 0.5).abs() < 1e-12, "one of two in sight");
    assert!(s["d"].explain().contains("capped by `base`"));
    // A cycle is cut where it closes.
    let mut x = grounded("x", "checked: v", 0.7, 1, 0);
    x.depends_on = vec![on("y")];
    let mut y = grounded("y", "checked: v", 0.7, 1, 0);
    y.depends_on = vec![on("x")];
    let s = strengths([&x, &y], now);
    assert_eq!(s["x"].class, Class::Derived);
}

/// Recall shows a claim under review as such, and ranks it below the same claim in good health.
#[test]
fn recall_shows_the_review_state() {
    let now = 1000 * DAY;
    let healthy = grounded("ok", "checked: log.md #1", 0.7, 1, 1);
    let mut moved = grounded("moved", "checked: log.md #1", 0.7, 1, 1);
    moved.review = Some("`src/lexer.rs` changed in log.md #2".into());
    let mut notes = vec![(moved, Relation::Own), (healthy, Relation::Own)];
    notes.extend(others());
    let r = recall(
        "tokenizer interval fractions",
        &notes,
        now,
        6,
        &Weights::default(),
    );
    let ids: Vec<&str> = r.anchors.iter().map(|h| h.note.id.as_str()).collect();
    assert_eq!(ids, vec!["ok", "moved"]);
    assert!(r.anchors[1].strength.review);
    assert!(r.anchors[1].strength.value <= 0.25);
    let said = lines(&r, &std::collections::BTreeSet::new());
    assert!(
        said.contains(
            &"@M moved shared grounded support review: the tokenizer keeps interval fractions whole"
                .to_owned()
        ),
        "{said:?}"
    );
    assert!(
        said.contains(
            &"@M ok shared grounded support: the tokenizer keeps interval fractions whole"
                .to_owned()
        ),
        "a claim in good health reads as before: {said:?}"
    );
}

/// A note written before the graph — no versions, no dependencies, even no note id — reads as
/// version 1 of a claim resting on nothing, its id derived from what it is: the same on every
/// read, nothing rewritten.
#[test]
fn old_notes_upgrade_on_reading() {
    let w = world("upgrade");
    let file = w.join(Tier::Shared.file());
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let old = concat!(
        r#"{"id":"e1","time":5,"note":"n-old","by":"record","proof":null,"kind":"create","text":"an old note","tags":[],"owner":"slime-s","polarity":"support","salience":0.6,"confidence":0.7,"source":null,"links":[]}"#,
        "\n",
        r#"{"id":"e2","time":6,"note":"n-old","by":"human","kind":"status","status":"grounded"}"#,
        "\n",
        r#"{"id":"e3","time":7,"by":"import","proof":"an archive","kind":"create","text":"a note with no id","owner":"slime-s","polarity":"support","salience":0.5,"confidence":0.5}"#,
        "\n",
    );
    fs::write(&file, old).unwrap();
    let first = fold(&read(&w, &Tier::Shared).unwrap().rows);
    let second = fold(&read(&w, &Tier::Shared).unwrap().rows);
    assert_eq!(first, second, "the same ids on every read");
    let n = &first["n-old"];
    assert_eq!((n.version, n.versions.len()), (1, 1));
    assert!(n.depends_on.is_empty() && n.review.is_none());
    assert_eq!(n.groundings.len(), 0, "grounded by a proofless human word");
    let derived: Vec<&String> = first.keys().filter(|k| *k != "n-old").collect();
    assert_eq!(derived.len(), 1);
    assert!(derived[0].starts_with('h') && derived[0].len() == 17);
    assert_eq!(first[derived[0]].text, "a note with no id");
    assert_eq!(fs::read_to_string(&file).unwrap(), old, "nothing rewritten");
    let _ = fs::remove_dir_all(&w);
}

/// A file moved by any hand — a human's edit, no landing — reads as needing review, computed live
/// in recall and in what `memory status` lists, with no event written. A passing ask that cites
/// such a claim marks it for good.
#[test]
fn a_file_moved_outside_a_landing_reads_as_needing_review() {
    use mnemorph_memory::{Anchor, confirm, gather, under_review};
    let w = world("live");
    fs::write(w.join("src/lexer.rs"), "fn lex() {}").unwrap();
    let c = propose_on(
        &w,
        "slime-s",
        "src/lexer.rs splits interval fractions",
        vec![],
        None,
        vec!["src/lexer.rs".into()],
    )
    .unwrap();
    let id = ground(&w, c, &Proof::confirmed("the human")).unwrap().note;
    let tier = Tier::Shared;
    let live = |w: &Path| {
        gather(w, "slime-s", |_| false)
            .unwrap()
            .notes
            .into_iter()
            .find(|(n, _)| n.id == id)
            .unwrap()
            .0
    };
    assert!(live(&w).review.is_none());
    assert_eq!(under_review(&w, "slime-s").unwrap(), vec![]);
    fs::write(w.join("src/lexer.rs"), "fn lex() { todo!() }").unwrap();
    let n = live(&w);
    assert!(
        n.review
            .as_deref()
            .is_some_and(|c| c.contains("src/lexer.rs")),
        "{:?}",
        n.review
    );
    assert_eq!(under_review(&w, "slime-s").unwrap().len(), 1);
    assert_eq!(reviews(&w, &tier), 0, "computed: no event written");
    // A pass that cites it vouches for nothing the bytes now say: marked, as an event.
    let cited = [Anchor {
        id: id.clone(),
        personal: true,
        dissent: false,
        cited: true,
    }];
    assert_eq!(confirm(&w, "slime-s", &cited, "log.md #9").unwrap(), 1);
    let marked = note_of(&w, &Tier::Personal("slime-s".into()), &id);
    assert!(
        marked
            .review
            .as_deref()
            .is_some_and(|c| c.contains("credited by log.md #9")),
        "{:?}",
        marked.review
    );
    // Put back as it was, the file reads as recorded: only the marked event remains.
    fs::write(w.join("src/lexer.rs"), "fn lex() {}").unwrap();
    assert!(note_of(&w, &tier, &id).review.is_some());
    let _ = fs::remove_dir_all(&w);
}

/// Two id-less lines saying the same words, by the same owner, in the same millisecond are two
/// notes: the event id is hashed in. A second creation of one id in other words is reported as a
/// collision, never silently dropped.
#[test]
fn derived_ids_never_collide_and_a_collision_is_reported() {
    let w = world("collide");
    let file = w.join(Tier::Personal("slime-s".into()).file());
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let line = |event: &str, note: &str, text: &str| {
        let note = if note.is_empty() {
            String::new()
        } else {
            format!(r#""note":"{note}","#)
        };
        format!(
            r#"{{"id":"{event}","time":5,{note}"by":"record","kind":"create","text":"{text}","owner":"slime-s","polarity":"support","salience":0.5,"confidence":0.5}}"#
        )
    };
    let lines = [
        line("e1", "", "the same words"),
        line("e2", "", "the same words"),
        line("e3", "n-x", "the first words"),
        line("e4", "n-x", "other words"),
    ];
    fs::write(&file, lines.join("\n") + "\n").unwrap();
    let notes = fold(&read(&w, &Tier::Personal("slime-s".into())).unwrap().rows);
    let derived: Vec<&String> = notes.keys().filter(|k| k.starts_with('h')).collect();
    assert_eq!(derived.len(), 2, "{:?}", notes.keys());
    assert_eq!(notes["n-x"].text, "the first words");
    let found = mnemorph_memory::findings(&w, "slime-s").unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].event.as_str(), found[0].note.as_str()),
        ("e4", "n-x")
    );
    assert!(found[0].why.contains("collision"));
    let _ = fs::remove_dir_all(&w);
}

/// The worst case, fixed: the weakest grounding — accepted, never corroborated, 900 days old, its
/// confidence at the doubt floor — against an unproven claim at the highest confidence it can
/// reach, both at the same match, relation, salience and age.
#[test]
fn the_weakest_grounded_claim_still_outranks_the_strongest_unproven_one() {
    let now = 1000 * DAY;
    let mut proven = grounded("g", "accepted: log.md #2", 0.3, 1, 900);
    proven.salience = 1.0;
    let mut unproven = proven.clone();
    unproven.id = "u".into();
    unproven.status = Status::Proposed;
    unproven.proof = None;
    unproven.groundings.clear();
    unproven.grounded_at = None;
    unproven.confidence = 0.95;
    let mut notes = vec![(unproven, Relation::Own), (proven, Relation::Own)];
    notes.extend(others());
    let r = recall(
        "tokenizer interval fractions",
        &notes,
        now,
        6,
        &Weights::default(),
    );
    let ids: Vec<&str> = r.anchors.iter().map(|h| h.note.id.as_str()).collect();
    assert_eq!(ids, vec!["g", "u"]);
    assert!(r.anchors[0].score > r.anchors[1].score);
}
