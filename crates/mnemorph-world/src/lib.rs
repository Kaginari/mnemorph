//! The world: creatures, territory, dispatch, the end gate, the log writer, evolution.

/// The world's memory: notes, recall, the write rule and the audit, for the human's commands.
pub use mnemorph_memory as memory;

pub mod chains;
pub mod challenge;
pub mod dispatch;
pub mod doc;
pub mod facts;
pub mod findings;
pub mod graph;
pub mod heal;
pub mod log;
pub mod minds;
pub mod own;
pub mod queue;
pub mod relief;
pub mod report;
pub mod roster;
pub mod soil;
pub mod start;
pub mod verify;
pub mod worth;

pub use dispatch::{Ask, Hop, Refused, dispatch};
pub use doc::{Creature, parse};
pub use facts::{Facts, Status as FactStatus};
pub use findings::{
    Finding, Severity, check, fact_findings, mind_findings, prose_claims, provenance,
};
pub use graph::{Bond, Graph};
pub use own::{Owner, Ownership, Rule, WORKSPACE, explain, own};
pub use queue::{Queue, Refusal as QueueRefusal};
pub use roster::Roster;
pub use soil::{Soil, soil};
