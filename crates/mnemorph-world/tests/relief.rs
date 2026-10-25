//! Relief: a ground split in two young, or a seat retired; the old seat keeps its memory and
//! claims nothing; one journal entry.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_world::Roster;
use mnemorph_world::relief::{Act, relieve};

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-relief-{name}-{}", std::process::id()));
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
        "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** src/, lib/\n",
    );
    w
}

fn doc(w: &Path, race: &str, short: &str, body: &str) {
    let dir = w.join(".mnemorph").join(race).join(short);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.md"), body).unwrap();
}

#[test]
fn a_ground_of_prefixes_splits_in_halves() {
    let w = world("prefixes");
    doc(
        &w,
        "slime",
        "s",
        "# slime-s\r\n\r\n- **Race:** slime\r\n- **Reports to:** orc-o\r\n- **Territory:** src/a/, src/b/, src/c.rs\r\n- **Verify:** `cargo test`\r\n\r\n## Notes\r\nkept as written\r\n",
    );
    fs::create_dir_all(w.join(".mnemorph/slime/s/memory")).unwrap();
    let dry = relieve(&w, "slime-s", false).unwrap();
    assert!(
        !w.join(".mnemorph/slime/s-a").exists(),
        "a plan writes nothing"
    );
    let done = relieve(&w, "slime-s", true).unwrap();
    assert_eq!(dry, done);
    assert_eq!(
        done.act,
        Act::Split {
            young: [
                ("slime-s-a".into(), vec!["src/a/".into(), "src/b/".into()]),
                ("slime-s-b".into(), vec!["src/c.rs".into()]),
            ]
        }
    );
    let roster = Roster::load(&w);
    assert!(roster.creatures["slime-s"].retired);
    let a = &roster.creatures["slime-s-a"];
    assert_eq!(
        (a.reports_to.as_deref(), a.verify.clone()),
        (Some("orc-o"), vec!["cargo test".to_owned()])
    );
    assert!(roster.problems.is_empty(), "{:?}", roster.problems);
    assert!(
        mnemorph_world::findings::check(&roster, &[])
            .iter()
            .all(|f| f.code != "overlap")
    );
    // The old doc gained one line; its memory and prose are untouched.
    let old = fs::read_to_string(w.join(".mnemorph/slime/s/README.md")).unwrap();
    assert!(
        old.contains(
            "- **Verify:** `cargo test`\r\n- **Retired:** yes\r\n\r\n## Notes\r\nkept as written"
        ),
        "{old:?}"
    );
    assert!(w.join(".mnemorph/slime/s/memory").exists());
    let journal = fs::read_to_string(w.join(".mnemorph/log.md")).unwrap();
    assert!(journal.contains("] slime-s — relief"), "{journal}");
    // Relieved once: the retired seat is no living creature to relieve again.
    assert!(relieve(&w, "slime-s", true).is_err());
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn a_one_directory_ground_splits_by_its_tree_and_one_file_retires() {
    let w = world("tree");
    for d in ["lib/x", "lib/y"] {
        fs::create_dir_all(w.join(d)).unwrap();
    }
    fs::write(w.join("lib/z.rs"), "").unwrap();
    fs::write(w.join("lib/.hidden"), "").unwrap();
    doc(
        &w,
        "slime",
        "t",
        "# slime-t\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** lib/\n",
    );
    doc(
        &w,
        "slime",
        "u",
        "# slime-u\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** ./CHANGELOG.md\n",
    );
    let t = relieve(&w, "slime-t", true).unwrap();
    assert_eq!(
        t.act,
        Act::Split {
            young: [
                ("slime-t-a".into(), vec!["lib/x/".into(), "lib/y/".into()]),
                ("slime-t-b".into(), vec!["lib/z.rs".into()]),
            ]
        }
    );
    let u = relieve(&w, "slime-u", true).unwrap();
    assert_eq!(u.act, Act::Retire);
    let roster = Roster::load(&w);
    assert!(roster.creatures["slime-u"].retired);
    assert!(
        relieve(&w, "elf-e", false)
            .unwrap_err()
            .contains("splits Slimes and Orcs")
    );
    let _ = fs::remove_dir_all(&w);
}
