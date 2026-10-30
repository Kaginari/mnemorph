//! May one creature commission another? The races' law decides on five facts; the world finds
//! them — the lineage, the plan, the verify line, the lane, the budget — and names the creature a
//! refused order should go to instead.

use std::fmt;

use mnemorph_ledger::{BudgetShort, Shortfall};
use mnemorph_races::{DispatchContext, Race, Refusal, Route, may_dispatch};
use mnemorph_wire::Hole;

use crate::roster::Roster;

/// What the dispatcher brings: its open plan items' targets, and the slice it carves.
#[derive(Debug, Clone, Default)]
pub struct Ask<'a> {
    /// The creatures its open plan items name (asked of Elves).
    pub plan: &'a [String],
    /// The slice it would carve, in weighted tokens.
    pub slice_wt: u64,
    /// What its envelope has left.
    pub left_wt: u64,
    /// The human's cap that sets what is left — `session`, `day` or `month` — when the dispatcher
    /// spends the session's budget itself; `None` for an envelope its commander carved.
    pub cap: Option<&'static str>,
}

/// A lawful dispatch: to whom the order goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hop {
    /// Commission the target directly.
    Direct(String),
    /// Send it as `@$ <elf>: <ask>` to the Elf whose lane holds the target.
    Via {
        /// The other lane's Elf.
        elf: String,
        /// The target.
        target: String,
    },
}

/// Why a dispatch is refused, and the lawful next act, naming the creature it goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The law's reason; `None` when a name is no living creature.
    pub why: Option<Refusal>,
    /// The lawful next act, in words — for a budget refusal, the wire's typed `@?` hole.
    pub next: String,
    /// The creature the next act goes to, when it goes to one.
    pub whom: Option<String>,
    /// For a budget refusal, the shortfall and the top-up that would let the slice fit.
    pub short: Option<Shortfall>,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.short, self.why) {
            (Some(short), _) => write!(f, "{short}: {}", self.next),
            (None, Some(why)) => write!(f, "{why}: {}", self.next),
            (None, None) => f.write_str(&self.next),
        }
    }
}

/// Whether `from` may commission `to`, the facts read from `roster`.
///
/// # Errors
///
/// The first rule the dispatch breaks, with the lawful next act.
pub fn dispatch(roster: &Roster, from: &str, to: &str, ask: &Ask<'_>) -> Result<Hop, Refused> {
    let living = |name: &str| roster.creatures.get(name).filter(|c| !c.retired);
    let unknown = |name: &str| Refused {
        why: None,
        next: format!("`{name}` is no living creature: Rimuru routes its birth"),
        whom: None,
        short: None,
    };
    let a = living(from).ok_or_else(|| unknown(from))?;
    let b = living(to).ok_or_else(|| unknown(to))?;
    let chain = roster.chain(to);
    // The Elf whose lane holds the target: the first Elf above it.
    let elf = chain
        .iter()
        .find(|n| living(n).is_some_and(|c| c.race.base() == Race::Elf))
        .cloned();
    let ctx = DispatchContext {
        lineage: chain.iter().any(|n| n == from),
        plan_names_target: ask.plan.iter().any(|n| n == to),
        verify_coded: !b.verify.is_empty(),
        other_lane: elf.as_deref().is_some_and(|e| e != from),
        budget_fits: ask.slice_wt <= ask.left_wt,
    };
    let parent = a.reports_to.clone().unwrap_or_else(|| "the human".into());
    match may_dispatch(a.race, b.race, &ctx) {
        Ok(Route::Direct) => Ok(Hop::Direct(to.to_owned())),
        Ok(Route::ViaItsElf) => Ok(Hop::Via {
            elf: elf.unwrap_or_default(),
            target: to.to_owned(),
        }),
        Err(why) => {
            let (next, whom) = match why {
                // Below the dispatcher, the lawful hop is the next rank down toward the target;
                // anywhere else, it goes up.
                Refusal::NotOneHop => match chain.iter().position(|n| n == from) {
                    Some(0) => (
                        format!(
                            "{to} reports to {from} directly, skipping a rank: Rimuru routes its lane"
                        ),
                        None,
                    ),
                    Some(i) => {
                        let n = chain[i - 1].clone();
                        (
                            format!("commission {n}, the next rank toward {to}"),
                            Some(n),
                        )
                    }
                    None => (format!("report it to {parent}"), a.reports_to.clone()),
                },
                Refusal::NotYourLine => {
                    let n = b.reports_to.clone().unwrap_or_else(|| "rimuru".into());
                    (format!("ask {n}, whose chain holds {to}"), Some(n))
                }
                Refusal::NoPlanItem => (format!("open a plan item that names {to}"), None),
                Refusal::VerifyFirst => (
                    format!("code the verify line on {to}'s doc ({})", b.doc()),
                    None,
                ),
                // The wire's own hole, so the two never drift; the commander is its own field.
                Refusal::Budget => (
                    format!(
                        "@? {}",
                        Hole::Budget {
                            need: ask.slice_wt,
                            have: ask.left_wt,
                        }
                    ),
                    a.reports_to.clone(),
                ),
            };
            // A route never names a creature that cannot take the order.
            if let Some(n) = whom.as_deref()
                && living(n).is_none()
            {
                return Err(Refused {
                    why: None,
                    next: format!(
                        "`{n}` is no living creature: Rimuru routes a new commander for its line"
                    ),
                    whom: None,
                    short: None,
                });
            }
            let short = (why == Refusal::Budget).then(|| {
                BudgetShort {
                    need: ask.slice_wt,
                    have: ask.left_wt,
                }
                .refusal(ask.cap)
            });
            Err(Refused {
                why: Some(why),
                next,
                whom,
                short,
            })
        }
    }
}
