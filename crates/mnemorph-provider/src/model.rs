//! The provider-neutral conversation: what a body sends and what comes back.

use mnemorph_ledger::Usage;
use serde_json::Value;

/// Who speaks a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The human, or the harness on its behalf (tool results included).
    User,
    /// The model.
    Assistant,
    /// An operator note appended mid-conversation — a change rides here instead of rewriting the
    /// prefix, so the request only ever grows.
    System,
}

/// One block of a message.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Text. `cache` marks a prompt-cache breakpoint after it.
    Text {
        /// The text.
        text: String,
        /// A cache breakpoint ends here.
        cache: bool,
    },
    /// A tool call the model made.
    ToolUse {
        /// Its id, echoed by the result.
        id: String,
        /// The tool's name.
        name: String,
        /// The arguments; `{}` for a call with none.
        input: Value,
    },
    /// The result of a tool call.
    ToolResult {
        /// The call it answers.
        tool_use_id: String,
        /// What the tool returned.
        content: String,
        /// The call failed.
        is_error: bool,
        /// A cache breakpoint ends here.
        cache: bool,
    },
    /// A block the harness must carry back exactly as it came — thinking blocks above all, whose
    /// signature binds them to the conversation. Never edited, never rebuilt.
    Opaque(Value),
}

/// One message of the conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// Who speaks.
    pub role: Role,
    /// What is said.
    pub content: Vec<Block>,
}

/// A tool the model may call.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// Its name.
    pub name: String,
    /// What it does, for the model.
    pub description: String,
    /// Its arguments, as JSON Schema.
    pub input_schema: Value,
    /// A cache breakpoint ends here (put it on the last tool: the tools tier is cached first).
    pub cache: bool,
}

/// How hard the model thinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    /// Least.
    Low,
    /// The default on some models.
    Medium,
    /// More.
    High,
    /// More still.
    XHigh,
    /// Most.
    Max,
}

impl Effort {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

/// One request to a model.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// The model id.
    pub model: String,
    /// The system prompt, in blocks (breakpoints between the crest, the race card, the creature
    /// card).
    pub system: Vec<Block>,
    /// The tools, in a stable order.
    pub tools: Vec<Tool>,
    /// The conversation.
    pub messages: Vec<Message>,
    /// The output ceiling.
    pub max_tokens: u32,
    /// Thinking depth; the model's default when `None`.
    pub effort: Option<Effort>,
}

impl Request {
    /// Prompt-cache breakpoints the request carries.
    #[must_use]
    pub fn breakpoints(&self) -> usize {
        let block = |b: &Block| {
            matches!(
                b,
                Block::Text { cache: true, .. } | Block::ToolResult { cache: true, .. }
            )
        };
        self.tools.iter().filter(|t| t.cache).count()
            + self.system.iter().filter(|b| block(b)).count()
            + self
                .messages
                .iter()
                .flat_map(|m| &m.content)
                .filter(|b| block(b))
                .count()
    }
}

/// Why the model stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// It finished its turn.
    EndTurn,
    /// It wants tools run.
    ToolUse,
    /// It hit `max_tokens`.
    MaxTokens,
    /// It hit a stop sequence.
    StopSequence,
    /// A server-side tool paused the turn; send the conversation back to continue.
    PauseTurn,
    /// A safety classifier declined; `content` is not to be trusted as an answer.
    Refusal,
    /// Any other reason, as given.
    Other(String),
}

/// A complete answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    /// The provider's message id.
    pub id: String,
    /// The model that answered.
    pub model: String,
    /// What it said, in order.
    pub content: Vec<Block>,
    /// Why it stopped.
    pub stop: StopReason,
    /// The provider's details on the stop — on a refusal, its category and any recommended
    /// model; a fallback chain reads it.
    pub stop_details: Option<serde_json::Value>,
    /// Tokens, by class.
    pub usage: Usage,
    /// Things the ledger should know that are not errors (e.g. a usage class Mnemorph does not
    /// price).
    pub warnings: Vec<String>,
}

/// What a stream reports as it arrives, for the surfaces.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A piece of text.
    Text(String),
    /// The model started a tool call.
    ToolStart {
        /// The call's id.
        id: String,
        /// The tool.
        name: String,
    },
}
