//! The world: creatures, territory, dispatch, the end gate, the log writer, evolution.

pub mod doc;
pub mod own;
pub mod roster;

pub use doc::{Creature, parse};
pub use own::{Owner, WORKSPACE, own};
pub use roster::Roster;
