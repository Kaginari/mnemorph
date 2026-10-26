//! Solo mode and its end gate, against a scripted model.

#![allow(
    clippy::unnecessary_wraps,
    reason = "scripted replies are Results: the script also holds errors"
)]

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use mnemorph_ledger::{Journal, Usage};
use mnemorph_loop::{Beat, EndGate, Ending, Limits, Run, ScriptedModel, Setup, solo};
use mnemorph_provider::{Block, Caller, Error, Response, StopReason, anthropic_prices};
use mnemorph_races::Race;
use mnemorph_tools::{Sandbox, registry};
use serde_json::{Value, json};

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-solo-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world")).unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn journal(&self) -> Journal<Beat> {
        Journal::new(self.root.join("run.jsonl"))
    }

    fn run(&self, model: Arc<ScriptedModel>, verify: &[&str]) -> Run {
        let (body, cx) = solo(&self.world(), "claude-sonnet-5-5");
        let cx = cx
            .with_sandbox(Sandbox::None("solo tests run unguarded".into()))
            .with_unguarded(true)
            .with_attic(
                mnemorph_tools::Attic::at(&self.root.join("attic"), &self.world()).expect("git"),
            );
        let setup = Setup {
            model,
            tools: registry(),
            cx,
            journal: self.journal(),
            ledger: None,
            prices: anthropic_prices().unwrap(),
            caller: Caller {
                session: "s".into(),
                run: "r".into(),
                body: "rimuru".into(),
                race: "rimuru".into(),
                commission: None,
                prefix: 0,
            },
        };
        Run::new(body, Limits::for_race(Race::Rimuru), setup).with_gate(EndGate {
            verify: verify.iter().map(|s| (*s).to_owned()).collect(),
            author: None,
        })
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn reply(content: Block, stop: StopReason) -> Result<Response, Error> {
    Ok(Response {
        id: "m".into(),
        model: "claude-sonnet-5-5".into(),
        content: vec![content],
        stop,
        stop_details: None,
        usage: Usage {
            input: 100,
            output: 20,
            cache_read: 0,
            cache_write: 0,
        },
        warnings: vec![],
    })
}

fn answer(text: &str) -> Result<Response, Error> {
    reply(
        Block::Text {
            text: text.into(),
            cache: false,
        },
        StopReason::EndTurn,
    )
}

fn tool(id: &str, name: &str, input: Value) -> Result<Response, Error> {
    reply(
        Block::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        },
        StopReason::ToolUse,
    )
}

async fn ask(run: &mut Run, text: &str) -> Ending {
    let mut quiet = |_| {};
    run.ask(text, &mut quiet).await
}

fn gate_beats(w: &World) -> Vec<Vec<String>> {
    w.journal()
        .read()
        .unwrap()
        .rows
        .into_iter()
        .filter_map(|b| match b {
            Beat::Gate { failures, .. } => Some(failures),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_change_that_verifies_passes_the_gate() {
    let w = World::new("pass");
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "write",
            json!({"path": "out.txt", "content": "fixed\n"}),
        ),
        answer("wrote out.txt"),
    ]));
    let mut run = w.run(model, &["grep -q fixed out.txt"]);
    assert_eq!(
        ask(&mut run, "make out.txt").await,
        Ending::Answered("wrote out.txt".into())
    );
    assert_eq!(gate_beats(&w), vec![Vec::<String>::new()]);
}

/// A failed gate gets one chance: the failure goes back to the body, which fixes it.
#[tokio::test]
async fn a_failed_gate_gets_one_chance_to_fix() {
    let w = World::new("fix");
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "write",
            json!({"path": "out.txt", "content": "broken\n"}),
        ),
        answer("done"),
        tool(
            "t2",
            "write",
            json!({"path": "out.txt", "content": "fixed\n"}),
        ),
        answer("fixed it"),
    ]));
    let mut run = w.run(model.clone(), &["grep -q fixed out.txt"]);
    assert_eq!(
        ask(&mut run, "make out.txt").await,
        Ending::Answered("fixed it".into())
    );
    let gates = gate_beats(&w);
    assert_eq!(gates.len(), 2);
    assert!(
        gates[0][0].contains("traits hold") && gates[1].is_empty(),
        "{gates:?}"
    );
    let told = model.requests()[2]
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, Block::Text { text, .. } if text.starts_with("The end gate failed")));
    assert!(told, "the failure reached the body");
}

#[tokio::test]
async fn a_second_failure_goes_up_as_fail() {
    let w = World::new("fail");
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "write",
            json!({"path": "out.txt", "content": "broken\n"}),
        ),
        answer("done"),
        answer("still done"),
    ]));
    let mut run = w.run(model, &["grep -q fixed out.txt"]);
    assert!(
        matches!(ask(&mut run, "make out.txt").await, Ending::Failed(why) if why.starts_with("the end gate failed twice"))
    );
}

/// Writes are found by snapshotting the tree: a shell redirect counts like the write tool, and a
/// deleted test fails the gate.
#[tokio::test]
async fn a_test_deleted_by_the_shell_fails_the_gate() {
    let w = World::new("deleted");
    fs::create_dir_all(w.world().join("tests")).unwrap();
    fs::write(w.world().join("tests/login.rs"), "#[test]\nfn login() {}\n").unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "bash", json!({"command": "rm tests/login.rs"})),
        answer("removed a flaky test"),
        answer("it was flaky"),
    ]));
    let mut run = w.run(model, &[]);
    let e = ask(&mut run, "the build is red").await;
    assert!(
        matches!(&e, Ending::Failed(why) if why.contains("tests/login.rs") && why.contains("deleted")),
        "{e:?}"
    );
}

#[tokio::test]
async fn a_newly_skipped_test_fails_the_gate() {
    let w = World::new("skipped");
    fs::write(
        w.world().join("auth_test.go"),
        "func TestLogin(t *testing.T) {}\n",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "edit",
            json!({"path": "auth_test.go", "old_string": "testing.T) {}", "new_string": "testing.T) { t.Skip(\"later\") }"}),
        ),
        answer("done"),
        answer("done"),
    ]));
    let mut run = w.run(model, &[]);
    let e = ask(&mut run, "make CI green").await;
    assert!(
        matches!(&e, Ending::Failed(why) if why.contains("skips a test")),
        "{e:?}"
    );
}

#[tokio::test]
async fn an_answer_that_changed_nothing_skips_the_gate() {
    let w = World::new("nothing");
    let model = Arc::new(ScriptedModel::new([answer("the build uses just")]));
    let mut run = w.run(model, &["false"]);
    assert_eq!(
        ask(&mut run, "how is it built?").await,
        Ending::Answered("the build uses just".into())
    );
    assert_eq!(
        gate_beats(&w),
        vec![Vec::<String>::new()],
        "judged, nothing changed, nothing failed"
    );
}

/// Solo holds no outward act: the shelf refuses it with the lawful route.
#[tokio::test]
async fn solo_cannot_reach_outside() {
    let w = World::new("outward");
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "bash", json!({"command": "git push origin main"})),
        answer("needs a push"),
    ]));
    let mut run = w.run(model.clone(), &[]);
    ask(&mut run, "ship it").await;
    let refused = model.requests()[1].messages.iter().flat_map(|m| &m.content).any(|b| {
        matches!(b, Block::ToolResult { content, is_error: true, .. } if content.contains("not on the slime shelf") && content.contains("kijin"))
    });
    assert!(refused);
}

/// `.only(` runs one test and silently skips the rest.
#[tokio::test]
async fn a_focused_test_fails_the_gate() {
    let w = World::new("only");
    fs::write(
        w.world().join("login.test.js"),
        "describe('login', () => { it('works', () => {}) })\n",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "edit",
            json!({"path": "login.test.js", "old_string": "describe(", "new_string": "describe.only("}),
        ),
        answer("done"),
        answer("done"),
    ]));
    let mut run = w.run(model, &[]);
    assert!(
        matches!(ask(&mut run, "focus").await, Ending::Failed(why) if why.contains("skips a test"))
    );
}

/// A large test file is read whole: padding it past any cap hides nothing.
#[tokio::test]
async fn a_large_test_file_is_read_whole() {
    let w = World::new("large");
    let filler = "// filler\n".repeat(40_000);
    fs::write(
        w.world().join("big_test.rs"),
        format!("{filler}#[test]\nfn a() {{}}\n"),
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "edit",
            json!({"path": "big_test.rs", "old_string": "#[test]\nfn a", "new_string": "#[test]\n#[ignore]\nfn a"}),
        ),
        answer("done"),
        answer("done"),
    ]));
    let mut run = w.run(model, &[]);
    assert!(
        matches!(ask(&mut run, "quiet it").await, Ending::Failed(why) if why.contains("skips a test"))
    );
}

/// Deleting a test from a file is deleting a test.
#[tokio::test]
async fn a_removed_test_function_fails_the_gate() {
    let w = World::new("removed");
    fs::write(
        w.world().join("auth_test.rs"),
        "#[test]\nfn a() {}\n#[test]\nfn b() {}\n",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "edit",
            json!({"path": "auth_test.rs", "old_string": "#[test]\nfn b() {}\n", "new_string": ""}),
        ),
        answer("done"),
        answer("done"),
    ]));
    let mut run = w.run(model, &[]);
    assert!(
        matches!(ask(&mut run, "trim").await, Ending::Failed(why) if why.contains("has 1 tests, it had 2"))
    );
}

/// A run-ending failure reconsolidates what the ask recalled: a grounded revise note per failed
/// act links the recalled note — 0.9, since the body's answer named it — and a first failure alone
/// writes nothing.
#[tokio::test]
async fn a_run_ending_failure_revises_the_note_it_rode_on() {
    use mnemorph_memory::{Polarity, Status, Tier, fold, read};
    let w = World::new("reconsolidate");
    let claim = mnemorph_memory::propose(
        &w.world(),
        "slime-other",
        "out.txt should say broken when the build is red",
        vec![],
        None,
    )
    .unwrap();
    let note = mnemorph_memory::ground(
        &w.world(),
        claim,
        &mnemorph_memory::Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "write",
            json!({"path": "out.txt", "content": "broken\n"}),
        ),
        answer("done"),
        tool(
            "t2",
            "write",
            json!({"path": "out.txt", "content": "still broken\n"}),
        ),
        answer(&format!("done, as {note} says")),
    ]));
    let mut run = w.run(model.clone(), &["grep -q fixed out.txt"]);
    let ending = ask(&mut run, "make out.txt say broken, the build is red").await;
    assert!(matches!(ending, Ending::Failed(_)), "{ending:?}");
    let shared = fold(&read(&w.world(), &Tier::Shared).unwrap().rows);
    let revises: Vec<_> = shared
        .values()
        .filter(|n| n.polarity == Polarity::Revise)
        .collect();
    assert_eq!(revises.len(), 1, "one per failed act: {revises:?}");
    let r = revises[0];
    assert_eq!(r.status, Status::Grounded);
    assert_eq!(r.links.len(), 1);
    assert_eq!(r.links[0].note, note);
    assert!(
        (r.links[0].weight - 0.9).abs() < 1e-9,
        "cited by the answer"
    );
    assert!(
        r.proof.as_deref().unwrap().contains("gate: FAIL"),
        "{:?}",
        r.proof
    );
}
