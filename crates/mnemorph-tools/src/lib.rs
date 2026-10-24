//! The tools: read, write, edit, glob, grep, bash, git; the shell classifier; secret injection.

pub mod classify;

pub use classify::{Place, Verdict, classify};
