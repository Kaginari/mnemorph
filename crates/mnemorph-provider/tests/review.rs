//! The M3 review's findings, each pinned.

use std::sync::Arc;
use std::time::Duration;

use mnemorph_ledger::{Prices, Usage, wt};
use mnemorph_provider::{
    Anthropic, Block, Caller, Error, Event, HttpRequest, Message, OpenAiCompatible, Policy, Reply,
    Request, Response, Role, Scripted, StopReason, Tool, Wait, encode, encode_chat, usage_row,
};
use serde_json::{Value, json};

fn sse(events: &[Value]) -> String {
    use std::fmt::Write as _;
    events.iter().fold(String::new(), |mut out, e| {
        let _ = write!(out, "event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap());
        out
    })
}

fn start() -> Value {
    json!({"type": "message_start", "message": {"id": "m", "model": "claude-opus-5-5", "usage": {"input_tokens": 1}}})
}

fn ok_text(text: &str) -> String {
    sse(&[
        start(),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": text}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 2}}),
        json!({"type": "message_stop"}),
    ])
}

fn user(text: &str) -> Message {
    Message {
        role: Role::User,
        content: vec![Block::Text {
            text: text.into(),
            cache: false,
        }],
    }
}

fn request(messages: Vec<Message>) -> Request {
    Request {
        model: "claude-opus-5-5".into(),
        system: vec![],
        tools: vec![],
        messages,
        max_tokens: 1000,
        effort: None,
    }
}

fn anthropic(script: &Arc<Scripted>, policy: Policy) -> Anthropic {
    Anthropic::new(
        script.clone(),
        "https://api.example",
        "sk-secret".into(),
        policy,
    )
}

async fn call(client: &Anthropic, req: &Request) -> Result<Response, Error> {
    let mut on = |_: Event| {};
    client.complete(req, &mut on).await
}

#[tokio::test(start_paused = true)]
async fn a_silent_head_times_out_and_the_turn_has_a_deadline() {
    let p = Policy::default();
    let script = Arc::new(Scripted::new([
        Reply::Silent(p.first_byte * 2),
        Reply::ok(&ok_text("after")),
    ]));
    assert!(
        call(&anthropic(&script, p), &request(vec![user("hi")]))
            .await
            .is_ok(),
        "the head wait ran out, then a retry"
    );
    assert_eq!(script.sent().len(), 2);

    let busy = r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#;
    let tight = Policy {
        turn: Duration::from_secs(100),
        ..p
    };
    let script = Arc::new(Scripted::new([
        Reply::status(529, busy, &[("retry-after", "300")]),
        Reply::ok(&ok_text("never")),
    ]));
    let err = call(&anthropic(&script, tight), &request(vec![user("hi")]))
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Api { status: 529, .. }),
        "a wait past the deadline is not taken: {err}"
    );
    assert_eq!(script.sent().len(), 1);
    assert!(!Error::Timeout(Wait::Turn).retryable());
}

#[tokio::test(start_paused = true)]
async fn the_server_says_whether_to_retry() {
    let busy = r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#;
    let script = Arc::new(Scripted::new([
        Reply::status(529, busy, &[("x-should-retry", "false")]),
        Reply::ok(&ok_text("never")),
    ]));
    assert!(
        call(
            &anthropic(&script, Policy::default()),
            &request(vec![user("hi")])
        )
        .await
        .is_err()
    );
    assert_eq!(script.sent().len(), 1);
    let bad = r#"{"type":"error","error":{"type":"invalid_request_error","message":"x"}}"#;
    let script = Arc::new(Scripted::new([
        Reply::status(400, bad, &[("x-should-retry", "true")]),
        Reply::ok(&ok_text("then")),
    ]));
    assert!(
        call(
            &anthropic(&script, Policy::default()),
            &request(vec![user("hi")])
        )
        .await
        .is_ok()
    );
}

#[tokio::test(start_paused = true)]
async fn thinking_alone_is_not_content_shown() {
    let thinking_then_overload = sse(&[
        start(),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": "", "signature": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "minutes of it"}}),
        json!({"type": "error", "error": {"type": "overloaded_error", "message": "busy"}}),
    ]);
    let script = Arc::new(Scripted::new([
        Reply::ok(&thinking_then_overload),
        Reply::ok(&ok_text("answer")),
    ]));
    assert!(
        call(
            &anthropic(&script, Policy::default()),
            &request(vec![user("hi")])
        )
        .await
        .is_ok()
    );
    assert_eq!(script.sent().len(), 2, "retried: nothing was shown");
}

#[test]
fn system_messages_follow_the_api_rules() {
    let sys = |t: &str| Message {
        role: Role::System,
        content: vec![Block::Text {
            text: t.into(),
            cache: false,
        }],
    };
    let assistant = Message {
        role: Role::Assistant,
        content: vec![Block::Text {
            text: "ok".into(),
            cache: false,
        }],
    };
    assert!(encode(&request(vec![user("a"), sys("note")])).is_ok());
    assert!(encode(&request(vec![user("a"), sys("note"), assistant.clone()])).is_ok());
    for (bad, why) in [
        (vec![sys("first")], "cannot open"),
        (
            vec![user("a"), assistant.clone(), sys("note")],
            "follow a user",
        ),
        (
            vec![user("a"), sys("note"), user("b")],
            "followed by an assistant",
        ),
        (
            vec![
                user("a"),
                Message {
                    role: Role::System,
                    content: vec![Block::Opaque(json!({"type": "x"}))],
                },
            ],
            "text only",
        ),
    ] {
        let err = encode(&request(bad)).unwrap_err();
        assert!(err.to_string().contains(why), "{err}");
    }
}

#[test]
fn tools_stream_their_input() {
    let mut req = request(vec![user("a")]);
    req.tools.push(Tool {
        name: "write".into(),
        description: "write a file".into(),
        input_schema: json!({"type": "object"}),
        cache: false,
    });
    assert_eq!(
        encode(&req).unwrap()["tools"][0]["eager_input_streaming"],
        json!(true)
    );
}

#[tokio::test(start_paused = true)]
async fn stop_details_refusals_and_cut_tool_calls() {
    let refused = sse(&[
        start(),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "partial"}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "refusal", "stop_details": {"type": "refusal", "category": "cyber"}}, "usage": {"output_tokens": 3}}),
        json!({"type": "message_stop"}),
    ]);
    let cut = sse(&[
        start(),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t", "name": "write", "input": {}}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"path\": \"a.rs\", \"body\": \"fn"}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "max_tokens"}, "usage": {"output_tokens": 1000}}),
        json!({"type": "message_stop"}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&refused), Reply::ok(&cut)]));
    let client = anthropic(&script, Policy::default());
    let r = call(&client, &request(vec![user("a")])).await.unwrap();
    assert_eq!(r.stop, StopReason::Refusal);
    assert_eq!(r.stop_details.as_ref().unwrap()["category"], "cyber");
    assert!(
        r.content.is_empty(),
        "a refusal's partial content is not an answer"
    );
    let r = call(&client, &request(vec![user("a")])).await.unwrap();
    assert_eq!(r.stop, StopReason::MaxTokens);
    assert!(r.content.is_empty() && r.warnings.iter().any(|w| w.contains("cut by max_tokens")));
}

#[tokio::test(start_paused = true)]
async fn opaque_blocks_keep_every_delta_kind() {
    let body = sse(&[
        start(),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "redacted_thinking", "data": "opaque-bytes"}}),
        json!({"type": "content_block_start", "index": 1, "content_block": {"type": "server_tool_use", "id": "s1", "name": "web_search", "input": {}}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"query\":"}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "\"mnemorph\"}"}}),
        json!({"type": "content_block_start", "index": 2, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 2, "delta": {"type": "citations_delta", "citation": {"cited_text": "x"}}}),
        json!({"type": "content_block_delta", "index": 2, "delta": {"type": "text_delta", "text": "found"}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 5}}),
        json!({"type": "message_stop"}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let r = call(
        &anthropic(&script, Policy::default()),
        &request(vec![user("a")]),
    )
    .await
    .unwrap();
    assert_eq!(
        r.content[0],
        Block::Opaque(json!({"type": "redacted_thinking", "data": "opaque-bytes"}))
    );
    assert_eq!(
        r.content[1],
        Block::Opaque(
            json!({"type": "server_tool_use", "id": "s1", "name": "web_search", "input": {"query": "mnemorph"}})
        )
    );
    assert_eq!(
        r.content[2],
        Block::Text {
            text: "found".into(),
            cache: false
        }
    );
    assert!(
        r.warnings.iter().any(|w| w.contains("citations_delta")),
        "an unknown text delta is named, not fatal"
    );
}

#[test]
fn credentials_never_print() {
    let req = HttpRequest {
        url: "https://api.example".into(),
        headers: vec![
            ("x-api-key".into(), "sk-secret".into()),
            ("Authorization".into(), "Bearer sk-or".into()),
            ("anthropic-version".into(), "2026-10-21".into()),
        ],
        body: b"{}".to_vec(),
    };
    let shown = format!("{req:?}");
    assert!(
        !shown.contains("sk-secret") && !shown.contains("sk-or"),
        "{shown}"
    );
    assert!(shown.contains("2026-10-21") && shown.contains("[redacted]"));
}

#[test]
fn model_not_found_is_a_cue_for_the_chain() {
    let not_found = Error::Api {
        status: 404,
        kind: "not_found_error".into(),
        message: "model: claude-x".into(),
    };
    assert!(not_found.model_not_found() && !not_found.retryable());
    assert!(
        !Error::Api {
            status: 500,
            kind: "api_error".into(),
            message: "model".into()
        }
        .model_not_found()
    );
    assert!(!Error::Transport("down".into()).model_not_found());
}

#[tokio::test(start_paused = true)]
async fn error_bodies_in_every_shape() {
    let cases = [
        (
            r#"[{"error":{"type":"invalid_request_error","message":"in an array"}}]"#,
            "invalid_request_error",
            "in an array",
        ),
        ("not json at all", "unknown", "not json at all"),
        (
            r#"{"error":{"code":"invalid_api_key","message":"bad key"}}"#,
            "invalid_api_key",
            "bad key",
        ),
    ];
    for (body, kind, message) in cases {
        let script = Arc::new(Scripted::new([Reply::status(400, body, &[])]));
        let err = chat(&openai(&script), &request(vec![user("a")]))
            .await
            .0
            .unwrap_err();
        assert_eq!(
            err,
            Error::Api {
                status: 400,
                kind: kind.into(),
                message: message.into()
            },
            "{body}"
        );
    }
}

#[test]
fn a_ratio_that_is_not_five_is_read() {
    let prices = Prices::parse(
        "[models.m]\ninput = 1.0\noutput = 3.0\ncache-read = 0.1\ncache-write = 1.25",
    )
    .unwrap();
    let r = Response {
        id: "i".into(),
        model: "m".into(),
        content: vec![],
        stop: StopReason::EndTurn,
        stop_details: None,
        usage: Usage {
            input: 10,
            output: 100,
            cache_read: 0,
            cache_write: 0,
        },
        warnings: vec![],
    };
    let caller = Caller {
        session: "s".into(),
        run: "r".into(),
        body: "b".into(),
        race: "slime".into(),
        commission: None,
        prefix: 0,
    };
    let row = usage_row(&r, &caller, "p", &prices, mnemorph_ledger::Timestamp(0));
    assert_eq!(row.wt, wt(r.usage, 3.0));
    assert_ne!(
        row.wt,
        wt(r.usage, 5.0),
        "the table's own ratio, not the default"
    );
}

// ---- the OpenAI-compatible client ----

fn chunks(parts: &[Value]) -> String {
    use std::fmt::Write as _;
    let mut out = parts.iter().fold(String::new(), |mut out, p| {
        let _ = write!(out, "data: {p}\n\n");
        out
    });
    out.push_str("data: [DONE]\n\n");
    out
}

fn openai(script: &Arc<Scripted>) -> OpenAiCompatible {
    OpenAiCompatible::new(
        script.clone(),
        "http://localhost:11434",
        None,
        Policy::default(),
    )
}

async fn chat(client: &OpenAiCompatible, req: &Request) -> (Result<Response, Error>, Vec<Event>) {
    let mut events = Vec::new();
    let mut on = |e: Event| events.push(e);
    let r = client.complete(req, &mut on).await;
    (r, events)
}

#[tokio::test(start_paused = true)]
async fn an_error_finish_is_retried_never_an_answer() {
    let errored = chunks(&[json!({"choices": [{"delta": {}, "finish_reason": "error"}]})]);
    let fine =
        chunks(&[json!({"choices": [{"delta": {"content": "ok"}, "finish_reason": "stop"}]})]);
    let script = Arc::new(Scripted::new([Reply::ok(&errored), Reply::ok(&fine)]));
    let (r, _) = chat(&openai(&script), &request(vec![user("a")])).await;
    assert_eq!(r.unwrap().stop, StopReason::EndTurn);
    assert_eq!(script.sent().len(), 2);

    let numeric = "data: {\"error\":{\"message\":\"upstream busy\",\"code\":502}}\n\n";
    let script = Arc::new(Scripted::new([Reply::ok(numeric), Reply::ok(&fine)]));
    assert!(
        chat(&openai(&script), &request(vec![user("a")]))
            .await
            .0
            .is_ok(),
        "a numeric 5xx code is retried"
    );

    let vllm = r#"{"object":"error","message":"max_tokens is too large","type":"BadRequestError","code":400}"#;
    let script = Arc::new(Scripted::new([Reply::status(400, vllm, &[])]));
    let err = chat(&openai(&script), &request(vec![user("a")]))
        .await
        .0
        .unwrap_err();
    assert_eq!(
        err,
        Error::Api {
            status: 400,
            kind: "BadRequestError".into(),
            message: "max_tokens is too large".into()
        }
    );
    let script = Arc::new(Scripted::new([
        Reply::status(503, "<html>down</html>", &[]),
        Reply::ok(&fine),
    ]));
    assert!(
        chat(&openai(&script), &request(vec![user("a")]))
            .await
            .0
            .is_ok(),
        "503 then 200"
    );
}

#[tokio::test(start_paused = true)]
async fn reasoning_and_call_extras_go_back() {
    let body = chunks(&[
        json!({"choices": [{"delta": {"reasoning_content": "let me "}}]}),
        json!({"choices": [{"delta": {"reasoning_content": "see"}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "c1", "type": "function", "function": {"name": "read", "arguments": "{}"}, "extra_content": {"google": {"thought_signature": "sig"}}}]}}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let (r, events) = chat(&openai(&script), &request(vec![user("a")])).await;
    let r = r.unwrap();
    assert_eq!(
        events,
        [Event::ToolStart {
            id: "c1".into(),
            name: "read".into()
        }],
        "reasoning is not shown"
    );
    let mut next = request(vec![user("a")]);
    next.messages.push(Message {
        role: Role::Assistant,
        content: r.content.clone(),
    });
    let body = encode_chat(&next);
    let replayed = &body["messages"][1];
    assert_eq!(replayed["reasoning_content"], "let me see");
    assert_eq!(
        replayed["tool_calls"][0]["extra_content"]["google"]["thought_signature"],
        "sig"
    );
    // The same blocks sent to Anthropic are left out.
    let to_anthropic = encode(&next).unwrap();
    assert!(
        !to_anthropic.to_string().contains("thought_signature")
            && !to_anthropic.to_string().contains("let me see")
    );
}

#[tokio::test(start_paused = true)]
async fn fragments_without_index_and_repeated_names() {
    let body = chunks(&[
        json!({"choices": [{"delta": {"tool_calls": [{"id": "a", "function": {"name": "read", "arguments": "{\"p\":"}}]}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{"function": {"name": "read", "arguments": "1}"}}]}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{"id": "b", "function": {"name": "list", "arguments": ""}}]}}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let (r, events) = chat(&openai(&script), &request(vec![user("a")])).await;
    assert_eq!(
        r.unwrap().content,
        [
            Block::ToolUse {
                id: "a".into(),
                name: "read".into(),
                input: json!({"p": 1})
            },
            Block::ToolUse {
                id: "b".into(),
                name: "list".into(),
                input: json!({})
            },
        ]
    );
    assert_eq!(events.len(), 2, "one ToolStart per call");
}

#[test]
fn operator_notes_and_consecutive_tool_results() {
    let mut req = request(vec![
        user("go"),
        Message {
            role: Role::Assistant,
            content: vec![
                Block::ToolUse {
                    id: "x".into(),
                    name: "a".into(),
                    input: json!({}),
                },
                Block::ToolUse {
                    id: "y".into(),
                    name: "b".into(),
                    input: json!({}),
                },
            ],
        },
        Message {
            role: Role::User,
            content: vec![
                Block::ToolResult {
                    tool_use_id: "x".into(),
                    content: "1".into(),
                    is_error: false,
                    cache: false,
                },
                Block::ToolResult {
                    tool_use_id: "y".into(),
                    content: "boom".into(),
                    is_error: true,
                    cache: false,
                },
                Block::Text {
                    text: "next?".into(),
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
    ]);
    req.system.push(Block::Text {
        text: "crest".into(),
        cache: false,
    });
    let m = encode_chat(&req)["messages"].as_array().unwrap().clone();
    assert_eq!(
        m.iter().filter(|x| x["role"] == "system").count(),
        1,
        "only the leading system message"
    );
    assert_eq!(
        m[3],
        json!({"role": "tool", "tool_call_id": "x", "content": "1"})
    );
    assert_eq!(
        m[4],
        json!({"role": "tool", "tool_call_id": "y", "content": "error: boom"})
    );
    assert_eq!(
        m[5],
        json!({"role": "user", "content": "next?\n\n[operator note] the date changed"})
    );
}

#[tokio::test(start_paused = true)]
async fn a_server_without_usage_still_answers() {
    let ollama =
        chunks(&[json!({"choices": [{"delta": {"content": "hi"}, "finish_reason": "stop"}]})]);
    let script = Arc::new(Scripted::new([Reply::ok(&ollama)]));
    let r = chat(&openai(&script), &request(vec![user("a")]))
        .await
        .0
        .unwrap();
    assert_eq!(r.usage, Usage::default());
}

#[tokio::test(start_paused = true)]
async fn cache_writes_are_their_own_class() {
    let body = chunks(&[
        json!({"choices": [{"delta": {"content": "ok"}, "finish_reason": "stop"}]}),
        json!({"choices": [], "usage": {"prompt_tokens": 1000, "completion_tokens": 5, "prompt_tokens_details": {"cached_tokens": 600, "cache_write_tokens": 300}}}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let r = chat(&openai(&script), &request(vec![user("a")]))
        .await
        .0
        .unwrap();
    assert_eq!(
        r.usage,
        Usage {
            input: 100,
            output: 5,
            cache_read: 600,
            cache_write: 300
        }
    );
}
