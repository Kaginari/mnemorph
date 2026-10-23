//! One attempt loop for every provider: the ladder, the three waits, and the rule that nothing
//! is retried once content has started. A provider brings only its request and its assembler.

use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;

use crate::error::{Error, Wait};
use crate::model::{Event, Response};
use crate::retry::{Policy, retry_after};
use crate::sse::SseReader;
use crate::transport::{HttpRequest, Transport};

/// Builds an answer from a provider's stream, one event's data at a time.
pub(crate) trait Assembler: Default {
    /// Applies one event. `Some` when the answer is complete. Sets `started` once content that
    /// a surface may have shown has begun.
    fn apply(
        &mut self,
        data: &str,
        on: &mut (dyn FnMut(Event) + Send),
        started: &mut bool,
    ) -> Result<Option<Response>, Error>;
}

/// Sends `body` to `url` until it is answered or no attempt is left.
pub(crate) async fn drive<A: Assembler>(
    transport: &dyn Transport,
    policy: &Policy,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
    on: &mut (dyn FnMut(Event) + Send),
) -> Result<Response, Error> {
    let mut attempt = 0;
    loop {
        let mut started = false;
        match once::<A>(transport, policy, url, headers, body, on, &mut started).await {
            Ok(response) => return Ok(response),
            Err((error, after)) => {
                if started || !error.retryable() || attempt >= policy.max_retries {
                    return Err(error);
                }
                attempt += 1;
                tokio::time::sleep(policy.wait(attempt, after)).await;
            }
        }
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
) -> Result<Response, (Error, Option<Duration>)> {
    let request = HttpRequest {
        url: url.to_owned(),
        headers: headers.to_vec(),
        body: body.to_vec(),
    };
    let response = transport
        .send(request)
        .await
        .map_err(|e| (Error::Transport(e.to_string()), None))?;
    let after = retry_after(
        response.header("retry-after-ms"),
        response.header("retry-after"),
    );
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
        return Err((
            Error::Api {
                status: response.status,
                kind,
                message,
            },
            after,
        ));
    }
    let mut reader = SseReader::default();
    let mut assembly = A::default();
    let mut wait = (policy.first_byte, Wait::FirstByte);
    loop {
        let next = tokio::time::timeout(wait.0, chunks.next())
            .await
            .map_err(|_| (Error::Timeout(wait.1), None))?;
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|e| (Error::Transport(e.to_string()), None))?;
        wait = (policy.stall, Wait::Stall);
        for event in reader.feed(&chunk) {
            if let Some(done) = assembly
                .apply(&event.data, on, started)
                .map_err(|e| (e, None))?
            {
                return Ok(done);
            }
        }
    }
    Err((Error::Truncated, None))
}

/// An error body's type and message, in the shape both families use
/// (`{"error": {"type": …, "message": …}}`), else the raw text.
fn api_error(body: &[u8]) -> (String, String) {
    let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let kind = v["error"]["type"]
        .as_str()
        .or_else(|| v["error"]["code"].as_str())
        .unwrap_or("unknown")
        .to_owned();
    let message = v["error"]["message"].as_str().map_or_else(
        || String::from_utf8_lossy(&body[..body.len().min(500)]).into_owned(),
        str::to_owned,
    );
    (kind, message)
}
