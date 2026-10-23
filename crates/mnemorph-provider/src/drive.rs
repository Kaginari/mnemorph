//! One attempt loop for every provider: the ladder, the waits, the call's deadline, and the rule
//! that nothing is retried once content has been shown. A provider brings only its request and
//! its assembler.

use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;
use tokio::time::Instant;

use crate::error::{Error, Wait};
use crate::model::{Event, Response};
use crate::retry::{Policy, retry_after};
use crate::sse::SseReader;
use crate::transport::{HttpRequest, Transport};

/// Builds an answer from a provider's stream, one event's data at a time.
pub(crate) trait Assembler: Default {
    /// Applies one event. `Some` when the answer is complete. Sets `started` once something has
    /// been handed to `on` — content a surface may have shown; thinking alone is not.
    fn apply(
        &mut self,
        data: &str,
        on: &mut (dyn FnMut(Event) + Send),
        started: &mut bool,
    ) -> Result<Option<Response>, Error>;
}

/// What one failed attempt says about the next.
struct Failure {
    error: Error,
    after: Option<Duration>,
    /// The server's `x-should-retry`, when it sent one: it overrides the status.
    should_retry: Option<bool>,
}

impl From<Error> for Failure {
    fn from(error: Error) -> Failure {
        Failure {
            error,
            after: None,
            should_retry: None,
        }
    }
}

/// Sends `body` to `url` until it is answered, no attempt is left, or the call's deadline
/// passes.
pub(crate) async fn drive<A: Assembler>(
    transport: &dyn Transport,
    policy: &Policy,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
    on: &mut (dyn FnMut(Event) + Send),
) -> Result<Response, Error> {
    let deadline = Instant::now() + policy.turn;
    let mut attempt = 0;
    loop {
        let mut started = false;
        let left = deadline.saturating_duration_since(Instant::now());
        let outcome = tokio::time::timeout(
            left,
            once::<A>(transport, policy, url, headers, body, on, &mut started),
        )
        .await
        .unwrap_or_else(|_| Err(Error::Timeout(Wait::Turn).into()));
        let failure = match outcome {
            Ok(response) => return Ok(response),
            Err(failure) => failure,
        };
        let retry = failure
            .should_retry
            .unwrap_or_else(|| failure.error.retryable());
        if started || !retry || attempt >= policy.max_retries {
            return Err(failure.error);
        }
        attempt += 1;
        let wait = policy.wait(attempt, failure.after);
        if Instant::now() + wait >= deadline {
            return Err(failure.error);
        }
        tokio::time::sleep(wait).await;
    }
}

async fn once<A: Assembler>(
    transport: &dyn Transport,
    policy: &Policy,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
    on: &mut (dyn FnMut(Event) + Send),
    started: &mut bool,
) -> Result<Response, Failure> {
    let request = HttpRequest {
        url: url.to_owned(),
        headers: headers.to_vec(),
        body: body.to_vec(),
    };
    // The head is waited for like the first byte: a server that accepts and never answers ends.
    let response = tokio::time::timeout(policy.first_byte, transport.send(request))
        .await
        .map_err(|_| Error::Timeout(Wait::FirstByte))?
        .map_err(|e| Error::Transport(e.to_string()))?;
    let after = retry_after(
        response.header("retry-after-ms"),
        response.header("retry-after"),
    );
    let should_retry = match response.header("x-should-retry") {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let mut chunks = response.body;
    if response.status != 200 {
        let mut text = Vec::new();
        while let Ok(Some(Ok(chunk))) = tokio::time::timeout(policy.stall, chunks.next()).await {
            text.extend_from_slice(&chunk);
            if text.len() > 64 * 1024 {
                break;
            }
        }
        let (kind, message) = api_error(&text);
        return Err(Failure {
            error: Error::Api {
                status: response.status,
                kind,
                message,
            },
            after,
            should_retry,
        });
    }
    let mut reader = SseReader::default();
    let mut assembly = A::default();
    let mut wait = (policy.first_byte, Wait::FirstByte);
    loop {
        let next = tokio::time::timeout(wait.0, chunks.next())
            .await
            .map_err(|_| Error::Timeout(wait.1))?;
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|e| Error::Transport(e.to_string()))?;
        if chunk.is_empty() {
            continue;
        }
        wait = (policy.stall, Wait::Stall);
        for event in reader.feed(&chunk) {
            if let Some(done) = assembly.apply(&event.data, on, started)? {
                return Ok(done);
            }
        }
    }
    Err(Error::Truncated.into())
}

/// An error body's type and message, whatever the provider's shape: `{"error": {…}}`, vLLM's
/// top-level `{"object": "error", …}`, an array of either, or plain text.
pub(crate) fn api_error(body: &[u8]) -> (String, String) {
    let parsed: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let first = parsed
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or(parsed);
    let e = if first["error"].is_object() {
        &first["error"]
    } else {
        &first
    };
    let kind = match (&e["type"], &e["code"]) {
        (Value::String(t), _) => t.clone(),
        (_, Value::String(c)) => c.clone(),
        (_, Value::Number(n)) => n.to_string(),
        _ => "unknown".to_owned(),
    };
    let message = e["message"].as_str().map_or_else(
        || String::from_utf8_lossy(&body[..body.len().min(500)]).into_owned(),
        str::to_owned,
    );
    (kind, message)
}
