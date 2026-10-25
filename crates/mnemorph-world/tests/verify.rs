//! An Orc codes its Slime's verify line through the harness: only its own Slimes, one clean line,
//! never through a link, journaled.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_world::Roster;
use mnemorph_world::verify::code;

fn doc(w: &Path, race: &str, short: &str, body: &str) {
    let dir = w.join(".mnemorph").join(race).join(short);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.md"), body).unwrap();
}

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-verify-{name}-{}", std::process::id()));
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
        "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** src/\n",
    );
    doc(
        &w,
        "orc",
        "p",
        "# orc-p\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** lib/\n",
    );
    doc(
        &w,
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n\n## Notes\nkept\n",
    );
    doc(
        &w,
        "slime",
        "t",
        "# slime-t\r\n\r\n- **Race:** slime\r\n- **Reports to:** orc-o\r\n- **Verify:** `old`\r\n- **Territory:** src/b/\r\n",
    );
    w
}

#[test]
fn an_orc_codes_its_own_slimes_verify_line() {
    let w = world("ok");
    code(&w, "orc-o", "slime-s", "python3 -m unittest -q").unwrap();
    let s = fs::read_to_string(w.join(".mnemorph/slime/s/README.md")).unwrap();
    assert!(
        s.contains(
            "- **Territory:** src/\n- **Verify:** `python3 -m unittest -q`\n\n## Notes\nkept\n"
        ),
        "{s}"
    );
    // A line someone else set stays: the Orc's is added, never in its place, and only once.
    code(&w, "orc-o", "slime-t", "true").unwrap();
    code(&w, "orc-o", "slime-t", "true").unwrap();
    let t = fs::read_to_string(w.join(".mnemorph/slime/t/README.md")).unwrap();
    assert_eq!(
        t,
        "# slime-t\r\n\r\n- **Race:** slime\r\n- **Reports to:** orc-o\r\n- **Verify:** `old`\r\n- **Territory:** src/b/\r\n- **Verify:** `true`\r\n"
    );
    let roster = Roster::load(&w);
    assert_eq!(
        roster.creatures["slime-s"].verify,
        ["python3 -m unittest -q"]
    );
    let journal = fs::read_to_string(w.join(".mnemorph/log.md")).unwrap();
    assert!(journal.contains("] orc-o — verify"), "{journal}");
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn only_its_own_orc_with_one_clean_line_never_through_a_link() {
    let w = world("refused");
    for (by, to, cmd, why) in [
        ("orc-p", "slime-s", "true", "does not report to orc-p"),
        ("elf-e", "slime-s", "true", "no living Orc"),
        ("orc-o", "orc-p", "true", "no living Slime"),
        ("orc-o", "slime-s", "true\nrm -rf /", "one line"),
        ("orc-o", "slime-s", "echo `id`", "a backtick"),
        ("orc-o", "slime-s", "true\u{1b}[2J", "control character"),
        ("orc-o", "slime-s", "  ", "1 to 300"),
    ] {
        let e = code(&w, by, to, cmd).unwrap_err();
        assert!(e.contains(why), "{by} {to} {cmd:?}: {e}");
    }
    let outside = w.with_file_name(format!("mnemorph-verify-outside-{}", std::process::id()));
    fs::write(
        &outside,
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n",
    )
    .unwrap();
    fs::remove_file(w.join(".mnemorph/slime/s/README.md")).unwrap();
    std::os::unix::fs::symlink(&outside, w.join(".mnemorph/slime/s/README.md")).unwrap();
    assert!(
        code(&w, "orc-o", "slime-s", "true")
            .unwrap_err()
            .contains("a link")
    );
    assert!(!fs::read_to_string(&outside).unwrap().contains("Verify"));
    let _ = fs::remove_file(&outside);
    let _ = fs::remove_dir_all(&w);
}
