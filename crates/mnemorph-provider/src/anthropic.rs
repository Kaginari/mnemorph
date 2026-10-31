//! The Anthropic Messages API, over raw HTTP and server-sent events.

use std::sync::Arc;

use mnemorph_ledger::Usage;
use serde_json::{Map, Value, json};

use crate::drive::{Assembler, drive};
use crate::error::Error;
use crate::model::{Block, Event, Message, Request, Response, Role, StopReason};
use crate::retry::Policy;
use crate::transport::Transport;

/// The API version every request names.
pub const API_VERSION: &str = "2026-10-21";
/// The most prompt-cache breakpoints one request may carry.
pub const MAX_BREAKPOINTS: usize = 4;

/// Whether `model` takes native deferred tools (`defer_loading`, expanded from a
/// `tool_reference`): the models Anthropic's tool-search documentation lists — Claude Opus and
/// Sonnet from 4.5, Haiku from 4.5, and every later family. Claude Opus 4.1 and earlier, and any id
/// not known here, do not: their MCP levels stay client-side. The list is the documentation's,
/// enumerated: a new model not on it runs client-side until it is added.
#[must_use]
pub fn defers(model: &str) -> bool {
    const FAMILIES: [&str; 11] = [
        "claude-fable-",
        "claude-mythos-",
        "claude-opus-5",
        "claude-sonnet-5",
        "claude-haiku-5",
        "claude-opus-4-5",
        "claude-opus-4-6",
        "claude-opus-4-7",
        "claude-opus-4-8",
        "claude-sonnet-4-5",
        "claude-sonnet-4-6",
    ];
    let model = model.strip_prefix("anthropic/").unwrap_or(model);
    FAMILIES.iter().any(|f| model.starts_with(f)) || model.starts_with("claude-haiku-4-5")
}

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
    check_system_messages(&request.messages)?;
    let mut body = Map::new();
    body.insert("model".into(), json!(request.model));
    body.insert("max_tokens".into(), json!(request.max_tokens));
    body.insert("stream".into(), json!(true));
    if !request.system.is_empty() {
        body.insert(
            "system".into(),
            Value::Array(request.system.iter().map(|b| block(b, true)).collect()),
        );
    }
    // A model without native deferral — a run that switched to one mid-session keeps its frame —
    // is sent no deferred tool and a reference's text, as any other provider is.
    let native = defers(&request.model);
    let sent: Vec<&crate::model::Tool> = request
        .tools
        .iter()
        .filter(|t| native || !t.defer)
        .collect();
    if !sent.is_empty() {
        if sent.iter().all(|t| t.defer) {
            return Err(Error::Invalid(
                "every tool is deferred; at least one must load up front".into(),
            ));
        }
        let tools = sent.iter().map(|t| {
            // Streamed as generated: a long input (a file's contents) arrives in pieces, never as a
            // silent gap that the stall wait would read as a dead stream.
            let mut tool = json!({"name": t.name, "description": t.description, "input_schema": t.input_schema, "eager_input_streaming": true});
            if t.defer {
                // Out of the context — and so of the cached prefix — until a tool result names it.
                // A deferred tool may not carry a breakpoint: the API refuses the pair.
                tool["defer_loading"] = json!(true);
            } else if t.cache {
                tool["cache_control"] = json!({"type": "ephemeral"});
            }
            tool
        });
        body.insert("tools".into(), Value::Array(tools.collect()));
    }
    body.insert(
        "messages".into(),
        Value::Array(
            request
                .messages
                .iter()
                .map(|m| message(m, native))
                .collect(),
        ),
    );
    if let Some(effort) = request.effort {
        body.insert("output_config".into(), json!({"effort": effort.word()}));
    }
    Ok(Value::Object(body))
}

/// The API's rules for a mid-conversation system message: never first, after a user message,
/// last or followed by an assistant turn, text only. Broken, they cost a 400 and the turn.
fn check_system_messages(messages: &[Message]) -> Result<(), Error> {
    for (i, m) in messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == Role::System)
    {
        let refuse = |why: &str| Err(Error::Invalid(format!("system message {i}: {why}")));
        if i == 0 {
            return refuse("a system message cannot open the conversation");
        }
        if messages[i - 1].role != Role::User {
            return refuse("it must follow a user message");
        }
        if messages
            .get(i + 1)
            .is_some_and(|next| next.role != Role::Assistant)
        {
            return refuse("it must be last, or followed by an assistant turn");
        }
        if !m.content.iter().all(|b| matches!(b, Block::Text { .. })) {
            return refuse("it may hold text only");
        }
    }
    Ok(())
}

fn message(m: &Message, native: bool) -> Value {
    let role = match m.role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    };
    // Another provider's opaque blocks are not this API's to read: left out.
    let content: Vec<Value> = m
        .content
        .iter()
        .filter(|b| !matches!(b, Block::Opaque(v) if v["type"].as_str().is_some_and(|t| t.starts_with(crate::openai::OPAQUE_PREFIX))))
        .map(|b| block(b, native))
        .collect();
    json!({"role": role, "content": content})
}

fn block(b: &Block, native: bool) -> Value {
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
            references,
        } => {
            // A result that loads deferred tools is their references alone: the API expands each
            // into its definition, inline, and the cached prefix never moves.
            let content = if references.is_empty() || !native {
                json!(content)
            } else {
                Value::Array(
                    references
                        .iter()
                        .map(|name| json!({"type": "tool_reference", "tool_name": name}))
                        .collect(),
                )
            };
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
    stop_details: Option<Value>,
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
                let index = index(&v)?;
                let cb = &v["content_block"];
                let partial = start_block(cb, on, started);
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
                        if !piece.is_empty() {
                            *started = true;
                            text.push_str(piece);
                            on(Event::Text(piece.to_owned()));
                        }
                    }
                    (Partial::Tool { id, json, .. }, "input_json_delta") => {
                        let piece = delta["partial_json"].as_str().unwrap_or_default();
                        json.push_str(piece);
                        if !piece.is_empty() {
                            on(Event::ToolInput {
                                id: id.clone(),
                                json: piece.to_owned(),
                            });
                        }
                    }
                    // Thinking, signatures, server tools and anything newer: assembled into the
                    // block, which goes back untouched — thinking's text shown as it comes.
                    (Partial::Opaque(block), kind) => {
                        if kind == "thinking_delta"
                            && let Some(piece) =
                                delta["thinking"].as_str().filter(|p| !p.is_empty())
                        {
                            on(Event::Thinking(piece.to_owned()));
                        }
                        opaque_delta(block, kind, delta);
                    }
                    (_, other) => {
                        let note = format!("a {other} delta on block {index} was not kept");
                        if !self.warnings.contains(&note) {
                            self.warnings.push(note);
                        }
                    }
                }
            }
            "message_delta" => {
                if let Some(reason) = v["delta"]["stop_reason"].as_str() {
                    self.stop = Some(stop_reason(reason));
                }
                if v["delta"]["stop_details"].is_object() {
                    self.stop_details = Some(v["delta"]["stop_details"].clone());
                }
                self.read_usage(&v["usage"]);
            }
            "message_stop" => return Ok(Some(self.finish())),
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
            let note = "1-hour cache writes reported: Mnemorph uses the 5-minute cache and prices every write at 1.25×".to_owned();
            if !self.warnings.contains(&note) {
                self.warnings.push(note);
            }
        }
    }

    fn finish(&mut self) -> Response {
        let stop = self
            .stop
            .take()
            .unwrap_or_else(|| StopReason::Other(String::new()));
        let mut blocks = std::mem::take(&mut self.blocks);
        blocks.sort_by_key(|(i, _)| *i);
        let mut content = Vec::with_capacity(blocks.len());
        for (_, partial) in blocks {
            match partial {
                Partial::Text(text) => content.push(Block::Text { text, cache: false }),
                Partial::Tool { id, name, json } => {
                    // A call with no arguments streams no input at all: that is `{}`.
                    let input = if json.trim().is_empty() {
                        Ok(json!({}))
                    } else {
                        serde_json::from_str(&json)
                    };
                    match input {
                        Ok(input) => content.push(Block::ToolUse { id, name, input }),
                        // Cut by the output ceiling: the call never finished, so it is dropped.
                        Err(_) if stop == StopReason::MaxTokens => {
                            self.warnings.push(format!(
                                "tool call {name} was cut by max_tokens and dropped"
                            ));
                        }
                        // The model's slip, not the stream's: kept under `MALFORMED`, for the loop
                        // to answer with an error the model can correct.
                        Err(e) => content.push(Block::ToolUse {
                            id,
                            name,
                            input: json!({ crate::MALFORMED: json, "error": e.to_string() }),
                        }),
                    }
                }
                Partial::Opaque(mut v) => {
                    if let Some(partial) = v.as_object_mut().and_then(|o| o.remove(PARTIAL_JSON)) {
                        let text = partial.as_str().unwrap_or_default();
                        v["input"] = serde_json::from_str(text).unwrap_or_else(|_| json!({}));
                    }
                    content.push(Block::Opaque(v));
                }
            }
        }
        if stop == StopReason::Refusal && !content.is_empty() {
            // A refusal mid-stream: what came before it is not an answer to keep.
            content.clear();
            self.warnings
                .push("refusal: the partial content was discarded".to_owned());
        }
        Response {
            id: std::mem::take(&mut self.id),
            model: std::mem::take(&mut self.model),
            content,
            stop,
            stop_details: self.stop_details.take(),
            usage: self.usage,
            warnings: std::mem::take(&mut self.warnings),
        }
    }
}

/// Where an opaque block's streamed JSON input collects until the block is finished.
const PARTIAL_JSON: &str = "__partial_json";

/// One delta on an opaque block: strings append to their field, `partial_json` collects into the
/// block's `input`, objects append to an array named after their field (`citation` →
/// `citations`).
/// The partial a `content_block_start` opens: a call announced to the surfaces as it starts,
/// reasoning announced as it begins.
fn start_block(cb: &Value, on: &mut (dyn FnMut(Event) + Send), started: &mut bool) -> Partial {
    match cb["type"].as_str().unwrap_or("") {
        "text" => Partial::Text(cb["text"].as_str().unwrap_or_default().to_owned()),
        "tool_use" => {
            let id = cb["id"].as_str().unwrap_or_default().to_owned();
            let name = cb["name"].as_str().unwrap_or_default().to_owned();
            *started = true;
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
        kind => {
            // Reasoning begins: a surface shows it as it comes, its text or not.
            if kind == "thinking" || kind == "redacted_thinking" {
                on(Event::Thinking(
                    cb["thinking"].as_str().unwrap_or_default().to_owned(),
                ));
            }
            Partial::Opaque(cb.clone())
        }
    }
}

fn opaque_delta(block: &mut Value, kind: &str, delta: &Value) {
    let (Some(slot), Some(fields)) = (block.as_object_mut(), delta.as_object()) else {
        return;
    };
    for (field, value) in fields.iter().filter(|(f, _)| *f != "type") {
        if kind == "input_json_delta" && field == "partial_json" {
            let entry = slot.entry(PARTIAL_JSON).or_insert_with(|| json!(""));
            *entry = json!(format!(
                "{}{}",
                entry.as_str().unwrap_or_default(),
                value.as_str().unwrap_or_default()
            ));
        } else if let Some(piece) = value.as_str() {
            let entry = slot.entry(field.clone()).or_insert_with(|| json!(""));
            *entry = json!(format!("{}{piece}", entry.as_str().unwrap_or_default()));
        } else if value.is_object() {
            let list = slot.entry(format!("{field}s")).or_insert_with(|| json!([]));
            if let Some(list) = list.as_array_mut() {
                list.push(value.clone());
            }
        }
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
