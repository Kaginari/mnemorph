//! The OpenAI-compatible client against scripted replies: the mapping, the stream, and the shared
//! ladder.

use std::sync::Arc;

use mnemorph_ledger::Usage;
use mnemorph_provider::{
    Block, Error, Event, Message, OpenAiCompatible, Policy, Reply, Request, Role, Scripted,
    StopReason, Tool, encode_chat,
};
use serde_json::{Value, json};

fn chunks(parts: &[Value]) -> String {
    use std::fmt::Write as _;
    let mut out = parts.iter().fold(String::new(), |mut out, p| {
        let _ = write!(out, "data: {p}\n\n");
        out
    });
    out.push_str("data: [DONE]\n\n");
    out
}

fn request() -> Request {
    Request {
        model: "qwen3-coder".into(),
        system: vec![
            Block::Text {
                text: "the crest".into(),
                cache: true,
            },
            Block::Text {
                text: "the card".into(),
                cache: true,
            },
        ],
        tools: vec![Tool {
            name: "read".into(),
            description: "read a file".into(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            cache: true,
            defer: false,
        }],
        messages: vec![
            Message {
                role: Role::User,
                content: vec![Block::Text {
                    text: "look".into(),
                    cache: false,
                }],
            },
            Message {
                role: Role::Assistant,
                content: vec![
                    Block::Opaque(json!({"type": "thinking", "thinking": "", "signature": "x"})),
                    Block::Text {
                        text: "reading".into(),
                        cache: false,
                    },
                    Block::ToolUse {
                        id: "c1".into(),
                        name: "read".into(),
                        input: json!({"path": "a.rs"}),
                    },
                ],
            },
            Message {
                role: Role::User,
                content: vec![
                    Block::ToolResult {
                        tool_use_id: "c1".into(),
                        content: "fn main() {}".into(),
                        is_error: false,
                        cache: true,
                        references: Vec::new(),
                    },
                    Block::Text {
                        text: "and now?".into(),
                        cache: false,
                    },
                ],
            },
            Message {
                role: Role::System,
                content: vec![Block::Text {
                    text: "the date changed".into(),
                    cache: false,
                }],
            },
        ],
        max_tokens: 4096,
        effort: None,
    }
}

fn client(script: &Arc<Scripted>, key: Option<&str>) -> OpenAiCompatible {
    OpenAiCompatible::new(
        script.clone(),
        "http://localhost:8000/",
        key.map(str::to_owned),
        Policy::default(),
    )
}

#[test]
fn the_mapping() {
    let body = encode_chat(&request());
    let m = body["messages"].as_array().unwrap();
    assert_eq!(
        m[0],
        json!({"role": "system", "content": "the crest\n\nthe card"})
    );
    assert_eq!(m[1], json!({"role": "user", "content": "look"}));
    assert_eq!(m[2]["role"], "assistant");
    assert_eq!(
        m[2]["content"], "reading",
        "the opaque thinking block is left out"
    );
    assert_eq!(
        m[2]["tool_calls"][0]["function"]["arguments"],
        "{\"path\":\"a.rs\"}"
    );
    assert_eq!(
        m[3],
        json!({"role": "tool", "tool_call_id": "c1", "content": "fn main() {}"})
    );
    assert_eq!(
        m[4],
        json!({"role": "user", "content": "and now?\n\n[operator note] the date changed"}),
        "a mid-conversation note joins the user's turn"
    );
    assert_eq!(m.len(), 5);
    assert_eq!(body["tools"][0]["function"]["name"], "read");
    assert_eq!(body["stream_options"], json!({"include_usage": true}));
    assert!(!body.to_string().contains("cache_control"));
    for sampling in ["temperature", "top_p", "top_k"] {
        assert!(body.get(sampling).is_none());
    }
}

#[tokio::test(start_paused = true)]
async fn text_tools_and_usage() {
    let body = chunks(&[
        json!({"id": "c-1", "model": "qwen3-coder", "choices": [{"index": 0, "delta": {"role": "assistant", "content": "Lo"}}]}),
        json!({"choices": [{"index": 0, "delta": {"content": "oking."}}]}),
        json!({"choices": [{"index": 0, "delta": {"tool_calls": [{"index": 0, "id": "t1", "type": "function", "function": {"name": "list", "arguments": ""}}]}}]}),
        json!({"choices": [{"index": 0, "delta": {"tool_calls": [{"index": 1, "id": "t2", "type": "function", "function": {"name": "read", "arguments": "{\"pa"}}]}}]}),
        json!({"choices": [{"index": 0, "delta": {"tool_calls": [{"index": 1, "function": {"arguments": "th\":\"b.rs\"}"}}]}}]}),
        json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]}),
        json!({"choices": [], "usage": {"prompt_tokens": 5000, "completion_tokens": 40, "prompt_tokens_details": {"cached_tokens": 4096}}}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let mut events = Vec::new();
    let mut on = |e: Event| events.push(e);
    let r = client(&script, Some("sk-or"))
        .complete(&request(), &mut on)
        .await
        .unwrap();
    assert_eq!(
        r.content,
        [
            Block::Text {
                text: "Looking.".into(),
                cache: false
            },
            Block::ToolUse {
                id: "t1".into(),
                name: "list".into(),
                input: json!({})
            },
            Block::ToolUse {
                id: "t2".into(),
                name: "read".into(),
                input: json!({"path": "b.rs"})
            },
        ]
    );
    assert_eq!(r.stop, StopReason::ToolUse);
    assert_eq!(
        r.usage,
        Usage {
            input: 904,
            output: 40,
            cache_read: 4096,
            cache_write: 0
        }
    );
    assert_eq!((r.id.as_str(), r.model.as_str()), ("c-1", "qwen3-coder"));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Text(_)))
            .count(),
        2
    );
    let sent = script.sent();
    assert_eq!(sent[0].url, "http://localhost:8000/v1/chat/completions");
    assert!(
        sent[0]
            .headers
            .contains(&("authorization".into(), "Bearer sk-or".into()))
    );
    let sent_body: Value = serde_json::from_slice(&sent[0].body).unwrap();
    assert_eq!(sent_body, encode_chat(&request()));
}

#[tokio::test(start_paused = true)]
async fn the_end_is_required_and_errors_are_named() {
    let no_done = "data: {\"choices\":[{\"delta\":{\"content\":\"half\"}}]}\n\n";
    let script = Arc::new(Scripted::new([Reply::ok(no_done)]));
    let mut on = |_: Event| {};
    assert_eq!(
        client(&script, None)
            .complete(&request(), &mut on)
            .await
            .unwrap_err(),
        Error::Truncated
    );
    assert!(
        !script.sent()[0]
            .headers
            .iter()
            .any(|(n, _)| n == "authorization"),
        "no key, no header"
    );

    let refused = chunks(&[json!({"choices": [{"delta": {}, "finish_reason": "content_filter"}]})]);
    let script = Arc::new(Scripted::new([Reply::ok(&refused)]));
    assert_eq!(
        client(&script, None)
            .complete(&request(), &mut on)
            .await
            .unwrap()
            .stop,
        StopReason::Refusal
    );

    let in_stream =
        "data: {\"error\":{\"message\":\"model overloaded\",\"type\":\"overloaded_error\"}}\n\n";
    let script = Arc::new(Scripted::new([
        Reply::ok(in_stream),
        Reply::ok(&chunks(&[
            json!({"choices": [{"delta": {"content": "ok"}, "finish_reason": "stop"}]}),
        ])),
    ]));
    let r = client(&script, None)
        .complete(&request(), &mut on)
        .await
        .unwrap();
    assert_eq!(
        r.content,
        [Block::Text {
            text: "ok".into(),
            cache: false
        }]
    );
    assert_eq!(
        script.sent().len(),
        2,
        "an error before content is retried, by the shared ladder"
    );

    let unauthorised = r#"{"error":{"message":"bad key","code":"invalid_api_key"}}"#;
    let script = Arc::new(Scripted::new([Reply::status(401, unauthorised, &[])]));
    assert_eq!(
        client(&script, Some("x"))
            .complete(&request(), &mut on)
            .await
            .unwrap_err(),
        Error::Api {
            status: 401,
            kind: "invalid_api_key".into(),
            message: "bad key".into()
        }
    );
}

/// A finished call whose arguments are no JSON is the model's slip: kept, its text under
/// `MALFORMED`, for the loop to answer — the response is never an error that ends the run.
#[tokio::test]
async fn a_call_with_broken_arguments_is_kept_for_the_loop_to_answer() {
    let body = chunks(&[
        json!({"id": "c-1", "model": "m", "choices": [{"index": 0, "delta": {"tool_calls": [{"index": 0, "id": "t1", "type": "function", "function": {"name": "read", "arguments": "{\"path\": "}}]}}]}),
        json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let r = client(&script, Some("sk-or"))
        .complete(&request(), &mut |_| {})
        .await
        .expect("a broken call is no stream error");
    let [Block::ToolUse { name, input, .. }] = r.content.as_slice() else {
        panic!("{:?}", r.content);
    };
    assert_eq!(name, "read");
    assert_eq!(input[mnemorph_provider::MALFORMED], "{\"path\": ");
    assert!(input["error"].as_str().unwrap().contains("EOF"));
}
