//! The evolution ledger's home: `$XDG_STATE_HOME/mnemorph/evolution/<world>-<fnv64a>.jsonl`,
//! outside the world and out of every command's reach, written by the harness alone, chained by
//! hash and anchored with the world's other chained journals ([`crate::chains`]). An evolution is
//! judged here from that ledger, held to its newest anchored head
//! ([`mnemorph_ledger::evolution`]).

use std::io;
use std::path::{Path, PathBuf};

use mnemorph_ledger::Journal;
use mnemorph_ledger::evolution::{self, EvolutionRow, Judgement};

/// Where a world's evolution ledger is kept:
/// `$XDG_STATE_HOME/mnemorph/evolution/<world>-<fnv64a>.jsonl`, else under `~/.local/state`;
/// `None` when there is no home.
#[must_use]
pub fn ledger_of(world: &Path) -> Option<PathBuf> {
    crate::queue::state_file(world, "evolution")
}

/// The world's evolution ledger, chained, for the harness to append swaps and runs to; `None`
/// when there is no home.
#[must_use]
pub fn journal(world: &Path) -> Option<Journal<EvolutionRow>> {
    ledger_of(world).map(Journal::chained)
}

/// Judges the swap `id` of `world` from its ledger, held to the newest head anchored for it.
///
/// # Errors
///
/// There is no home to keep the ledger under, or it cannot be read.
pub fn judge(world: &Path, id: &str, min_net_wins: u32) -> io::Result<Judgement> {
    let path =
        ledger_of(world).ok_or_else(|| io::Error::other("no home to keep the ledger under"))?;
    let anchor = crate::chains::anchor_of(world, "evolution");
    Ok(evolution::judge(
        &evolution::read(&path, anchor.as_ref())?,
        id,
        min_net_wins,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ledger_lives_outside_the_world_and_is_anchored_with_the_others() {
        let world = Path::new("/srv/worlds/jura");
        let Some(at) = ledger_of(world) else {
            return;
        };
        assert!(!at.starts_with(world), "{}", at.display());
        assert!(at.to_string_lossy().contains("mnemorph/evolution/jura-"));
        assert!(
            crate::chains::journals(world)
                .iter()
                .any(|j| j.name == "evolution" && j.path == at)
        );
        assert!(journal(world).is_some_and(|j| j.is_chained()));
    }
}
