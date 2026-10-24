//! The loop against a scripted model: every meter, every instrument, every ending.

#![allow(
    clippy::unnecessary_wraps,
    reason = "scripted replies are Results: the script also holds errors"
)]

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mnemorph_ledger::{Journal, Usage, UsageRow};
use mnemorph_loop::{Beat, Body, Ending, Limits, Run, ScriptedModel, Setup, conversation};
use mnemorph_provider::{
    Block, Caller, Error, Response, Role, StopReason, anthropic_prices, encode,
};
use mnemorph_races::Race;
use mnemorph_tools::{Context, Sandbox, registry};
use serde_json::{Value, json};

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-loop-{name}-{}", std::process::id()));
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

    fn ledger(&self) -> Journal<UsageRow> {
        Journal::new(self.root.join("usage.jsonl"))
    }

    fn run(&self, model: Arc<ScriptedModel>, limits: Limits) -> Run {
        Run::new(body(), limits, self.setup(model))
    }

    fn setup(&self, model: Arc<ScriptedModel>) -> Setup {
        let cx = Context::new(Race::Slime, self.world(), &["."])
            .with_sandbox(Sandbox::None("loop tests run unguarded".into()))
            .with_unguarded(true);
        Setup {
            model,
            tools: registry(),
            cx,
            journal: self.journal(),
            ledger: Some(self.ledger()),
            prices: anthropic_prices().unwrap(),
            caller: Caller {
                session: "s".into(),
                run: "r".into(),
                body: "slime-test".into(),
                race: "slime".into(),
                commission: None,
                prefix: 0,
            },
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn body() -> Body {
    Body {
        name: "slime-test".into(),
        race: Race::Slime,
        model: "claude-sonnet-5-5".into(),
        system: "You are a test body.".into(),
    }
}

fn limits() -> Limits {
    Limits::for_race(Race::Slime)
}

fn usage(input: u64) -> Usage {
    Usage {
        input,
        output: 20,
        cache_read: 0,
        cache_write: 0,
    }
}

fn answer(text: &str) -> Result<Response, Error> {
    Ok(Response {
        id: "m".into(),
        model: "claude-sonnet-5-5".into(),
        content: vec![Block::Text {
            text: text.into(),
            cache: false,
        }],
        stop: StopReason::EndTurn,
        stop_details: None,
        usage: usage(100),
        warnings: vec![],
    })
}

fn tool(id: &str, name: &str, input: Value) -> Result<Response, Error> {
    tool_with(id, name, input, usage(100))
}

fn tool_with(id: &str, name: &str, input: Value, usage: Usage) -> Result<Response, Error> {
    Ok(Response {
        id: "m".into(),
        model: "claude-sonnet-5-5".into(),
        content: vec![Block::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }],
        stop: StopReason::ToolUse,
        stop_details: None,
        usage,
        warnings: vec![],
    })
}

async fn ask(run: &mut Run, text: &str) -> Ending {
    let mut quiet = |_| {};
    run.ask(text, &mut quiet).await
}

fn results(model: &ScriptedModel, request: usize) -> Vec<(String, bool)> {
    model.requests()[request]
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|b| match b {
            Block::ToolResult {
                content, is_error, ..
            } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_direct_answer_is_journaled_and_priced() {
    let w = World::new("answer");
    let model = Arc::new(ScriptedModel::new([answer("hello")]));
    let mut run = w.run(model, limits());
    assert_eq!(ask(&mut run, "hi").await, Ending::Answered("hello".into()));
    let beats = w.journal().read().unwrap().rows;
    assert!(
        matches!(beats[0], Beat::Ask { .. })
            && matches!(beats[1], Beat::Model { .. })
            && matches!(beats[2], Beat::End { .. })
    );
    let rows = w.ledger().read().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].usage, usage(100));
    assert!(rows[0].usd.is_some(), "a priced model gets a dollar figure");
}

#[tokio::test]
async fn a_tool_round_trip_reaches_the_answer() {
    let w = World::new("roundtrip");
    fs::write(w.world().join("notes.md"), "the answer is 42\n").unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "read", json!({"path": "notes.md"})),
        answer("42"),
    ]));
    let mut run = w.run(model.clone(), limits());
    assert_eq!(
        ask(&mut run, "what is the answer?").await,
        Ending::Answered("42".into())
    );
    let r = results(&model, 1);
    assert!(r[0].0.contains("the answer is 42") && !r[0].1, "{r:?}");
    assert_eq!(run.steps(), 2);
}

#[tokio::test]
async fn the_steps_meter_checkpoints() {
    let w = World::new("steps");
    let model =
        Arc::new(ScriptedModel::new((0..5).map(|i| {
            tool(&format!("t{i}"), "glob", json!({"pattern": "*"}))
        })));
    let mut run = w.run(
        model,
        Limits {
            steps: 2,
            ..limits()
        },
    );
    let e = ask(&mut run, "loop").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why.starts_with("steps")),
        "{e:?}"
    );
}

#[tokio::test]
async fn three_failed_acts_escalate() {
    let w = World::new("escalate");
    let model =
        Arc::new(ScriptedModel::new((0..5).map(|i| {
            tool(&format!("t{i}"), "read", json!({"path": "missing.txt"}))
        })));
    let mut run = w.run(model, limits());
    assert!(matches!(
        ask(&mut run, "read it").await,
        Ending::Escalate(_)
    ));
    assert_eq!(run.steps(), 3);
}

/// Guts warns inside the conversation, and three windows without a landing end the run.
#[tokio::test]
async fn guts_warns_then_doom_stops_the_run() {
    let w = World::new("doom");
    fs::write(w.world().join("a.txt"), "a\n").unwrap();
    // A window of -3 each time, never three failures in a row.
    let script = (0..15).map(|i| {
        // Each window of five: fail, plain, fail, plain, fail.
        let fail = i % 5 % 2 == 0;
        let path = if fail { "missing.txt" } else { "." };
        if fail {
            tool(&format!("t{i}"), "read", json!({"path": path}))
        } else {
            tool(&format!("t{i}"), "glob", json!({"pattern": "*.nothing"}))
        }
    });
    let model = Arc::new(ScriptedModel::new(script));
    let mut run = w.run(model.clone(), limits());
    let e = ask(&mut run, "flail").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why.starts_with("doom")),
        "{e:?}"
    );
    let warned = model.requests()[5]
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, Block::Text { text, .. } if text.starts_with("[guts]")));
    assert!(warned, "the warning reaches the body");
}

/// Every relief resets its own trigger: a landing clears the warnings.
#[tokio::test]
async fn a_landing_resets_guts() {
    let w = World::new("landing");
    let mut script: Vec<_> = Vec::new();
    for round in 0..3 {
        for i in 0..5 {
            let id = format!("r{round}-{i}");
            script.push(if i % 2 == 0 {
                tool(&id, "read", json!({"path": "missing.txt"}))
            } else {
                tool(&id, "glob", json!({"pattern": "*.x"}))
            });
        }
        script.push(tool(
            &format!("w{round}"),
            "write",
            json!({"path": format!("out{round}.txt"), "content": "x"}),
        ));
    }
    script.push(answer("done"));
    let model = Arc::new(ScriptedModel::new(script));
    let mut run = w.run(model, limits());
    assert_eq!(ask(&mut run, "work").await, Ending::Answered("done".into()));
}

#[tokio::test]
async fn re_reading_an_unchanged_window_stalls() {
    let w = World::new("repin");
    fs::write(w.world().join("a.txt"), "a\n").unwrap();
    let model =
        Arc::new(ScriptedModel::new((0..6).map(|i| {
            tool(&format!("t{i}"), "read", json!({"path": "a.txt"}))
        })));
    let mut run = w.run(model, limits());
    let e = ask(&mut run, "stare").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why == "stall: repin"),
        "{e:?}"
    );
    assert_eq!(run.steps(), 4, "one read, then three unchanged");
}

/// A clock death gets one short turn, without tools, to say what it learned.
#[tokio::test]
async fn a_clock_death_salvages() {
    let w = World::new("clock");
    let model = Arc::new(ScriptedModel::new([answer(
        "I read the plan; the migration is left.",
    )]));
    let mut run = w.run(
        model.clone(),
        Limits {
            deadline: Duration::ZERO,
            ..limits()
        },
    );
    let e = ask(&mut run, "work").await;
    assert_eq!(
        e,
        Ending::Checkpoint {
            why: "clock: the deadline passed between turns".into(),
            salvage: Some("I read the plan; the migration is left.".into())
        }
    );
    let r = &model.requests()[0];
    assert_eq!(r.tools, [] as [mnemorph_provider::Tool; 0]);
    assert_eq!(r.max_tokens, 4096);
}

#[tokio::test]
async fn context_stress_checkpoints() {
    let w = World::new("context");
    let model = Arc::new(ScriptedModel::new([
        tool_with("t1", "glob", json!({"pattern": "*"}), usage(181_000)),
        answer("never"),
    ]));
    let mut run = w.run(model, limits());
    let e = ask(&mut run, "big").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why.starts_with("context")),
        "{e:?}"
    );
}

#[tokio::test]
async fn the_budget_checkpoints() {
    let w = World::new("budget");
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "glob", json!({"pattern": "*"})),
        answer("never"),
    ]));
    let mut run = w.run(
        model,
        Limits {
            budget_wt: Some(1),
            ..limits()
        },
    );
    let e = ask(&mut run, "spend").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why.starts_with("budget")),
        "{e:?}"
    );
}

#[tokio::test]
async fn a_refusal_fails_the_run() {
    let w = World::new("refusal");
    let refused = Ok(Response {
        id: "m".into(),
        model: "claude-sonnet-5-5".into(),
        content: vec![],
        stop: StopReason::Refusal,
        stop_details: Some(json!({"category": "cyber"})),
        usage: usage(10),
        warnings: vec![],
    });
    let mut run = w.run(Arc::new(ScriptedModel::new([refused])), limits());
    assert!(matches!(ask(&mut run, "x").await, Ending::Failed(why) if why.contains("cyber")));
}

/// A checkpointed run resumes from its journal, the steps it took still counted.
#[tokio::test]
async fn a_run_resumes_from_its_journal() {
    let w = World::new("resume");
    fs::write(w.world().join("a.txt"), "alpha\n").unwrap();
    let first = Arc::new(ScriptedModel::new([
        tool("t1", "read", json!({"path": "a.txt"})),
        answer("never"),
    ]));
    let mut run = w.run(
        first,
        Limits {
            steps: 1,
            ..limits()
        },
    );
    assert!(matches!(
        ask(&mut run, "read a").await,
        Ending::Checkpoint { .. }
    ));

    let second = Arc::new(ScriptedModel::new([answer("alpha")]));
    let mut run = Run::resume(body(), limits(), w.setup(second.clone())).unwrap();
    assert_eq!(run.steps(), 1);
    assert_eq!(
        ask(&mut run, "go on").await,
        Ending::Answered("alpha".into())
    );
    let sent = &second.requests()[0].messages;
    assert!(
        results(&second, 0)[0].0.contains("alpha"),
        "the earlier result came back"
    );
    assert_eq!(sent.last().unwrap().role, Role::User);
}

/// An act the journal shows no answer for may not have run: it is answered with an error, never
/// replayed.
#[test]
fn a_call_cut_mid_flight_is_answered_not_replayed() {
    let beats = vec![
        Beat::Ask {
            text: "push it".into(),
        },
        Beat::Model {
            content: vec![
                json!({"type": "tool_use", "id": "t1", "name": "bash", "input": {"command": "git push"}}),
            ],
            stop: "ToolUse".into(),
        },
    ];
    let messages = conversation(&beats);
    let last = messages.last().unwrap();
    assert_eq!(last.role, Role::User);
    assert!(
        matches!(&last.content[0], Block::ToolResult { tool_use_id, is_error: true, content, .. } if tool_use_id == "t1" && content.contains("may not have run"))
    );
}

/// The cached prefix — system prompt and tool JSON — is byte-identical turn after turn.
#[tokio::test]
async fn the_cached_prefix_is_stable() {
    let w = World::new("prefix");
    fs::write(w.world().join("a.txt"), "a\n").unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "read", json!({"path": "a.txt"})),
        tool("t2", "glob", json!({"pattern": "*"})),
        answer("ok"),
    ]));
    let mut run = w.run(model.clone(), limits());
    ask(&mut run, "go").await;
    let encoded: Vec<Value> = model
        .requests()
        .iter()
        .map(|r| encode(r).unwrap())
        .collect();
    for e in &encoded[1..] {
        assert_eq!(e["system"], encoded[0]["system"]);
        assert_eq!(e["tools"], encoded[0]["tools"]);
    }
    let tools = encoded[0]["tools"].as_array().unwrap();
    assert!(
        tools.last().unwrap().get("cache_control").is_some(),
        "the prefix ends cached"
    );
}
