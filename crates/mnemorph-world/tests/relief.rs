//! Relief: a ground split in two young, or a seat retired; the old seat keeps its memory and
//! claims nothing; one journal entry.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_world::Roster;
use mnemorph_world::creatures::{self, Act as Wrote, HUMAN};
use mnemorph_world::relief::{Act, Relief};

/// Each test world's creatures' witness, beside it: tests never write the human's state directory.
fn state_of(w: &Path) -> PathBuf {
    let mut s = w.as_os_str().to_owned();
    s.push("-state");
    PathBuf::from(s)
}

/// Relief on `w`, every card adopted first as the human would: relief runs on the trusted roster.
fn relieve(w: &Path, name: &str, apply: bool) -> Result<Relief, String> {
    let state = state_of(w);
    let held = creatures::witnessed_in(Some(&state), w).unwrap().unwrap();
    let rows = Roster::cards(w)
        .creatures
        .values()
        .filter(|c| c.name != "rimuru" && !held.contains_key(&c.name))
        .map(|c| creatures::row_of(Wrote::Adopt, c, HUMAN))
        .collect();
    creatures::record_in(Some(&state), w, rows).unwrap();
    mnemorph_world::relief::relieve_in(Some(&state), w, name, apply)
}

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-relief-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    let _ = fs::remove_dir_all(state_of(&w));
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
    let roster = Roster::cards(&w);
    assert!(roster.creatures["slime-s"].retired);
    let a = &roster.creatures["slime-s-a"];
    assert_eq!(
        (a.reports_to.as_deref(), a.verify.clone()),
        (Some("orc-o"), vec!["cargo test".to_owned()])
    );
    assert_eq!(a.heir_of.as_deref(), Some("slime-s"));
    assert_eq!(
        roster.creatures["slime-s"].heirs,
        ["slime-s-a", "slime-s-b"]
    );
    assert!(roster.problems.is_empty(), "{:?}", roster.problems);
    assert!(
        mnemorph_world::findings::check(&roster, &[])
            .iter()
            .all(|f| f.code != "overlap")
    );
    // The old doc gained two lines — retired, and its heirs — in its own line endings; its memory
    // and prose are untouched. Each young names the creature whose ground it took.
    let old = fs::read_to_string(w.join(".mnemorph/slime/s/README.md")).unwrap();
    assert!(
        old.contains(
            "- **Verify:** `cargo test`\r\n- **Retired:** yes\r\n- **Heirs:** slime-s-a, slime-s-b\r\n\r\n## Notes\r\nkept as written"
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
                (
                    "slime-t-a".into(),
                    vec!["lib/x/".into(), "lib/y/".into(), "lib/.hidden".into()]
                ),
                ("slime-t-b".into(), vec!["lib/z.rs".into()]),
            ]
        }
    );
    let u = relieve(&w, "slime-u", true).unwrap();
    assert_eq!(u.act, Act::Retire);
    let roster = Roster::cards(&w);
    assert!(roster.creatures["slime-u"].retired);
    assert!(
        relieve(&w, "elf-e", false)
            .unwrap_err()
            .contains("splits Slimes and Orcs")
    );
    let _ = fs::remove_dir_all(&w);
}

/// An Orc's relief relinks its living Slimes to the young that routes their ground.
#[test]
fn an_orc_relief_relinks_its_slimes() {
    let w = world("orc");
    doc(
        &w,
        "slime",
        "l",
        "# slime-l\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** lib/x/\n",
    );
    doc(
        &w,
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/a/\n",
    );
    let done = relieve(&w, "orc-o", true).unwrap();
    // Sorted: lib/ to the first young, src/ to the second.
    assert_eq!(
        done.children,
        vec![
            ("slime-l".into(), "orc-o-a".into()),
            ("slime-s".into(), "orc-o-b".into())
        ]
    );
    let roster = Roster::cards(&w);
    assert_eq!(
        roster.creatures["slime-s"].reports_to.as_deref(),
        Some("orc-o-b")
    );
    assert!(roster.problems.is_empty(), "{:?}", roster.problems);
    let findings = mnemorph_world::findings::check(&roster, &[]);
    assert!(
        findings
            .iter()
            .all(|f| f.code != "dead-bond" && f.code != "overlap"),
        "{findings:?}"
    );
    // The witness learnt it from relief itself: the trusted view agrees with the cards written.
    let trusted = Roster::load_in(Some(&state_of(&w)), &w);
    assert_eq!(
        trusted.creatures["orc-o-a"].race,
        mnemorph_world::races::Race::Orc
    );
    assert_eq!(
        trusted.creatures["slime-s"].reports_to.as_deref(),
        Some("orc-o-b")
    );
    assert!(trusted.creatures["orc-o"].retired);
    assert!(trusted.mismatches.is_empty(), "{:?}", trusted.mismatches);
    let _ = fs::remove_dir_all(&w);
}

/// A root file stays a claim in the young; a seat already taken stops the relief before any write;
/// a lone directory is descended.
#[test]
fn relief_checks_first_and_keeps_every_claim_readable() {
    let w = world("checks");
    doc(
        &w,
        "slime",
        "r",
        "# slime-r\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** ./CHANGELOG.md, ./README.md\n",
    );
    relieve(&w, "slime-r", true).unwrap();
    let roster = Roster::cards(&w);
    assert!(roster.problems.is_empty(), "{:?}", roster.problems);
    assert_eq!(roster.creatures["slime-r-a"].zone, ["CHANGELOG.md"]);

    doc(
        &w,
        "slime",
        "q",
        "# slime-q\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/p/, src/q/\n",
    );
    fs::create_dir_all(w.join(".mnemorph/slime/q-b")).unwrap();
    assert!(relieve(&w, "slime-q", true).is_err());
    assert!(
        !w.join(".mnemorph/slime/q-a").exists(),
        "nothing written before the check failed"
    );
    assert!(!Roster::cards(&w).creatures["slime-q"].retired);

    fs::create_dir_all(w.join("deep/only/x")).unwrap();
    fs::create_dir_all(w.join("deep/only/y")).unwrap();
    doc(
        &w,
        "slime",
        "d",
        "# slime-d\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** deep/\n",
    );
    let d = relieve(&w, "slime-d", false).unwrap();
    assert_eq!(
        d.act,
        Act::Split {
            young: [
                ("slime-d-a".into(), vec!["deep/only/x/".into()]),
                ("slime-d-b".into(), vec!["deep/only/y/".into()]),
            ]
        }
    );
    let _ = fs::remove_dir_all(&w);
}

/// Personal memory survives a split: each heir inherits the notes whose source lies in its half.
#[test]
fn a_split_hands_the_notes_to_the_heir_whose_half_holds_their_source() {
    let w = world("notes");
    doc(
        &w,
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/a/, src/b/, src/c.rs\n",
    );
    for (text, source) in [("of-a", "src/a/x.rs:3"), ("of-c", "src/c.rs")] {
        mnemorph_memory::propose(&w, "slime-s", text, vec![], Some(source.into())).unwrap();
    }
    relieve(&w, "slime-s", true).unwrap();
    let notes_of = |name: &str| {
        let back =
            mnemorph_memory::read(&w, &mnemorph_memory::Tier::Personal(name.into())).unwrap();
        let mut texts: Vec<String> = mnemorph_memory::fold(&back.rows)
            .into_values()
            .map(|n| n.text)
            .collect();
        texts.sort();
        texts
    };
    assert_eq!(notes_of("slime-s-a"), ["of-a"]);
    assert_eq!(notes_of("slime-s-b"), ["of-c"]);
    assert_eq!(
        notes_of("slime-s"),
        ["of-a", "of-c"],
        "the old file is kept whole"
    );
}
