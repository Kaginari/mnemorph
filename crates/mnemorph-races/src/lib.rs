//! The race law: the six races and five ascended forms, their shelves, powers, scores,
//! ascension and falls. No I/O.
//!
//! Everything a race may do is compiled here and nowhere else; no configuration adds a race,
//! changes a reporting line or widens a shelf. The book's page is `docs/02-races.md`; where the
//! two could read differently, this crate is what runs and the page is corrected.

mod dispatch;
mod race;
mod shelf;
mod value;

pub use dispatch::{DispatchContext, Refusal, Route, may_dispatch};
pub use race::{ModelTier, Race};
pub use shelf::{Gate, Post, ShelfRefusal, Soil, ToolClass, check_class};
pub use value::{
    Band, DECLINE_COOLDOWN_RUNS, FallReading, Meter, RECOVER_REPORTS, Recent, Signals, falls,
    meter, recovers, rises, score,
};
