//! The keys a cassette may hold. The scrubber blanks what it knows; this allowlist catches what it
//! does not: a field a server adds is refused, by the recorder before writing and by the replay
//! tests after, until someone reads it and adds it here (or to the scrubber).

use serde_json::Value;

/// Every key of OpenRouter's chunks, usage, cost and error bodies seen so far.
pub const KNOWN: &[&str] = &[
    "arguments",
    "audio_tokens",
    "cache_write_tokens",
    "cached_tokens",
    "choices",
    "code",
    "completion_tokens",
    "completion_tokens_details",
    "content",
    "cost",
    "cost_details",
    "created",
    "delta",
    "error",
    "finish_reason",
    "format",
    "function",
    "id",
    "image_tokens",
    "index",
    "is_byok",
    "message",
    "model",
    "name",
    "native_finish_reason",
    "object",
    "prompt_tokens",
    "prompt_tokens_details",
    "provider",
    "reasoning",
    "reasoning_details",
    "reasoning_tokens",
    "role",
    "service_tier",
    "text",
    "tool_calls",
    "total_tokens",
    "type",
    "upstream_inference_completions_cost",
    "upstream_inference_cost",
    "upstream_inference_prompt_cost",
    "usage",
    "user_id",
    "video_tokens",
];

/// What in a recorded body is outside the allowlist: unknown keys, at any depth, and lines that
/// are neither `data:` JSON, an SSE comment nor blank. Empty when the body may be kept.
pub fn unknown(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    if body.trim_start().starts_with('{') {
        check(body, &mut found);
    } else {
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() != "[DONE]" {
                    check(data, &mut found);
                }
            } else if !(line.is_empty() || line.starts_with(':')) {
                found.push(format!(
                    "a line that is not data: {}",
                    &line[..line.len().min(40)]
                ));
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn check(json: &str, found: &mut Vec<String>) {
    match serde_json::from_str::<Value>(json) {
        Ok(v) => walk(&v, found),
        Err(e) => found.push(format!("unparsed JSON ({e})")),
    }
}

fn walk(v: &Value, found: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, x) in map {
                if !KNOWN.contains(&k.as_str()) {
                    found.push(format!("key `{k}`"));
                }
                walk(x, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|x| walk(x, found)),
        _ => {}
    }
}
