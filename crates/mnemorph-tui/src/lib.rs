//! Mnemorph's terminal UI: one session hosted in-process — no protocol between the screen and
//! the core — drawn with Ratatui in the Elm architecture.
//!
//! The kit ([`kit`]) holds the reusable parts, Charm's ecosystem rebuilt; [`app`] is the model,
//! its messages, `update` and `view`; [`run`] owns the terminal, the session's driver and the
//! loop; [`gate`] is the human gate as an approval card.

pub mod app;
pub mod approval;
pub mod blocks;
pub mod diff;
pub mod gate;
pub mod kit;
pub mod run;
pub mod safe;
pub mod theme;
pub mod transcript;
pub mod view;
