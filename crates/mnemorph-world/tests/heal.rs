//! `housekeep --heal`: the mechanical repairs, made once, journaled, within the race law; nothing
//! else touched, nothing written through a link.

use std::fs;

use mnemorph_world::Roster;
use mnemorph_world::heal::{Fix, heal};

fn doc(world: &std::path::Path, race: &str, short: &str, body: &str) {
    let dir = world.join(".mnemorph").join(race).join(short);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.md"), body).unwrap();
}

/// A throne, an Elf, two living Orcs and retired ones, Slimes with broken links, an Orc whose Elf
/// is unknown, and a seat with no doc.
fn fixture(w: &std::path::Path) {
    doc(w, "rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n");
    doc(
        w,
        "elf",
        "e",
        "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n",
    );
    doc(
        w,
        "orc",
        "k",
        "# orc-k\n\n- **Race:** orc\n- **Reports to:** elf-e\n",
    );
    // A retired Orc between: its Slime goes to the next living Orc above.
    doc(
        w,
        "orc",
        "x",
        "# orc-x\n\n- **Race:** orc\n- **Reports to:** orc-k\n- **Retired:** yes\n",
    );
    doc(
        w,
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-x\n- **Territory:** src/\n",
    );
    // An Orc reporting to the retired Orc, in CRLF, with a field-looking line in its prose.
    doc(
        w,
        "orc",
        "o",
        "# orc-o\r\n\r\n- **Race:** orc\r\n- **Reports to:** orc-x\r\n\r\n## Notes\r\n- **Reports to:** prose, not a field",
    );
    // A Slime whose Orc is gone, with no living Orc above it: an Elf may not command it.
    doc(
        w,
        "orc",
        "y",
        "# orc-y\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Retired:** yes\n",
    );
    doc(
        w,
        "slime",
        "t",
        "# slime-t\n\n- **Race:** slime\n- **Reports to:** orc-y\n",
    );
    // An Orc whose Elf is unknown: the throne commands no Orc, so it is left.
    doc(
        w,
        "orc",
        "q",
        "# orc-q\n\n- **Race:** orc\n- **Reports to:** elf-gone\n",
    );
    fs::create_dir_all(w.join(".mnemorph/slime/z/memory")).unwrap();
}

#[test]
fn a_heal_repairs_what_is_mechanical_once() {
    let w = std::env::temp_dir().join(format!("mnemorph-heal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fixture(&w);

    // A dry run says what it would do and changes nothing.
    let dry = heal(&w, false).unwrap();
    assert_eq!(dry.fixes.len(), 3, "{:?}", dry.fixes);
    assert!(!w.join(".mnemorph/slime/z/README.md").exists());

    let done = heal(&w, true).unwrap();
    for fix in [
        Fix::Stub {
            seat: ".mnemorph/slime/z/".into(),
            name: "slime-z".into(),
        },
        Fix::Relink {
            body: "slime-s".into(),
            from: "orc-x".into(),
            to: "orc-k".into(),
        },
        Fix::Relink {
            body: "orc-o".into(),
            from: "orc-x".into(),
            to: "elf-e".into(),
        },
    ] {
        assert!(done.fixes.contains(&fix), "{fix:?} in {:?}", done.fixes);
    }
    assert!(
        done.left
            .iter()
            .any(|l| l.contains("slime-t reports to orc-y") && l.contains("no living orc")),
        "{:?}",
        done.left
    );
    let roster = Roster::cards(&w);
    assert!(
        roster.creatures["slime-z"].retired,
        "the stub claims nothing"
    );
    assert_eq!(
        roster.creatures["slime-s"].reports_to.as_deref(),
        Some("orc-k")
    );
    // Only the field's line changed: CRLF kept, the prose untouched.
    let orc = fs::read_to_string(w.join(".mnemorph/orc/o/README.md")).unwrap();
    assert_eq!(
        orc,
        "# orc-o\r\n\r\n- **Race:** orc\r\n- **Reports to:** elf-e\r\n\r\n## Notes\r\n- **Reports to:** prose, not a field"
    );
    // Each repair is one journal entry; a second heal repairs nothing more.
    let journal = fs::read_to_string(w.join(".mnemorph/log.md")).unwrap();
    assert_eq!(
        journal.matches("] housekeep — heal").count(),
        3,
        "{journal}"
    );
    let again = heal(&w, true).unwrap();
    assert!(again.fixes.is_empty(), "{again:?}");
    let _ = fs::remove_dir_all(&w);
}

/// A seat that is a link, or holds one, is never written through.
#[test]
fn a_heal_never_writes_through_a_link() {
    let w = std::env::temp_dir().join(format!("mnemorph-heal-link-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    let outside = w.with_file_name(format!("mnemorph-heal-outside-{}", std::process::id()));
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).unwrap();
    doc(&w, "rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n");
    fs::create_dir_all(w.join(".mnemorph/slime")).unwrap();
    std::os::unix::fs::symlink(&outside, w.join(".mnemorph/slime/z")).unwrap();
    let done = heal(&w, true).unwrap();
    assert!(done.fixes.is_empty(), "{:?}", done.fixes);
    assert!(
        done.left.iter().any(|l| l.contains("link")),
        "{:?}",
        done.left
    );
    assert!(!outside.join("README.md").exists());
    let _ = fs::remove_dir_all(&w);
    let _ = fs::remove_dir_all(&outside);
}
