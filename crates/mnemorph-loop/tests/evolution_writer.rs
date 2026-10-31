//! An evolution end to end: a Mind change swapped in writes its swap row, every ask of its body
//! after it writes a run row, the do-nothing controls run on demand on the before state, and the
//! judgement — keep or rollback — replays from the chained ledger. The window's tasks carry the
//! issue's do-nothing fixtures: a review where no finding is right, and an ask where no recall hit
//! is right.

#![allow(
    clippy::unnecessary_wraps,
    reason = "scripted replies are Results: the script also holds errors"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_config::{Config, Paths};
use mnemorph_ledger::Journal;
use mnemorph_ledger::evolution::{self, EVOLUTION_WINDOW, EvolutionRow, Gate, Judgement};
use mnemorph_loop::evolution::{Controller, run_controls};
use mnemorph_loop::{Beat, EndGate, Limits, Run, ScriptedModel, Setup, solo};
use mnemorph_provider::{Caller, Error, Response, anthropic_prices};
use mnemorph_races::Race;
use mnemorph_tools::{Context, Sandbox, registry};
use mnemorph_world::evolution::{Arm, Plan, Staged, swap_at};
use serde_json::json;

const MIND: &str = ".mnemorph/minds/careful/README.md";
const OLD: &str = "# Careful\n\nRead before writing.\n";
const NEW: &str = "# Careful\n\nRead before writing. File a finding only with a witness.\n";
const DOC: &str = ".mnemorph/rimuru/rimuru/README.md";
const NOTE: &str = "The deploy pipeline needs DOCKER_HOST exported before compose runs";

/// The kinds of task in a window, each with the right output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// A fix the gate checks: `out-<i>.txt` must read `fixed`.
    Fix,
    /// Nothing to change: no change is the right output.
    Nothing,
    /// A review of a sound file: no finding is the right output.
    NoFinding,
    /// An ask whose words no note in memory holds: no recall hit is the right output.
    NoRecall,
}

fn kind(i: usize) -> Kind {
    [Kind::Fix, Kind::Nothing, Kind::NoFinding, Kind::NoRecall][i % 4]
}

fn ask(i: usize) -> String {
    match kind(i) {
        Kind::Fix => format!("Make out-{i}.txt say fixed."),
        Kind::Nothing => format!("Check that task {i} needs no change."),
        Kind::NoFinding => {
            format!("Review sound-{i}.rs. Write any finding, with its witness, to findings-{i}.md.")
        }
        Kind::NoRecall => format!("Tell me whether sound-{i}.rs compiles as it stands."),
    }
}

/// The right output for task `i`, or — `wrong` — what a body that always does something does.
fn script(i: usize, wrong: bool) -> Vec<Result<Response, Error>> {
    let write = |path: String, content: &str| {
        ScriptedModel::tool("t1", "write", json!({"path": path, "content": content}))
    };
    match (kind(i), wrong) {
        (Kind::Fix, false) => vec![
            write(format!("out-{i}.txt"), "fixed\n"),
            ScriptedModel::text("wrote it"),
        ],
        (Kind::Fix, true) => vec![
            write(format!("out-{i}.txt"), "broken\n"),
            ScriptedModel::text("done"),
            ScriptedModel::text("still done"),
        ],
        (Kind::NoFinding, true) => vec![
            write(format!("findings-{i}.md"), "maybe an overflow\n"),
            ScriptedModel::text("filed one"),
            ScriptedModel::text("still filed"),
        ],
        (Kind::Nothing | Kind::NoFinding | Kind::NoRecall, _) => {
            vec![ScriptedModel::text("nothing to do: it already holds")]
        }
    }
}

/// A world holding the Mind as it was, the throne's doc with its verify lines, sound files to
/// review, and one note memory holds about something else.
struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "mnemorph-evolution-writer-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let w = root.join("world");
        for (path, text) in [
            (MIND, OLD),
            (
                DOC,
                "# rimuru\n\n- **Race:** rimuru\n- **Minds:** careful\n\
                 - **Verify:** `! grep -qs broken out-*.txt`\n\
                 - **Verify:** `! ls findings-*.md`\n",
            ),
        ] {
            fs::create_dir_all(w.join(path).parent().unwrap()).unwrap();
            fs::write(w.join(path), text).unwrap();
        }
        for i in 0..EVOLUTION_WINDOW {
            fs::write(
                w.join(format!("sound-{i}.rs")),
                "pub fn add(a: u8, b: u8) -> Option<u8> { a.checked_add(b) }\n",
            )
            .unwrap();
        }
        mnemorph_memory::propose(&w, "rimuru", NOTE, Vec::new(), None).unwrap();
        fs::write(root.join("global.toml"), "").unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn ledger(&self) -> Journal<EvolutionRow> {
        Journal::chained(self.root.join("state/evolution.jsonl"))
    }

    fn usage(&self) -> Journal<mnemorph_ledger::UsageRow> {
        Journal::chained(self.world().join(".mnemorph/instruments/usage.jsonl"))
    }

    fn config(&self) -> Config {
        Config::load(&Paths::with_global(
            self.root.join("global.toml"),
            &self.world(),
        ))
        .unwrap()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Fixtures run unguarded, with the attic beside the world.
fn ready(cx: Context, root: &Path) -> Context {
    let world = cx.world().to_path_buf();
    let cx = cx
        .with_sandbox(Sandbox::None("fixtures run unguarded".into()))
        .with_unguarded(true);
    match mnemorph_tools::Attic::at(&root.join("attic"), &world) {
        Some(a) => cx.with_attic(a),
        None => cx,
    }
}

/// One ask of the throne in `w`, as a session runs it: its gate the doc's verify lines, its asks
/// stamped on the world's ledger. Returns its journal.
async fn ask_once(w: &World, i: usize, replies: Vec<Result<Response, Error>>) -> Journal<Beat> {
    let world = w.world();
    let journal: Journal<Beat> = Journal::new(w.root.join(format!("run-{i}.jsonl")));
    let (body, cx) = solo(&world, "claude-sonnet-5-5");
    let roster = mnemorph_world::Roster::load(&world);
    let setup = Setup {
        model: Arc::new(ScriptedModel::new(replies)),
        tools: registry(),
        cx: ready(cx, &w.root),
        journal: journal.clone(),
        ledger: None,
        prices: anthropic_prices().unwrap(),
        caller: Caller {
            session: "s".into(),
            run: format!("r{i}"),
            body: "rimuru".into(),
            race: "rimuru".into(),
            commission: None,
            prefix: 0,
        },
    };
    let mut run = Run::new(body, Limits::for_race(Race::Rimuru), setup)
        .with_gate(EndGate {
            verify: roster.creatures["rimuru"].verify.clone(),
            author: None,
            ..Default::default()
        })
        .with_evolution_ledger(w.ledger().path().to_path_buf());
    let mut quiet = |_| {};
    run.ask(&ask(i), &mut quiet).await;
    journal
}

fn swap(w: &World) {
    swap_at(
        &w.world(),
        &w.ledger(),
        &Plan {
            id: "careful-1".into(),
            creature: "rimuru".into(),
            bodies: vec!["rimuru".into()],
            arm: Arm::Mutate,
            files: vec![Staged {
                path: MIND.into(),
                after: Some(NEW.as_bytes().to_vec()),
                creates: false,
            }],
            proven_on: Vec::new(),
        },
    )
    .unwrap();
}

fn runs(w: &World) -> Vec<evolution::Run> {
    w.ledger()
        .read()
        .unwrap()
        .rows
        .into_iter()
        .filter_map(|r| match r {
            EvolutionRow::Run(r) => Some(r),
            EvolutionRow::Swap(_) => None,
        })
        .collect()
}

/// The window's runs, then its controls run on demand by a body that always does something: the
/// evolution beats it on the fixes and the reviews where no finding is right, so it is kept; the
/// judgement replays from the ledger.
#[tokio::test]
async fn a_swapped_mind_is_stamped_controlled_and_judged() {
    let w = World::new("keep");
    swap(&w);
    for i in 0..EVOLUTION_WINDOW {
        let journal = ask_once(&w, i, script(i, false)).await;
        let beats = journal.read().unwrap().rows;
        if kind(i) == Kind::NoRecall {
            assert!(
                !beats.iter().any(|b| matches!(b, Beat::Recall { .. })),
                "task {i}: no note holds its words, so nothing is recalled"
            );
        }
    }
    let window = runs(&w);
    assert_eq!(
        window.len(),
        EVOLUTION_WINDOW,
        "every ask after the swap wrote its row"
    );
    assert!(window.iter().all(|r| !r.control
        && r.swap.as_deref() == Some("careful-1")
        && r.gate == Some(Gate::Pass)));
    // The controls: the same asks, by the same body, on the before state, in scratch worlds.
    let replies: Vec<_> = (0..EVOLUTION_WINDOW)
        .flat_map(|i| script(i, true))
        .collect();
    let config = w.config();
    let said = std::cell::RefCell::new(Vec::<String>::new());
    let say = |l: &str| said.borrow_mut().push(l.to_owned());
    let c = Controller {
        model: Arc::new(ScriptedModel::new(replies)),
        model_id: "claude-sonnet-5-5".into(),
        config: &config,
        ready,
        usage: w.usage(),
        session: "judge-1".into(),
        say: &say,
    };
    let lines = run_controls(&w.world(), w.ledger().path(), "careful-1", &c)
        .await
        .unwrap();
    assert_eq!(lines.len(), EVOLUTION_WINDOW, "{lines:?}");
    assert!(
        said.borrow()[0].starts_with("10 control(s) to run through claude-sonnet-5-5"),
        "said before any is spent: {:?}",
        said.borrow()
    );
    // Every control's call is on the live world's usage ledger, under the judge's session.
    let spent = w.usage().read().unwrap().rows;
    assert_ne!(spent.len(), 0, "the controls spent on the live ledger");
    assert!(spent.iter().all(|r| r.session == "judge-1"));
    assert_eq!(
        fs::read_to_string(w.world().join(MIND)).unwrap(),
        NEW,
        "the controls never touch the world"
    );
    let controls: Vec<_> = runs(&w).into_iter().filter(|r| r.control).collect();
    assert_eq!(controls.len(), EVOLUTION_WINDOW);
    for (i, c) in controls.iter().enumerate() {
        let want = match kind(i) {
            Kind::Fix | Kind::NoFinding => Gate::Fail,
            Kind::Nothing | Kind::NoRecall => Gate::Pass,
        };
        assert_eq!(c.gate, Some(want), "control {i}");
    }
    // Run again, nothing waits: no model is called.
    let rerun = run_controls(&w.world(), w.ledger().path(), "careful-1", &c)
        .await
        .unwrap();
    assert_eq!(
        rerun,
        Vec::<String>::new(),
        "nothing waits: no model is called"
    );
    let head = w.ledger().head().unwrap();
    let read = evolution::read(w.ledger().path(), head.as_ref()).unwrap();
    let judged = evolution::judge(&read, "careful-1", mnemorph_config::MIN_NET_WINS);
    let Judgement::Keep(lines) = &judged else {
        panic!("{judged}");
    };
    // Three fixes and two reviews won, nothing lost.
    assert_eq!(lines[0].meter, "netWins");
    assert!((lines[0].reading - 5.0).abs() < 1e-9, "{judged}");
    let to = lines[0].to.unwrap();
    let again = evolution::replay(
        w.ledger().path(),
        "careful-1",
        to,
        head.as_ref(),
        mnemorph_config::MIN_NET_WINS,
    )
    .unwrap();
    assert_eq!(
        again, judged,
        "the ledger read again gives the same judgement"
    );
}

/// The fixtures where doing nothing is right, run with the right output and with a body that
/// always does something: the same gate word every time, and only the right output lands.
#[tokio::test]
async fn the_do_nothing_fixtures_are_deterministic() {
    let w = World::new("fixtures");
    for i in (0..EVOLUTION_WINDOW).filter(|&i| matches!(kind(i), Kind::NoFinding | Kind::NoRecall))
    {
        for wrong in [false, true] {
            let mut words = Vec::new();
            for round in 0..2 {
                let at = World::new(&format!("fixture-{i}-{wrong}-{round}"));
                let beats = ask_once(&at, i, script(i, wrong))
                    .await
                    .read()
                    .unwrap()
                    .rows;
                let gate = beats.iter().rev().find_map(|b| match b {
                    Beat::Gate { verdict, .. } => Some(verdict.clone()),
                    _ => None,
                });
                assert!(
                    !beats.iter().any(|b| matches!(b, Beat::Recall { .. })),
                    "{i}: no recall hit"
                );
                words.push(gate);
            }
            assert_eq!(words[0], words[1], "task {i} ({wrong}) is deterministic");
            let landed = words[0].as_deref().is_none_or(|v| v == "PASS");
            let want = !(wrong && kind(i) == Kind::NoFinding);
            assert_eq!(
                landed, want,
                "task {i}, always-something {wrong}: {words:?}"
            );
        }
    }
    // The no-recall fixture is not empty by accident: an ask in the note's own words recalls it.
    let journal: Journal<Beat> = Journal::new(w.root.join("hit.jsonl"));
    let (body, cx) = solo(&w.world(), "claude-sonnet-5-5");
    let setup = Setup {
        model: Arc::new(ScriptedModel::new([ScriptedModel::text("noted")])),
        tools: registry(),
        cx: ready(cx, &w.root),
        journal: journal.clone(),
        ledger: None,
        prices: anthropic_prices().unwrap(),
        caller: Caller {
            session: "s".into(),
            run: "hit".into(),
            body: "rimuru".into(),
            race: "rimuru".into(),
            commission: None,
            prefix: 0,
        },
    };
    let mut run = Run::new(body, Limits::for_race(Race::Rimuru), setup);
    let mut quiet = |_| {};
    run.ask(
        "What does the deploy pipeline need exported before compose runs?",
        &mut quiet,
    )
    .await;
    assert!(
        journal
            .read()
            .unwrap()
            .rows
            .iter()
            .any(|b| matches!(b, Beat::Recall { anchors, .. } if !anchors.is_empty())),
        "the note is recalled by its own words"
    );
}

/// A spent budget cap runs no control: the judge says so, and nothing is written.
#[tokio::test]
async fn a_spent_cap_runs_no_control() {
    let w = World::new("capped");
    swap(&w);
    for i in 0..EVOLUTION_WINDOW {
        ask_once(&w, i, script(i, false)).await;
    }
    fs::write(w.root.join("global.toml"), "[budget]\nday = 10\n").unwrap();
    w.usage()
        .append(&mnemorph_ledger::UsageRow {
            ts: mnemorph_ledger::Timestamp::now(),
            session: "earlier".into(),
            run: "r".into(),
            body: "rimuru".into(),
            race: "rimuru".into(),
            commission: None,
            model: "m".into(),
            provider: "p".into(),
            usage: mnemorph_ledger::Usage::default(),
            prefix: 0,
            wt: 50,
            usd: None,
            envelope: None,
            purpose: None,
        })
        .unwrap();
    let config = w.config();
    let c = Controller {
        model: Arc::new(ScriptedModel::new([])),
        model_id: "claude-sonnet-5-5".into(),
        config: &config,
        ready,
        usage: w.usage(),
        session: "judge-2".into(),
        say: &|_| {},
    };
    let e = run_controls(&w.world(), w.ledger().path(), "careful-1", &c)
        .await
        .unwrap_err();
    assert!(e.contains("cap is spent"), "{e}");
    assert!(runs(&w).iter().all(|r| !r.control), "no control written");
}
