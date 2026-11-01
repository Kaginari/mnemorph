//! The do-nothing fixtures of an evolution: the same tasks run with and without the change, on a
//! scripted model, and the evolution judged against doing nothing over its post-swap window.

#![allow(
    clippy::unnecessary_wraps,
    reason = "scripted replies are Results: the script also holds errors"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_ledger::evolution::{
    self, Arm, EVOLUTION_WINDOW, EvolutionRow, FileChange, Gate, Judgement, Run as Ran, Swap,
};
use mnemorph_ledger::{Build, Journal};
use mnemorph_loop::{Beat, EndGate, Limits, Run, ScriptedModel, Setup, solo};
use mnemorph_provider::{Caller, Error, Response, anthropic_prices};
use mnemorph_races::Race;
use mnemorph_tools::{Sandbox, registry};
use serde_json::json;

const MIND: &str = ".mnemorph/minds/careful/README.md";
const OLD: &str = "# Careful\n\nRead before writing.\n";
const NEW: &str = "# Careful\n\nRead before writing. Write once.\n";

/// One task: its fingerprint, the script the model answers with, and the end gate's verify lines.
struct Task {
    id: String,
    script: Vec<Result<Response, Error>>,
    verify: Vec<String>,
}

fn tasks() -> Vec<Task> {
    (0..EVOLUTION_WINDOW)
        .map(|i| match i % 3 {
            // A fix the gate checks.
            0 => Task {
                id: format!("fix-{i}"),
                script: vec![
                    ScriptedModel::tool(
                        "t1",
                        "write",
                        json!({"path": "out.txt", "content": "fixed\n"}),
                    ),
                    ScriptedModel::text("wrote out.txt"),
                ],
                verify: vec!["grep -q fixed out.txt".into()],
            },
            // Doing nothing is the right output: nothing to change, nothing changed.
            1 => Task {
                id: format!("nothing-{i}"),
                script: vec![ScriptedModel::text("nothing to do: it already holds")],
                verify: vec!["test ! -e out.txt".into()],
            },
            // A change the gate refuses twice.
            _ => Task {
                id: format!("broken-{i}"),
                script: vec![
                    ScriptedModel::tool(
                        "t1",
                        "write",
                        json!({"path": "out.txt", "content": "broken\n"}),
                    ),
                    ScriptedModel::text("done"),
                    ScriptedModel::text("still done"),
                ],
                verify: vec!["grep -q fixed out.txt".into()],
            },
        })
        .collect()
}

/// A fresh world holding the Mind as `doc`; dropped with its directory.
struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str, doc: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-evolution-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mind = root.join("world").join(MIND);
        fs::create_dir_all(mind.parent().unwrap()).unwrap();
        fs::write(mind, doc).unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    /// Runs `task` once and reads the gate's word from the run's beats.
    async fn run(&self, task: Task) -> (Option<Gate>, bool) {
        let journal: Journal<Beat> = Journal::new(self.root.join("run.jsonl"));
        let (body, cx) = solo(&self.world(), "claude-sonnet-5-5");
        let cx = cx
            .with_sandbox(Sandbox::None("fixtures run unguarded".into()))
            .with_unguarded(true)
            .with_attic(
                mnemorph_tools::Attic::at(&self.root.join("attic"), &self.world()).expect("git"),
            );
        let setup = Setup {
            model: Arc::new(ScriptedModel::new(task.script)),
            tools: registry(),
            cx,
            journal: journal.clone(),
            ledger: None,
            prices: anthropic_prices().unwrap(),
            caller: Caller {
                session: "s".into(),
                run: task.id.clone(),
                body: "rimuru".into(),
                race: "rimuru".into(),
                commission: None,
                prefix: 0,
            },
        };
        let mut run = Run::new(body, Limits::for_race(Race::Rimuru), setup).with_gate(EndGate {
            verify: task.verify,
            held: Vec::new(),
            by: Vec::new(),
            author: None,
            ..Default::default()
        });
        let mut quiet = |_| {};
        run.ask("the task", &mut quiet).await;
        let gates: Vec<bool> = journal
            .read()
            .unwrap()
            .rows
            .into_iter()
            .filter_map(|b| match b {
                Beat::Gate { failures, .. } => Some(failures.is_empty()),
                _ => None,
            })
            .collect();
        match gates.last() {
            None => (None, false),
            Some(true) => (Some(Gate::Pass), gates.len() == 1),
            Some(false) => (Some(Gate::Fail), false),
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn swap(world: &Path) -> Swap {
    let digest = |s: &str| Some(evolution::digest(s.as_bytes()));
    let files = vec![FileChange {
        path: MIND.into(),
        before: digest(OLD),
        after: digest(NEW),
    }];
    let before = evolution::snapshot(world).unwrap();
    let mut after = before.clone();
    after.insert(MIND.into(), evolution::digest(NEW.as_bytes()));
    Swap {
        id: "careful-1".into(),
        creature: "rimuru".into(),
        bodies: vec!["rimuru".into()],
        arm: Arm::Mutate,
        build: Build::this(),
        diff: evolution::diff(&before, &after),
        files,
        proven_on: Vec::new(),
    }
}

/// A Mind change a scripted model never reads cannot beat doing nothing: the same tasks give the
/// same gate words with and without it, so the judgement proposes a rollback.
#[tokio::test]
async fn a_change_that_changes_nothing_rolls_back() {
    let probe = World::new("probe", OLD);
    let ledger: Journal<EvolutionRow> = Journal::chained(probe.root.join("evolution.jsonl"));
    let s = swap(&probe.world());
    ledger.append(&EvolutionRow::Swap(s.clone())).unwrap();
    for (with, without) in tasks().into_iter().zip(tasks()) {
        let id = with.id.clone();
        for (control, doc, task) in [(false, NEW, with), (true, OLD, without)] {
            let w = World::new(&format!("{id}-{control}"), doc);
            let state = evolution::state_now(&w.world(), &s).unwrap();
            let (gate, first_try) = w.run(task).await;
            // Stamped after the run too: a Mind edited mid-run shows.
            let state_after = evolution::state_now(&w.world(), &s).unwrap();
            ledger
                .append(&EvolutionRow::Run(Ran {
                    swap: Some(s.id.clone()),
                    build: s.build.clone(),
                    body: "rimuru".into(),
                    task: id.clone(),
                    control,
                    state,
                    state_after,
                    gate,
                    first_try,
                    holes_found: 0,
                    ask: None,
                }))
                .unwrap();
        }
    }
    let rows = ledger.read().unwrap().rows;
    let gate_of = |task: &str| {
        rows.iter()
            .filter_map(|r| match r {
                EvolutionRow::Run(r) if r.task == task => Some(r.gate),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(gate_of("fix-0"), [Some(Gate::Pass); 2]);
    assert_eq!(gate_of("broken-2"), [Some(Gate::Fail); 2]);
    let judged = evolution::judge(
        &evolution::read(ledger.path(), ledger.head().unwrap().as_ref()).unwrap(),
        "careful-1",
        mnemorph_config::MIN_NET_WINS,
    );
    let Judgement::Rollback(lines) = &judged else {
        panic!("{judged}");
    };
    assert_eq!(lines[0].meter, "netWins");
    assert!(lines[0].reading.abs() < 1e-9, "{judged}");
}
