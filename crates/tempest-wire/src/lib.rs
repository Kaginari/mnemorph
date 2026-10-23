//! The wire: the envelope bodies speak — tags, parser, encoder, trimming. No I/O.
//!
//! A [`Commission`] goes down (`@ROOT @SCOPE @ASK @CAP @BUDGET @DUMP @SIZE`), a [`Report`] comes
//! back (`@S @? @U @V @F @W @T @$ @FORGE @R @H @E`). The tags and their meaning are defined in
//! the book, `docs/04-communication.md`.
//!
//! Three promises hold for every report this crate emits:
//! - `@E` is the exact byte length of the whole answer, its own line included;
//! - over `@CAP`, lines are cut in a fixed order and `@S`, `@?`, `@U`, `@R`, `@H` never are —
//!   the cut itself is named as a hole;
//! - parsing what was emitted gives back the same report.

mod commission;
mod fnv;
mod forge;
mod report;
mod tag;

pub use commission::{Ask, Commission, DEFAULT_CAP, Office, parse_budget};
pub use forge::{Forge, ForgeKind, forges_of};
pub use report::{
    COURIER_BYTES, Courier, Enqueue, Finding, Report, Unsaid, UnsaidKind, enqueues_of,
};
