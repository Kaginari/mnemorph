//! OpenAI-compatible chat completions — vLLM, OpenRouter, Ollama and the like — over SSE.

use std::sync::Arc;

use mnemorph_ledger::Usage;
use serde_json::{Map, Value, json};

use crate::drive::{Assembler, api_error, drive};
use crate::error::Error;
use crate::model::{Block, Event, Request, Response, Role, StopReason};
use crate::retry::Policy;
use crate::transport::Transport;

/// The key a tool call's input carries when the model's arguments were no JSON: the raw text,
/// beside `error`, for the loop to hand back as the call's error.
pub const MALFORMED: &str = "__malformed_arguments";

/// Opaque blocks this client produces carry a type with this prefix; it replays them, and every
/// other provider leaves them out.
pub const OPAQUE_PREFIX: &str = "openai.";

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

/// The chat-completions body. Cache marks become one automatic breakpoint for Claude, and are
/// dropped for the rest (those servers cache a stable prefix on their own); opaque
/// blocks from another provider are left out, this client's own are replayed. A mid-conversation
/// system note joins the user's turn as an operator note: many chat templates reject a system
/// role anywhere but first.
#[must_use]
pub fn encode_chat(request: &Request) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    let system: Vec<&str> = request.system.iter().filter_map(text_of).collect();
    if !system.is_empty() {
        messages.push(json!({"role": "system", "content": system.join("\n\n")}));
    }
    for m in &request.messages {
        match m.role {
            Role::System => {
                let note = format!("[operator note] {}", texts(&m.content));
                match messages.last_mut() {
                    Some(last) if last["role"] == "user" && last["content"].is_string() => {
                        let joined =
                            format!("{}\n\n{note}", last["content"].as_str().unwrap_or_default());
                        last["content"] = json!(joined);
                    }
                    _ => messages.push(json!({"role": "user", "content": note})),
                }
            }
            Role::Assistant => messages.push(assistant(&m.content)),
            Role::User => messages.extend(user(&m.content)),
        }
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(request.model));
    body.insert("max_tokens".into(), json!(request.max_tokens));
    body.insert("stream".into(), json!(true));
    body.insert("stream_options".into(), json!({"include_usage": true}));
    body.insert("messages".into(), Value::Array(messages));
    // Claude behind a gateway caches only what is marked: one automatic breakpoint that moves
    // with the conversation. The others cache a stable prefix on their own.
    if request.breakpoints() > 0 && marks_cache(&request.model) {
        body.insert("cache_control".into(), json!({"type": "ephemeral"}));
    }
    if !request.tools.is_empty() {
        let tools = request.tools.iter().map(|t| {
            json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.input_schema}})
        });
        body.insert("tools".into(), Value::Array(tools.collect()));
    }
    Value::Object(body)
}

/// Whether a chat-completions model caches only what is marked: Claude, by any gateway's name.
fn marks_cache(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.starts_with("anthropic/") || m.contains("claude")
}

fn text_of(b: &Block) -> Option<&str> {
    match b {
        Block::Text { text, .. } => Some(text),
        _ => None,
    }
}

fn texts(content: &[Block]) -> String {
    content
        .iter()
        .filter_map(text_of)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// This client's own opaque block of `kind`, if `b` is one.
fn own<'a>(b: &'a Block, kind: &str) -> Option<&'a Value> {
    match b {
        Block::Opaque(v) if v["type"].as_str() == Some(&format!("{OPAQUE_PREFIX}{kind}")) => {
            Some(v)
        }
        _ => None,
    }
}

fn assistant(content: &[Block]) -> Value {
    let text: Vec<&str> = content.iter().filter_map(text_of).collect();
    let mut msg = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text.join("")) }});
    if let Some(r) = content.iter().find_map(|b| own(b, "reasoning_content")) {
        msg["reasoning_content"] = r["text"].clone();
    }
    if let Some(r) = content.iter().find_map(|b| own(b, "reasoning_details")) {
        msg["reasoning_details"] = r["items"].clone();
    }
    let extras: Vec<&Value> = content
        .iter()
        .filter_map(|b| own(b, "tool_call_extra"))
        .collect();
    let calls: Vec<Value> = content
        .iter()
        .filter_map(|b| match b {
            Block::ToolUse { id, name, input } => {
                let mut call = json!({"id": id, "type": "function", "function": {"name": name, "arguments": input.to_string()}});
                // Fields the server attached to this call (a Gemini thought signature, …) go back.
                if let Some(extra) = extras.iter().find(|e| e["id"] == json!(id)) {
                    for (k, v) in extra["fields"].as_object().into_iter().flatten() {
                        call[k] = v.clone();
                    }
                }
                Some(call)
            }
            _ => None,
        })
        .collect();
    if !calls.is_empty() {
        msg["tool_calls"] = Value::Array(calls);
    }
    msg
}

fn user(content: &[Block]) -> Vec<Value> {
    // Tool results become `tool` messages, one per call, before any text the user adds.
    let mut out: Vec<Value> = content
        .iter()
        .filter_map(|b| match b {
            Block::ToolResult {
                tool_use_id,
                content,
                is_error,
                ..
            } => {
                let content = if *is_error {
                    format!("error: {content}")
                } else {
                    content.clone()
                };
                Some(json!({"role": "tool", "tool_call_id": tool_use_id, "content": content}))
            }
            _ => None,
        })
        .collect();
    let text = texts(content);
    if !text.is_empty() {
        out.push(json!({"role": "user", "content": text}));
    }
    out
}

/// A tool call being assembled from its streamed fragments.
#[derive(Debug, Default)]
struct Call {
    id: String,
    name: String,
    arguments: String,
    extra: Map<String, Value>,
    announced: bool,
}

/// The fields of a streamed tool call the client reads itself; any other is the server's own and
/// goes back with the call.
const KNOWN_CALL_FIELDS: [&str; 4] = ["index", "id", "type", "function"];

/// The answer, assembled chunk by chunk.
#[derive(Debug, Default)]
pub(crate) struct Chat {
    id: String,
    model: String,
    text: String,
    reasoning: String,
    reasoning_details: Vec<Value>,
    calls: Vec<(Option<u64>, Call)>,
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
            return Ok(Some(self.finish()));
        }
        let v: Value = serde_json::from_str(data)
            .map_err(|e| Error::Decode(format!("{e}: {}", &data[..data.len().min(200)])))?;
        if v.get("error").is_some() {
            let (kind, message) = api_error(data.as_bytes());
            return Err(Error::Stream { kind, message });
        }
        if let Some(id) = v["id"].as_str() {
            id.clone_into(&mut self.id);
        }
        if let Some(model) = v["model"].as_str() {
            model.clone_into(&mut self.model);
        }
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            // `prompt_tokens` counts every input token; the cached and the newly cached ones are
            // split out so the ledger weighs each class at its own rate.
            let prompt = u["prompt_tokens"].as_u64().unwrap_or(0);
            let details = &u["prompt_tokens_details"];
            let cached = details["cached_tokens"].as_u64().unwrap_or(0);
            let written = details["cache_write_tokens"].as_u64().unwrap_or(0);
            self.usage = Usage {
                input: prompt.saturating_sub(cached).saturating_sub(written),
                output: u["completion_tokens"].as_u64().unwrap_or(0),
                cache_read: cached,
                cache_write: written,
            };
        }
        let Some(choice) = v["choices"].as_array().and_then(|c| c.first()) else {
            return Ok(None);
        };
        let delta = &choice["delta"];
        // Reasoning is kept to replay, never shown: it does not start the answer.
        if let Some(piece) = delta["reasoning_content"]
            .as_str()
            .or_else(|| delta["reasoning"].as_str())
        {
            self.reasoning.push_str(piece);
        }
        if let Some(items) = delta["reasoning_details"].as_array() {
            self.reasoning_details.extend(items.iter().cloned());
        }
        if let Some(piece) = delta["content"].as_str().filter(|p| !p.is_empty()) {
            *started = true;
            self.text.push_str(piece);
            on(Event::Text(piece.to_owned()));
        }
        for fragment in delta["tool_calls"].as_array().into_iter().flatten() {
            self.tool_fragment(fragment, on, started);
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.stop = Some(match reason {
                "stop" => StopReason::EndTurn,
                "tool_calls" | "function_call" => StopReason::ToolUse,
                "length" => StopReason::MaxTokens,
                "content_filter" => StopReason::Refusal,
                // The server failed mid-answer: a failure to retry, never an answer.
                "error" => {
                    return Err(Error::Stream {
                        kind: "finish_reason_error".into(),
                        message: "the server ended the answer with an error".into(),
                    });
                }
                other => StopReason::Other(other.to_owned()),
            });
        }
        Ok(None)
    }
}

impl Chat {
    fn tool_fragment(
        &mut self,
        fragment: &Value,
        on: &mut (dyn FnMut(Event) + Send),
        started: &mut bool,
    ) {
        let index = fragment["index"].as_u64();
        let id = fragment["id"].as_str().filter(|i| !i.is_empty());
        // The call this fragment continues: by index when the server gives one; else by id; else
        // the last call. A new index or a new id starts a call.
        let at = match (index, id) {
            (Some(i), _) => self.calls.iter().position(|(ci, _)| *ci == Some(i)),
            (None, Some(id)) => self.calls.iter().position(|(_, c)| c.id == id),
            (None, None) => self.calls.len().checked_sub(1),
        };
        let at = at.unwrap_or_else(|| {
            self.calls.push((index, Call::default()));
            self.calls.len() - 1
        });
        let call = &mut self.calls[at].1;
        if let Some(id) = id {
            id.clone_into(&mut call.id);
        }
        // A name arrives once; a server that repeats it is not doubled.
        if let Some(name) = fragment["function"]["name"]
            .as_str()
            .filter(|n| !n.is_empty())
            && call.name.is_empty()
        {
            name.clone_into(&mut call.name);
        }
        if let Some(args) = fragment["function"]["arguments"].as_str() {
            call.arguments.push_str(args);
        }
        for (k, v) in fragment.as_object().into_iter().flatten() {
            if !KNOWN_CALL_FIELDS.contains(&k.as_str()) {
                call.extra.insert(k.clone(), v.clone());
            }
        }
        if !call.announced && !call.name.is_empty() {
            call.announced = true;
            *started = true;
            on(Event::ToolStart {
                id: call.id.clone(),
                name: call.name.clone(),
            });
        }
    }

    fn finish(&mut self) -> Response {
        let mut content = Vec::new();
        if !self.reasoning.is_empty() {
            content.push(Block::Opaque(json!({
                "type": format!("{OPAQUE_PREFIX}reasoning_content"),
                "text": std::mem::take(&mut self.reasoning),
            })));
        }
        if !self.reasoning_details.is_empty() {
            content.push(Block::Opaque(json!({
                "type": format!("{OPAQUE_PREFIX}reasoning_details"),
                "items": std::mem::take(&mut self.reasoning_details),
            })));
        }
        if !self.text.is_empty() {
            content.push(Block::Text {
                text: std::mem::take(&mut self.text),
                cache: false,
            });
        }
        let stop = self
            .stop
            .take()
            .unwrap_or_else(|| StopReason::Other(String::new()));
        let mut warnings = Vec::new();
        for (_, call) in std::mem::take(&mut self.calls) {
            // A call with no arguments arrives with an empty string: that is `{}`.
            let input = if call.arguments.trim().is_empty() {
                Ok(json!({}))
            } else {
                serde_json::from_str(&call.arguments)
            };
            let input = match input {
                Ok(input) => input,
                // Cut by the output ceiling: the call never finished, so it is dropped, and said.
                Err(_) if stop == StopReason::MaxTokens => {
                    warnings.push(format!(
                        "tool call {} was cut by max_tokens and dropped",
                        call.name
                    ));
                    continue;
                }
                // A finished call whose arguments are no JSON is the model's slip, not the
                // stream's: kept, with its text under `MALFORMED`, so the loop answers it with an
                // error the model can correct, and one bad call never ends a run.
                Err(e) => json!({ MALFORMED: call.arguments, "error": e.to_string() }),
            };
            if !call.extra.is_empty() {
                content.push(Block::Opaque(json!({
                    "type": format!("{OPAQUE_PREFIX}tool_call_extra"),
                    "id": call.id,
                    "fields": call.extra,
                })));
            }
            content.push(Block::ToolUse {
                id: call.id,
                name: call.name,
                input,
            });
        }
        Response {
            id: std::mem::take(&mut self.id),
            model: std::mem::take(&mut self.model),
            content,
            stop,
            stop_details: None,
            usage: self.usage,
            warnings,
        }
    }
}
