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
        matches!(&beats[0], Beat::Rides { model } if model == "claude-sonnet-5-5")
            && matches!(beats[1], Beat::Ask { .. })
            && matches!(beats[2], Beat::Model { .. })
            && matches!(beats[3], Beat::End { .. }),
        "{beats:?}"
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
async fn a_manifest_read_is_sighted_by_the_harness_and_the_sandbox_stays_as_it_was() {
    use mnemorph_world::dimension::{load_sightings, register_at, sightings_file, world_id};
    let w = World::new("sighting");
    let neighbour = w.root.join("neighbour");
    fs::create_dir_all(&neighbour).unwrap();
    fs::write(neighbour.join("secret.txt"), "theirs\n").unwrap();
    let registry = w.root.join("state/worlds.jsonl");
    register_at(&registry, &w.world(), None).unwrap();
    register_at(&registry, &neighbour, None).unwrap();
    fs::write(
        w.world().join("Cargo.toml"),
        "[dependencies]\nn = { path = \"../neighbour\" }\n",
    )
    .unwrap();
    // Only a committed manifest is evidence.
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "w",
        ],
    ] {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(w.world())
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok);
    }
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "read", json!({"path": "Cargo.toml"})),
        tool("t2", "read", json!({"path": "../neighbour/secret.txt"})),
        answer("done"),
    ]));
    let mut run = w
        .run(model.clone(), limits())
        .with_registry(Some(registry.clone()));
    assert_eq!(ask(&mut run, "look").await, Ending::Answered("done".into()));
    let seen = load_sightings(&registry, &w.world());
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].body, "slime-test");
    assert_eq!(seen[0].anchor, "Cargo.toml:2");
    assert_eq!(seen[0].to, world_id(&neighbour.canonicalize().unwrap()));
    // A known neighbour is still outside the world: its files are not the body's to read.
    let r = results(&model, 2);
    assert!(r[1].1 && r[1].0.contains("outside the world"), "{r:?}");
    assert!(!r[1].0.contains("theirs"));
    // A run with no registry sights nothing.
    fs::remove_file(sightings_file(&registry, &w.world())).unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "read", json!({"path": "Cargo.toml"})),
        answer("done"),
    ]));
    let mut run = w.run(model, limits());
    ask(&mut run, "look").await;
    assert_eq!(load_sightings(&registry, &w.world()), vec![]);
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

/// A body alone — no lane below, no one above — has no one to hand the turn to: three failed
/// acts in a row warn it to change approach, and the run goes on.
#[tokio::test]
async fn three_failed_acts_alone_are_warned() {
    let w = World::new("escalate-alone");
    let mut replies: Vec<_> = (0..3)
        .map(|i| tool(&format!("t{i}"), "read", json!({"path": "missing.txt"})))
        .collect();
    replies.push(answer("missing.txt does not exist"));
    let model = Arc::new(ScriptedModel::new(replies));
    let mut run = w.run(model.clone(), limits());
    let e = ask(&mut run, "read it").await;
    assert_eq!(e, Ending::Answered("missing.txt does not exist".into()));
    let told = format!("{:?}", model.requests()[3].messages.last());
    assert!(
        told.contains("[escalate] More than two failed acts in a row"),
        "{told}"
    );
}

/// A commissioned body escalates: three failed acts in a row end its turn one hop up.
#[tokio::test]
async fn three_failed_acts_escalate_a_commissioned_body() {
    let w = World::new("escalate");
    let model =
        Arc::new(ScriptedModel::new((0..5).map(|i| {
            tool(&format!("t{i}"), "read", json!({"path": "missing.txt"}))
        })));
    let mut setup = w.setup(model);
    setup.caller.commission = Some("orc-o".into());
    let mut run = Run::new(body(), limits(), setup);
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

/// A re-read of an unchanged window is pointed at once — with where the file goes on — and shown
/// again if asked again: the body is never stuck behind a pointer to text it lost. A body that
/// keeps asking the same thing still stalls, on the identical-call loop.
#[tokio::test]
async fn an_unchanged_re_read_is_pointed_at_once_then_shown() {
    let w = World::new("repin");
    let lines = (1..=3000).fold(String::new(), |mut s, i| {
        use std::fmt::Write as _;
        let _ = writeln!(s, "line {i}");
        s
    });
    fs::write(w.world().join("a.txt"), lines).unwrap();
    let model = Arc::new(ScriptedModel::new((0..6).map(|i| {
        tool(
            &format!("t{i}"),
            "read",
            json!({"path": "a.txt", "offset": 1, "end": 2000}),
        )
    })));
    let mut run = w.run(model.clone(), limits());
    let e = ask(&mut run, "stare").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why == "stall: loop"),
        "{e:?}"
    );
    let result = |i: usize| -> String { format!("{:?}", model.requests()[i].messages.last()) };
    assert!(
        result(2).contains("unchanged since you last read them"),
        "the second read points"
    );
    assert!(
        result(2).contains("The file has 3000 lines: read on with offset 2001"),
        "and says where the file goes on: {}",
        result(2)
    );
    assert!(
        result(3).contains("line 1999"),
        "the third shows it again: {}",
        &result(3)[..200]
    );
}

/// A clock death gets one short turn, told to call no tool, to say what it learned.
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
    assert!(
        !r.tools.is_empty(),
        "the salvage turn sends the tools as any turn does: the cached prefix holds"
    );
    assert_eq!(r.max_tokens, 4096);
}

#[tokio::test]
async fn context_stress_checkpoints() {
    let w = World::new("context");
    let model = Arc::new(ScriptedModel::new([
        tool_with("t1", "glob", json!({"pattern": "*"}), usage(181_000)),
        answer("never"),
    ]));
    let mut run = w.run(
        model,
        Limits {
            compactions: 0,
            ..limits()
        },
    );
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

/// Over many turns and asks — tool calls, then a second ask — every
/// request only appends to the one before: the system and the tools never change, and the run
/// never says its cached prefix moved.
#[tokio::test]
async fn every_request_only_appends_to_the_one_before() {
    let w = World::new("append-only");
    fs::write(w.world().join("a.txt"), "a\n").unwrap();
    let model = Arc::new(ScriptedModel::new([
        tool("t1", "read", json!({"path": "a.txt"})),
        tool("t2", "glob", json!({"pattern": "*"})),
        answer("first"),
        tool("t3", "grep", json!({"pattern": "a"})),
        tool("t4", "read", json!({"path": "a.txt"})),
        answer("second"),
    ]));
    let mut run = w.run(model.clone(), limits());
    ask(&mut run, "go").await;
    ask(&mut run, "and again").await;
    let requests = model.requests();
    assert!(requests.len() >= 6, "{} requests", requests.len());
    let fps = |r: &mnemorph_provider::Request| mnemorph_loop::compact::fingerprints(&r.messages);
    for pair in requests.windows(2) {
        assert_eq!(
            encode(&pair[1]).unwrap()["system"],
            encode(&pair[0]).unwrap()["system"]
        );
        assert_eq!(
            encode(&pair[1]).unwrap()["tools"],
            encode(&pair[0]).unwrap()["tools"]
        );
        assert_eq!(
            mnemorph_loop::compact::moved(&fps(&pair[0]), &fps(&pair[1])),
            None,
            "a request rewrote the one before"
        );
    }
    let warned = w.journal().read().unwrap().rows.iter().any(|b| {
        matches!(b, mnemorph_loop::Beat::Warning { text } if text.contains("cached prefix moved"))
    });
    assert!(!warned, "the run said its prefix moved");
}

/// What the watch catches: a rewritten message, a dropped one; an appended one is fine, and a
/// moved breakpoint is no change.
#[test]
fn a_rewritten_or_dropped_message_moves_the_prefix() {
    use mnemorph_loop::compact::{fingerprints, moved};
    use mnemorph_provider::{Message, Role};
    let text = |t: &str, cache: bool| Block::Text {
        text: t.into(),
        cache,
    };
    let user = |blocks: Vec<Block>| Message {
        role: Role::User,
        content: blocks,
    };
    let before = fingerprints(&[user(vec![text("a", false)]), user(vec![text("b", true)])]);
    // A moved breakpoint and a new message: an append.
    let appended = fingerprints(&[
        user(vec![text("a", true)]),
        user(vec![text("b", false)]),
        user(vec![text("c", true)]),
    ]);
    assert_eq!(moved(&before, &appended), None);
    // A block added to the last message: still an append, as the cache reads it.
    let extended = fingerprints(&[
        user(vec![text("a", false)]),
        user(vec![text("b", false), text("b2", true)]),
    ]);
    assert_eq!(moved(&before, &extended), None);
    // A rewritten message, a dropped one: the prefix moved.
    let rewritten = fingerprints(&[user(vec![text("a", false)]), user(vec![text("B", true)])]);
    assert_eq!(moved(&before, &rewritten), Some(3));
    let dropped = fingerprints(&[user(vec![text("a", false)])]);
    assert_eq!(moved(&before, &dropped), Some(2));
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
            tool(
                &format!("t{i}"),
                "read",
                json!({"path": "a.txt", "offset": 1, "end": 2000}),
            ),
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
fn verification_scripts_count_as_test_runs() {
    use mnemorph_loop::{bench_of, muted};
    // Bodies often check their work with a script, not a test runner: those runs are tests too.
    let inline = r#"python3 -c "from run import f; assert f(2) == 4""#;
    assert_eq!(
        bench_of(inline).as_deref(),
        Some("python3 (an inline check)")
    );
    assert_eq!(
        bench_of("node -e \"const a = require('assert'); a.equal(1, 1); assert(true)\"").as_deref(),
        Some("node (an inline check)")
    );
    let heredoc = "python3 - <<'EOF'\nimport run; assert run.f(2) == 4\nEOF";
    assert_eq!(
        bench_of(heredoc).as_deref(),
        Some("python3 (an inline check)")
    );
    assert_eq!(
        bench_of("python3 tests/check_run.py").as_deref(),
        Some("python3 tests/check_run.py")
    );
    assert_eq!(
        bench_of("bash ./verify.sh").as_deref(),
        Some("bash ./verify.sh")
    );
    // A script's own `;` and `|` are not the shell's: its exit status is seen.
    assert!(!muted(inline) && !muted(heredoc));
    assert!(muted(&format!("{inline} | tail -1")));
    assert!(muted("python3 - <<'EOF' | tail -1\nassert True\nEOF"));
    // A script that asserts nothing, or one not named for checking, is not a test run.
    assert_eq!(bench_of(r#"python3 -c "print(1)""#), None);
    assert_eq!(bench_of(r#"python3 -c "print('assert')""#), None);
    assert_eq!(bench_of(r#"python3 -c "import assertions""#), None);
    assert_eq!(bench_of("python3 run.py"), None);
    assert_eq!(bench_of("node build.js"), None);
    // A test runner wrapped in a shell is still not read through its wrapper.
    assert_eq!(bench_of(r#"bash -c "cargo test""#), None);
}

/// A body that checks with a one-liner after every fix is testing, not stalling: a passing check
/// is green, a failing one red, and the stall rules read them as they read any test run.
#[tokio::test]
async fn inline_checks_count_green_and_red_and_do_not_stall() {
    let w = World::new("inline-checks");
    let mut script = Vec::new();
    for i in 0..9 {
        script.push(tool(
            &format!("c{i}"),
            "bash",
            json!({"command": format!("python3 -c \"import os; assert {i} == {i}\"")}),
        ));
    }
    script.push(tool(
        "red",
        "bash",
        json!({"command": "python3 -c \"import os; assert os.sep == '?'\""}),
    ));
    script.push(answer("checked"));
    let mut run = w.run(
        Arc::new(ScriptedModel::new(script)),
        Limits {
            steps: 30,
            ..limits()
        },
    );
    let e = ask(&mut run, "check it").await;
    assert!(matches!(e, Ending::Answered(_)), "{e:?}");
    let outcomes: Vec<String> = w
        .journal()
        .read()
        .unwrap()
        .rows
        .into_iter()
        .filter_map(|b| match b {
            Beat::Guts { bench_outcome, .. } => bench_outcome,
            _ => None,
        })
        .collect();
    let mut want = vec!["green".to_owned(); 9];
    want.push("red".into());
    assert_eq!(outcomes, want);
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
    let ceiling_tokens = usize::try_from(mnemorph_races::Race::Rimuru.prefix_ceiling()).unwrap();
    let w = World::new("prefix-ceiling");
    // The prompt carries the world's law: the ceiling holds with this repository's own.
    let law = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.mnemorph/law.md");
    fs::create_dir_all(w.world().join(".mnemorph")).unwrap();
    fs::copy(law, w.world().join(".mnemorph/law.md")).unwrap();
    let (body, _) = mnemorph_loop::solo(&w.world(), "m");
    assert!(
        body.system.contains("## The crest"),
        "the law is in the prompt"
    );
    let tools: Vec<mnemorph_provider::Tool> = registry()
        .iter()
        .map(|t| mnemorph_provider::Tool {
            name: t.name().into(),
            description: t.description().into(),
            input_schema: t.schema(),
            cache: false,
            defer: false,
        })
        .collect();
    let bytes = body.system.len() + serde_json::to_string(&tools.iter().map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.input_schema})).collect::<Vec<_>>()).unwrap().len();
    let tokens = bytes / 4;
    eprintln!("solo prefix: {bytes} bytes ≈ {tokens} tokens (ceiling {ceiling_tokens})");
    assert!(
        tokens <= ceiling_tokens,
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
            author: None,
            ..Default::default()
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
        "[stress]\ncontext_tokens = 90000\n[steps]\nsolo = 150\n[recall]\nfloor = 0.9\nsaturation_long = 2.0\n",
    )
    .unwrap();
    // Letting a rule ask loosens the gate: the global file alone says it.
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("none.toml"),
        "[gate]\nask_rules = [\"in-place-edit\"]\n",
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
    // Recall's weights: the keys set, the defaults for the rest.
    let d = mnemorph_memory::Weights::default();
    assert_eq!(
        (
            limits.weights.floor,
            limits.weights.saturation[2],
            limits.weights.meaning
        ),
        (0.9, 2.0, d.meaning)
    );
    // The solo body's steps: the human's, else the shortest chain's — an Elf's, an Orc's, a
    // Slime's. The session sets them when it has no Elf to dispatch to.
    assert_eq!(config.solo_steps(), 150);
    let bare = mnemorph_config::Config::load(&mnemorph_config::Paths::with_global(
        root.join("none.toml"),
        &root.join("nowhere"),
    ))
    .unwrap();
    assert_eq!(bare.solo_steps(), mnemorph_config::SOLO_STEPS);
    assert_eq!(mnemorph_config::SOLO_STEPS, 30 + 30 + 40);
    // Its steps can be spent: the stall guard waits for them, never stopping it at 40.
    let mut stress = mnemorph_loop::Stress::for_steps(100);
    for turn in 1..=100 {
        assert_eq!(stress.turn(), None, "turn {turn}");
    }
    assert_eq!(
        stress.turn(),
        Some(mnemorph_loop::Signal::Stall("actBudget"))
    );
    // A body with fewer steps keeps the guard's own 40.
    let mut short = mnemorph_loop::Stress::for_steps(30);
    assert!((1..=40).all(|_| short.turn().is_none()));
    assert!(short.turn().is_some());
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
    // A torn last write: the line is counted high, as the largest call, and the run goes on.
    std::fs::remove_dir_all(&path).unwrap();
    std::fs::write(&path, "{\"torn\n").unwrap();
    let m = Arc::new(ScriptedModel::new([answer("still")]));
    let e = ask(
        &mut w.run(m.clone(), limits()).with_caps(mnemorph_ledger::Caps {
            day: Some(u64::MAX),
            ..caps
        }),
        "go",
    )
    .await;
    assert!(matches!(e, Ending::Answered(_)), "{e:?}");
    // Under a cap smaller than what the torn line is taken to have cost, it weighs only the
    // cap's hidden reserve, so by itself it stops nothing.
    std::fs::write(&path, "{\"torn\n").unwrap();
    let m = Arc::new(ScriptedModel::new([answer("small cap")]));
    let e = ask(
        &mut w.run(m.clone(), limits()).with_caps(mnemorph_ledger::Caps {
            day: Some(mnemorph_ledger::UNSEEN_FLOOR_WT),
            ..caps
        }),
        "go",
    )
    .await;
    assert!(matches!(e, Ending::Answered(_)), "{e:?}");
    assert_eq!(m.requests().len(), 1);
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

/// Every act is journaled as the guts read it, with its instruments: the record a learned value
/// model trains on. The readings are for the record only: a resumed conversation is the same.
#[tokio::test]
async fn every_act_journals_its_guts_reading_and_instruments() {
    let w = World::new("guts-beats");
    let model = Arc::new(ScriptedModel::new([
        tool(
            "t1",
            "write",
            json!({"path": "a.txt", "content": "one\ntwo\n"}),
        ),
        tool(
            "t2",
            "edit",
            json!({"path": "a.txt", "old_string": "three", "new_string": "3"}),
        ),
        tool(
            "t3",
            "bash",
            json!({"command": "python3 -c \"assert open('a.txt').read().startswith('one')\""}),
        ),
        answer("done"),
    ]));
    let mut run = w.run(model, limits());
    assert!(matches!(
        ask(&mut run, "write a").await,
        Ending::Answered(_)
    ));
    let rows = w.journal().read().unwrap().rows;
    let guts: Vec<&Beat> = rows
        .iter()
        .filter(|b| matches!(b, Beat::Guts { .. }))
        .collect();
    assert_eq!(guts.len(), 3, "one reading per act");
    let Beat::Guts {
        score,
        lines_added,
        lines_removed,
        wrote,
        edit_miss,
        failed,
        ..
    } = guts[0]
    else {
        unreachable!()
    };
    assert_eq!(
        (*score, *wrote, *lines_added, *lines_removed),
        (2, 1, 2, 0),
        "a landed write"
    );
    assert!(!edit_miss && !failed);
    let Beat::Guts {
        score,
        edit_miss,
        failed,
        window,
        ..
    } = guts[1]
    else {
        unreachable!()
    };
    assert!(*edit_miss && *failed, "the edit missed");
    assert_eq!((*score, *window), (-1, 1), "+2 then −1");
    let Beat::Guts {
        bench,
        bench_outcome,
        ..
    } = guts[2]
    else {
        unreachable!()
    };
    assert_eq!(bench.as_deref(), Some("python3 (an inline check)"));
    assert_eq!(
        bench_outcome.as_deref(),
        Some("green"),
        "the check counted as a test run"
    );
    // The readings never reach the model: the rebuilt conversation is the run's own.
    assert_eq!(conversation(&rows), run.messages());
}

/// Each ask recalls from the body's own notes and the shared ones: a matching grounded note is
/// shown as an `@M` line with its text, and the turn's anchors are journaled; an ask that matches
/// nothing adds nothing.
#[tokio::test]
async fn an_ask_recalls_what_memory_holds_and_journals_the_anchors() {
    let w = World::new("recall");
    let claim = mnemorph_memory::propose(
        &w.world(),
        "slime-other",
        "the tokenizer keeps interval fractions whole",
        vec![],
        None,
    )
    .unwrap();
    let grounded = mnemorph_memory::ground(
        &w.world(),
        claim,
        &mnemorph_memory::Proof::confirmed("the human"),
    )
    .unwrap();
    // A torn line in the shared notes: left out of recall, and told.
    let shared = w.world().join(".mnemorph/memory/shared/notes.jsonl");
    let mut torn = fs::read_to_string(&shared).unwrap();
    torn.push_str("{\"id\": \"torn\n");
    fs::write(&shared, torn).unwrap();
    let model = Arc::new(ScriptedModel::new([answer("noted"), answer("fine")]));
    let mut run = w.run(model.clone(), limits());
    ask(&mut run, "why does the tokenizer split interval fractions").await;
    ask(&mut run, "rename the button colour").await;
    let text_of = |i: usize| -> String {
        model.requests()[i]
            .messages
            .iter()
            .rev()
            .find(|m| m.role == mnemorph_provider::Role::User)
            .unwrap()
            .content
            .iter()
            .filter_map(|b| match b {
                Block::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    };
    let first = text_of(0);
    assert!(
        first.contains(&format!(
            "@M {} shared grounded support: the tokenizer keeps interval fractions whole",
            grounded.note
        )),
        "{first}"
    );
    assert!(first.contains("@M -0 dissent: none selected"), "{first}");
    assert!(
        !text_of(1).contains("@M"),
        "an ask that matches nothing adds nothing"
    );
    let recalls: Vec<Vec<String>> = w
        .journal()
        .read()
        .unwrap()
        .rows
        .into_iter()
        .filter_map(|b| match b {
            Beat::Recall { anchors, .. } => Some(anchors),
            _ => None,
        })
        .collect();
    assert_eq!(recalls, [vec![grounded.note.clone()]]);
    let rows = w.journal().read().unwrap().rows;
    let ask_at = rows
        .iter()
        .position(|b| matches!(b, Beat::Ask { .. }))
        .unwrap();
    let recall_at = rows
        .iter()
        .position(|b| matches!(b, Beat::Recall { .. }))
        .unwrap();
    assert_eq!(
        recall_at,
        ask_at + 1,
        "the turn's recall row follows its ask"
    );
    assert!(
        rows.iter()
            .any(|b| matches!(b, Beat::Warning { text } if text.contains("could not be read")))
    );
}

/// The same recall a third time in five with nothing landed, and no correction in memory: the body
/// is told to change its approach, and each ended ask leaves its trace.
#[tokio::test]
async fn a_replayed_recall_tells_the_body_to_change_its_approach() {
    let w = World::new("replay");
    let claim = mnemorph_memory::propose(
        &w.world(),
        "slime-other",
        "the tokenizer keeps interval fractions whole",
        vec![],
        None,
    )
    .unwrap();
    mnemorph_memory::ground(
        &w.world(),
        claim,
        &mnemorph_memory::Proof::confirmed("the human"),
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        answer("looking"),
        answer("still looking"),
        answer("looking again"),
    ]));
    let mut run = w.run(model.clone(), limits());
    for _ in 0..3 {
        ask(&mut run, "why does the tokenizer split interval fractions").await;
    }
    let last_user = |i: usize| -> String {
        model.requests()[i]
            .messages
            .iter()
            .rev()
            .find(|m| m.role == mnemorph_provider::Role::User)
            .unwrap()
            .content
            .iter()
            .filter_map(|b| match b {
                Block::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    };
    assert!(!last_user(1).contains("@M replay"), "twice is no loop");
    assert!(
        last_user(2).contains("@M replay: this recall came back three times"),
        "{}",
        last_user(2)
    );
    let name = "slime-test";
    let traces = mnemorph_memory::pathology::traces(&w.world(), name).unwrap();
    assert_eq!(traces.len(), 3);
    assert_eq!(traces[2].forced.as_deref(), Some("replay-escalated"));
    assert!(
        traces
            .iter()
            .all(|t| t.outcome.is_none() && t.fingerprint == traces[0].fingerprint)
    );
}

/// Lock-in: a revision newer than every note recalled, linked to one of them, while no correction
/// was selected — it is recalled too, though its words match nothing in the ask.
#[tokio::test]
async fn a_newer_revision_is_forced_into_a_locked_in_recall() {
    let w = World::new("lockin");
    let claim = mnemorph_memory::propose(
        &w.world(),
        "slime-other",
        "the tokenizer keeps interval fractions whole",
        vec![],
        None,
    )
    .unwrap();
    let old = mnemorph_memory::ground(
        &w.world(),
        claim,
        &mnemorph_memory::Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    std::thread::sleep(std::time::Duration::from_millis(5));
    let revision = mnemorph_memory::revise(
        &w.world(),
        "slime-other",
        &["traits hold: a rounding regression broke the money columns".into()],
        &[mnemorph_memory::Anchor {
            id: old.clone(),
            personal: false,
            dissent: false,
            cited: true,
        }],
        "log.md gate: FAIL",
    )
    .unwrap()
    .remove(0);
    let model = Arc::new(ScriptedModel::new([answer("noted")]));
    let mut run = w.run(model.clone(), limits());
    ask(&mut run, "why does the tokenizer split interval fractions").await;
    let first: String = model.requests()[0]
        .messages
        .iter()
        .rev()
        .find(|m| m.role == mnemorph_provider::Role::User)
        .unwrap()
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(first.contains(&format!("@M {old} ")), "{first}");
    assert!(
        first.contains(&format!("@M {revision} ")),
        "the revision is recalled: {first}"
    );
    // It reaches the recall on the words it borrows from the note it cites; lock-in forces in
    // only a revision that still misses (pathology.rs's own tests hold that case).
}

/// A full context drains instead of dying: one turn told to call no tool, its `@U` lines kept as the
/// body's proposed notes, then the checkpoint.
#[tokio::test]
async fn a_full_context_drains_its_unsaid_into_memory() {
    let w = World::new("drain");
    let model = Arc::new(ScriptedModel::new([
        tool_with("t1", "glob", json!({"pattern": "*"}), usage(181_000)),
        answer(
            "@U territory src/parser keeps interval fractions whole\n@U colony run uv before pytest\nLeft: the docs.",
        ),
    ]));
    let mut run = w.run(
        model.clone(),
        Limits {
            compactions: 0,
            ..limits()
        },
    );
    let e = ask(&mut run, "big").await;
    let Ending::Checkpoint { why, salvage } = &e else {
        panic!("{e:?}");
    };
    assert!(why.starts_with("context"), "{why}");
    let salvage = salvage.as_deref().unwrap();
    assert!(
        salvage.ends_with("[memory: 1 line(s) kept as proposed notes, 1 territory fact(s) filed]"),
        "{salvage}"
    );
    let drain = &model.requests()[1];
    assert_eq!(
        drain.tools,
        model.requests()[0].tools,
        "the drain turn sends the tools as any turn does: the cached prefix holds"
    );
    assert_eq!(drain.max_tokens, 4096);
    let notes = mnemorph_memory::fold(
        &mnemorph_memory::read(
            &w.world(),
            &mnemorph_memory::Tier::Personal("slime-test".into()),
        )
        .unwrap()
        .rows,
    );
    let mut kept: Vec<(String, Vec<String>)> = notes
        .values()
        .map(|n| {
            assert_eq!(n.status, mnemorph_memory::Status::Proposed);
            (n.text.clone(), n.tags.clone())
        })
        .collect();
    kept.sort();
    assert_eq!(
        kept,
        [(
            "run uv before pytest".to_owned(),
            vec!["colony".to_owned(), "drain".to_owned()]
        )],
        "a territory fact is no memory"
    );
    // It is filed to the ontology instead, by the body that learned it.
    let facts = fs::read_to_string(w.world().join(mnemorph_world::facts::FACTS)).unwrap();
    assert!(facts.contains("interval fractions whole"), "{facts}");
}

/// A jump to the window's edge leaves no room for a drain turn: the plain checkpoint.
#[tokio::test]
async fn a_context_at_the_windows_edge_checkpoints_without_a_drain() {
    let w = World::new("nodrain");
    let model = Arc::new(ScriptedModel::new([
        tool_with("t1", "glob", json!({"pattern": "*"}), usage(199_000)),
        // The compaction's summary turn fails: with no room left, no drain follows it.
        Err(mnemorph_provider::Error::Invalid("context too long".into())),
    ]));
    let mut run = w.run(model.clone(), limits());
    let e = ask(&mut run, "big").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, salvage: None } if why.starts_with("context")),
        "{e:?}"
    );
    assert_eq!(
        model.requests().len(),
        2,
        "the compaction tried, no drain turn was sent"
    );
}

/// A run drains once: the next ask on the same full context checkpoints plainly, with no second
/// drain turn and no second set of notes.
#[tokio::test]
async fn a_run_drains_once() {
    let w = World::new("drain-once");
    let model = Arc::new(ScriptedModel::new([
        tool_with("t1", "glob", json!({"pattern": "*"}), usage(181_000)),
        answer("@U colony run uv before pytest"),
        answer("never sent"),
    ]));
    let mut run = w.run(
        model.clone(),
        Limits {
            compactions: 0,
            ..limits()
        },
    );
    ask(&mut run, "big").await;
    let again = ask(&mut run, "and now?").await;
    assert!(
        matches!(&again, Ending::Checkpoint { why, salvage: None } if why.starts_with("context")),
        "{again:?}"
    );
    assert_eq!(model.requests().len(), 2, "no second drain turn");
    let notes = mnemorph_memory::fold(
        &mnemorph_memory::read(
            &w.world(),
            &mnemorph_memory::Tier::Personal("slime-test".into()),
        )
        .unwrap()
        .rows,
    );
    assert_eq!(notes.len(), 1, "no second set of notes");
}

/// What a compacting body writes: the fixed sections, a discovery on the wire.
const SUMMARY: &str = "## Goal\nmap the repo\n## Instructions\nread only\n## Discoveries\n@U colony src/main.rs run uv before pytest\n## Accomplished\nglobbed the tree\n## Files\n@F src/main.rs the entry point\n## Todo\nread the tests";

/// A full context with compactions left does not stop: the body summarises in fixed sections,
/// its discoveries are kept, and the run goes on over the ask, that summary and the newest
/// exchanges — and a resume rebuilds the very same conversation from the journal.
#[tokio::test]
async fn a_full_context_compacts_and_the_run_goes_on() {
    let w = World::new("compact");
    let model = Arc::new(ScriptedModel::new([
        tool_with("t1", "glob", json!({"pattern": "*"}), usage(181_000)),
        answer(SUMMARY),
        answer("done: the repo is mapped"),
    ]));
    let mut run = w.run(model.clone(), limits());
    let e = ask(&mut run, "map the repo").await;
    assert_eq!(e, Ending::Answered("done: the repo is mapped".into()));
    let requests = model.requests();
    assert_eq!(
        requests[1].tools, requests[0].tools,
        "the summary turn sends the tools as any turn does: the cached prefix holds"
    );
    let after = &requests[2].messages;
    let head = format!("{:?}", after[0]);
    assert!(
        head.contains("map the repo") && head.contains("[compacted"),
        "{head}"
    );
    assert!(
        head.contains("## Todo\\nread the tests"),
        "the summary leads: {head}"
    );
    assert!(
        after
            .iter()
            .any(|m| m.content.iter().any(|b| matches!(b, Block::ToolUse { .. }))),
        "the newest exchange is kept verbatim"
    );
    let rows = w.journal().read().unwrap().rows;
    assert!(
        rows.iter()
            .any(|b| matches!(b, Beat::Compacted { tail, .. } if *tail > 0))
    );
    assert_eq!(
        conversation(&rows),
        run.messages(),
        "a resume rebuilds what was sent"
    );
    let notes = mnemorph_memory::fold(
        &mnemorph_memory::read(
            &w.world(),
            &mnemorph_memory::Tier::Personal("slime-test".into()),
        )
        .unwrap()
        .rows,
    );
    assert!(
        notes
            .values()
            .any(|n| n.text == "src/main.rs run uv before pytest")
    );
}

/// An ask that has spent its compactions drains at the next full context, as before.
#[tokio::test]
async fn spent_compactions_drain() {
    let w = World::new("compact-spent");
    let model = Arc::new(ScriptedModel::new([
        tool_with("t1", "glob", json!({"pattern": "*"}), usage(181_000)),
        answer(SUMMARY),
        tool_with("t2", "glob", json!({"pattern": "*"}), usage(185_000)),
        answer("@U colony the second pass found nothing new"),
    ]));
    let mut run = w.run(
        model.clone(),
        Limits {
            compactions: 1,
            ..limits()
        },
    );
    let e = ask(&mut run, "map the repo").await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, salvage: Some(_) } if why.starts_with("context")),
        "{e:?}"
    );
    assert_eq!(model.requests().len(), 4, "one compaction, then the drain");
}

/// Two compactions in one ask: each leaves the head and its summary, the journal replays exactly
/// what was sent, and a resume reads the compaction's own estimate — it does not compact the
/// small conversation again.
#[tokio::test]
async fn two_compactions_replay_and_a_resume_reads_their_estimate() {
    let w = World::new("compact-twice");
    let model = Arc::new(ScriptedModel::new([
        tool_with("t1", "glob", json!({"pattern": "*"}), usage(181_000)),
        answer(SUMMARY),
        tool_with("t2", "glob", json!({"pattern": "*"}), usage(182_000)),
        answer(SUMMARY),
        answer("done"),
    ]));
    let mut run = w.run(model.clone(), limits());
    let e = ask(&mut run, "map the repo").await;
    assert_eq!(e, Ending::Answered("done".into()));
    let rows = w.journal().read().unwrap().rows;
    let tails: Vec<usize> = rows
        .iter()
        .filter_map(|b| match b {
            Beat::Compacted { tail, .. } => Some(*tail),
            _ => None,
        })
        .collect();
    assert_eq!(tails.len(), 2, "two compactions");
    assert_eq!(
        conversation(&rows),
        run.messages(),
        "the journal replays what was sent"
    );
    // A resume over this journal reads the last compaction's estimate, not a full reading.
    let after = Arc::new(ScriptedModel::new([answer("resumed")]));
    let mut resumed = Run::resume(body(), limits(), w.setup(after.clone())).unwrap();
    let e = ask(&mut resumed, "and now?").await;
    assert_eq!(
        e,
        Ending::Answered("resumed".into()),
        "no compaction of the compacted"
    );
    assert_eq!(after.requests().len(), 1);
}

/// An ask that names a path the ontology holds facts on carries them: the `@ONTO` line, contested
/// first, so the body sees what creatures learned there — and both sides when they disagree.
#[tokio::test]
async fn an_ask_naming_a_path_carries_its_facts() {
    use mnemorph_world::facts::{Event, from_unsaid};
    let w = World::new("onto-view");
    fs::create_dir_all(w.world().join("src")).unwrap();
    fs::write(w.world().join("src/build.rs"), "fn main() {}\n").unwrap();
    for (text, by) in [
        ("build=cargo test", "slime-a"),
        ("build=cargo nextest run", "slime-b"),
    ] {
        let e: Event = from_unsaid(&format!("src/ {text}"), by, None).unwrap();
        mnemorph_world::Facts::record(&w.world(), &e).unwrap();
    }
    let model = Arc::new(ScriptedModel::new([answer("noted")]));
    let mut run = w.run(model.clone(), limits());
    ask(
        &mut run,
        "fix the build script in src/build.rs, not /etc/passwd nor ../x",
    )
    .await;
    let first = format!("{:?}", model.requests()[0].messages.last());
    assert!(first.contains("@ONTO src/"), "{first}");
    assert!(
        first.contains("contested"),
        "both sides of a disagreement: {first}"
    );
    assert!(
        !first.contains("@ONTO /etc"),
        "a host path is never shown: {first}"
    );
    assert!(!first.contains("@ONTO ../"), "nor one outside the world");
}

/// Recall reads its weights from the run's limits, which the human's `recall.*` keys set: a floor
/// no match can clear recalls nothing, where the default recalls the note.
#[tokio::test]
async fn recall_reads_its_configured_weights() {
    let w = World::new("weights");
    let claim = mnemorph_memory::propose(
        &w.world(),
        "slime-other",
        "the tokenizer keeps interval fractions whole",
        vec![],
        None,
    )
    .unwrap();
    let id = mnemorph_memory::ground(
        &w.world(),
        claim,
        &mnemorph_memory::Proof::confirmed("the human"),
    )
    .unwrap()
    .note;
    let first = |floor: Option<f64>| {
        let w = &w;
        async move {
            let model = Arc::new(ScriptedModel::new([answer("noted")]));
            let mut l = limits();
            if let Some(f) = floor {
                l.weights.floor = f;
            }
            let mut run = w.run(model.clone(), l);
            ask(&mut run, "why does the tokenizer split interval fractions").await;
            format!("{:?}", model.requests()[0].messages.last())
        }
    };
    assert!(
        first(None).await.contains(&format!("@M {id} ")),
        "the default recalls it"
    );
    assert!(
        !first(Some(9.0)).await.contains("@M "),
        "an unreachable floor recalls nothing"
    );
}

/// A scripted model that sends the human's line down the run's send lane while its `at`-th call
/// is in flight, as a human typing during a turn would.
struct Steered {
    script: ScriptedModel,
    steer: std::sync::OnceLock<mnemorph_loop::Steer>,
    at: usize,
    line: &'static str,
}

impl mnemorph_loop::Model for Steered {
    fn provider(&self) -> &'static str {
        "scripted"
    }

    fn complete<'a>(
        &'a self,
        request: &'a mnemorph_provider::Request,
        on: &'a mut (dyn FnMut(mnemorph_provider::Event) + Send),
    ) -> mnemorph_loop::model::Completion<'a> {
        if self.script.requests().len() == self.at
            && let Some(steer) = self.steer.get()
        {
            steer.lock().unwrap().push_back(self.line.to_owned());
        }
        self.script.complete(request, on)
    }
}

/// Where each request rewrote the one before it — `(request, block)` — after checking every one
/// sends the system prompt and the tools byte-identical.
fn moves_between(requests: &[mnemorph_provider::Request]) -> Vec<(usize, usize)> {
    let fps = |r: &mnemorph_provider::Request| mnemorph_loop::compact::fingerprints(&r.messages);
    let mut moves = Vec::new();
    for (i, pair) in requests.windows(2).enumerate() {
        let (a, b) = (encode(&pair[0]).unwrap(), encode(&pair[1]).unwrap());
        assert_eq!(
            b["system"],
            a["system"],
            "request {} rewrote the system prompt",
            i + 1
        );
        assert_eq!(
            b["tools"],
            a["tools"],
            "request {} rewrote the tools",
            i + 1
        );
        if let Some(at) = mnemorph_loop::compact::moved(&fps(&pair[0]), &fps(&pair[1])) {
            moves.push((i + 1, at));
        }
    }
    moves
}

/// #38: over a session of many turns — a steering line, a Mind worn, a model fallback and a
/// reloaded configuration between asks, then a fold — every request sends the system prompt and
/// the tools byte-identical and only appends to the one before, except at the fold, which the
/// prefix beat explains. Each change rides as an appended message.
#[tokio::test]
async fn a_session_of_changes_only_appends_except_at_its_fold() {
    let w = World::new("append-many");
    let big = "a line of the file that the body reads, long enough to fold\n".repeat(40);
    for f in ["f1.txt", "f2.txt", "f3.txt", "f4.txt"] {
        fs::write(w.world().join(f), &big).unwrap();
    }
    adopted_mind(
        &w.world(),
        "light",
        "# Light\n\n- **Shared:** yes\n\nThe light Mind's know-how.\n",
    );
    let model = Arc::new(Steered {
        script: ScriptedModel::new([
            tool("t1", "read", json!({"path": "f1.txt"})),
            tool("t2", "read", json!({"path": "f2.txt"})),
            tool("t3", "wear", json!({"mind": "light"})),
            tool("t4", "read", json!({"path": "f3.txt"})),
            answer("first"),
            tool_with("t5", "read", json!({"path": "f4.txt"}), usage(2_000)),
            tool("t6", "glob", json!({"pattern": "*.txt"})),
            answer("second"),
        ]),
        steer: std::sync::OnceLock::new(),
        at: 1,
        line: "keep it short",
    });
    let mut setup = w.setup(Arc::new(ScriptedModel::default()));
    setup.model = model.clone();
    setup
        .tools
        .push(Box::new(mnemorph_loop::wear_tool::WearTool));
    let mut run = Run::new(
        body(),
        Limits {
            stress_tokens: 3_000,
            ..limits()
        },
        setup,
    );
    model.steer.set(run.steer()).unwrap();
    assert_eq!(
        ask(&mut run, "read the files").await,
        Ending::Answered("first".into())
    );
    run.switch_model("claude-haiku-5-5", "the provider is overloaded");
    run.update("the configuration reloaded: `limits.steps` is 50");
    assert_eq!(
        ask(&mut run, "go on").await,
        Ending::Answered("second".into())
    );

    let requests = model.script.requests();
    assert_eq!(requests.len(), 8, "eight turns");
    let moves = moves_between(&requests);
    let said =
        |r: &mnemorph_provider::Request, what: &str| format!("{:?}", r.messages).contains(what);
    assert!(
        said(&requests[2], "keep it short"),
        "the steering line rides"
    );
    assert!(
        said(&requests[3], "light Mind's know-how"),
        "the Mind rides"
    );
    assert!(said(&requests[5], "claude-haiku-5-5") && said(&requests[5], "50"));
    assert_eq!(requests[4].model, "claude-sonnet-5-5");
    assert_eq!(
        requests[5].model, "claude-haiku-5-5",
        "the fallback rides the model field"
    );
    assert_eq!(moves.len(), 1, "one move, the fold: {moves:?}");
    assert_eq!(moves[0].0, 6, "the fold is the only move: {moves:?}");
    assert!(
        said(&requests[6], "[folded:"),
        "request 6 is the folded one"
    );
    let beats = w.journal().read().unwrap().rows;
    let prefix: Vec<(usize, String, usize)> = beats
        .iter()
        .filter_map(|b| match b {
            Beat::Prefix { at, why, stripped } => Some((*at, why.clone(), *stripped)),
            _ => None,
        })
        .collect();
    assert_eq!(prefix.len(), 1, "{prefix:?}");
    assert_eq!(
        prefix[0].1,
        mnemorph_loop::FOLD,
        "the move says it was a fold"
    );
    assert!(
        !beats
            .iter()
            .any(|b| matches!(b, Beat::Warning { text } if text.contains("cached prefix moved"))),
        "nothing moved the prefix unexplained"
    );
    // The changes are journaled where they were said: a resume rebuilds the same conversation.
    let rebuilt = conversation(&beats);
    assert_eq!(
        mnemorph_loop::compact::fingerprints(&rebuilt),
        mnemorph_loop::compact::fingerprints(run.messages())
    );
}

/// The Mind `name` written into `world` with `text`, and adopted in the witness kept for it, as
/// the human's `mnemorphd mind adopt` would record it.
fn adopted_mind(world: &std::path::Path, name: &str, text: &str) {
    use mnemorph_world::minds::{propose_at, witness};
    fs::create_dir_all(world.join(".mnemorph/minds")).unwrap();
    fs::write(world.join(format!(".mnemorph/minds/{name}.md")), text).unwrap();
    let at = witness::path_of(world).expect("a home");
    let row = propose_at(world, &at, witness::Act::Adopt, name, None)
        .unwrap()
        .row;
    witness::record_at(&at, row).unwrap();
}

/// A fake MCP server `hub` for `w`: twelve tools, all `read`, offered to Slimes; and where its
/// state is kept.
fn fake_hub(w: &World) -> (Arc<mnemorph_tools::mcp::Mcp>, std::path::PathBuf) {
    use mnemorph_tools::mcp::{Mcp, Server, Settings};
    let tools: Vec<Value> = (0..10)
        .map(|i| json!({"name": format!("filler_{i}"), "description": format!("Chore {i}")}))
        .chain([
            json!({"name": "web_search", "description": "Search the web"}),
            json!({"name": "issue_find", "description": "Find an issue in a tracker"}),
        ])
        .collect();
    let list = w.world().join("../mcp-tools.json");
    fs::write(&list, Value::Array(tools).to_string()).unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mnemorph-tools/tests/fixtures/fake-mcp.sh");
    let state = w.world().join("../mcp-state");
    let mut hub = Server::new("hub", "bash", &[Race::Slime]);
    hub.args = vec![
        fixture.display().to_string(),
        list.display().to_string(),
        w.world().join("../mcp-log").display().to_string(),
    ];
    hub.class = Some(mnemorph_races::ToolClass::Read);
    hub.purpose = "Issues and the web".into();
    hub.timeout = Some(std::time::Duration::from_secs(10));
    let mcp = Mcp::new(
        Settings {
            top_k: 2,
            servers: vec![hub],
            ..Settings::default()
        },
        state.clone(),
    );
    (mcp, state)
}

/// #48 under #231: a run's MCP doors are fixed when it is made. The first ask's tail names the
/// servers at level 0; a server opened (level 1), a schema loaded (level 2) and a call ride the
/// conversation as tool results, so every request sends the same tools block and only appends —
/// the prefix guard records no move. A later ask's tail names the tools ranked for it, by BM25
/// over what is now listed. What each ask called is learned at its end.
#[tokio::test]
async fn mcp_levels_loaded_and_called_mid_run_never_move_the_prefix() {
    use mnemorph_tools::mcp::{CALL, Kind, OPEN, SCHEMA};
    let w = World::new("mcp-append");
    let (mcp, state) = fake_hub(&w);
    let model = Arc::new(ScriptedModel::new([
        tool("t1", OPEN, json!({"server": "hub"})),
        tool("t2", SCHEMA, json!({"name": "hub__issue_find"})),
        tool(
            "t3",
            CALL,
            json!({"name": "hub__issue_find", "arguments": {}}),
        ),
        answer("first"),
        tool("t4", CALL, json!({"name": "hub__filler_3"})),
        answer("second"),
    ]));
    let mut setup = w.setup(model.clone());
    let offer = mcp.offer(Race::Slime, &setup.cx);
    assert_eq!(offer.lines.len(), 1);
    setup.tools.extend(offer.tools);
    let mut run = Run::new(body(), limits(), setup).with_mcp(Some(Arc::clone(&mcp)));
    assert_eq!(
        ask(&mut run, "find the issue").await,
        Ending::Answered("first".into())
    );
    assert_eq!(
        ask(&mut run, "a chore 3").await,
        Ending::Answered("second".into())
    );

    let requests = model.requests();
    assert_eq!(requests.len(), 6);
    for (i, r) in requests.iter().enumerate() {
        assert_eq!(
            r.tools, requests[0].tools,
            "request {i} rewrote the tools block"
        );
        assert_eq!(r.system, requests[0].system);
    }
    let names: Vec<&str> = requests[0].tools.iter().map(|t| t.name.as_str()).collect();
    for door in [OPEN, SCHEMA, CALL] {
        assert!(names.contains(&door), "{names:?}");
    }
    assert!(
        !names.iter().any(|n| n.starts_with("hub__")),
        "no MCP tool joins the frame: {names:?}"
    );
    assert!(
        moves_between(&requests).is_empty(),
        "the requests only append"
    );
    let beats = w.journal().read().unwrap().rows;
    assert!(
        !beats.iter().any(|b| matches!(b, Beat::Prefix { .. })),
        "the guard recorded no prefix move"
    );
    let first = format!("{:?}", requests[0].messages);
    assert!(
        first.contains("- hub — Issues and the web · not listed yet"),
        "the first ask's tail names the server at level 0: {first}"
    );
    let said = format!("{:?}", requests[3].messages);
    assert!(
        said.contains("hub__issue_find — read · schema ~"),
        "level 1 was loaded"
    );
    assert!(said.contains("level 2"), "the schema was loaded");
    assert!(said.contains("called issue_find"), "the door called it");
    let second = format!("{:?}", requests[4].messages.last());
    assert!(
        second.contains("MCP tools that fit this ask") && second.contains("hub__filler_3 — read"),
        "the later ask's tail ranks what is listed: {second}"
    );
    assert!(
        !second.contains("level 0"),
        "level 0 is said once: {second}"
    );
    // Each load and call is journaled under the state directory, chained and witnessed.
    let rows = mnemorph_loop::mcp_journal::read(&state).unwrap();
    let did = |kind, tool: Option<&str>| {
        rows.iter()
            .find(|r| r.kind == kind && r.tool.as_deref() == tool)
            .unwrap_or_else(|| panic!("{kind:?} {tool:?} in {rows:?}"))
    };
    let open = did(Kind::Open, None);
    assert!(open.used && open.helped && open.race == "slime");
    assert!(did(Kind::Schema, Some("hub__issue_find")).used);
    assert_eq!(did(Kind::Call, Some("hub__issue_find")).outcome, "ok");
    assert!(
        did(Kind::Call, Some("hub__filler_3"))
            .terms
            .contains(&"chore".to_owned())
    );
    let offer = did(Kind::Offer, Some("hub__filler_3"));
    assert!(
        offer.used && offer.why.as_deref().unwrap().starts_with("bm25 "),
        "{offer:?}"
    );
}
