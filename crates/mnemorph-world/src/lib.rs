//! The world: creatures, territory, dispatch, the end gate, the log writer, evolution.

pub mod doc;
pub mod findings;
pub mod graph;
pub mod own;
pub mod roster;
pub mod soil;

pub use doc::{Creature, parse};
pub use findings::{Finding, Severity, check};
pub use graph::{Bond, Graph};
pub use own::{Owner, Ownership, Rule, WORKSPACE, explain, own};
pub use roster::Roster;
pub use soil::{Soil, soil};
