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
        "Commission the creature one rank below you on the work in `ask`: it runs with a fresh context over its own soil, under its own end gate, and its report comes back as this call's result. A refusal names the creature the order goes to instead. `budget_wt` is what you give it out of your own envelope (default: half of what is left; with no cap set there is no envelope, and it is not read); the harness grants at most your reserve-kept share for its race, scaled by its worth, and the report says when it granted less. `topup_wt` sets aside more from your envelope, given to it once if its slice runs out."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "to": {"type": "string", "description": "The creature, by name (`slime-wire`)."},
                "ask": {"type": "string", "description": "The order: what is wanted, in a few lines."},
                "budget_wt": {"type": "integer", "minimum": 1, "description": "Its slice, in weighted tokens."},
                "topup_wt": {"type": "integer", "minimum": 1, "description": "Set aside from your own envelope: given once, if its slice runs out before it is done."},
                "office": {"type": "string", "enum": ["findings", "verdict", "draft"], "description": "What the work is: findings (read wide, thin facts), verdict (judge, with evidence) or draft (write). It chooses the court's model."}
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
    #[serde(default)]
    topup_wt: Option<u64>,
    #[serde(default)]
    office: Option<String>,
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
    /// The council's keys.
    pub council: crate::council::Thresholds,
    /// What chooses each court's model; `None` rides the dispatcher's.
    pub router: Option<Router>,
    /// The least slice a commission carries ([`SLICE_FLOOR`] in a session).
    pub slice_floor: u64,
    /// The shared work queue a report's `@$` orders are held on; `None` holds none.
    pub queue: Option<mnemorph_world::Queue>,
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
    // A plan that is a link is no plan: the law reads only what stands in the seat.
    let plan = world.join(&c.seat).join("plan.md");
    if std::fs::symlink_metadata(&plan).is_ok_and(|m| m.file_type().is_symlink()) {
        return Vec::new();
    }
    std::fs::read_to_string(plan)
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
{card}\n\n\
Your doc:\n{doc}\n\
When the work is done, answer without calling a tool, on the wire, one tag per line:\n\
@S DONE (or FAIL, or ESCALATE, with a reason)\n\
@W <path> for each path you wrote\n\
@F <file:line> <claim> for each finding\n\
@U territory <path> <fact> for what you learned that is written nowhere\n\
@E",
        race = c.race.name(),
        card = race_card(c.race),
    ) + &cards_below(world, roster, name)
}

/// What a race does, in the words its body reads first: who writes, who hands work down.
fn race_card(race: mnemorph_races::Race) -> &'static str {
    use mnemorph_races::Race;
    match race.base() {
        Race::Elf => {
            "As an Elf you plan and route across your lane; you write no code. First write the plan \
item in your own seat's plan.md with the write or edit tool (a shell command's write there is put \
back) — a line `- [ ] <orc>: <what it does>` — then hand the work down \
with the commission tool to that Orc, read its report, and answer with yours."
        }
        Race::Orc => {
            "As an Orc you own a domain: you write no code. First code the Slime's verify line with \
the verify tool — one command that passes once the work is done — then hand the writing with the \
commission tool to the Slime whose zone holds the files, judge its report against the ask, and \
answer with yours."
        }
        Race::Slime => "As a Slime you change the code of your one zone, check it, and answer.",
        Race::Kijin => {
            "As a Kijin you do the outward acts of your post — what leaves the machine — and answer."
        }
        Race::DarkElf => {
            "As a Dark Elf you challenge what landed: find what is wrong, with evidence."
        }
        // The throne is never commissioned; ascended forms read their base race's card.
        Race::Rimuru
        | Race::HighElf
        | Race::HighOrc
        | Race::NamedSlime
        | Race::Oni
        | Race::ArchDarkElf => "",
    }
}

/// The cards of the bodies reporting to `name`, for it to read before it funds one; empty when
/// none reports to it.
pub(crate) fn cards_below(world: &Path, roster: &Roster, name: &str) -> String {
    let records = mnemorph_world::worth::Records::load(world);
    let lines: Vec<String> = roster
        .living()
        .filter(|c| c.reports_to.as_deref() == Some(name))
        .map(|c| format!("- {}", records.card(roster, &c.name).line()))
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "\n\nWhat you give a body comes out of your own budget, and what it spends counts as yours: \
read its card before you fund it, and set `budget_wt` by what it is worth to the work. The harness \
grants at most your reserve-kept share for its race, scaled by its worth. The cards:\n{}",
        lines.join("\n")
    )
}

/// The slice a commission carries, the top-up set aside for it, and a line for the dispatcher
/// when either is less than it asked. The grant is the dispatcher's judgment, bounded: its
/// reserve kept, the child's race share, the child's worth from its card; a top-up is the
/// dispatcher's own further trust, bounded by what it may give at all and by twice the ceiling.
fn grant(
    records: &mnemorph_world::worth::Records,
    roster: &Roster,
    a: &CommissionArgs,
    left: Option<u64>,
    floor: u64,
) -> (u64, Option<u64>, Option<String>) {
    // An ask of nothing is refused, cap or none.
    if a.budget_wt == Some(0) {
        return (0, None, None);
    }
    // With no cap set there is no envelope to carve: the slice is unbounded, whatever was asked.
    let Some(left) = left else {
        return (u64::MAX, None, None);
    };
    // What the dispatcher may give at all: its envelope, less a tenth kept.
    let givable_all = left / 10 * 9;
    let asked = a.budget_wt.unwrap_or(left / 2);
    let ceiling = roster.creatures.get(&a.to).map(|child| {
        mnemorph_world::worth::ceiling(left, child.race, &records.card(roster, &a.to))
    });
    // A slice too small for one call is no grant: it is raised to the floor, within what the
    // dispatcher may give at all.
    let least = floor.min(givable_all);
    let slice = ceiling.map_or(asked, |cap| asked.min(cap)).max(least);
    // A top-up is the dispatcher's further trust: slice and top-up together stay within what it
    // may give at all, and within twice the child's ceiling — worth still counts.
    let most = ceiling.map_or(givable_all, |cap| givable_all.min(cap.saturating_mul(2)));
    let topup = a
        .topup_wt
        .map(|t| t.min(most.saturating_sub(slice)))
        .filter(|t| *t > 0);
    let mut said = Vec::new();
    if slice > asked {
        let why = if least < floor {
            "a slice is at least one call's worth, held within 90% of what you had left"
        } else {
            "a slice is at least one call's worth"
        };
        said.push(format!(
            "[granted {slice} wt, not the {asked} asked: {why}]"
        ));
    }
    if slice < asked {
        said.push(format!(
            "[granted {slice} wt of the {asked} asked: the ceiling for {} out of what you had left]",
            a.to
        ));
    }
    if let Some(wanted) = a.topup_wt.filter(|w| topup.is_none_or(|t| t < *w)) {
        said.push(format!(
            "[set aside {} wt of the {wanted} top-up asked: what you could still give]",
            topup.unwrap_or(0)
        ));
    }
    (slice, topup, (!said.is_empty()).then(|| said.join("\n")))
}

/// The law on one commission: the dispatch rules, and no order that names again the files its
/// body failed on three runs in a row.
fn lawful(
    records: &mnemorph_world::worth::Records,
    roster: &Roster,
    parent: &str,
    a: &CommissionArgs,
    plan: &[String],
    slice: u64,
    left: Option<u64>,
) -> Result<(), String> {
    let ask = Ask {
        plan,
        slice_wt: slice,
        left_wt: left.unwrap_or(u64::MAX),
    };
    match dispatch(roster, parent, &a.to, &ask) {
        Err(why) => return Err(why.to_string()),
        Ok(Hop::Via { elf, target }) => {
            return Err(format!(
                "{target} is in {elf}'s lane: commission {elf} with a plan item naming {target}"
            ));
        }
        Ok(Hop::Direct(_)) => {}
    }
    // Three failed runs in a row on the same files harden into the lawful next acts: an order
    // that names them again is refused.
    let stuck = records.stuck(&a.to);
    if let Some(path) = stuck
        .iter()
        .find(|p| mnemorph_world::worth::names(&a.ask, p))
    {
        return Err(format!(
            "{} failed its last {} runs on `{path}`: rewrite the order without it, split the work to another body, or escalate with @? to your commander",
            a.to,
            mnemorph_world::worth::STUCK_AFTER
        ));
    }
    Ok(())
}

/// The least slice a commission carries, in weighted tokens: about one model call.
pub const SLICE_FLOOR: u64 = 20_000;

/// What chooses a court's model: the human's pins and tiers, and Metis when routing is on. A
/// model on another provider than the session's cannot ride this session's client: the
/// dispatcher's model rides instead, and the report says so.
pub struct Router {
    /// The configuration: pins, tiers, bands.
    pub config: Arc<mnemorph_config::Config>,
    /// Metis's decider, when routing is on.
    pub decisions: Option<crate::decisions::Decisions>,
    /// The session's provider.
    pub provider: String,
}

/// The office an `office` argument names, or why it names none.
fn office_of(word: &str) -> Result<mnemorph_wire::Office, String> {
    [
        mnemorph_wire::Office::Findings,
        mnemorph_wire::Office::Verdict,
        mnemorph_wire::Office::Draft,
    ]
    .into_iter()
    .find(|o| o.word() == word)
    .ok_or_else(|| format!("commission: the offices are findings, verdict and draft, not `{word}`"))
}

/// The model a court rides, a line for the report, and what Metis's decision cost — booked
/// against the commission, as a bench read is against its run.
async fn route_child(
    c: &Commissioner,
    roster: &Roster,
    a: &CommissionArgs,
    office: Option<mnemorph_wire::Office>,
    parent: &Body,
    interrupt: &crate::run::Interrupt,
) -> (String, Option<String>, u64) {
    let (Some(chooser), Some(child)) = (&c.router, roster.creatures.get(&a.to)) else {
        return (parent.model.clone(), None, 0);
    };
    let caller = Caller {
        session: c.session.clone(),
        run: format!("{}-route", a.to),
        body: a.to.clone(),
        race: child.race.name().to_owned(),
        // The route is the dispatcher's act: its row is booked in the dispatcher's name.
        commission: Some(parent.name.clone()),
        prefix: 0,
    };
    let routed = crate::route::route(
        &chooser.config,
        chooser.decisions.as_ref(),
        &caller,
        crate::route::Work {
            race: child.race,
            office,
            commission: &a.ask,
            ceiling: Some(crate::route::tier_of(
                &chooser.config,
                parent.race,
                &parent.model,
            )),
        },
        Some(crate::route::Stop {
            interrupt,
            left: std::time::Duration::from_secs(60),
        }),
        crate::route::Meter {
            prices: &c.prices,
            ledger: c.ledger.as_ref(),
        },
    )
    .await;
    let cost = routed.decision.as_ref().map_or(0, |d| {
        crate::decisions::decision_usage(d, &caller, &c.prices).wt
    });
    let backend = match chooser.config.backend(&routed.model) {
        Ok(b) => b,
        Err(why) => {
            let note = format!(
                "[{} was routed to {}, which cannot ride: {why}; it rode {}]",
                a.to, routed.model, parent.model
            );
            return (parent.model.clone(), Some(note), cost);
        }
    };
    if backend.provider_name != chooser.provider {
        let note = format!(
            "[{} was routed to {} on {}, another provider than this session's: it rode {}]",
            a.to, routed.model, backend.provider_name, parent.model
        );
        return (parent.model.clone(), Some(note), cost);
    }
    // Nothing routed and nothing changed: nothing to say.
    let note = (backend.model != parent.model || routed.reason != crate::route::Reason::Off)
        .then(|| format!("[{} rode {} — {}]", a.to, backend.model, routed.reason));
    (backend.model, note, cost)
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
    let office = match a.office.as_deref().map(office_of).transpose() {
        Ok(office) => office,
        Err(why) => return refused(why),
    };
    let (model, rode, route_wt) = route_child(c, &roster, &a, office, parent, interrupt).await;
    // Routing comes first: what Metis costs comes out of what the dispatcher has left to give.
    let left = left.map(|l| l.saturating_sub(route_wt));
    let records = mnemorph_world::worth::Records::load(&c.world);
    let (slice, topup, granted) = grant(&records, &roster, &a, left, c.slice_floor);
    if slice == 0 {
        return refused(format!(
            "no budget left to carve for {}: @? budget to your commander",
            a.to
        ));
    }
    if let Err(why) = lawful(&records, &roster, &parent.name, &a, &plan, slice, left) {
        return refused(why);
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
        model,
    )
    .with_interrupt(Arc::clone(interrupt))
    .with_topup(topup);
    // What the world held before the child, to put back what it changes off its ground.
    let held = cx
        .attic()
        .and_then(|attic| attic.snapshot("before a commission").ok());
    let before = snapshot(&c.world);
    let ending = Box::pin(child.ask(&a.ask, &mut |_| {})).await;
    drop(seat);
    // Metis's decision is booked against the commission, as a bench read is against its run.
    let spent = child.spent_wt().saturating_add(route_wt);
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
    let failed = child.failed_acts();
    let notes: Vec<String> = granted.into_iter().chain(rode).collect();
    let report = report_for(
        c, &roster, &a.to, &ending, failed, notes, &landed, &put_back,
    );
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
/// File an answering child's `@U territory` facts in its name, and return the triage its parent
/// owes: for each new free fact that resembles older ones, a line naming them and how to label it.
fn file_facts(world: &Path, ending: &Ending, by: &str) -> Vec<String> {
    let Ending::Answered(text) = ending else {
        return Vec::new();
    };
    let Some(report) = Report::parse(text) else {
        return Vec::new();
    };
    let mut filed = Vec::new();
    for u in &report.unsaid {
        if u.kind == Some(mnemorph_wire::UnsaidKind::Territory)
            && let Some(event) = mnemorph_world::facts::from_unsaid(&u.text, by, None)
        {
            // A fact that cannot be filed is lost to the ontology, never to the report — and said.
            match mnemorph_world::Facts::record(world, &event) {
                Ok(()) => {
                    if let mnemorph_world::facts::Event::Support { id, .. } = event {
                        filed.push(id);
                    }
                }
                Err(e) => eprintln!("mnemorph: {by}'s fact could not be filed: {e}"),
            }
        }
    }
    let facts = mnemorph_world::Facts::load(world);
    filed
        .iter()
        .filter_map(|id| facts.facts.get(id))
        .filter(|f| f.keyed.is_none() && !f.labelled)
        .filter_map(|f| {
            let shown: Vec<String> = facts
                .candidates(f, mnemorph_world::facts::THETA_T)
                .iter()
                .map(|o| format!("#{} \"{}\"", o.id, quoted(&o.text)))
                .collect();
            (!shown.is_empty()).then(|| {
                format!(
                    "triage #{} \"{}\" on {}: it resembles {} — label it in your report: @V #{} support|counter|refines #<id>, or @V #{} distinct",
                    f.id,
                    quoted(&f.text),
                    f.about,
                    shown.join("; "),
                    f.id,
                    f.id
                )
            })
        })
        .collect()
}

/// A fact's text as a triage line quotes it: one short line, no quote or bracket of its own, so
/// a fact's words never pass for the line around them.
fn quoted(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| {
            if c.is_control() || "\"[]".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    match flat.char_indices().nth(TRIAGE_TEXT) {
        Some((at, _)) => format!("{}…", &flat[..at]),
        None => flat,
    }
}

/// The most of a fact's text a triage line quotes, in characters.
const TRIAGE_TEXT: usize = 200;

/// File a reporting body's triage labels: each `@V #<new> <label> [#<id>]` verdict on a fact
/// said only by bodies reporting to it, and not labelled yet — the first label stands. A label
/// on anyone else's fact is not this body's to give.
fn file_labels(world: &Path, roster: &Roster, ending: &Ending, by: &str) {
    let Ending::Answered(text) = ending else {
        return;
    };
    if let Some(report) = Report::parse(text) {
        file_verdicts(world, roster, &report.verdicts, by);
    }
}

/// File `by`'s triage labels from its `@V` verdicts, by the rules of [`file_labels`].
pub(crate) fn file_verdicts(world: &Path, roster: &Roster, verdicts: &[String], by: &str) {
    let facts = mnemorph_world::Facts::load(world);
    // The first label stands, in this report as across reports.
    let mut given = std::collections::BTreeSet::new();
    for event in verdicts
        .iter()
        .filter_map(|v| mnemorph_world::facts::label_of(v, by))
    {
        let mnemorph_world::facts::Event::Label { new, .. } = &event else {
            continue;
        };
        let its_own = facts.facts.get(new).is_some_and(|f| {
            !f.labelled
                && !f.supporters.is_empty()
                && f.supporters.iter().all(|s| {
                    roster
                        .creatures
                        .get(s)
                        .is_some_and(|c| c.reports_to.as_deref() == Some(by))
                })
        });
        if its_own
            && given.insert(new.clone())
            && let Err(e) = mnemorph_world::Facts::record(world, &event)
        {
            eprintln!("mnemorph: {by}'s triage could not be filed: {e}");
        }
    }
}

/// Who can carry out a held order, if anyone: the body it is for must be able to — the body it
/// reports to may lawfully commission it, and every path the order names lies on its ground.
/// `Ok` names the dispatcher; `Err` says why the order stays held.
pub(crate) fn carrier(
    world: &Path,
    roster: &Roster,
    item: &mnemorph_world::queue::Item,
    left: Option<u64>,
) -> Result<String, String> {
    let Some(to) = item.to.as_deref() else {
        return Err("the throne's work waits for the human's turn".into());
    };
    let Some(parent) = roster.chain(to).into_iter().next() else {
        return Err(format!("{to} reports to no one: the human dispatches it"));
    };
    let plan = plan_items(world, roster, &parent);
    let left_wt = left.unwrap_or(u64::MAX);
    let ask = Ask {
        plan: &plan,
        slice_wt: left.map_or(u64::MAX, |l| l / 2),
        left_wt,
    };
    match dispatch(roster, &parent, to, &ask) {
        Err(why) => return Err(why.to_string()),
        Ok(Hop::Via { elf, .. }) => {
            return Err(format!("{to} is in {elf}'s lane: {elf} dispatches it"));
        }
        Ok(Hop::Direct(_)) => {}
    }
    let stuck = mnemorph_world::worth::Records::load(world).stuck(to);
    if let Some(path) = stuck
        .iter()
        .find(|p| mnemorph_world::worth::names(&item.ask, p))
    {
        return Err(format!(
            "{to} failed its last {} runs on `{path}`: the order waits to be rewritten, split or escalated",
            mnemorph_world::worth::STUCK_AFTER
        ));
    }
    // Its ground is its own and that of every body below it: an Orc carries an order through
    // its Slimes.
    let below: Vec<String> = roster
        .living()
        .filter(|c| c.name == to || roster.chain(&c.name).iter().any(|n| n == to))
        .map(|c| c.name.clone())
        .collect();
    let grounds: Vec<_> = below.iter().filter_map(|n| soil(roster, n)).collect();
    for word in item.ask.split_whitespace() {
        let Some((path, inside)) = named_path(world, word) else {
            continue;
        };
        if !inside || !grounds.iter().any(|g| g.admits(&path)) {
            return Err(format!(
                "`{path}` is not on the ground of {to} or any body below it: it waits for a body that can carry it"
            ));
        }
    }
    Ok(parent)
}

/// The world path a word of an order names, when it names one that exists: quotes and
/// punctuation trimmed, a `:line` suffix dropped, an absolute path inside the world made
/// relative. A path that climbs (`..`) or leaves the world is named but on no ground: the flag
/// says whether it stays inside.
fn named_path(world: &Path, said: &str) -> Option<(String, bool)> {
    use std::path::Component;
    // Quotes and brackets go at both ends, punctuation only at the end: `./src` keeps its dot.
    let mut path = said
        .trim_start_matches(|c: char| "`'\"([".contains(c))
        .trim_end_matches(|c: char| "`'\",.;)]".contains(c));
    while let Some((head, tail)) = path.rsplit_once(':') {
        if tail.is_empty() || tail.bytes().all(|b| b.is_ascii_digit()) {
            path = head;
        } else {
            break;
        }
    }
    if path.is_empty() || !(path.contains('/') || path.contains('.')) {
        return None;
    }
    let p = Path::new(path);
    let rel = if p.is_absolute() {
        p.strip_prefix(world)
            .map_or_else(|_| p.to_path_buf(), Path::to_path_buf)
    } else {
        p.to_path_buf()
    };
    let climbs = rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir));
    // `./src/a.rs` is `src/a.rs`: the ground reads paths without their `.` steps.
    let rel: PathBuf = if climbs || rel.is_absolute() {
        rel
    } else {
        rel.components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .collect()
    };
    let shown = rel.display().to_string();
    if climbs || rel.is_absolute() {
        return (rel.is_absolute() || world.join(&rel).exists()).then_some((shown, false));
    }
    world.join(&rel).exists().then_some((shown, true))
}

/// Book what a woken order came to: `DONE` closes it, any other report leaves it claimed with the
/// exit noted, and a refused commission goes back on hold. The line says so, for the human and
/// the next turn.
pub(crate) fn settle(
    q: &mnemorph_world::Queue,
    roster: &Roster,
    item: &mnemorph_world::queue::Item,
    outcome: &Outcome,
) -> String {
    use mnemorph_world::queue::{HARNESS, Move};
    let to = item.holder();
    let note = |n: String| {
        // Said once: an order that keeps coming back does not grow a note every turn.
        if item.notes.last() != Some(&format!("{HARNESS}: {n}")) {
            let _ = q.move_item(roster, HARNESS, &item.id, &Move::Touch { note: n });
        }
    };
    if outcome.is_error {
        let _ = q.move_item(roster, HARNESS, &item.id, &Move::Requeue);
        note(outcome.content.clone());
        return format!(
            "queue {}: {to} was not commissioned — {}",
            item.id, outcome.content
        );
    }
    let status = Report::parse(&outcome.content)
        .map(|r| r.status)
        .unwrap_or_default();
    match status.split_whitespace().next() {
        Some("DONE") => {
            let _ = q.move_item(roster, HARNESS, &item.id, &Move::Close { drop: false });
        }
        // Stopped by the clock, the budget, the context or the human: nothing the body did wrong,
        // so the order waits again.
        Some("INTERRUPTED" | "CHECKPOINT") => {
            let _ = q.move_item(roster, HARNESS, &item.id, &Move::Requeue);
            note(format!("waits again: {status}"));
        }
        _ => note(format!("exit: {status}")),
    }
    // Said as the queue now reads, whatever the body did to the order meanwhile.
    let now = q.items().get(&item.id).map_or("gone", |i| i.state.word());
    format!(
        "queue {}: {to} reported {status}; the order is {now}",
        item.id
    )
}

/// What a child's run leaves, after it: its facts filed and its parent's triage owed, its own
/// triage labels, its `@$` orders held, the council sat — and the report its dispatcher reads.
#[allow(clippy::too_many_arguments)]
fn report_for(
    c: &Commissioner,
    roster: &Roster,
    to: &str,
    ending: &Ending,
    failed: u32,
    notes: Vec<String>,
    landed: &[PathBuf],
    put_back: &[(PathBuf, bool)],
) -> String {
    let mut notes = notes;
    // Triage is a hole the parent must settle: holes are never cut from a report.
    let mut triage = file_facts(&c.world, ending, to);
    file_labels(&c.world, roster, ending, to);
    if let Some(q) = &c.queue {
        hold_orders(q, &c.world, ending, to);
    }
    // A clock death's memo is a hole: never cut from the report.
    triage.extend(clock_memo(&c.world, roster, to, ending));
    let forced = council(c, to, ending, failed, &mut notes, &mut triage);
    report_of(ending, landed, put_back, &notes, &triage, forced.as_deref())
}

/// Where clock deaths are kept, under the world.
pub const DEATHS: &str = ".mnemorph/instruments/deaths.jsonl";

/// One clock death.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Death {
    /// When.
    pub ts: mnemorph_ledger::Timestamp,
    /// The body whose clock ran out.
    pub body: String,
    /// The instrument's words.
    pub why: String,
    /// The body named to the parent as the next fit, if any.
    pub next: Option<String>,
}

/// A clock death leaves a memo: a `death` row, and the next-fit body named to the parent — the
/// living sibling of the same base race under the same parent, not stuck, with the best worth.
/// Advice, never a change on its own.
fn clock_memo(world: &Path, roster: &Roster, to: &str, ending: &Ending) -> Option<String> {
    // The clock's own checkpoint — whether or not its salvage turn wrote anything.
    let Ending::Checkpoint { why, .. } = ending else {
        return None;
    };
    if !why.starts_with("clock") {
        return None;
    }
    let me = roster.creatures.get(to);
    let records = mnemorph_world::worth::Records::load(world);
    // The best worth wins; a tie goes to the sibling that spent least, then by name.
    let next = me.and_then(|me| {
        roster
            .living()
            .filter(|k| {
                k.name != to
                    && k.reports_to == me.reports_to
                    && k.race.base() == me.race.base()
                    && records.stuck(&k.name).is_empty()
            })
            .map(|k| {
                let card = records.card(roster, &k.name);
                (
                    std::cmp::Reverse(card.worth_milli()),
                    card.spent_wt,
                    k.name.clone(),
                )
            })
            .min()
    });
    let death = Death {
        ts: mnemorph_ledger::Timestamp::now(),
        body: to.to_owned(),
        why: why.clone(),
        next: next.as_ref().map(|(_, _, n)| n.clone()),
    };
    if let Err(e) = Journal::<Death>::new(world.join(DEATHS)).append(&death) {
        eprintln!("mnemorph: {to}'s clock death could not be recorded: {e}");
    }
    Some(match next {
        Some((std::cmp::Reverse(worth), _, name)) => format!(
            "clock death: {to}'s clock ran out ({why}); next fit: {name}, worth 0.{worth:03} — advice, never a change"
        ),
        None => format!(
            "clock death: {to}'s clock ran out ({why}); no sibling fits: give a smaller order, or a longer clock"
        ),
    })
}

/// Sit the council after a child's run: a warrant becomes a hole for the parent, relief a note,
/// and too many warrants the escalation its report is forced to. An interrupted run never adds.
fn council(
    c: &Commissioner,
    body: &str,
    ending: &Ending,
    failed: u32,
    notes: &mut Vec<String>,
    holes: &mut Vec<String>,
) -> Option<String> {
    let done = match ending {
        Ending::Answered(text) => {
            Report::parse(text).is_some_and(|r| r.status.split_whitespace().next() == Some("DONE"))
        }
        _ => false,
    };
    let counted = matches!(ending, Ending::Answered(_));
    match crate::council::sit(&c.world, c.council, body, u64::from(failed), done, counted) {
        crate::council::Sitting::Quiet => None,
        crate::council::Sitting::Warrant(text) => {
            holes.push(text);
            None
        }
        crate::council::Sitting::Relieved(line) => {
            notes.push(format!("[council: relief — {line}]"));
            None
        }
        crate::council::Sitting::Escalate(why) => Some(why),
    }
}

/// Relay-hold: an answering child's `@$ <body>: <ask>` orders are held on the shared queue in its
/// name; nothing dispatches them mid-turn.
fn hold_orders(q: &mnemorph_world::Queue, world: &Path, ending: &Ending, by: &str) {
    let Ending::Answered(text) = ending else {
        return;
    };
    let Some(report) = Report::parse(text) else {
        return;
    };
    let roster = mnemorph_world::Roster::load(world);
    for order in &report.enqueues {
        if let Err(e) = q.add(&roster, Some(&order.body), by, &order.ask, 0) {
            eprintln!(
                "mnemorph: {by}'s order for {} could not be held: {e}",
                order.body
            );
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
    model: String,
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
        model,
        system: prompt(&c.world, roster, &name, &parent.name),
    };
    // A creature that may dispatch is handed the same commissioner: the order goes on down.
    let dispatches = creature.race.shelf().contains(&ToolClass::Dispatch);
    let mut tools = registry();
    if dispatches {
        tools.push(Box::new(CommissionTool));
    }
    // An Orc codes its Slimes' verify lines before it commissions them.
    if creature.race.base() == mnemorph_races::Race::Orc {
        tools.push(Box::new(crate::verify_tool::VerifyTool));
    }
    if c.queue.is_some() {
        tools.push(Box::new(crate::queue_tool::QueueTool));
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
    if c.queue.is_some() {
        child = child.with_queue(Arc::clone(c));
    }
    child
}

/// The child's report as its dispatcher reads it: its own wire answer — an answer off the wire is
/// a FAIL, never a DONE — or its ending in words; its written paths as the world shows them; what
/// was put back after it.
fn report_of(
    ending: &Ending,
    landed: &[PathBuf],
    put_back: &[(PathBuf, bool)],
    notes: &[String],
    triage: &[String],
    forced: Option<&str>,
) -> String {
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
    report.other.extend(notes.iter().cloned());
    // The council's last word: after its warrants, the report escalates to the human.
    if let Some(why) = forced {
        report.status = format!("ESCALATE {why} (was: {})", report.status);
    }
    report
        .holes
        .extend(triage.iter().cloned().map(mnemorph_wire::Hole::Other));
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
            .with_build(&build)
            .with_seat(&creature.seat),
    )
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    /// A clock death leaves a `death` row and names the next-fit sibling; another checkpoint
    /// leaves no memo.
    #[test]
    fn a_clock_death_leaves_a_memo() {
        let w = std::env::temp_dir().join(format!("mnemorph-clock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        for (race, short, body) in [
            ("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n"),
            (
                "elf",
                "e",
                "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n",
            ),
            (
                "orc",
                "o",
                "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n",
            ),
            (
                "slime",
                "s",
                "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/a/\n",
            ),
            (
                "slime",
                "t",
                "# slime-t\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/b/\n",
            ),
        ] {
            let dir = w.join(".mnemorph").join(race).join(short);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("README.md"), body).unwrap();
        }
        let roster = Roster::load(&w);
        let died = Ending::Checkpoint {
            why: "clock: the deadline passed".into(),
            salvage: Some("I wrote half of it".into()),
        };
        let memo = clock_memo(&w, &roster, "slime-s", &died).unwrap();
        assert!(memo.contains("next fit: slime-t, worth 0.750"), "{memo}");
        let rows = Journal::<Death>::new(w.join(DEATHS)).read().unwrap().rows;
        assert_eq!((rows.len(), rows[0].body.as_str()), (1, "slime-s"));
        // A salvage turn that wrote nothing is still a clock death.
        let failed_salvage = Ending::Checkpoint {
            why: "clock: the deadline passed during the model call".into(),
            salvage: None,
        };
        assert!(clock_memo(&w, &roster, "slime-t", &failed_salvage).is_some());
        let stopped = Ending::Checkpoint {
            why: "budget: spent".into(),
            salvage: None,
        };
        assert_eq!(clock_memo(&w, &roster, "slime-s", &stopped), None);
        let alone = clock_memo(&w, &roster, "orc-o", &died).unwrap();
        assert!(alone.contains("no sibling fits"), "{alone}");
        let _ = std::fs::remove_dir_all(&w);
    }
}
