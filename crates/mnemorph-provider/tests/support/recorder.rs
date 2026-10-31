//! The recorder the live tests share: it keeps each response body and saves it, scrubbed, as a
//! cassette (`MNEMORPH_RECORD=<dir>`).

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Mutex;

use bytes::Bytes;
use futures_util::StreamExt;
use mnemorph_provider::{HttpRequest, HttpResponse, HttpTransport, Transport};

use super::keys;

/// Wraps the real transport and keeps each response body, so a run can be saved as a cassette
/// (`MNEMORPH_RECORD=<dir>`). Only bodies are kept, never headers: the key travels in a request
/// header, and the response headers carry request ids.
pub struct Recorder {
    pub inner: HttpTransport,
    pub bodies: Mutex<Vec<Vec<u8>>>,
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
    /// Saves the bodies as `<dir>/<name>-<n>.sse` (`.json` for an error body), scrubbed.
    pub fn save(&self, name: &str) {
        let Ok(dir) = std::env::var("MNEMORPH_RECORD") else {
            return;
        };
        for (n, body) in self.bodies.lock().expect("lock").iter().enumerate() {
            let scrubbed = scrub(&String::from_utf8_lossy(body));
            let unknown = keys::unknown(&scrubbed);
            assert!(
                unknown.is_empty(),
                "{name}-{n}: not written, outside the allowlist in tests/support/keys.rs: {unknown:?}"
            );
            let ext = if scrubbed.trim_start().starts_with('{') {
                "json"
            } else {
                "sse"
            };
            std::fs::write(format!("{dir}/{name}-{n}.{ext}"), scrubbed).expect("write cassette");
        }
    }
}

/// Replaces what differs between runs or names a call with stable placeholders: generation ids
/// (`gen-recorded`, `chatcmpl-recorded`), tool call ids (`call-recorded-<n>`, in order of
/// appearance), the account's `user_id` (`user-recorded`), `created` timestamps (`0`) and every
/// number of a local server's `timings` (`0`). A body holding anything like a credential is never
/// written.
fn scrub(text: &str) -> String {
    zero_timings(&scrub_ids(text))
}

/// Every number inside each `"timings":{…}` object (flat, as Ollama sends it) becomes `0`.
fn zero_timings(text: &str) -> String {
    const TIMINGS: &str = "\"timings\":{";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(TIMINGS) {
        let start = at + TIMINGS.len();
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail.find('}').unwrap_or(tail.len());
        let mut number = false;
        for ch in tail[..end].chars() {
            let part = ch.is_ascii_digit() || matches!(ch, '.' | '-' | 'e' | 'E' | '+');
            if part && (number || ch.is_ascii_digit() || ch == '-') && !in_key(&out) {
                if !number {
                    out.push('0');
                    number = true;
                }
            } else {
                number = false;
                out.push(ch);
            }
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// Whether `out` ends inside a quoted string (an odd count of quotes since the last `{`).
fn in_key(out: &str) -> bool {
    let from = out.rfind('{').unwrap_or(0);
    out[from..].matches('"').count() % 2 == 1
}

fn scrub_ids(text: &str) -> String {
    const ID: &str = "\"id\":\"";
    const CREATED: &str = "\"created\":";
    const USER: &str = "\"user_id\":\"";
    let mut out = String::with_capacity(text.len());
    let mut calls: Vec<String> = Vec::new();
    let mut rest = text;
    loop {
        let next = [ID, CREATED, USER]
            .into_iter()
            .filter_map(|key| rest.find(key).map(|at| (at, key)))
            .min();
        let Some((at, key)) = next else { break };
        let start = at + key.len();
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        if key == ID {
            let end = tail.find('"').unwrap_or(tail.len());
            let value = &tail[..end];
            if value.starts_with("gen-") {
                out.push_str("gen-recorded");
            } else if value.starts_with("chatcmpl-") {
                out.push_str("chatcmpl-recorded");
            } else {
                let n = calls.iter().position(|c| c == value).unwrap_or_else(|| {
                    calls.push(value.to_owned());
                    calls.len() - 1
                });
                out.push_str("call-recorded-");
                out.push_str(&n.to_string());
            }
            rest = &tail[end..];
        } else if key == USER {
            let end = tail.find('"').unwrap_or(tail.len());
            out.push_str("user-recorded");
            rest = &tail[end..];
        } else {
            let end = tail
                .find(|ch: char| !ch.is_ascii_digit())
                .unwrap_or(tail.len());
            out.push('0');
            rest = &tail[end..];
        }
    }
    out.push_str(rest);
    assert!(
        !out.contains("sk-or-") && !out.contains("Bearer"),
        "a recorded body holds a credential: not written"
    );
    out
}
