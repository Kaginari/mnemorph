//! `mnemorphd memory status`: each creature's unproven note events, named for the human; it fails
//! while any is found.

use std::fs;
use std::process::Command;

fn status(world: &std::path::Path, json: bool) -> (Option<i32>, String) {
    let mut args = vec!["memory", "status", "--world"];
    let w = world.to_str().unwrap();
    args.push(w);
    if json {
        args.push("--json");
    }
    let out = Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(&args)
        .env_remove("ANTHROPIC_API_KEY")
        .env("XDG_STATE_HOME", world.join("state"))
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn a_hand_edit_is_named_and_fails_the_status() {
    let world = std::env::temp_dir().join(format!("mnemorph-memory-status-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    let dir = world.join(".mnemorph/memory/creature");
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(
        status(&world, false),
        (Some(0), "no creature has memory here yet\n".into())
    );
    fs::write(
        dir.join("slime-a.jsonl"),
        concat!(
            r#"{"id":"e1","time":1,"note":"n1","by":"record","kind":"create","text":"done","tags":[],"owner":"slime-a","polarity":"support","salience":0.5,"confidence":0.5,"source":null,"links":[]}"#,
            "\n",
            r#"{"id":"e2","time":2,"note":"n1","by":"record","kind":"status","status":"grounded"}"#,
            "\n",
        ),
    )
    .unwrap();
    let (code, out) = status(&world, false);
    assert_eq!(code, Some(1));
    assert!(out.contains("\nslime-a: frozen — 1 note event(s)"), "{out}");
    assert!(
        out.contains("event e2  note n1: a standing claimed by a plain record"),
        "{out}"
    );
    let (code, out) = status(&world, true);
    assert_eq!(code, Some(1));
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v[0]["creature"], "slime-a");
    assert_eq!(v[0]["unproven"][0]["event"], "e2");
    let _ = fs::remove_dir_all(&world);
}

/// A note the gate retired is listed for the human with its undo; `wake` restores it, and a body is
/// never let wake one.
#[test]
fn a_retired_note_is_listed_and_the_human_wakes_it() {
    let world = std::env::temp_dir().join(format!("mnemorph-memory-wake-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    let dir = world.join(".mnemorph/memory/creature");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("slime-a.jsonl"),
        concat!(
            r#"{"id":"e1","time":1,"note":"n1","by":"record","kind":"create","text":"skip the fixture","tags":[],"owner":"slime-a","polarity":"support","salience":0.5,"confidence":0.5,"source":null,"links":[]}"#,
            "\n",
            r#"{"id":"e2","time":2,"note":"n1","by":"human","kind":"status","status":"grounded"}"#,
            "\n",
            r#"{"id":"e3","time":3,"note":"n1","by":"gate","proof":"log.md two","kind":"status","status":"dormant"}"#,
            "\n",
        ),
    )
    .unwrap();
    let (code, out) = status(&world, false);
    assert_eq!(code, Some(0), "a retirement waits on nobody: {out}");
    assert!(
        out.contains(
            "retired, kept dormant: n1 — skip the fixture (undo: mnemorph memory wake n1)"
        ),
        "{out}"
    );
    let bin = env!("CARGO_BIN_EXE_mnemorphd");
    let body = Command::new(bin)
        .args(["memory", "wake", "n1", "--world", world.to_str().unwrap()])
        .env("MNEMORPH_BODY", "slime-a")
        .output()
        .unwrap();
    assert_eq!(body.status.code(), Some(2), "a body never wakes a note");
    let human = Command::new(bin)
        .args(["memory", "wake", "n1", "--world", world.to_str().unwrap()])
        .env_remove("MNEMORPH_BODY")
        .output()
        .unwrap();
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    let (_, out) = status(&world, false);
    assert_eq!(
        out,
        "memory: 0 note(s) shared and grounded; personal: 0 proposed, 1 grounded\nslime-a: clear\n"
    );
    let _ = fs::remove_dir_all(&world);
}
