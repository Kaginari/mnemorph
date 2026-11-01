//! The trusted facts and the relations between worlds live under the human's state directory, and
//! the sandbox is what keeps a body's command from them — not `MNEMORPH_BODY`, which a body can
//! unset. Inside the real guard, a body writes neither journal, and `mnemorphd onto relate` run
//! with the variable unset still states nothing.

#![cfg(target_os = "linux")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_races::Race;
use mnemorph_sandbox::Sandbox;
use mnemorph_tools::gate::Scripted;
use mnemorph_tools::{Context, Outcome, call, registry};

/// `target/<profile>/<name>`, built beside this test.
fn built(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let at = exe.parent()?.parent()?.join(name);
    at.is_file().then_some(at)
}

fn bash(cx: &mut Context, command: &str) -> Outcome {
    let args = serde_json::json!({ "command": command }).to_string();
    call(&registry(), "bash", &args, cx)
}

fn text(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

#[test]
fn a_guarded_body_reaches_neither_the_trusted_facts_nor_the_relations() {
    let (Some(guard), Some(mnemorphd)) = (built("mnemorph-guard"), built("mnemorphd")) else {
        eprintln!(
            "skipped: build the workspace so target/<profile>/mnemorph-guard and mnemorphd exist"
        );
        return;
    };
    let root = std::env::temp_dir().join(format!("mnemorph-state-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let world = root.join("world");
    let other = root.join("other");
    let state = root.join("state");
    for d in [world.join("src"), other.clone(), root.join("home")] {
        fs::create_dir_all(d).unwrap();
    }
    // The journals as the harness keeps them, under the state directory.
    let facts = state.join("mnemorph/facts/world.jsonl");
    let relations = state.join("mnemorph/dimension/world/relations.jsonl");
    for j in [&facts, &relations] {
        fs::create_dir_all(j.parent().unwrap()).unwrap();
        fs::write(j, "{\"kept\":true}\n").unwrap();
    }
    // The core, where a body can run it: inside the world.
    fs::copy(&mnemorphd, world.join("src/mnemorphd")).unwrap();
    fs::write(world.join("src/forge.sh"), "echo forged >> \"$1\"\n").unwrap();
    fs::write(
        world.join("src/relate.sh"),
        format!(
            "XDG_STATE_HOME='{}' env -u MNEMORPH_BODY ./src/mnemorphd onto relate '{}' deploys --yes --world .\n",
            state.display(),
            other.display()
        ),
    )
    .unwrap();
    let mut cx = Context::new(Race::Rimuru, world.clone(), &["."])
        .with_body("rimuru")
        .with_gate(Arc::new(Scripted::new([])))
        .with_home(root.join("home"))
        .with_sandbox(Sandbox::Guard(guard));
    // The control: the same command writes the world, so a refusal below is the guard's.
    let o = bash(&mut cx, "bash src/forge.sh src/control.txt");
    assert!(!o.is_error, "{o:?}");
    assert_eq!(text(&world.join("src/control.txt")), "forged\n");
    for j in [&facts, &relations] {
        let o = bash(&mut cx, &format!("bash src/forge.sh {}", j.display()));
        assert!(o.is_error, "{o:?}");
        assert_eq!(text(j), "{\"kept\":true}\n", "{o:?}");
    }
    let o = bash(&mut cx, "bash src/relate.sh");
    // The core ran, and was refused: its write never reached the state directory.
    assert!(o.content.contains("mnemorphd: refused"), "{o:?}");
    assert!(o.is_error, "{o:?}");
    assert_eq!(text(&relations), "{\"kept\":true}\n", "{o:?}");
    let real = state.join("mnemorph/dimension");
    let written: Vec<_> = walk(&real)
        .into_iter()
        .filter(|p| p.ends_with("relations.jsonl") && text(p) != "{\"kept\":true}\n")
        .collect();
    assert_eq!(written, Vec::<PathBuf>::new(), "{o:?}");
    assert!(!world.join(".mnemorph/dimension.toml").exists(), "{o:?}");
    let _ = fs::remove_dir_all(&root);
}

/// Every file under `dir`.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}
