//! Metis — measure, wise counsel — routes a commission to the tier its work deserves.
//!
//! Each office has a band: Mneme (read) may run fast, Melete (judge) and Aoede (write) never
//! below strong, and any of them may rise to frontier when the work is hard or hard to undo. The
//! default is the tier the body rides today; Metis only moves within the band, and only on an
//! answer that is sure for its basis. A model the human chose for a creature or an office is
//! never routed.

use std::time::Instant;

use mnemorph_ledger::Timestamp;
use mnemorph_provider::{Ask, Caller, Decision, Error, Question, SystemOne, Wait};
use mnemorph_races::{ModelTier, Race};
use mnemorph_wire::Office;

use crate::decisions::Decisions;

/// The decision point that routes a commission.
pub const ROUTE_POINT: &str = "model.route";

/// The most of a commission's text a route reads.
const ROUTE_HEAD: usize = 8_000;

/// Why a body rides the model it rides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The human chose the model for this creature or office.
    Pinned,
    /// Routing is off, or there is no decider: the body's default tier.
    Off,
    /// Metis decided, surely.
    Decided,
    /// Metis was not sure enough for its basis: the default tier.
    Unsure,
    /// The decision failed, or the human's Esc stopped it: the default tier.
    Failed(String),
}

/// A route: the model, its tier (none when pinned), why, and the decision when one was made.
#[derive(Debug, Clone)]
pub struct Routed {
    /// The model the body rides.
    pub model: String,
    /// Its tier; `None` for a model the human pinned.
    pub tier: Option<ModelTier>,
    /// Why.
    pub reason: Reason,
    /// The decision, when Metis was asked: journaled already, for the caller to price.
    pub decision: Option<Decision>,
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
/// the ceiling frontier. The Dark Elf, the adversary, never goes below its own tier.
#[must_use]
pub fn band(race: Race, office: Option<Office>) -> (ModelTier, ModelTier, ModelTier) {
    let default = race.default_tier();
    let office_floor = match office {
        Some(Office::Findings) => ModelTier::Fast,
        Some(Office::Verdict | Office::Draft) => ModelTier::Strong,
        None => default,
    };
    let floor = if race.base() == Race::DarkElf {
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

/// The tier a decision picks within `(floor, default, ceiling)`: the ceiling when the work is
/// surely hard to undo or surely hard, the floor when surely routine, else the default.
#[must_use]
pub fn pick(
    decision: &Decision,
    decisions: &Decisions,
    band: (ModelTier, ModelTier, ModelTier),
) -> (ModelTier, Reason) {
    let (floor, default, ceiling) = band;
    let cut = decisions.cutoffs;
    if decision
        .sure("irreversible", cut)
        .is_some_and(|a| a.choice() == "true")
    {
        return (ceiling, Reason::Decided);
    }
    match decision
        .sure("demand", cut)
        .map(mnemorph_provider::Answer::choice)
    {
        Some("0") => (floor, Reason::Decided),
        Some("1") => (default, Reason::Decided),
        Some("2") => (ceiling, Reason::Decided),
        _ => (default, Reason::Unsure),
    }
}

/// Routes one body: the model the human pinned, else the tier Metis picks within the band, else
/// the body's default tier. Each decision is journaled at [`ROUTE_POINT`].
pub async fn route(
    config: &mnemorph_config::Config,
    decisions: Option<&Decisions>,
    caller: &Caller,
    race: Race,
    office: Option<Office>,
    commission: &str,
) -> Routed {
    if let Some(model) = config.model_pinned(&caller.body, race, office.map(Office::name)) {
        return Routed {
            model,
            tier: None,
            reason: Reason::Pinned,
            decision: None,
        };
    }
    let b = band(race, office);
    let at = |tier: ModelTier, reason: Reason, decision: Option<Decision>| Routed {
        model: config.tier_model(tier),
        tier: Some(tier),
        reason,
        decision,
    };
    let Some(d) = decisions.filter(|d| d.route) else {
        return at(b.1, Reason::Off, None);
    };
    let started = Instant::now();
    let ask = route_ask(race, office, commission);
    let answer = tokio::time::timeout(SystemOne::policy().turn, d.decider.decide(&ask))
        .await
        .unwrap_or(Err(Error::Timeout(Wait::Turn)));
    let decision = match answer {
        Ok(decision) => decision,
        Err(e) => {
            return at(
                b.1,
                Reason::Failed(format!("{e} after {:?}", started.elapsed())),
                None,
            );
        }
    };
    let _ = d
        .journal
        .append(&decision.row(caller, ROUTE_POINT, Timestamp::now()));
    let (tier, reason) = pick(&decision, d, b);
    at(tier, reason, Some(decision))
}
