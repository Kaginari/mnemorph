//! `housekeep --heal`: the mechanical repairs, made once, journaled; nothing else touched.

use std::fs;

use mnemorph_world::Roster;
use mnemorph_world::heal::{Fix, heal};

fn doc(world: &std::path::Path, race: &str, short: &str, body: &str) {
    let dir = world.join(".mnemorph").join(race).join(short);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.md"), body).unwrap();
}

#[test]
fn a_heal_repairs_what_is_mechanical_once() {
    let w = std::env::temp_dir().join(format!("mnemorph-heal-{}", std::process::id()));
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
        "x",
        "# orc-x\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Retired:** yes\n",
    );
    doc(
        &w,
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-x\n- **Territory:** src/\n",
    );
    doc(
        &w,
        "orc",
        "o",
        "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-gone\n",
    );
    fs::create_dir_all(w.join(".mnemorph/slime/z/memory")).unwrap();

    // A dry run says what it would do and changes nothing.
    let dry = heal(&w, false).unwrap();
    assert_eq!(dry.fixes.len(), 3, "{:?}", dry.fixes);
    assert!(!w.join(".mnemorph/slime/z/README.md").exists());

    let done = heal(&w, true).unwrap();
    assert!(done.fixes.contains(&Fix::Stub {
        seat: ".mnemorph/slime/z/".into(),
        name: "slime-z".into()
    }));
    assert!(done.fixes.contains(&Fix::Relink {
        body: "slime-s".into(),
        from: "orc-x".into(),
        to: "elf-e".into()
    }));
    assert!(done.fixes.contains(&Fix::Relink {
        body: "orc-o".into(),
        from: "elf-gone".into(),
        to: "rimuru".into()
    }));
    let roster = Roster::load(&w);
    assert!(
        roster.creatures["slime-z"].retired,
        "the stub claims nothing"
    );
    assert_eq!(
        roster.creatures["slime-s"].reports_to.as_deref(),
        Some("elf-e")
    );
    assert_eq!(
        roster.creatures["orc-o"].reports_to.as_deref(),
        Some("rimuru")
    );
    // The rest of a relinked doc is untouched.
    let slime = fs::read_to_string(w.join(".mnemorph/slime/s/README.md")).unwrap();
    assert!(slime.contains("- **Territory:** src/"), "{slime}");
    // Each repair is one journal entry; a second heal finds the roster whole.
    let journal = fs::read_to_string(w.join(".mnemorph/log.md")).unwrap();
    assert_eq!(
        journal.matches("] housekeep — heal").count(),
        3,
        "{journal}"
    );
    let again = heal(&w, true).unwrap();
    assert!(again.fixes.is_empty() && again.left.is_empty(), "{again:?}");
    let _ = fs::remove_dir_all(&w);
}
