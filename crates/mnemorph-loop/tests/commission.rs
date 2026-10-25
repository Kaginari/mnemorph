//! One order, down and back: Rimuru commissions an Elf, the Elf its Orc, the Orc its Slime; the
//! Slime lands under its own gate, and each report climbs the chain as its dispatcher's tool
//! result. A body that strays off its ground has the stray put back.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use mnemorph_ledger::Journal;
use mnemorph_loop::{
    Beat, Body, CommissionTool, Commissioner, Ending, Limits, Run, ScriptedModel, Setup,
};
use mnemorph_provider::{Caller, anthropic_prices};
use mnemorph_races::Race;
use mnemorph_tools::{Context, Sandbox, Tool, registry};
use serde_json::json;

struct World(PathBuf);

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-commission-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("lib")).unwrap();
        let w = World(root);
        w.doc("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n");
        w.doc(
            "elf",
            "e",
            "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n- **Territory:** src/\n",
        );
        fs::write(
            w.0.join(".mnemorph/elf/e/plan.md"),
            "- [ ] orc-o: the done marker\n",
        )
        .unwrap();
        w.doc(
            "orc",
            "o",
            "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** src/\n",
        );
        w.doc(
            "slime",
            "s",
            "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n- **Verify:** `test -f src/done.txt`\n",
        );
        w
    }

    /// This world's own attic, beside it: never one another test shares.
    fn attic(&self) -> PathBuf {
        let mut name = self.0.file_name().unwrap().to_os_string();
        name.push("-attic");
        self.0.with_file_name(name)
    }

    fn doc(&self, race: &str, short: &str, body: &str) {
        let dir = self.0.join(".mnemorph").join(race).join(short);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), body).unwrap();
    }

    /// Rimuru's run, able to commission, every body riding `model`.
    fn rimuru(&self, model: Arc<ScriptedModel>) -> Run {
        self.rimuru_with(model, true)
    }

    /// As [`World::rimuru`], with or without an attic to put strays back from.
    fn rimuru_with(&self, model: Arc<ScriptedModel>, attic: bool) -> Run {
        let mut cx = Context::new(Race::Rimuru, &self.0, &["."])
            .with_sandbox(Sandbox::None("commission tests run unguarded".into()))
            .with_unguarded(true);
        if attic {
            cx = cx.with_attic(
                mnemorph_tools::Attic::at(&self.attic(), &self.0).expect("git is installed"),
            );
        }
        let mut tools = registry();
        tools.push(Box::new(CommissionTool) as Box<dyn Tool>);
        let journals = self.0.join(".mnemorph/instruments/loop");
        let commissioner = Commissioner {
            world: self.0.canonicalize().unwrap(),
            model: model.clone(),
            prices: anthropic_prices().unwrap(),
            ledger: None,
            journals: journals.clone(),
            session: "s".into(),
            caps: None,
            stress_tokens: None,
            busy: std::sync::Mutex::default(),
            observer: None,
            queue: Some(mnemorph_loop::world::Queue::at(&queue_of(&self.0))),
        };
        let setup = Setup {
            model,
            tools,
            cx,
            journal: Journal::new(journals.join("rimuru.jsonl")),
            ledger: None,
            prices: anthropic_prices().unwrap(),
            caller: Caller {
                session: "s".into(),
                run: "rimuru".into(),
                body: "rimuru".into(),
                race: "rimuru".into(),
                commission: None,
                prefix: 0,
            },
        };
        let body = Body {
            name: "rimuru".into(),
            race: Race::Rimuru,
            model: "claude-sonnet-5-5".into(),
            system: "You are Rimuru.".into(),
        };
        Run::new(body, Limits::for_race(Race::Rimuru), setup)
            .with_commissioner(Arc::new(commissioner))
    }

    fn results(&self) -> Vec<String> {
        Journal::<Beat>::new(self.0.join(".mnemorph/instruments/loop/rimuru.jsonl"))
            .read()
            .unwrap()
            .rows
            .into_iter()
            .filter_map(|b| match b {
                Beat::Result { content, .. } => Some(content),
                _ => None,
            })
            .collect()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.attic());
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn one_order_down_and_back() {
    let w = World::new("down");
    let order = |id: &str, to: &str| {
        ScriptedModel::tool(
            id,
            "commission",
            json!({"to": to, "ask": "leave src/done.txt"}),
        )
    };
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::tool(
            "w1",
            "write",
            json!({"path": "src/done.txt", "content": "done\n"}),
        ),
        ScriptedModel::text("@S DONE\n@W src/done.txt\n@U territory src/ build=cargo test\n@E"),
        ScriptedModel::text("@S DONE\n@V the landing holds\n@$ slime-s: add the tab test next\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("Done: src/done.txt is there."),
    ]));
    let mut run = w.rimuru(model.clone());
    let ending = run.ask("leave a done marker", &mut |_| {}).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    assert_eq!(
        fs::read_to_string(w.0.join("src/done.txt")).unwrap(),
        "done\n"
    );
    let results = w.results();
    assert_eq!(results.len(), 1, "{results:?}");
    assert!(
        results[0].starts_with("@S DONE"),
        "the Elf's report is Rimuru's tool result: {results:?}"
    );
    assert_eq!(model.requests().len(), 8);
    // What the Slime learned about its territory is a fact, in its name.
    let facts = mnemorph_loop::world::Facts::load(&w.0);
    let f = facts
        .live()
        .find(|f| f.about == "src")
        .expect("the Slime's fact was filed");
    assert_eq!(f.text, "build=cargo test");
    assert!(f.supporters.contains("slime-s"), "{f:?}");
    // The Orc's order for its Slime is held on the shared queue, in the Orc's name.
    let held = mnemorph_loop::world::Queue::at(&queue_of(&w.0)).open();
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(
        (held[0].holder(), held[0].by.as_str(), held[0].ask.as_str()),
        ("slime-s", "orc-o", "add the tab test next")
    );
}

#[tokio::test]
async fn a_refused_order_names_its_route_and_a_stray_is_put_back() {
    let w = World::new("stray");
    let model = Arc::new(ScriptedModel::new([
        // Rimuru reaches past its Elf: refused, naming the next rank.
        ScriptedModel::tool("c0", "commission", json!({"to": "slime-s", "ask": "x"})),
        ScriptedModel::tool("c1", "commission", json!({"to": "elf-e", "ask": "x"})),
        ScriptedModel::tool("c2", "commission", json!({"to": "orc-o", "ask": "x"})),
        ScriptedModel::tool("c3", "commission", json!({"to": "slime-s", "ask": "x"})),
        // The Slime's command writes off its ground; its gate fails twice.
        ScriptedModel::tool(
            "b1",
            "bash",
            json!({"command": "echo stray > lib/stray.txt && echo done > src/done.txt"}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S FAIL its landing was refused\n@E"),
        ScriptedModel::text("@S FAIL\n@E"),
        ScriptedModel::text("It failed."),
    ]));
    let mut run = w.rimuru(model.clone());
    let ending = run.ask("go", &mut |_| {}).await;
    assert_eq!(
        model.requests().len(),
        10,
        "every body ran, the Slime's gate twice"
    );
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    let results = w.results();
    assert!(
        results[0].contains("commission elf-e, the next rank toward slime-s"),
        "{results:?}"
    );
    assert!(
        !w.0.join("lib/stray.txt").exists(),
        "the stray was put back"
    );
    assert_eq!(
        model.requests().len(),
        10,
        "every body ran, the Slime's gate twice"
    );
    assert!(
        !w.0.join("src/done.txt").exists(),
        "a failed landing is put back whole"
    );
}

fn order(id: &str, to: &str) -> Result<mnemorph_provider::Response, mnemorph_provider::Error> {
    ScriptedModel::tool(
        id,
        "commission",
        json!({"to": to, "ask": "leave src/done.txt"}),
    )
}

/// An Orc that rewrites what its Slime landed is judged for it: its baseline took the landing,
/// so the rewrite is its own change, and right author refuses it.
#[tokio::test]
async fn a_dispatcher_that_rewrites_court_work_is_judged_for_it() {
    let w = World::new("tamper");
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::tool(
            "w1",
            "write",
            json!({"path": "src/done.txt", "content": "done\n"}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        // The Orc, back from its commission, rewrites the Slime's file through a command.
        ScriptedModel::tool(
            "b1",
            "bash",
            json!({"command": "echo tampered > src/done.txt"}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S FAIL\n@E"),
        ScriptedModel::text("It failed."),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("go", &mut |_| {}).await;
    assert_eq!(
        model.requests().len(),
        10,
        "the Orc's gate refused it twice"
    );
    let results = w.results();
    assert!(results[0].starts_with("@S FAIL"), "{results:?}");
    assert!(
        !w.0.join("src/done.txt").exists(),
        "a failed subtree lands nothing"
    );
}

/// A stray that changed an existing file is put back from the attic, and the gates above see
/// nothing of it: the put-back path is in their baselines.
#[tokio::test]
async fn a_modified_stray_put_back_fails_no_gate_above() {
    let w = World::new("modified");
    fs::write(w.0.join("lib/x.txt"), "old\n").unwrap();
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::tool(
            "b1",
            "bash",
            json!({"command": "echo stray >> lib/x.txt && echo done > src/done.txt"}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE the Slime failed; its work was put back\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("Reported."),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("go", &mut |_| {}).await;
    assert_eq!(
        model.requests().len(),
        9,
        "the Orc's and the Elf's gates passed first time"
    );
    assert_eq!(fs::read_to_string(w.0.join("lib/x.txt")).unwrap(), "old\n");
    assert!(w.results()[0].starts_with("@S DONE"), "{:?}", w.results());
}

/// The human's Esc reaches a body three ranks down.
#[tokio::test]
async fn esc_stops_the_child() {
    let w = World::new("esc");
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::tool(
            "b1",
            "bash",
            json!({"command": "touch src/running; sleep 20"}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("never"),
    ]));
    let mut run = w.rimuru(model);
    let interrupt = run.interrupt();
    let running = w.0.join("src/running");
    // A thread of its own: the command blocks the test's runtime while it runs, as in the loop.
    std::thread::spawn(move || {
        while !running.exists() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        interrupt.fire();
    });
    let started = std::time::Instant::now();
    let ending = run.ask("go", &mut |_| {}).await;
    assert!(matches!(ending, Ending::Interrupted), "{ending:?}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}

/// A body never commissions itself: one court per seat.
#[tokio::test]
async fn one_court_per_seat() {
    let w = World::new("seat");
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "orc-o"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("done"),
    ]));
    let mut run = w.rimuru(model);
    run.ask("go", &mut |_| {}).await;
    let orc = fs::read_dir(w.0.join(".mnemorph/instruments/loop"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("orc-o-"))
        .count();
    assert_eq!(orc, 1, "the Orc's second court never opened");
}

/// A child that answers in prose, off the wire, is a FAIL to its dispatcher.
#[tokio::test]
async fn an_answer_off_the_wire_is_a_fail() {
    let w = World::new("prose");
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        ScriptedModel::text(
            "I could not find anything to do.\n@U territory src/ build=guess\n@$ orc-o: try again",
        ),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru(model);
    run.ask("go", &mut |_| {}).await;
    assert!(
        w.results()[0].starts_with("@S FAIL off the wire"),
        "{:?}",
        w.results()
    );
    // Lines that look like the wire inside prose are not the wire: nothing is filed or held.
    assert_eq!(mnemorph_loop::world::Facts::load(&w.0).live().count(), 0);
    assert!(
        mnemorph_loop::world::Queue::at(&queue_of(&w.0))
            .items()
            .is_empty()
    );
}

/// A dispatcher's own write before its commission stays its own to answer for, even when the
/// child touches the same file: the baseline keeps what the dispatcher had changed.
#[tokio::test]
async fn a_dispatchers_own_write_is_not_laundered_by_its_child() {
    let w = World::new("launder");
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        ScriptedModel::tool(
            "b0",
            "bash",
            json!({"command": "echo illegal > src/illegal.txt"}),
        ),
        order("c3", "slime-s"),
        ScriptedModel::tool(
            "b1",
            "bash",
            json!({"command": "echo more >> src/illegal.txt && echo done > src/done.txt"}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S FAIL\n@E"),
        ScriptedModel::text("It failed."),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("go", &mut |_| {}).await;
    assert_eq!(
        model.requests().len(),
        10,
        "the Orc's gate refused its own write twice"
    );
    assert!(w.results()[0].starts_with("@S FAIL"), "{:?}", w.results());
    assert!(
        !w.0.join("src/illegal.txt").exists(),
        "the failed subtree is put back"
    );
}

/// A stray that cannot be put back stays in every dispatcher's view: no gate absorbs it.
#[tokio::test]
async fn a_stray_that_cannot_be_put_back_is_refused_above() {
    let w = World::new("noattic");
    fs::write(w.0.join("lib/x.txt"), "old\n").unwrap();
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::tool("b1", "bash", json!({"command": "echo stray >> lib/x.txt"})),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S FAIL\n@E"),
        ScriptedModel::text("@S FAIL\n@E"),
        ScriptedModel::text("It failed."),
    ]));
    let mut run = w.rimuru_with(model.clone(), false);
    run.ask("go", &mut |_| {}).await;
    assert_eq!(
        model.requests().len(),
        11,
        "each gate above refused the stray twice"
    );
    assert!(w.results()[0].starts_with("@S FAIL"), "{:?}", w.results());
}

/// A commission with nothing to carve is refused before any body runs.
#[tokio::test]
async fn a_zero_slice_is_refused() {
    let w = World::new("zero");
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "elf-e", "ask": "x", "budget_wt": 0}),
        ),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru(model);
    run.ask("go", &mut |_| {}).await;
    assert!(
        w.results()[0].contains("no budget left"),
        "{:?}",
        w.results()
    );
}

/// An Esc already pending on the shared flag when a commissioned body starts is its own: it stops
/// before any call. A body that does not share its dispatcher's flag starts clean.
#[tokio::test]
async fn an_esc_pending_at_a_shared_start_is_honoured() {
    let w = World::new("pending");
    let model = Arc::new(ScriptedModel::new([ScriptedModel::text("never")]));
    let shared = Arc::new(mnemorph_loop::run::Interrupt::default());
    shared.fire();
    let mut run = w.rimuru(model.clone()).with_interrupt(Arc::clone(&shared));
    let ending = run.ask("go", &mut |_| {}).await;
    assert!(matches!(ending, Ending::Interrupted), "{ending:?}");
    assert_eq!(model.requests().len(), 0);
    let model = Arc::new(ScriptedModel::new([ScriptedModel::text("ok")]));
    let mut own = w.rimuru(model.clone());
    own.interrupt().fire();
    let ending = own.ask("go", &mut |_| {}).await;
    assert!(
        matches!(ending, Ending::Answered(_)),
        "an Esc from before the ask is not this ask's: {ending:?}"
    );
}

/// A session opened in a world with a throne and Elves can commission: the human's Rimuru hands
/// the order down the same way the scripted Rimuru does. A world with no creatures stays solo.
#[tokio::test]
async fn a_session_in_a_world_with_creatures_dispatches() {
    use mnemorph_loop::open::{Bound, open_with};
    use mnemorph_loop::{Config, Paths};
    let w = World::new("session");
    // A test binary has no guard beside it; the human allows commands unguarded, as a session
    // without the guard installed would have to.
    fs::write(
        w.0.join(".mnemorph/config.local.toml"),
        "[sandbox]\nallow_unguarded = true\n",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::tool(
            "w1",
            "write",
            json!({"path": "src/done.txt", "content": "done\n"}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("Done."),
    ]));
    let bodies: Arc<std::sync::Mutex<std::collections::BTreeSet<String>>> = Arc::default();
    let config = Config::load(&Paths::with_global(w.attic().join("no-global.toml"), &w.0)).unwrap();
    let attic = w.attic();
    let mut opened = open_with(
        &w.0,
        &config,
        Bound {
            model: model.clone(),
            model_id: "scripted-model",
            attic: Some(&attic),
        },
        Arc::new(mnemorph_tools::NoHuman),
        {
            let seen = Arc::clone(&bodies);
            Arc::new(move |s: &mnemorph_loop::Seen| {
                let mut at = s;
                let mut who = None;
                while let mnemorph_loop::Seen::From { body, seen, .. } = at {
                    who = Some(body.clone());
                    at = seen;
                }
                if let Some(w) = who {
                    seen.lock().unwrap().insert(w);
                }
            })
        },
        |_| None,
    )
    .unwrap();
    let ending = opened
        .session
        .run()
        .ask("leave a done marker", &mut |_| {})
        .await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    assert_eq!(
        fs::read_to_string(w.0.join("src/done.txt")).unwrap(),
        "done\n"
    );
    assert_eq!(model.requests().len(), 8);
    // The human sees every body working below, under its own name.
    let seen: Vec<String> = bodies.lock().unwrap().iter().cloned().collect();
    assert_eq!(seen, ["elf-e", "orc-o", "slime-s"]);
    // The model was offered the tool and told the lanes.
    let first = &model.requests()[0];
    assert!(
        first.tools.iter().any(|t| t.name == "commission"),
        "the session offers commission"
    );
    let system = format!("{:?}", first.system);
    assert!(
        system.contains("elf-e: src/"),
        "the lanes are named: {system}"
    );
}

/// A world with no throne and no Elves stays solo: `commission` is no tool there.
#[tokio::test]
async fn a_world_without_creatures_stays_solo() {
    use mnemorph_loop::open::{Bound, open_with};
    use mnemorph_loop::{Config, Paths};
    // No throne, no Elves: solo, and `commission` is no tool here.
    let bare =
        std::env::temp_dir().join(format!("mnemorph-commission-bare-{}", std::process::id()));
    let _ = fs::remove_dir_all(&bare);
    fs::create_dir_all(&bare).unwrap();
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        ScriptedModel::text("alone"),
    ]));
    let config = Config::load(&Paths::with_global(bare.join("no-global.toml"), &bare)).unwrap();
    let attic = bare.join("../mnemorph-commission-bare-attic");
    let mut opened = open_with(
        &bare,
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
    opened.session.run().ask("go", &mut |_| {}).await;
    assert!(
        !model.requests()[0]
            .tools
            .iter()
            .any(|t| t.name == "commission")
    );
    let journals: Vec<_> = fs::read_dir(bare.join(".mnemorph/instruments/loop"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert_eq!(journals.len(), 1, "no child ran");
    let _ = fs::remove_dir_all(&bare);
    let _ = fs::remove_dir_all(&attic);
}

/// Each test world's queue, beside it: tests never write the human's state directory.
fn queue_of(world: &std::path::Path) -> std::path::PathBuf {
    let mut q = world.as_os_str().to_owned();
    q.push("-queue.jsonl");
    std::path::PathBuf::from(q)
}

/// The world writes the queue's witness where the tripwire reads it.
#[test]
fn the_queue_witness_is_one_place() {
    let world = std::path::Path::new("/some/world");
    assert_eq!(
        mnemorph_loop::world::queue::witness_of(world),
        mnemorph_tools::queue_witness(world)
    );
}

/// At a turn's end the wake runs each held order its body can carry out — claimed in its name,
/// commissioned by the body it reports to — closes it on DONE, leaves an order off its body's
/// ground held with the reason, and opens the next turn with what it ran.
#[tokio::test]
async fn the_wake_runs_what_its_body_can_carry() {
    use mnemorph_loop::world::queue::State;
    let w = World::new("wake");
    fs::write(w.0.join("src/done.txt"), "done\n").unwrap();
    fs::write(w.0.join("lib/x.rs"), "x\n").unwrap();
    let q = mnemorph_loop::world::Queue::at(&queue_of(&w.0));
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let can = q
        .add(&roster, Some("slime-s"), "orc-o", "write src/tab.txt", 0)
        .unwrap();
    let cannot = q
        .add(&roster, Some("slime-s"), "orc-o", "fix lib/x.rs", 1)
        .unwrap();
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "w1",
            "write",
            json!({"path": "src/tab.txt", "content": "tab\n"}),
        ),
        ScriptedModel::text("@S DONE\n@W src/tab.txt\n@E"),
        ScriptedModel::text("Seen."),
    ]));
    let mut run = w.rimuru(model.clone());
    run.wake(&Ending::Answered(String::new())).await;
    assert_eq!(
        fs::read_to_string(w.0.join("src/tab.txt")).unwrap(),
        "tab\n"
    );
    let items = q.items();
    assert_eq!(items[&can.id].state, State::Done, "{:?}", items[&can.id]);
    let held = &items[&cannot.id];
    assert_eq!(held.state, State::Held);
    assert!(
        held.notes
            .last()
            .unwrap()
            .contains("`lib/x.rs` is not on the ground of slime-s"),
        "{held:?}"
    );
    // A second wake leaves the waiting order's notes as they were.
    run.wake(&Ending::Answered(String::new())).await;
    assert_eq!(q.items()[&cannot.id].notes, held.notes);
    // The next turn opens with what the wake ran.
    run.ask("anything new?", &mut |_| {}).await;
    let last = format!("{:?}", model.requests().last().unwrap());
    assert!(
        last.contains("While you waited, the shared queue ran"),
        "{last}"
    );
    assert!(
        last.contains(&format!("queue {}: slime-s reported DONE", can.id)),
        "{last}"
    );
}

/// An Orc carries an order on its Slimes' ground; an interrupted turn wakes nothing; a path that
/// climbs out is on no ground; an order stopped short waits again.
#[tokio::test]
async fn the_wake_reads_ground_below_and_stops_for_the_human() {
    use mnemorph_loop::world::queue::State;
    let w = World::new("wake-below");
    fs::write(w.0.join("src/done.txt"), "done\n").unwrap();
    let q = mnemorph_loop::world::Queue::at(&queue_of(&w.0));
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let orc = q
        .add(&roster, Some("orc-o"), "elf-e", "check src/done.txt:3", 0)
        .unwrap();
    let climbs = q
        .add(&roster, Some("slime-s"), "orc-o", "edit src/../lib", 1)
        .unwrap();
    let model = Arc::new(ScriptedModel::new([ScriptedModel::text("@S DONE\n@E")]));
    let mut run = w.rimuru(model.clone());
    // The human's Esc ended the turn: nothing runs.
    run.wake(&Ending::Interrupted).await;
    assert_eq!(model.requests().len(), 0);
    assert_eq!(q.items()[&orc.id].state, State::Held);
    run.wake(&Ending::Answered(String::new())).await;
    let items = q.items();
    assert_eq!(items[&orc.id].state, State::Done, "{:?}", items[&orc.id]);
    let held = &items[&climbs.id];
    assert_eq!(held.state, State::Held);
    assert!(
        held.notes.last().unwrap().contains("is not on the ground"),
        "{held:?}"
    );
}

/// An order stopped by the human mid-run waits again, its note said once.
#[tokio::test]
async fn an_interrupted_order_waits_again() {
    use mnemorph_loop::world::queue::State;
    let w = World::new("wake-again");
    let q = mnemorph_loop::world::Queue::at(&queue_of(&w.0));
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let item = q
        .add(&roster, Some("orc-o"), "elf-e", "look around", 0)
        .unwrap();
    let model = Arc::new(ScriptedModel::new([ScriptedModel::text(
        "@S CHECKPOINT clock\n@E",
    )]));
    let mut run = w.rimuru(model);
    run.wake(&Ending::Answered(String::new())).await;
    let now = &q.items()[&item.id];
    assert_eq!(now.state, State::Held, "{now:?}");
    assert!(
        now.notes
            .last()
            .unwrap()
            .contains("waits again: CHECKPOINT clock"),
        "{now:?}"
    );
}

/// Bodies read the queue for free and move only what they hold or stand over; there is no claim.
#[tokio::test]
async fn the_queue_tool_follows_the_rank_law() {
    use mnemorph_loop::world::queue::State;
    let w = World::new("queue-tool");
    fs::write(w.0.join("src/done.txt"), "done\n").unwrap();
    let q = mnemorph_loop::world::Queue::at(&queue_of(&w.0));
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let low = q
        .add(&roster, Some("slime-s"), "orc-o", "tidy src/", 0)
        .unwrap();
    let high = q
        .add(&roster, Some("orc-o"), "elf-e", "plan the lane", 0)
        .unwrap();
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool("q1", "queue", json!({"action": "list"})),
        ScriptedModel::tool("q2", "queue", json!({"action": "close", "id": low.id})),
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::tool("q3", "queue", json!({"action": "drop", "id": high.id})),
        ScriptedModel::tool("q4", "queue", json!({"action": "claim", "id": high.id})),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("go", &mut |_| {}).await;
    let results = w.results();
    assert!(
        results[0].contains(&low.id) && results[0].contains(&high.id),
        "{results:?}"
    );
    let items = q.items();
    assert_eq!(
        items[&low.id].state,
        State::Done,
        "Rimuru stands over every body"
    );
    assert_eq!(
        items[&high.id].state,
        State::Held,
        "a Slime does not stand over its Orc"
    );
    let seen = format!("{:?}", model.requests());
    assert!(
        seen.contains("refused: orc-o holds it"),
        "the Slime is told why"
    );
    assert!(seen.contains("queue has no `claim`"), "a body never claims");
}
