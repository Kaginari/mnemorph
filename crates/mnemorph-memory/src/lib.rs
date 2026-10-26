//! Memory: the archive is never rewritten; notes are traces that point at it. A note is a fold
//! over appended events, in three tiers — personal, shared and short — and only a route's proof
//! grounds one into shared memory.

pub mod note;
pub mod recall;
pub mod store;
pub mod text;

pub use note::{By, Change, Event, Note, Polarity, Status, fold};
pub use recall::{Hit, Recalled, Relation, Weights, recall};
pub use store::{Tier, append, inherit, read, union_merge};
