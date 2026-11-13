//! An Orc codes its Slime's verify line through the harness: only its own Slimes, one clean line,
//! never through a link, journaled.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_world::Roster;
use mnemorph_world::creatures::{self, Act, HUMAN};

/// Each test world's creatures' witness, beside it: tests never write the human's state directory.
fn state_of(w: &Path) -> PathBuf {
    let mut s = w.as_os_str().to_owned();
    s.push("-state");
    PathBuf::from(s)
}

/// The Orc's code on `w`, every card adopted first as the human would.
fn code(
    w: &Path,
    orc: &str,
    slime: &str,
    command: &str,
) -> Result<mnemorph_world::verify::Coded, String> {
    let state = state_of(w);
    let held = creatures::witnessed_in(Some(&state), w).unwrap().unwrap();
    let rows = Roster::cards(w)
        .creatures
        .values()
        .filter(|c| c.name != "summoner" && !held.contains_key(&c.name))
        .map(|c| creatures::row_of(Act::Adopt, c, HUMAN))
        .collect();
    creatures::record_in(Some(&state), w, rows).unwrap();
    mnemorph_world::verify::code_in(Some(&state), w, orc, slime, command)
}

/// A stranger is no Orc, whatever its card says: it codes no line until the human adopts it.
#[test]
fn a_stranger_orc_codes_nothing() {
    let w = world("stranger");
    let e = mnemorph_world::verify::code_in(Some(&state_of(&w)), &w, "orc-o", "slime-s", "true")
        .unwrap_err();
    assert!(e.contains("no living Orc"), "{e}");
    assert!(code(&w, "orc-o", "slime-s", "true").is_ok());
    let _ = fs::remove_dir_all(&w);
    let _ = fs::remove_dir_all(state_of(&w));
}

fn doc(w: &Path, race: &str, short: &str, body: &str) {
    let dir = w.join(".mnemorph").join(race).join(short);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.md"), body).unwrap();
}

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-verify-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    let _ = fs::remove_dir_all(state_of(&w));
    doc(
        &w,
        "summoner",
        "summoner",
        "# summoner\n\n- **Race:** summoner\n",
    );
    doc(
        &w,
        "elf",
        "e",
        "# elf-e\n\n- **Race:** elf\n- **Reports to:** summoner\n",
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
    let roster = Roster::cards(&w);
    assert_eq!(
        roster.creatures["slime-s"].verify,
        ["python3 -m unittest -q"]
    );
    let journal = fs::read_to_string(w.join(".mnemorph/log.md")).unwrap();
    assert!(journal.contains("] orc-o — verify"), "{journal}");
    let _ = fs::remove_dir_all(&w);
}

/// The lines `creature`'s gate may run, as the pins hold them: the command and who pinned it.
fn pinned(w: &Path, creature: &str) -> Vec<(String, String)> {
    mnemorph_world::pins::pinned(w)
        .unwrap()
        .unwrap()
        .get(creature)
        .map(|lines| {
            lines
                .iter()
                .map(|p| (p.line.clone(), p.by.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// An Orc that codes a line again for a `Done when` name replaces the line it coded for that name
/// before. A coded line was only ever added, so one that could not run (a quoting slip, the wrong
/// folder) or that asked a Slime for another zone's work stayed and failed every later landing:
/// in one kept chain run all 14 failed Slime landings failed on such a line, 7 of each kind,
/// though the Orcs had coded better lines for the same names since. One coded line a name; a
/// line with no name, another name's line and the human's pinned line all stay.
#[test]
fn a_line_coded_again_for_a_name_replaces_the_orcs_earlier_one() {
    let w = world("recode");
    let card = |short: &str| {
        fs::read_to_string(w.join(format!(".mnemorph/slime/{short}/README.md"))).unwrap()
    };
    // A line that cannot run, then the same name coded again: the first is gone.
    let first = code(
        &w,
        "orc-o",
        "slime-s",
        "cd nowhere && python3 -m unittest -q # D3",
    )
    .unwrap();
    assert!(first.replaced.is_empty(), "{:?}", first.replaced);
    let again = code(&w, "orc-o", "slime-s", "python3 -m unittest -q # D3").unwrap();
    assert_eq!(again.line, "- **Verify:** `python3 -m unittest -q # D3`");
    assert_eq!(
        again.replaced,
        ["cd nowhere && python3 -m unittest -q # D3"]
    );
    assert_eq!(
        Roster::cards(&w).creatures["slime-s"].verify,
        ["python3 -m unittest -q # D3"],
        "{}",
        card("s")
    );
    assert!(
        card("s").ends_with("\n\n## Notes\nkept\n"),
        "the rest of the doc stands: {}",
        card("s")
    );
    assert_eq!(
        pinned(&w, "slime-s"),
        [("python3 -m unittest -q # D3".to_owned(), "orc-o".to_owned())],
        "the gate runs the new line alone"
    );
    // Another name's line and a line with no name are added beside it.
    code(&w, "orc-o", "slime-s", "true # D4").unwrap();
    code(&w, "orc-o", "slime-s", "python3 -c pass").unwrap();
    code(&w, "orc-o", "slime-s", "python3 -c 1").unwrap();
    // A line for two names takes the place of each name's line.
    let both = code(&w, "orc-o", "slime-s", "make check # D3, D4").unwrap();
    assert_eq!(both.replaced, ["python3 -m unittest -q # D3", "true # D4"]);
    // The doc and the pins hold the same three lines: the gate reads them as a set.
    let sorted = |mut lines: Vec<String>| {
        lines.sort();
        lines
    };
    let three = ["make check # D3, D4", "python3 -c 1", "python3 -c pass"];
    assert_eq!(
        sorted(Roster::cards(&w).creatures["slime-s"].verify.clone()),
        three
    );
    assert_eq!(
        sorted(pinned(&w, "slime-s").into_iter().map(|(l, _)| l).collect()),
        three
    );
    let journal = fs::read_to_string(w.join(".mnemorph/log.md")).unwrap();
    assert!(
        journal.contains("in place of `python3 -m unittest -q # D3`"),
        "the journal says what a line replaced: {journal}"
    );

    // The human's pinned line stays, whatever name it carries: the Orc's is added beside it.
    doc(
        &w,
        "slime",
        "u",
        "# slime-u\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/u/\n- **Verify:** `make human # D3`\n",
    );
    code(&w, "orc-o", "slime-s", "true").unwrap();
    mnemorph_world::pins::record(
        &w,
        mnemorph_world::pins::row(
            mnemorph_world::pins::Act::Pin,
            "slime-u",
            &["make human # D3".to_owned()],
            mnemorph_world::pins::HUMAN,
        ),
    )
    .unwrap();
    let beside = code(&w, "orc-o", "slime-u", "true # D3").unwrap();
    assert!(beside.replaced.is_empty(), "{:?}", beside.replaced);
    assert_eq!(
        sorted(Roster::cards(&w).creatures["slime-u"].verify.clone()),
        ["make human # D3", "true # D3"]
    );
    assert_eq!(
        pinned(&w, "slime-u"),
        [
            ("make human # D3".to_owned(), "human".to_owned()),
            ("true # D3".to_owned(), "orc-o".to_owned())
        ]
    );
    let _ = fs::remove_dir_all(&w);
    let _ = fs::remove_dir_all(state_of(&w));
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
        ("orc-o", "slime-s", "  ", "1 to 500"),
    ] {
        let e = code(&w, by, to, cmd).unwrap_err();
        assert!(e.contains(why), "{by} {to} {cmd:?}: {e}");
    }
    // A line of a `Done when` list is a whole command with what it must give: in two kept runs
    // eight coded lines were refused at 300 characters, seven of them by under 65. 500 holds
    // them; a longer one is still refused.
    let long = |n: usize| format!("python3 -c \"{}\"", "x".repeat(n - 13));
    assert_eq!(long(362).chars().count(), 362);
    assert!(code(&w, "orc-o", "slime-s", &long(362)).is_ok());
    assert!(code(&w, "orc-o", "slime-s", &long(500)).is_ok());
    let e = code(&w, "orc-o", "slime-s", &long(501)).unwrap_err();
    assert!(e.contains("1 to 500"), "{e}");
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

/// A session card with no verify line is said, with the line to add: the command the project's
/// manifest names — never one creatures wrote.
#[test]
fn a_session_card_without_verify_is_said_with_a_suggestion() {
    use mnemorph_world::Facts;
    use mnemorph_world::facts::from_unsaid;
    use mnemorph_world::verify::{missing_session_verify, suggest};
    let w = world("suggest");
    assert_eq!(suggest(&w), None, "nothing to read");
    fs::write(w.join("package.json"), r#"{"scripts": {"test": "vitest"}}"#).unwrap();
    fs::write(w.join("pnpm-lock.yaml"), "").unwrap();
    assert_eq!(suggest(&w).as_deref(), Some("pnpm test"));
    fs::write(w.join("Cargo.toml"), "[workspace]\n").unwrap();
    assert_eq!(suggest(&w).as_deref(), Some("cargo test --workspace"));
    let said = missing_session_verify(&w, &Roster::cards(&w)).unwrap();
    assert!(said.contains("`cargo test --workspace`"), "{said}");
    // Creatures agreeing on a test command change nothing: a suggested line is never a model's.
    for by in ["slime-a", "slime-b"] {
        let e = from_unsaid("./Cargo.toml test=rm -rf ~", by, None).unwrap();
        Facts::record(&w, &e).unwrap();
    }
    assert_eq!(suggest(&w).as_deref(), Some("cargo test --workspace"));
    // A compiled project's cold build may outrun the gate's limit: the note says which key.
    assert!(said.contains("`gate.verify_secs`"), "{said}");
    let python = world("suggest-python");
    fs::write(python.join("pyproject.toml"), "[project]\nname = \"p\"\n").unwrap();
    let said = missing_session_verify(&python, &Roster::cards(&python)).unwrap();
    assert!(!said.contains("verify_secs"), "{said}");
    // npm's placeholder test script suggests nothing.
    let bare = world("suggest-npm");
    fs::write(
        bare.join("package.json"),
        r#"{"scripts": {"test": "echo \"Error: no test specified\" && exit 1"}}"#,
    )
    .unwrap();
    assert_eq!(suggest(&bare), None);
    // A card that has its line is never told.
    doc(
        &w,
        "summoner",
        "summoner",
        "# summoner\n\n- **Race:** summoner\n- **Verify:** `cargo nextest run`\n",
    );
    assert_eq!(missing_session_verify(&w, &Roster::cards(&w)), None);
}

/// A Cargo workspace is suggested whole — every member's tests — and nothing from its manifest
/// reaches the line: a package name is an agent's words, never pasted. A compiled project is told
/// its cold build may need `gate.verify_secs`.
#[test]
fn a_cargo_workspace_is_suggested_whole_and_never_by_its_names() {
    use mnemorph_world::verify::{missing_session_verify, suggest};
    let w = world("suggest-members");
    fs::write(
        w.join("Cargo.toml"),
        "[package]\nname = \"x; rm -rf ~\"\n\n[workspace]\nmembers = [\"crates/*\"]\n",
    )
    .unwrap();
    assert_eq!(suggest(&w).as_deref(), Some("cargo test --workspace"));
    let said = missing_session_verify(&w, &Roster::cards(&w)).unwrap();
    assert!(!said.contains("rm -rf"), "{said}");
    assert!(said.contains("`gate.verify_secs`"), "{said}");
}
