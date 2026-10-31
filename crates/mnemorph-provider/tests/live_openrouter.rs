//! Real calls to OpenRouter, only when asked for:
//! `MNEMORPH_LIVE=1 OPENROUTER_API_KEY=… cargo nextest run -p mnemorph-provider --run-ignored only
//! live_openrouter`. The default model is a free one; `MNEMORPH_LIVE_OR_MODEL` picks another.
//! `MNEMORPH_RECORD=tests/cassettes` saves each run's bodies, scrubbed, as the cassettes
//! `tests/cassettes.rs` replays.

use std::sync::{Arc, Mutex};

use mnemorph_provider::{
    Block, Event, HttpTransport, Message, OpenAiCompatible, Policy, Request, Role, StopReason, Tool,
};
use serde_json::json;

#[path = "support/keys.rs"]
mod keys;
#[path = "support/recorder.rs"]
mod recorder;

use recorder::Recorder;

fn client() -> Option<(OpenAiCompatible, String, Arc<Recorder>)> {
    if std::env::var("MNEMORPH_LIVE").as_deref() != Ok("1") {
        return None;
    }
    let key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY");
    let model = std::env::var("MNEMORPH_LIVE_OR_MODEL")
        .unwrap_or_else(|_| "nvidia/nemotron-3.5-lightning:free".into());
    let policy = Policy::default();
    let recorder = Arc::new(Recorder {
        inner: HttpTransport::new(policy.connect).expect("tls"),
        bodies: Mutex::new(Vec::new()),
    });
    let client = OpenAiCompatible::new(
        recorder.clone(),
        "https://openrouter.ai/api",
        Some(key),
        policy,
    );
    Some((client, model, recorder))
}

fn ask(model: &str, text: &str, tools: Vec<Tool>) -> Request {
    Request {
        model: model.to_owned(),
        system: vec![Block::Text {
            text: "You are a terse test assistant.".into(),
            cache: false,
        }],
        tools,
        messages: vec![Message {
            role: Role::User,
            content: vec![Block::Text {
                text: text.into(),
                cache: false,
            }],
        }],
        max_tokens: 2048,
        effort: None,
    }
}

#[tokio::test]
#[ignore = "calls OpenRouter: run with MNEMORPH_LIVE=1 and OPENROUTER_API_KEY"]
async fn live_openrouter_text() {
    let Some((client, model, recorder)) = client() else {
        return;
    };
    let mut streamed = String::new();
    let mut on = |e: Event| {
        if let Event::Text(t) = e {
            streamed.push_str(&t);
        }
    };
    let r = client
        .complete(
            &ask(&model, "Reply with the single word: ready", vec![]),
            &mut on,
        )
        .await
        .expect("a live answer");
    assert_eq!(r.stop, StopReason::EndTurn, "{r:?}");
    assert!(streamed.to_lowercase().contains("ready"), "{streamed}");
    assert!(
        r.usage.input + r.usage.cache_read > 0 && r.usage.output > 0,
        "usage reported: {:?}",
        r.usage
    );
    eprintln!("model {} · usage {:?}", r.model, r.usage);
    recorder.save("openrouter-text");
}

#[tokio::test]
#[ignore = "calls OpenRouter: run with MNEMORPH_LIVE=1 and OPENROUTER_API_KEY"]
async fn live_openrouter_tool_round_trip() {
    let Some((client, model, recorder)) = client() else {
        return;
    };
    let read = Tool {
        name: "read_file".into(),
        description: "Read a file of the project and return its contents.".into(),
        input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        cache: false,
        defer: false,
    };
    let mut req = ask(
        &model,
        "Use the read_file tool to read the file named NOTES.md, then tell me its first word.",
        vec![read],
    );
    let mut on = |_: Event| {};
    let first = client.complete(&req, &mut on).await.expect("a tool call");
    assert_eq!(first.stop, StopReason::ToolUse, "{first:?}");
    let (id, input) = first
        .content
        .iter()
        .find_map(|b| match b {
            Block::ToolUse { id, input, .. } => Some((id.clone(), input.clone())),
            _ => None,
        })
        .expect("a tool_use block");
    assert!(
        input["path"].as_str().is_some_and(|p| p.contains("NOTES")),
        "{input}"
    );
    // Answer the call and let the model finish: the whole loop, against a real server.
    req.messages.push(Message {
        role: Role::Assistant,
        content: first.content.clone(),
    });
    req.messages.push(Message {
        role: Role::User,
        content: vec![Block::ToolResult {
            tool_use_id: id,
            content: "Mnemorph is a harness.".into(),
            is_error: false,
            cache: false,
            references: Vec::new(),
        }],
    });
    let second = client
        .complete(&req, &mut on)
        .await
        .expect("an answer after the tool result");
    assert_eq!(second.stop, StopReason::EndTurn, "{second:?}");
    let text: String = second
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.to_lowercase().contains("mnemorph"), "{text}");
    eprintln!(
        "tool round trip on {}: {:?} then {:?}",
        second.model, first.usage, second.usage
    );
    recorder.save("openrouter-tool");
}

#[tokio::test]
#[ignore = "calls OpenRouter: run with MNEMORPH_LIVE=1 and OPENROUTER_API_KEY"]
async fn live_openrouter_reasoning() {
    let Some((client, model, recorder)) = client() else {
        return;
    };
    let mut thought = String::new();
    let mut on = |e: Event| {
        if let Event::Thinking(t) = e {
            thought.push_str(&t);
        }
    };
    let r = client
        .complete(
            &ask(&model, "Is 91 a prime number? Answer yes or no.", vec![]),
            &mut on,
        )
        .await
        .expect("a live answer");
    assert_eq!(r.stop, StopReason::EndTurn, "{r:?}");
    assert!(!thought.is_empty(), "the model reasoned");
    eprintln!("model {} · usage {:?}", r.model, r.usage);
    recorder.save("openrouter-reasoning");
}

/// A model id that does not exist, refused with OpenRouter's error body and its numeric `code`.
#[tokio::test]
#[ignore = "calls OpenRouter: run with MNEMORPH_LIVE=1 and OPENROUTER_API_KEY"]
async fn live_openrouter_unknown_model() {
    let Some((client, _, recorder)) = client() else {
        return;
    };
    let req = ask("mnemorph/no-such-model:free", "hi", vec![]);
    let mut on = |_: Event| {};
    let err = client
        .complete(&req, &mut on)
        .await
        .expect_err("an unknown model is refused");
    eprintln!("{err:?}");
    recorder.save("openrouter-error");
}

/// A prompt over a small free model's context (`liquid/lfm-2.5-2.6b:free`, 65 536 tokens),
/// refused before any byte streams: a 400 with a numeric `code`, never a mid-stream error.
#[tokio::test]
#[ignore = "calls OpenRouter: run with MNEMORPH_LIVE=1 and OPENROUTER_API_KEY"]
async fn live_openrouter_context_overflow() {
    let Some((client, _, recorder)) = client() else {
        return;
    };
    let mut req = ask(
        "liquid/lfm-2.5-2.6b:free",
        &format!("{}Reply ready.", "hello ".repeat(90_000)),
        vec![],
    );
    req.max_tokens = 64;
    let mut on = |_: Event| {};
    let err = client
        .complete(&req, &mut on)
        .await
        .expect_err("an over-long prompt is refused");
    eprintln!("{err:?}");
    recorder.save("openrouter-context");
}
