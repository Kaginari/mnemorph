//! May one creature commission another? The races' law decides on five facts; the world finds
//! them — the lineage, the plan, the verify line, the lane, the budget — and names the creature a
//! refused order should go to instead.

use std::fmt;

use mnemorph_races::{DispatchContext, Race, Refusal, Route, may_dispatch};

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
    /// The lawful next act, in words.
    pub next: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.why {
            Some(why) => write!(f, "{why}: {}", self.next),
            None => f.write_str(&self.next),
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
            let next = match why {
                // Below the dispatcher, the lawful hop is the next rank down toward the target;
                // anywhere else, it goes up.
                Refusal::NotOneHop => match chain.iter().position(|n| n == from) {
                    Some(i) if i > 0 => {
                        format!("commission {}, the next rank toward {to}", chain[i - 1])
                    }
                    _ => format!("report it to {parent}"),
                },
                Refusal::NotYourLine => format!(
                    "ask {}, whose chain holds {to}",
                    b.reports_to.as_deref().unwrap_or("Rimuru")
                ),
                Refusal::NoPlanItem => format!("open a plan item that names {to}"),
                Refusal::VerifyFirst => {
                    format!("code the verify line on {to}'s doc ({})", b.doc())
                }
                Refusal::Budget => format!(
                    "@? budget: need ~{} wt, have {} wt — to {parent}",
                    ask.slice_wt, ask.left_wt
                ),
            };
            Err(Refused {
                why: Some(why),
                next,
            })
        }
    }
}
