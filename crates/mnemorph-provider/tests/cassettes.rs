//! Real responses, recorded from OpenRouter's free models (`tests/live_openrouter.rs`) and a local
//! Ollama (`tests/live_ollama.rs`) with `MNEMORPH_RECORD`, scrubbed, and replayed here: no key, no
//! network, every run.

use std::sync::Arc;

use mnemorph_ledger::Usage;
use mnemorph_provider::{
    Block, Error, Event, Message, OPAQUE_PREFIX, OpenAiCompatible, Policy, Reply, Request, Role,
    Scripted, StopReason, encode_chat,
};
use serde_json::{Value, json};

#[path = "support/keys.rs"]
mod keys;

// The recordings predate the rename: the tool result the server read, and so its answer, say
// "Tempest". They stay as recorded.
const TEXT: &str = include_str!("cassettes/openrouter-text-0.sse");
const TOOL_CALL: &str = include_str!("cassettes/openrouter-tool-0.sse");
const TOOL_ANSWER: &str = include_str!("cassettes/openrouter-tool-1.sse");
const REASONING: &str = include_str!("cassettes/openrouter-reasoning-0.sse");
const ERROR: &str = include_str!("cassettes/openrouter-error-0.json");
// An over-long prompt, refused before any byte streamed: a 400 status, never a mid-stream error.
const CONTEXT: &str = include_str!("cassettes/openrouter-context-0.json");
// A free model's shared upstream pool, exhausted: met while probing `google/gemma-4-26b-a4b-it:free`
// for a thought signature by hand, its `user_id` scrubbed as the recorder scrubs it. It cannot be
// provoked on demand, so no live test records it again.
const RATE_LIMIT: &str = include_str!("cassettes/openrouter-ratelimit-0.json");
// Ollama (`qwen2.5:0.5b`): usage asked for, usage not asked for, a tool call, an unknown model.
const OLLAMA_TEXT: &str = include_str!("cassettes/ollama-text-0.sse");
const OLLAMA_NO_USAGE: &str = include_str!("cassettes/ollama-nousage-0.sse");
const OLLAMA_TOOL: &str = include_str!("cassettes/ollama-tool-0.sse");
const OLLAMA_ERROR: &str = include_str!("cassettes/ollama-error-0.json");
const ALL: [&str; 11] = [
    TEXT,
    TOOL_CALL,
    TOOL_ANSWER,
    REASONING,
    ERROR,
    CONTEXT,
    RATE_LIMIT,
    OLLAMA_TEXT,
    OLLAMA_NO_USAGE,
    OLLAMA_TOOL,
    OLLAMA_ERROR,
];
const NAMED: [(&str, &str); 11] = [
    ("ollama-text-0.sse", OLLAMA_TEXT),
    ("ollama-nousage-0.sse", OLLAMA_NO_USAGE),
    ("ollama-tool-0.sse", OLLAMA_TOOL),
    ("ollama-error-0.json", OLLAMA_ERROR),
    ("openrouter-text-0.sse", TEXT),
    ("openrouter-tool-0.sse", TOOL_CALL),
    ("openrouter-tool-1.sse", TOOL_ANSWER),
    ("openrouter-reasoning-0.sse", REASONING),
    ("openrouter-error-0.json", ERROR),
    ("openrouter-context-0.json", CONTEXT),
    ("openrouter-ratelimit-0.json", RATE_LIMIT),
];

fn request() -> Request {
    Request {
        model: "nvidia/nemotron-3.5-lightning:free".into(),
        system: vec![],
        tools: vec![],
        messages: vec![Message {
            role: Role::User,
            content: vec![Block::Text {
                text: "go".into(),
                cache: false,
            }],
        }],
        max_tokens: 2048,
        effort: None,
    }
}

async fn replay(cassette: &str) -> (mnemorph_provider::Response, Vec<Event>) {
    let script = Arc::new(Scripted::new([Reply::ok(cassette)]));
    let client = OpenAiCompatible::new(
        script,
        "https://openrouter.ai/api",
        Some("k".into()),
        Policy::default(),
    );
    let mut events = Vec::new();
    let mut on = |e: Event| events.push(e);
    let r = client
        .complete(&request(), &mut on)
        .await
        .expect("a recorded answer replays");
    (r, events)
}

fn recorded_usage(cassette: &str) -> Value {
    cassette
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find_map(|v| v.get("usage").filter(|u| u.is_object()).cloned())
        .expect("the recording carries usage")
}

fn opaque_kinds(content: &[Block]) -> Vec<String> {
    content
        .iter()
        .filter_map(|b| match b {
            Block::Opaque(v) => v["type"].as_str().map(str::to_owned),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_recorded_text_answer() {
    let (r, events) = replay(TEXT).await;
    assert_eq!(r.stop, StopReason::EndTurn);
    let text: String = r
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.to_lowercase().contains("ready"), "{text}");
    // Reasoning streams to a surface as its own event, never as the answer's text.
    assert!(
        events
            .iter()
            .all(|e| matches!(e, Event::Text(_) | Event::Thinking(_))),
        "{events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(e, Event::Thinking(_))),
        "the recorded reasoning is shown as it comes"
    );
    let shown: String = events
        .iter()
        .filter_map(|e| match e {
            Event::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(shown, text, "the text events are the answer, and only it");
    let kinds = opaque_kinds(&r.content);
    assert!(
        kinds.contains(&format!("{OPAQUE_PREFIX}reasoning_content")),
        "{kinds:?}"
    );
    assert!(
        kinds.contains(&format!("{OPAQUE_PREFIX}reasoning_details")),
        "{kinds:?}"
    );
    let u = recorded_usage(TEXT);
    // OpenRouter's own charge, a number of dollars (`0` on a free model), kept beside the usage.
    assert!(u["cost"].is_number(), "{u}");
    assert_eq!(r.reported_cost, u["cost"].as_f64());
    assert_eq!(r.reported_cost, Some(0.0));
    assert_eq!(
        r.usage,
        Usage {
            input: u["prompt_tokens"].as_u64().unwrap()
                - u["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .unwrap(),
            output: u["completion_tokens"].as_u64().unwrap(),
            cache_read: u["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap(),
            cache_write: u["prompt_tokens_details"]["cache_write_tokens"]
                .as_u64()
                .unwrap(),
        }
    );
}

#[tokio::test(start_paused = true)]
async fn a_recorded_tool_round_trip() {
    let (call, events) = replay(TOOL_CALL).await;
    assert_eq!(call.stop, StopReason::ToolUse);
    let (id, input) = call
        .content
        .iter()
        .find_map(|b| match b {
            Block::ToolUse { id, name, input } if name == "read_file" => {
                Some((id.clone(), input.clone()))
            }
            _ => None,
        })
        .expect("the recorded call");
    assert!(
        input["path"].as_str().is_some_and(|p| p.contains("NOTES")),
        "{input}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::ToolStart { .. }))
            .count(),
        1
    );

    // The recorded call, appended and answered, encodes to the shape the server accepted live.
    let mut next = request();
    next.messages.push(Message {
        role: Role::Assistant,
        content: call.content,
    });
    next.messages.push(Message {
        role: Role::User,
        content: vec![Block::ToolResult {
            tool_use_id: id.clone(),
            content: "Tempest is a harness.".into(),
            is_error: false,
            cache: false,
            references: Vec::new(),
        }],
    });
    let body = encode_chat(&next);
    assert_eq!(body["messages"][1]["tool_calls"][0]["id"], json!(id));
    assert!(body["messages"][1]["reasoning_details"].is_array());
    assert_eq!(
        body["messages"][2],
        json!({"role": "tool", "tool_call_id": id, "content": "Tempest is a harness."})
    );

    let (answer, _) = replay(TOOL_ANSWER).await;
    assert_eq!(answer.stop, StopReason::EndTurn);
    assert!(
        answer.content.iter().any(
            |b| matches!(b, Block::Text { text, .. } if text.to_lowercase().contains("tempest"))
        )
    );
}

#[tokio::test(start_paused = true)]
async fn a_recorded_reasoning_turn() {
    let (r, events) = replay(REASONING).await;
    assert_eq!(r.stop, StopReason::EndTurn);
    let thought: String = events
        .iter()
        .filter_map(|e| match e {
            Event::Thinking(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert!(!thought.is_empty(), "the reasoning streams as it comes");
    // Every `reasoning_details` item is kept, in order, to go back with the next turn; on this
    // server their texts are the streamed reasoning itself.
    let details = r
        .content
        .iter()
        .find_map(|b| match b {
            Block::Opaque(v)
                if v["type"].as_str() == Some(&format!("{OPAQUE_PREFIX}reasoning_details")) =>
            {
                v["items"].as_array().cloned()
            }
            _ => None,
        })
        .expect("the reasoning details are kept");
    assert!(
        details.iter().all(|d| d["type"] == "reasoning.text"),
        "{details:?}"
    );
    let joined: String = details.iter().filter_map(|d| d["text"].as_str()).collect();
    assert_eq!(joined, thought);
    let text: String = r
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text.trim().to_lowercase(), "no", "91 is 7 × 13");

    let mut next = request();
    next.messages.push(Message {
        role: Role::Assistant,
        content: r.content,
    });
    let body = encode_chat(&next);
    assert_eq!(body["messages"][1]["reasoning_details"], json!(details));
}

/// OpenRouter's refusal of a request carries a numeric `code`: it is the error's kind, the body's
/// message its message, and a 400 is not retried.
#[tokio::test(start_paused = true)]
async fn a_recorded_refusal() {
    let script = Arc::new(Scripted::new([Reply::status(400, ERROR, &[])]));
    let client = OpenAiCompatible::new(
        script.clone(),
        "https://openrouter.ai/api",
        Some("k".into()),
        Policy::default(),
    );
    let mut on = |_: Event| {};
    let err = client
        .complete(&request(), &mut on)
        .await
        .expect_err("the recorded refusal");
    assert_eq!(
        err,
        Error::Api {
            status: 400,
            kind: "400".into(),
            message: "mnemorph/no-such-model:free is not a valid model ID".into(),
        }
    );
    assert_eq!(script.sent().len(), 1, "a refusal is not retried");
}

/// Every recorded answer's reported cost reaches the ledger's row, beside the estimate.
#[tokio::test(start_paused = true)]
async fn every_recorded_answer_reports_its_cost_to_the_row() {
    for cassette in [TEXT, TOOL_CALL, TOOL_ANSWER, REASONING] {
        let (r, _) = replay(cassette).await;
        let cost = recorded_usage(cassette)["cost"].as_f64();
        assert!(cost.is_some(), "the recording carries usage.cost");
        let row = mnemorph_provider::usage_row(
            &r,
            &mnemorph_provider::Caller {
                session: "s".into(),
                run: "r".into(),
                body: "b".into(),
                race: "slime".into(),
                commission: None,
                prefix: 0,
            },
            "openrouter",
            &mnemorph_ledger::Prices::default(),
            mnemorph_ledger::Timestamp(0),
        );
        assert_eq!(row.reported_cost, cost);
    }
}

/// A prompt over the model's context is refused before the stream: a 400, its message the
/// server's, not retried.
#[tokio::test(start_paused = true)]
async fn a_recorded_context_overflow() {
    let script = Arc::new(Scripted::new([Reply::status(400, CONTEXT, &[])]));
    let client = OpenAiCompatible::new(
        script.clone(),
        "https://openrouter.ai/api",
        Some("k".into()),
        Policy::default(),
    );
    let mut on = |_: Event| {};
    let err = client
        .complete(&request(), &mut on)
        .await
        .expect_err("the recorded refusal");
    let Error::Api {
        status: 400,
        kind,
        message,
    } = &err
    else {
        panic!("{err:?}");
    };
    assert_eq!(kind, "400");
    assert!(
        message.contains("maximum context length is 65536"),
        "{message}"
    );
    assert_eq!(script.sent().len(), 1, "an over-long prompt is not retried");
}

/// An upstream rate limit: OpenRouter's message says only "Provider returned error", the reason
/// is the upstream's `metadata.raw`, joined to it; a 429 is retried.
#[tokio::test(start_paused = true)]
async fn a_recorded_upstream_rate_limit() {
    let script = Arc::new(Scripted::new([
        Reply::status(429, RATE_LIMIT, &[]),
        Reply::status(429, RATE_LIMIT, &[]),
    ]));
    let client = OpenAiCompatible::new(
        script.clone(),
        "https://openrouter.ai/api",
        Some("k".into()),
        Policy {
            max_retries: 1,
            ..Policy::default()
        },
    );
    let mut on = |_: Event| {};
    let err = client
        .complete(&request(), &mut on)
        .await
        .expect_err("the recorded rate limit");
    let Error::Api {
        status: 429,
        kind,
        message,
    } = &err
    else {
        panic!("{err:?}");
    };
    assert_eq!(kind, "429");
    assert!(
        message.starts_with("Provider returned error: google/gemma-4-26b-a4b-it:free is temporarily rate-limited upstream"),
        "{message}"
    );
    assert!(err.retryable());
    assert_eq!(script.sent().len(), 2, "a 429 is retried");
}

async fn replay_ollama(cassette: &str) -> (mnemorph_provider::Response, Vec<Event>) {
    let script = Arc::new(Scripted::new([Reply::ok(cassette)]));
    let client = OpenAiCompatible::new(script, "http://127.0.0.1:11434", None, Policy::default());
    let mut events = Vec::new();
    let mut on = |e: Event| events.push(e);
    let mut req = request();
    req.model = "qwen2.5:0.5b".into();
    let r = client
        .complete(&req, &mut on)
        .await
        .expect("a recorded answer replays");
    (r, events)
}

fn ollama_row(r: &mnemorph_provider::Response) -> mnemorph_ledger::UsageRow {
    let prices = mnemorph_ledger::Prices::parse(
        "[models.\"qwen2.5:0.5b\"]\ninput = 1.0\noutput = 1.0\ncache-read = 1.0\ncache-write = 1.0",
    )
    .unwrap();
    mnemorph_provider::usage_row(
        r,
        &mnemorph_provider::Caller {
            session: "s".into(),
            run: "r".into(),
            body: "b".into(),
            race: "slime".into(),
            commission: None,
            prefix: 0,
        },
        "ollama",
        &prices,
        mnemorph_ledger::Timestamp(0),
    )
}

/// Ollama sends usage only when `stream_options.include_usage` asks for it: in a last chunk with
/// no choices, after the finish; its cached tokens are split out like any server's.
#[tokio::test(start_paused = true)]
async fn a_recorded_ollama_answer_with_usage() {
    let (r, events) = replay_ollama(OLLAMA_TEXT).await;
    assert_eq!(r.stop, StopReason::EndTurn);
    assert!(
        events.iter().all(|e| matches!(e, Event::Text(_))),
        "{events:?}"
    );
    let u = recorded_usage(OLLAMA_TEXT);
    let cached = u["prompt_tokens_details"]["cached_tokens"]
        .as_u64()
        .unwrap();
    assert!(r.usage_known);
    assert_eq!(
        r.usage,
        Usage {
            input: u["prompt_tokens"].as_u64().unwrap() - cached,
            output: u["completion_tokens"].as_u64().unwrap(),
            cache_read: cached,
            cache_write: 0,
        }
    );
    assert_eq!(r.reported_cost, None, "Ollama reports no cost");
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    let row = ollama_row(&r);
    assert!(row.usd.is_some() && row.wt > 0, "{row:?}");
    assert_eq!(row.reported_cost, None);
}

/// Without `stream_options` Ollama sends no usage at all: the tokens are unknown, said so, and the
/// call is ledgered unpriced — never as a free call.
#[tokio::test(start_paused = true)]
async fn a_recorded_ollama_answer_without_usage() {
    assert!(!OLLAMA_NO_USAGE.contains("\"usage\""));
    let (r, _) = replay_ollama(OLLAMA_NO_USAGE).await;
    assert_eq!(r.stop, StopReason::EndTurn);
    assert!(!r.usage_known);
    assert_eq!(r.warnings, [mnemorph_provider::NO_USAGE]);
    // The estimate, at its worst: every byte sent as input, every byte answered as output.
    let text: usize = r
        .content
        .iter()
        .map(|b| match b {
            Block::Text { text, .. } => text.len(),
            _ => 0,
        })
        .sum();
    assert_eq!(
        r.usage.output,
        u64::try_from(text.div_ceil(4)).unwrap().max(1)
    );
    assert!(
        r.usage.input > 0 && r.usage.cache_read == 0,
        "{:?}",
        r.usage
    );
    let row = ollama_row(&r);
    assert_eq!(row.usd, None, "unknown tokens are unpriced, never $0");
    assert!(row.estimated);
    assert!(row.wt > 0, "the estimate is charged: {row:?}");
}

/// Ollama streams a tool call whole, in one chunk: its id, name and complete arguments.
#[tokio::test(start_paused = true)]
async fn a_recorded_ollama_tool_call() {
    let calls = OLLAMA_TOOL.matches("\"tool_calls\":[").count();
    assert_eq!(calls, 1, "one fragment carries the whole call");
    let (r, events) = replay_ollama(OLLAMA_TOOL).await;
    assert_eq!(r.stop, StopReason::ToolUse);
    let (id, input) = r
        .content
        .iter()
        .find_map(|b| match b {
            Block::ToolUse { id, name, input } if name == "read_file" => {
                Some((id.clone(), input.clone()))
            }
            _ => None,
        })
        .expect("the recorded call");
    assert_eq!(id, "call-recorded-0");
    assert_eq!(input["path"], "NOTES.md");
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(
        events[0],
        Event::ToolStart {
            id: id.clone(),
            name: "read_file".into()
        }
    );
    assert!(
        matches!(&events[1], Event::ToolInput { id: at, json } if *at == id && serde_json::from_str::<Value>(json).unwrap() == input),
        "the arguments arrive whole: {events:?}"
    );
    assert!(r.usage_known && r.usage.output > 0);
}

/// An unknown model: a 404 whose `type` names it, the cue for a fallback chain, not a retry.
#[tokio::test(start_paused = true)]
async fn a_recorded_ollama_unknown_model() {
    let script = Arc::new(Scripted::new([Reply::status(404, OLLAMA_ERROR, &[])]));
    let client = OpenAiCompatible::new(
        script.clone(),
        "http://127.0.0.1:11434",
        None,
        Policy::default(),
    );
    let mut on = |_: Event| {};
    let err = client
        .complete(&request(), &mut on)
        .await
        .expect_err("the recorded refusal");
    assert_eq!(
        err,
        Error::Api {
            status: 404,
            kind: "not_found_error".into(),
            message: "model 'mnemorph-no-such-model' not found".into(),
        }
    );
    assert!(err.model_not_found());
    assert_eq!(script.sent().len(), 1, "an unknown model is not retried");
}

#[test]
fn the_recordings_hold_no_secret() {
    for cassette in ALL {
        assert!(
            !cassette.contains("sk-or-") && !cassette.contains("Bearer"),
            "a cassette holds a credential"
        );
    }
}

/// What differs between runs or names the account is scrubbed to stable placeholders.
#[test]
fn the_recordings_are_scrubbed() {
    for cassette in ALL {
        for (key, kept) in [
            (
                "\"id\":\"",
                &["gen-recorded\"", "chatcmpl-recorded\"", "call-recorded-"][..],
            ),
            ("\"created\":", &["0,", "0}"][..]),
            ("\"user_id\":\"", &["user-recorded\""][..]),
        ] {
            for (at, _) in cassette.match_indices(key) {
                let value = &cassette[at + key.len()..];
                assert!(
                    kept.iter().any(|k| value.starts_with(k)),
                    "unscrubbed {key}{}",
                    &value[..value.len().min(40)]
                );
            }
        }
    }
}

/// Every key in every cassette is one the allowlist knows: a field a server adds cannot land unseen.
#[test]
fn the_recordings_hold_only_known_keys() {
    for (name, cassette) in NAMED {
        let unknown = keys::unknown(cassette);
        assert!(unknown.is_empty(), "{name}: {unknown:?}");
    }
}

#[test]
fn the_allowlist_refuses_a_new_field() {
    let chunk =
        "data: {\"id\":\"gen-recorded\",\"choices\":[{\"delta\":{\"session_token\":\"x\"}}]}\n\n";
    assert_eq!(keys::unknown(chunk), ["key `session_token`"]);
    assert_eq!(
        keys::unknown(r#"{"error":{"code":400,"account":"a"}}"#),
        ["key `account`"]
    );
    assert_eq!(
        keys::unknown("event: x\n").len(),
        1,
        "a line that is not data"
    );
}
