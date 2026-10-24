//! A commission: one creature's work, handed down. This part builds the body's tools over the
//! soil the world yields for it.

use std::path::Path;

use mnemorph_tools::Context;
use mnemorph_world::{Roster, soil};

/// The tools' context for the creature `name`: its race's shelf, and the soil `own` yields for
/// it — claims, the claims inside them it does not hold, and the root files if they are its own.
/// `None` when it is not a living creature.
#[must_use]
pub fn context_for(world: &Path, roster: &Roster, name: &str) -> Option<Context> {
    let creature = roster.creatures.get(name).filter(|c| !c.retired)?;
    let s = soil(roster, name)?;
    let none: [&str; 0] = [];
    Some(Context::new(creature.race, world, &none).with_soil(&s.write, &s.except, s.root_files))
}
