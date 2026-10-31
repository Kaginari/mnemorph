//! Independent verification, against scripted models: a reviewer's valid finding of medium or
//! higher is sent by the throne to a fresh verifier of another speciality, given the record
//! alone; its verdict, through the `verdict` tool, decides whether the finding counts, blocks or
//! is proposed as a seal — which is never applied.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mnemorph_ledger::Journal;
use mnemorph_loop::model::Completion;
use mnemorph_loop::world::challenge::{Challenges, Record, Ruling, Staff, Status};
use mnemorph_loop::{EndGate, Ending, Limits, Model, Run, ScriptedModel, Setup, solo};
use mnemorph_provider::{Block, Caller, Event, Request, anthropic_prices};
use mnemorph_proving::{
    FindingRecord, Impact, Likelihood, Severity, Step, StepKind, Verdict, Witness, WitnessKind,
};
use mnemorph_races::Race;
use mnemorph_tools::{Sandbox, registry};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const FILE: &str = "crates/mnemorph-tools/src/files.rs";
const BODY: &str = "pub fn read_path(p: &str) -> String { std::fs::read_to_string(p).unwrap() }\n";
const POC: &str = ".mnemorph/dark-elf/dark-elf-harness/poc/read_path.sh";
/// The proof-of-concept test: it fails, for the stated reason.
const POC_BODY: &str = "echo 'read_path read ../x: it left the world'\nexit 1\n";
/// A command witness and the output it captured.
const LEAK: &str = "sh .mnemorph/dark-elf/dark-elf-harness/poc/leak.sh";
const LEAK_SH: &str = ".mnemorph/dark-elf/dark-elf-harness/poc/leak.sh";
const LEAK_OUT: &str = ".mnemorph/dark-elf/dark-elf-harness/poc/leak.out";
const LEAKED: &str = "read ../x\n";
/// What only the finder's own run holds: its reasoning, never its verifier's.
const SECRET: &str = "FINDER-SECRET-REASONING";
const IMPACT_REASON: &str = "the test reads a file outside the world";
const FIX: &str = "refuse dot-dot before the read";
const NOTE: &str = "read without a check";
const RERUN: &str = "sh .mnemorph/dark-elf/dark-elf-harness/poc/read_path.sh";

fn sha(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// A scripted model that keeps every request it is sent, and fills a tool call's `LANDING` with
/// the landing its commission names, as a real model would read it.
struct Recording {
    inner: ScriptedModel,
    seen: Mutex<Vec<String>>,
}

impl Model for Recording {
    fn provider(&self) -> &'static str {
        "scripted"
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
        self.seen.lock().unwrap().push(seen);
        let inner = self.inner.complete(request, on);
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
        let root = std::env::temp_dir().join(format!(
            "mnemorph-verification-{name}-{}",
            std::process::id()
        ));
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

    fn throne(&self, model: Arc<dyn Model>) -> Run {
        let world = self.world().canonicalize().unwrap();
        let (body, cx) = solo(&world, "claude-sonnet-5-5");
        let cx = cx
            .with_sandbox(Sandbox::None("verification tests run unguarded".into()))
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
        let commissioner = mnemorph_loop::Commissioner {
            registry: None,
            law: mnemorph_loop::Law::On,
            world: world.clone(),
            model,
            prices: anthropic_prices().unwrap(),
            ledger: None,
            journals: world.join(".mnemorph/instruments/loop"),
            session: "s".into(),
            caps: None,
            stress_tokens: None,
            verify_secs: None,
            weights: None,
            busy: std::sync::Mutex::default(),
            observer: None,
            council: mnemorph_loop::council::Thresholds::default(),
            router: None,
            slice_floor: 0,
            queue: None,
            challenges: Some(self.records()),
            reviewers: 3,
            mcp: None,
        };
        Run::new(body, Limits::for_race(Race::Rimuru), setup)
            .with_gate(EndGate::default())
            .with_challenger(Arc::new(commissioner))
    }

    fn only(&self) -> Record {
        let all = self.records().records();
        assert_eq!(all.len(), 1, "{all:?}");
        all.into_values().next().unwrap()
    }

    /// The finder's proving ground: its proof-of-concept and its record at `severity`; the
    /// finding's fingerprint, and the finder's report naming it.
    fn finding(&self, severity: Severity) -> (String, String) {
        self.finding_by(severity, false)
    }

    /// As [`World::finding`]; with `command`, the witness is [`LEAK`] and its captured output.
    fn finding_by(&self, severity: Severity, command: bool) -> (String, String) {
        let fp =
            mnemorph_wire::fingerprint("dark-elf-harness", "path-traversal", FILE, "read_path")
                .unwrap();
        let rec = mnemorph_proving::record_path("dark-elf-harness", &fp);
        let record = FindingRecord {
            verdict: Verdict::Finding,
            fingerprint: fp.clone(),
            speciality: "dark-elf-harness".into(),
            class: "path-traversal".into(),
            file: FILE.into(),
            symbol: "read_path".into(),
            claim: "read_path follows dot-dot out of the world".into(),
            trace: vec![
                Step {
                    kind: StepKind::Entry,
                    at: "src/main.rs:1".into(),
                    note: "a path from a repository file".into(),
                },
                Step {
                    kind: StepKind::Sink,
                    at: format!("{FILE}:1"),
                    note: NOTE.into(),
                },
            ],
            witness: Some(if command {
                Witness {
                    kind: WitnessKind::PocCommand,
                    reference: LEAK.into(),
                    sha256: sha(LEAK.as_bytes()),
                    output: Some(LEAK_OUT.into()),
                    output_sha256: Some(sha(LEAKED.as_bytes())),
                }
            } else {
                Witness {
                    kind: WitnessKind::Test,
                    reference: POC.into(),
                    sha256: sha(POC_BODY.as_bytes()),
                    output: None,
                    output_sha256: None,
                }
            }),
            preconditions: vec!["the body reads an untrusted repository".into()],
            reachability: "a repository file names the path the tool reads".into(),
            likelihood: Some(Likelihood::Medium),
            likelihood_reason: "worlds are cloned from strangers".into(),
            impact: Some(if severity == Severity::Critical {
                Impact::Bypass
            } else {
                Impact::Disclosure
            }),
            impact_reason: IMPACT_REASON.into(),
            severity: Some(severity),
            fix: FIX.into(),
            invariant: "no read leaves the world".into(),
            rec: rec.clone(),
        };
        let files = [
            (POC, POC_BODY.to_owned()),
            (LEAK_SH, "printf 'read ../x\\n'\n".to_owned()),
            (LEAK_OUT, LEAKED.to_owned()),
            (rec.as_str(), record.to_json()),
        ];
        for (path, text) in files {
            let at = self.world().join(path);
            fs::create_dir_all(at.parent().unwrap()).unwrap();
            fs::write(at, text).unwrap();
        }
        let report = format!(
            "@S DONE\n@F #{fp} {FILE}:1 read_path follows dot-dot out of the world witness:{} rec:{rec}\n@E",
            if command { LEAK_OUT } else { POC }
        );
        (fp, report)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write(
    path: &str,
    content: &str,
) -> Result<mnemorph_provider::Response, mnemorph_provider::Error> {
    ScriptedModel::tool("w1", "write", json!({"path": path, "content": content}))
}

/// A harness ledger that holds for a landing that wrote [`FILE`] as [`BODY`].
fn ledger() -> Result<mnemorph_provider::Response, mnemorph_provider::Error> {
    let high = mnemorph_proving::coverage::HIGH_RISK;
    let entries: Vec<Value> = mnemorph_races::Speciality::Harness
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
    ScriptedModel::tool(
        "c1",
        "coverage",
        json!({"landing": "LANDING", "entries": entries}),
    )
}

/// The throne's landing and its harness reviewer, which thinks aloud, files its ledger and
/// reports `report`; then `verifier`'s turns.
fn script(
    report: &str,
    verifier: Vec<Result<mnemorph_provider::Response, mnemorph_provider::Error>>,
) -> Arc<Recording> {
    let mut replies = vec![
        write(FILE, BODY),
        ScriptedModel::text("Wrote the reader."),
        ScriptedModel::tool("b0", "bash", json!({"command": format!("echo {SECRET}")})),
        ledger(),
        ScriptedModel::text(report),
    ];
    replies.extend(verifier);
    Arc::new(Recording {
        inner: ScriptedModel::new(replies),
        seen: Mutex::new(Vec::new()),
    })
}

fn verdict(
    fp: &str,
    rest: &Value,
) -> Result<mnemorph_provider::Response, mnemorph_provider::Error> {
    let mut input = json!({"landing": "LANDING", "fingerprint": fp});
    for (k, v) in rest.as_object().unwrap() {
        input[k] = v.clone();
    }
    ScriptedModel::tool("v1", "verdict", input)
}

fn rerun() -> Result<mnemorph_provider::Response, mnemorph_provider::Error> {
    ScriptedModel::tool("r1", "bash", json!({"command": RERUN}))
}

async fn answer(run: &mut Run, ask: &str) -> String {
    match run.ask(ask, &mut |_| {}).await {
        Ending::Answered(text) => text,
        other => panic!("answered: {other:?}"),
    }
}

/// A confirmed high finding counts and blocks; the verifier is another speciality's, sent by the
/// throne, given the record alone — never the finder's transcript, reasoning or fix; the seal is
/// proposed with its witness frozen, and never applied: a later landing on the same file passes.
#[tokio::test]
async fn a_confirmed_finding_counts_blocks_and_proposes_a_seal() {
    let w = World::new("confirmed", &["dark-elf-harness", "dark-elf-agent"]);
    let (fp, report) = w.finding(Severity::High);
    let model = script(
        &report,
        vec![
            rerun(),
            verdict(
                &fp,
                &json!({"verdict": "confirmed", "commands": [{"run": RERUN}]}),
            ),
            ScriptedModel::text("@S DONE\n@E"),
            // A second landing on the sealed-to-be file, and its reviewer.
            write(FILE, "pub fn read_path() {}\n"),
            ScriptedModel::text("Changed the reader."),
            ScriptedModel::text("@S DONE\n@E"),
        ],
    );
    let mut run = w.throne(model.clone());
    let text = answer(&mut run, "add a reader").await;
    assert!(text.contains(&format!("blocked — #{fp} high")), "{text}");
    assert!(text.contains("confirmed by dark-elf-agent"), "{text}");
    assert!(
        text.contains("seal proposed to the human, not applied"),
        "{text}"
    );
    let r = w.only();
    let f = &r.findings[0];
    assert!(f.counts() && f.blocks());
    assert!(matches!(
        &f.staff,
        Some(Staff::Commissioned { body, by, .. }) if body == "dark-elf-agent" && by == "rimuru"
    ));
    let v = f.verdict.as_ref().unwrap();
    assert_eq!(v.speciality, "dark-elf-agent");
    assert_eq!(v.finder_speciality, "dark-elf-harness");
    let Ruling::Confirmed {
        severity, reran, ..
    } = &v.ruling
    else {
        panic!("confirmed");
    };
    assert_eq!(*severity, Severity::High);
    // The re-run's output and its failure, stamped from the verifier's journal.
    assert_eq!(reran[0].sha256.len(), 64);
    assert!(reran[0].failed, "the witness test failed again");
    assert_eq!(
        v.tree, r.opening.tree,
        "the verdict is bound to the opened tree"
    );
    let seal = f.proposal.as_ref().unwrap();
    assert_eq!(
        seal.witness.sha256,
        sha(POC_BODY.as_bytes()),
        "frozen by hash"
    );
    assert_eq!(seal.by, "rimuru");
    let rows = w.records().outcomes();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].confirmed && rows[0].finder_speciality == "dark-elf-harness");
    // The verifier saw the record, and nothing of the finder's run.
    let seen = model.seen.lock().unwrap().clone();
    let asked: Vec<&String> = seen
        .iter()
        .filter(|s| s.contains("Verify the finding #"))
        .collect();
    assert!(!asked.is_empty(), "the verifier was asked");
    for s in &asked {
        assert!(
            s.contains("read_path follows dot-dot out of the world"),
            "the claim"
        );
        assert!(s.contains(POC), "the witness");
        for never in [SECRET, IMPACT_REASON, FIX, NOTE, "Wrote the reader."] {
            assert!(!s.contains(never), "the verifier saw `{never}`");
        }
    }
    // The finder's own run did hold it.
    assert!(seen.iter().any(|s| s.contains(SECRET)));
    // Never applied: the next landing on the same file passes the gate like any other.
    let _ = answer(&mut run, "change it").await;
    let log = fs::read_to_string(w.world().join(".mnemorph/log.md")).unwrap();
    assert_eq!(
        log.matches("gate: PASS (challenge pending)").count(),
        2,
        "{log}"
    );
    assert_eq!(w.records().records().len(), 2);
}

/// A refuted finding is demoted to `@?` with its reason, never counts nor blocks, and the
/// challenge clears; with no other hunter born, the generalist verifies.
#[tokio::test]
async fn a_refuted_finding_is_demoted() {
    let w = World::new("refuted", &["dark-elf-harness", "dark-elf"]);
    let (fp, report) = w.finding(Severity::High);
    let model = script(
        &report,
        vec![
            verdict(
                &fp,
                &json!({
                    "verdict": "refuted",
                    "reason": "read_path is reached only after the grant check refuses dot-dot",
                    "files": [{"path": FILE, "sha256": sha(BODY.as_bytes())}]
                }),
            ),
            ScriptedModel::text("@S DONE\n@E"),
        ],
    );
    let mut run = w.throne(model);
    let text = answer(&mut run, "add a reader").await;
    assert!(
        text.contains(&format!("@? #{fp} refuted by dark-elf:")),
        "{text}"
    );
    assert!(text.contains("challenge cleared"), "{text}");
    let r = w.only();
    assert_eq!(r.status, Status::Cleared);
    let f = &r.findings[0];
    assert!(!f.counts() && !f.blocks());
    assert!(matches!(&f.staff, Some(Staff::Commissioned { body, .. }) if body == "dark-elf"));
    assert!(!w.records().outcomes()[0].confirmed);
}

/// `cannot-verify` on a critical finding blocks the landing and goes to the human.
#[tokio::test]
async fn a_critical_that_cannot_be_verified_blocks_for_the_human() {
    let w = World::new("cannot-critical", &["dark-elf-harness", "dark-elf-agent"]);
    let (fp, report) = w.finding(Severity::Critical);
    let model = script(
        &report,
        vec![
            verdict(
                &fp,
                &json!({"verdict": "cannot-verify", "blocker": "the proof needs a second machine to reach"}),
            ),
            ScriptedModel::text("@S DONE\n@E"),
        ],
    );
    let mut run = w.throne(model);
    let text = answer(&mut run, "add a reader").await;
    assert!(
        text.contains(&format!("blocked — #{fp} critical")),
        "{text}"
    );
    assert!(text.contains("the human decides"), "{text}");
    assert!(text.contains("challenge waive"), "{text}");
    let f = &w.only().findings[0];
    assert!(!f.counts() && f.blocks() && f.proposal.is_none());
}

/// `cannot-verify` on a high finding: it never counts, does not block, and the challenge clears.
#[tokio::test]
async fn a_high_that_cannot_be_verified_does_not_count() {
    let w = World::new("cannot-high", &["dark-elf-harness", "dark-elf-agent"]);
    let (fp, report) = w.finding(Severity::High);
    let model = script(
        &report,
        vec![
            verdict(
                &fp,
                &json!({"verdict": "cannot-verify", "blocker": "the proof needs a second machine to reach"}),
            ),
            ScriptedModel::text("@S DONE\n@E"),
        ],
    );
    let mut run = w.throne(model);
    let text = answer(&mut run, "add a reader").await;
    assert!(text.contains(&format!("@? nv #{fp} high")), "{text}");
    let r = w.only();
    assert_eq!(r.status, Status::Cleared);
    assert!(!r.findings[0].counts() && !r.findings[0].blocks());
    assert!(w.records().outcomes().is_empty(), "no outcome to fold");
}

/// A verifier that raises the severity is refused, and so is a confirmation with no re-run of
/// its own; the refusal reaches it while it can still fix the verdict.
#[tokio::test]
async fn a_raised_or_unrederived_confirmation_is_refused() {
    let w = World::new("raised", &["dark-elf-harness", "dark-elf-agent"]);
    let (fp, report) = w.finding(Severity::High);
    let model = script(
        &report,
        vec![
            verdict(
                &fp,
                &json!({"verdict": "confirmed", "severity": "critical", "commands": [{"run": RERUN}]}),
            ),
            ScriptedModel::text("@S DONE\n@E"),
        ],
    );
    let mut run = w.throne(model);
    let _ = answer(&mut run, "add a reader").await;
    let f = &w.only().findings[0];
    assert!(f.verdict.is_none(), "refused: {f:?}");
    let journal = fs::read_dir(w.world().join(".mnemorph/instruments/loop"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("dark-elf-agent")
        })
        .map(|p| fs::read_to_string(p).unwrap())
        .collect::<String>();
    assert!(journal.contains("no-raise"), "{journal}");
    assert!(journal.contains("rederived"), "{journal}");
}

/// With only the finder's speciality born, no body may verify: the finding is said unverified,
/// a high one does not block, and the challenge waits — never cleared in silence.
#[tokio::test]
async fn with_no_other_speciality_born_the_finding_waits_unverified() {
    let w = World::new("unstaffed", &["dark-elf-harness"]);
    let (fp, report) = w.finding(Severity::High);
    let model = script(&report, Vec::new());
    let mut run = w.throne(model);
    let text = answer(&mut run, "add a reader").await;
    assert!(text.contains("verifying 1 finding(s)"), "{text}");
    assert!(
        text.contains(&format!(
            "#{fp} high unverified: no verifier of another speciality"
        )),
        "{text}"
    );
    let r = w.only();
    assert!(matches!(r.status, Status::Verifying { .. }));
    assert_eq!(r.findings[0].staff, Some(Staff::Unstaffed));
}

/// A command witness with captured output confirms through `reproduced`: the verifier runs the
/// witness's own command, and it prints the captured output again — while `cat` of that output,
/// filed first, is refused.
#[tokio::test]
async fn a_command_witness_confirms_by_reproducing_its_output() {
    let w = World::new("reproduced", &["dark-elf-harness", "dark-elf-agent"]);
    let (fp, report) = w.finding_by(Severity::High, true);
    let cat = format!("cat {LEAK_OUT}");
    let model = script(
        &report,
        vec![
            ScriptedModel::tool("r0", "bash", json!({"command": cat})),
            verdict(
                &fp,
                &json!({"verdict": "confirmed", "commands": [{"run": cat}]}),
            ),
            ScriptedModel::tool("r1", "bash", json!({"command": LEAK})),
            verdict(
                &fp,
                &json!({"verdict": "confirmed", "commands": [{"run": LEAK}]}),
            ),
            ScriptedModel::text("@S DONE\n@E"),
        ],
    );
    let mut run = w.throne(model);
    let text = answer(&mut run, "add a reader").await;
    assert!(text.contains("confirmed by dark-elf-agent"), "{text}");
    let f = &w.only().findings[0];
    assert!(f.counts(), "{f:?}");
    let Some(Ruling::Confirmed { reran, .. }) = f.verdict.as_ref().map(|v| &v.ruling) else {
        panic!("confirmed");
    };
    assert_eq!(reran[0].run, LEAK);
    assert_eq!(
        reran[0].sha256,
        sha(LEAKED.as_bytes()),
        "the captured output, printed again"
    );
}
