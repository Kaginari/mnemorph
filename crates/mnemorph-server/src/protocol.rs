//! The client protocol's types: JSON-RPC 2.0, the events the core sends, and the human gate's
//! request. Field names are camelCase on the wire, as the TypeScript clients read them.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The protocol's version: a client that speaks another refuses to start, instead of
/// misreading.
pub const PROTOCOL: u32 = 1;

/// The most events kept for a client that reconnects and asks for what it missed.
pub const REPLAY: usize = 10_000;

/// The most bytes of a tool's result an event carries: the journal keeps the whole of it.
pub const RESULT_BYTES: usize = 16_000;

/// One line from the client: a request, a notification, or the answer to the core's own request.
#[derive(Debug, Clone, Deserialize)]
pub struct Incoming {
    /// The request's id; absent on a notification.
    #[serde(default)]
    pub id: Option<Value>,
    /// The method; absent on a response.
    #[serde(default)]
    pub method: Option<String>,
    /// The parameters.
    #[serde(default)]
    pub params: Value,
    /// A response's result.
    #[serde(default)]
    pub result: Option<Value>,
    /// A response's error.
    #[serde(default)]
    pub error: Option<Value>,
}

/// What `initialize` answers.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    /// [`PROTOCOL`].
    pub protocol: u32,
    /// The core's version.
    pub version: String,
    /// The world the session works in.
    pub world: String,
    /// The model, as the configuration names it.
    pub model: String,
    /// The provider serving it.
    pub provider: String,
    /// The session's id.
    pub session: String,
    /// The run's id.
    pub run: String,
    /// The configuration's holes: keys it did not take, and why.
    pub holes: Vec<String>,
}

/// An event: sent as the notification `event/<type>`, its fields beside `session`, `run`, `body`
/// and `seq`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    /// A turn began on a line the human sent.
    TurnStarted {
        /// The line.
        text: String,
    },
    /// A line sent while a turn runs: it waits its turn, in order.
    Queued {
        /// The line.
        text: String,
    },
    /// A piece of the model's text, as it streams.
    TextDelta {
        /// The piece.
        text: String,
    },
    /// The model started a tool call; its input follows in `toolCall`.
    ToolStart {
        /// The call's id.
        id: String,
        /// The tool.
        name: String,
    },
    /// The model's turn, whole: its text and why it stopped.
    Assistant {
        /// The text blocks, joined.
        text: String,
        /// Why the model stopped.
        stop: String,
    },
    /// A tool call, with its input.
    ToolCall {
        /// The call's id.
        id: String,
        /// The tool.
        name: String,
        /// Its input.
        input: Value,
    },
    /// What a tool returned.
    ToolResult {
        /// The call it answers.
        id: String,
        /// Its text, cut at [`RESULT_BYTES`].
        content: String,
        /// Whether it failed.
        is_error: bool,
        /// The bytes cut from `content`.
        cut: usize,
    },
    /// One model turn's spend, and the run's totals.
    Usage {
        /// Fresh input tokens.
        input: u64,
        /// Output tokens.
        output: u64,
        /// Tokens read from the cache.
        cache_read: u64,
        /// Tokens written to the cache.
        cache_write: u64,
        /// The turn's price, when known.
        usd: Option<f64>,
        /// The turn's weighted tokens.
        wt: u64,
        /// The run's weighted tokens so far.
        spent_wt: u64,
        /// How full the context is.
        occupancy: u64,
        /// The line the context is measured against.
        stress_line: u64,
    },
    /// A note the loop added to the conversation.
    Note {
        /// The text.
        text: String,
    },
    /// A warning for the human, never shown to the model.
    Warning {
        /// The text.
        text: String,
    },
    /// The end gate's verdict on an answer.
    GateVerdict {
        /// Whether it passed.
        passed: bool,
        /// The paths the run changed.
        changed: Vec<String>,
        /// What failed.
        failures: Vec<String>,
    },
    /// A turn ended.
    TurnEnded {
        /// How, in words: `answered`, `interrupted`, `checkpoint: …`, `failed: …`.
        ending: String,
    },
}

impl Event {
    /// The event's type, as its method names it.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Event::TurnStarted { .. } => "turnStarted",
            Event::Queued { .. } => "queued",
            Event::TextDelta { .. } => "textDelta",
            Event::ToolStart { .. } => "toolStart",
            Event::Assistant { .. } => "assistant",
            Event::ToolCall { .. } => "toolCall",
            Event::ToolResult { .. } => "toolResult",
            Event::Usage { .. } => "usage",
            Event::Note { .. } => "note",
            Event::Warning { .. } => "warning",
            Event::GateVerdict { .. } => "gateVerdict",
            Event::TurnEnded { .. } => "turnEnded",
        }
    }
}

/// The core's request to the human: `gate/ask`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateRequest {
    /// The body asking.
    pub race: String,
    /// The act's class: `write`, `destructive`, `outward`, ….
    pub class: String,
    /// The act as it will run.
    pub act: String,
    /// Why it has its class.
    pub why: Vec<String>,
    /// Paths outside the world it opens.
    pub outside: Vec<String>,
}

/// The human's answer to `gate/ask`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateReply {
    /// Run it, this once.
    pub allow: bool,
    /// Why not, for the body.
    #[serde(default)]
    pub reason: Option<String>,
}
