//! Challenge routing at the gate, against scripted models: a landing on a Dark Elf surface opens
//! a challenge, the throne's harness commissions the routed specialities, and the landing waits
//! until each has filed a valid coverage ledger and nothing blocks.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_ledger::Journal;
use mnemorph_loop::model::Completion;
use mnemorph_loop::world::challenge::{Challenges, Staff, Status};
use mnemorph_loop::{
    Body, CommissionTool, Commissioner, EndGate, Ending, Limits, Model, Run, ScriptedModel, Setup,
    solo,
};
use mnemorph_provider::{Block, Caller, Event, Request, anthropic_prices};
use mnemorph_races::{Race, Speciality};
use mnemorph_tools::{Context, Sandbox, Tool, registry};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

/// The harness file every landing here writes.
const FILE: &str = "crates/mnemorph-tools/src/files.rs";
const BODY: &str = "pub fn read_path(p: &str) -> String { std::fs::read_to_string(p).unwrap() }\n";

fn sha(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// A scripted model whose `coverage` calls name the landing `LANDING`: the id is read from the
/// commission the reviewer was given, as a real model would.
struct Reviewing(ScriptedModel);

impl Model for Reviewing {
    fn provider(&self) -> &'static str {
        "scripted"
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        let seen = format!("{request:?}");
        let landing = seen
            .split("Challenge the landing `")
            .nth(1)
            .and_then(|rest| rest.split('`').next())
            .map(str::to_owned);
        let inner = self.0.complete(request, on);
        Box::pin(async move {
            let mut reply = inner.await?;
            for b in &mut reply.content {
                if let Block::ToolUse { input, .. } = b
                    && input["landing"] == "LANDING"
                    && let Some(id) = &landing
                {
                    input["landing"] = Value::String(id.clone());
                }
            }
            Ok(reply)
        })
    }
}

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str, elves: &[&str]) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-challenge-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let w = World { root };
        fs::create_dir_all(w.world().join("crates/mnemorph-tools/src")).unwrap();
        fs::create_dir_all(w.world().join("src")).unwrap();
        fs::write(w.world().join("src/main.rs"), "fn main() {}\n").unwrap();
        w.doc("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n");
        for e in elves {
            w.doc(
                "dark-elf",
                e,
                &format!("# {e}\n\n- **Race:** dark-elf\n- **Reports to:** rimuru\n"),
            );
        }
        w
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn doc(&self, race: &str, seat: &str, text: &str) {
        let dir = self.world().join(".mnemorph").join(race).join(seat);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), text).unwrap();
    }

    fn records(&self) -> Challenges {
        Challenges::at(&self.root.join("state/challenges.jsonl"))
    }

    fn commissioner(&self, model: Arc<dyn Model>, reviewers: usize) -> Commissioner {
        Commissioner {
            registry: None,
            law: mnemorph_loop::Law::On,
            guts: None,
            world: self.world().canonicalize().unwrap(),
            model,
            prices: anthropic_prices().unwrap(),
            ledger: None,
            journals: self.world().join(".mnemorph/instruments/loop"),
            session: "s".into(),
            caps: None,
            stress_tokens: None,
            verify_secs: None,
            weights: None,
            steps: mnemorph_loop::commission::RaceSteps::default(),
            busy: std::sync::Mutex::default(),
            observer: None,
            council: mnemorph_loop::council::Thresholds::default(),
            router: None,
            slice_floor: 0,
            queue: None,
            challenges: Some(self.records()),
            reviewers,
            mcp: None,
        }
    }

    /// The throne alone, under its end gate, sending the Dark Elves the challenges its landings
    /// route.
    fn throne(&self, model: Arc<dyn Model>, reviewers: usize) -> Run {
        let world = self.world().canonicalize().unwrap();
        let (body, cx) = solo(&world, "claude-sonnet-5-5");
        let cx = cx
            .with_sandbox(Sandbox::None("challenge tests run unguarded".into()))
            .with_unguarded(true);
        let setup = Setup {
            model: model.clone(),
            tools: registry(),
            cx,
            journal: Journal::new(self.root.join("rimuru.jsonl")),
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
        Run::new(body, Limits::for_race(Race::Rimuru), setup)
            .with_gate(EndGate::default())
            .with_challenger(Arc::new(self.commissioner(model, reviewers)))
    }

    /// The raw rows of the challenge records.
    fn rows(&self) -> Vec<Value> {
        fs::read_to_string(self.root.join("state/challenges.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    /// The one challenge kept.
    fn only(&self) -> mnemorph_loop::world::challenge::Record {
        let all = self.records().records();
        assert_eq!(all.len(), 1, "{all:?}");
        all.into_values().next().unwrap()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write(
    id: &str,
    path: &str,
    content: &str,
) -> Result<mnemorph_provider::Response, mnemorph_provider::Error> {
    ScriptedModel::tool(id, "write", json!({"path": path, "content": content}))
}

/// A harness ledger that holds for a landing that wrote [`FILE`] as [`BODY`]: sandbox escape
/// checked on the file, the other high-risk classes skipped each with its risk, the rest ruled
/// out each for its own reason.
fn harness_ledger() -> Value {
    let high = mnemorph_proving::coverage::HIGH_RISK;
    let entries: Vec<Value> = Speciality::Harness
        .classes()
        .iter()
        .enumerate()
        .map(|(n, c)| {
            let state = if c.name() == "sandbox-escape" {
                json!({"checked": {
                    "method": "read every path the file opens and traced it to the sandbox grant",
                    "evidence": {"files": [{"path": FILE, "symbols": ["read_path"], "sha256": sha(BODY.as_bytes())}]}
                }})
            } else if high.contains(c) {
                json!({"skipped": {
                    "reason": format!("fact {n}: the landing adds one reader and no process"),
                    "risk": format!("risk {n}: a later caller could pass an untrusted path"),
                }})
            } else {
                json!({"not_applicable": {
                    "reason": format!("fact {n}: nothing here renders or races on the file"),
                }})
            };
            json!({"surface": "crates/mnemorph-tools/**", "class": c.name(), "state": state})
        })
        .collect();
    json!({"landing": "LANDING", "entries": entries})
}

/// A landing on the harness surface opens a challenge routed to the harness speciality; the
/// reviewer is commissioned by the throne and answers without its ledger, so the landing waits —
/// journaled `PASS (challenge pending)` — until a valid ledger is filed; then it clears.
#[tokio::test]
async fn a_harness_landing_waits_on_its_ledger_then_clears() {
    let w = World::new("pending", &["dark-elf-harness"]);
    let model = Arc::new(Reviewing(ScriptedModel::new([
        write("w1", FILE, BODY),
        ScriptedModel::text("Wrote the reader."),
        // The reviewer, commissioned by the harness.
        ScriptedModel::text("@S DONE\n@E"),
    ])));
    let mut run = w.throne(model, 3);
    let Ending::Answered(text) = run.ask("add a reader", &mut |_| {}).await else {
        panic!("answered");
    };
    assert!(
        text.contains("challenge pending — awaiting dark-elf-harness"),
        "{text}"
    );
    let r = w.only();
    assert_eq!(
        r.landing
            .routed
            .iter()
            .map(|x| x.speciality)
            .collect::<Vec<_>>(),
        [Speciality::Harness]
    );
    assert_eq!(r.opening.by, "rimuru");
    assert_eq!(r.opening.tree.len(), 64);
    assert!(matches!(
        &r.staff[..],
        [(Speciality::Harness, Staff::Commissioned { body, by, .. })]
            if body == "dark-elf-harness" && by == "rimuru"
    ));
    assert!(r.status.open());
    let log = fs::read_to_string(w.world().join(".mnemorph/log.md")).unwrap();
    assert!(log.contains("gate: PASS (challenge pending)"), "{log}");
    // A pass still waiting on its challenge proves nothing checked.
    assert!(run.passed_gate().is_none());
    // The ledger, filed now, clears it.
    let mut filed = harness_ledger();
    filed["landing"] = json!(r.landing.id);
    filed["speciality"] = json!("dark-elf-harness");
    let ledger: mnemorph_proving::coverage::CoverageLedger = serde_json::from_value(filed).unwrap();
    let status = w.records().file(
        ledger,
        &mnemorph_proving::Tree::new(w.world()),
        &mnemorph_proving::coverage::Ran::default(),
    );
    assert_eq!(status, Ok(Status::Cleared));
}

/// The reviewer files its ledger through the `coverage` tool in its own run: the challenge clears
/// in the same ask, and the throne says so.
#[tokio::test]
async fn a_reviewer_that_files_clears_the_challenge() {
    let w = World::new("files", &["dark-elf-harness"]);
    let model = Arc::new(Reviewing(ScriptedModel::new([
        write("w1", FILE, BODY),
        ScriptedModel::text("Wrote the reader."),
        ScriptedModel::tool("c1", "coverage", harness_ledger()),
        ScriptedModel::text("@S DONE\n@E"),
    ])));
    let mut run = w.throne(model, 3);
    let Ending::Answered(text) = run.ask("add a reader", &mut |_| {}).await else {
        panic!("answered");
    };
    assert!(text.contains("challenge cleared"), "{text}");
    assert!(!text.contains("does not count as landed"), "{text}");
    assert_eq!(w.only().status, Status::Cleared);
}

/// Two hunters' surfaces add the generalist; a cap of two seats the riskiest two and queues the
/// generalist, never dropping it — the next sweep commissions it.
#[tokio::test]
async fn cross_surface_adds_the_generalist_and_the_cap_queues_it() {
    let w = World::new("cap", &["dark-elf-harness", "dark-elf-supply", "dark-elf"]);
    let model = Arc::new(Reviewing(ScriptedModel::new([
        write("w1", FILE, BODY),
        write("w2", "Cargo.lock", "# lock\n"),
        ScriptedModel::text("Wrote the reader and the lock."),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        // The next ask: nothing to land; the sweep seats the queued generalist.
        ScriptedModel::text("Nothing more."),
        ScriptedModel::text("@S DONE\n@E"),
    ])));
    let mut run = w.throne(model, 2);
    let Ending::Answered(text) = run.ask("add a reader", &mut |_| {}).await else {
        panic!("answered");
    };
    let r = w.only();
    assert_eq!(
        r.landing
            .routed
            .iter()
            .map(|x| x.speciality)
            .collect::<Vec<_>>(),
        [Speciality::Harness, Speciality::Supply, Speciality::General]
    );
    let staffed: Vec<(Speciality, bool)> = r
        .staff
        .iter()
        .map(|(s, st)| (*s, matches!(st, Staff::Commissioned { .. })))
        .collect();
    assert_eq!(
        staffed,
        [
            (Speciality::Harness, true),
            (Speciality::Supply, true),
            (Speciality::General, false)
        ]
    );
    assert!(text.contains("queued: dark-elf"), "{text}");
    let _ = run.ask("anything else?", &mut |_| {}).await;
    let r = w.only();
    assert!(matches!(
        r.staff.last(),
        Some((Speciality::General, Staff::Commissioned { body, .. })) if body == "dark-elf"
    ));
}

/// A routed speciality with no body born is unstaffed: said to the human, and the challenge
/// stays pending — never cleared in silence.
#[tokio::test]
async fn an_unstaffed_speciality_is_said_to_the_human() {
    let w = World::new("unstaffed", &[]);
    let model = Arc::new(Reviewing(ScriptedModel::new([
        write("w1", FILE, BODY),
        ScriptedModel::text("Wrote the reader."),
    ])));
    let mut run = w.throne(model, 3);
    let Ending::Answered(text) = run.ask("add a reader", &mut |_| {}).await else {
        panic!("answered");
    };
    assert!(
        text.contains("unstaffed (no body born): dark-elf-harness"),
        "{text}"
    );
    assert!(text.contains("challenge waive"), "{text}");
    let r = w.only();
    assert_eq!(r.staff, [(Speciality::Harness, Staff::Unstaffed)]);
    assert!(matches!(r.status, Status::Pending { .. }));
}

/// A valid high finding of the reviewer's own speciality is kept for its verifier: unverified,
/// it does not block, and with no other speciality born it waits, said to the human.
#[tokio::test]
async fn a_high_finding_waits_on_its_verifier() {
    use mnemorph_proving::{
        FindingRecord, Impact, Likelihood, Severity, Step, StepKind, Verdict, Witness, WitnessKind,
    };
    let w = World::new("finding", &["dark-elf-harness"]);
    let poc = ".mnemorph/dark-elf/dark-elf-harness/poc/read_path.rs";
    let poc_body = "#[test]\nfn escapes() { assert!(read_path(\"../x\").is_err()); }\n";
    let fp = mnemorph_wire::fingerprint("dark-elf-harness", "path-traversal", FILE, "read_path")
        .unwrap();
    let rec = mnemorph_proving::record_path("dark-elf-harness", &fp);
    let record = FindingRecord {
        verdict: Verdict::Finding,
        fingerprint: fp.clone(),
        speciality: "dark-elf-harness".into(),
        class: "path-traversal".into(),
        file: FILE.into(),
        symbol: "read_path".into(),
        claim: "read_path follows `..` out of the world".into(),
        trace: vec![
            Step {
                kind: StepKind::Entry,
                at: "src/main.rs:1".into(),
                note: "a path from a repository file".into(),
            },
            Step {
                kind: StepKind::Sink,
                at: format!("{FILE}:1"),
                note: "read without a check".into(),
            },
        ],
        witness: Some(Witness {
            kind: WitnessKind::Test,
            reference: poc.into(),
            sha256: sha(poc_body.as_bytes()),
            output: None,
            output_sha256: None,
        }),
        preconditions: vec!["the body reads an untrusted repository".into()],
        reachability: "a repository file names the path the tool reads".into(),
        likelihood: Some(Likelihood::Medium),
        likelihood_reason: "worlds are cloned from strangers".into(),
        impact: Some(Impact::Disclosure),
        impact_reason: "the test reads a file outside the world".into(),
        severity: Some(Severity::High),
        fix: "refuse `..` before the read".into(),
        invariant: "no read leaves the world".into(),
        rec: rec.clone(),
    };
    // The proving ground, as the reviewer leaves it.
    for (path, text) in [(poc, poc_body.to_owned()), (rec.as_str(), record.to_json())] {
        let at = w.world().join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, text).unwrap();
    }
    let report = format!(
        "@S DONE\n@F #{fp} {FILE}:1 read_path follows `..` out of the world witness:{poc} rec:{rec}\n@E"
    );
    let model = Arc::new(Reviewing(ScriptedModel::new([
        write("w1", FILE, BODY),
        ScriptedModel::text("Wrote the reader."),
        ScriptedModel::tool("c1", "coverage", harness_ledger()),
        ScriptedModel::text(&report),
    ])));
    let mut run = w.throne(model, 3);
    let Ending::Answered(text) = run.ask("add a reader", &mut |_| {}).await else {
        panic!("answered");
    };
    assert!(text.contains(&format!("#{fp} high unverified")), "{text}");
    let r = w.only();
    assert!(matches!(r.status, Status::Verifying { ref findings } if *findings == [fp.clone()]));
    assert_eq!(r.findings[0].fingerprint(), fp);
}

/// The chain: an Orc's Slime lands on the harness surface. The challenge is sent by the throne
/// alone — the Orc whose domain it is, and the Elf above it, never commission its reviewer.
#[tokio::test]
async fn the_reviewed_orc_never_sends_its_reviewer() {
    let w = World::new("orc", &["dark-elf-harness"]);
    w.doc(
        "elf",
        "e",
        "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n- **Territory:** crates/\n",
    );
    fs::write(
        w.world().join(".mnemorph/elf/e/plan.md"),
        "- [ ] orc-o: the reader\n",
    )
    .unwrap();
    w.doc(
        "orc",
        "o",
        "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** crates/\n",
    );
    w.doc(
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** crates/mnemorph-tools/\n- **Verify:** `test -f crates/mnemorph-tools/src/files.rs`\n",
    );
    pin(
        &w.world(),
        "slime-s",
        &["test -f crates/mnemorph-tools/src/files.rs"],
    );
    let order = |id: &str, to: &str| {
        ScriptedModel::tool(id, "commission", json!({"to": to, "ask": "add the reader"}))
    };
    let model: Arc<dyn Model> = Arc::new(Reviewing(ScriptedModel::new([
        order("c1", "elf-e"),
        order("c2", "orc-o"),
        order("c3", "slime-s"),
        write("w1", FILE, BODY),
        ScriptedModel::text(&format!("@S DONE\n@W {FILE}\n@E")),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("Done: the reader is in."),
        // The reviewer, sent by the throne.
        ScriptedModel::text("@S DONE\n@E"),
    ])));
    let world = w.world().canonicalize().unwrap();
    let cx = Context::new(Race::Rimuru, &world, &["."])
        .with_sandbox(Sandbox::None("challenge tests run unguarded".into()))
        .with_unguarded(true);
    let mut tools = registry();
    tools.push(Box::new(CommissionTool) as Box<dyn Tool>);
    let setup = Setup {
        model: model.clone(),
        tools,
        cx,
        journal: Journal::new(w.root.join("rimuru.jsonl")),
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
    let mut run = Run::new(body, Limits::for_race(Race::Rimuru), setup)
        .with_commissioner(Arc::new(w.commissioner(model, 3)));
    let ending = run.ask("add the reader", &mut |_| {}).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    let r = w.only();
    assert_eq!(r.opening.by, "slime-s");
    assert_eq!(r.opening.chain, ["orc-o", "elf-e", "rimuru"]);
    // The Orc read its Slime's landing as waiting on the challenge, not as landed.
    let orc = fs::read_dir(w.world().join(".mnemorph/instruments/loop"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orc-o")
        })
        .unwrap();
    let read = fs::read_to_string(orc).unwrap();
    assert!(
        read.contains("waits on its Dark Elf challenge (dark-elf-harness): written, not landed"),
        "{read}"
    );
    // Every staffing row the records hold names the throne as the sender: none the Orc's, none
    // the Elf's, none queued by a refused send from below.
    let rows = w.rows();
    let staffed: Vec<&Value> = rows.iter().filter(|row| row["row"] == "staffed").collect();
    assert_eq!(staffed.len(), 1, "{staffed:?}");
    assert_eq!(staffed[0]["staff"]["staff"], "commissioned");
    assert_eq!(staffed[0]["staff"]["by"], "rimuru");
    assert_eq!(staffed[0]["staff"]["body"], "dark-elf-harness");
}

/// The landing's tree hash binds the challenge to what landed: the same paths with other bytes
/// hash otherwise.
#[test]
fn the_tree_hash_is_the_landing_s_bytes() {
    let w = World::new("tree", &[]);
    let p = |s: &str| vec![s.to_owned()];
    fs::write(w.world().join(FILE), BODY).unwrap();
    let a = mnemorph_loop::challenge::tree_hash(&w.world(), &p(FILE));
    fs::write(w.world().join(FILE), "other").unwrap();
    assert_ne!(a, mnemorph_loop::challenge::tree_hash(&w.world(), &p(FILE)));
    assert!(Path::new(&w.world()).exists());
}

/// A routed landing whose challenge cannot be recorded fails closed: journaled
/// `PASS (challenge unrecorded)`, crediting nothing, said as written and not landed.
#[tokio::test]
async fn an_unrecorded_challenge_credits_nothing() {
    let w = World::new("unrecorded", &["dark-elf-harness"]);
    // The records' folder is a file: nothing can be kept there.
    fs::write(w.root.join("state"), "not a folder").unwrap();
    let model = Arc::new(Reviewing(ScriptedModel::new([
        write("w1", FILE, BODY),
        ScriptedModel::text("Wrote the reader."),
    ])));
    let mut run = w.throne(model, 3);
    let ending = run.ask("add a reader", &mut |_| {}).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    let log = fs::read_to_string(w.world().join(".mnemorph/log.md")).unwrap();
    assert!(log.contains("gate: PASS (challenge unrecorded)"), "{log}");
    assert!(run.passed_gate().is_none());
    assert!(
        run.challenged()[0].contains("could not be recorded"),
        "{:?}",
        run.challenged()
    );
}

/// A reviewer that never files is not forgotten: its challenge is said at every answer, and
/// after [`RESEND_AFTER`] sweeps it is sent again.
#[tokio::test]
async fn an_unfiled_review_is_said_and_sent_again() {
    let w = World::new("resend", &["dark-elf-harness"]);
    let model = Arc::new(Reviewing(ScriptedModel::new([
        write("w1", FILE, BODY),
        ScriptedModel::text("Wrote the reader."),
        ScriptedModel::text("@S DONE\n@E"),
        ScriptedModel::text("One."),
        ScriptedModel::text("Two."),
        ScriptedModel::text("Three."),
        // Sent again.
        ScriptedModel::text("@S DONE\n@E"),
    ])));
    let mut run = w.throne(model, 3);
    for ask in ["add a reader", "one", "two", "three"] {
        let Ending::Answered(text) = run.ask(ask, &mut |_| {}).await else {
            panic!("answered");
        };
        assert!(
            text.contains("challenge pending — awaiting dark-elf-harness"),
            "{ask}: {text}"
        );
    }
    let sends = w
        .rows()
        .iter()
        .filter(|r| r["row"] == "staffed" && r["staff"]["staff"] == "commissioned")
        .filter(|r| r["staff"].get("waited").is_none())
        .count();
    assert_eq!(sends, 2, "sent, then sent again");
}

/// `creature`'s verify lines pinned in `world`, as the human's `mnemorphd verify pin` would pin
/// them: the end gate runs only pinned lines.
fn pin(world: &std::path::Path, creature: &str, lines: &[&str]) {
    use mnemorph_world::pins;
    let lines: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
    pins::record(
        world,
        pins::row(pins::Act::Pin, creature, &lines, pins::HUMAN),
    )
    .unwrap();
}
