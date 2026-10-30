//! The wire: the envelope bodies speak — tags, parser, encoder, trimming — and the one rule for
//! what a human could not see for what it is ([`hidden`], [`shown`]). No I/O.
//!
//! A [`Commission`] goes down (`@ROOT @SCOPE @ASK @CAP @BUDGET @DUMP @SIZE`), a [`Report`] comes
//! back (`@S @? @U @V @F @W @T @$ @FORGE @R @H @E`). The tags and their meaning are defined in
//! the book, `docs/04-communication.md`.
//!
//! Four promises hold for every report this crate emits:
//! - `@E` is the exact byte length of the whole answer, its own line included;
//! - over `@CAP`, lines are cut in a fixed order and `@S`, `@?`, `@U`, `@R`, `@H` never are —
//!   the cut itself is named as a hole;
//! - parsing what was emitted gives back the same report, up to the normalisations the book
//!   lists (line breaks folded, whitespace trimmed, order bodies lower-cased, empty ids dropped);
//! - no field can open a line of its own: line breaks inside fields are folded to spaces, and a
//!   prose line that would read as a tag rides behind one `\`.

mod commission;
mod fnv;
pub use fnv::fnv64a;
mod forge;
mod hole;
mod report;
mod shown;
mod tag;

pub use commission::{Ask, Commission, DEFAULT_CAP, Office, parse_budget};
pub use forge::{Forge, ForgeKind, forges_of};
pub use hole::Hole;
pub use report::{
    COURIER_BYTES, Courier, DarkElfReport, Enqueue, FIELD_BYTES, Finding, Proven, Report,
    RootCause, Unsaid, UnsaidKind, Word, enqueues_of, fingerprint, one_line, repo_path,
};
pub use shown::{
    HIDDEN, MARKS, MARKS_KEPT, SPACES, Strict, hidden, invisible, kept_in_prose, mark, shown,
};
