//! The `verify` tool: an Orc codes the verify line of a Slime that reports to it — the test
//! before the change, which the dispatch law asks before the Orc commissions it. The loop runs the
//! call through the world; this body only describes it.

use mnemorph_races::ToolClass;
use mnemorph_tools::{Context, Outcome, Tool};
use serde::Deserialize;
use serde_json::{Value, json};

/// The tool's name.
pub const VERIFY: &str = "verify";

/// The `verify` tool as the model sees it.
pub struct VerifyTool;

impl Tool for VerifyTool {
    fn name(&self) -> &'static str {
        VERIFY
    }

    fn description(&self) -> &'static str {
        "Code the verify line of a Slime that reports to you: one shell command that passes once the work is done (the test before the change). The dispatch law asks it before you commission that Slime; its end gate runs it on every landing."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "to": {"type": "string", "description": "The Slime, by name (`slime-wire`)."},
                "command": {"type": "string", "description": "One command, one line, no backticks: e.g. `python3 -m unittest -q`."}
            },
            "required": ["to", "command"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Dispatch
    }

    fn example(&self) -> Value {
        json!({"to": "slime-wire", "command": "cargo test -p wire"})
    }

    fn run(&self, _: Value, _: &mut Context) -> Outcome {
        Outcome::error("no world runs under this body to code a verify line in")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    to: String,
    command: String,
}

/// Run one `verify` call for `body`, in `world`.
pub(crate) fn run(world: &std::path::Path, body: &str, input: &str) -> Outcome {
    let a: Args = match serde_json::from_str(input) {
        Ok(a) => a,
        Err(e) => return Outcome::error(format!("verify: {e}")),
    };
    match mnemorph_world::verify::code(world, body, &a.to, &a.command) {
        Ok(line) => Outcome::ok(
            format!("{}'s doc now reads {line}", a.to),
            ToolClass::Dispatch,
        ),
        Err(why) => Outcome::error(format!("refused: {why}")),
    }
}
