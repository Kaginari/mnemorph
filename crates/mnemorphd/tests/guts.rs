//! `mnemorphd guts` through the binary: a fit over runs from a file, scored on the runs it held out
//! against their base rate, kept under the human's state directory with `--save`, and shown.

#[path = "support/isolated.rs"]
mod isolated;

use std::path::Path;

fn cmd(root: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(args)
        .current_dir(root)
        .env_remove("MNEMORPH_BODY")
        .env("HOME", root)
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_fit_is_scored_on_runs_it_held_out_and_kept_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    // Runs that failed came with failing acts, runs that landed with few.
    let lines: Vec<String> = (0..80)
        .map(|i| {
            let landed = i % 2 == 0;
            let fails = if landed { 0 } else { 4 };
            format!(
                r#"{{"run":"r{i}","race":"slime","windows":[[{fails},0,0,0,0,0,0,50,10],[{fails},0,1,0,0,0,0,50,10]],"inputs":2,"landed":{landed}}}"#
            )
        })
        .chain(std::iter::once(
            // A run read with the six class counts alone: left out, and counted.
            r#"{"run":"r-old","race":"slime","windows":[[0,0,0,0,0,0]],"landed":true}"#.to_owned(),
        ))
        .collect();
    std::fs::write(root.join("runs.jsonl"), lines.join("\n")).unwrap();

    let (code, out, err) = cmd(&root, &["guts", "show"]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("no learned guts kept yet"), "{out}");

    let (code, out, err) = cmd(&root, &["guts", "fit", "runs.jsonl", "--save"]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("slime:") && out.contains("beats the base rate"),
        "{out}"
    );
    assert!(
        out.contains("1 of 81 runs were read with other inputs"),
        "{out}"
    );
    assert!(out.contains("kept at"), "{out}");
    assert!(root.join("state/mnemorph/learned/guts.json").exists());

    let (code, out, _) = cmd(&root, &["guts", "show"]);
    assert_eq!(code, Some(0));
    assert!(
        out.contains("slime:")
            && out.contains("red-pin +0.000")
            && out.contains("rows passing")
            && out.contains("clock"),
        "{out}"
    );
}

#[test]
fn a_line_that_is_no_run_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("runs.jsonl"), "{\"run\": 1}\n").unwrap();
    let (code, _, err) = cmd(&root, &["guts", "fit", "runs.jsonl"]);
    assert_ne!(code, Some(0));
    assert!(err.contains("runs.jsonl:1"), "{err}");
}

/// A fit is read against the base rate of the same stretch of the run, and says how many fits its held-out set
/// has judged: none the first time, one the second. The count is kept under the state directory.
#[test]
fn a_fit_names_its_reference_and_how_often_its_held_out_set_has_judged() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    // Ten tasks, eight runs each, half of them landed: every group holds both.
    let lines: Vec<String> = (0..80)
        .map(|i| {
            let landed = (i / 10) % 2 == 0;
            let fails = if landed { 0 } else { 4 };
            format!(
                r#"{{"run":"r{i}","race":"slime","windows":[[{fails},0,0,0,0,0,0,50,10],[{fails},0,1,0,0,0,0,50,10]],"inputs":2,"landed":{landed},"group":"task-{}"}}"#,
                i % 10
            )
        })
        .collect();
    std::fs::write(root.join("runs.jsonl"), lines.join("\n")).unwrap();
    let (code, first, err) = cmd(&root, &["guts", "fit", "runs.jsonl"]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        first.contains("against the base rate of the same stretch of the run"),
        "{first}"
    );
    // Here the acts tell and the clock cannot: by group, the guts clears the reference.
    assert!(
        first.contains("no worse in any of the") && first.contains("clears the reference"),
        "{first}"
    );
    assert!(first.contains("has judged no fit before"), "{first}");
    assert!(!first.contains("has judged 1 fit"), "{first}");
    assert!(root.join("state/mnemorph/learned/held-out.json").exists());
    let (_, second, _) = cmd(&root, &["guts", "fit", "runs.jsonl"]);
    assert!(second.contains("has judged 1 fit before"), "{second}");
    assert!(!second.contains("has judged no fit before"), "{second}");
}
