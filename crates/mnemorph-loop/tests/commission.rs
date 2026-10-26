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
        self.rimuru_with(model, true, None)
    }

    /// As [`World::rimuru`], with or without an attic to put strays back from.
    fn rimuru_with(&self, model: Arc<ScriptedModel>, attic: bool, budget: Option<u64>) -> Run {
        self.rimuru_full(model, attic, budget, None, 0)
    }

    /// As [`World::rimuru`], over any model: one that answers from what it is shown.
    fn rimuru_dyn(&self, model: Arc<dyn mnemorph_loop::Model>) -> Run {
        self.rimuru_full(model, true, None, None, 0)
    }

    /// As [`World::rimuru_with`], each court's model chosen by `router`.
    fn rimuru_full(
        &self,
        model: Arc<dyn mnemorph_loop::Model>,
        attic: bool,
        budget: Option<u64>,
        router: Option<mnemorph_loop::commission::Router>,
        slice_floor: u64,
    ) -> Run {
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
        tools.push(Box::new(mnemorph_loop::QueueTool) as Box<dyn Tool>);
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
            council: mnemorph_loop::council::Thresholds::default(),
            router,
            slice_floor,
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
        let mut limits = Limits::for_race(Race::Rimuru);
        limits.budget_wt = budget;
        Run::new(body, limits, setup).with_commissioner(Arc::new(commissioner))
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
    // Every gate verdict reached the journal through its writer.
    let log = fs::read_to_string(w.0.join(".mnemorph/log.md")).unwrap();
    assert!(
        log.contains("] slime-s — gate: PASS\n- **Changed:** `src/done.txt`"),
        "{log}"
    );
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

/// No body deletes what it did not write: a foreign untracked file — the human's notes, outside
/// the Slime's zone — survives a cleanup turn that wipes its folder. The shell says the attic kept
/// it, the Slime's gate fails the landing for touching what is not its own, and the landing is put
/// back whole: the notes return, and the Slime's own write goes with the rest.
#[tokio::test]
async fn a_foreign_untracked_file_survives_a_cleanup_turn() {
    let w = World::new("foreign");
    fs::create_dir_all(w.0.join("notes")).unwrap();
    fs::write(w.0.join("notes/human.md"), "mine, not yours\n").unwrap();
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::tool(
            "b1",
            "bash",
            json!({"command": "rm -rf notes && echo done > src/done.txt"}),
        ),
        ScriptedModel::text("@S DONE cleaned up\n@E"),
        ScriptedModel::text("@S DONE cleaned up\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("Reported."),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("tidy up", &mut |_| {}).await;
    let results: Vec<String> = fs::read_dir(w.0.join(".mnemorph/instruments/loop"))
        .unwrap()
        .flat_map(|j| {
            fs::read_to_string(j.unwrap().path())
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|l| l.contains("\"beat\":\"result\""))
        .collect();
    let said = |needle: &str| results.iter().any(|l| l.contains(needle));
    assert!(said("kept in the attic"), "{results:?}");
    assert!(said("right author: `notes/human.md`"), "{results:?}");
    assert_eq!(
        fs::read_to_string(w.0.join("notes/human.md")).unwrap(),
        "mine, not yours\n",
        "the human's notes were put back"
    );
    assert!(
        !w.0.join("src/done.txt").exists(),
        "a failed landing is put back whole"
    );
}

/// The write rule through a commission: a Slime's `@U` claim lands personal and proposed, and
/// its landing — a verify line over a real diff — grounds it into shared memory by **checked**;
/// a territory fact goes to the ontology, never memory.
#[tokio::test]
async fn a_checked_landing_grounds_its_claims_and_a_bare_answer_does_not() {
    use mnemorph_memory::{Status, Tier, fold, read};
    for landing in [true, false] {
        let w = World::new(if landing {
            "remember-landed"
        } else {
            "remember-bare"
        });
        let act = if landing {
            "echo done > src/done.txt"
        } else {
            "true"
        };
        let model = Arc::new(ScriptedModel::new([
            order("c1", "elf-e"),
            order("c2", "orc-o"),
            order("c3", "slime-s"),
            ScriptedModel::tool("b1", "bash", json!({"command": act})),
            ScriptedModel::text(
                "@S DONE\n@U law src/done.txt marks the work done\n@U law the parser keeps interval fractions\n@U territory src/: the parser lives here\n@E",
            ),
            // The Orc wrote nothing itself: its gate ran nothing, whatever its Slime landed.
            ScriptedModel::text("@S DONE\n@U law the orc thinks the slime was right\n@E"),
            ScriptedModel::text("@S DONE\n@E"),
            ScriptedModel::text("@S DONE\n@E"),
            ScriptedModel::text("Reported."),
        ]));
        let mut run = w.rimuru(model.clone());
        run.ask("go", &mut |_| {}).await;
        let mine = fold(&read(&w.0, &Tier::Personal("slime-s".into())).unwrap().rows);
        let shared = fold(&read(&w.0, &Tier::Shared).unwrap().rows);
        let mut texts: Vec<&str> = mine.values().map(|n| n.text.as_str()).collect();
        texts.sort_unstable();
        assert_eq!(
            texts,
            [
                "src/done.txt marks the work done",
                "the parser keeps interval fractions"
            ],
            "territory is not memory"
        );
        let of = |t: &str| mine.values().find(|n| n.text == t).unwrap();
        let n = of("src/done.txt marks the work done");
        // A check observes only its diff: a claim about the world beyond it stays proposed.
        assert_eq!(
            of("the parser keeps interval fractions").status,
            Status::Proposed
        );
        let orc = fold(&read(&w.0, &Tier::Personal("orc-o".into())).unwrap().rows);
        assert!(
            orc.values().all(|n| n.status == Status::Proposed),
            "a gate that ran nothing grounds nothing: {orc:?}"
        );
        if landing {
            assert_eq!(n.status, Status::Grounded);
            assert_eq!(
                shared.len(),
                1,
                "only the Slime's claim, grounded into shared memory"
            );
            assert!(shared.values().all(|s| s.status == Status::Grounded));
        } else {
            assert_eq!(
                n.status,
                Status::Proposed,
                "a pass over no diff proves nothing"
            );
            assert!(shared.is_empty(), "nothing unconfirmed is shared");
        }
    }
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
    let mut run = w.rimuru_with(model.clone(), false, None);
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
        last.contains(&format!(
            "queue {}: slime-s reported DONE; the order is done",
            can.id
        )),
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

/// A dispatcher asks; the harness grants at most its reserve-kept share for the child's race,
/// scaled by the child's worth, and says so in the report.
#[tokio::test]
async fn a_grant_is_bounded_and_the_dispatcher_is_told() {
    let w = World::new("grant");
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "elf-e", "ask": "look", "budget_wt": 900_000}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru_with(model, true, Some(1_000_000));
    run.ask("go", &mut |_| {}).await;
    // About 1 000 000 left (less Rimuru's own call): 90% kept for grants, an Elf's 60%, worth
    // 0.75 with no record — just under 405 000.
    let result = &w.results()[0];
    let granted: u64 = result
        .split("[granted ")
        .nth(1)
        .and_then(|r| r.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{result}"));
    assert!((400_000..=405_000).contains(&granted), "{result}");
    assert!(
        result.contains("of the 900000 asked: the ceiling for elf-e"),
        "{result}"
    );
}

/// One wake runs at most three orders; the rest wait for the next.
#[tokio::test]
async fn a_wake_runs_at_most_three_orders() {
    use mnemorph_loop::world::queue::State;
    let w = World::new("wake-three");
    let q = mnemorph_loop::world::Queue::at(&queue_of(&w.0));
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let ids: Vec<String> = (0..4)
        .map(|n| {
            q.add(&roster, Some("orc-o"), "elf-e", &format!("look {n}"), 0)
                .unwrap()
                .id
        })
        .collect();
    let done = || ScriptedModel::text("@S DONE\n@E");
    let model = Arc::new(ScriptedModel::new([done(), done(), done()]));
    let mut run = w.rimuru(model);
    run.wake(&Ending::Answered(String::new())).await;
    let items = q.items();
    let states: Vec<State> = ids.iter().map(|id| items[id].state).collect();
    assert_eq!(states, [State::Done, State::Done, State::Done, State::Held]);
}

/// What a dispatcher sets aside from its own envelope is given once, when the child's slice runs
/// out; without it, the child stops at its budget checkpoint.
#[tokio::test]
async fn a_topup_carries_a_child_past_its_slice() {
    for topup in [Some(100_000), None] {
        let w = World::new(if topup.is_some() { "topup" } else { "no-topup" });
        let mut args = json!({"to": "elf-e", "ask": "look", "budget_wt": 1});
        if let Some(t) = topup {
            args["topup_wt"] = json!(t);
        }
        let model = Arc::new(ScriptedModel::new([
            ScriptedModel::tool("c1", "commission", args),
            ScriptedModel::tool("g1", "glob", json!({"pattern": "*"})),
            ScriptedModel::text("@S DONE\n@E"),
            ScriptedModel::text("ok"),
        ]));
        let mut run = w.rimuru_with(model, true, Some(1_000_000));
        run.ask("go", &mut |_| {}).await;
        let result = &w.results()[0];
        if topup.is_some() {
            assert!(
                result.starts_with("@S DONE"),
                "the top-up carried it: {result}"
            );
        } else {
            assert!(result.starts_with("@S CHECKPOINT budget"), "{result}");
        }
    }
}

/// A top-up stays within twice the child's ceiling, and the dispatcher is told what was set aside.
#[tokio::test]
async fn a_topup_is_bounded_by_worth() {
    let w = World::new("topup-cut");
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "elf-e", "ask": "look", "budget_wt": 1, "topup_wt": 900_000}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru_with(model, true, Some(1_000_000));
    run.ask("go", &mut |_| {}).await;
    // Twice the Elf's ceiling (about 405 000), less the slice of 1.
    let result = &w.results()[0];
    let set: u64 = result
        .split("[set aside ")
        .nth(1)
        .and_then(|r| r.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{result}"));
    assert!((800_000..=810_000).contains(&set), "{result}");
}

/// An order that names again the files its body failed on three runs in a row is refused with the
/// lawful next acts; a rewritten order goes through.
#[tokio::test]
async fn three_fails_harden_into_the_lawful_next_acts() {
    let w = World::new("stuck");
    for _ in 0..3 {
        let entry = mnemorph_loop::world::log::Entry {
            body: "elf-e".into(),
            title: "gate: FAIL".into(),
            lines: vec![
                ("Changed".into(), "`src/x.rs`".into()),
                ("By".into(), "the end gate".into()),
            ],
        };
        mnemorph_loop::world::log::append(&w.0, &entry).unwrap();
    }
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "elf-e", "ask": "fix src/x.rs again"}),
        ),
        ScriptedModel::tool(
            "c2",
            "commission",
            json!({"to": "elf-e", "ask": "plan the parser work"}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru(model);
    run.ask("go", &mut |_| {}).await;
    let results = w.results();
    assert!(
        results[0].contains("elf-e failed its last 3 runs on `src/x.rs`: rewrite the order"),
        "{results:?}"
    );
    assert!(results[1].starts_with("@S DONE"), "{results:?}");
}

/// A held order on a stuck file stays held through the wake, its reason noted, never claimed.
#[tokio::test]
async fn a_stuck_order_waits_on_the_queue() {
    use mnemorph_loop::world::queue::State;
    let w = World::new("stuck-queue");
    for _ in 0..3 {
        let entry = mnemorph_loop::world::log::Entry {
            body: "orc-o".into(),
            title: "gate: FAIL".into(),
            lines: vec![
                ("Changed".into(), "`src/x.rs`".into()),
                ("By".into(), "the end gate".into()),
            ],
        };
        mnemorph_loop::world::log::append(&w.0, &entry).unwrap();
    }
    let q = mnemorph_loop::world::Queue::at(&queue_of(&w.0));
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let item = q
        .add(&roster, Some("orc-o"), "elf-e", "fix src/x.rs", 0)
        .unwrap();
    let model = Arc::new(ScriptedModel::new([]));
    let mut run = w.rimuru(model.clone());
    run.wake(&Ending::Answered(String::new())).await;
    let now = &q.items()[&item.id];
    assert_eq!((now.state, now.rev), (State::Held, 2), "{now:?}");
    assert!(
        now.notes[0].contains("failed its last 3 runs on `src/x.rs`"),
        "{now:?}"
    );
    assert_eq!(model.requests().len(), 0);
}

/// `./src/…` is a path inside the world, never an absolute one off every ground.
#[tokio::test]
async fn a_dot_slash_path_is_inside_the_world() {
    use mnemorph_loop::world::queue::State;
    let w = World::new("dot-slash");
    fs::write(w.0.join("src/done.txt"), "done\n").unwrap();
    let q = mnemorph_loop::world::Queue::at(&queue_of(&w.0));
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let item = q
        .add(&roster, Some("orc-o"), "elf-e", "check ./src/done.txt.", 0)
        .unwrap();
    let model = Arc::new(ScriptedModel::new([ScriptedModel::text("@S DONE\n@E")]));
    let mut run = w.rimuru(model);
    run.wake(&Ending::Answered(String::new())).await;
    assert_eq!(q.items()[&item.id].state, State::Done);
}

/// A free fact that resembles an older one reaches the parent as a triage line; the parent's
/// `@V` label is filed, and a label on a fact no body of its said is not.
#[tokio::test]
async fn a_resembling_fact_reaches_the_parent_and_its_label_is_filed() {
    use mnemorph_loop::world::facts::{Event, fact_id};
    let w = World::new("triage");
    fs::write(w.0.join("src/done.txt"), "done\n").unwrap();
    let old = fact_id("src", "build runs make");
    mnemorph_loop::world::Facts::record(
        &w.0,
        &Event::Support {
            id: old.clone(),
            about: "src".into(),
            text: "build runs make".into(),
            by: "elf-e".into(),
            source: None,
        },
    )
    .unwrap();
    let new = fact_id("src", "build runs cargo test");
    let order = |id: &str, to: &str| {
        ScriptedModel::tool(
            id,
            "commission",
            json!({"to": to, "ask": "check the build"}),
        )
    };
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        ScriptedModel::text("@S DONE\n@U territory src/ build runs cargo test\n@E"),
        ScriptedModel::text(&format!(
            "@S DONE\n@V #{new} counter #{old}\n@V #{new} support #{old}\n@V #{old} distinct\n@E"
        )),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("go", &mut |_| {}).await;
    let seen = format!("{:?}", model.requests());
    assert!(
        seen.contains(&format!("@? triage #{new}")),
        "the Orc is shown the triage, as a hole"
    );
    let facts = mnemorph_loop::world::Facts::load(&w.0);
    assert!(facts.facts[&new].labelled, "the Orc's label is filed");
    assert!(
        facts.facts[&old].counters.contains("slime-s"),
        "{:?}",
        facts.facts[&old]
    );
    // The first label stands: the second, the other way, is not filed.
    assert!(!facts.facts[&old].supporters.contains("slime-s"));
    assert!(
        !facts.facts[&old].labelled,
        "a label on the Elf's own fact is not the Orc's to give"
    );
}

/// The throne labels its Elves' resembling facts with `@V` lines in its plain answer.
#[tokio::test]
async fn the_throne_triages_in_its_answer() {
    use mnemorph_loop::world::facts::{Event, fact_id};
    let w = World::new("throne-triage");
    let old = fact_id("src", "build runs make");
    mnemorph_loop::world::Facts::record(
        &w.0,
        &Event::Support {
            id: old.clone(),
            about: "src".into(),
            text: "build runs make".into(),
            by: "orc-o".into(),
            source: None,
        },
    )
    .unwrap();
    let new = fact_id("src", "build runs cargo test");
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "elf-e", "ask": "check the build"}),
        ),
        ScriptedModel::text("@S DONE\n@U territory src/ build runs cargo test\n@E"),
        ScriptedModel::text(&format!(
            "The build moved to cargo.\n```\n@V #{new} support #{old}\n```\n@V #{new} counter #{old}"
        )),
    ]));
    let mut run = w.rimuru(model.clone());
    let ending = run.ask("go", &mut |_| {}).await;
    assert!(
        w.results()[0].contains(&format!("@? triage #{new}")),
        "{:?}",
        w.results()
    );
    run.file_triage(&ending);
    let facts = mnemorph_loop::world::Facts::load(&w.0);
    assert!(facts.facts[&new].labelled);
    assert!(
        facts.facts[&old].counters.contains("elf-e"),
        "{:?}",
        facts.facts[&old]
    );
}

/// Five failed runs: the fifth report carries the council's warrant to the parent, as a hole.
#[tokio::test]
async fn the_council_warrant_reaches_the_parent() {
    let w = World::new("council");
    let mut script = Vec::new();
    for n in 0..5 {
        script.push(ScriptedModel::tool(
            &format!("c{n}"),
            "commission",
            json!({"to": "elf-e", "ask": "plan it"}),
        ));
        script.push(ScriptedModel::text("@S FAIL no plan holds\n@E"));
    }
    script.push(ScriptedModel::text("ok"));
    let model = Arc::new(ScriptedModel::new(script));
    let mut run = w.rimuru(model);
    run.ask("go", &mut |_| {}).await;
    let results = w.results();
    assert!(!results[3].contains("council"), "{results:?}");
    assert!(
        results[4].contains("@? council: elf-e strain 5 — answer as its manager"),
        "{results:?}"
    );
}

/// Lesson (mnemorph-research, budget-loop): a run that runs out of its slice is hardship, never a
/// failure on the card. Counted as one, a body funded thinly fails for want of budget, its worth
/// falls, its next slice is thinner, and it fails again: in the study that loop cost strong bodies
/// 2 to 10 landing points and trapped a third of them at a low worth. A budget checkpoint ends the
/// run before its body answers, so the end gate never judges it and writes no verdict.
#[tokio::test]
async fn a_slice_that_runs_out_is_never_a_failure_on_the_card() {
    let w = World::new("ran-out");
    let order = |id: &str, to: &str, budget: Option<u64>| {
        let mut args = json!({"to": to, "ask": "leave src/done.txt"});
        if let Some(b) = budget {
            args["budget_wt"] = json!(b);
        }
        ScriptedModel::tool(id, "commission", args)
    };
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e", None),
        order("c2", "orc-o", None),
        // A slice of one weighted token: spent by the Slime's first call.
        order("c3", "slime-s", Some(1)),
        ScriptedModel::tool(
            "w1",
            "write",
            json!({"path": "src/done.txt", "content": "done\n"}),
        ),
        ScriptedModel::text("@S ESCALATE\n@? the Slime's slice ran out\n@E"),
        ScriptedModel::text("@S ESCALATE\n@E"),
        ScriptedModel::text("The Slime's slice ran out before it could answer."),
    ]));
    // A cap: under no cap there is nothing to carve, and a slice never runs out.
    let mut run = w.rimuru_with(model.clone(), true, Some(1_000_000));
    let _ = run.ask("leave a done marker", &mut |_| {}).await;

    // The Slime's run ended at its budget checkpoint, before any answer: not vacuous.
    let loops = w.0.join(".mnemorph/instruments/loop");
    let slime_end = fs::read_dir(&loops)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("slime-s"))
        .flat_map(|e| Journal::<Beat>::new(e.path()).read().unwrap().rows)
        .find_map(|b| match b {
            Beat::End { ending } => Some(ending),
            _ => None,
        })
        .expect("the Slime's run ended");
    assert!(slime_end.contains("budget"), "{slime_end}");

    // No verdict reached the journal for it, so its card holds no failure.
    let log = fs::read_to_string(w.0.join(".mnemorph/log.md")).unwrap_or_default();
    assert!(!log.contains("slime-s — gate:"), "{log}");
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let card = mnemorph_loop::world::worth::card(&w.0, &roster, "slime-s");
    assert_eq!(
        (card.passes, card.fails, card.handed_back),
        (0, 0, 0),
        "{card:?}"
    );
    assert_eq!(
        card.worth_milli(),
        750,
        "a body that only ran out still reads as a newcomer"
    );
}

/// A commission's office chooses its court's model: the human's pin for (race, office) rides; an
/// office that is none of the three is refused.
#[tokio::test]
async fn an_office_chooses_the_courts_model() {
    let w = World::new("office");
    fs::write(
        w.0.join(".mnemorph/config.local.toml"),
        "[models.office.elf]\nmelete = \"judge-model\"\n",
    )
    .unwrap();
    let config = mnemorph_config::Config::load(&mnemorph_config::Paths::with_global(
        w.0.join("none.toml"),
        &w.0,
    ))
    .unwrap();
    let router = mnemorph_loop::commission::Router {
        config: Arc::new(config),
        decisions: None,
        provider: "anthropic".into(),
    };
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "elf-e", "ask": "is the plan sound?", "office": "verdict"}),
        ),
        ScriptedModel::text("@S DONE\n@V the plan holds\n@E"),
        ScriptedModel::tool(
            "c2",
            "commission",
            json!({"to": "elf-e", "ask": "x", "office": "chant"}),
        ),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru_full(model.clone(), true, None, Some(router), 0);
    run.ask("go", &mut |_| {}).await;
    let requests = model.requests();
    assert_eq!(
        requests[1].model, "judge-model",
        "the Elf's court rode the pin"
    );
    let results = w.results();
    assert!(
        results[0].contains("[elf-e rode judge-model — the human's pin]"),
        "{results:?}"
    );
    assert!(
        results[1].contains("the offices are findings, verdict and draft, not `chant`"),
        "{results:?}"
    );
}

/// Solo escalates on a signal read by code: an outward act the shelf refused goes to the Kijin, the
/// act itself handed down, and solo's answer stands beside its report; with no Kijin, or a turn
/// with no signal, the ending stands.
#[tokio::test]
async fn solo_escalates_on_an_act_it_does_not_hold() {
    let w = World::new("escalate");
    w.doc(
        "kijin",
        "k",
        "# kijin-k\n\n- **Race:** kijin\n- **Reports to:** rimuru\n- **Post:** .github/\n",
    );
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "b1",
            "bash",
            json!({"command": "curl -fsS https://example.com/release"}),
        ),
        ScriptedModel::text("I cannot reach the network."),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("Nothing to hand down."),
    ]));
    let mut run = w.rimuru(model.clone());
    let ending = run.ask("publish the release notes", &mut |_| {}).await;
    let ending = run.escalate(ending).await;
    let Ending::Answered(text) = &ending else {
        panic!("{ending:?}");
    };
    assert!(
        text.starts_with(
            "I cannot reach the network.\n\nEscalated to kijin-k (an outward act it does not hold)"
        ),
        "{text}"
    );
    let handoff = format!("{:?}", model.requests()[2]);
    assert!(
        handoff.contains("it is yours to do") && handoff.contains("curl -fsS"),
        "{handoff}"
    );
    // No signal: the ending stands.
    let quiet = run.ask("say hello", &mut |_| {}).await;
    let kept = run.escalate(quiet.clone()).await;
    assert_eq!(format!("{kept:?}"), format!("{quiet:?}"));
}

/// With no Kijin to take it, an outward act the shelf refused leaves the turn as it ended.
#[tokio::test]
async fn an_unheld_act_with_no_kijin_stays() {
    let w = World::new("escalate-none");
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "b1",
            "bash",
            json!({"command": "curl -fsS https://example.com"}),
        ),
        ScriptedModel::text("I cannot reach the network."),
    ]));
    let mut run = w.rimuru(model.clone());
    let ending = run.ask("fetch it", &mut |_| {}).await;
    let kept = run.escalate(ending.clone()).await;
    assert_eq!(format!("{kept:?}"), format!("{ending:?}"));
    assert_eq!(model.requests().len(), 2, "nothing was commissioned");
}

/// A slice too small for one call is raised to the floor, and the dispatcher is told.
#[tokio::test]
async fn a_slice_is_at_least_one_call() {
    let w = World::new("floor");
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "elf-e", "ask": "look", "budget_wt": 1}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru_full(
        model,
        true,
        Some(1_000_000),
        None,
        mnemorph_loop::commission::SLICE_FLOOR,
    );
    run.ask("go", &mut |_| {}).await;
    let result = &w.results()[0];
    assert!(
        result.starts_with("@S DONE"),
        "the Elf finished on its floor: {result}"
    );
    assert!(
        result.contains("[granted 20000 wt, not the 1 asked"),
        "{result}"
    );
}

/// The dispatch law reads a plan that stands in the seat; a plan that is a link is no plan.
#[test]
fn a_linked_plan_is_no_plan() {
    let w = World::new("plan-link");
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    let plan = w.0.join(".mnemorph/elf/e/plan.md");
    assert_eq!(
        mnemorph_loop::commission::plan_items(&w.0, &roster, "elf-e"),
        ["orc-o"]
    );
    let elsewhere = w.0.join("src/plan.md");
    fs::rename(&plan, &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &plan).unwrap();
    assert_eq!(
        mnemorph_loop::commission::plan_items(&w.0, &roster, "elf-e"),
        Vec::<String>::new()
    );
}

/// With no cap set there is nothing to carve: a `budget_wt` is not read, and the whole chain runs
/// unbounded — the Elf past its first call, and the Orc it commissions with an ask of its own.
#[tokio::test]
async fn under_no_cap_a_budget_is_not_read() {
    let w = World::new("no-cap");
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "elf-e", "ask": "look", "budget_wt": 1}),
        ),
        ScriptedModel::tool("g1", "glob", json!({"pattern": "*"})),
        ScriptedModel::tool(
            "c2",
            "commission",
            json!({"to": "orc-o", "ask": "look", "budget_wt": 1}),
        ),
        ScriptedModel::tool("g2", "glob", json!({"pattern": "*"})),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru(model);
    run.ask("go", &mut |_| {}).await;
    let results = w.results();
    assert!(
        results[0].starts_with("@S DONE"),
        "both bodies ran past one call: {results:?}"
    );
}

/// An Orc refused a Slime with no verify line codes one with the verify tool, and the commission
/// then goes through.
#[tokio::test]
async fn an_orc_codes_the_verify_line_then_commissions() {
    let w = World::new("verify-tool");
    w.doc(
        "slime",
        "t",
        "# slime-t\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/t/\n",
    );
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-t"),
        ScriptedModel::tool("v1", "verify", json!({"to": "slime-t", "command": "true"})),
        order("c4", "slime-t"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("go", &mut |_| {}).await;
    let seen = format!("{:?}", model.requests());
    assert!(seen.contains("code its verify line first"), "refused first");
    assert!(
        seen.contains("slime-t's doc now reads - **Verify:** `true`"),
        "coded"
    );
    let roster = mnemorph_loop::world::Roster::load(&w.0);
    assert_eq!(roster.creatures["slime-t"].verify, ["true"]);
    assert!(w.results()[0].starts_with("@S DONE"), "{:?}", w.results());
}

/// A commissioned body gets the toolbox too: its first ask lists the Minds it may wear, and `wear`
/// is on its shelf.
#[tokio::test]
async fn a_commissioned_body_is_offered_the_minds() {
    let w = World::new("child-minds");
    fs::create_dir_all(w.0.join(".mnemorph/minds")).unwrap();
    fs::write(
        w.0.join(".mnemorph/minds/dialects.md"),
        "# Dialects\n\n- **Shared:** yes\n\nHow a dialect overrides the generator.\n",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        order("c1", "elf-e"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("Done."),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("look", &mut |_| {}).await;
    let elf = &model.requests()[1];
    let asked: String = elf
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|b| match b {
            mnemorph_provider::Block::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        asked.contains("@T mind:dialects — How a dialect"),
        "{asked}"
    );
    assert!(elf.tools.iter().any(|t| t.name == "wear"));
}

/// A model that answers from what it is shown: once a commission's result names the claim it
/// checked, it accepts that claim by the id the harness minted.
struct Accepting {
    inner: ScriptedModel,
    claim: &'static str,
}

impl mnemorph_loop::Model for Accepting {
    fn provider(&self) -> &'static str {
        "scripted"
    }

    fn complete<'a>(
        &'a self,
        request: &'a mnemorph_provider::Request,
        on: &'a mut (dyn FnMut(mnemorph_provider::Event) + Send),
    ) -> mnemorph_loop::model::Completion<'a> {
        let shown = format!("{:?}", request.messages);
        let marker = format!(" «{}", self.claim);
        if let Some(at) = shown.find(&marker) {
            let id = shown[..at]
                .rsplit([' ', '[', ':', ';'])
                .next()
                .unwrap_or("");
            self.inner.then(ScriptedModel::text(&format!(
                "I read the API's docs myself: it holds.\n@V accept {id}"
            )));
        }
        self.inner.complete(request, on)
    }
}

/// The **accepted** route: a findings-only commission lands nothing, so no test runs and no claim
/// is checked; the commissioner accepts one claim by its id, and that one reaches shared memory —
/// the claim it did not accept stays personal and proposed.
#[tokio::test]
async fn a_findings_commission_s_accepted_claim_is_shared_and_the_rest_stay_personal() {
    use mnemorph_memory::{Status, Tier, fold, read};
    let w = World::new("accepted");
    let model = Arc::new(Accepting {
        inner: ScriptedModel::new([
            ScriptedModel::tool(
                "c1",
                "commission",
                json!({"to": "elf-e", "ask": "how fast may we call the API?", "office": "findings"}),
            ),
            ScriptedModel::text(
                "@S DONE\n@U colony the API caps at 100 calls a minute\n@U colony the cache is warm on mondays\n@E",
            ),
        ]),
        claim: "the API caps",
    });
    let mut run = w.rimuru_dyn(model.clone());
    let ending = run.ask("find the API's rate limit", &mut |_| {}).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    let personal = fold(&read(&w.0, &Tier::Personal("elf-e".into())).unwrap().rows);
    let shared = fold(&read(&w.0, &Tier::Shared).unwrap().rows);
    let by_text = |t: &str| {
        personal
            .values()
            .find(|n| n.text.starts_with(t))
            .unwrap()
            .clone()
    };
    let caps = by_text("the API caps");
    let cache = by_text("the cache is warm");
    assert_eq!(caps.status, Status::Grounded, "accepted");
    let shared_caps = &shared[&caps.id];
    assert_eq!(shared_caps.status, Status::Grounded);
    assert!(
        (shared_caps.confidence - 0.6).abs() < 1e-9,
        "accepted grounds at 0.6"
    );
    assert!(
        shared_caps
            .proof
            .as_deref()
            .unwrap()
            .starts_with(&format!("accepted: rimuru accepted {} in run", caps.id)),
        "{:?}",
        shared_caps.proof
    );
    assert_eq!(cache.status, Status::Proposed, "not accepted: personal");
    assert!(!shared.contains_key(&cache.id), "never shared");
}

/// A body never grounds its own words: a refused order to itself offers nothing, so accepting its
/// own proposed claim grounds nothing.
#[tokio::test]
async fn a_body_cannot_accept_its_own_claim() {
    use mnemorph_memory::{Status, Tier, fold, read};
    let w = World::new("self-accept");
    let own =
        mnemorph_memory::propose(&w.0, "rimuru", "the API caps at 100 a minute", vec![], None)
            .unwrap()
            .note;
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "c1",
            "commission",
            json!({"to": "rimuru", "ask": "check yourself", "office": "findings"}),
        ),
        ScriptedModel::text(&format!("Checked.\n@V accept {own}")),
    ]));
    let mut run = w.rimuru(model.clone());
    run.ask("look", &mut |_| {}).await;
    let mine = fold(&read(&w.0, &Tier::Personal("rimuru".into())).unwrap().rows);
    assert_eq!(
        mine[&own].status,
        Status::Proposed,
        "its own words stay proposed"
    );
    assert!(!fold(&read(&w.0, &Tier::Shared).unwrap().rows).contains_key(&own));
}

/// The offer is a hole, never cut: a report long past the cut still offers its claims, and an
/// accepted one is shared.
#[tokio::test]
async fn a_long_report_still_offers_its_claims() {
    use mnemorph_memory::{Status, Tier, fold, read};
    let w = World::new("long-offer");
    let prose = "The rate limit is documented in several places, read them all. ".repeat(60);
    let model = Arc::new(Accepting {
        inner: ScriptedModel::new([
            ScriptedModel::tool(
                "c1",
                "commission",
                json!({"to": "elf-e", "ask": "how fast may we call the API?", "office": "findings"}),
            ),
            ScriptedModel::text(&format!(
                "@S DONE\n{prose}\n@U colony the API caps at 100 calls a minute\n@E"
            )),
        ]),
        claim: "the API caps",
    });
    let mut run = w.rimuru_dyn(model.clone());
    run.ask("find the API's rate limit", &mut |_| {}).await;
    let personal = fold(&read(&w.0, &Tier::Personal("elf-e".into())).unwrap().rows);
    let caps = personal
        .values()
        .find(|n| n.text.starts_with("the API caps"))
        .unwrap();
    assert_eq!(
        caps.status,
        Status::Grounded,
        "accepted though the report was cut"
    );
}
