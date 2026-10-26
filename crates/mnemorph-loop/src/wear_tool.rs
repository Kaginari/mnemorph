//! The `wear` tool: the toolbox's second level. A body wears one of the Minds its manifest offers,
//! and the Mind's whole text enters its context — priced in the manifest by exactly that text. The
//! loop runs the call through the world; this body only describes it.

use mnemorph_races::ToolClass;
use mnemorph_tools::{Context, Outcome, Tool};
use serde::Deserialize;
use serde_json::{Value, json};

/// The tool's name.
pub const WEAR: &str = "wear";

/// The `wear` tool as the model sees it.
pub struct WearTool;

impl Tool for WearTool {
    fn name(&self) -> &'static str {
        WEAR
    }

    fn description(&self) -> &'static str {
        "Wear a Mind your manifest offers (`@T mind:<name>`): its whole text enters your context and stays there, at the price the manifest shows. Wear one only when the work needs its know-how."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mind": {"type": "string", "description": "The Mind, by name (`sqlglot-dialects`)."}
            },
            "required": ["mind"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({"mind": "sqlglot-dialects"})
    }

    fn run(&self, _: Value, _: &mut Context) -> Outcome {
        Outcome::error("no world runs under this body to wear a Mind from")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    mind: String,
}

/// Run one `wear` call for `body`, in `world`, with `room` tokens left in its context. A body that
/// can hand work down never wears a heavy Mind: a worn Mind cannot be unloaded, and the body below
/// that wears it dies with its task. No body wears one bigger than half the room it has left.
pub(crate) fn run(
    world: &std::path::Path,
    body: &str,
    input: &str,
    dispatches: bool,
    room: u64,
) -> Outcome {
    let a: Args = match serde_json::from_str(input) {
        Ok(a) => a,
        Err(e) => return Outcome::error(format!("wear: {e}")),
    };
    let roster = mnemorph_world::Roster::load(world);
    let wearable = mnemorph_world::minds::wearable(world, &roster, body);
    let name = a.mind.trim().trim_start_matches("mind:");
    let Some(mind) = wearable.iter().find(|m| m.name == name) else {
        let offered: Vec<&str> = wearable.iter().map(|m| m.name.as_str()).collect();
        return Outcome::error(format!(
            "wear: `{name}` is no Mind you may wear; your manifest offers: {}",
            if offered.is_empty() {
                "none".to_owned()
            } else {
                offered.join(", ")
            }
        ));
    };
    if dispatches && mind.price() > mnemorph_world::minds::HEAVY {
        return Outcome::error(format!(
            "wear: `{name}` is heavy (~{} tokens) and a worn Mind cannot be unloaded: hand the \
             work that needs it to a body below, which wears it for its task",
            mind.price()
        ));
    }
    if mind.price() > room / 2 {
        return Outcome::error(format!(
            "wear: `{name}` (~{} tokens) would take more than half the context left (~{room}): \
             finish or hand down the work first",
            mind.price()
        ));
    }
    Outcome::ok(mind.injected(), ToolClass::Read)
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn a_dispatching_throne_never_wears_a_heavy_mind() {
        let w = std::env::temp_dir().join(format!("mnemorph-wear-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        std::fs::create_dir_all(w.join(".mnemorph/minds")).unwrap();
        let heavy = format!(
            "# Heavy\n\n- **Shared:** yes\n\nA long one.\n\n{}",
            "x".repeat(10_000)
        );
        std::fs::write(w.join(".mnemorph/minds/heavy.md"), heavy).unwrap();
        std::fs::write(
            w.join(".mnemorph/minds/light.md"),
            "# Light\n\n- **Shared:** yes\n\nShort.\n",
        )
        .unwrap();
        let worn = |mind: &str, dispatches: bool| {
            run(
                &w,
                "rimuru",
                &format!(r#"{{"mind":"{mind}"}}"#),
                dispatches,
                100_000,
            )
        };
        assert!(
            !worn("light", true).is_error,
            "a light Mind is worn even by a dispatching throne"
        );
        let refused = worn("heavy", true);
        assert!(
            refused.is_error && refused.content.contains("heavy"),
            "{}",
            refused.content
        );
        let alone = worn("heavy", false);
        assert!(
            !alone.is_error && alone.content.contains(&"x".repeat(100)),
            "solo has no one to hand it to"
        );
        let cramped = run(&w, "rimuru", r#"{"mind":"heavy"}"#, false, 3_000);
        assert!(
            cramped.is_error && cramped.content.contains("half the context"),
            "{}",
            cramped.content
        );
        let unknown = worn("nope", false);
        assert!(
            unknown.is_error && unknown.content.contains("heavy, light"),
            "{}",
            unknown.content
        );
        let _ = std::fs::remove_dir_all(&w);
    }
}
