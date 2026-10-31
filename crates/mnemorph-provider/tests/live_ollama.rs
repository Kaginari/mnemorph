//! Real calls to a local Ollama, only when asked for:
//! `MNEMORPH_LIVE_OLLAMA=1 cargo nextest run -p mnemorph-provider --run-ignored only live_ollama`.
//! The server is `MNEMORPH_OLLAMA_URL` (`http://127.0.0.1:11434`), the model
//! `MNEMORPH_OLLAMA_MODEL` (`qwen2.5:0.5b`). `MNEMORPH_RECORD=tests/cassettes` saves each run's
//! bodies, scrubbed, as the cassettes `tests/cassettes.rs` replays.

use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use mnemorph_provider::{
    Block, Error, Event, HttpRequest, HttpTransport, Message, OpenAiCompatible, Policy, Request,
    Role, StopReason, Tool, Transport, encode_chat,
};
use serde_json::json;

#[path = "support/keys.rs"]
mod keys;
#[path = "support/recorder.rs"]
mod recorder;

use recorder::Recorder;

fn url() -> String {
    std::env::var("MNEMORPH_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into())
}

fn model() -> String {
    std::env::var("MNEMORPH_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:0.5b".into())
}

fn client() -> Option<(OpenAiCompatible, Arc<Recorder>)> {
    if std::env::var("MNEMORPH_LIVE_OLLAMA").as_deref() != Ok("1") {
        return None;
    }
    let policy = Policy::default();
    let recorder = Arc::new(Recorder {
        inner: HttpTransport::new(policy.connect).expect("tls"),
        bodies: Mutex::new(Vec::new()),
    });
    let client = OpenAiCompatible::new(recorder.clone(), &url(), None, policy);
    Some((client, recorder))
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
        max_tokens: 256,
        effort: None,
    }
}

/// A text answer, usage asked for (`stream_options.include_usage`, as the client always sends it).
#[tokio::test]
#[ignore = "calls a local Ollama: run with MNEMORPH_LIVE_OLLAMA=1"]
async fn live_ollama_text() {
    let Some((client, recorder)) = client() else {
        return;
    };
    let mut on = |_: Event| {};
    let r = client
        .complete(
            &ask(&model(), "Reply with the single word: ready", vec![]),
            &mut on,
        )
        .await
        .expect("a live answer");
    assert_eq!(r.stop, StopReason::EndTurn, "{r:?}");
    assert!(r.usage_known && r.usage.output > 0, "{:?}", r.usage);
    eprintln!("model {} · usage {:?}", r.model, r.usage);
    recorder.save("ollama-text");
}

/// The same ask without `stream_options`: what the server sends when usage is not asked for.
#[tokio::test]
#[ignore = "calls a local Ollama: run with MNEMORPH_LIVE_OLLAMA=1"]
async fn live_ollama_text_without_usage() {
    let Some((_, recorder)) = client() else {
        return;
    };
    let mut body = encode_chat(&ask(&model(), "Reply with the single word: ready", vec![]));
    body.as_object_mut()
        .expect("an object")
        .remove("stream_options");
    let response = recorder
        .send(HttpRequest {
            url: format!("{}/v1/chat/completions", url()),
            headers: vec![("content-type".into(), "application/json".into())],
            body: serde_json::to_vec(&body).expect("json"),
        })
        .await
        .expect("an answer");
    assert_eq!(response.status, 200);
    let _: Vec<_> = response.body.collect().await;
    recorder.save("ollama-nousage");
}

#[tokio::test]
#[ignore = "calls a local Ollama: run with MNEMORPH_LIVE_OLLAMA=1"]
async fn live_ollama_tool_call() {
    let Some((client, recorder)) = client() else {
        return;
    };
    let read = Tool {
        name: "read_file".into(),
        description: "Read a file of the project and return its contents.".into(),
        input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        cache: false,
        defer: false,
    };
    let req = ask(
        &model(),
        "Use the read_file tool to read the file named NOTES.md, then tell me its first word.",
        vec![read],
    );
    let mut on = |_: Event| {};
    let r = client.complete(&req, &mut on).await.expect("a tool call");
    assert_eq!(r.stop, StopReason::ToolUse, "{r:?}");
    recorder.save("ollama-tool");
}

/// A model the server does not have, refused with its error body.
#[tokio::test]
#[ignore = "calls a local Ollama: run with MNEMORPH_LIVE_OLLAMA=1"]
async fn live_ollama_unknown_model() {
    let Some((client, recorder)) = client() else {
        return;
    };
    let mut on = |_: Event| {};
    let err = client
        .complete(&ask("mnemorph-no-such-model", "hi", vec![]), &mut on)
        .await
        .expect_err("an unknown model is refused");
    assert!(matches!(err, Error::Api { status: 404, .. }), "{err:?}");
    recorder.save("ollama-error");
}
