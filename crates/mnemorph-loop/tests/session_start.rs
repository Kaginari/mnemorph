//! Session start: a fresh session is given the world's law and the journal's last entries by the
//! harness itself — the law in the stable system prompt, the entries with its first ask alone.

use std::fs;
use std::sync::Arc;

use mnemorph_loop::open::{Bound, open_with};
use mnemorph_loop::{Config, Paths, ScriptedModel};
use mnemorph_provider::{Block, Role};

fn user_text(model: &ScriptedModel, i: usize) -> String {
    model.requests()[i]
        .messages
        .iter()
        .rev()
        .find(|m| m.role == Role::User)
        .unwrap()
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_fresh_session_carries_the_law_and_the_last_journal_entries() {
    let w = std::env::temp_dir().join(format!("mnemorph-session-start-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join(".mnemorph")).unwrap();
    fs::write(
        w.join(".mnemorph/law.md"),
        "# The law\n\n1. **Vitality** — docs change with code.\n",
    )
    .unwrap();
    let log: String = std::iter::once("# Journal\n".to_owned())
        .chain((1..=7).map(|i| {
            format!(
                "\n---\n\n### [2026-10-2{i}T10:00:00+02:00] rimuru — entry {i}\n- learned thing {i}\n"
            )
        }))
        .collect();
    fs::write(w.join(".mnemorph/log.md"), log).unwrap();
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::text("first"),
        ScriptedModel::text("second"),
    ]));
    let config = Config::load(&Paths::with_global(w.join("no-global.toml"), &w)).unwrap();
    let attic = w.join("../mnemorph-session-start-attic");
    let mut opened = open_with(
        &w,
        &config,
        Bound {
            model: model.clone(),
            model_id: "scripted-model",
            attic: Some(&attic),
        },
        Arc::new(mnemorph_tools::NoHuman),
        Arc::new(|_: &mnemorph_loop::Seen| {}),
        |_| None,
    )
    .unwrap();
    opened.session.run().ask("what next", &mut |_| {}).await;
    opened.session.run().ask("and then", &mut |_| {}).await;
    let system = format!("{:?}", model.requests()[0].system);
    assert!(
        system.contains("**Vitality** — docs change with code."),
        "{system}"
    );
    let first = user_text(&model, 0);
    for i in 3..=7 {
        assert!(
            first.contains(&format!("entry {i}\n- learned thing {i}")),
            "{first}"
        );
    }
    assert!(!first.contains("entry 2\n"), "the last five only: {first}");
    assert!(first.ends_with("what next"), "the ask stays last: {first}");
    assert!(
        !user_text(&model, 1).contains("learned thing"),
        "said once, with the first ask"
    );
    assert_eq!(
        format!("{:?}", model.requests()[1].system),
        system,
        "the system prompt stays the cached prefix"
    );
    let _ = fs::remove_dir_all(&w);
    let _ = fs::remove_dir_all(&attic);
}

/// The toolbox's two levels: a fresh session's first ask lists the Minds it may wear, each priced,
/// and `wear` brings one's whole text in; the tool list stays the cached tier, whatever the Minds.
#[tokio::test]
async fn the_first_ask_lists_the_minds_and_wear_brings_one_in() {
    let w = std::env::temp_dir().join(format!("mnemorph-toolbox-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join(".mnemorph/minds")).unwrap();
    fs::write(
        w.join(".mnemorph/minds/dialects.md"),
        "# Dialects\n\n- **Shared:** yes\n\nHow a dialect overrides the generator.\n\nOverride `TRANSFORMS`.\n",
    )
    .unwrap();
    adopt(&w, &["dialects"]);
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool("w1", "wear", serde_json::json!({"mind": "dialects"})),
        ScriptedModel::text("done"),
    ]));
    let config = Config::load(&Paths::with_global(w.join("no-global.toml"), &w)).unwrap();
    let attic = w.join("../mnemorph-toolbox-attic");
    let mut opened = open_with(
        &w,
        &config,
        Bound {
            model: model.clone(),
            model_id: "scripted-model",
            attic: Some(&attic),
        },
        Arc::new(mnemorph_tools::NoHuman),
        Arc::new(|_: &mnemorph_loop::Seen| {}),
        |_| None,
    )
    .unwrap();
    opened.session.run().ask("add a dialect", &mut |_| {}).await;
    let first = user_text(&model, 0);
    assert!(
        first.contains("@T mind:dialects — How a dialect overrides the generator. (~"),
        "{first}"
    );
    assert!(model.requests()[0].tools.iter().any(|t| t.name == "wear"));
    let worn: String = model.requests()[1]
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|b| match b {
            Block::ToolResult { content, .. } => Some(format!("{content:?}")),
            _ => None,
        })
        .collect();
    assert!(worn.contains("Override `TRANSFORMS`."), "{worn}");
    let _ = fs::remove_dir_all(&w);
    let _ = fs::remove_dir_all(&attic);
}

/// The session's own body lands through the end gate, as a commissioned one does: its card's
/// verify lines must pass, and a claim about what passed is grounded by the check.
#[tokio::test]
async fn the_session_lands_through_the_end_gate() {
    use mnemorph_memory::{Status, Tier, fold, read};
    let w = std::env::temp_dir().join(format!("mnemorph-session-gate-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join(".mnemorph/rimuru/rimuru")).unwrap();
    fs::write(
        w.join(".mnemorph/rimuru/rimuru/README.md"),
        "# rimuru\n\n- **Race:** rimuru\n- **Verify:** `grep -q fixed out.txt`\n",
    )
    .unwrap();
    // No kernel guard in a test run: the verify line runs unguarded, as the human allows here.
    fs::write(
        w.join(".mnemorph/config.local.toml"),
        "[sandbox]\nallow_unguarded = true\n",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "w1",
            "write",
            serde_json::json!({"path": "out.txt", "content": "broken\n"}),
        ),
        ScriptedModel::text("Done."),
        ScriptedModel::tool(
            "w2",
            "write",
            serde_json::json!({"path": "out.txt", "content": "fixed\n"}),
        ),
        ScriptedModel::text("Fixed.\n@U law out.txt says fixed once the gate passes"),
    ]));
    let config = Config::load(&Paths::with_global(w.join("no-global.toml"), &w)).unwrap();
    let attic = w.join("../mnemorph-session-gate-attic");
    let mut opened = open_with(
        &w,
        &config,
        Bound {
            model: model.clone(),
            model_id: "scripted-model",
            attic: Some(&attic),
        },
        Arc::new(mnemorph_tools::NoHuman),
        Arc::new(|_: &mnemorph_loop::Seen| {}),
        |_| None,
    )
    .unwrap();
    let ending = opened
        .session
        .run()
        .ask("make out.txt say fixed", &mut |_| {})
        .await;
    assert!(
        matches!(ending, mnemorph_loop::Ending::Answered(_)),
        "{ending:?}"
    );
    assert_eq!(
        model.requests().len(),
        4,
        "the gate sent the first answer back"
    );
    let mine = fold(&read(&w, &Tier::Personal("rimuru".into())).unwrap().rows);
    let n = mine
        .values()
        .find(|n| n.text.starts_with("out.txt says fixed"))
        .unwrap_or_else(|| panic!("{mine:?}"));
    assert_eq!(n.status, Status::Grounded, "the check grounds it: {n:?}");
    let _ = fs::remove_dir_all(&w);
    let _ = fs::remove_dir_all(&attic);
}

/// The Minds `names` of `world` adopted in the witness kept for it, as the human's `mnemorphd mind
/// adopt` would record them.
fn adopt(world: &std::path::Path, names: &[&str]) {
    use mnemorph_world::minds::{propose_at, witness};
    let at = witness::path_of(world).expect("a home");
    for name in names {
        let row = propose_at(world, &at, witness::Act::Adopt, name, None)
            .unwrap()
            .row;
        witness::record_at(&at, row).unwrap();
    }
}
