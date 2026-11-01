//! `mnemorphd onto owner`: each path's owner, the rule and the claim, read from the docs with no
//! key and no session.

#[path = "support/isolated.rs"]
mod isolated;

use std::process::Command;

fn onto(args: &[&str], cwd: &std::path::Path) -> (bool, String, String) {
    let out = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(args)
        .current_dir(cwd)
        .env_remove("ANTHROPIC_API_KEY")
        .env("XDG_STATE_HOME", state())
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn owner_names_the_owner_the_rule_and_the_claim() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (ok, out, err) = onto(
        &[
            "onto",
            "owner",
            "crates/mnemorph-wire/src/lib.rs",
            ".mnemorph/law.md",
            "../outside",
        ],
        &repo,
    );
    assert!(ok, "{err}");
    assert!(err.is_empty(), "the world's docs all read: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines[0],
        "crates/mnemorph-wire/src/lib.rs\tslime-wire\tzone crates/mnemorph-wire/"
    );
    assert_eq!(
        lines[1],
        ".mnemorph/law.md\tthe human\tthe human's file .mnemorph/law.md"
    );
    assert_eq!(lines[2], "../outside\tnobody\toutside the world");
    // A directory that exists is read as one, its `/` typed or not; the world's root is no claim.
    let (_, out, _) = onto(&["onto", "owner", "crates/mnemorph-wire", "."], &repo);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines[0],
        "crates/mnemorph-wire\tslime-wire\tzone crates/mnemorph-wire/"
    );
    assert_eq!(lines[1], ".\tnobody\tthe world's root");
    // The version answers first, wherever it stands.
    let (ok, out, _) = onto(&["onto", "owner", "--version"], &repo);
    assert!(ok && out.starts_with("mnemorphd "), "{out}");
    // From inside the world, a path is the human's: relative to where they stand.
    let (ok, out, _) = onto(
        &["onto", "owner", "src/lib.rs", "--world", "../.."],
        &repo.join("crates/mnemorph-wire"),
    );
    assert!(ok);
    assert_eq!(
        out.trim(),
        "src/lib.rs\tslime-wire\tzone crates/mnemorph-wire/"
    );
}

/// Where these tests' queues live: never the human's state directory.
fn state() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mnemorph-q-state-{}", std::process::id()))
}

#[test]
fn onto_says_what_it_needs() {
    let here = std::env::temp_dir();
    let (ok, _, err) = onto(&["onto", "owner"], &here);
    assert!(!ok && err.contains("needs a path"), "{err}");
    let (ok, _, err) = onto(&["onto", "who"], &here);
    assert!(!ok && err.contains("onto has no `who`"), "{err}");
}

#[test]
fn check_passes_this_world_and_names_its_findings() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (ok, out, err) = onto(&["onto", "check"], &repo);
    assert!(ok, "{out}{err}");
    assert!(out.starts_with("@S PASS "), "{out}");
    for line in out.lines().skip(1) {
        assert!(
            line.starts_with("medium\t") || line.starts_with("low\t"),
            "{line}"
        );
    }
    let (ok, _, err) = onto(&["onto", "check", "src/"], &repo);
    assert!(!ok && err.contains("takes no paths"), "{err}");
}

/// A name git would quote (`café`) reaches the check as it is.
#[test]
fn check_reads_names_git_would_quote() {
    let dir = std::env::temp_dir().join(format!("mnemorph-onto-quote-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".mnemorph/slime/a")).unwrap();
    std::fs::create_dir_all(dir.join(".mnemorph/orc/o")).unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join(".mnemorph/orc/o/README.md"),
        "# orc-o\n\n- **Race:** orc\n",
    )
    .unwrap();
    std::fs::write(
        dir.join(".mnemorph/slime/a/README.md"),
        "# slime-a\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/café.rs"), "").unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(args)
            .output()
            .unwrap()
    };
    git(&["init", "-q"]);
    git(&["add", "src", ".mnemorph"]);
    let (_, out, _) = onto(&["onto", "check"], &dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!out.contains("src/"), "the Slime owns src/café.rs: {out}");
}

/// The queue's door: add, list, claim, and a refusal that exits 2 saying why.
#[test]
fn the_queue_door() {
    let dir = std::env::temp_dir().join(format!("mnemorph-q-door-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".mnemorph/slime/s")).unwrap();
    std::fs::write(
        dir.join(".mnemorph/slime/s/README.md"),
        "# slime-s\n\n- **Race:** slime\n",
    )
    .unwrap();
    let q = |args: &[&str]| {
        let mut all = vec!["q"];
        all.extend_from_slice(args);
        onto(&all, &dir)
    };
    let (ok, out, _) = q(&[
        "add", "tidy", "the", "test", "--for", "slime-s", "--rank", "1",
    ]);
    assert!(ok, "{out}");
    let id = out.split('\t').next().unwrap().to_owned();
    assert!(out.contains("\theld\tslime-s\t1\ttidy the test"), "{out}");
    let (ok, out, _) = q(&["claim", &id]);
    assert!(ok && out.contains("\tclaimed\t"), "{out}");
    let (ok, out, _) = q(&["list"]);
    assert!(ok && out.lines().count() == 1, "{out}");
    let out = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(["q", "close", "nope"])
        .env("XDG_STATE_HOME", state())
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("refused: no item `nope`"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A body's command never reaches the queue as the human.
#[test]
fn a_bodys_command_reads_the_queue_but_never_moves_it() {
    let dir = std::env::temp_dir().join(format!("mnemorph-q-body-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".mnemorph")).unwrap();
    let run = |args: &[&str]| {
        isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
            .args(args)
            .current_dir(&dir)
            .env("MNEMORPH_BODY", "slime-s")
            .env("XDG_STATE_HOME", state())
            .output()
            .unwrap()
    };
    assert_eq!(run(&["q", "list"]).status.code(), Some(2));
    let out = run(&["q", "add", "sneak", "in"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("slime-s runs this"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Housekeep refuses inside a body's command, and says the roster is whole when it is.
#[test]
fn housekeep_refuses_a_body_and_reads_a_whole_roster() {
    let dir = std::env::temp_dir().join(format!("mnemorph-housekeep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".mnemorph/rimuru/rimuru")).unwrap();
    std::fs::write(
        dir.join(".mnemorph/rimuru/rimuru/README.md"),
        "# rimuru\n\n- **Race:** rimuru\n",
    )
    .unwrap();
    let run = |body: Option<&str>| {
        let mut c = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"));
        c.args(["housekeep", "--heal"]).current_dir(&dir);
        if let Some(b) = body {
            c.env("MNEMORPH_BODY", b);
        }
        c.output().unwrap()
    };
    let out = run(Some("slime-s"));
    assert_eq!(out.status.code(), Some(2));
    let out = run(None);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("the roster is whole"));
    let _ = std::fs::remove_dir_all(&dir);
}
