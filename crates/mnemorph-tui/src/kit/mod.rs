//! The kit: the good parts of Charm's Bubble Tea ecosystem, rebuilt in Rust on Ratatui, for
//! every view of the terminal UI to build on.
//!
//! | Charm | Here | What it gives |
//! |---|---|---|
//! | Lip Gloss | [`style`], [`text`] | a chainable box style — padding, borders with titles, width, alignment, margins — and blocks joined across and down, measured in terminal cells |
//! | Bubbles | [`textarea`], [`viewport`], [`spinner`], [`keyhelp`], [`gauge`] | components, each a small struct with its own `update` and `view` |
//! | Harmonica | [`spring`] | a critically damped spring: smooth scrolling, later panel motion |
//! | Glamour | [`markdown`], [`highlight`] | Markdown drawn from a stylesheet the theme derives, code in colour |
//!
//! Bubble Tea's own part, the Elm architecture — one model, messages, a pure `update` that
//! returns commands, a `view` — is the application itself ([`crate::app`]). A component never
//! reads the terminal: it is given keys and a width, and returns lines.

pub mod gauge;
pub mod highlight;
pub mod keyhelp;
pub mod markdown;
pub mod spinner;
pub mod spring;
pub mod style;
pub mod text;
pub mod textarea;
pub mod viewport;
