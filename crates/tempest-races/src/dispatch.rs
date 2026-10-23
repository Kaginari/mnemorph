//! May one body commission another? The decision table of `docs/04-communication.md`.

use std::fmt;

use crate::Race;

/// What the world knows about one proposed dispatch, beyond the two races. The world fills it
/// in (it holds the lineage, the plans, the docs and the ledger); the law only decides.
// Five independent facts the world supplies; the law reads each on its own, so a bool apiece
// is the honest shape.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DispatchContext {
    /// The target's reporting chain reaches the dispatcher.
    pub lineage: bool,
    /// The dispatcher's plan holds an open item naming the target (asked of Elves).
    pub plan_names_target: bool,
    /// The target Slime's doc carries a coded verify line (asked of Orcs).
    pub verify_coded: bool,
    /// The target sits in another Elf's lane (asked of High Elves).
    pub other_lane: bool,
    /// The carved slice fits the dispatcher's envelope.
    pub budget_fits: bool,
}

/// How a lawful dispatch travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Commission the target directly.
    Direct,
    /// Send it as `@$ <that lane's elf>: <ask>`: through the other Elf, never past it.
    ViaItsElf,
}

/// Why a dispatch is refused. Every refusal names the lawful next act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The target is not one hop below the dispatcher.
    NotOneHop,
    /// The target's chain does not reach the dispatcher.
    NotYourLine,
    /// An Elf dispatched with no plan item naming the target.
    NoPlanItem,
    /// An Orc dispatched a Slime whose verify line is not coded yet.
    VerifyFirst,
    /// The slice does not fit the envelope.
    Budget,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Refusal::NotOneHop => "refused: not one hop below you — report up, one hop at a time",
            Refusal::NotYourLine => {
                "refused: its chain does not reach you — ask the body it reports to"
            }
            Refusal::NoPlanItem => "refused: write the plan item that names it first",
            Refusal::VerifyFirst => {
                "refused: code its verify line first (the test before the change)"
            }
            Refusal::Budget => "refused: the slice does not fit — @? budget to your parent",
        })
    }
}

/// Decides whether `from` may commission `to`, in the fixed order of the decision table:
/// command (or sideways, or a High Elf's other lane), lineage, plan, verify, budget.
///
/// # Errors
///
/// The first rule the dispatch breaks.
pub fn may_dispatch(from: Race, to: Race, ctx: &DispatchContext) -> Result<Route, Refusal> {
    // A High Elf's other lane comes first: the target is an Orc it commands by race, but not by
    // lineage, so the order travels through that lane's Elf.
    if from == Race::HighElf && to.base() == Race::Orc && ctx.other_lane {
        return if ctx.budget_fits {
            Ok(Route::ViaItsElf)
        } else {
            Err(Refusal::Budget)
        };
    }
    let commanded = from.commands().contains(&to.base());
    let sideways = from.sideways() && to.base() == from.base();
    if !commanded && !sideways {
        return Err(Refusal::NotOneHop);
    }
    if commanded && !ctx.lineage {
        return Err(Refusal::NotYourLine);
    }
    if from.base() == Race::Elf && !ctx.plan_names_target {
        return Err(Refusal::NoPlanItem);
    }
    if from.base() == Race::Orc && to.base() == Race::Slime && !ctx.verify_coded {
        return Err(Refusal::VerifyFirst);
    }
    if !ctx.budget_fits {
        return Err(Refusal::Budget);
    }
    Ok(Route::Direct)
}
