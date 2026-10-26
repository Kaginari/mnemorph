//! The tools: read, write, edit, glob, grep, bash, git; the shell classifier; secret injection.

pub mod attic;
mod beneath;
pub mod classify;
pub mod files;
pub mod gate;
pub mod patch;
pub mod shell;
pub mod tool;
mod tripwire;

pub use attic::{Attic, Snapshot};
pub use classify::{Place, Rule, Verdict, classify};
pub use gate::{Answer, Ask, Gate, NoHuman, Pending, Terminal};
/// The sandbox a context runs commands in, as the context takes it.
pub use mnemorph_sandbox::Sandbox;
pub use tool::{Context, Meta, Outcome, OwnerOf, Tool, call, ignored, registry};
pub use tripwire::queue_witness;
