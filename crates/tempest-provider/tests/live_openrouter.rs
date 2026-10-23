//! Real calls to OpenRouter, only when asked for:
//! `TEMPEST_LIVE=1 OPENROUTER_API_KEY=… cargo nextest run -p tempest-provider --run-ignored all
//! live_openrouter`. The default model is a free one; `TEMPEST_LIVE_OR_MODEL` picks another.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::json;
use tempest_provider::{
    Block, Event, HttpRequest, HttpResponse, HttpTransport, Message, OpenAiCompatible, Policy,
    Request, Role, StopReason, Tool, Transport,
};

/// Wraps the real transport and keeps each response body, so a run can be saved as a cassette
/// (`TEMPEST_RECORD=<dir>`). Bodies hold no credentials: the key travels in a request header.
struct Recorder {
    inner: HttpTransport,
    bodies: Mutex<Vec<Vec<u8>>>,
}

impl Transport for Recorder {
    fn send(
        &self,
        request: HttpRequest,
    ) -> Pin<Box<dyn Future<Output = io::Result<HttpResponse>> + Send + '_>> {
        Box::pin(async move {
            let response = self.inner.send(request).await?;
            let chunks: Vec<io::Result<Bytes>> = response.body.collect().await;
            let mut body = Vec::new();
            for chunk in chunks.iter().flatten() {
                body.extend_from_slice(chunk);
            }
            self.bodies.lock().expect("lock").push(body);
            Ok(HttpResponse {
                status: response.status,
                headers: response.headers,
                body: Box::pin(futures_util::stream::iter(chunks)),
            })
        })
    }
}

impl Recorder {
    /// Saves the bodies as `<dir>/<name>-<n>.sse`, with generation ids blanked.
    fn save(&self, name: &str) {
        let Ok(dir) = std::env::var("TEMPEST_RECORD") else {
            return;
        };
        for (n, body) in self.bodies.lock().expect("lock").iter().enumerate() {
            let text = String::from_utf8_lossy(body);
            let scrubbed = regex_free_scrub(&text);
            std::fs::write(format!("{dir}/{name}-{n}.sse"), scrubbed).expect("write cassette");
        }
    }
}

/// Blanks every `"id":"gen-…"` and `"id":"call_…"`-like generation id without a regex crate.
fn regex_free_scrub(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("\"id\":\"") {
        let start = at + 6;
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail.find('"').unwrap_or(tail.len());
        let id = &tail[..end];
        out.push_str(if id.starts_with("gen-") {
            "gen-recorded"
        } else {
            id
        });
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn client() -> Option<(OpenAiCompatible, String, Arc<Recorder>)> {
    if std::env::var("TEMPEST_LIVE").as_deref() != Ok("1") {
        return None;
    }
    let key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY");
    let model = std::env::var("TEMPEST_LIVE_OR_MODEL")
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
#[ignore = "calls OpenRouter: run with TEMPEST_LIVE=1 and OPENROUTER_API_KEY"]
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
#[ignore = "calls OpenRouter: run with TEMPEST_LIVE=1 and OPENROUTER_API_KEY"]
async fn live_openrouter_tool_round_trip() {
    let Some((client, model, recorder)) = client() else {
        return;
    };
    let read = Tool {
        name: "read_file".into(),
        description: "Read a file of the project and return its contents.".into(),
        input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        cache: false,
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
            content: "Tempest is a harness.".into(),
            is_error: false,
            cache: false,
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
    assert!(text.to_lowercase().contains("tempest"), "{text}");
    eprintln!(
        "tool round trip on {}: {:?} then {:?}",
        second.model, first.usage, second.usage
    );
    recorder.save("openrouter-tool");
}
