//! `mnemorphd onto owner`: each path's owner, the rule and the claim, read from the docs with no
//! key and no session.

use std::process::Command;

fn onto(args: &[&str], cwd: &std::path::Path) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(args)
        .current_dir(cwd)
        .env_remove("ANTHROPIC_API_KEY")
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
