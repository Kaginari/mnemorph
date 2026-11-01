//! The seeded fixture end to end, against scripted models: the throne lands a plant, the landing
//! opens a challenge routed to the plant's speciality, the scripted reviewer files its coverage
//! ledger and the plant's finding, a verifier of another speciality re-runs its witness and
//! confirms it, and the landing blocks on the confirmed finding.
//!
//! The paid evaluation over the same fixture, with real models, is `seeded_eval` below: ignored,
//! never run in CI.

#[path = "../../mnemorph-proving/tests/support/seeded.rs"]
mod seeded;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use mnemorph_ledger::Journal;
use mnemorph_loop::model::Completion;
use mnemorph_loop::world::challenge::{Challenges, Status};
use mnemorph_loop::{
    Commissioner, EndGate, Ending, Limits, Model, Run, ScriptedModel, Setup, solo,
};
use mnemorph_provider::{Block, Caller, Event, Request, anthropic_prices};
use mnemorph_races::{Race, Speciality};
use mnemorph_tools::{Context, Sandbox, registry};
use serde_json::{Value, json};

use seeded::{Plant, finding_line, landing, ledger, looked, manifest, tree};

/// A model whose `coverage` calls name the landing `LANDING`: the id is read from the commission
/// the reviewer was given, as a real model would.
struct Reviewing(Arc<dyn Model>);

impl Model for Reviewing {
    fn provider(&self) -> &str {
        self.0.provider()
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        let seen = format!("{request:?}");
        let landing = ["Challenge the landing `", "on the landing `"]
            .iter()
            .find_map(|m| seen.split(m).nth(1))
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

/// A world in a temporary folder, the throne and the Dark Elves `elves` born in it.
struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str, elves: &[&str]) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-seeded-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let w = World { root };
        fs::create_dir_all(w.world()).unwrap();
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
        // The human adopts each card as written: the trusted roster holds it.
        let w = self.world();
        mnemorph_world::creatures::record(&w, mnemorph_world::creatures::adoption(&w)).unwrap();
    }

    fn put(&self, path: &str, bytes: &[u8]) {
        let at = self.world().join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, bytes).unwrap();
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

    /// The throne, scripted by `throne`, sending its landings' challenges to `reviewer`; every
    /// body rides `model_name` and inherits the throne's containment, [`contain`]'s for `live`.
    fn throne(
        &self,
        throne: Arc<dyn Model>,
        reviewer: Arc<dyn Model>,
        model_name: &str,
        reviewers: usize,
        live: bool,
    ) -> Run {
        let world = self.world().canonicalize().unwrap();
        let (body, cx) = solo(&world, model_name);
        let cx = contain(cx, live);
        let setup = Setup {
            model: throne,
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
            .with_challenger(Arc::new(self.commissioner(reviewer, reviewers)))
    }
}

/// The containment a session's commands run under. A live model's is a normal session's: the
/// sandbox detected for the machine, and no command at all when there is none (never
/// unguarded). Scripted models run no command — they only write and file — so theirs runs
/// unguarded, the guard helper not being beside the test binary.
fn contain(cx: Context, live: bool) -> Context {
    if live {
        cx.with_sandbox(Sandbox::detect()).with_unguarded(false)
    } else {
        cx.with_sandbox(Sandbox::None("scripted seeded tests run no command".into()))
            .with_unguarded(true)
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
    bytes: &[u8],
) -> Result<mnemorph_provider::Response, mnemorph_provider::Error> {
    let content = String::from_utf8(bytes.to_vec()).expect("utf-8");
    ScriptedModel::tool(id, "write", json!({"path": path, "content": content}))
}

/// The plants a landing of their file alone routes to exactly their own speciality.
fn single_routed() -> Vec<Plant> {
    manifest()
        .plant
        .into_iter()
        .filter(|p| mnemorph_races::route(&[p.file.as_str()]) == [p.speciality()])
        .collect()
}

/// Every plant whose file routes one hunter, end to end: the throne writes the plant; its
/// speciality's reviewer is commissioned, files a ledger that holds and the plant's finding; a
/// verifier of another speciality, sent the record alone, re-runs the witness (scripted, a run of
/// it that fails) and confirms it; and the landing is blocked on the confirmed finding.
#[tokio::test]
async fn each_plant_blocks_its_landing() {
    let key = manifest();
    let fixture = tree();
    let plants = single_routed();
    // Every plant but one: `env-unredacted`'s `crates/mnemorph-ledger/src/journal.rs` lies on
    // both the secrets and the law surfaces, so it routes two hunters and the generalist.
    assert_eq!(
        plants.len(),
        key.plant.len() - 1,
        "{} plants routed alone",
        plants.len()
    );
    assert!(!plants.iter().any(|p| p.id == "env-unredacted"));
    for p in &plants {
        let s = p.speciality();
        let checker = if s == Speciality::Agent {
            Speciality::Harness
        } else {
            Speciality::Agent
        };
        let w = World::new(&p.id, &[s.creature(), checker.creature()]);
        // The proving ground, as the reviewer leaves it.
        let mut proven = fixture.clone();
        let record = seeded::prove(&mut proven, p);
        for path in [seeded::poc_path(p), record.rec.clone()] {
            w.put(&path, &proven[&path]);
        }
        let written = vec![p.file.clone()];
        let filed = ledger(
            &landing("LANDING", &written),
            s,
            &looked(&key, &fixture, s, &written),
        );
        let report = format!("@S DONE\n{}\n@E", finding_line(p, &record));
        let rerun = format!("sh {}", seeded::poc_path(p));
        let model: Arc<dyn Model> = Arc::new(ScriptedModel::new([
            write("w1", &p.file, &fixture[&p.file]),
            ScriptedModel::text("Wrote it."),
            ScriptedModel::tool(
                "c1",
                "coverage",
                json!({"landing": "LANDING", "entries": filed.entries}),
            ),
            ScriptedModel::text(&report),
            // The verifier: it re-runs the witness, which fails, and confirms.
            ScriptedModel::tool("r1", "bash", json!({"command": rerun})),
            ScriptedModel::tool(
                "v1",
                "verdict",
                json!({"landing": "LANDING", "fingerprint": record.fingerprint,
                       "verdict": "confirmed", "commands": [{"run": rerun}]}),
            ),
            ScriptedModel::text("@S DONE\n@E"),
        ]));
        let model: Arc<dyn Model> = Arc::new(Reviewing(model));
        let mut run = w.throne(model.clone(), model, "claude-sonnet-5-5", 3, false);
        let Ending::Answered(text) = run.ask("land the change", &mut |_| {}).await else {
            panic!("{}: answered", p.id);
        };
        let all = w.records().records();
        assert_eq!(all.len(), 1, "{}: {all:?}", p.id);
        let r = all.into_values().next().unwrap();
        assert_eq!(
            r.landing
                .routed
                .iter()
                .map(|x| x.speciality)
                .collect::<Vec<_>>(),
            [s],
            "{}",
            p.id
        );
        let Status::Blocked { findings } = &r.status else {
            panic!("{}: {:?}\n{text}", p.id, r.status);
        };
        let derived =
            mnemorph_wire::fingerprint(&p.speciality, &p.class, &p.file, &p.symbol).unwrap();
        assert_eq!(findings.len(), 1, "{}", p.id);
        assert_eq!(findings[0].fingerprint, derived, "{}", p.id);
        assert_eq!(findings[0].severity, p.severity.word(), "{}", p.id);
        assert!(
            text.contains(&format!("blocked — #{derived} {}", p.severity.word())),
            "{}: {text}",
            p.id
        );
        // Confirmed by the verifier of another speciality: it counts.
        let f = &r.findings[0];
        assert!(f.counts(), "{}: {f:?}", p.id);
        let verdict = f.verdict.as_ref().expect("a verdict");
        assert_eq!(verdict.speciality, checker.creature(), "{}", p.id);
    }
}

/// A landing of a negative control on its speciality's surface is challenged and cleared by a
/// ledger ruling the control's class out: no finding, nothing blocks.
#[tokio::test]
async fn a_control_landing_clears() {
    let key = manifest();
    let fixture = tree();
    let c = key
        .control
        .iter()
        .find(|c| c.id == "constant-argv")
        .expect("the control");
    let s = c.speciality();
    let w = World::new(&c.id, &[s.creature()]);
    let written = vec![c.file.clone()];
    let filed = ledger(
        &landing("LANDING", &written),
        s,
        &looked(&key, &fixture, s, &written),
    );
    let model: Arc<dyn Model> = Arc::new(Reviewing(Arc::new(ScriptedModel::new([
        write("w1", &c.file, &fixture[&c.file]),
        ScriptedModel::text("Wrote it."),
        ScriptedModel::tool(
            "c1",
            "coverage",
            json!({"landing": "LANDING", "entries": filed.entries}),
        ),
        ScriptedModel::text("@S DONE\n@E"),
    ]))));
    let mut run = w.throne(model.clone(), model, "claude-sonnet-5-5", 3, false);
    let Ending::Answered(text) = run.ask("land the change", &mut |_| {}).await else {
        panic!("answered");
    };
    assert!(text.contains("challenge cleared"), "{text}");
    let all = w.records().records();
    assert_eq!(all.into_values().next().unwrap().status, Status::Cleared);
}

/// The paid evaluation's session is guarded: its live reviewers inherit the throne's context, so
/// it runs commands only under the machine's sandbox, never unguarded on the host.
#[test]
fn the_eval_session_is_guarded() {
    let w = World::new("guarded", &[]);
    let (_, cx) = solo(&w.world().canonicalize().unwrap(), "any:free");
    let cx = contain(cx, true);
    assert!(!cx.allows_unguarded());
    assert_eq!(cx.sandbox(), &Sandbox::detect());
    // What a commissioned reviewer takes from it.
    let (_, child) = solo(&w.world().canonicalize().unwrap(), "any:free");
    let child = child.with_unguarded(true).inherit(&cx);
    assert!(!child.allows_unguarded());
    assert_eq!(child.sandbox(), &Sandbox::detect());
}

/// The paid evaluation: real models hunt the whole seeded world, and each speciality's recall and
/// precision are scored against the key. Never run in CI.
///
/// `MNEMORPH_LIVE=1 OPENROUTER_API_KEY=… cargo nextest run -p mnemorph-loop --test seeded
/// --run-ignored only seeded_eval --no-capture` (or `just seeded-eval`). The model is a free one
/// (`MNEMORPH_LIVE_OR_MODEL` picks another, `:free` by default); the key travels only through the
/// environment and is never printed.
///
/// The throne is scripted: it lands every file of the fixture in one landing, so every hunter is
/// routed with the generalist. Each reviewer is the live model. A finding counts when its record
/// validates on the world; it is a hit when its (class, file, symbol) — the symbol's last segment
/// — is a plant's of that speciality, a false alarm otherwise (on a control, named as such).
#[tokio::test]
#[ignore = "calls OpenRouter: run with MNEMORPH_LIVE=1 and OPENROUTER_API_KEY"]
async fn seeded_eval() {
    use mnemorph_loop::OpenAiModel;
    use mnemorph_provider::{HttpTransport, OpenAiCompatible, Policy};

    if std::env::var("MNEMORPH_LIVE").as_deref() != Ok("1") {
        return;
    }
    let api_key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY");
    let model_name = std::env::var("MNEMORPH_LIVE_OR_MODEL")
        .unwrap_or_else(|_| "nvidia/nemotron-3.5-lightning:free".into());
    let key = manifest();
    let fixture = tree();
    let elves: Vec<&str> = Speciality::ALL.iter().map(|s| s.creature()).collect();
    let w = World::new("eval", &elves);

    let mut script = Vec::new();
    for (n, (path, bytes)) in fixture.iter().enumerate() {
        script.push(write(&format!("w{n}"), path, bytes));
    }
    script.push(ScriptedModel::text("Landed the seeded world."));
    let throne: Arc<dyn Model> = Arc::new(ScriptedModel::new(script));
    let policy = Policy::default();
    let transport = Arc::new(HttpTransport::new(policy.connect).expect("tls"));
    let client = OpenAiCompatible::new(
        transport,
        "https://openrouter.ai/api",
        Some(api_key),
        policy,
    );
    let reviewer: Arc<dyn Model> = Arc::new(OpenAiModel {
        client,
        name: "openrouter".into(),
    });
    let mut run = w.throne(throne, reviewer, &model_name, Speciality::ALL.len(), true);
    let ending = run.ask("land the seeded world", &mut |_| {}).await;
    eprintln!("model: {model_name}\nending: {ending:?}");

    // Every record a reviewer left, validated on the world.
    let world = mnemorph_proving::Tree::new(w.world());
    let mut found: Vec<mnemorph_proving::Valid> = Vec::new();
    for s in Speciality::HUNTERS {
        let dir = w
            .world()
            .join(format!(".mnemorph/dark-elf/{}/findings", s.creature()));
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let rec = format!(
                ".mnemorph/dark-elf/{}/findings/{}",
                s.creature(),
                entry.file_name().to_string_lossy()
            );
            let Some(r) = mnemorph_proving::World::read(&world, &rec)
                .and_then(|b| mnemorph_proving::FindingRecord::parse(&rec, &b))
            else {
                continue;
            };
            match mnemorph_proving::validate(&r, &world) {
                Ok(v) => found.push(v),
                Err(rules) => eprintln!("  refused {rec}: {rules:?}"),
            }
        }
    }
    score(&key, &found);
}

/// Each speciality's hits, false alarms, recall and precision, printed.
fn score(key: &seeded::Manifest, found: &[mnemorph_proving::Valid]) {
    let last = |sym: &str| {
        sym.rsplit([':', '.'])
            .next()
            .unwrap_or(sym)
            .trim()
            .to_owned()
    };
    eprintln!(
        "{:<18} {:>6} {:>6} {:>7} {:>9}",
        "speciality", "plants", "hits", "alarms", "controls"
    );
    for s in Speciality::HUNTERS {
        let plants: Vec<&Plant> = key.plant.iter().filter(|p| p.speciality() == s).collect();
        let mine: Vec<&mnemorph_proving::Valid> = found
            .iter()
            .filter(|v| v.record().speciality == s.creature())
            .collect();
        let hit = |p: &Plant| {
            mine.iter().any(|v| {
                let r = v.record();
                r.class == p.class && r.file == p.file && last(&r.symbol) == p.symbol
            })
        };
        let hits = plants.iter().filter(|p| hit(p)).count();
        let alarms = mine
            .iter()
            .filter(|v| {
                let r = v.record();
                !plants
                    .iter()
                    .any(|p| r.class == p.class && r.file == p.file)
            })
            .count();
        let on_controls = mine
            .iter()
            .filter(|v| key.control.iter().any(|c| c.file == v.record().file))
            .count();
        let recall = ratio(hits, plants.len());
        let precision = if mine.is_empty() {
            1.0
        } else {
            ratio(mine.len() - alarms, mine.len())
        };
        eprintln!(
            "{:<18} {:>6} {:>6} {:>7} {:>9}   recall {recall:.2} precision {precision:.2}",
            s.creature(),
            plants.len(),
            hits,
            alarms,
            on_controls
        );
    }
}

/// `a / b`, for a score.
fn ratio(a: usize, b: usize) -> f64 {
    let f = |n: usize| u32::try_from(n).map_or(f64::from(u32::MAX), f64::from);
    if b == 0 { 0.0 } else { f(a) / f(b) }
}
