//! The loop: one body's six beats — perceive, recall, plan, act, verify, record — until it
//! answers, or stops honestly at a meter or an instrument.

pub mod beats;
pub mod gate;
pub mod model;
pub mod run;
pub mod solo;
pub mod stress;

pub use beats::{Beat, conversation};
pub use gate::{EndGate, GateVerdict, Tree, snapshot};
pub use model::{AnthropicModel, Model, OpenAiModel, ScriptedModel};
pub use run::{Body, Ending, Limits, Run, Setup};
pub use solo::{SOLO_PROMPT, solo};
pub use stress::{Signal, Stress};
