//! Metis — measure, wise counsel — routes a commission to the tier its work deserves.
//!
//! Each office has a band: Mneme (read) may run fast, Melete (judge) and Aoede (write) never
//! below strong, and any of them may rise to frontier when the work is hard or hard to undo. The
//! default is the tier the body rides today; Metis only moves within the band, and only on an
//! answer that is sure for its basis. A model the human chose for a creature or an office is
//! never routed.

use std::time::Duration;

use mnemorph_ledger::{Journal, Prices, Timestamp, UsageRow};
use mnemorph_provider::{Ask, Caller, Decision, Error, Question, SystemOne, Wait};
use mnemorph_races::{ModelTier, Race};
use mnemorph_wire::Office;

use crate::decisions::{Decisions, decision_usage};
use crate::run::Interrupt;

/// The decision point that routes a commission.
pub const ROUTE_POINT: &str = "model.route";

/// The most of a commission's text a route reads.
const ROUTE_HEAD: usize = 8_000;

/// Why a body rides the model it rides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The human chose the model for this creature or office.
    Pinned,
    /// Routing is off, there is no decider, or the body is the throne: the default tier.
    Off,
    /// Metis decided, surely.
    Decided,
    /// Metis was not sure enough for its basis to move the body: the default tier.
    Unsure,
    /// Metis picked a tier no model is configured for: the default tier.
    Unconfigured(ModelTier),
    /// The decision failed or ran out of time: the default tier.
    Failed(Error),
    /// The human's Esc stopped the decision: the default tier.
    Interrupted,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Reason::Pinned => "the human's pin",
            Reason::Off => "routing off: the race's tier",
            Reason::Decided => "Metis decided",
            Reason::Unsure => "Metis unsure: the race's tier",
            Reason::Unconfigured(_) => "no model for the tier Metis picked: the race's tier",
            Reason::Failed(_) => "Metis failed: the race's tier",
            Reason::Interrupted => "Esc during the route: the race's tier",
        })
    }
}

/// A route: the model, its tier (none when pinned), why, and the decision when one was made —
/// journaled and priced already.
#[derive(Debug, Clone)]
pub struct Routed {
    /// The model the body rides.
    pub model: String,
    /// Its tier; `None` for a model the human pinned.
    pub tier: Option<ModelTier>,
    /// Why.
    pub reason: Reason,
    /// The decision, when Metis was asked.
    pub decision: Option<Decision>,
}

/// The work to route: who will do it, in which office, and the commission's text.
#[derive(Debug, Clone, Copy)]
pub struct Work<'a> {
    /// The body's race.
    pub race: Race,
    /// Its office, from the commission's `office`.
    pub office: Option<Office>,
    /// The commission, as sent.
    pub commission: &'a str,
}

/// What can stop a route: the human's Esc, and the time the caller has left.
#[derive(Clone, Copy)]
pub struct Stop<'a> {
    /// The run's interrupt.
    pub interrupt: &'a Interrupt,
    /// The time left on the caller's clock.
    pub left: Duration,
}

/// Where a route's cost is counted: priced like any call into the usage ledger.
#[derive(Clone, Copy)]
pub struct Meter<'a> {
    /// The price table.
    pub prices: &'a Prices,
    /// The usage ledger, when the caller keeps one.
    pub ledger: Option<&'a Journal<UsageRow>>,
}

/// A tier's place, fast lowest.
fn rank(t: ModelTier) -> u8 {
    match t {
        ModelTier::Fast => 0,
        ModelTier::Strong => 1,
        ModelTier::Frontier => 2,
    }
}

fn lowest(a: ModelTier, b: ModelTier) -> ModelTier {
    if rank(a) <= rank(b) { a } else { b }
}

/// The band a body may be routed within: `(floor, default, ceiling)`. The default is its race's
/// tier; the floor its office's (fast to read, strong to judge or write), never above the default;
/// the ceiling frontier. The Dark Elf, the adversary, never goes below its own tier, and the throne
/// is never routed: it does not choose its own horse.
#[must_use]
pub fn band(race: Race, office: Option<Office>) -> (ModelTier, ModelTier, ModelTier) {
    let default = race.default_tier();
    let office_floor = match office {
        Some(Office::Findings) => ModelTier::Fast,
        Some(Office::Verdict | Office::Draft) => ModelTier::Strong,
        None => default,
    };
    let floor = if matches!(race.base(), Race::DarkElf | Race::Rimuru) {
        default
    } else {
        lowest(office_floor, default)
    };
    (floor, default, ModelTier::Frontier)
}

/// The questions a commission puts to Metis.
#[must_use]
pub fn route_ask(race: Race, office: Option<Office>, commission: &str) -> Ask {
    let mut end = commission.len().min(ROUTE_HEAD);
    while !commission.is_char_boundary(end) {
        end -= 1;
    }
    let who = office.map_or_else(
        || format!("a {} body", race.name()),
        |o| format!("{} ({}), a {} body", o.name(), o.function(), race.name()),
    );
    Ask {
        state: format!("For {who}:\n{}", &commission[..end]),
        questions: vec![
            (
                "demand".into(),
                Question::Score {
                    instructions: "How demanding is this work for the model that will do it?"
                        .into(),
                    levels: vec![
                        "Routine: a lookup, a rename, a small change with an obvious shape.".into(),
                        "Involved: several files or steps, some judgement, a known pattern.".into(),
                        "Hard: subtle reasoning, an unclear design, wide or tricky change.".into(),
                    ],
                },
            ),
            (
                "irreversible".into(),
                Question::Noul {
                    instructions: "A mistake in this work would be costly or hard to undo: \
                                   security, money, migrations, data loss, or an outward act."
                        .into(),
                },
            ),
        ],
    }
}

/// The tier a decision picks within `(floor, default, ceiling)`. It rises to the ceiling when the
/// work is surely hard to undo, or surely hard. It falls to the floor only when the work is surely
/// routine *and* surely not hard to undo: an unresolved risk never lowers a body. Surely involved
/// work, and anything unsure, stays at the default.
#[must_use]
pub fn pick(
    decision: &Decision,
    decisions: &Decisions,
    band: (ModelTier, ModelTier, ModelTier),
) -> (ModelTier, Reason) {
    let (floor, default, ceiling) = band;
    let cut = decisions.cutoffs;
    let risk = decision
        .sure("irreversible", cut)
        .map(mnemorph_provider::Answer::choice);
    let demand = decision
        .sure("demand", cut)
        .map(mnemorph_provider::Answer::choice);
    match (risk, demand) {
        (Some("true"), _) | (_, Some("2")) => (ceiling, Reason::Decided),
        (Some("false"), Some("0")) => (floor, Reason::Decided),
        (_, Some("1")) => (default, Reason::Decided),
        _ => (default, Reason::Unsure),
    }
}

/// Routes one body: the model the human pinned, else the tier Metis picks within the band, else
/// the body's default tier. Each decision is journaled at [`ROUTE_POINT`] and priced into the
/// meter's ledger. A route waits at most the least of a decision's budget and the time `stop`
/// leaves, and an Esc ends the wait.
pub async fn route(
    config: &mnemorph_config::Config,
    decisions: Option<&Decisions>,
    caller: &Caller,
    work: Work<'_>,
    stop: Option<Stop<'_>>,
    meter: Meter<'_>,
) -> Routed {
    let Work {
        race,
        office,
        commission,
    } = work;
    if let Some(model) = config.model_pinned(&caller.body, race, office.map(Office::name)) {
        return Routed {
            model,
            tier: None,
            reason: Reason::Pinned,
            decision: None,
        };
    }
    let b = band(race, office);
    let default = |reason: Reason, decision: Option<Decision>| Routed {
        model: config.model_at(race, b.1),
        tier: Some(b.1),
        reason,
        decision,
    };
    // A band with no room — one tier, or every tier riding the same model — has nothing to decide,
    // so no classifier is asked and nothing is spent.
    let room = b.0 != b.2 && config.model_at(race, b.0) != config.model_at(race, b.2);
    let Some(d) = decisions.filter(|d| d.route && room) else {
        return default(Reason::Off, None);
    };
    let budget = stop.map_or(SystemOne::policy().turn, |s| {
        s.left.min(SystemOne::policy().turn)
    });
    let ask = route_ask(race, office, commission);
    let decide = tokio::time::timeout(budget, d.decider.decide(&ask));
    let answer = match stop {
        Some(s) => tokio::select! {
            a = decide => a,
            () = s.interrupt.fired() => return default(Reason::Interrupted, None),
        },
        None => decide.await,
    };
    let decision = match answer.unwrap_or(Err(Error::Timeout(Wait::Turn))) {
        Ok(decision) => decision,
        Err(e) => return default(Reason::Failed(e), None),
    };
    if let Err(e) = d
        .journal
        .append(&decision.row(caller, ROUTE_POINT, Timestamp::now()))
    {
        eprintln!("mnemorph: the decision journal could not be written: {e}");
    }
    if let Some(ledger) = meter.ledger
        && let Err(e) = ledger.append(&decision_usage(&decision, caller, meter.prices))
    {
        eprintln!("mnemorph: the usage ledger could not be written: {e}");
    }
    let (tier, reason) = pick(&decision, d, b);
    let model = config.model_at(race, tier);
    if model.is_empty() {
        return default(Reason::Unconfigured(tier), Some(decision));
    }
    Routed {
        model,
        tier: Some(tier),
        reason,
        decision: Some(decision),
    }
}
