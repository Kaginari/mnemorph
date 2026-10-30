//! Memory: the archive is never rewritten; notes are traces that point at it. A note is a fold
//! over appended events, in three tiers — personal, shared and short — and only a route's proof
//! grounds one into shared memory.

pub mod audit;
pub mod consolidate;
pub mod graph;
pub mod note;
pub mod pathology;
pub mod recall;
pub mod standing;
pub mod store;
pub mod text;
pub mod write;

pub use audit::{Unproven, findings, status, unfrozen};
pub use consolidate::{Forget, distill, forget};
pub use graph::{Class, Moved, Strength, invalidate, landed, strengths, under_review};
pub use note::{By, Change, Dependency, Event, Link, Note, Polarity, Status, Version, fold};
pub use recall::{Hit, Memory, Recalled, Relation, Weights, gather, lines, recall};
pub use standing::{Standing, standing};
pub use store::{Tier, inherit, read, union_merge};
pub use write::{
    Anchor, Claim, Evidence, Grounded, Proof, Proposed, Retired, Route, Seconded, amend,
    attributable, cache, confirm, corroborate, ground, proposals, propose, propose_on,
    propose_resting, proposed, retire, retired, retired_in, revise, vouch, wake, wake_by,
};
