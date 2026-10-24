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
        let cx = Context::new(Race::Rimuru, &self.0, &["."])
            .with_sandbox(Sandbox::None("commission tests run unguarded".into()))
            .with_unguarded(true)
            .with_attic(
                mnemorph_tools::Attic::at(&self.attic(), &self.0).expect("git is installed"),
            );
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
        ScriptedModel::text("@S DONE\n@W src/done.txt\n@E"),
        ScriptedModel::text("@S DONE\n@V the landing holds\n@E"),
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
        ScriptedModel::text("I could not find anything to do."),
        ScriptedModel::text("ok"),
    ]));
    let mut run = w.rimuru(model);
    run.ask("go", &mut |_| {}).await;
    assert!(
        w.results()[0].starts_with("@S FAIL off the wire"),
        "{:?}",
        w.results()
    );
}
