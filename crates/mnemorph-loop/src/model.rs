//! The one thing the loop needs from a provider: complete a request, streaming as it goes.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use mnemorph_provider::{Anthropic, Error, Event, OpenAiCompatible, Request, Response};

/// One scripted reply: what a model answers, or how it fails.
pub type Reply = Result<Response, Error>;

/// A boxed future, so models of every kind sit behind one trait object.
pub type Completion<'a> = Pin<Box<dyn Future<Output = Result<Response, Error>> + Send + 'a>>;

/// A model the loop can call.
pub trait Model: Send + Sync {
    /// The provider's name, for the ledger (`anthropic`, `openrouter`, `scripted`).
    fn provider(&self) -> &str;

    /// Whether this provider takes native deferred tools for `model`: then MCP's level 2 is a
    /// reference to a deferred definition, out of the cached prefix. None does by default.
    fn defers(&self, model: &str) -> bool {
        let _ = model;
        false
    }

    /// Send `request`; report shown text and tool starts to `on` as they arrive.
    fn complete<'a>(
        &'a self,
        request: &'a Request,
        on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a>;
}

/// Anthropic's Messages API.
pub struct AnthropicModel(pub Anthropic);

impl Model for AnthropicModel {
    fn provider(&self) -> &'static str {
        "anthropic"
    }

    fn defers(&self, model: &str) -> bool {
        mnemorph_provider::defers(model)
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        Box::pin(self.0.complete(request, on))
    }
}

/// Any OpenAI-compatible server, under the provider name the configuration gave it.
pub struct OpenAiModel {
    /// The client.
    pub client: OpenAiCompatible,
    /// The provider's name in the configuration (`openrouter`, `vllm`, …).
    pub name: String,
}

impl Model for OpenAiModel {
    fn provider(&self) -> &str {
        &self.name
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        Box::pin(self.client.complete(request, on))
    }
}

/// A model that answers from a script, for tests: each call takes the next reply and keeps the
/// request it was given.
#[derive(Default)]
pub struct ScriptedModel {
    replies: Mutex<VecDeque<Result<Response, Error>>>,
    requests: Mutex<Vec<Request>>,
}

impl ScriptedModel {
    /// A reply that answers with `text`.
    ///
    /// # Errors
    ///
    /// Never: the `Result` is the shape a script holds.
    pub fn text(text: &str) -> Result<Response, Error> {
        Ok(scripted(
            vec![mnemorph_provider::Block::Text {
                text: text.into(),
                cache: false,
            }],
            mnemorph_provider::StopReason::EndTurn,
        ))
    }

    /// A reply that calls `name` with `input`.
    ///
    /// # Errors
    ///
    /// Never: the `Result` is the shape a script holds.
    pub fn tool(id: &str, name: &str, input: serde_json::Value) -> Result<Response, Error> {
        Ok(scripted(
            vec![mnemorph_provider::Block::ToolUse {
                id: id.into(),
                name: name.into(),
                input,
            }],
            mnemorph_provider::StopReason::ToolUse,
        ))
    }

    /// A model that gives `replies` in order, then fails.
    #[must_use]
    pub fn new(replies: impl IntoIterator<Item = Result<Response, Error>>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Every request sent so far.
    ///
    /// # Panics
    ///
    /// The lock was poisoned by a panicking test.
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().expect("requests lock").clone()
    }

    /// Append a reply to the script while it runs: a test model that answers from what it was
    /// shown — an id the harness minted — queues its reply here.
    ///
    /// # Panics
    ///
    /// The lock was poisoned by a panicking test.
    pub fn then(&self, reply: Result<Response, Error>) {
        self.replies.lock().expect("replies lock").push_back(reply);
    }
}

impl Model for ScriptedModel {
    fn provider(&self) -> &'static str {
        "scripted"
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        self.requests
            .lock()
            .expect("requests lock")
            .push(request.clone());
        let reply = self
            .replies
            .lock()
            .expect("replies lock")
            .pop_front()
            .unwrap_or_else(|| Err(Error::Invalid("the script ran out of replies".into())));
        if let Ok(r) = &reply {
            for b in &r.content {
                match b {
                    mnemorph_provider::Block::Text { text, .. } => on(Event::Text(text.clone())),
                    mnemorph_provider::Block::ToolUse { id, name, .. } => {
                        on(Event::ToolStart {
                            id: id.clone(),
                            name: name.clone(),
                        });
                    }
                    _ => {}
                }
            }
        }
        Box::pin(async move { reply })
    }
}

fn scripted(
    content: Vec<mnemorph_provider::Block>,
    stop: mnemorph_provider::StopReason,
) -> Response {
    Response {
        id: "scripted".into(),
        model: "scripted".into(),
        content,
        stop,
        stop_details: None,
        usage: mnemorph_ledger::Usage {
            input: 100,
            output: 20,
            cache_read: 0,
            cache_write: 0,
        },
        reported_cost: None,
        warnings: vec![],
    }
}
