//! Cards, worth and the ceiling of a grant: a body's record counts with every body below it.

use std::fs;

use mnemorph_ledger::{Journal, UsageRow};
use mnemorph_races::Race;
use mnemorph_world::Roster;
use mnemorph_world::log::{Entry, append};
use mnemorph_world::worth::{Card, USAGE, card, ceiling};

fn doc(world: &std::path::Path, race: &str, short: &str, body: &str) {
    let dir = world.join(".mnemorph").join(race).join(short);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.md"), body).unwrap();
}

fn verdict(world: &std::path::Path, body: &str, said: &str) {
    let entry = Entry {
        body: body.into(),
        title: format!("gate: {said}"),
        lines: vec![("By".into(), "the end gate".into())],
    };
    append(world, &entry).unwrap();
}

#[test]
fn a_card_counts_the_body_and_every_body_below_it() {
    let w = std::env::temp_dir().join(format!("mnemorph-worth-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    doc(&w, "rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n");
    doc(
        &w,
        "elf",
        "e",
        "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n",
    );
    doc(
        &w,
        "orc",
        "o",
        "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n",
    );
    doc(
        &w,
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n",
    );
    doc(
        &w,
        "slime",
        "t",
        "# slime-t\n\n- **Race:** slime\n- **Reports to:** orc-o\n",
    );
    let roster = Roster::load(&w);
    // No record yet: worth 0.75.
    assert_eq!(card(&w, &roster, "slime-s").worth_milli(), 750);
    verdict(&w, "slime-s", "PASS");
    verdict(&w, "slime-s", "PASS");
    verdict(&w, "slime-t", "FAIL");
    verdict(&w, "slime-t", "FAIL, handed back");
    verdict(&w, "orc-o", "PASS");
    // An entry the gate did not write counts for nothing.
    let hand = Entry {
        body: "slime-t".into(),
        title: "gate: PASS".into(),
        lines: Vec::new(),
    };
    append(&w, &hand).unwrap();
    let row = |body: &str, wt: u64| UsageRow {
        ts: mnemorph_ledger::Timestamp(0),
        session: "s".into(),
        run: "r".into(),
        body: body.into(),
        race: "slime".into(),
        commission: None,
        model: "m".into(),
        provider: "p".into(),
        usage: mnemorph_ledger::Usage::default(),
        prefix: 0,
        wt,
        usd: None,
        reported_cost: None,
        estimated: false,
        envelope: None,
        purpose: None,
    };
    let ledger = Journal::<UsageRow>::new(w.join(USAGE));
    ledger.append(&row("slime-s", 100)).unwrap();
    ledger.append(&row("slime-t", 50)).unwrap();
    ledger.append(&row("elf-e", 7)).unwrap();
    let s = card(&w, &roster, "slime-s");
    assert_eq!((s.passes, s.fails, s.spent_wt), (2, 0, 100));
    let t = card(&w, &roster, "slime-t");
    assert_eq!(
        (t.passes, t.fails, t.handed_back),
        (0, 1, 1),
        "the hand entry is not counted"
    );
    // The Orc answers for its Slimes: their records are in its card.
    let o = card(&w, &roster, "orc-o");
    assert_eq!((o.passes, o.fails, o.spent_wt), (3, 1, 150));
    assert!(s.worth_milli() > o.worth_milli() && o.worth_milli() > t.worth_milli());
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn the_ceiling_keeps_the_reserve_the_share_and_the_worth() {
    let fresh = Card {
        handed_back: 0,
        body: "slime-s".into(),
        spent_wt: 0,
        passes: 0,
        fails: 0,
    };
    // 1000 left: 90% kept for grants, a Slime's 70%, worth 0.75.
    assert_eq!(ceiling(1000, Race::Slime, &fresh), 472);
    let failing = Card {
        fails: 8,
        ..fresh.clone()
    };
    assert!(ceiling(1000, Race::Slime, &failing) < 472);
    // A chain of grants shrinks: each generation gets at most a share of what its giver had.
    let mut left = 1_000_000;
    for race in [Race::Elf, Race::Orc, Race::Slime] {
        let next = ceiling(left, race, &fresh);
        assert!(next * 2 < left, "{race:?}: {next} of {left}");
        left = next;
    }
}

/// Three failed runs in a row on the same files make a body stuck on them; a pass clears it.
#[test]
fn three_fails_on_the_same_files_make_a_body_stuck() {
    let w = std::env::temp_dir().join(format!("mnemorph-stuck-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join(".mnemorph")).unwrap();
    let fail = |changed: &str| {
        let entry = Entry {
            body: "slime-s".into(),
            title: "gate: FAIL".into(),
            lines: vec![
                ("Changed".into(), changed.into()),
                ("By".into(), "the end gate".into()),
            ],
        };
        append(&w, &entry).unwrap();
    };
    fail("`src/a.rs`, `src/b.rs`");
    fail("`src/a.rs`");
    assert!(
        mnemorph_world::worth::Records::load(&w)
            .stuck("slime-s")
            .is_empty(),
        "two is not three"
    );
    fail("`src/a.rs`, `src/c.rs`");
    assert_eq!(
        mnemorph_world::worth::Records::load(&w).stuck("slime-s"),
        ["src/a.rs"]
    );
    verdict(&w, "slime-s", "PASS");
    assert!(
        mnemorph_world::worth::Records::load(&w)
            .stuck("slime-s")
            .is_empty(),
        "a pass clears it"
    );
    let _ = fs::remove_dir_all(&w);
}

/// An order names a path as a word of its own, never as part of another.
#[test]
fn an_order_names_a_path_as_a_word() {
    use mnemorph_world::worth::names;
    assert!(names("fix `./src/a.rs:12` again", "src/a.rs"));
    assert!(!names("fix src/a.rsx", "src/a.rs"));
    assert!(!names("fix tests/src/a.rs", "src/a.rs"));
}

/// A changed path holding ", " is read whole.
#[test]
fn a_path_with_a_comma_is_read_whole() {
    let w = std::env::temp_dir().join(format!("mnemorph-comma-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join(".mnemorph")).unwrap();
    for _ in 0..3 {
        let entry = Entry {
            body: "slime-s".into(),
            title: "gate: FAIL".into(),
            lines: vec![
                ("Changed".into(), "`docs/a, b.md`, `src/c.rs`".into()),
                ("By".into(), "the end gate".into()),
            ],
        };
        append(&w, &entry).unwrap();
    }
    assert_eq!(
        mnemorph_world::worth::Records::load(&w).stuck("slime-s"),
        ["docs/a, b.md", "src/c.rs"]
    );
    let _ = fs::remove_dir_all(&w);
}

/// Not verifying never buys a body anything: a `NOT_VERIFIED` counts as a fail on its card and in
/// its streak, and one handed back is skipped like a failure handed back. One the machine would not
/// run — `NOT_VERIFIED, unrun` — counts neither way.
#[test]
fn a_check_that_gave_no_result_counts_as_a_fail() {
    let w = std::env::temp_dir().join(format!("mnemorph-unverified-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join(".mnemorph")).unwrap();
    doc(
        &w,
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Territory:** src/\n",
    );
    let said = |word: &str| {
        let entry = Entry {
            body: "slime-s".into(),
            title: format!("gate: {word}"),
            lines: vec![
                ("Changed".into(), "`src/a.rs`".into()),
                ("By".into(), "the end gate".into()),
            ],
        };
        append(&w, &entry).unwrap();
    };
    said("FAIL");
    said("NOT_VERIFIED, handed back");
    said("NOT_VERIFIED");
    // A check the machine would not run is no mark either way, and breaks no streak.
    said("NOT_VERIFIED, unrun");
    said("FAIL");
    said("NOT_VERIFIED, unrun");
    let records = mnemorph_world::worth::Records::load(&w);
    assert_eq!(records.stuck("slime-s"), ["src/a.rs"]);
    let card = records.card(&mnemorph_world::Roster::load(&w), "slime-s");
    assert_eq!((card.passes, card.fails, card.handed_back), (0, 3, 1));
    let _ = fs::remove_dir_all(&w);
}
