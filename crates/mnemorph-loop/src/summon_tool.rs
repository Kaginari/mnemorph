//! The `summon` tool: an Orc starts a Slime on part of its own domain, when no Slime keeps the
//! files, for the commission it is working ([`mnemorph_world::summon`]). The loop runs the call
//! through the world; this body only describes it.

use mnemorph_races::ToolClass;
use mnemorph_tools::{Context, Outcome, Tool};
use serde::Deserialize;
use serde_json::{Value, json};

/// The tool's name.
pub const SUMMON: &str = "summon";

/// The `summon` tool as the model sees it.
pub struct SummonTool;

impl Tool for SummonTool {
    fn name(&self) -> &'static str {
        SUMMON
    }

    fn description(&self) -> &'static str {
        "Start a Slime on part of your own domain when no Slime keeps those files: it reports to you, you code its verify line and commission it like any other, and it is retired when your commission ends. At most three at once; ground a Slime already keeps is refused."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "zone": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "The paths of your domain it keeps: directories end with `/` (`src/parse/`)."
                }
            },
            "required": ["zone"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Dispatch
    }

    fn example(&self) -> Value {
        json!({"zone": ["src/parse/"]})
    }

    fn run(&self, _: Value, _: &mut Context) -> Outcome {
        Outcome::error("no world runs under this body to start a Slime in")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    zone: Vec<String>,
}

/// Run one `summon` call for `body`, in `world`.
pub(crate) fn run(world: &std::path::Path, body: &str, input: &str) -> Outcome {
    let a: Args = match serde_json::from_str(input) {
        Ok(a) => a,
        Err(e) => return Outcome::error(format!("summon: {e}")),
    };
    match mnemorph_world::summon::summon(world, body, &a.zone) {
        Ok(s) => {
            let how = if s.woke {
                format!("woke {}, with what it learned before,", s.name)
            } else {
                format!("started {}", s.name)
            };
            Outcome::ok(
                format!(
                    "{how} on {}: code its verify line, then commission it",
                    a.zone.join(", ")
                ),
                ToolClass::Dispatch,
            )
        }
        Err(why) => Outcome::error(format!("refused: {why}")),
    }
}
