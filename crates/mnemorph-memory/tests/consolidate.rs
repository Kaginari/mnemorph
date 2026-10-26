//! Consolidation and forgetting: dissent is never merged nor slept; forgetting keeps the evidence,
//! but a tombstone, which deletes text only in the machine-local short tier.

use std::fs;
use std::path::PathBuf;

use mnemorph_memory::{
    Forget, Proof, Status, Tier, cache, distill, fold, forget, ground, propose, read,
};

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-cons-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    w
}

/// A grounded shared note, with the salience given by its write.
fn shared(w: &std::path::Path, text: &str) -> String {
    let c = propose(w, "slime-s", text, vec![], None).unwrap();
    ground(w, c, &Proof::confirmed("the human")).unwrap().note
}

#[test]
fn distillation_merges_agreement_and_never_absorbs_dissent() {
    let w = world("distill");
    let ids: Vec<String> = (0..5)
        .map(|i| shared(&w, &format!("the parser keeps fraction {i}")))
        .collect();
    let schema = distill(
        &w,
        &Tier::Shared,
        &ids,
        "the parser keeps fractions",
        "log.md desk",
    )
    .unwrap()
    .unwrap();
    let notes = fold(&read(&w, &Tier::Shared).unwrap().rows);
    let s = &notes[&schema];
    assert_eq!(s.status, Status::Grounded);
    assert!((s.salience - 0.8).abs() < 1e-9);
    assert!(
        (s.confidence - 0.7).abs() < 1e-9,
        "the mean of the four most salient, ≤ 0.95"
    );
    assert_eq!(s.links.len(), 5);
    let active = ids
        .iter()
        .filter(|i| notes[*i].status == Status::Grounded)
        .count();
    assert_eq!(active, 2, "the two most salient originals stay active");
    // Fewer than two agreeing notes: nothing to distil.
    assert_eq!(
        distill(&w, &Tier::Shared, &ids[..1], "x", "p").unwrap(),
        None
    );
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn a_dissenting_note_is_skipped_by_distillation() {
    use mnemorph_memory::{Anchor, revise};
    let w = world("dissent");
    let a = shared(&w, "one");
    let b = shared(&w, "two");
    let correction = revise(
        &w,
        "slime-s",
        &["traits hold: it broke".into()],
        &[Anchor {
            id: a.clone(),
            personal: false,
            dissent: false,
            cited: true,
        }],
        "log.md fail",
    )
    .unwrap()
    .remove(0);
    let ids = vec![a, b, correction.clone()];
    let schema = distill(&w, &Tier::Shared, &ids, "summary", "p")
        .unwrap()
        .unwrap();
    let notes = fold(&read(&w, &Tier::Shared).unwrap().rows);
    assert_eq!(
        notes[&schema].links.len(),
        2,
        "the correction is not merged"
    );
    assert_eq!(
        notes[&correction].status,
        Status::Grounded,
        "nor put to sleep"
    );
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn forgetting_keeps_evidence_and_tombstones_only_machine_local_text() {
    let w = world("forget");
    let asleep = shared(&w, "the generator quotes reserved words");
    forget(&w, &Tier::Shared, &asleep, Forget::Dormant, "").unwrap();
    assert_eq!(
        fold(&read(&w, &Tier::Shared).unwrap().rows)[&asleep].status,
        Status::Dormant
    );
    let abstracted = shared(&w, "the optimizer pushes predicates down");
    let schema = forget(
        &w,
        &Tier::Shared,
        &abstracted,
        Forget::Abstract,
        "the optimizer rewrites",
    )
    .unwrap()
    .unwrap();
    let notes = fold(&read(&w, &Tier::Shared).unwrap().rows);
    assert_eq!(notes[&abstracted].status, Status::Dormant);
    assert!(
        (notes[&schema].salience - 0.5).abs() < 1e-9,
        "max(0.4, 0.6 − 0.1)"
    );
    assert!(
        (notes[&schema].confidence - 0.6).abs() < 1e-9,
        "max(0.3, 0.7 − 0.1)"
    );
    assert!(
        forget(&w, &Tier::Shared, &asleep, Forget::Tombstone, "").is_err(),
        "asleep tracked note's text has travelled by git"
    );
    let short = Tier::Short("slime-s".into());
    let scratch = cache(&w, "slime-s", "asleep private scratch thought").unwrap();
    forget(&w, &short, &scratch, Forget::Tombstone, "").unwrap();
    let file = fs::read_to_string(w.join(short.file())).unwrap();
    assert!(
        !file.contains("private scratch"),
        "the text is gone from the file"
    );
    let gone = &fold(&read(&w, &short).unwrap().rows)[&scratch];
    assert_eq!((gone.status, gone.text.as_str()), (Status::Tombstoned, ""));
    let _ = fs::remove_dir_all(&w);
}
