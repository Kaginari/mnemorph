//! The run's journal: one append-only line per beat, enough to rebuild the conversation exactly.

use mnemorph_provider::{Block, Message, Role};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One line of a run's journal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "beat", rename_all = "snake_case")]
pub enum Beat {
    /// The human's ask (or a queued line) that starts or continues the run.
    Ask {
        /// The text.
        text: String,
    },
    /// What the model answered: its blocks, kept whole (thinking included) so a resume sends them
    /// back unchanged.
    Model {
        /// The response's blocks, as stored by [`block_json`].
        content: Vec<Value>,
        /// Why it stopped.
        stop: String,
    },
    /// What a tool returned.
    Result {
        /// The tool call it answers.
        tool_use_id: String,
        /// Its text.
        content: String,
        /// Whether it failed.
        is_error: bool,
    },
    /// A note the loop added to the conversation (a guts warning, a salvage instruction).
    Note {
        /// The text.
        text: String,
    },
    /// A warning from the provider (a tool call cut by the output ceiling, …): journaled for the
    /// human, never fed back to the model.
    Warning {
        /// The text.
        text: String,
    },
    /// The end gate's verdict on an answer.
    Gate {
        /// The paths the run changed.
        changed: Vec<String>,
        /// What failed; empty when it passed.
        failures: Vec<String>,
    },
    /// One act as the guts read it, with the instruments a learned value model will train on.
    /// Journaled for the record, never fed back to the model.
    Guts {
        /// The tool call it reads.
        tool_use_id: String,
        /// The tool.
        tool: String,
        /// The act's score: +2 a landed write, −1 a failed act or an unchanged re-read, else 0.
        score: i32,
        /// The guts window's sum after the act.
        window: i32,
        /// The guts window fell below its floor on this act: a warning, or the stop that follows the
        /// last one.
        warned: bool,
        /// The act failed.
        failed: bool,
        /// Files it wrote.
        wrote: u32,
        /// Lines it added to them, as a multiset: a moved line is neither added nor removed.
        lines_added: u32,
        /// Lines it removed from them.
        lines_removed: u32,
        /// An edit whose `old_string` was not in the file.
        edit_miss: bool,
        /// The bench it ran, if any: a test runner or a verification script.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bench: Option<String>,
        /// How that bench run counted: green, red, muted, read green or read red.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bench_outcome: Option<String>,
    },
    /// How the run ended.
    End {
        /// The ending, in words.
        ending: String,
    },
}

/// A block as the journal keeps it: lossless, provider-neutral.
#[must_use]
pub fn block_json(b: &Block) -> Value {
    match b {
        Block::Text { text, cache } => json!({"type": "text", "text": text, "cache": cache}),
        Block::ToolUse { id, name, input } => {
            json!({"type": "tool_use", "id": id, "name": name, "input": input})
        }
        Block::ToolResult {
            tool_use_id,
            content,
            is_error,
            cache,
        } => {
            json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": content, "is_error": is_error, "cache": cache})
        }
        Block::Opaque(v) => json!({"type": "opaque", "value": v}),
    }
}

/// A block back from the journal.
#[must_use]
pub fn json_block(v: &Value) -> Option<Block> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let b = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
    Some(match v.get("type")?.as_str()? {
        "text" => Block::Text {
            text: s("text")?,
            cache: b("cache"),
        },
        "tool_use" => Block::ToolUse {
            id: s("id")?,
            name: s("name")?,
            input: v.get("input")?.clone(),
        },
        "tool_result" => Block::ToolResult {
            tool_use_id: s("tool_use_id")?,
            content: s("content")?,
            is_error: b("is_error"),
            cache: b("cache"),
        },
        "opaque" => Block::Opaque(v.get("value")?.clone()),
        _ => return None,
    })
}

/// Rebuild the conversation a journal records. A tool call the journal shows no result for —
/// the run stopped between the model's ask and the tool's answer — is answered with an error:
/// it may not have run, and an act cut mid-flight is gated again, never replayed.
#[must_use]
pub fn conversation(beats: &[Beat]) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let close = |messages: &mut Vec<Message>, pending: &mut Vec<String>| {
        if pending.is_empty() {
            return;
        }
        let content = pending
            .drain(..)
            .map(|id| Block::ToolResult {
                tool_use_id: id,
                content: "the run stopped before this call answered; it may not have run — ask again if it is still needed".into(),
                is_error: true,
                cache: false,
            })
            .collect();
        push_user(messages, content);
    };
    for beat in beats {
        match beat {
            Beat::Ask { text } | Beat::Note { text } => {
                close(&mut messages, &mut pending);
                push_user(
                    &mut messages,
                    vec![Block::Text {
                        text: text.clone(),
                        cache: false,
                    }],
                );
            }
            Beat::Model { content, .. } => {
                close(&mut messages, &mut pending);
                let blocks = clean(content.iter().filter_map(json_block).collect());
                pending = blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::ToolUse { id, .. } => Some(id.clone()),
                        _ => None,
                    })
                    .collect();
                messages.push(Message {
                    role: Role::Assistant,
                    content: blocks,
                });
            }
            Beat::Result {
                tool_use_id,
                content,
                is_error,
            } => {
                pending.retain(|id| id != tool_use_id);
                push_user(
                    &mut messages,
                    vec![Block::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: content.clone(),
                        is_error: *is_error,
                        cache: false,
                    }],
                );
            }
            Beat::End { .. } => close(&mut messages, &mut pending),
            Beat::Gate { .. } | Beat::Warning { .. } | Beat::Guts { .. } => {}
        }
    }
    close(&mut messages, &mut pending);
    messages
}

/// Append blocks as the user's turn, merging into the last user message so results of one model
/// turn, and a note after them, travel together.
pub(crate) fn push_user(messages: &mut Vec<Message>, content: Vec<Block>) {
    match messages.last_mut() {
        Some(m) if m.role == Role::User => m.content.extend(content),
        _ => messages.push(Message {
            role: Role::User,
            content,
        }),
    }
}

/// What the model returned, made fit to send back: empty text blocks dropped, a repeated tool
/// call id made unique, and an empty reply stated as such — a provider refuses all three, and
/// the loop must never build a conversation it would refuse.
#[must_use]
pub fn clean(content: Vec<Block>) -> Vec<Block> {
    let mut ids = std::collections::BTreeSet::new();
    let mut out: Vec<Block> = content
        .into_iter()
        .filter(|b| !matches!(b, Block::Text { text, .. } if text.trim().is_empty()))
        .map(|b| match b {
            Block::ToolUse { id, name, input } => {
                let mut unique = id.clone();
                let mut n = 2;
                while !ids.insert(unique.clone()) {
                    unique = format!("{id}-{n}");
                    n += 1;
                }
                Block::ToolUse {
                    id: unique,
                    name,
                    input,
                }
            }
            other => other,
        })
        .collect();
    if out.is_empty() {
        out.push(Block::Text {
            text: "(the model returned nothing)".into(),
            cache: false,
        });
    }
    out
}
