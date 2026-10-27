//! The race law: the six races and five ascended forms, their shelves, powers, scores,
//! ascension and falls, and the Dark Elf's specialities, routing and playbooks. No I/O.
//!
//! Everything a race may do is compiled here and nowhere else; no configuration adds a race,
//! changes a reporting line or widens a shelf. The book's page is `docs/02-races.md`; where the
//! two could read differently, this crate is what runs and the page is corrected.

mod dispatch;
mod playbook;
mod race;
mod shelf;
mod speciality;
mod value;

pub use dispatch::{DispatchContext, Refusal, Route, may_dispatch};
pub use playbook::{MAX_CLASS, MAX_SPECIALITY, brief, for_landing, playbook};
pub use race::{ModelTier, Race};
pub use shelf::{
    Gate, OutwardKind, Post, PostRefusal, ShelfRefusal, Soil, ToolClass, check_class, check_post,
};
pub use speciality::{AttackClass, Speciality, glob, route, route_with};
pub use value::{
    Band, DECLINE_COOLDOWN_RUNS, FallReading, Meter, RECOVER_REPORTS, Recent, Signals, falls,
    meter, recovers, rises, score,
};
