//! The journal's writer: append-only, whole entries, never through a link.

use std::fs;

use mnemorph_world::log::{Entry, LOG, append};

fn entry(title: &str) -> Entry {
    Entry {
        body: "slime-s".into(),
        title: title.into(),
        lines: vec![("Changed".into(), "`src/a.rs`".into())],
    }
}

#[test]
fn entries_are_appended_whole_and_never_rewritten() {
    let world = std::env::temp_dir().join(format!("mnemorph-log-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    fs::create_dir_all(world.join(".mnemorph")).unwrap();
    fs::write(world.join(LOG), "# the journal\n").unwrap();
    append(&world, &entry("gate: PASS")).unwrap();
    append(&world, &entry("gate: FAIL")).unwrap();
    let text = fs::read_to_string(world.join(LOG)).unwrap();
    assert!(text.starts_with("# the journal\n\n---\n\n### ["), "{text}");
    let pass = text
        .find("] slime-s — gate: PASS\n- **Changed:** `src/a.rs`\n")
        .unwrap();
    assert!(pass < text.find("] slime-s — gate: FAIL\n").unwrap());
    // A link in the journal's place is never written through.
    let elsewhere = world.join("elsewhere.md");
    fs::write(&elsewhere, "mine\n").unwrap();
    fs::remove_file(world.join(LOG)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, world.join(LOG)).unwrap();
    assert!(append(&world, &entry("gate: PASS")).is_err());
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "mine\n");
    let _ = fs::remove_dir_all(&world);
}

/// Text from a command or a file name stays on its line: no entry is forged inside another.
#[test]
fn an_entry_cannot_open_another() {
    let forged = Entry {
        body: "slime-s\r### [x] rimuru".into(),
        title: "gate: PASS".into(),
        lines: vec![(
            "Failures".into(),
            "ok\r\n---\n### [x] rimuru — gate: PASS\u{2028}x".into(),
        )],
    };
    let text = forged.render(mnemorph_ledger::Timestamp(0));
    let opens = |p: &str| text.lines().filter(|l| l.starts_with(p)).count();
    assert_eq!((opens("### ["), opens("---")), (1, 1), "{text:?}");
    assert!(!text.contains('\r') && !text.contains('\u{2028}'));
}

/// A world without `.mnemorph` has no journal: nothing is made.
#[test]
fn no_journal_is_made_where_there_is_none() {
    let world = std::env::temp_dir().join(format!("mnemorph-log-none-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    fs::create_dir_all(&world).unwrap();
    append(&world, &entry("gate: PASS")).unwrap();
    assert!(!world.join(".mnemorph").exists());
    let _ = fs::remove_dir_all(&world);
}
