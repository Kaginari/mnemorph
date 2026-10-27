//! The kit: the good parts of Charm's Bubble Tea ecosystem, rebuilt in Rust on Ratatui, for
//! every view of the terminal UI to build on.
//!
//! | Charm | Here | What it gives |
//! |---|---|---|
//! | Lip Gloss | [`style`], [`text`] | a chainable box style — padding, borders with titles, width, alignment, margins — and blocks joined across and down, measured in terminal cells |
//! | Bubbles | [`textarea`], [`viewport`], [`spinner`], [`keyhelp`], [`gauge`] | components, each a small struct with its own `update` and `view` |
//! | Harmonica | [`spring`], [`toast`] | a critically damped spring: smooth scrolling, a toast's fade |
//! | Glamour | [`markdown`], [`highlight`] | Markdown drawn from a stylesheet the theme derives, code in colour |
//! | Isekai's depth | [`slab`], [`shimmer`] | raised panels — a spine, a lit top edge, a cast shadow — and a crest of light across a busy word |
//! | mnemorph-animations | [`mnanim`], [`glyphs`], [`pixels`] | each race's animations, drawn natively at 256 px: sent as real pixels — Kitty, iTerm2 or Sixel — one frame pixel a screen pixel; or decoded once into glyph cells, octants down to half blocks |
//!
//! Bubble Tea's own part, the Elm architecture — one model, messages, a pure `update` that
//! returns commands, a `view` — is the application itself ([`crate::app`]). A component never
//! reads the terminal: it is given keys and a width, and returns lines.

pub mod gauge;
mod glyph_tables;
pub mod glyphs;
pub mod highlight;
pub mod keyhelp;
pub mod markdown;
pub mod mnanim;
pub mod pixels;
pub mod shimmer;
pub mod slab;
pub mod spinner;
pub mod spring;
pub mod style;
pub mod text;
pub mod textarea;
pub mod toast;
pub mod viewport;
