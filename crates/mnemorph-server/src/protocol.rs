//! The client protocol's types: JSON-RPC 2.0, the events the core sends, and the human gate's
//! request. Field names are camelCase on the wire, as the TypeScript clients read them.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

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
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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
    /// The configuration's holes — keys it did not take, and why — under a cap, the usage
    /// ledger's lines that cannot be read, and each outward act a probe left in `reconcile`.
    pub holes: Vec<String>,
    /// What the session may spend under the human's caps; `null` when they set none.
    pub budget: Option<Budget>,
    /// Where the provider's key comes from — its variable and the file naming it, never the
    /// value; `null` when the provider takes no key.
    pub key: Option<String>,
    /// The sandbox actually in force for the session's commands, or `none — <reason>`.
    pub sandbox: String,
    /// The session's own body: events under any other body come from the bodies it commissioned.
    pub body: String,
    /// The orders held on the shared work queue as the session opens.
    pub held: u32,
}

/// A session's budget under the human's caps.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Budget {
    /// Weighted tokens it may spend; the run stops at a checkpoint when they run out.
    #[ts(type = "number")]
    pub left_wt: u64,
    /// The cap that sets it: `session`, `day` or `month`.
    pub cap: String,
}

/// An event: sent as the notification `event/<type>`, its fields beside `session`, `run`, `body`
/// and `seq`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
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
        #[ts(type = "number")]
        cut: usize,
    },
    /// One model turn's spend, and the run's totals.
    Usage {
        /// Fresh input tokens.
        #[ts(type = "number")]
        input: u64,
        /// Output tokens.
        #[ts(type = "number")]
        output: u64,
        /// Tokens read from the cache.
        #[ts(type = "number")]
        cache_read: u64,
        /// Tokens written to the cache.
        #[ts(type = "number")]
        cache_write: u64,
        /// The turn's price, when known.
        #[ts(type = "number | null")]
        usd: Option<f64>,
        /// The turn's weighted tokens.
        #[ts(type = "number")]
        wt: u64,
        /// The run's weighted tokens so far.
        #[ts(type = "number")]
        spent_wt: u64,
        /// How full the context is.
        #[ts(type = "number")]
        occupancy: u64,
        /// The line the context is measured against.
        #[ts(type = "number")]
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
        /// Whether it passed: every check ran and held.
        passed: bool,
        /// PASS, FAIL or `NOT_VERIFIED`: a check that gave no result is never a pass.
        verdict: String,
        /// The paths the run changed.
        changed: Vec<String>,
        /// What failed.
        failures: Vec<String>,
        /// The checks that gave no result.
        unverified: Vec<String>,
    },
    /// A turn ended.
    TurnEnded {
        /// How, in words: `answered`, `interrupted`, `checkpoint: …`, `failed: …`.
        ending: String,
    },
    /// The orders held on the shared work queue, once the session's turns and their wake are done.
    Held {
        /// How many.
        held: u32,
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
            Event::Held { .. } => "held",
        }
    }
}

/// The core's request to the human: `gate/ask`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GateRequest {
    /// The body asking, by name.
    pub body: String,
    /// Its race.
    pub race: String,
    /// The act's class: `write`, `destructive`, `outward`, ….
    pub class: String,
    /// The act as it will run.
    pub act: String,
    /// Why it has its class.
    pub why: Vec<String>,
    /// Paths outside the world it opens.
    pub outside: Vec<String>,
    /// The plan the approval binds to, in short (`plan 3f9a1c2e4b5d · head 1a2b3c4d · diff
    /// 5d6e7f8a · target 9a8b7c6d`): the command runs only if the world still matches it at the
    /// spawn. Absent for an ask that spawns nothing.
    #[ts(optional = nullable)]
    pub plan: Option<String>,
}

/// The human's answer to `gate/ask`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GateReply {
    /// Run it, this once.
    pub allow: bool,
    /// Why not, for the body.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub reason: Option<String>,
}

/// Every event's envelope, beside its own fields.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Envelope {
    /// The session.
    pub session: String,
    /// The run.
    pub run: String,
    /// The body.
    pub body: String,
    /// The event's number: it rises by one, in the order events reach the wire.
    #[ts(type = "number")]
    pub seq: u64,
}

/// `session/prompt`'s parameters.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct PromptParams {
    /// The line the human sent.
    pub text: String,
}

/// `session/prompt`'s answer.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PromptResult {
    /// Whether it waits behind a turn that runs.
    pub queued: bool,
}

/// `session/events`'s parameters.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct EventsParams {
    /// The last `seq` the client saw.
    #[ts(type = "number")]
    pub since: u64,
}

/// `session/events`'s answer.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct EventsResult {
    /// Each event as it was sent: `{jsonrpc, method, params}`.
    pub events: Vec<Value>,
    /// The oldest `seq` kept, if any.
    #[ts(type = "number | null")]
    pub oldest: Option<u64>,
    /// False when the client asked from further back than is kept.
    pub complete: bool,
}

#[cfg(test)]
mod tests {
    /// The TypeScript package states the same protocol version as the core.
    #[test]
    fn the_typescript_protocol_version_is_the_cores() {
        let ts = include_str!("../../../packages/protocol/src/index.ts");
        assert!(
            ts.contains(&format!("export const PROTOCOL = {};", super::PROTOCOL)),
            "packages/protocol/src/index.ts states another PROTOCOL"
        );
    }
}
