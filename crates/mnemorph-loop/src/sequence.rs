//! The message sequence a provider accepts, checked before every request: a broken conversation
//! fails here, with its reason, instead of at the provider.

use std::collections::BTreeSet;

use mnemorph_provider::{Block, Message, Role};

/// Check a conversation about to be sent.
///
/// # Errors
///
/// The first rule broken: the conversation does not start with the user, two turns of one role
/// follow each other, a turn is empty, a tool call is not answered by the very next user turn
/// (with its results first), or a result answers no call.
pub fn validate(messages: &[Message]) -> Result<(), String> {
    let Some(first) = messages.first() else {
        return Err("the conversation is empty".into());
    };
    if first.role != Role::User {
        return Err("the conversation must start with the user".into());
    }
    if messages.last().is_some_and(|m| m.role != Role::User) {
        return Err("the conversation must end with the user's turn".into());
    }
    let mut open: BTreeSet<String> = BTreeSet::new();
    for (i, m) in messages.iter().enumerate() {
        if m.role == Role::System {
            return Err(format!(
                "message {i}: system notes travel in the system prompt"
            ));
        }
        if m.content.is_empty() {
            return Err(format!("message {i}: an empty turn"));
        }
        if i > 0 && messages[i - 1].role == m.role {
            return Err(format!("message {i}: two {:?} turns in a row", m.role));
        }
        match m.role {
            Role::Assistant => {
                if !open.is_empty() {
                    return Err(format!(
                        "message {i}: tool calls {open:?} were never answered"
                    ));
                }
                for b in &m.content {
                    if let Block::ToolUse { id, .. } = b
                        && !open.insert(id.clone())
                    {
                        return Err(format!(
                            "message {i}: the tool call id `{id}` is used twice"
                        ));
                    }
                }
            }
            Role::User => {
                let mut seen_other = false;
                for b in &m.content {
                    match b {
                        Block::ToolResult { tool_use_id, .. } => {
                            if seen_other {
                                return Err(format!(
                                    "message {i}: a tool result after other content; results come first"
                                ));
                            }
                            if !open.remove(tool_use_id) {
                                return Err(format!(
                                    "message {i}: a result for `{tool_use_id}`, which no open call made"
                                ));
                            }
                        }
                        _ => seen_other = true,
                    }
                }
                if !open.is_empty() {
                    return Err(format!(
                        "message {i}: tool calls {open:?} are not answered in the next turn"
                    ));
                }
            }
            Role::System => {}
        }
    }
    Ok(())
}
