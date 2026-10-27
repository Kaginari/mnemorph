//! `mnemorph run` against a scripted world: the session's own path and end gate, the answer on
//! stdout, progress and the ending on stderr, `--json`'s lines, and a gated act denied.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use mnemorph_loop::open::{Bound, open_with};
use mnemorph_loop::{Config, Paths, ScriptedModel};
use mnemorph_tui::headless;
use serde_json::{Value, json};

struct World(PathBuf);

impl World {
    /// A world whose session card's verify line is `grep -q fixed out.txt`.
    fn new(name: &str) -> Self {
        let w = std::env::temp_dir().join(format!("mnemorph-run-{name}-{}", std::process::id()));
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
        Self(w)
    }

    /// Run `ask` with the model's `replies`: the exit code, stdout and stderr.
    fn run(&self, model: ScriptedModel, json: bool) -> (u8, String, String) {
        let model = Arc::new(model);
        let config =
            Config::load(&Paths::with_global(self.0.join("no-global.toml"), &self.0)).unwrap();
        let attic = self.0.with_extension("attic");
        let (tx, rx) = headless::channel();
        let opened = open_with(
            &self.0,
            &config,
            Bound {
                model,
                model_id: "scripted",
                attic: Some(&attic),
            },
            headless::gate(tx.clone(), false),
            headless::observer(tx.clone()),
            |_| None,
        )
        .unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = headless::run(
            opened,
            "make out.txt say fixed",
            json,
            (tx, rx),
            &mut out,
            &mut err,
        )
        .unwrap();
        let _ = fs::remove_dir_all(&attic);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

macro_rules! write {
    ($id:expr, $content:expr) => {
        ScriptedModel::tool($id, "write", json!({"path": "out.txt", "content": $content}))
    };
}

#[test]
fn an_answer_lands_through_the_end_gate() {
    let w = World::new("pass");
    let (code, out, err) = w.run(
        ScriptedModel::new([write!("w1", "fixed\n"), ScriptedModel::text("Fixed it.")]),
        false,
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "Fixed it.\n", "the answer alone on stdout");
    assert!(err.contains("⏺ write out.txt"), "{err}");
    assert!(err.contains("◆ end gate PASS"), "{err}");
    assert!(
        !err.contains("mnemorph: "),
        "an answer says no ending: {err}"
    );
}

#[test]
fn a_failing_verify_line_sends_it_back_and_then_fails() {
    let w = World::new("fail");
    // The first failure goes back to the body; it fixes it, and lands.
    let (code, out, err) = w.run(
        ScriptedModel::new([
            write!("w1", "broken\n"),
            ScriptedModel::text("Done."),
            write!("w2", "fixed\n"),
            ScriptedModel::text("Fixed now."),
        ]),
        false,
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "Fixed now.\n");
    assert!(err.contains("◆ end gate FAIL"), "{err}");
    // Failing twice, it does not land: no answer on stdout, the ending on stderr's last line.
    let w = World::new("fail-twice");
    let (code, out, err) = w.run(
        ScriptedModel::new([
            write!("w1", "broken\n"),
            ScriptedModel::text("Done."),
            write!("w2", "still broken\n"),
            ScriptedModel::text("Done again."),
        ]),
        false,
    );
    assert_ne!(code, 0, "{err}");
    assert_eq!(out, "");
    let last = err.lines().last().unwrap_or_default();
    assert!(last.starts_with("mnemorph: "), "{err}");
    assert!(!last.contains("answered"), "{err}");
}

#[test]
fn json_prints_every_event_then_the_result() {
    let w = World::new("json");
    let (code, out, err) = w.run(
        ScriptedModel::new([write!("w1", "fixed\n"), ScriptedModel::text("Fixed it.")]),
        true,
    );
    assert_eq!(code, 0, "{err}");
    let lines: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect();
    let kinds: Vec<&str> = lines.iter().filter_map(|l| l["type"].as_str()).collect();
    for kind in [
        "turnStarted",
        "toolCall",
        "toolResult",
        "gateVerdict",
        "turnEnded",
    ] {
        assert!(kinds.contains(&kind), "{kind} in {kinds:?}");
    }
    for l in &lines[..lines.len() - 1] {
        assert_eq!(l["body"], "rimuru", "{l}");
        assert!(l["run"].is_string() && l["session"].is_string() && l["seq"].is_u64());
    }
    let ended = lines.iter().find(|l| l["type"] == "turnEnded").unwrap();
    assert_eq!(ended["ending"], "answered");
    let result = lines.last().unwrap();
    assert_eq!(result["type"], "result");
    assert_eq!(result["ending"], "answered");
    assert_eq!(result["answered"], true);
    assert_eq!(result["answer"], "Fixed it.");
    assert_eq!(result["exit"], 0);
    assert_eq!(result["gate"]["verdict"], "PASS");
    assert_eq!(result["gate"]["passed"], true);
    assert!(result["usage"]["calls"].is_u64(), "{result}");
    assert!(result["run"].is_string() && result["session"].is_string());
}

#[test]
fn a_gated_act_is_denied_and_said_escaped() {
    let w = World::new("gated");
    fs::write(w.0.join("keep.txt"), "keep\n").unwrap();
    // An in-place edit is refused unless the human lets its rule ask: here it asks.
    fs::write(
        w.0.join(".mnemorph/config.local.toml"),
        "[sandbox]\nallow_unguarded = true\n[gate]\nask_rules = [\"in-place-edit\"]\n",
    )
    .unwrap();
    fs::write(w.0.join("out.txt"), "fixed\n").unwrap();
    let (code, out, err) = w.run(
        ScriptedModel::new([
            ScriptedModel::tool(
                "b1",
                "bash",
                json!({"command": "sed -i 's/keep/gone\u{1b}[2J/' keep.txt"}),
            ),
            ScriptedModel::text("I could not remove it."),
        ]),
        false,
    );
    assert_eq!(
        fs::read_to_string(w.0.join("keep.txt")).unwrap(),
        "keep\n",
        "never approved on its own"
    );
    assert!(err.contains("mnemorph: denied"), "{err}");
    assert!(!err.contains('\u{1b}'), "escaped: {err:?}");
    assert!(err.contains("\\u{1b}"), "{err}");
    assert_eq!(
        (code, out.as_str()),
        (0, "I could not remove it.\n"),
        "{err}"
    );
}
