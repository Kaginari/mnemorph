//! Mnemorph's terminal UI: one session hosted in-process — no protocol between the screen and
//! the core — drawn with Ratatui in the Elm architecture.
//!
//! The kit ([`kit`]) holds the reusable parts, Charm's ecosystem rebuilt; [`app`] is the model,
//! its messages, `update` and `view`; [`run`] owns the terminal, the session's driver and the
//! loop; [`gate`] is the human gate as an approval card; [`sidebar`] the column beside the
//! transcript; [`activity`] the living status — what the session does now, in words; [`welcome`]
//! the card the conversation opens on; [`probe`] asks the terminal what it can draw, and
//! [`pictures`] places the races' portraits over the cells as real pixels where it can.

pub mod activity;
pub mod app;
pub mod approval;
pub mod blocks;
pub mod cli;
pub mod diff;
pub mod gate;
pub mod headless;
pub mod kit;
pub mod pictures;
pub mod pin;
pub mod probe;
pub mod run;
pub mod safe;
pub mod sidebar;
pub mod theme;
pub mod transcript;
pub mod tty_gate;
pub mod view;
pub mod welcome;
