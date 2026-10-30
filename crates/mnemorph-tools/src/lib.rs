//! The tools: read, index, write, edit, glob, grep, bash, git; the shell classifier; secret
//! injection; the MCP client and the tools it offers.

pub mod attic;
mod beneath;
pub mod classify;
pub mod files;
pub mod fingerprint;
mod forgive;
pub mod gate;
mod index;
pub mod mcp;
pub mod patch;
mod pinned;
pub mod shell;
pub mod tool;
mod tripwire;

pub use attic::{Act, Attic, Conflict, Held, Restore, Snapshot, State};
pub use classify::{
    Place, Push, Rule, Verdict, classify, classify_acts, classify_reach, remote_name,
};
pub use fingerprint::Fingerprint;
pub use gate::{Answer, Ask, Gate, NoHuman, Pending, Terminal};
/// What a sandboxed command may touch, for a harness process the tools do not start.
pub use mnemorph_sandbox::Policy;
/// The sandbox a context runs commands in, as the context takes it.
pub use mnemorph_sandbox::Sandbox;
pub use pinned::remove_created;
pub use tool::{Context, Grant, Meta, Outcome, OwnerOf, Tool, call, ignored, registry};
pub use tripwire::{kijin_witness, queue_witness, usage_witness};
