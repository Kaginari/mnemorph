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
