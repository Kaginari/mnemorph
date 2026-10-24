//! The run's decision points against a scripted model and scripted deciders: a piped test run
//! read from its output when the decider is sure, left muted when it is not.

#![allow(
    clippy::unnecessary_wraps,
    reason = "scripted replies are Results: the script also holds errors"
)]

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mnemorph_ledger::{DecisionRow, Journal, Usage, UsageRow};
use mnemorph_loop::{
    Beat, Body, Decisions, Ending, Limits, Run, ScriptedModel, Setup, bench_ask, decisions,
};
use mnemorph_provider::{
    ANSWER_TOOL, Block, Caller, Complete, Completion, Cutoffs, Decider, Error, Fallback, Primary,
    Reply, Request, Response, Scripted, StopReason, SystemOne, anthropic_prices,
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
            std::env::temp_dir().join(format!("mnemorph-decide-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world")).unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn usage(&self) -> Journal<UsageRow> {
        Journal::new(self.root.join("usage.jsonl"))
    }

    fn decisions_journal(&self) -> Journal<DecisionRow> {
        Journal::new(self.root.join("decisions.jsonl"))
    }

    fn run(&self, model: Arc<ScriptedModel>) -> Run {
        let cx = Context::new(Race::Slime, self.world(), &["."])
            .with_sandbox(Sandbox::None("decision tests run unguarded".into()))
            .with_unguarded(true);
        let setup = Setup {
            model,
            tools: registry(),
            cx,
            journal: Journal::new(self.root.join("run.jsonl")),
            ledger: Some(self.usage()),
            prices: anthropic_prices().unwrap(),
            caller: Caller {
                session: "s".into(),
                run: "r".into(),
                body: "slime-test".into(),
                race: "slime".into(),
                commission: None,
                prefix: 0,
            },
        };
        let body = Body {
            name: "slime-test".into(),
            race: Race::Slime,
            model: "claude-sonnet-5-5".into(),
            system: "You are a test body.".into(),
        };
        Run::new(
            body,
            Limits {
                steps: 60,
                ..Limits::for_race(Race::Slime)
            },
            setup,
        )
    }

    fn warnings(&self) -> Vec<String> {
        Journal::<Beat>::new(self.root.join("run.jsonl"))
            .read()
            .unwrap()
            .rows
            .into_iter()
            .filter_map(|b| match b {
                Beat::Warning { text } => Some(text),
                _ => None,
            })
            .collect()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn usage() -> Usage {
    Usage {
        input: 100,
        output: 20,
        cache_read: 0,
        cache_write: 0,
    }
}

fn tool(id: &str, name: &str, input: Value) -> Result<Response, Error> {
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
        usage: usage(),
        warnings: vec![],
    })
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
        usage: usage(),
        warnings: vec![],
    })
}

/// Nine piped test runs with a fix landing between each: without a read, a muted bench stalls.
fn piped_runs(then_answer: bool) -> Arc<ScriptedModel> {
    runs_of("pytest -q | tail -1", then_answer)
}

fn runs_of(command: &str, then_answer: bool) -> Arc<ScriptedModel> {
    let mut script = Vec::new();
    for i in 0..9 {
        script.push(tool(&format!("b{i}"), "bash", json!({"command": command})));
        script.push(tool(
            &format!("w{i}"),
            "write",
            json!({"path": format!("fix{i}.py"), "content": "x = 1\n"}),
        ));
    }
    if then_answer {
        script.push(answer("the tests pass"));
    }
    Arc::new(ScriptedModel::new(script))
}

/// An LLM fallback that answers from a script and counts what it was asked.
#[derive(Default)]
struct Llm {
    replies: Mutex<Vec<Result<Response, Error>>>,
    asked: Mutex<u32>,
}

impl Complete for Llm {
    fn complete<'a>(&'a self, _: &'a Request) -> Completion<'a> {
        *self.asked.lock().unwrap() += 1;
        let reply = {
            let mut r = self.replies.lock().unwrap();
            if r.is_empty() {
                Err(Error::Invalid("the fallback was not expected".into()))
            } else {
                r.remove(0)
            }
        };
        Box::pin(async move { reply })
    }
}

fn green_from_system_one() -> String {
    r#"{"model":"jeff","answers":{"outcome":{"type":"choice","probabilities":{"green":0.97,"red":0.02,"unknown":0.01}}}}"#.into()
}

fn stated_green(p: f64) -> Result<Response, Error> {
    Ok(Response {
        id: "d".into(),
        model: "claude-haiku-5-5".into(),
        content: vec![Block::ToolUse {
            id: "a".into(),
            name: ANSWER_TOOL.into(),
            input: json!({"answers": {"outcome": {"green": p, "red": 1.0 - p, "unknown": 0}}}),
        }],
        stop: StopReason::ToolUse,
        stop_details: None,
        usage: usage(),
        warnings: vec![],
    })
}

fn with(w: &World, primary: Option<SystemOne>, llm: &Arc<Llm>) -> Decisions {
    Decisions {
        decider: Arc::new(Decider {
            primary: primary.map(|client| Primary {
                client,
                provider: "jeff".into(),
                model: "jeff-latest".into(),
            }),
            fallback: Fallback {
                client: llm.clone(),
                provider: "anthropic".into(),
                model: "claude-haiku-5-5".into(),
            },
        }),
        cutoffs: Cutoffs::default(),
        journal: w.decisions_journal(),
        bench: true,
        route: false,
    }
}

async fn ask(run: &mut Run) -> Ending {
    let mut quiet = |_| {};
    run.ask("make the tests pass", &mut quiet).await
}

/// Lesson: a piped test run hides its exit status, so a run that pipes every check stalled even
/// while its fixes worked. A System One model reads the output; a sure green counts as green.
#[tokio::test]
async fn a_sure_read_turns_a_muted_bench_green_and_the_run_finishes() {
    let w = World::new("green");
    let jeff = Arc::new(Scripted::new(
        (0..9).map(|_| Reply::ok(&green_from_system_one())),
    ));
    let client = SystemOne::new(
        jeff.clone(),
        "http://127.0.0.1:8765",
        None,
        SystemOne::policy(),
    );
    let llm = Arc::new(Llm::default());
    let mut run = w
        .run(piped_runs(true))
        .with_decisions(with(&w, Some(client), &llm));
    let e = ask(&mut run).await;
    assert!(matches!(&e, Ending::Answered(_)), "{e:?}");
    assert_eq!(jeff.sent().len(), 9, "one decision per piped run");
    assert_eq!(*llm.asked.lock().unwrap(), 0);
    let body: Value = serde_json::from_slice(&jeff.sent()[0].body).unwrap();
    assert!(
        body["state"]
            .as_str()
            .unwrap()
            .starts_with("$ pytest -q | tail -1\n")
    );

    let rows = w.decisions_journal().read().unwrap().rows;
    assert_eq!(rows.len(), 9);
    assert!(
        rows.iter()
            .all(|r| r.basis == "measured" && r.point == "bench.outcome")
    );
    assert!(
        w.warnings()
            .iter()
            .any(|t| t.contains("read green by jeff") && t.contains("measured")),
        "{:?}",
        w.warnings()
    );
    let spent = w.usage().read().unwrap().rows;
    assert_eq!(
        spent.iter().filter(|r| r.provider == "jeff").count(),
        9,
        "each decision reaches the usage ledger"
    );
}

fn system_one(replies: impl IntoIterator<Item = Reply>) -> (Arc<Scripted>, SystemOne) {
    let t = Arc::new(Scripted::new(replies));
    let c = SystemOne::new(
        t.clone(),
        "http://127.0.0.1:8765",
        None,
        SystemOne::policy(),
    );
    (t, c)
}

/// A sure red is a red seen: with fixes landing between, it is test-driven work, not a muted
/// streak, so the run that would have stalled goes on.
#[tokio::test]
async fn a_sure_red_read_is_test_driven_work_not_a_muted_streak() {
    let w = World::new("red");
    let red = r#"{"model":"jeff","answers":{"outcome":{"type":"choice","probabilities":{"green":0.01,"red":0.98,"unknown":0.01}}}}"#;
    let (_, client) = system_one((0..9).map(|_| Reply::ok(red)));
    let llm = Arc::new(Llm::default());
    let mut run = w
        .run(piped_runs(true))
        .with_decisions(with(&w, Some(client), &llm));
    let e = ask(&mut run).await;
    assert!(matches!(&e, Ending::Answered(_)), "{e:?}");
    assert!(w.warnings().iter().any(|t| t.contains("read red by jeff")));
}

/// A decision that fails changes nothing: the run stays muted and stalls where it always did.
#[tokio::test]
async fn a_failed_decision_leaves_the_run_as_it_was() {
    let w = World::new("failed");
    let (_, client) =
        system_one((0..9).map(|_| Reply::status(401, r#"{"error":{"message":"bad key"}}"#, &[])));
    let llm = Arc::new(Llm::default());
    let mut run = w
        .run(piped_runs(false))
        .with_decisions(with(&w, Some(client), &llm));
    let e = ask(&mut run).await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why == "stall: bench"),
        "{e:?}"
    );
    assert!(
        w.warnings()
            .iter()
            .any(|t| t.contains("the decision failed"))
    );
}

/// The shell keeps the first 30 000 bytes, so a long log's tail is not its summary: it is never
/// read, and nothing is sent.
#[tokio::test]
async fn output_the_shell_cut_short_is_never_read() {
    let w = World::new("cut");
    let (jeff, client) = system_one([]);
    let model = Arc::new(ScriptedModel::new([
        tool(
            "b",
            "bash",
            json!({"command": "pytest -q; head -c 40000 /dev/zero | tr '\\0' a"}),
        ),
        answer("done"),
    ]));
    let llm = Arc::new(Llm::default());
    let mut run = w.run(model).with_decisions(with(&w, Some(client), &llm));
    let _ = ask(&mut run).await;
    assert_eq!(jeff.sent(), Vec::new(), "nothing is sent");
    assert!(
        w.warnings().iter().any(|t| t.contains("cut short")),
        "{:?}",
        w.warnings()
    );
}

/// An Esc that stops the test command is not followed by a decision about its output.
#[tokio::test]
async fn an_esc_during_the_command_sends_nothing() {
    let w = World::new("esc");
    let (jeff, client) = system_one([]);
    let model = Arc::new(ScriptedModel::new([
        tool(
            "b",
            "bash",
            json!({"command": "pytest -q; touch running; sleep 30"}),
        ),
        answer("never"),
    ]));
    let llm = Arc::new(Llm::default());
    let mut run = w.run(model).with_decisions(with(&w, Some(client), &llm));
    let marker = w.world().join("running");
    let interrupt = run.interrupt();
    std::thread::spawn(move || {
        while !marker.exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        interrupt.fire();
    });
    assert_eq!(ask(&mut run).await, Ending::Interrupted);
    assert_eq!(jeff.sent(), Vec::new(), "nothing is sent");
    assert_eq!(*llm.asked.lock().unwrap(), 0);
}

/// An LLM's 80% is a guess about itself: under the stated cut-off, the run stays muted and stalls
/// exactly as it would with no decider.
#[tokio::test]
async fn an_unsure_stated_read_leaves_the_bench_muted() {
    let w = World::new("unsure");
    let llm = Arc::new(Llm {
        replies: Mutex::new((0..9).map(|_| stated_green(0.8)).collect()),
        asked: Mutex::new(0),
    });
    let mut run = w
        .run(piped_runs(false))
        .with_decisions(with(&w, None, &llm));
    let e = ask(&mut run).await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why == "stall: bench"),
        "{e:?}"
    );
    let rows = w.decisions_journal().read().unwrap().rows;
    assert!(!rows.is_empty() && rows.iter().all(|r| r.basis == "stated"));
    assert!(
        w.warnings()
            .iter()
            .any(|t| t.contains("read muted by anthropic"))
    );
}

#[tokio::test]
async fn a_run_whose_status_is_seen_asks_nothing() {
    let w = World::new("seen");
    let jeff = Arc::new(Scripted::new([]));
    let client = SystemOne::new(jeff.clone(), "http://x", None, SystemOne::policy());
    let model = Arc::new(ScriptedModel::new([
        tool("b", "bash", json!({"command": "pytest -q"})),
        answer("done"),
    ]));
    let llm = Arc::new(Llm::default());
    let mut run = w.run(model).with_decisions(with(&w, Some(client), &llm));
    let _ = ask(&mut run).await;
    assert_eq!(jeff.sent(), Vec::new(), "nothing is sent");
    assert_eq!(*llm.asked.lock().unwrap(), 0);
}

#[test]
fn the_bench_question_reads_the_tail_where_runners_print_their_summary() {
    let output = format!("{}\n=== 3 failed, 41 passed ===", "é".repeat(20_000));
    let ask = bench_ask("pytest | tail", &output);
    assert!(ask.state.ends_with("3 failed, 41 passed ==="));
    assert!(ask.state.len() < 13_000, "{}", ask.state.len());
}

/// A config builds the run's decisions: off with `decide.bench = false`; the LLM through the run's
/// own provider when no System One is named; the named one, with its key from the environment.
#[test]
fn decisions_are_built_from_the_humans_configuration() {
    let w = World::new("config");
    let global = w.root.join("global.toml");
    let world = w.world();
    let load = |text: &str| {
        fs::write(&global, text).unwrap();
        mnemorph_config::Config::load(&mnemorph_config::Paths::with_global(global.clone(), &world))
            .unwrap()
    };
    let model: Arc<ScriptedModel> = Arc::new(ScriptedModel::new([]));
    let none = |_: &str| None;

    let off = load("[decide]\nbench = false\n");
    assert!(
        decisions(&off, &world, model.clone(), none)
            .unwrap()
            .is_none()
    );

    // Routing alone is a reason to build the decider; the bench stays off.
    let route_only = decisions(
        &load("[decide]\nroute = true\n"),
        &world,
        model.clone(),
        none,
    )
    .unwrap()
    .unwrap();
    assert!(route_only.route && !route_only.bench);

    // No System One named: nothing is asked unless the human turns the point on.
    assert!(
        decisions(&load(""), &world, model.clone(), none)
            .unwrap()
            .is_none()
    );
    let llm_only = load("[decide]\nbench = true\n");
    let d = decisions(&llm_only, &world, model.clone(), none)
        .unwrap()
        .unwrap();
    assert!(d.decider.primary.is_none());
    assert_eq!(d.decider.fallback.provider, "scripted");
    assert_eq!(d.decider.fallback.model, "claude-haiku-5-5");
    assert!(
        d.journal
            .path()
            .ends_with(".mnemorph/instruments/decisions.jsonl")
    );

    let jeff = load(
        "[providers.jeff]\nkind = \"systemone\"\nbase_url = \"http://127.0.0.1:8765\"\n\
         [decide]\nvia = \"jeff\"\nmodel = \"jeff-latest\"\n",
    );
    let d = decisions(&jeff, &world, model.clone(), none)
        .unwrap()
        .unwrap();
    let p = d.decider.primary.as_ref().unwrap();
    assert_eq!(
        (p.provider.as_str(), p.model.as_str()),
        ("jeff", "jeff-latest")
    );

    let jev = load(
        "[providers.jev]\nkind = \"systemone\"\nbase_url = \"https://api.typesafe.ai\"\n\
         key_env = \"TYPESAFE_API_KEY\"\n[decide]\nvia = \"jev\"\nmodel = \"jev-latest\"\n",
    );
    let err = decisions(&jev, &world, model.clone(), none).err().unwrap();
    assert!(err.contains("TYPESAFE_API_KEY is not set"), "{err}");
    let key = |v: &str| (v == "TYPESAFE_API_KEY").then(|| "k".to_owned());
    assert!(decisions(&jev, &world, model, key).unwrap().is_some());
}

/// A run the shell saw fail is never read: a decider that would say green is not asked, and the
/// muted runs still stall.
#[tokio::test]
async fn a_failed_run_is_never_read_green() {
    let w = World::new("seen-red");
    let jeff = Arc::new(Scripted::new(
        (0..9).map(|_| Reply::ok(&green_from_system_one())),
    ));
    let client = SystemOne::new(
        jeff.clone(),
        "http://127.0.0.1:8765",
        None,
        SystemOne::policy(),
    );
    let llm = Arc::new(Llm::default());
    let mut run = w
        .run(runs_of("pytest -q; echo '42 passed'; false", false))
        .with_decisions(with(&w, Some(client), &llm));
    let e = ask(&mut run).await;
    assert!(
        matches!(&e, Ending::Checkpoint { why, .. } if why == "stall: bench"),
        "{e:?}"
    );
    assert!(
        jeff.sent().is_empty(),
        "a seen failure is not sent to be read"
    );
    assert_eq!(*llm.asked.lock().unwrap(), 0);
}
