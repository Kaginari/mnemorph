//! Memory: the archive is never rewritten; notes are traces that point at it. A note is a fold
//! over appended events, in three tiers — personal, shared and short — and only a route's proof
//! grounds one into shared memory.

pub mod audit;
pub mod consolidate;
pub mod note;
pub mod pathology;
pub mod recall;
pub mod store;
pub mod text;
pub mod write;

pub use audit::{Unproven, findings, status, unfrozen};
pub use consolidate::{Forget, distill, forget};
pub use note::{By, Change, Event, Link, Note, Polarity, Status, fold};
pub use recall::{Hit, Memory, Recalled, Relation, Weights, gather, lines, recall};
pub use store::{Tier, inherit, read, union_merge};
pub use write::{
    Anchor, Claim, Evidence, Grounded, Proof, Proposed, Retired, Route, attributable, cache,
    confirm, ground, propose, retire, retired, revise, wake,
};
