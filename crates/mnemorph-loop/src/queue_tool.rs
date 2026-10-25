//! The `queue` tool: a body reads the shared work queue for free and moves what it holds or
//! stands over. It never claims and never adds: work reaches a body claimed in its name by the
//! wake, and its own orders go down as `@$` lines in its report.

use std::path::Path;

use mnemorph_races::ToolClass;
use mnemorph_tools::{Context, Outcome, Tool};
use mnemorph_world::queue::{Item, Move};
use mnemorph_world::{Queue, Roster};
use serde::Deserialize;
use serde_json::{Value, json};

/// The tool's name.
pub const QUEUE: &str = "queue";

/// The `queue` tool as the model sees it. The loop runs the call; this body only describes it.
pub struct QueueTool;

impl Tool for QueueTool {
    fn name(&self) -> &'static str {
        QUEUE
    }

    fn description(&self) -> &'static str {
        "Read the shared work queue (`list`, `show`), or move an order you hold or stand over: `touch` adds a note, `requeue` puts a claimed order back on hold, `close` marks it done, `drop` drops it. You never claim or add here: work reaches you claimed in your name, and your own orders go down as `@$` lines in your report."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["list", "show", "touch", "requeue", "close", "drop"]},
                "id": {"type": "string", "description": "The order's id, for every action but `list`."},
                "note": {"type": "string", "description": "The note, for `touch`."}
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({"action": "touch", "id": "3fa94c1e0b2d", "note": "blocked on the parser fix"})
    }

    fn run(&self, _: Value, _: &mut Context) -> Outcome {
        Outcome::error("no shared work queue runs under this body")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    action: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

/// One order, on one line.
fn line(i: &Item) -> String {
    format!(
        "{} {} {} rank {}: {}",
        i.id,
        i.state.word(),
        i.holder(),
        i.rank,
        i.ask
    )
}

/// Run one `queue` call for `body`: reading is free, a move follows the rank law.
pub(crate) fn run(q: &Queue, world: &Path, body: &str, input: &str) -> Outcome {
    let a: Args = match serde_json::from_str(input) {
        Ok(a) => a,
        Err(e) => return Outcome::error(format!("queue: {e}")),
    };
    let ok = |text: String| Outcome::ok(text, ToolClass::Read);
    if a.action == "list" {
        let open: Vec<String> = q.open().iter().map(line).collect();
        return ok(if open.is_empty() {
            "the queue is empty".into()
        } else {
            open.join("\n")
        });
    }
    let Some(id) = a.id else {
        return Outcome::error(format!("queue {}: which order? give its `id`", a.action));
    };
    let mv = match a.action.as_str() {
        "show" => {
            return match q.items().remove(&id) {
                Some(i) => {
                    let notes: Vec<String> = i.notes.iter().map(|n| format!("  {n}")).collect();
                    let mut lines = vec![line(&i), format!("ordered by {}", i.by)];
                    lines.extend(notes);
                    let text = lines.join("\n");
                    ok(text)
                }
                None => Outcome::error(format!("no order `{id}` on the queue")),
            };
        }
        "touch" => match a.note.filter(|n| !n.trim().is_empty()) {
            Some(note) => Move::Touch { note },
            None => return Outcome::error("queue touch: give the `note`"),
        },
        "requeue" => Move::Requeue,
        "close" => Move::Close { drop: false },
        "drop" => Move::Close { drop: true },
        other => return Outcome::error(format!("queue has no `{other}`")),
    };
    match q.move_item(&Roster::load(world), body, &id, &mv) {
        Ok(i) => ok(line(&i)),
        Err(e) => Outcome::error(format!("refused: {e}")),
    }
}
