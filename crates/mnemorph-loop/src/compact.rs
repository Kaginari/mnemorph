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
                    Block::ToolResult { tool_use_id, content, is_error, cache, references } if content.len() > 200 => Block::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: format!(
                            "[folded: {} lines of earlier output — call the tool again if you need it]",
                            content.lines().count()
                        ),
                        is_error: *is_error,
                        cache: *cache,
                        // A deferred tool once loaded stays loaded: its reference is kept.
                        references: references.clone(),
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
    let mut out = Vec::new();
    for m in messages {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        ("role", m.role == Role::User, m.role == Role::System).hash(&mut h);
        out.push(h.finish());
        for b in &m.content {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match b {
                Block::Text { text, .. } => ("text", text).hash(&mut h),
                Block::ToolUse { id, name, input } => {
                    ("use", id, name, input.to_string()).hash(&mut h);
                }
                Block::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => ("result", tool_use_id, content, is_error).hash(&mut h),
                Block::Opaque(v) => ("opaque", v.to_string()).hash(&mut h),
            }
            out.push(h.finish());
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

/// The newest of the conversation a structured compaction keeps verbatim, in bytes of content.
pub const KEEP_BYTES: usize = 40_000;

/// A message's size in bytes of content: its text, its tool calls' arguments, its results.
#[must_use]
pub fn bytes(m: &Message) -> usize {
    m.content
        .iter()
        .map(|b| match b {
            Block::Text { text, .. } => text.len(),
            Block::ToolUse { input, .. } => input.to_string().len(),
            Block::ToolResult { content, .. } => content.len(),
            Block::Opaque(_) => 0,
        })
        .sum()
}

/// Where the verbatim tail of a structured compaction starts: the earliest assistant turn from
/// which the rest of `messages` fits in `budget` bytes — an assistant turn, so every tool result
/// kept has its call beside it. `messages.len()` when even the last exchange does not fit.
#[must_use]
pub fn tail_from(messages: &[Message], budget: usize) -> usize {
    let mut size = 0;
    let mut from = messages.len();
    for (i, m) in messages.iter().enumerate().rev() {
        size += bytes(m);
        if size > budget {
            break;
        }
        if m.role == Role::Assistant {
            from = i;
        }
    }
    from
}

/// The conversation a structured compaction leaves: the ask with the body's summary of all that
/// came before, then the newest `tail` verbatim.
#[must_use]
pub fn compacted(ask: &str, summary: &str, tail: &[Message]) -> Vec<Message> {
    let mut out = vec![Message {
        role: Role::User,
        content: vec![Block::Text {
            text: format!(
                "{ask}\n\n[compacted — the conversation before this point, in your own summary; \
                 the newest exchanges follow verbatim]\n\n{summary}"
            ),
            cache: false,
        }],
    }];
    out.extend(tail.iter().cloned());
    out
}

#[cfg(test)]
mod tests {
    use super::{compacted, tail_from};
    use mnemorph_provider::{Block, Message, Role};

    fn text(role: Role, t: &str) -> Message {
        Message {
            role,
            content: vec![Block::Text {
                text: t.into(),
                cache: false,
            }],
        }
    }

    #[test]
    fn the_tail_starts_at_an_assistant_turn_and_may_be_empty() {
        let small = [
            text(Role::User, "ask"),
            text(Role::Assistant, "a"),
            text(Role::User, "r"),
            text(Role::Assistant, "b"),
            text(Role::User, "s"),
        ];
        assert_eq!(
            tail_from(&small, 1_000),
            1,
            "all but the ask fits, from the first assistant"
        );
        assert_eq!(
            tail_from(&small, 2),
            3,
            "from the last assistant turn that fits"
        );
        let big = [
            text(Role::Assistant, "a"),
            text(Role::User, &"x".repeat(50_000)),
        ];
        assert_eq!(
            tail_from(&big, 40_000),
            2,
            "a result past the budget: nothing kept"
        );
        let alone = compacted("ask", "## Goal\nx", &big[2..]);
        assert_eq!(alone.len(), 1, "the head alone");
        assert_eq!(alone[0].role, Role::User);
    }
}
