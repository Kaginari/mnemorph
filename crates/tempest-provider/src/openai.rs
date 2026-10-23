//! OpenAI-compatible chat completions — vLLM, OpenRouter, Ollama and the like — over SSE.

use std::sync::Arc;

use serde_json::{Map, Value, json};
use tempest_ledger::Usage;

use crate::drive::{Assembler, drive};
use crate::error::Error;
use crate::model::{Block, Event, Message, Request, Response, Role, StopReason};
use crate::retry::Policy;
use crate::transport::Transport;

/// A client for an OpenAI-compatible `/v1/chat/completions` endpoint.
pub struct OpenAiCompatible {
    transport: Arc<dyn Transport>,
    url: String,
    key: Option<String>,
    policy: Policy,
}

impl OpenAiCompatible {
    /// A client sending to `base_url` (e.g. `http://localhost:8000` or
    /// `https://openrouter.ai/api`), with a bearer key when the server wants one.
    #[must_use]
    pub fn new(
        transport: Arc<dyn Transport>,
        base_url: &str,
        key: Option<String>,
        policy: Policy,
    ) -> OpenAiCompatible {
        OpenAiCompatible {
            transport,
            url: format!("{}/v1/chat/completions", base_url.trim_end_matches('/')),
            key,
            policy,
        }
    }

    /// Sends `request` and streams the answer, as [`crate::Anthropic::complete`] does.
    ///
    /// # Errors
    ///
    /// The request is invalid, or every attempt failed.
    pub async fn complete(
        &self,
        request: &Request,
        on: &mut (dyn FnMut(Event) + Send),
    ) -> Result<Response, Error> {
        let body =
            serde_json::to_vec(&encode_chat(request)).map_err(|e| Error::Invalid(e.to_string()))?;
        let mut headers = vec![("content-type".to_owned(), "application/json".to_owned())];
        if let Some(key) = &self.key {
            headers.push(("authorization".to_owned(), format!("Bearer {key}")));
        }
        drive::<Chat>(
            self.transport.as_ref(),
            &self.policy,
            &self.url,
            &headers,
            &body,
            on,
        )
        .await
    }
}

/// The chat-completions body. Cache breakpoints are dropped (these servers cache on their own),
/// and so are opaque blocks: they belong to the provider that produced them.
#[must_use]
pub fn encode_chat(request: &Request) -> Value {
    let mut messages = Vec::new();
    let system: Vec<&str> = request.system.iter().filter_map(text_of).collect();
    if !system.is_empty() {
        messages.push(json!({"role": "system", "content": system.join("\n\n")}));
    }
    for m in &request.messages {
        messages.extend(chat_messages(m));
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(request.model));
    body.insert("max_tokens".into(), json!(request.max_tokens));
    body.insert("stream".into(), json!(true));
    body.insert("stream_options".into(), json!({"include_usage": true}));
    body.insert("messages".into(), Value::Array(messages));
    if !request.tools.is_empty() {
        let tools = request.tools.iter().map(|t| {
            json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.input_schema}})
        });
        body.insert("tools".into(), Value::Array(tools.collect()));
    }
    Value::Object(body)
}

fn text_of(b: &Block) -> Option<&str> {
    match b {
        Block::Text { text, .. } => Some(text),
        _ => None,
    }
}

fn chat_messages(m: &Message) -> Vec<Value> {
    match m.role {
        Role::System => {
            let text: Vec<&str> = m.content.iter().filter_map(text_of).collect();
            vec![json!({"role": "system", "content": text.join("\n\n")})]
        }
        Role::Assistant => {
            let text: Vec<&str> = m.content.iter().filter_map(text_of).collect();
            let calls: Vec<Value> = m
                .content
                .iter()
                .filter_map(|b| match b {
                    Block::ToolUse { id, name, input } => Some(json!({
                        "id": id, "type": "function",
                        "function": {"name": name, "arguments": input.to_string()}
                    })),
                    _ => None,
                })
                .collect();
            let mut msg = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text.join("")) }});
            if !calls.is_empty() {
                msg["tool_calls"] = Value::Array(calls);
            }
            vec![msg]
        }
        Role::User => {
            // Tool results become `tool` messages, one per call, before any text the user adds.
            let mut out: Vec<Value> = m
                .content
                .iter()
                .filter_map(|b| match b {
                    Block::ToolResult { tool_use_id, content, is_error, .. } => {
                        let content = if *is_error { format!("error: {content}") } else { content.clone() };
                        Some(json!({"role": "tool", "tool_call_id": tool_use_id, "content": content}))
                    }
                    _ => None,
                })
                .collect();
            let text: Vec<&str> = m.content.iter().filter_map(text_of).collect();
            if !text.is_empty() {
                out.push(json!({"role": "user", "content": text.join("\n\n")}));
            }
            out
        }
    }
}

/// A tool call being assembled from its streamed fragments.
#[derive(Debug, Default)]
struct Call {
    id: String,
    name: String,
    arguments: String,
}

/// The answer, assembled chunk by chunk.
#[derive(Debug, Default)]
pub(crate) struct Chat {
    id: String,
    model: String,
    text: String,
    calls: Vec<(u64, Call)>,
    stop: Option<StopReason>,
    usage: Usage,
}

impl Assembler for Chat {
    fn apply(
        &mut self,
        data: &str,
        on: &mut (dyn FnMut(Event) + Send),
        started: &mut bool,
    ) -> Result<Option<Response>, Error> {
        if data.trim() == "[DONE]" {
            return self.finish().map(Some);
        }
        let v: Value = serde_json::from_str(data)
            .map_err(|e| Error::Decode(format!("{e}: {}", &data[..data.len().min(200)])))?;
        if let Some(error) = v.get("error") {
            return Err(Error::Stream {
                kind: error["type"]
                    .as_str()
                    .or_else(|| error["code"].as_str())
                    .unwrap_or("unknown")
                    .to_owned(),
                message: error["message"].as_str().unwrap_or_default().to_owned(),
            });
        }
        if let Some(id) = v["id"].as_str() {
            id.clone_into(&mut self.id);
        }
        if let Some(model) = v["model"].as_str() {
            model.clone_into(&mut self.model);
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            let prompt = u["prompt_tokens"].as_u64().unwrap_or(0);
            let cached = u["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap_or(0);
            self.usage = Usage {
                input: prompt.saturating_sub(cached),
                output: u["completion_tokens"].as_u64().unwrap_or(0),
                cache_read: cached,
                cache_write: 0,
            };
        }
        let Some(choice) = v["choices"].as_array().and_then(|c| c.first()) else {
            return Ok(None);
        };
        let delta = &choice["delta"];
        if let Some(piece) = delta["content"].as_str().filter(|p| !p.is_empty()) {
            *started = true;
            self.text.push_str(piece);
            on(Event::Text(piece.to_owned()));
        }
        for call in delta["tool_calls"].as_array().into_iter().flatten() {
            *started = true;
            let index = call["index"].as_u64().unwrap_or(0);
            let at = self
                .calls
                .iter()
                .position(|(i, _)| *i == index)
                .unwrap_or_else(|| {
                    self.calls.push((index, Call::default()));
                    self.calls.len() - 1
                });
            let slot = &mut self.calls[at].1;
            if let Some(id) = call["id"].as_str() {
                id.clone_into(&mut slot.id);
            }
            if let Some(name) = call["function"]["name"].as_str() {
                slot.name.push_str(name);
                on(Event::ToolStart {
                    id: slot.id.clone(),
                    name: slot.name.clone(),
                });
            }
            if let Some(args) = call["function"]["arguments"].as_str() {
                slot.arguments.push_str(args);
            }
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.stop = Some(match reason {
                "stop" => StopReason::EndTurn,
                "tool_calls" | "function_call" => StopReason::ToolUse,
                "length" => StopReason::MaxTokens,
                "content_filter" => StopReason::Refusal,
                other => StopReason::Other(other.to_owned()),
            });
        }
        Ok(None)
    }
}

impl Chat {
    fn finish(&mut self) -> Result<Response, Error> {
        let mut content = Vec::new();
        if !self.text.is_empty() {
            content.push(Block::Text {
                text: std::mem::take(&mut self.text),
                cache: false,
            });
        }
        let mut calls = std::mem::take(&mut self.calls);
        calls.sort_by_key(|(i, _)| *i);
        for (_, call) in calls {
            // A call with no arguments arrives with an empty string: that is `{}`.
            let input = if call.arguments.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&call.arguments)
                    .map_err(|e| Error::Decode(format!("tool {}'s arguments: {e}", call.name)))?
            };
            content.push(Block::ToolUse {
                id: call.id,
                name: call.name,
                input,
            });
        }
        Ok(Response {
            id: std::mem::take(&mut self.id),
            model: std::mem::take(&mut self.model),
            content,
            stop: self
                .stop
                .take()
                .unwrap_or_else(|| StopReason::Other(String::new())),
            usage: self.usage,
            warnings: Vec::new(),
        })
    }
}
