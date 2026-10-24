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

fn reply(content: Vec<Block>, stop: StopReason) -> Result<Response, Error> {
    Ok(Response {
        id: "m".into(),
        model: "claude-sonnet-5-5".into(),
        content,
        stop,
        stop_details: None,
        usage: usage(100),
        warnings: vec![],
    })
}

/// An empty reply is common on OpenAI-compatible servers: it is not an answer, and it must not
/// poison the conversation for the asks after it.
#[tokio::test]
async fn an_empty_reply_is_nudged_and_the_session_lives_on() {
    let w = World::new("empty");
    let model = Arc::new(ScriptedModel::new([
        reply(vec![], StopReason::EndTurn),
        reply(
            vec![Block::Text {
                text: String::new(),
                cache: false,
            }],
            StopReason::EndTurn,
        ),
        answer("here"),
        answer("again"),
    ]));
    let mut run = w.run(model.clone(), limits());
    assert_eq!(ask(&mut run, "one").await, Ending::Answered("here".into()));
    assert_eq!(ask(&mut run, "two").await, Ending::Answered("again".into()));
    for r in model.requests() {
        mnemorph_loop::validate(&r.messages).unwrap();
    }
}

#[tokio::test]
async fn a_reply_cut_by_the_ceiling_is_continued() {
    let w = World::new("maxtokens");
    let model = Arc::new(ScriptedModel::new([
        reply(
            vec![Block::Text {
                text: "I will now ed".into(),
                cache: false,
            }],
            StopReason::MaxTokens,
        ),
        answer("edited"),
    ]));
    let mut run = w.run(model, limits());
    assert_eq!(
        ask(&mut run, "edit").await,
        Ending::Answered("edited".into())
    );
}

/// The stress instruments belong to an ask: a session of questions is not thrash.
#[tokio::test]
async fn stress_starts_again_with_each_ask() {
    let w = World::new("per-ask");
    fs::write(w.world().join("a.txt"), "a\n").unwrap();
    let script = (0..5).flat_map(|i| {
        [
            tool(&format!("t{i}"), "read", json!({"path": "a.txt"})),
            answer("a"),
        ]
    });
    let mut run = w.run(Arc::new(ScriptedModel::new(script)), limits());
    for _ in 0..5 {
        assert_eq!(
            ask(&mut run, "what is in a.txt?").await,
            Ending::Answered("a".into())
        );
    }
}

/// An Esc that landed after the last answer does not interrupt the next ask.
#[tokio::test]
async fn a_stale_esc_does_not_interrupt_the_next_ask() {
    let w = World::new("stale-esc");
    let mut run = w.run(
        Arc::new(ScriptedModel::new([answer("one"), answer("two")])),
        limits(),
    );
    assert_eq!(ask(&mut run, "first").await, Ending::Answered("one".into()));
    run.interrupt().fire();
    assert_eq!(
        ask(&mut run, "second").await,
        Ending::Answered("two".into())
    );
}

/// A budget checkpoint resumes knowing what was spent: no call it already could not afford.
#[tokio::test]
async fn resume_remembers_the_spend() {
    let w = World::new("resume-spend");
    let first = Arc::new(ScriptedModel::new([
        tool("t1", "glob", json!({"pattern": "*"})),
        answer("never"),
    ]));
    let mut run = w.run(
        first,
        Limits {
            budget_wt: Some(1),
            ..limits()
        },
    );
    assert!(
        matches!(ask(&mut run, "spend").await, Ending::Checkpoint { why, .. } if why.starts_with("budget"))
    );
    let second = Arc::new(ScriptedModel::new([answer("should not be called")]));
    let mut run = Run::resume(
        body(),
        Limits {
            budget_wt: Some(1),
            ..limits()
        },
        w.setup(second.clone()),
    )
    .unwrap();
    assert!(
        matches!(ask(&mut run, "go on").await, Ending::Checkpoint { why, .. } if why.starts_with("budget"))
    );
    assert!(second.requests().is_empty(), "no call past the budget");
}

#[tokio::test]
async fn a_repeated_tool_id_is_made_unique() {
    let w = World::new("dup-ids");
    let calls = vec![
        Block::ToolUse {
            id: "x".into(),
            name: "glob".into(),
            input: json!({"pattern": "*"}),
        },
        Block::ToolUse {
            id: "x".into(),
            name: "glob".into(),
            input: json!({"pattern": "*"}),
        },
    ];
    let model = Arc::new(ScriptedModel::new([
        reply(calls, StopReason::ToolUse),
        answer("ok"),
    ]));
    let mut run = w.run(model.clone(), limits());
    assert_eq!(ask(&mut run, "twice").await, Ending::Answered("ok".into()));
    mnemorph_loop::validate(&model.requests()[1].messages).unwrap();
}

/// Thinking travels as an opaque block: the journal keeps it whole so a resume sends it back
/// unchanged.
#[tokio::test]
async fn opaque_blocks_survive_the_journal() {
    let w = World::new("opaque");
    let thinking =
        Block::Opaque(json!({"type": "thinking", "thinking": "plan", "signature": "sig=="}));
    let model = Arc::new(ScriptedModel::new([
        reply(
            vec![
                thinking.clone(),
                Block::ToolUse {
                    id: "t1".into(),
                    name: "glob".into(),
                    input: json!({"pattern": "*"}),
                },
            ],
            StopReason::ToolUse,
        ),
        answer("never"),
    ]));
    let mut run = w.run(
        model,
        Limits {
            steps: 1,
            ..limits()
        },
    );
    ask(&mut run, "think").await;
    let rebuilt = conversation(&w.journal().read().unwrap().rows);
    assert_eq!(rebuilt[1].content[0], thinking);
}

/// Esc during a long command ends the turn at once, not when the command would have finished.
#[tokio::test]
async fn esc_reaches_a_running_command() {
    let w = World::new("esc-command");
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "bash", json!({"command": "touch running; sleep 30"})),
        answer("never"),
    ]));
    let mut run = w.run(model.clone(), limits());
    fire_once_running(run.interrupt(), w.world().join("running"));
    let started = std::time::Instant::now();
    assert_eq!(ask(&mut run, "wait").await, Ending::Interrupted);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(model.requests().len(), 1, "no turn after the interrupt");
}

/// The clock reaches a running command; the salvage turn follows at once.
#[tokio::test]
async fn the_clock_reaches_a_running_command() {
    let w = World::new("clock-command");
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "bash", json!({"command": "sleep 30"})),
        answer("I was waiting on sleep."),
    ]));
    let mut run = w.run(
        model,
        Limits {
            deadline: Duration::from_millis(500),
            ..limits()
        },
    );
    let started = std::time::Instant::now();
    let e = ask(&mut run, "wait").await;
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
    assert!(
        matches!(&e, Ending::Checkpoint { why, salvage: Some(_) } if why.starts_with("clock")),
        "{e:?}"
    );
}

/// The same call with the same input, again and again: warned at three, stopped at five.
#[tokio::test]
async fn an_identical_call_loop_is_broken() {
    let w = World::new("loop-breaker");
    let model =
        Arc::new(ScriptedModel::new((0..6).map(|i| {
            tool(&format!("t{i}"), "glob", json!({"pattern": "*.rs"}))
        })));
    let mut run = w.run(model.clone(), limits());
    let e = ask(&mut run, "find it").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why == "stall: loop"),
        "{e:?}"
    );
    let warned = model.requests()[3]
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, Block::Text { text, .. } if text.starts_with("[loop]")));
    assert!(warned);
}

/// A bench whose runs all hide their exit status stalls the run, however much lands between.
#[tokio::test]
async fn a_muted_bench_stalls() {
    let w = World::new("bench-muted");
    let mut script = Vec::new();
    for i in 0..9 {
        script.push(tool(
            &format!("b{i}"),
            "bash",
            json!({"command": "pytest -q | tail -1"}),
        ));
        script.push(tool(
            &format!("w{i}"),
            "write",
            json!({"path": format!("fix{i}.py"), "content": "x = 1\n"}),
        ));
    }
    let mut run = w.run(
        Arc::new(ScriptedModel::new(script)),
        Limits {
            steps: 60,
            ..limits()
        },
    );
    let e = ask(&mut run, "make the tests pass").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why == "stall: bench"),
        "{e:?}"
    );
}

/// Red runs with fixes landing between them are test-driven work, not thrash.
#[tokio::test]
async fn red_runs_with_fixes_between_them_do_not_stall() {
    let w = World::new("bench-tdd");
    let mut script = Vec::new();
    for i in 0..14 {
        script.push(tool(
            &format!("b{i}"),
            "bash",
            json!({"command": "pytest -q"}),
        ));
        script.push(tool(
            &format!("w{i}"),
            "write",
            json!({"path": format!("fix{i}.py"), "content": "x = 1\n"}),
        ));
    }
    script.push(answer("still red, here is what I learned"));
    let mut run = w.run(
        Arc::new(ScriptedModel::new(script)),
        Limits {
            steps: 60,
            ..limits()
        },
    );
    assert!(matches!(
        ask(&mut run, "make the tests pass").await,
        Ending::Answered(_)
    ));
}

#[test]
fn benches_are_known_by_their_command() {
    use mnemorph_loop::bench_of;
    assert!(bench_of("cargo test -p x").is_some());
    assert!(bench_of("cd sub && npm test").is_some());
    assert_eq!(
        bench_of("pytest -q | tail -3"),
        bench_of("pytest -q"),
        "piped or not, the same bench"
    );
    assert!(bench_of("RUST_BACKTRACE=1 cargo test").is_some());
    assert!(bench_of("timeout 600 cargo test").is_some());
    assert!(bench_of("uv run pytest").is_some());
    assert!(bench_of("cargo test || true").is_some());
    assert!(bench_of("cargo build").is_none());
    assert!(bench_of("echo testing").is_none());
}

/// #43: the cached prefix — the system prompt and the tool JSON — has a ceiling, checked in CI.
#[test]
fn the_solo_prefix_stays_under_its_ceiling() {
    const CEILING_TOKENS: usize = 4000;
    let w = World::new("prefix-ceiling");
    let (body, _) = mnemorph_loop::solo(&w.world(), "m");
    let tools: Vec<mnemorph_provider::Tool> = registry()
        .iter()
        .map(|t| mnemorph_provider::Tool {
            name: t.name().into(),
            description: t.description().into(),
            input_schema: t.schema(),
            cache: false,
        })
        .collect();
    let bytes = body.system.len() + serde_json::to_string(&tools.iter().map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.input_schema})).collect::<Vec<_>>()).unwrap().len();
    let tokens = bytes / 4;
    eprintln!("solo prefix: {bytes} bytes ≈ {tokens} tokens (ceiling {CEILING_TOKENS})");
    assert!(
        tokens <= CEILING_TOKENS,
        "the solo prefix grew to ≈{tokens} tokens"
    );
}

/// Esc during a verify command is not a verdict: the ask ends interrupted, with no failure
/// recorded and no "fix it" note.
#[tokio::test]
async fn esc_during_a_verify_is_not_a_verdict() {
    let w = World::new("esc-verify");
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "write", json!({"path": "a.txt", "content": "x"})),
        answer("done"),
        answer("never"),
    ]));
    let mut run = w
        .run(model.clone(), limits())
        .with_gate(mnemorph_loop::EndGate {
            verify: vec!["touch running; sleep 30".into()],
        });
    fire_once_running(run.interrupt(), w.world().join("running"));
    assert_eq!(ask(&mut run, "write a").await, Ending::Interrupted);
    let beats = w.journal().read().unwrap().rows;
    assert!(
        !beats.iter().any(|b| matches!(b, Beat::Gate { .. })),
        "no verdict was recorded"
    );
    assert_eq!(
        model.requests().len(),
        2,
        "no turn to 'fix' an interrupted gate"
    );
}

/// The human's configuration reaches the run.
#[test]
fn the_configuration_reaches_the_run() {
    let w = World::new("configure");
    let root = w.root.join("cfg");
    fs::create_dir_all(w.world().join(".mnemorph")).unwrap();
    fs::write(
        w.world().join(".mnemorph/config.local.toml"),
        "[stress]\ncontext_tokens = 90000\n[gate]\nask_rules = [\"in-place-edit\"]\n",
    )
    .unwrap();
    let config = mnemorph_config::Config::load(&mnemorph_config::Paths::with_global(
        root.join("none.toml"),
        &w.world(),
    ))
    .unwrap();
    let (body, cx) = mnemorph_loop::solo(&w.world(), "m");
    let (cx, limits) = mnemorph_loop::configure(&config, cx, Limits::for_race(body.race));
    assert_eq!(limits.stress_tokens, 90_000);
    assert!(cx.asks(mnemorph_tools::Rule::InPlaceEdit));
}

/// Fire Esc once the command has started — marked by the file it touches — not on a timer that
/// a slow start would outrun.
fn fire_once_running(interrupt: Arc<mnemorph_loop::Interrupt>, marker: std::path::PathBuf) {
    std::thread::spawn(move || {
        while !marker.exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
        interrupt.fire();
    });
}

/// The day's cap holds across the sessions that share a world: the run rereads the ledger before
/// every call, so a second session finds the day spent and makes no call at all.
#[tokio::test]
async fn the_days_cap_holds_across_sessions() {
    let w = World::new("caps-across");
    // What one scripted call weighs, measured.
    let probe = Arc::new(ScriptedModel::new([answer("x")]));
    ask(&mut w.run(probe, limits()), "weigh").await;
    let one = w.ledger().read().unwrap().rows[0].wt;
    let caps = mnemorph_ledger::Caps {
        day: Some(3 * one),
        ..mnemorph_ledger::Caps::default()
    };
    let a = Arc::new(ScriptedModel::new([
        tool("t1", "glob", json!({"pattern": "*.x"})),
        tool("t2", "glob", json!({"pattern": "*.y"})),
        tool("t3", "glob", json!({"pattern": "*.z"})),
        answer("never"),
    ]));
    let e = ask(&mut w.run(a.clone(), limits()).with_caps(caps), "spend").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why.starts_with("budget: the day cap is spent")),
        "{e:?}"
    );
    assert_eq!(
        a.requests().len(),
        2,
        "the first call's spend, and the second's, fill the day"
    );
    let b = Arc::new(ScriptedModel::new([answer("never")]));
    let e = ask(&mut w.run(b.clone(), limits()).with_caps(caps), "again").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why.starts_with("budget: the day cap is spent")),
        "{e:?}"
    );
    assert!(
        b.requests().is_empty(),
        "a second session spends nothing once the day is spent"
    );
}

/// At 80% of a cap the human is warned, once; a ledger that cannot be read stops the run.
#[tokio::test]
async fn a_cap_warns_at_eighty_percent_and_an_unreadable_ledger_stops() {
    let w = World::new("caps-warn");
    let probe = Arc::new(ScriptedModel::new([answer("x")]));
    ask(&mut w.run(probe, limits()), "weigh").await;
    let one = w.ledger().read().unwrap().rows[0].wt;
    let caps = mnemorph_ledger::Caps {
        day: Some(one * 5 / 4),
        ..mnemorph_ledger::Caps::default()
    };
    let m = Arc::new(ScriptedModel::new([answer("fine")]));
    assert!(matches!(
        ask(&mut w.run(m, limits()).with_caps(caps), "go").await,
        Ending::Answered(_)
    ));
    let warned = w
        .journal()
        .read()
        .unwrap()
        .rows
        .iter()
        .filter(|b| matches!(b, Beat::Warning { text } if text.contains("of the day cap")))
        .count();
    assert_eq!(warned, 1);
    // The ledger becomes a directory: unreadable.
    let path = w.ledger().path().to_path_buf();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    let m = Arc::new(ScriptedModel::new([answer("never")]));
    let e = ask(
        &mut w.run(m.clone(), limits()).with_caps(mnemorph_ledger::Caps {
            day: Some(u64::MAX),
            ..caps
        }),
        "go",
    )
    .await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why.contains("could not be read")),
        "{e:?}"
    );
    assert_eq!(m.requests(), [] as [mnemorph_provider::Request; 0]);
}

/// A line sent while the body works reaches it after its next tool step, marked as the human's.
#[tokio::test]
async fn a_sent_line_reaches_the_body_at_its_next_tool_step() {
    let w = World::new("send");
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "glob", json!({"pattern": "*.x"})),
        answer("steered"),
    ]));
    let mut run = w.run(model.clone(), limits());
    let lane = run.steer();
    let mut on = |e: mnemorph_provider::Event| {
        // The human sends while the first call is being made.
        if matches!(e, mnemorph_provider::Event::ToolStart { .. }) {
            lane.lock()
                .unwrap()
                .push_back("use the tests folder".into());
        }
    };
    let e = run.ask("look", &mut on).await;
    assert!(matches!(e, Ending::Answered(_)), "{e:?}");
    let second = &model.requests()[1];
    let said = second.messages.last().unwrap().content.iter().any(|b| {
        matches!(b, Block::Text { text, .. } if text == &format!("{} use the tests folder", mnemorph_loop::SENT))
    });
    assert!(said, "the body read it after its tool step");
}

/// A line sent to a turn that ends without another tool step is said to the human, and never
/// carried into the next turn.
#[tokio::test]
async fn a_sent_line_the_turn_never_reached_is_reported_not_smuggled() {
    let w = World::new("send-late");
    let model = Arc::new(ScriptedModel::new([answer("done"), answer("next")]));
    let mut run = w.run(model.clone(), limits());
    ask(&mut run, "first").await;
    run.steer().lock().unwrap().push_back("too late".into());
    ask(&mut run, "second").await;
    let beats = w.journal().read().unwrap().rows;
    assert!(beats.iter().any(|b| matches!(b, Beat::Warning { text } if text.contains("not delivered") && text.contains("too late"))));
    let second = &model.requests()[1];
    assert!(
        !format!("{:?}", second.messages).contains("too late"),
        "never in the next turn"
    );
}

/// A line sent while the last model call runs is said with that turn's end, not the next's.
#[tokio::test]
async fn a_late_line_is_reported_when_its_turn_ends() {
    let w = World::new("send-tail");
    let model = Arc::new(ScriptedModel::new([answer("done")]));
    let mut run = w.run(model, limits());
    let lane = run.steer();
    let mut on = |e: mnemorph_provider::Event| {
        if matches!(e, mnemorph_provider::Event::Text(_)) {
            lane.lock().unwrap().push_back("too late".into());
        }
    };
    run.ask("go", &mut on).await;
    let beats = w.journal().read().unwrap().rows;
    let warned = beats
        .iter()
        .position(|b| matches!(b, Beat::Warning { text } if text.contains("too late")));
    let ended = beats.iter().position(|b| matches!(b, Beat::End { .. }));
    assert!(
        matches!((warned, ended), (Some(a), Some(b)) if a < b),
        "{beats:?}"
    );
}
