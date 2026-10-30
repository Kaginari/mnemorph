//! Real responses, recorded from OpenRouter's free models (`tests/live_openrouter.rs` with
//! `MNEMORPH_RECORD`), scrubbed, and replayed here: no key, no network, every run.

use std::sync::Arc;

use mnemorph_ledger::Usage;
use mnemorph_provider::{
    Block, Error, Event, Message, OPAQUE_PREFIX, OpenAiCompatible, Policy, Reply, Request, Role,
    Scripted, StopReason, encode_chat,
};
use serde_json::{Value, json};

// The recordings predate the rename: the tool result the server read, and so its answer, say
// "Tempest". They stay as recorded.
const TEXT: &str = include_str!("cassettes/openrouter-text-0.sse");
const TOOL_CALL: &str = include_str!("cassettes/openrouter-tool-0.sse");
const TOOL_ANSWER: &str = include_str!("cassettes/openrouter-tool-1.sse");
const REASONING: &str = include_str!("cassettes/openrouter-reasoning-0.sse");
const ERROR: &str = include_str!("cassettes/openrouter-error-0.json");
const ALL: [&str; 5] = [TEXT, TOOL_CALL, TOOL_ANSWER, REASONING, ERROR];

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
            ("\"id\":\"", ["gen-recorded\"", "call-recorded-"]),
            ("\"created\":", ["0,", "0}"]),
            ("\"user_id\":\"", ["user-recorded\"", "user-recorded\""]),
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
