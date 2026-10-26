//! The unproven write: an event whose author or proof does not resolve is ignored by recall, freezes
//! the creature's memory writes and is reported by `memory status`.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use mnemorph_memory::note::{By, Change, Event};
use mnemorph_memory::{Proof, Status, Tier, distill, gather, ground, propose, status};

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-audit-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    w
}

/// A line written by hand into a tier's file, as an edit or a bad merge would.
fn by_hand(w: &Path, tier: &Tier, note: &str, by: By, proof: Option<&str>, change: Change) {
    let e = Event {
        id: format!("hand-{note}-{}", proof.unwrap_or("none").len()),
        time: 9_999_999_999_999,
        note: note.into(),
        by,
        proof: proof.map(str::to_owned),
        change,
    };
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(w.join(tier.file()))
        .unwrap();
    writeln!(f, "{}", serde_json::to_string(&e).unwrap()).unwrap();
}

fn grounded() -> Change {
    Change::Status {
        status: Status::Grounded,
    }
}

#[test]
fn a_hand_grounded_note_is_ignored_and_freezes_its_creature() {
    let w = world("hand");
    let claim = propose(&w, "slime-a", "the lexer is done", vec![], None).unwrap();
    let note = claim.note.clone();
    let personal = Tier::Personal("slime-a".into());
    by_hand(&w, &personal, &note, By::Record, None, grounded());
    // Recall ignores the grounding: the note stays proposed.
    let memory = gather(&w, "slime-a", |_| false).unwrap();
    assert_eq!(memory.unproven, 1);
    let seen = memory.notes.iter().find(|(n, _)| n.id == note).unwrap();
    assert_eq!(seen.0.status, Status::Proposed);
    // Its writes are frozen; another creature's are not.
    let refused = propose(&w, "slime-a", "more", vec![], None).unwrap_err();
    assert!(refused.to_string().contains("frozen"), "{refused}");
    assert!(propose(&w, "slime-b", "fine", vec![], None).is_ok());
    // Status names it, by creature.
    let report = status(&w).unwrap();
    let found = &report.iter().find(|(c, _)| c == "slime-a").unwrap().1;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].why, "a standing claimed by a plain record");
    assert_eq!(
        report.iter().find(|(c, _)| c == "slime-b").unwrap().1,
        vec![]
    );
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn routes_resolve_by_their_proof_and_schemas_by_what_they_rest_on() {
    let w = world("routes");
    let a = ground(
        &w,
        propose(&w, "slime-s", "one", vec![], None).unwrap(),
        &Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    let b = ground(
        &w,
        propose(&w, "slime-s", "two", vec![], None).unwrap(),
        &Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    distill(&w, &Tier::Shared, &[a.clone(), b], "both", Some("slime-s")).unwrap();
    assert!(
        status(&w).unwrap().iter().all(|(_, f)| f.is_empty()),
        "Mnemorph's own writes resolve"
    );
    // A route naming no proof, and a schema resting on a note never grounded, do not.
    by_hand(
        &w,
        &Tier::Shared,
        &a,
        By::Route,
        Some("trust me"),
        grounded(),
    );
    by_hand(
        &w,
        &Tier::Shared,
        &a,
        By::Record,
        Some("distilled: ghost"),
        grounded(),
    );
    let s = status(&w).unwrap();
    let whys: Vec<&str> = s
        .iter()
        .flat_map(|(_, f)| f.iter().map(|u| u.why.as_str()))
        .collect();
    assert!(whys.contains(&"a route that names no proof"), "{whys:?}");
    assert!(
        whys.contains(&"a schema resting on a note this tier never grounded"),
        "{whys:?}"
    );
    let _ = fs::remove_dir_all(&w);
}
