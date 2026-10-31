//! Model providers: Anthropic and OpenAI-compatible streaming clients, retries, cache
//! breakpoints — and System One decisions, with an LLM fallback.
//!
//! [`Anthropic`] speaks the Messages API; [`OpenAiCompatible`] speaks chat completions to vLLM,
//! OpenRouter, Ollama and the like. Both share one attempt loop: the ladder, the waits, and no
//! retry once content has started.
//!
//! A provider takes a provider-neutral [`Request`], streams the answer with three timeouts
//! (connect, first byte, stall), retries what can be retried before any content is shown, and
//! returns a [`Response`] whose usage the ledger prices ([`usage_row`]).
//!
//! [`SystemOne`] asks a typed question of TypeSafe's Jev or a jeff server and gets a probability
//! for every option; [`Decider`] asks it first and falls back to an LLM, and its [`Decision`]
//! always says which one answered and what its probabilities are worth ([`Basis`]).

mod anthropic;
mod decide;
mod drive;
mod error;
mod model;
mod openai;
mod retry;
mod sse;
mod systemone;
mod transport;

pub use anthropic::{API_VERSION, Anthropic, MAX_BREAKPOINTS, encode};
pub use decide::{
    ANSWER_TOOL, Basis, Complete, Completion, Cutoffs, Decider, Decision, Fallback, Primary,
    Source, answer_schema, llm_answers, llm_request,
};
pub use error::{Error, Wait};
pub use model::{Block, Effort, Event, Message, Request, Response, Role, StopReason, Tool};
pub use openai::{MALFORMED, OPAQUE_PREFIX, OpenAiCompatible, encode_chat};
pub use retry::{Policy, retry_after, retryable};
pub use sse::{SseEvent, SseReader};
pub use systemone::{
    Answer, Ask, MAX_STATE_BYTES, Question, Reply as SystemOneReply, SystemOne,
    decode as systemone_decode, encode as systemone_encode,
};
pub use transport::{Body, HttpRequest, HttpResponse, HttpTransport, Reply, Scripted, Transport};

use mnemorph_ledger::{Prices, PricesError, Timestamp, UsageRow};

/// Anthropic's published prices, shipped with the binary.
///
/// # Errors
///
/// The shipped table does not parse — a build defect, caught by its test.
pub fn anthropic_prices() -> Result<Prices, PricesError> {
    Prices::parse(include_str!("../prices/anthropic.toml"))
}

/// Who made a call, for its usage row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    /// The session.
    pub session: String,
    /// The run.
    pub run: String,
    /// The creature.
    pub body: String,
    /// Its race, by name.
    pub race: String,
    /// The commission, if any.
    pub commission: Option<String>,
    /// Tokens of the fixed per-session prefix in this call's input.
    pub prefix: u64,
}

/// The ledger's row for one answered call, priced.
#[must_use]
pub fn usage_row(
    response: &Response,
    caller: &Caller,
    provider: &str,
    prices: &Prices,
    ts: Timestamp,
) -> UsageRow {
    UsageRow {
        ts,
        session: caller.session.clone(),
        run: caller.run.clone(),
        body: caller.body.clone(),
        race: caller.race.clone(),
        commission: caller.commission.clone(),
        model: response.model.clone(),
        provider: provider.to_owned(),
        usage: response.usage,
        prefix: caller.prefix,
        wt: 0,
        usd: None,
        envelope: None,
        purpose: None,
    }
    .priced(prices)
}
