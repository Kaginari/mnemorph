//! The `verdict` tool: a verifier files its verdict on the finding it was assigned. The harness
//! stamps the commands it cites from its own run journal, holds the verdict to its guards — the
//! verifier assigned, independent of the finder and the author, the severity never raised, a
//! confirmation re-derived, a refutation evidenced — and keeps it with the landing's challenge
//! outside the world, or refuses it with every guard it broke. The loop runs the call; this body
//! only describes it.

use std::path::Path;

use mnemorph_proving::coverage::Ran;
use mnemorph_proving::{Severity, Tree};
use mnemorph_races::{Speciality, ToolClass};
use mnemorph_tools::{Context, Outcome, Tool};
use mnemorph_world::challenge::{Challenges, Filing, Pinned, Rerun, Ruling, Status};
use serde::Deserialize;
use serde_json::{Value, json};

/// The tool's name.
pub const VERDICT: &str = "verdict";

/// The most bytes one verdict may hold, read before it is parsed.
const MAX_INPUT: usize = 64 * 1024;

/// The `verdict` tool as the model sees it.
pub struct VerdictTool;

impl Tool for VerdictTool {
    fn name(&self) -> &'static str {
        VERDICT
    }

    fn description(&self) -> &'static str {
        "File your verdict on the finding you were commissioned to verify — one verdict, for that landing and that fingerprint only. `confirmed`: you re-ran the witness yourself; list the `bash` commands exactly as you ran them (the harness reads their output from your own journal; a command's captured output must be printed again), and give a lower `severity` if the evidence shows less — never a higher one. `refuted`: `reason` says why it does not hold, with the `files` (path and `sha256sum`) or the `commands` that show it. `cannot-verify`: `blocker` says what stopped you. Reasons and blockers state their own fact: 20 characters or more, never `n/a`. A refused verdict comes back with every guard it broke."
    }

    fn schema(&self) -> Value {
        let text = json!({"type": "string"});
        json!({
            "type": "object",
            "properties": {
                "landing": {"type": "string", "description": "The landing's id, as your commission names it."},
                "fingerprint": {"type": "string", "description": "The finding's fingerprint, as your commission names it."},
                "verdict": {"type": "string", "enum": ["confirmed", "refuted", "cannot-verify"]},
                "severity": {"type": "string", "enum": ["informational", "low", "medium", "high", "critical"], "description": "For `confirmed`: the severity the evidence shows, at most the record's. Left out, the record's stands."},
                "reason": {"type": "string", "description": "For `refuted`: why it does not hold."},
                "blocker": {"type": "string", "description": "For `cannot-verify`: what stopped you."},
                "files": {"type": "array", "items": {
                    "type": "object",
                    "properties": {"path": text, "sha256": {"type": "string", "description": "64 hex digits, as `sha256sum` prints them."}},
                    "required": ["path", "sha256"],
                    "additionalProperties": false
                }},
                "commands": {"type": "array", "items": {
                    "type": "object",
                    "properties": {"run": {"type": "string", "description": "A command you ran through `bash`, exactly as you ran it."}},
                    "required": ["run"],
                    "additionalProperties": false
                }}
            },
            "required": ["landing", "fingerprint", "verdict"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({
            "landing": "rimuru-20261027T101500:2",
            "fingerprint": "3f9a0c2d71e4b856",
            "verdict": "confirmed",
            "commands": [{"run": "cargo test --test read_path"}]
        })
    }

    fn run(&self, _: Value, _: &mut Context) -> Outcome {
        Outcome::error("no world runs under this body to file a verdict in")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    run: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    landing: String,
    fingerprint: String,
    verdict: String,
    #[serde(default)]
    severity: Option<Severity>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    blocker: Option<String>,
    #[serde(default)]
    files: Vec<Pinned>,
    #[serde(default)]
    commands: Vec<Command>,
}

/// Run one `verdict` call for `body`, in `world`, against `challenges`, its commands held to
/// `ran`.
pub(crate) fn run(
    challenges: &Challenges,
    world: &Path,
    ran: &Ran,
    body: &str,
    input: &str,
) -> Outcome {
    if Speciality::of(body).is_none() {
        return Outcome::error(format!(
            "verdict: `{body}` holds no Dark Elf speciality, so it verifies nothing"
        ));
    }
    if input.len() > MAX_INPUT {
        return Outcome::error(format!(
            "verdict: the filing is {} bytes, over {MAX_INPUT}: nothing was read",
            input.len()
        ));
    }
    let a: Args = match serde_json::from_str(input) {
        Ok(a) => a,
        Err(e) => return Outcome::error(format!("verdict: {e}")),
    };
    let reran: Vec<Rerun> = a
        .commands
        .into_iter()
        .map(|c| Rerun {
            run: c.run,
            sha256: String::new(),
        })
        .collect();
    let ruling = match a.verdict.as_str() {
        "confirmed" => {
            // Left out, the severity the finding was found at stands.
            let found = challenges.record(&a.landing).and_then(|r| {
                r.findings
                    .iter()
                    .find(|f| f.fingerprint() == a.fingerprint)
                    .map(|f| f.found.severity())
            });
            let Some(severity) = a.severity.or(found) else {
                return Outcome::error(format!(
                    "verdict refused: no finding #{} waits on landing `{}`",
                    crate::challenge::safe(&a.fingerprint),
                    crate::challenge::safe(&a.landing)
                ));
            };
            Ruling::Confirmed { severity, reran }
        }
        "refuted" => Ruling::Refuted {
            reason: a.reason.unwrap_or_default(),
            files: a.files,
            reran,
        },
        "cannot-verify" => Ruling::CannotVerify {
            blocker: a.blocker.unwrap_or_default(),
        },
        other => {
            return Outcome::error(format!(
                "verdict: `{}` is no verdict: `confirmed`, `refuted` or `cannot-verify`",
                crate::challenge::safe(other)
            ));
        }
    };
    let said = ruling.word();
    let filing = Filing {
        landing: a.landing.clone(),
        fingerprint: a.fingerprint.clone(),
        verifier: body.to_owned(),
        ruling,
    };
    match challenges.verdict(filing, &Tree::new(world), ran) {
        Ok(status) => {
            let now = match status {
                Status::Blocked { findings } => {
                    format!("the landing is blocked by {} finding(s)", findings.len())
                }
                Status::Pending { .. } => "the challenge still awaits ledgers".to_owned(),
                Status::Verifying { findings } => {
                    format!("{} finding(s) still wait on a verdict", findings.len())
                }
                Status::Cleared => "the challenge is cleared".to_owned(),
                Status::Waived { .. } => "the human waived the challenge".to_owned(),
            };
            Outcome::ok(
                format!(
                    "filed: {said} on #{} for {}; {now}",
                    crate::challenge::safe(&a.fingerprint),
                    crate::challenge::safe(&a.landing)
                ),
                ToolClass::Read,
            )
        }
        Err(why) => Outcome::error(format!("verdict refused: {why}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the body assigned files the verdict, and it never raises the severity.
    #[test]
    fn an_unassigned_or_raised_verdict_is_refused() {
        use mnemorph_world::Roster;
        use mnemorph_world::challenge::{Found, Staff, route};
        let w =
            std::env::temp_dir().join(format!("mnemorph-verdict-assigned-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        let c = Challenges::at(&w.join("records.jsonl"));
        let l = route(
            &Roster::default(),
            "l-1",
            &["crates/mnemorph-tools/src/files.rs".to_owned()],
        );
        c.open(&l).unwrap();
        let record = serde_json::from_value(json!({
            "verdict": "finding",
            "fingerprint": "0123456789abcdef",
            "speciality": "dark-elf-harness",
            "severity": "high",
            "witness": {"kind": "poc-command", "reference": "cargo test",
                        "sha256": mnemorph_proving::sha256_hex(b"cargo test")}
        }))
        .unwrap();
        c.found(
            "l-1",
            Found {
                finder: "dark-elf-harness".into(),
                rec: String::new(),
                record,
            },
        )
        .unwrap();
        c.assign(
            "l-1",
            "0123456789abcdef",
            Staff::Commissioned {
                body: "dark-elf-agent".into(),
                by: "rimuru".into(),
                wt: 0,
                waited: 0,
            },
        )
        .unwrap();
        let mut ran = Ran::default();
        ran.record("cargo test", "FAILED");
        let filing = |severity: &str| {
            json!({"landing": "l-1", "fingerprint": "0123456789abcdef", "verdict": "confirmed",
                   "severity": severity, "commands": [{"run": "cargo test"}]})
            .to_string()
        };
        let other = run(&c, &w, &ran, "dark-elf-web", &filing("high"));
        assert!(
            other.is_error && other.content.contains("assigned"),
            "{}",
            other.content
        );
        let raised = run(&c, &w, &ran, "dark-elf-agent", &filing("critical"));
        assert!(
            raised.is_error && raised.content.contains("no-raise"),
            "{}",
            raised.content
        );
        let lowered = run(&c, &w, &ran, "dark-elf-agent", &filing("medium"));
        assert!(!lowered.is_error, "{}", lowered.content);
        assert!(
            lowered.content.contains("filed: confirmed"),
            "{}",
            lowered.content
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn the_tool_describes_itself_and_refuses_what_it_cannot_read() {
        let t = VerdictTool;
        assert_eq!(t.name(), VERDICT);
        assert_eq!(t.class(), ToolClass::Read);
        assert_ne!(t.description(), "");
        assert!(t.schema()["properties"]["verdict"].is_object());
        assert!(serde_json::from_value::<Args>(t.example()).is_ok());
        let w = std::env::temp_dir().join(format!("mnemorph-verdict-{}", std::process::id()));
        let c = Challenges::at(&w.join("records.jsonl"));
        let ran = Ran::default();
        let good = json!({"landing": "l", "fingerprint": "f", "verdict": "cannot-verify", "blocker": "the sandbox denies the network"}).to_string();
        for (body, input, says) in [
            ("orc-o", good.clone(), "holds no Dark Elf speciality"),
            (
                "dark-elf-agent",
                "x".repeat(MAX_INPUT + 1),
                "nothing was read",
            ),
            ("dark-elf-agent", "{}".to_owned(), "verdict:"),
            (
                "dark-elf-agent",
                json!({"landing": "l", "fingerprint": "f", "verdict": "maybe"}).to_string(),
                "is no verdict",
            ),
            (
                "dark-elf-agent",
                json!({"landing": "l", "fingerprint": "f", "verdict": "confirmed"}).to_string(),
                "no finding #f waits",
            ),
            ("dark-elf-agent", good, "no challenge is open"),
        ] {
            let out = run(&c, &w, &ran, body, &input);
            assert!(
                out.is_error && out.content.contains(says),
                "{body}: {}",
                out.content
            );
        }
        assert!(!w.join("records.jsonl").exists());
    }
}
