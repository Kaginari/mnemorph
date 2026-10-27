//! The `coverage` tool: a Dark Elf speciality files its coverage ledger for a landing it was
//! routed. The harness holds the ledger to the state table — the world's files read for their
//! hashes — and keeps it with the landing's challenge outside the world, or refuses it with every
//! rule it broke, for the body to fix and file again. The loop runs the call; this body only
//! describes it.
//!
//! A tool call rather than a wire block: the ledger is structured and arrives as JSON a schema
//! already bounds, a refusal reaches the body in the same turn while it can still fix the ledger,
//! and no report cap can cut it or prose forge it.

use std::path::Path;

use mnemorph_races::{Speciality, ToolClass};
use mnemorph_tools::{Context, Outcome, Tool};
use mnemorph_world::challenge::{Challenges, Status, tree_hash};
use mnemorph_world::coverage::{CoverageLedger, Entry};
use serde::Deserialize;
use serde_json::{Value, json};

/// The tool's name.
pub const COVERAGE: &str = "coverage";

/// The most bytes one filing may hold, read before it is parsed.
const MAX_INPUT: usize = 512 * 1024;

/// The `coverage` tool as the model sees it.
pub struct CoverageTool;

impl Tool for CoverageTool {
    fn name(&self) -> &'static str {
        COVERAGE
    }

    fn description(&self) -> &'static str {
        "File your coverage ledger for a landing you were routed: one entry for every surface the landing touched that you are routed × every attack class you own — `checked` (how, and the files read with their symbols and `sha256sum`, the commands run with the `sha256sum` of their output), `not_applicable` (the fact of the code that rules it out) or `skipped` (why; for a high-risk class, the risk left open). Reasons say something: 20 characters or more, never `n/a` or `not relevant`. Evidence stays on the landing's surfaces. A refused ledger comes back with every rule it broke: fix them and file again. The landing's challenge clears when every routed speciality has filed."
    }

    fn schema(&self) -> Value {
        let text = json!({"type": "string"});
        let hash =
            json!({"type": "string", "description": "64 hex digits, as `sha256sum` prints them."});
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
                                            "evidence": {
                                                "type": "object",
                                                "properties": {
                                                    "files": {"type": "array", "items": {
                                                        "type": "object",
                                                        "properties": {"path": text, "symbols": {"type": "array", "items": text}, "sha256": hash},
                                                        "required": ["path", "sha256"],
                                                        "additionalProperties": false
                                                    }},
                                                    "commands": {"type": "array", "items": {
                                                        "type": "object",
                                                        "properties": {"run": text, "sha256": hash},
                                                        "required": ["run", "sha256"],
                                                        "additionalProperties": false
                                                    }}
                                                },
                                                "additionalProperties": false
                                            }
                                        },
                                        "required": ["method", "evidence"],
                                        "additionalProperties": false
                                    },
                                    "not_applicable": {
                                        "type": "object",
                                        "properties": {"reason": {"type": "string", "description": "The fact of the code that rules it out."}},
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
                "state": {"not_applicable": {"reason": "the landing changes only the courier's hash, which no parser reads"}}
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

/// Run one `coverage` call for `body`, in `world`, against `challenges`.
pub(crate) fn run(challenges: &Challenges, world: &Path, body: &str, input: &str) -> Outcome {
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
    let ledger = CoverageLedger {
        landing: a.landing,
        speciality,
        entries: a.entries,
    };
    match challenges.file(&ledger, &|path: &str| tree_hash(world, path)) {
        Ok(Status::Cleared) => Outcome::ok(
            format!(
                "filed: {speciality}'s ledger for {} holds; every routed speciality has filed, \
                 and the challenge is cleared",
                ledger.landing
            ),
            ToolClass::Read,
        ),
        Ok(Status::Pending { awaiting }) => Outcome::ok(
            format!(
                "filed: {speciality}'s ledger for {} holds; the challenge still awaits {}",
                ledger.landing,
                awaiting
                    .iter()
                    .map(|s| s.creature())
                    .collect::<Vec<_>>()
                    .join(", ")
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

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn filing(landing: &str, hash: &str) -> String {
        let entries: Vec<Value> = Speciality::Agent
            .classes()
            .iter()
            .map(|c| {
                if c.name() == "wire-forgery" {
                    json!({"surface": "crates/mnemorph-wire/**", "class": c.name(), "state": {"checked": {
                        "method": "forged an @F through a claim and read the parser back",
                        "evidence": {"files": [{"path": "crates/mnemorph-wire/src/report.rs", "symbols": ["Finding::parse"], "sha256": hash}]}
                    }}})
                } else {
                    json!({"surface": "crates/mnemorph-wire/**", "class": c.name(), "state": {"not_applicable": {
                        "reason": "the landing changes only the courier's hash, which no parser reads"
                    }}})
                }
            })
            .collect();
        json!({"landing": landing, "entries": entries}).to_string()
    }

    #[test]
    fn a_speciality_files_and_the_challenge_clears() {
        let w = world("file");
        let c = Challenges::at(&w.join("records.jsonl"));
        let landing = route(
            &Roster::default(),
            "l-1",
            &["crates/mnemorph-wire/src/report.rs".to_owned()],
        );
        c.open(&landing).unwrap();
        let stale = run(&c, &w, "dark-elf-agent", &filing("l-1", &"0".repeat(64)));
        assert!(stale.is_error, "{}", stale.content);
        assert!(
            stale.content.contains("stale-evidence"),
            "{}",
            stale.content
        );
        let ok = run(&c, &w, "dark-elf-agent", &filing("l-1", ABC));
        assert!(!ok.is_error, "{}", ok.content);
        assert!(ok.content.contains("cleared"), "{}", ok.content);
        assert_eq!(c.status("l-1"), Some(Status::Cleared));
        let again = run(&c, &w, "dark-elf-agent", &filing("l-1", ABC));
        assert!(again.is_error && again.content.contains("has filed"));
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn a_pending_filing_names_who_it_awaits() {
        let w = world("pending");
        let c = Challenges::at(&w.join("records.jsonl"));
        let landing = route(
            &Roster::default(),
            "l-2",
            &[
                "crates/mnemorph-wire/src/report.rs".to_owned(),
                "Cargo.lock".to_owned(),
            ],
        );
        c.open(&landing).unwrap();
        let ok = run(&c, &w, "dark-elf-agent", &filing("l-2", ABC));
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
        for (body, input, says) in [
            (
                "dark-elf-agent-2",
                filing("l", ABC),
                "holds no Dark Elf speciality",
            ),
            ("orc-wire", filing("l", ABC), "holds no Dark Elf speciality"),
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
                filing("nope", ABC),
                "no challenge is open",
            ),
        ] {
            let out = run(&c, &w, body, &input);
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
            let out = run(&c, &w, "dark-elf-agent", &input);
            prop_assert!(out.is_error);
            prop_assert!(!w.join("records.jsonl").exists());
        }
    }
}
