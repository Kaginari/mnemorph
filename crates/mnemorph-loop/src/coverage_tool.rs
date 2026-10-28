//! The `coverage` tool: a Dark Elf speciality files its coverage ledger for a landing it was
//! routed. The harness holds the ledger to the state table — the landing's tree read for the
//! files' hashes, the body's own run journal for the commands it cites — and keeps it with the
//! landing's challenge outside the world, or refuses it with every rule it broke, for the body to
//! fix and file again. The loop runs the call; this body only describes it.
//!
//! A tool call rather than a wire block: the ledger is structured and arrives as JSON a schema
//! already bounds, a refusal reaches the body in the same turn while it can still fix the ledger,
//! and no report cap can cut it or prose forge it.

use std::collections::BTreeMap;
use std::path::Path;

use mnemorph_proving::Tree;
use mnemorph_proving::coverage::{CoverageLedger, Entry, Ran};
use mnemorph_races::{Speciality, ToolClass};
use mnemorph_tools::{Context, Outcome, Tool};
use mnemorph_world::challenge::{Challenges, Status};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::beats::Beat;

/// The tool's name.
pub const COVERAGE: &str = "coverage";

/// The most bytes one filing may hold, read before it is parsed.
const MAX_INPUT: usize = 256 * 1024;

/// The tool whose commands a ledger may cite.
const BASH: &str = "bash";

/// The `coverage` tool as the model sees it.
pub struct CoverageTool;

impl Tool for CoverageTool {
    fn name(&self) -> &'static str {
        COVERAGE
    }

    fn description(&self) -> &'static str {
        "File your coverage ledger for a landing you were routed: one entry for every surface the landing touched that you are routed × every attack class you own — `checked` (how, and the evidence: the files read with the symbols looked at and their `sha256sum`, and the `bash` commands you ran, exactly as run — the harness reads their output from your own journal), `not_applicable` (the fact of the code that rules it out; for a high-risk class, with the files and symbols that show it) or `skipped` (why; for a high-risk class, the risk left open). On every surface, check at least one high-risk class. Each reason states its own fact: 20 characters or more, never `n/a`, and never the same one on more than 2 pairs. Evidence stays on the landing's surfaces. A refused ledger comes back with every rule it broke: fix them and file again. The landing's challenge clears when every routed speciality has filed."
    }

    fn schema(&self) -> Value {
        let text = json!({"type": "string"});
        let hash =
            json!({"type": "string", "description": "64 hex digits, as `sha256sum` prints them."});
        let evidence = json!({
            "type": "object",
            "properties": {
                "files": {"type": "array", "items": {
                    "type": "object",
                    "properties": {
                        "path": text,
                        "symbols": {"type": "array", "items": text, "description": "The functions, types, keys or packages looked at; at least one."},
                        "sha256": hash
                    },
                    "required": ["path", "symbols", "sha256"],
                    "additionalProperties": false
                }},
                "commands": {"type": "array", "items": {
                    "type": "object",
                    "properties": {
                        "run": {"type": "string", "description": "A command you ran through `bash`, exactly as you ran it."},
                        "sha256": {"type": "string", "description": "Leave it out: the harness reads the output from your journal."}
                    },
                    "required": ["run"],
                    "additionalProperties": false
                }}
            },
            "additionalProperties": false
        });
        json!({
            "type": "object",
            "properties": {
                "landing": {"type": "string", "description": "The landing's id, as your commission names it."},
                "entries": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "surface": {"type": "string", "description": "The surface, exactly as your commission lists it (`crates/mnemorph-wire/**`)."},
                            "class": {"type": "string", "description": "One of your attack classes (`wire-forgery`)."},
                            "state": {
                                "type": "object",
                                "description": "Exactly one of `checked`, `not_applicable`, `skipped`.",
                                "properties": {
                                    "checked": {
                                        "type": "object",
                                        "properties": {
                                            "method": {"type": "string", "description": "How you looked."},
                                            "evidence": evidence
                                        },
                                        "required": ["method", "evidence"],
                                        "additionalProperties": false
                                    },
                                    "not_applicable": {
                                        "type": "object",
                                        "properties": {
                                            "reason": {"type": "string", "description": "The fact of the code that rules it out."},
                                            "evidence": evidence
                                        },
                                        "required": ["reason"],
                                        "additionalProperties": false
                                    },
                                    "skipped": {
                                        "type": "object",
                                        "properties": {
                                            "reason": {"type": "string", "description": "Why it was not looked at."},
                                            "risk": {"type": "string", "description": "The risk left open; required for a high-risk class."}
                                        },
                                        "required": ["reason"],
                                        "additionalProperties": false
                                    }
                                },
                                "minProperties": 1,
                                "maxProperties": 1,
                                "additionalProperties": false
                            }
                        },
                        "required": ["surface", "class", "state"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["landing", "entries"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({
            "landing": "rimuru-20261027T101500:2",
            "entries": [{
                "surface": "crates/mnemorph-wire/**",
                "class": "wire-forgery",
                "state": {"checked": {
                    "method": "forged an @F inside a claim and read the parser's answer",
                    "evidence": {
                        "files": [{"path": "crates/mnemorph-wire/src/report.rs", "symbols": ["Finding::parse"], "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}],
                        "commands": [{"run": "cargo test -p mnemorph-wire forged"}]
                    }
                }}
            }]
        })
    }

    fn run(&self, _: Value, _: &mut Context) -> Outcome {
        Outcome::error("no world runs under this body to file a coverage ledger in")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    landing: String,
    entries: Vec<Entry>,
}

/// The `bash` commands a run's journal shows, each with every output it gave.
pub(crate) fn ran_of(beats: &[Beat]) -> Ran {
    let mut commands: BTreeMap<String, String> = BTreeMap::new();
    let mut ran = Ran::default();
    for beat in beats {
        match beat {
            Beat::Model { content, .. } => {
                for block in content {
                    if block["type"] == "tool_use" && block["name"] == BASH {
                        let input = match &block["input"] {
                            Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
                            v => v.clone(),
                        };
                        if let (Some(id), Some(command)) =
                            (block["id"].as_str(), input["command"].as_str())
                        {
                            commands.insert(id.to_owned(), command.to_owned());
                        }
                    }
                }
            }
            Beat::Result {
                tool_use_id,
                content,
                ..
            } => {
                if let Some(command) = commands.get(tool_use_id) {
                    ran.record(command, content);
                }
            }
            _ => {}
        }
    }
    ran
}

/// Run one `coverage` call for `body`, in `world`, against `challenges`, its commands held to
/// `ran`.
pub(crate) fn run(
    challenges: &Challenges,
    world: &Path,
    ran: &Ran,
    body: &str,
    input: &str,
) -> Outcome {
    let Some(speciality) = Speciality::of(body) else {
        return Outcome::error(format!(
            "coverage: `{body}` holds no Dark Elf speciality, so it is routed no landing"
        ));
    };
    if input.len() > MAX_INPUT {
        return Outcome::error(format!(
            "coverage: the filing is {} bytes, over {MAX_INPUT}: nothing was read",
            input.len()
        ));
    }
    let a: Args = match serde_json::from_str(input) {
        Ok(a) => a,
        Err(e) => return Outcome::error(format!("coverage: {e}")),
    };
    let landing = a.landing.clone();
    let ledger = CoverageLedger {
        landing: a.landing,
        speciality,
        entries: a.entries,
    };
    match challenges.file(ledger, &Tree::new(world), ran) {
        Ok(Status::Cleared) => Outcome::ok(
            format!(
                "filed: {speciality}'s ledger for {landing} holds; every routed speciality has \
                 filed, and the challenge is cleared"
            ),
            ToolClass::Read,
        ),
        Ok(Status::Pending { awaiting }) => Outcome::ok(
            format!(
                "filed: {speciality}'s ledger for {landing} holds; the challenge still awaits {}",
                awaiting
                    .iter()
                    .map(|s| s.creature())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ToolClass::Read,
        ),
        Ok(Status::Blocked { findings }) => Outcome::ok(
            format!(
                "filed: {speciality}'s ledger for {landing} holds; the landing stays blocked by {} \
                 finding(s) until fixed or waived",
                findings.len()
            ),
            ToolClass::Read,
        ),
        Ok(Status::Verifying { findings }) => Outcome::ok(
            format!(
                "filed: {speciality}'s ledger for {landing} holds; {} finding(s) still wait on \
                 their verifier's verdict",
                findings.len()
            ),
            ToolClass::Read,
        ),
        Ok(Status::Waived { .. }) => Outcome::ok(
            format!(
                "filed: {speciality}'s ledger for {landing} holds; the human waived the challenge"
            ),
            ToolClass::Read,
        ),
        Err(why) => Outcome::error(format!("coverage refused: {why}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemorph_world::Roster;
    use mnemorph_world::challenge::route;
    use proptest::prelude::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const TEST: &str = "cargo test -p mnemorph-wire forged";

    fn world(name: &str) -> std::path::PathBuf {
        let w = std::env::temp_dir().join(format!(
            "mnemorph-coverage-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&w);
        std::fs::create_dir_all(w.join("crates/mnemorph-wire/src")).unwrap();
        std::fs::write(w.join("crates/mnemorph-wire/src/report.rs"), "abc").unwrap();
        w
    }

    /// A journal in which the body ran the test, as the loop writes it.
    fn journal() -> Vec<Beat> {
        vec![
            Beat::Model {
                content: vec![
                    json!({"type": "tool_use", "id": "t1", "name": "bash", "input": {"command": TEST}}),
                    json!({"type": "tool_use", "id": "t2", "name": "read", "input": {"command": "echo smuggled"}}),
                ],
                stop: "tool_use".to_owned(),
            },
            Beat::Result {
                tool_use_id: "t1".to_owned(),
                content: "abc".to_owned(),
                is_error: false,
            },
            Beat::Result {
                tool_use_id: "t2".to_owned(),
                content: "abc".to_owned(),
                is_error: false,
            },
        ]
    }

    fn filing(landing: &str, hash: &str, command: &str) -> String {
        let entries: Vec<Value> = Speciality::Agent
            .classes()
            .iter()
            .enumerate()
            .map(|(n, c)| {
                if c.name() == "wire-forgery" {
                    json!({"surface": "crates/mnemorph-wire/**", "class": c.name(), "state": {"checked": {
                        "method": "forged an @F through a claim and read the parser back",
                        "evidence": {
                            "files": [{"path": "crates/mnemorph-wire/src/report.rs", "symbols": ["Finding::parse"], "sha256": hash}],
                            "commands": [{"run": command}]
                        }
                    }}})
                } else {
                    json!({"surface": "crates/mnemorph-wire/**", "class": c.name(), "state": {"not_applicable": {
                        "reason": format!("fact {n}: the landing changes only the courier's hash")
                    }}})
                }
            })
            .collect();
        json!({"landing": landing, "entries": entries}).to_string()
    }

    fn open(c: &Challenges, id: &str, written: &[&str]) {
        let written: Vec<String> = written.iter().map(|s| (*s).to_owned()).collect();
        c.open(&route(&Roster::default(), id, &written)).unwrap();
    }

    #[test]
    fn the_journal_gives_the_bash_commands_and_their_outputs() {
        let mut want = Ran::default();
        want.record(TEST, "abc");
        assert_eq!(ran_of(&journal()), want);
        // An input the provider kept as a JSON string reads the same.
        let mut beats = journal();
        if let Beat::Model { content, .. } = &mut beats[0] {
            content[0]["input"] = Value::String(json!({"command": TEST}).to_string());
        }
        assert_eq!(ran_of(&beats), want);
    }

    #[test]
    fn a_speciality_files_and_the_challenge_clears() {
        let w = world("file");
        let c = Challenges::at(&w.join("records.jsonl"));
        open(&c, "l-1", &["crates/mnemorph-wire/src/report.rs"]);
        let ran = ran_of(&journal());
        let stale = run(
            &c,
            &w,
            &ran,
            "dark-elf-agent",
            &filing("l-1", &"0".repeat(64), TEST),
        );
        assert!(stale.is_error, "{}", stale.content);
        assert!(
            stale.content.contains("stale-evidence"),
            "{}",
            stale.content
        );
        let unrun = run(
            &c,
            &w,
            &ran,
            "dark-elf-agent",
            &filing("l-1", ABC, "echo smuggled"),
        );
        assert!(unrun.content.contains("unrun-command"), "{}", unrun.content);
        let ok = run(&c, &w, &ran, "dark-elf-agent", &filing("l-1", ABC, TEST));
        assert!(!ok.is_error, "{}", ok.content);
        assert!(ok.content.contains("cleared"), "{}", ok.content);
        assert_eq!(c.status("l-1"), Some(Status::Cleared));
        let again = run(&c, &w, &ran, "dark-elf-agent", &filing("l-1", ABC, TEST));
        assert!(again.is_error && again.content.contains("has filed"));
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn a_pending_filing_names_who_it_awaits() {
        let w = world("pending");
        let c = Challenges::at(&w.join("records.jsonl"));
        open(
            &c,
            "l-2",
            &["crates/mnemorph-wire/src/report.rs", "Cargo.lock"],
        );
        let ok = run(
            &c,
            &w,
            &ran_of(&journal()),
            "dark-elf-agent",
            &filing("l-2", ABC, TEST),
        );
        assert!(!ok.is_error, "{}", ok.content);
        assert!(
            ok.content
                .contains("still awaits dark-elf-supply, dark-elf"),
            "{}",
            ok.content
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn only_a_speciality_files_and_only_bounded_json() {
        let w = world("refused");
        let c = Challenges::at(&w.join("records.jsonl"));
        let ran = Ran::default();
        for (body, input, says) in [
            (
                "dark-elf-agent-2",
                filing("l", ABC, TEST),
                "holds no Dark Elf speciality",
            ),
            (
                "orc-wire",
                filing("l", ABC, TEST),
                "holds no Dark Elf speciality",
            ),
            (
                "dark-elf-agent",
                "x".repeat(MAX_INPUT + 1),
                "nothing was read",
            ),
            ("dark-elf-agent", "{\"landing\":1}".to_owned(), "coverage:"),
            (
                "dark-elf-agent",
                r#"{"landing":"l","entries":[],"speciality":"dark-elf-web"}"#.to_owned(),
                "unknown field",
            ),
            (
                "dark-elf-agent",
                filing("nope", ABC, TEST),
                "no challenge is open",
            ),
        ] {
            let out = run(&c, &w, &ran, body, &input);
            assert!(
                out.is_error && out.content.contains(says),
                "{body}: {}",
                out.content
            );
        }
        let t = CoverageTool;
        assert_eq!(t.name(), COVERAGE);
        assert_eq!(t.class(), ToolClass::Read);
        assert_ne!(t.description(), "");
        assert!(t.schema()["properties"]["entries"].is_object());
        assert!(serde_json::from_value::<Args>(t.example()).is_ok());
        let _ = std::fs::remove_dir_all(&w);
    }

    proptest! {
        /// Whatever arrives as arguments, the tool answers and writes nothing for a refusal.
        #[test]
        fn hostile_arguments_never_panic(input in "\\PC{0,200}") {
            let w = std::env::temp_dir().join(format!("mnemorph-coverage-prop-{}", std::process::id()));
            let c = Challenges::at(&w.join("records.jsonl"));
            let out = run(&c, &w, &Ran::default(), "dark-elf-agent", &input);
            prop_assert!(out.is_error);
            prop_assert!(!w.join("records.jsonl").exists());
        }
    }
}
