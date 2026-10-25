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
