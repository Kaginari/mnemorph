//! A commission: one creature's work, handed down. This part builds the body's tools over the
//! soil the world yields for it.

use std::path::Path;

use mnemorph_tools::{Context, ignored};
use mnemorph_world::{Owner, Roster, Rule, explain, soil};

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
