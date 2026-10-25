//! Replay compaction: a pure function over a copy of the conversation. Old tool outputs fold to
//! a caption; every block that carries meaning — the text, the tool calls, the thinking bound to
//! them — stays. The journal is never touched: compaction shapes only what is sent.

use mnemorph_provider::{Block, Message, Role};

/// The last exchanges kept whole when a run compacts.
pub const KEEP_RECENT: usize = 4;

/// The conversation as sent: every tool result in `messages[..upto]` folded to a caption.
#[must_use]
pub fn fold(messages: &[Message], upto: usize) -> Vec<Message> {
    messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if i >= upto || m.role != Role::User {
                return m.clone();
            }
            let content = m
                .content
                .iter()
                .map(|b| match b {
                    Block::ToolResult { tool_use_id, content, is_error, cache } if content.len() > 200 => Block::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: format!(
                            "[folded: {} lines of earlier output — call the tool again if you need it]",
                            content.lines().count()
                        ),
                        is_error: *is_error,
                        cache: *cache,
                    },
                    other => other.clone(),
                })
                .collect();
            Message { role: m.role, content }
        })
        .collect()
}

/// Where to fold up to when a run compacts now: everything but the last [`KEEP_RECENT`]
/// exchanges (an assistant turn and the user turn after it).
#[must_use]
pub fn fold_point(messages: &[Message]) -> usize {
    messages.len().saturating_sub(2 * KEEP_RECENT)
}

/// How much folding `messages[from..upto]` would take out of the next request, in bytes.
#[must_use]
pub fn foldable(messages: &[Message], from: usize, upto: usize) -> usize {
    messages
        .get(from..upto.min(messages.len()))
        .unwrap_or_default()
        .iter()
        .filter(|m| m.role == Role::User)
        .flat_map(|m| &m.content)
        .map(|b| match b {
            Block::ToolResult { content, .. } if content.len() > 200 => content.len(),
            _ => 0,
        })
        .sum()
}

/// The last block of `messages` a cache breakpoint can sit on: a text or a tool result, never
/// in a mid-conversation system message.
fn tail(messages: &[Message]) -> Option<(usize, usize)> {
    messages
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, m)| m.role != Role::System)
        .find_map(|(i, m)| {
            m.content
                .iter()
                .rposition(|b| matches!(b, Block::Text { .. } | Block::ToolResult { .. }))
                .map(|j| (i, j))
        })
}

/// Mark the conversation as sent for the provider's prefix cache, with at most `budget`
/// breakpoints: its tail, so the next turn reads everything up to here, and the previous
/// request's tail — everything before the latest reply is exactly what that request sent — so a
/// turn that appends many blocks still finds the entry the last one wrote.
pub fn mark_cache(messages: &mut [Message], budget: usize) {
    let last_reply = messages.iter().rposition(|m| m.role == Role::Assistant);
    let points = [
        tail(messages),
        last_reply.and_then(|r| tail(&messages[..r])),
    ];
    for (i, j) in points.into_iter().flatten().take(budget) {
        if let Block::Text { cache, .. } | Block::ToolResult { cache, .. } =
            &mut messages[i].content[j]
        {
            *cache = true;
        }
    }
}

/// A conversation as the provider's prefix cache sees it: for each message a mark of its role,
/// then each block — its breakpoints aside, which move every turn and change no byte the cache
/// compares. Blocks added to the last message read as appended, as the cache reads them.
#[must_use]
pub fn fingerprints(messages: &[Message]) -> Vec<u64> {
    use std::hash::{Hash, Hasher};
    let hash = |what: String| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        what.hash(&mut h);
        h.finish()
    };
    let mut out = Vec::new();
    for m in messages {
        out.push(hash(format!("role {:?}", m.role)));
        for b in &m.content {
            let bare = match b {
                Block::Text { text, .. } => Block::Text {
                    text: text.clone(),
                    cache: false,
                },
                Block::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => Block::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: content.clone(),
                    is_error: *is_error,
                    cache: false,
                },
                other => other.clone(),
            };
            out.push(hash(format!("{bare:?}")));
        }
    }
    out
}

/// Where the request `now` first changed what the request `before` sent — an index into their
/// [`fingerprints`] — or `None` when `now` only appends to it: a request is append-only between
/// folds.
#[must_use]
pub fn moved(before: &[u64], now: &[u64]) -> Option<usize> {
    before
        .iter()
        .zip(now)
        .position(|(a, b)| a != b)
        .or_else(|| (now.len() < before.len()).then_some(now.len()))
}
