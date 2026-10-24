//! The loop: one body's six beats — perceive, recall, plan, act, verify, record — until it
//! answers, or stops honestly at a meter or an instrument.

pub mod beats;
pub mod budget;
pub mod compact;
pub mod decisions;
pub mod gate;
pub mod model;
pub mod open;
pub mod route;
pub mod run;
pub mod sequence;
pub mod session;
pub mod solo;
pub mod stress;

pub use beats::{Beat, conversation};
pub use budget::{Caps, Left};
pub use decisions::{BENCH_POINT, Decisions, bench_ask, bench_read, decision_usage, decisions};
pub use gate::{EndGate, GateVerdict, Tree, snapshot};
/// The configuration, for a client that opens a session from it.
pub use mnemorph_config::{Config, Paths};
pub use mnemorph_provider::Event as StreamEvent;
pub use mnemorph_races::ToolClass;
/// The human gate's types, for a client that answers it.
pub use mnemorph_tools::gate::{Answer as GateAnswer, Ask as GateAsk, Gate};
pub use mnemorph_world as world;
pub use model::{AnthropicModel, Model, OpenAiModel, ScriptedModel};
pub use open::{Bound, Opened, open, open_with};
pub use route::{Meter, ROUTE_POINT, Reason, Routed, Stop, Work, band, pick, route, route_ask};
pub use run::{Body, Ending, Interrupt, Limits, Observer, Run, SENT, Seen, Setup, Steer};
pub use sequence::validate;
pub use session::{Queue, Session};
pub use solo::{SOLO_PROMPT, configure, solo};
pub use stress::{Bench, Read, Signal, Stress, bench_of, muted};
