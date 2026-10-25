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

/// What every commission of one session shares: the world, the model, where spend and beats are
/// written, the human's caps and stress line, and the seats that hold a live court.
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
    /// The human's caps, read before every call of every body.
    pub caps: Option<mnemorph_ledger::Caps>,
    /// The human's stress line, when the configuration sets one.
    pub stress_tokens: Option<u64>,
    /// Seats with a live court: one court per seat.
    pub busy: std::sync::Mutex<std::collections::BTreeSet<String>>,
    /// Who watches the session: every commissioned body's beats reach it, labelled.
    pub observer: Option<crate::run::Observer>,
}

/// What a commission came to: the dispatcher's tool result, the weighted tokens the child spent,
/// what it landed and what was put back — both judged, so the dispatcher's baseline takes them —
/// and whether the human's Esc stopped it.
pub struct Order {
    /// The tool result.
    pub outcome: Outcome,
    /// What the child spent.
    pub spent: u64,
    /// What the child and its own commissions landed, when it answered.
    pub landed: Vec<PathBuf>,
    /// What was put back after it — only what really was.
    pub put_back: Vec<PathBuf>,
    /// The world when the commission began: what the dispatcher's own changes are told apart by.
    pub at: crate::gate::Tree,
    /// The human's Esc stopped it.
    pub interrupted: bool,
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
    interrupt: &Arc<crate::run::Interrupt>,
    input: &str,
) -> Order {
    let refused = |text: String| Order {
        outcome: Outcome::error(text),
        spent: 0,
        landed: Vec::new(),
        put_back: Vec::new(),
        at: crate::gate::Tree::default(),
        interrupted: false,
    };
    let a: CommissionArgs = match serde_json::from_str(input) {
        Ok(a) => a,
        Err(e) => return refused(format!("commission: {e}")),
    };
    let roster = Roster::load(&c.world);
    let plan = plan_items(&c.world, &roster, &parent.name);
    let slice = a.budget_wt.or(left.map(|l| l / 2)).unwrap_or(u64::MAX);
    if slice == 0 {
        return refused(format!(
            "no budget left to carve for {}: @? budget to your commander",
            a.to
        ));
    }
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
    // One live court per seat: a body cannot commission itself, nor a seat already at work.
    let seat = (a.to != parent.name)
        .then(|| Seat::claim(c, &a.to))
        .flatten();
    if seat.is_none() {
        return refused(format!(
            "{} holds a live court already; one court per seat — ask again when it has reported",
            a.to
        ));
    }
    let Some(child_cx) = context_for(&c.world, &roster, &a.to) else {
        return refused(format!("`{}` is no living creature", a.to));
    };
    let mut child = child_run(
        c,
        &roster,
        &a.to,
        parent,
        slice,
        child_cx.inherit(cx).with_body(&a.to),
    )
    .with_interrupt(Arc::clone(interrupt));
    // What the world held before the child, to put back what it changes off its ground.
    let held = cx
        .attic()
        .and_then(|attic| attic.snapshot("before a commission").ok());
    let before = snapshot(&c.world);
    let ending = Box::pin(child.ask(&a.ask, &mut |_| {})).await;
    drop(seat);
    let spent = child.spent_wt();
    // Court work counts only when the child answered: a failed landing is put back whole.
    let answered = matches!(ending, Ending::Answered(_));
    let courts = if answered { child.courts() } else { Vec::new() };
    let (mut landed, put_back) = repair(
        c,
        cx,
        &roster,
        &a.to,
        &before,
        held.as_ref(),
        &courts,
        answered,
    );
    landed.extend(courts);
    landed.sort();
    landed.dedup();
    file_facts(&c.world, &ending, &a.to);
    let report = report_of(&ending, &landed, &put_back);
    Order {
        outcome: Outcome::ok(report, ToolClass::Dispatch),
        spent,
        landed,
        // A put-back that failed stays out of the dispatcher's baseline: its gate judges it.
        put_back: put_back
            .into_iter()
            .filter(|(_, ok)| *ok)
            .map(|(p, _)| p)
            .collect(),
        at: before,
        interrupted: matches!(ending, Ending::Interrupted),
    }
}

/// What the child learned about its territory, said in its `@U territory` lines, filed as facts
/// in its name: a support, or a counter naming a fact's id.
fn file_facts(world: &Path, ending: &Ending, by: &str) {
    let Ending::Answered(text) = ending else {
        return;
    };
    let Some(report) = Report::parse(text) else {
        return;
    };
    for u in &report.unsaid {
        if u.kind == Some(mnemorph_wire::UnsaidKind::Territory)
            && let Some(event) = mnemorph_world::facts::from_unsaid(&u.text, by, None)
        {
            // A fact that cannot be filed is lost to the ontology, never to the report — and said.
            if let Err(e) = mnemorph_world::Facts::record(world, &event) {
                eprintln!("mnemorph: {by}'s fact could not be filed: {e}");
            }
        }
    }
}

/// A seat holding a live court: freed when dropped, on every path — a refusal, an answer, a
/// panic or a dropped future.
struct Seat {
    commissioner: Arc<Commissioner>,
    name: String,
}

impl Seat {
    /// Take `name` for a court; `None` when it holds one already.
    fn claim(c: &Arc<Commissioner>, name: &str) -> Option<Seat> {
        let mut busy = c
            .busy
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        busy.insert(name.to_owned()).then(|| Seat {
            commissioner: Arc::clone(c),
            name: name.to_owned(),
        })
    }
}

impl Drop for Seat {
    fn drop(&mut self) {
        let mut busy = self
            .commissioner
            .busy
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        busy.remove(&self.name);
    }
}

/// The child body for `to`: its prompt, its tools over its own soil, its slice as its budget,
/// the human's caps and stress line, its gate, and the commission tool when its race dispatches.
fn child_run(
    c: &Arc<Commissioner>,
    roster: &Roster,
    to: &str,
    parent: &Body,
    slice: u64,
    cx: Context,
) -> Run {
    let name = to.to_owned();
    let creature = &roster.creatures[&name];
    let run_id = format!("{name}-{}", stamp());
    let mut limits = Limits::for_race(creature.race);
    limits.budget_wt = (slice != u64::MAX).then_some(slice);
    if let Some(stress) = c.stress_tokens {
        limits.stress_tokens = stress;
    }
    let body = Body {
        name: name.clone(),
        race: creature.race,
        model: parent.model.clone(),
        system: prompt(&c.world, roster, &name, &parent.name),
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
            body: name.clone(),
            race: creature.race.name().into(),
            commission: Some(parent.name.clone()),
            prefix: 0,
        },
    };
    let gate = EndGate {
        verify: creature.verify.clone(),
        author: Some(Author {
            name: name.clone(),
            roster: roster.clone(),
        }),
    };
    let run_name = setup.caller.run.clone();
    let mut child = Run::new(body, limits, setup).with_gate(gate);
    // What the child sees reaches the session's watcher, under the child's own name.
    if let Some(watch) = c.observer.clone() {
        let body_name = name.clone();
        child = child.with_observer(Arc::new(move |seen: &crate::run::Seen| {
            watch(&crate::run::Seen::From {
                body: body_name.clone(),
                run: run_name.clone(),
                seen: Box::new(seen.clone()),
            });
        }));
    }
    if let Some(caps) = c.caps {
        child = child.with_caps(caps);
    }
    if dispatches {
        child = child.with_commissioner(Arc::clone(c));
    }
    child
}

/// The child's report as its dispatcher reads it: its own wire answer — an answer off the wire is
/// a FAIL, never a DONE — or its ending in words; its written paths as the world shows them; what
/// was put back after it.
fn report_of(ending: &Ending, landed: &[PathBuf], put_back: &[(PathBuf, bool)]) -> String {
    let mut report = match ending {
        Ending::Answered(text) => Report::parse(text).unwrap_or_else(|| Report {
            status: "FAIL off the wire: the answer opened with no @S".into(),
            other: text.lines().map(str::to_owned).collect(),
            ..Report::default()
        }),
        other => Report {
            status: match other {
                Ending::Checkpoint { why, .. } => format!("CHECKPOINT {why}"),
                Ending::Escalate(why) => format!("ESCALATE {why}"),
                Ending::Failed(why) => format!("FAIL {why}"),
                _ => "INTERRUPTED by the human".into(),
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
        let shown: Vec<String> = put_back
            .iter()
            .map(|(p, ok)| {
                let p = p.to_string_lossy();
                if *ok {
                    p.into_owned()
                } else {
                    format!("{p} (could not be put back)")
                }
            })
            .collect();
        report.other.push(format!(
            "[put back after the commission: {}]",
            shown.join(", ")
        ));
    }
    report.emit(DEFAULT_CAP, None)
}

/// Split what the child changed: its landing (and its commissions'), kept only when it answered;
/// and everything else, put back from the attic, or removed when it was new.
#[allow(clippy::too_many_arguments)]
fn repair(
    c: &Commissioner,
    cx: &Context,
    roster: &Roster,
    name: &str,
    before: &crate::gate::Tree,
    held: Option<&mnemorph_tools::Snapshot>,
    courts: &[PathBuf],
    answered: bool,
) -> (Vec<PathBuf>, Vec<(PathBuf, bool)>) {
    let me = Owner::Creature(name.to_owned());
    let mut landed = Vec::new();
    let mut put_back = Vec::new();
    for rel in changed(before, &c.world) {
        if courts.contains(&rel) {
            continue;
        }
        let shown = rel.to_string_lossy().into_owned();
        let e = explain(roster, &shown);
        if answered && e.owner == me && e.rule != Rule::Domain {
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
        put_back.push((rel, restored));
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
