//! The human's door to the Oni's envelopes (`mnemorphd envelope …`): list each — its scope, its
//! expiry, its uses against its cap, whether it is in force, revoked or suspended, and every act it
//! admitted — and revoke one. Envelopes live in the human's own file and their uses under the
//! human's state directory, outside the world; a body never runs any of this.

use std::path::Path;

use mnemorph_config::EnvelopeGate;

fn gate(world: &Path) -> Result<EnvelopeGate, String> {
    EnvelopeGate::of(world)
        .ok_or_else(|| "there is no state directory, so no envelope use is counted".into())
}

/// One block per envelope signed for `world`, escaped.
///
/// # Errors
///
/// There is no state directory to read uses from.
pub fn list(world: &Path) -> Result<Vec<String>, String> {
    Ok(gate(world)?.lines(world))
}

/// Revoke the envelope `name`, today: it stays on record and admits nothing.
///
/// # Errors
///
/// Why it cannot be ([`EnvelopeGate::revoke`]).
pub fn revoke(world: &Path, name: &str) -> Result<String, String> {
    gate(world)?.revoke(name)
}
