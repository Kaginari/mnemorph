//! The Anthropic Messages API, over raw HTTP and server-sent events.

use std::sync::Arc;

use serde_json::{Map, Value, json};
use tempest_ledger::Usage;

use crate::drive::{Assembler, drive};
use crate::error::Error;
use crate::model::{Block, Event, Message, Request, Response, Role, StopReason};
use crate::retry::Policy;
use crate::transport::Transport;

/// The API version every request names.
pub const API_VERSION: &str = "2026-10-21";
/// The most prompt-cache breakpoints one request may carry.
pub const MAX_BREAKPOINTS: usize = 4;

/// A client for the Anthropic Messages API.
pub struct Anthropic {
    transport: Arc<dyn Transport>,
    url: String,
    key: String,
    policy: Policy,
}

impl Anthropic {
    /// A client sending to `base_url` (e.g. `https://api.anthropic.com`) with `key`.
    #[must_use]
    pub fn new(
        transport: Arc<dyn Transport>,
        base_url: &str,
        key: String,
        policy: Policy,
    ) -> Anthropic {
        Anthropic {
            transport,
            url: format!("{}/v1/messages", base_url.trim_end_matches('/')),
            key,
            policy,
        }
    }

    /// Sends `request` and streams the answer, reporting text and tool calls to `on` as they
    /// arrive. Retries what can be retried — but never once content has started, so nothing
    /// shown is shown twice.
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
            serde_json::to_vec(&encode(request)?).map_err(|e| Error::Invalid(e.to_string()))?;
        let headers = [
            ("content-type".to_owned(), "application/json".to_owned()),
            ("x-api-key".to_owned(), self.key.clone()),
            ("anthropic-version".to_owned(), API_VERSION.to_owned()),
        ];
        drive::<Assembly>(
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

/// The request's JSON body. Refused locally when the API would refuse it: more than four cache
/// breakpoints, or a system message first. Sampling parameters are never sent (API version
/// 2026-10-21 rejects them).
///
/// # Errors
///
/// The request breaks a rule the API enforces.
pub fn encode(request: &Request) -> Result<Value, Error> {
    let breakpoints = request.breakpoints();
    if breakpoints > MAX_BREAKPOINTS {
        return Err(Error::Invalid(format!(
            "{breakpoints} cache breakpoints; at most {MAX_BREAKPOINTS}"
        )));
    }
    if request
        .messages
        .first()
        .is_some_and(|m| m.role == Role::System)
    {
        return Err(Error::Invalid(
            "a system message cannot open the conversation".into(),
        ));
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(request.model));
    body.insert("max_tokens".into(), json!(request.max_tokens));
    body.insert("stream".into(), json!(true));
    if !request.system.is_empty() {
        body.insert(
            "system".into(),
            Value::Array(request.system.iter().map(block).collect()),
        );
    }
    if !request.tools.is_empty() {
        let tools = request.tools.iter().map(|t| {
            let mut tool = json!({"name": t.name, "description": t.description, "input_schema": t.input_schema});
            if t.cache {
                tool["cache_control"] = json!({"type": "ephemeral"});
            }
            tool
        });
        body.insert("tools".into(), Value::Array(tools.collect()));
    }
    body.insert(
        "messages".into(),
        Value::Array(request.messages.iter().map(message).collect()),
    );
    if let Some(effort) = request.effort {
        body.insert("output_config".into(), json!({"effort": effort.word()}));
    }
    Ok(Value::Object(body))
}

fn message(m: &Message) -> Value {
    let role = match m.role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    };
    json!({"role": role, "content": m.content.iter().map(block).collect::<Vec<_>>()})
}

fn block(b: &Block) -> Value {
    let cached = |mut v: Value, cache: bool| {
        if cache {
            v["cache_control"] = json!({"type": "ephemeral"});
        }
        v
    };
    match b {
        Block::Text { text, cache } => cached(json!({"type": "text", "text": text}), *cache),
        Block::ToolUse { id, name, input } => {
            json!({"type": "tool_use", "id": id, "name": name, "input": input})
        }
        Block::ToolResult {
            tool_use_id,
            content,
            is_error,
            cache,
        } => {
            let mut v =
                json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": content});
            if *is_error {
                v["is_error"] = json!(true);
            }
            cached(v, *cache)
        }
        Block::Opaque(v) => v.clone(),
    }
}

/// A content block being assembled from deltas.
#[derive(Debug)]
enum Partial {
    Text(String),
    Tool {
        id: String,
        name: String,
        json: String,
    },
    Opaque(Value),
}

/// The answer, assembled event by event.
#[derive(Debug, Default)]
struct Assembly {
    id: String,
    model: String,
    blocks: Vec<(usize, Partial)>,
    stop: Option<StopReason>,
    usage: Usage,
    warnings: Vec<String>,
}

impl Assembler for Assembly {
    fn apply(
        &mut self,
        data: &str,
        on: &mut (dyn FnMut(Event) + Send),
        started: &mut bool,
    ) -> Result<Option<Response>, Error> {
        let v: Value = serde_json::from_str(data)
            .map_err(|e| Error::Decode(format!("{e}: {}", &data[..data.len().min(200)])))?;
        match v["type"].as_str().unwrap_or("") {
            "message_start" => {
                v["message"]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .clone_into(&mut self.id);
                v["message"]["model"]
                    .as_str()
                    .unwrap_or_default()
                    .clone_into(&mut self.model);
                self.read_usage(&v["message"]["usage"]);
            }
            "content_block_start" => {
                *started = true;
                let index = index(&v)?;
                let cb = &v["content_block"];
                let partial = match cb["type"].as_str().unwrap_or("") {
                    "text" => Partial::Text(cb["text"].as_str().unwrap_or_default().to_owned()),
                    "tool_use" => {
                        let id = cb["id"].as_str().unwrap_or_default().to_owned();
                        let name = cb["name"].as_str().unwrap_or_default().to_owned();
                        on(Event::ToolStart {
                            id: id.clone(),
                            name: name.clone(),
                        });
                        Partial::Tool {
                            id,
                            name,
                            json: String::new(),
                        }
                    }
                    _ => Partial::Opaque(cb.clone()),
                };
                self.blocks.push((index, partial));
            }
            "content_block_delta" => {
                let index = index(&v)?;
                let delta = &v["delta"];
                let Some((_, partial)) = self.blocks.iter_mut().find(|(i, _)| *i == index) else {
                    return Err(Error::Decode(format!(
                        "a delta for block {index}, which never started"
                    )));
                };
                match (partial, delta["type"].as_str().unwrap_or("")) {
                    (Partial::Text(text), "text_delta") => {
                        let piece = delta["text"].as_str().unwrap_or_default();
                        text.push_str(piece);
                        on(Event::Text(piece.to_owned()));
                    }
                    (Partial::Tool { json, .. }, "input_json_delta") => {
                        json.push_str(delta["partial_json"].as_str().unwrap_or_default());
                    }
                    // Thinking, signatures and anything else: append every string field of the
                    // delta to the same field of the block, which goes back untouched.
                    (Partial::Opaque(block), _) => {
                        if let Some(fields) = delta.as_object() {
                            for (field, value) in fields.iter().filter(|(f, _)| *f != "type") {
                                if let (Some(piece), Some(slot)) =
                                    (value.as_str(), block.as_object_mut())
                                {
                                    let entry =
                                        slot.entry(field.clone()).or_insert_with(|| json!(""));
                                    let joined =
                                        format!("{}{piece}", entry.as_str().unwrap_or_default());
                                    *entry = json!(joined);
                                }
                            }
                        }
                    }
                    (_, other) => {
                        return Err(Error::Decode(format!(
                            "a {other} delta for block {index} of another kind"
                        )));
                    }
                }
            }
            "message_delta" => {
                if let Some(reason) = v["delta"]["stop_reason"].as_str() {
                    self.stop = Some(stop_reason(reason));
                }
                self.read_usage(&v["usage"]);
            }
            "message_stop" => return self.finish().map(Some),
            "error" => {
                return Err(Error::Stream {
                    kind: v["error"]["type"].as_str().unwrap_or("unknown").to_owned(),
                    message: v["error"]["message"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                });
            }
            _ => {} // content_block_stop, ping, and events a newer API adds
        }
        Ok(None)
    }
}

impl Assembly {
    fn read_usage(&mut self, u: &Value) {
        let read = |field: &str| u[field].as_u64();
        if let Some(n) = read("input_tokens") {
            self.usage.input = n;
        }
        if let Some(n) = read("output_tokens") {
            self.usage.output = n;
        }
        if let Some(n) = read("cache_read_input_tokens") {
            self.usage.cache_read = n;
        }
        if let Some(n) = read("cache_creation_input_tokens") {
            self.usage.cache_write = n;
        }
        if u["cache_creation"]["ephemeral_1h_input_tokens"]
            .as_u64()
            .is_some_and(|n| n > 0)
        {
            let note = "1-hour cache writes reported: Tempest uses the 5-minute cache and prices every write at 1.25×".to_owned();
            if !self.warnings.contains(&note) {
                self.warnings.push(note);
            }
        }
    }

    fn finish(&mut self) -> Result<Response, Error> {
        let mut blocks = std::mem::take(&mut self.blocks);
        blocks.sort_by_key(|(i, _)| *i);
        let content = blocks
            .into_iter()
            .map(|(_, p)| match p {
                Partial::Text(text) => Ok(Block::Text { text, cache: false }),
                Partial::Tool { id, name, json } => {
                    // A call with no arguments streams no input at all: that is `{}`.
                    let input = if json.trim().is_empty() {
                        json!({})
                    } else {
                        serde_json::from_str(&json)
                            .map_err(|e| Error::Decode(format!("tool {name}'s input: {e}")))?
                    };
                    Ok(Block::ToolUse { id, name, input })
                }
                Partial::Opaque(v) => Ok(Block::Opaque(v)),
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Response {
            id: std::mem::take(&mut self.id),
            model: std::mem::take(&mut self.model),
            content,
            stop: self
                .stop
                .take()
                .unwrap_or_else(|| StopReason::Other(String::new())),
            usage: self.usage,
            warnings: std::mem::take(&mut self.warnings),
        })
    }
}

fn index(v: &Value) -> Result<usize, Error> {
    v["index"]
        .as_u64()
        .and_then(|i| usize::try_from(i).ok())
        .ok_or_else(|| Error::Decode("a block event without an index".into()))
}

fn stop_reason(word: &str) -> StopReason {
    match word {
        "end_turn" => StopReason::EndTurn,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence,
        "pause_turn" => StopReason::PauseTurn,
        "refusal" => StopReason::Refusal,
        other => StopReason::Other(other.to_owned()),
    }
}
