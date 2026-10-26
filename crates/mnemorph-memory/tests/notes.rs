//! Notes as events: the fold never trusts line order, the store writes one synced line and reads
//! tolerantly, a split hands each heir its notes, and git merges note files by union.

use std::fs;
use std::path::PathBuf;

use mnemorph_memory::{
    By, Change, Event, Polarity, Status, Tier, append, fold, inherit, read, union_merge,
};

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-notes-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    w
}

fn create(note: &str, time: u64, source: Option<&str>) -> Event {
    Event {
        id: format!("{time}-c"),
        time,
        note: note.into(),
        by: By::Record,
        proof: None,
        change: Change::Create {
            text: format!("{note} holds"),
            tags: vec![],
            owner: Some("slime-s".into()),
            polarity: Polarity::Support,
            salience: 0.5,
            confidence: 1.4,
            source: source.map(str::to_owned),
        },
    }
}

fn status(note: &str, time: u64, id: &str, status: Status) -> Event {
    Event {
        id: id.into(),
        time,
        note: note.into(),
        by: By::Route,
        proof: Some(format!("log.md#{id}")),
        change: Change::Status { status },
    }
}

#[test]
fn the_fold_orders_by_time_and_id_and_the_latest_status_stands() {
    let events = vec![
        status("n", 30, "c", Status::Dormant),
        create("n", 10, None),
        status("n", 20, "b", Status::Grounded),
        // About a note never created: set aside.
        status("ghost", 15, "g", Status::Grounded),
        // A second creation: ignored.
        create("n", 40, Some("src/other.rs")),
    ];
    let mut reversed = events.clone();
    reversed.reverse();
    for evs in [events, reversed] {
        let notes = fold(&evs);
        let n = &notes["n"];
        assert_eq!(
            n.status,
            Status::Dormant,
            "the latest event, a demotion, stands"
        );
        assert_eq!(n.proof.as_deref(), Some("log.md#c"));
        assert!(
            (n.confidence - 1.0).abs() < f64::EPSILON,
            "clamped to [0, 1]"
        );
        assert_eq!(n.source, None, "the first creation stands");
        assert!(!notes.contains_key("ghost"));
    }
}

#[test]
fn only_a_merges_duplicate_takes_the_stronger_status() {
    // The same time and id twice — one machine's grounded, another's tombstone — the stronger.
    let a = vec![
        create("n", 10, None),
        status("n", 20, "same", Status::Tombstoned),
        status("n", 20, "same", Status::Grounded),
    ];
    assert_eq!(fold(&a)["n"].status, Status::Tombstoned);
    // A later grounding after a tombstone at another time is simply the latest.
    let b = vec![
        create("n", 10, None),
        status("n", 20, "x", Status::Tombstoned),
        status("n", 30, "y", Status::Grounded),
    ];
    assert_eq!(fold(&b)["n"].status, Status::Grounded);
    assert!(Polarity::Counter.dissents() && Polarity::Revise.dissents());
    assert!(!Polarity::Support.dissents());
}

#[test]
fn the_store_appends_whole_lines_and_reads_tolerantly() {
    let w = world("store");
    let tier = Tier::Personal("slime-s".into());
    assert_eq!(
        tier.file(),
        PathBuf::from(".mnemorph/memory/creature/slime-s.jsonl")
    );
    assert_eq!(
        Tier::Shared.file(),
        PathBuf::from(".mnemorph/memory/shared/notes.jsonl")
    );
    let made = append(
        &w,
        &tier,
        "n1",
        By::Record,
        None,
        create("n1", 1, None).change,
    )
    .unwrap();
    // A torn line, then another event: the torn one is skipped and counted.
    let path = w.join(tier.file());
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("{\"id\": \"torn");
    fs::write(&path, text).unwrap();
    append(
        &w,
        &tier,
        "n1",
        By::Gate,
        Some("verdict".into()),
        Change::Status {
            status: Status::Grounded,
        },
    )
    .unwrap();
    let back = read(&w, &tier).unwrap();
    assert_eq!(back.rows.len(), 2);
    assert_eq!(back.skipped.len(), 1);
    assert_eq!(back.rows[0], made);
    assert_eq!(fold(&back.rows)["n1"].status, Status::Grounded);
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn a_split_hands_each_heir_the_notes_whose_source_is_in_its_ground() {
    let w = world("inherit");
    let journal = Tier::Personal("slime-s".into()).journal(&w);
    for e in [
        create("in-a", 1, Some("src/a/x.rs:12")),
        create("in-b", 2, Some("src/b/y.rs")),
        create("anywhere", 3, Some("log.md#2026")),
        create("journal", 5, Some(".mnemorph/log.md#4")),
        create("at-root", 6, Some("README.md:3")),
        status("in-a", 4, "p", Status::Grounded),
    ] {
        journal.append(&e).unwrap();
    }
    let given = inherit(
        &w,
        "slime-s",
        &[
            (
                "slime-s-a".into(),
                vec!["src/a/".into(), "./README.md".into()],
            ),
            ("slime-s-b".into(), vec!["src/b/".into()]),
        ],
    )
    .unwrap();
    assert_eq!(
        given,
        [("slime-s-a".to_owned(), 5), ("slime-s-b".to_owned(), 3)]
    );
    let a = fold(&read(&w, &Tier::Personal("slime-s-a".into())).unwrap().rows);
    let b = fold(&read(&w, &Tier::Personal("slime-s-b".into())).unwrap().rows);
    // A source no heir covers — a journal entry — goes to both; a root file only to its keeper.
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        ["anywhere", "at-root", "in-a", "journal"]
    );
    assert_eq!(
        a["in-a"].status,
        Status::Grounded,
        "its standing travels with it"
    );
    assert_eq!(
        b.keys().collect::<Vec<_>>(),
        ["anywhere", "in-b", "journal"]
    );
    // The old creature's file is kept whole.
    assert_eq!(
        read(&w, &Tier::Personal("slime-s".into()))
            .unwrap()
            .rows
            .len(),
        6
    );
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn git_merges_note_files_by_union_through_its_local_attributes() {
    let w = world("union");
    union_merge(&w).unwrap();
    assert!(!w.join(".git").exists(), "no git: nothing");
    let git = |dir: &std::path::Path, args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap()
                .status
                .success()
        );
    };
    git(&w, &["init", "-q"]);
    fs::write(w.join(".git/info/attributes"), "*.png binary").unwrap();
    union_merge(&w).unwrap();
    union_merge(&w).unwrap();
    let attributes = fs::read_to_string(w.join(".git/info/attributes")).unwrap();
    assert_eq!(
        attributes,
        "*.png binary\n.mnemorph/memory/**/*.jsonl merge=union\n"
    );
    // A worktree's `.git` is a file: git names the attributes it shares, and they hold.
    fs::write(w.join("a.txt"), "a\n").unwrap();
    git(&w, &["add", "-A"]);
    git(&w, &["commit", "-qm", "a"]);
    let tree = w.with_file_name(format!("{}-tree", w.file_name().unwrap().to_string_lossy()));
    let _ = fs::remove_dir_all(&tree);
    git(&w, &["worktree", "add", "-q", tree.to_str().unwrap()]);
    union_merge(&tree).unwrap();
    let out = std::process::Command::new("git")
        .args(["check-attr", "merge", ".mnemorph/memory/creature/x.jsonl"])
        .current_dir(&tree)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).ends_with("merge: union\n"),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let _ = fs::remove_dir_all(&tree);
    let _ = fs::remove_dir_all(&w);
}

/// Events the order cannot separate fold the same whatever the line order, and two ids drawn
/// here never collide.
#[test]
fn colliding_events_fold_the_same_in_any_order() {
    let mut a = create("n", 10, None);
    let mut b = create("n", 10, None);
    if let Change::Create { text, .. } = &mut a.change {
        *text = "one".into();
    }
    if let Change::Create { text, .. } = &mut b.change {
        *text = "two".into();
    }
    let conf = |c: f64| Event {
        id: "same".into(),
        time: 20,
        note: "n".into(),
        by: By::Gate,
        proof: None,
        change: Change::Confidence { confidence: c },
    };
    let one = vec![a.clone(), b.clone(), conf(0.2), conf(0.9)];
    let two = vec![conf(0.9), b, conf(0.2), a];
    assert_eq!(fold(&one), fold(&two));
    assert_ne!(
        mnemorph_memory::store::event_id(5),
        mnemorph_memory::store::event_id(5)
    );
}
