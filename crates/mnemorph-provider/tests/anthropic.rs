//! The Anthropic client against scripted replies, on a paused clock: encoding, assembly, the
//! retry ladder, the three timeouts, and the junction with the ledger.

use std::sync::Arc;
use std::time::Duration;

use mnemorph_ledger::{Timestamp, Usage, wt};
use mnemorph_provider::{
    API_VERSION, Anthropic, Block, Caller, Effort, Error, Event, Message, Policy, Reply, Request,
    Role, Scripted, StopReason, Tool, Wait, anthropic_prices, encode, usage_row,
};
use serde_json::{Value, json};

fn sse(events: &[Value]) -> String {
    use std::fmt::Write as _;
    events.iter().fold(String::new(), |mut out, e| {
        let _ = write!(
            out,
            "event: {}
data: {e}

",
            e["type"].as_str().unwrap()
        );
        out
    })
}

fn start(usage: &Value) -> Value {
    json!({"type": "message_start", "message": {"id": "msg_1", "model": "claude-opus-5-5", "usage": usage}})
}

fn text_answer(text: &str) -> String {
    sse(&[
        start(
            &json!({"input_tokens": 20, "cache_read_input_tokens": 4000, "cache_creation_input_tokens": 300, "output_tokens": 1}),
        ),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": &text[..text.len() / 2]}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": &text[text.len() / 2..]}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 12}}),
        json!({"type": "message_stop"}),
    ])
}

fn request() -> Request {
    Request {
        model: "claude-opus-5-5".into(),
        system: vec![Block::Text {
            text: "the crest".into(),
            cache: true,
        }],
        tools: vec![Tool {
            name: "read".into(),
            description: "read a file".into(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
            cache: true,
        }],
        messages: vec![Message {
            role: Role::User,
            content: vec![Block::Text {
                text: "hello".into(),
                cache: false,
            }],
        }],
        max_tokens: 64_000,
        effort: Some(Effort::High),
    }
}

fn client(script: &Arc<Scripted>) -> Anthropic {
    Anthropic::new(
        script.clone(),
        "https://api.example",
        "sk-test".into(),
        Policy::default(),
    )
}

async fn run(
    client: &Anthropic,
    req: &Request,
) -> (Result<mnemorph_provider::Response, Error>, Vec<Event>) {
    let mut events = Vec::new();
    let mut on = |e: Event| events.push(e);
    let result = client.complete(req, &mut on).await;
    (result, events)
}

#[test]
fn encoding_follows_the_api() {
    let body = encode(&request()).unwrap();
    assert_eq!(body["stream"], json!(true));
    assert_eq!(body["output_config"]["effort"], json!("high"));
    for sampling in ["temperature", "top_p", "top_k"] {
        assert!(body.get(sampling).is_none(), "{sampling} is never sent");
    }
    assert_eq!(
        body["tools"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert_eq!(
        body["system"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert!(
        body["messages"][0]["content"][0]
            .get("cache_control")
            .is_none()
    );

    let mut five = request();
    for i in 0..4 {
        five.messages[0].content.push(Block::Text {
            text: format!("t{i}"),
            cache: true,
        });
    }
    assert_eq!(five.breakpoints(), 6);
    assert!(
        matches!(encode(&five), Err(Error::Invalid(why)) if why.contains("6 cache breakpoints"))
    );

    let mut opens_with_system = request();
    opens_with_system.messages[0].role = Role::System;
    assert!(matches!(encode(&opens_with_system), Err(Error::Invalid(_))));
}

#[tokio::test(start_paused = true)]
async fn a_text_answer_streams_and_is_metered() {
    let script = Arc::new(Scripted::new([Reply::ok(&text_answer("Hello, world"))]));
    let (result, events) = run(&client(&script), &request()).await;
    let r = result.unwrap();
    assert_eq!(
        r.content,
        [Block::Text {
            text: "Hello, world".into(),
            cache: false
        }]
    );
    assert_eq!(r.stop, StopReason::EndTurn);
    assert_eq!(
        r.usage,
        Usage {
            input: 20,
            output: 12,
            cache_read: 4000,
            cache_write: 300
        }
    );
    assert_eq!(
        events,
        [Event::Text("Hello,".into()), Event::Text(" world".into())]
    );
    let sent = script.sent();
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0]
            .headers
            .contains(&("anthropic-version".into(), API_VERSION.into()))
    );
    assert_eq!(sent[0].url, "https://api.example/v1/messages");
    // The mock encodes for real: what was sent is valid JSON, the same as `encode`.
    let sent_body: Value = serde_json::from_slice(&sent[0].body).unwrap();
    assert_eq!(sent_body, encode(&request()).unwrap());
}

#[tokio::test(start_paused = true)]
async fn tool_calls_with_and_without_arguments() {
    let body = sse(&[
        start(&json!({"input_tokens": 5, "output_tokens": 1})),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t1", "name": "list", "input": {}}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "t2", "name": "read", "input": {}}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"pa"}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "th\": \"a.rs\"}"}}),
        json!({"type": "content_block_stop", "index": 1}),
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 30}}),
        json!({"type": "message_stop"}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let (result, events) = run(&client(&script), &request()).await;
    let r = result.unwrap();
    assert_eq!(r.stop, StopReason::ToolUse);
    assert_eq!(
        r.content,
        [
            Block::ToolUse {
                id: "t1".into(),
                name: "list".into(),
                input: json!({})
            },
            Block::ToolUse {
                id: "t2".into(),
                name: "read".into(),
                input: json!({"path": "a.rs"})
            },
        ]
    );
    // Each call announced as it starts, then its arguments piece by piece, as they came.
    assert_eq!(
        events,
        [
            Event::ToolStart {
                id: "t1".into(),
                name: "list".into()
            },
            Event::ToolStart {
                id: "t2".into(),
                name: "read".into()
            },
            Event::ToolInput {
                id: "t2".into(),
                json: "{\"pa".into()
            },
            Event::ToolInput {
                id: "t2".into(),
                json: "th\": \"a.rs\"}".into()
            },
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn thinking_blocks_come_back_exactly() {
    let body = sse(&[
        start(&json!({"input_tokens": 5})),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": "", "signature": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "first, "}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "then"}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "c2ln"}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "content_block_start", "index": 1, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "ok"}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 9}}),
        json!({"type": "message_stop"}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let (result, _) = run(&client(&script), &request()).await;
    let r = result.unwrap();
    let thinking = json!({"type": "thinking", "thinking": "first, then", "signature": "c2ln"});
    assert_eq!(r.content[0], Block::Opaque(thinking.clone()));
    // Appended to the next request, it is sent exactly as it came.
    let mut next = request();
    next.messages.push(Message {
        role: Role::Assistant,
        content: r.content,
    });
    assert_eq!(
        encode(&next).unwrap()["messages"][1]["content"][0],
        thinking
    );
}

#[tokio::test(start_paused = true)]
async fn reasoning_then_a_call_streams_and_assembles_whole() {
    let body = sse(&[
        start(&json!({"input_tokens": 5})),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": "", "signature": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "read "}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "it"}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "c2ln"}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "t1", "name": "read", "input": {}}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"path\": "}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "\"a.rs\"}"}}),
        json!({"type": "content_block_stop", "index": 1}),
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 9}}),
        json!({"type": "message_stop"}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let (result, events) = run(&client(&script), &request()).await;
    let r = result.unwrap();
    // The whole answer, pinned: the thinking block exactly as it came, then the call.
    assert_eq!(
        r.content,
        [
            Block::Opaque(json!({"type": "thinking", "thinking": "read it", "signature": "c2ln"})),
            Block::ToolUse {
                id: "t1".into(),
                name: "read".into(),
                input: json!({"path": "a.rs"})
            },
        ]
    );
    // And what streamed to the surface, in order: reasoning begun, its pieces, the call, its
    // arguments.
    assert_eq!(
        events,
        [
            Event::Thinking(String::new()),
            Event::Thinking("read ".into()),
            Event::Thinking("it".into()),
            Event::ToolStart {
                id: "t1".into(),
                name: "read".into()
            },
            Event::ToolInput {
                id: "t1".into(),
                json: "{\"path\": ".into()
            },
            Event::ToolInput {
                id: "t1".into(),
                json: "\"a.rs\"}".into()
            },
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn the_ladder_retries_what_can_be_retried() {
    let script = Arc::new(Scripted::new([
        Reply::status(
            529,
            r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#,
            &[],
        ),
        Reply::status(
            429,
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#,
            &[("retry-after", "7")],
        ),
        Reply::ConnectError,
        Reply::ok(&text_answer("done")),
    ]));
    let begin = tokio::time::Instant::now();
    let (result, _) = run(&client(&script), &request()).await;
    assert_eq!(
        result.unwrap().content,
        [Block::Text {
            text: "done".into(),
            cache: false
        }]
    );
    assert_eq!(script.sent().len(), 4);
    assert!(
        begin.elapsed() >= Duration::from_secs(7),
        "retry-after was honoured"
    );
    let bodies: Vec<_> = script.sent().into_iter().map(|r| r.body).collect();
    assert!(
        bodies.windows(2).all(|w| w[0] == w[1]),
        "every attempt sends the same request"
    );
}

#[tokio::test(start_paused = true)]
async fn the_ladder_stops() {
    let bad = r#"{"type":"error","error":{"type":"invalid_request_error","message":"max_tokens: too large"}}"#;
    let script = Arc::new(Scripted::new([Reply::status(400, bad, &[])]));
    let (result, _) = run(&client(&script), &request()).await;
    assert_eq!(
        result.unwrap_err(),
        Error::Api {
            status: 400,
            kind: "invalid_request_error".into(),
            message: "max_tokens: too large".into()
        }
    );
    assert_eq!(script.sent().len(), 1, "a 400 is not retried");

    let busy = r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#;
    let script = Arc::new(Scripted::new(
        (0..10).map(|_| Reply::status(529, busy, &[])),
    ));
    let (result, _) = run(&client(&script), &request()).await;
    assert!(matches!(result, Err(Error::Api { status: 529, .. })));
    assert_eq!(
        script.sent().len(),
        4,
        "the first attempt and three retries"
    );
}

#[tokio::test(start_paused = true)]
async fn a_slow_prefill_is_not_killed_but_a_stall_is() {
    let p = Policy::default();
    let answer = text_answer("late but fine");
    // Nothing for nine minutes (queueing, prefill), then the whole answer: under the first-byte wait.
    let slow = Reply::Respond {
        status: 200,
        headers: vec![],
        chunks: vec![(Duration::from_secs(540), answer.into_bytes())],
    };
    let script = Arc::new(Scripted::new([slow]));
    assert!(run(&client(&script), &request()).await.0.is_ok());

    // Content started, then silence past the stall wait: an error, and no retry (it was shown).
    let first = text_answer("x");
    let cut = first.find("event: message_delta").unwrap();
    let stalled = Reply::Respond {
        status: 200,
        headers: vec![],
        chunks: vec![
            (Duration::ZERO, first.as_bytes()[..cut].to_vec()),
            (
                p.stall + Duration::from_secs(1),
                first.as_bytes()[cut..].to_vec(),
            ),
        ],
    };
    let script = Arc::new(Scripted::new([stalled, Reply::ok(&text_answer("never"))]));
    assert_eq!(
        run(&client(&script), &request()).await.0.unwrap_err(),
        Error::Timeout(Wait::Stall)
    );
    assert_eq!(script.sent().len(), 1);

    // Nothing at all within the first-byte wait: retried.
    let silent = Reply::Respond {
        status: 200,
        headers: vec![],
        chunks: vec![(p.first_byte + Duration::from_secs(1), Vec::new())],
    };
    let script = Arc::new(Scripted::new([
        silent,
        Reply::ok(&text_answer("second try")),
    ]));
    assert!(run(&client(&script), &request()).await.0.is_ok());
    assert_eq!(script.sent().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_cut_stream_is_never_a_complete_answer() {
    let full = text_answer("almost");
    let without_stop = &full[..full.find("event: message_stop").unwrap()];
    let script = Arc::new(Scripted::new([Reply::ok(without_stop)]));
    assert_eq!(
        run(&client(&script), &request()).await.0.unwrap_err(),
        Error::Truncated
    );
    assert_eq!(script.sent().len(), 1, "content had started: not retried");

    // Cut before any content: nothing was shown, so it is retried.
    let only_start = sse(&[start(&json!({"input_tokens": 1}))]);
    let script = Arc::new(Scripted::new([
        Reply::ok(&only_start),
        Reply::ok(&text_answer("whole")),
    ]));
    assert!(run(&client(&script), &request()).await.0.is_ok());
}

#[tokio::test(start_paused = true)]
async fn errors_inside_the_stream() {
    let overloaded = sse(&[
        start(&json!({})),
        json!({"type": "error", "error": {"type": "overloaded_error", "message": "busy"}}),
    ]);
    let script = Arc::new(Scripted::new([
        Reply::ok(&overloaded),
        Reply::ok(&text_answer("recovered")),
    ]));
    assert!(
        run(&client(&script), &request()).await.0.is_ok(),
        "before content: retried"
    );

    let mut after = text_answer("half");
    after = after[..after.find("event: content_block_stop").unwrap()].to_owned();
    after.push_str(&sse(&[
        json!({"type": "error", "error": {"type": "overloaded_error", "message": "busy"}}),
    ]));
    let script = Arc::new(Scripted::new([
        Reply::ok(&after),
        Reply::ok(&text_answer("never")),
    ]));
    let err = run(&client(&script), &request()).await.0.unwrap_err();
    assert_eq!(
        err,
        Error::Stream {
            kind: "overloaded_error".into(),
            message: "busy".into()
        }
    );
    assert_eq!(
        script.sent().len(),
        1,
        "after content: returned to the loop"
    );
}

#[tokio::test(start_paused = true)]
async fn refusals_and_one_hour_writes_are_named() {
    let body = sse(&[
        start(
            &json!({"input_tokens": 3, "cache_creation_input_tokens": 100, "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 100}}),
        ),
        json!({"type": "message_delta", "delta": {"stop_reason": "refusal"}, "usage": {"output_tokens": 0}}),
        json!({"type": "message_stop"}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let r = run(&client(&script), &request()).await.0.unwrap();
    assert_eq!(r.stop, StopReason::Refusal);
    assert_eq!(r.warnings.len(), 1);
    assert!(r.warnings[0].contains("1-hour"));
}

#[tokio::test(start_paused = true)]
async fn the_ledger_prices_every_answer() {
    let script = Arc::new(Scripted::new([Reply::ok(&text_answer("metered"))]));
    let r = run(&client(&script), &request()).await.0.unwrap();
    let prices = anthropic_prices().unwrap();
    let caller = Caller {
        session: "s".into(),
        run: "r".into(),
        body: "rimuru".into(),
        race: "rimuru".into(),
        commission: None,
        prefix: 4000,
    };
    let row = usage_row(&r, &caller, "anthropic", &prices, Timestamp(1_792_782_420));
    assert_eq!(row.usage, r.usage);
    assert_eq!(
        row.wt,
        wt(r.usage, 5.0),
        "Opus 5.5's output ratio is 20 / 4"
    );
    let usd = row.usd.unwrap();
    let by_hand = (20.0 * 4.0 + 12.0 * 20.0 + 4000.0 * 0.20 + 300.0 * 5.0) / 1_000_000.0;
    assert!((usd - by_hand).abs() < 1e-12, "{usd} vs {by_hand}");
    let unpriced = usage_row(
        &mnemorph_provider::Response {
            model: "claude-haiku-5-5".into(),
            ..r
        },
        &caller,
        "anthropic",
        &prices,
        Timestamp(0),
    );
    assert_eq!(
        unpriced.usd, None,
        "a model without published cache prices is unpriced"
    );
    for model in ["claude-opus-5-5", "claude-sonnet-5-5", "claude-fable-5-1"] {
        assert!(prices.get(model).is_some(), "{model}");
    }
}

/// A finished call whose input is no JSON is the model's slip: kept under `MALFORMED`, never an
/// error that ends the run.
#[tokio::test(start_paused = true)]
async fn a_call_with_broken_input_is_kept_for_the_loop_to_answer() {
    let body = sse(&[
        start(&json!({"input_tokens": 5, "output_tokens": 1})),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t1", "name": "read", "input": {}}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"path\": "}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 9}}),
        json!({"type": "message_stop"}),
    ]);
    let script = Arc::new(Scripted::new([Reply::ok(&body)]));
    let (result, _) = run(&client(&script), &request()).await;
    let r = result.expect("a broken call is no stream error");
    let [Block::ToolUse { name, input, .. }] = r.content.as_slice() else {
        panic!("{:?}", r.content);
    };
    assert_eq!(name, "read");
    assert_eq!(input[mnemorph_provider::MALFORMED], "{\"path\": ");
}
