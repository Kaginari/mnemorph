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

/// The human settles a contested fact; a body is never let do it, and an unknown id is refused.
#[test]
fn the_human_settles_a_fact_and_a_body_cannot() {
    use mnemorph_server::world::Facts;
    use mnemorph_server::world::facts::{Event, from_unsaid};
    let world = std::env::temp_dir().join(format!("mnemorph-onto-settle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    fs::create_dir_all(world.join("src")).unwrap();
    let said = from_unsaid("src/ build=cargo test", "slime-a", None).unwrap();
    let Event::Support { id, .. } = &said else {
        panic!("{said:?}");
    };
    let id = id.clone();
    Facts::record(&world, &said).unwrap();
    let w = world.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_mnemorphd");
    let body = Command::new(bin)
        .args(["onto", "settle", &id, "--world", w])
        .env("MNEMORPH_BODY", "slime-a")
        .output()
        .unwrap();
    assert_eq!(body.status.code(), Some(2));
    let unknown = Command::new(bin)
        .args(["onto", "settle", "nope", "--world", w])
        .env_remove("MNEMORPH_BODY")
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    let human = Command::new(bin)
        .args(["onto", "settle", &format!("#{id}"), "--world", w])
        .env_remove("MNEMORPH_BODY")
        .output()
        .unwrap();
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    let f = &Facts::load(&world).facts[&id];
    assert!(f.settled);
    let _ = fs::remove_dir_all(&world);
}

/// A proposed claim is listed with its vouch; the human confirms it, and it is grounded and
/// shared by the confirmed route. A body is never let, and an id no creature proposes is refused.
#[test]
fn the_human_confirms_a_proposed_claim_and_a_body_cannot() {
    use mnemorph_server::world::memory::{Status, Tier, fold, propose, read};
    let world =
        std::env::temp_dir().join(format!("mnemorph-memory-confirm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    fs::create_dir_all(&world).unwrap();
    let claim = propose(&world, "slime-a", "run uv before pytest", vec![], None).unwrap();
    let id = claim.note.clone();
    let (_, out) = status(&world, false);
    assert!(
        out.contains(&format!(
            "proposed: {id} — run uv before pytest (vouch: mnemorph memory confirm {id})"
        )),
        "{out}"
    );
    let w = world.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_mnemorphd");
    let run = |args: &[&str], body: Option<&str>| {
        let mut c = Command::new(bin);
        c.args(args).args(["--world", w]);
        match body {
            Some(b) => c.env("MNEMORPH_BODY", b),
            None => c.env_remove("MNEMORPH_BODY"),
        };
        c.output().unwrap()
    };
    assert_eq!(
        run(&["memory", "confirm", &id], Some("slime-a"))
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        run(&["memory", "confirm", "n0"], None).status.code(),
        Some(2)
    );
    let human = run(&["memory", "confirm", &id], None);
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    let shared = fold(&read(&world, &Tier::Shared).unwrap().rows);
    assert_eq!(shared[&id].status, Status::Grounded);
    assert!(
        shared[&id]
            .proof
            .as_deref()
            .unwrap()
            .starts_with("confirmed: ")
    );
    let _ = fs::remove_dir_all(&world);
}
