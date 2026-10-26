//! The loop: one body's six beats — perceive, recall, plan, act, verify, record — until it
//! answers, or stops honestly at a meter or an instrument.

pub mod beats;
pub mod budget;
pub mod commission;
pub mod compact;
pub mod council;
pub mod decisions;
pub mod gate;
pub mod model;
pub mod open;
pub mod queue_tool;
pub mod route;
pub mod run;
pub mod sequence;
pub mod session;
pub mod solo;
pub mod stress;
pub mod verify_tool;
pub mod wear_tool;

pub use beats::{Beat, conversation};
pub use budget::{Caps, Left};
pub use commission::{COMMISSION, CommissionTool, Commissioner};
pub use decisions::{BENCH_POINT, Decisions, bench_ask, bench_read, decision_usage, decisions};
pub use gate::{Author, EndGate, GateVerdict, Tree, Verdict, snapshot};
/// The configuration, for a client that opens a session from it.
pub use mnemorph_config::{Config, Paths};
pub use mnemorph_provider::Event as StreamEvent;
pub use mnemorph_provider::OPAQUE_PREFIX;
pub use mnemorph_races::ToolClass;
/// The human gate's types, for a client that answers it.
pub use mnemorph_tools::gate::{Answer as GateAnswer, Ask as GateAsk, Gate};
pub use mnemorph_world as world;
pub use model::{AnthropicModel, Model, OpenAiModel, ScriptedModel};
pub use open::{Bound, Opened, open, open_with};
pub use queue_tool::{QUEUE, QueueTool};
pub use route::{
    Meter, ROUTE_POINT, Reason, Routed, Stop, Work, band, capped, pick, route, route_ask, tier_of,
};
pub use run::{
    Body, Ending, Interrupt, Limits, Observer, Run, SENT, Seen, Setup, Steer, hidden, shown,
};
pub use sequence::validate;
pub use session::{Queue, Session};
pub use solo::{SOLO_PROMPT, configure, solo};
pub use stress::{Bench, Read, Reading, Signal, Stress, bench_of, muted};
