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
