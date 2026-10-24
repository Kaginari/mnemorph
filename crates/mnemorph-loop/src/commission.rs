//! A commission: one creature's work, handed down. The dispatcher's `commission` call is run by
//! the loop: the law is asked, a child body runs with a fresh context over its own soil under
//! its own end gate, what it changed off its ground is put back, and its report comes up as the
//! dispatcher's tool result.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_ledger::{Journal, Prices, UsageRow};
use mnemorph_provider::Caller;
use mnemorph_races::ToolClass;
use mnemorph_tools::{Context, Outcome, Tool, ignored, registry};
use mnemorph_wire::{DEFAULT_CAP, Report};
use mnemorph_world::{Ask, Hop, Owner, Roster, Rule, dispatch, explain, soil};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::gate::{Author, EndGate, changed, snapshot};
use crate::model::Model;
use crate::run::{Body, Ending, Limits, Run, Setup};

/// The tool's name.
pub const COMMISSION: &str = "commission";

/// The `commission` tool as the model sees it. The loop runs the call; this body only describes
/// it, and answers a body that may not commission.
pub struct CommissionTool;

impl Tool for CommissionTool {
    fn name(&self) -> &'static str {
        COMMISSION
    }

    fn description(&self) -> &'static str {
        "Commission the creature one rank below you on the work in `ask`: it runs with a fresh context over its own soil, under its own end gate, and its report comes back as this call's result. A refusal names the creature the order goes to instead. `budget_wt` carves its slice of your envelope (default: half of what is left)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "to": {"type": "string", "description": "The creature, by name (`slime-wire`)."},
                "ask": {"type": "string", "description": "The order: what is wanted, in a few lines."},
                "budget_wt": {"type": "integer", "minimum": 1, "description": "Its slice, in weighted tokens."}
            },
            "required": ["to", "ask"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Dispatch
    }

    fn example(&self) -> Value {
        json!({"to": "slime-wire", "ask": "read @W lines into Report.Wrote, with a test"})
    }

    fn run(&self, _: Value, _: &mut Context) -> Outcome {
        Outcome::error("this body cannot commission: no commissions run under it")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommissionArgs {
    to: String,
    ask: String,
    #[serde(default)]
    budget_wt: Option<u64>,
}

/// What every commission of one session shares: the world, the model, and where spend and beats
/// are written.
pub struct Commissioner {
    /// The world.
    pub world: PathBuf,
    /// The model that rides every commissioned body.
    pub model: Arc<dyn Model>,
    /// Prices for the ledger.
    pub prices: Prices,
    /// The usage ledger.
    pub ledger: Option<Journal<UsageRow>>,
    /// Where each child's beat journal goes (`.mnemorph/instruments/loop/`).
    pub journals: PathBuf,
    /// The session.
    pub session: String,
}

/// What a commission came to: the dispatcher's tool result, the weighted tokens the child spent,
/// and the paths it landed, which the dispatcher's gate does not judge again.
pub struct Order {
    /// The tool result.
    pub outcome: Outcome,
    /// What the child spent.
    pub spent: u64,
    /// What the child landed on its own soil.
    pub landed: Vec<PathBuf>,
}

/// The creatures an Elf's open plan items name: `- [ ] <creature>: …` lines of the `plan.md`
/// in its seat.
#[must_use]
pub fn plan_items(world: &Path, roster: &Roster, name: &str) -> Vec<String> {
    let Some(c) = roster.creatures.get(name) else {
        return Vec::new();
    };
    std::fs::read_to_string(world.join(&c.seat).join("plan.md"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- [ ] "))
        .filter_map(|rest| rest.split([':', ' ']).next())
        .map(|n| n.trim_matches('`').to_owned())
        .filter(|n| !n.is_empty())
        .collect()
}

/// A commissioned body's system prompt: who it is, its doc, its soil, and how it answers.
fn prompt(world: &Path, roster: &Roster, name: &str, parent: &str) -> String {
    let c = &roster.creatures[name];
    let doc = std::fs::read_to_string(world.join(c.doc())).unwrap_or_default();
    let ground = soil(roster, name)
        .map(|s| s.write.join(", "))
        .unwrap_or_default();
    format!(
        "You are {name}, a {race} of Mnemorph, commissioned by {parent}. You work only on your own \
soil: {ground}. A refusal names its reason and the lawful route; never work around it.\n\n\
Your doc:\n{doc}\n\
When the work is done, answer without calling a tool, on the wire, one tag per line:\n\
@S DONE (or FAIL, or ESCALATE, with a reason)\n\
@W <path> for each path you wrote\n\
@F <file:line> <claim> for each finding\n\
@U territory <path> <fact> for what you learned that is written nowhere\n\
@E",
        race = c.race.name(),
    )
}

/// Run one commission: the law, the child body, the repair of what it changed off its ground,
/// and its report.
pub(crate) async fn order(
    c: &Arc<Commissioner>,
    parent: &Body,
    left: Option<u64>,
    cx: &Context,
    input: &str,
) -> Order {
    let refused = |text: String| Order {
        outcome: Outcome::error(text),
        spent: 0,
        landed: Vec::new(),
    };
    let a: CommissionArgs = match serde_json::from_str(input) {
        Ok(a) => a,
        Err(e) => return refused(format!("commission: {e}")),
    };
    let roster = Roster::load(&c.world);
    let plan = plan_items(&c.world, &roster, &parent.name);
    let slice = a.budget_wt.or(left.map(|l| l / 2)).unwrap_or(u64::MAX);
    let ask = Ask {
        plan: &plan,
        slice_wt: slice,
        left_wt: left.unwrap_or(u64::MAX),
    };
    match dispatch(&roster, &parent.name, &a.to, &ask) {
        Err(why) => return refused(why.to_string()),
        Ok(Hop::Via { elf, target }) => {
            return refused(format!(
                "{target} is in {elf}'s lane: commission {elf} with a plan item naming {target}"
            ));
        }
        Ok(Hop::Direct(_)) => {}
    }
    let Some(child_cx) = context_for(&c.world, &roster, &a.to) else {
        return refused(format!("`{}` is no living creature", a.to));
    };
    let mut child = child_run(c, &roster, &a.to, parent, slice, child_cx.inherit(cx));
    // What the world held before the child, to put back what it changes off its ground.
    let held = cx
        .attic()
        .and_then(|attic| attic.snapshot("before a commission").ok());
    let before = snapshot(&c.world);
    let ending = Box::pin(child.ask(&a.ask, &mut |_| {})).await;
    let spent = child.spent_wt();
    // What its own commissions landed was judged by their gates: it is the child's court work.
    let courts = child.courts();
    let (mut landed, put_back) = repair(c, cx, &roster, &a.to, &before, held.as_ref(), &courts);
    landed.extend(courts);
    landed.sort();
    landed.dedup();
    let report = report_of(&ending, &landed, &put_back, &a.to);
    Order {
        outcome: Outcome::ok(report, ToolClass::Dispatch),
        spent,
        landed: landed.into_iter().collect(),
    }
}

/// The child body for `to`: its prompt, its tools over its own soil, its slice as its budget,
/// its gate, and the commission tool when its race dispatches.
fn child_run(
    c: &Arc<Commissioner>,
    roster: &Roster,
    to: &str,
    parent: &Body,
    slice: u64,
    cx: Context,
) -> Run {
    let a_to = to.to_owned();
    let creature = &roster.creatures[&a_to];
    let run_id = format!("{}-{}", a_to, stamp());
    let mut limits = Limits::for_race(creature.race);
    limits.budget_wt = (slice != u64::MAX).then_some(slice);
    let body = Body {
        name: a_to.clone(),
        race: creature.race,
        model: parent.model.clone(),
        system: prompt(&c.world, roster, &a_to, &parent.name),
    };
    // A creature that may dispatch is handed the same commissioner: the order goes on down.
    let dispatches = creature.race.shelf().contains(&ToolClass::Dispatch);
    let mut tools = registry();
    if dispatches {
        tools.push(Box::new(CommissionTool));
    }
    let setup = Setup {
        model: Arc::clone(&c.model),
        tools,
        cx,
        journal: Journal::new(c.journals.join(format!("{run_id}.jsonl"))),
        ledger: c.ledger.clone(),
        prices: c.prices.clone(),
        caller: Caller {
            session: c.session.clone(),
            run: run_id,
            body: a_to.clone(),
            race: creature.race.name().into(),
            commission: Some(parent.name.clone()),
            prefix: 0,
        },
    };
    let gate = EndGate {
        verify: creature.verify.clone(),
        author: Some(Author {
            name: a_to.clone(),
            roster: roster.clone(),
        }),
        courts: Vec::new(),
    };
    let mut child = Run::new(body, limits, setup).with_gate(gate);
    if dispatches {
        child = child.with_commissioner(Arc::clone(c));
    }
    child
}

/// The child's report as its dispatcher reads it: its own wire answer, or its ending in words;
/// its written paths as the world shows them; what was put back.
fn report_of(ending: &Ending, landed: &[PathBuf], put_back: &[String], to: &str) -> String {
    let mut report = match ending {
        Ending::Answered(text) => Report::parse(text).unwrap_or_else(|| Report {
            status: "DONE".into(),
            other: text.lines().map(str::to_owned).collect(),
            ..Report::default()
        }),
        other => Report {
            status: match other {
                Ending::Checkpoint { why, .. } => format!("CHECKPOINT {why}"),
                Ending::Escalate(why) => format!("ESCALATE {why}"),
                Ending::Failed(why) => format!("FAIL {why}"),
                _ => "FAIL interrupted".into(),
            },
            ..Report::default()
        },
    };
    // What it wrote is what the world shows, not what it says.
    report.written = landed
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if !put_back.is_empty() {
        report.other.push(format!(
            "[put back, off {}'s ground: {}]",
            to,
            put_back.join(", ")
        ));
    }
    report.emit(DEFAULT_CAP, None)
}

/// Split what the child changed: its own landing, and what lay off its ground — put back from
/// the attic, or removed when it was new.
fn repair(
    c: &Commissioner,
    cx: &Context,
    roster: &Roster,
    name: &str,
    before: &crate::gate::Tree,
    held: Option<&mnemorph_tools::Snapshot>,
    courts: &[PathBuf],
) -> (Vec<PathBuf>, Vec<String>) {
    let me = Owner::Creature(name.to_owned());
    let mut landed = Vec::new();
    let mut put_back = Vec::new();
    for rel in changed(before, &c.world) {
        if courts.contains(&rel) {
            continue;
        }
        let shown = rel.to_string_lossy().into_owned();
        let e = explain(roster, &shown);
        if e.owner == me && e.rule != Rule::Domain {
            landed.push(rel);
            continue;
        }
        let restored = if before.has(&rel) {
            match (cx.attic(), held) {
                (Some(attic), Some(snap)) if attic.holds(snap, &shown) => {
                    attic.restore(snap, &[shown.as_str()]).is_ok()
                }
                _ => false,
            }
        } else {
            let path = c.world.join(&rel);
            path.symlink_metadata().is_ok_and(|m| !m.is_dir())
                && std::fs::remove_file(&path).is_ok()
        };
        put_back.push(if restored {
            shown
        } else {
            format!("{shown} (could not be put back)")
        });
    }
    (landed, put_back)
}

/// A run id's suffix: unique within the session.
fn stamp() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{nanos:x}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// The tools' context for the creature `name`: its race's shelf, the soil `own` yields for it —
/// claims, the claims inside them it does not hold, the root files if they are its own — and the
/// build output it may write.
/// `None` when it is not a living creature.
#[must_use]
pub fn context_for(world: &Path, roster: &Roster, name: &str) -> Option<Context> {
    let creature = roster.creatures.get(name).filter(|c| !c.retired)?;
    let s = soil(roster, name)?;
    let none: [&str; 0] = [];
    // Build output is what git ignores where `own` names this creature or nobody: never another's
    // zone, the records, or a soil compiled for someone else.
    let build: Vec<_> = ignored(world)
        .into_iter()
        .filter(|rel| {
            // A directory is asked of as one, with its `/`.
            let mut path = rel.to_string_lossy().into_owned();
            if world.join(rel).is_dir() {
                path.push('/');
            }
            let o = explain(roster, &path);
            [o].iter().all(|o| match &o.owner {
                Owner::Creature(c) => c == name && o.rule != Rule::Domain,
                Owner::Unowned => o.rule == Rule::Unclaimed,
                Owner::Harness | Owner::Human => false,
            })
        })
        .collect();
    Some(
        Context::new(creature.race, world, &none)
            .with_soil(&s.write, &s.except, s.root_files)
            .with_build(&build),
    )
}
