//! Replay compaction: a long run stops re-sending its history.

use std::collections::VecDeque;
use std::fs;
use std::sync::{Arc, Mutex};

use mnemorph_ledger::{Journal, Usage};
use mnemorph_loop::compact::{KEEP_RECENT, fold, fold_point};
use mnemorph_loop::model::Completion;
use mnemorph_loop::{Ending, Limits, Model, Run, Setup, solo, validate};
use mnemorph_provider::{
    Block, Caller, Event, Message, Request, Response, Role, StopReason, anthropic_prices, encode,
};
use mnemorph_races::Race;
use mnemorph_tools::{Sandbox, registry};
use serde_json::json;

/// A model whose reported input is the size of what it was sent, as a real one's is — and whose
/// cache works as Anthropic's does: a request reads the longest prefix an earlier request marked,
/// writes up to its own last mark, and pays full price for the rest.
struct Sized {
    script: Mutex<VecDeque<Vec<Block>>>,
    sent: Mutex<Vec<usize>>,
    folded_seen: Mutex<bool>,
    cached: Mutex<std::collections::HashSet<String>>,
    /// Input billed in hundredths of a base-price byte: full, 1.25× written, 0.1× read.
    billed: Mutex<usize>,
}

/// The request as the cache sees it: one string per position, marks stripped, and whether each
/// position carries a mark.
fn positions(request: &Request) -> Vec<(String, bool)> {
    let body = encode(request).unwrap();
    let mut out = Vec::new();
    let mut push = |v: &serde_json::Value, role: &str| {
        let mut v = v.clone();
        let marked = v
            .as_object_mut()
            .and_then(|o| o.remove("cache_control"))
            .is_some();
        out.push((format!("{role}{v}"), marked));
    };
    for t in body["tools"].as_array().into_iter().flatten() {
        push(t, "tool");
    }
    for b in body["system"].as_array().into_iter().flatten() {
        push(b, "system");
    }
    for m in body["messages"].as_array().into_iter().flatten() {
        for b in m["content"].as_array().into_iter().flatten() {
            push(b, m["role"].as_str().unwrap());
        }
    }
    out
}

impl Model for Sized {
    fn provider(&self) -> &'static str {
        "sized"
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        _on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        let bytes = serde_json::to_vec(&encode(request).unwrap()).unwrap().len();
        self.sent.lock().unwrap().push(bytes);
        let (mut read, mut written, mut prefix, mut len) = (0, 0, String::new(), 0);
        let mut cached = self.cached.lock().unwrap();
        for (text, marked) in positions(request) {
            prefix.push_str(&text);
            len += text.len();
            if marked {
                if cached.contains(&prefix) {
                    read = len;
                } else {
                    written = len;
                    cached.insert(prefix.clone());
                }
            }
        }
        let written = written.saturating_sub(read);
        let full = len - read - written;
        *self.billed.lock().unwrap() += 100 * full + 125 * written + 10 * read;
        let folded = request.messages.iter().flat_map(|m| &m.content).any(
            |b| matches!(b, Block::ToolResult { content, .. } if content.starts_with("[folded")),
        );
        *self.folded_seen.lock().unwrap() |= folded;
        validate(&request.messages).unwrap();
        let content = self.script.lock().unwrap().pop_front().unwrap_or_else(|| {
            vec![Block::Text {
                text: "done".into(),
                cache: false,
            }]
        });
        let stop = if content.iter().any(|b| matches!(b, Block::ToolUse { .. })) {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };
        let usage = Usage {
            input: (full / 4) as u64,
            output: 20,
            cache_read: (read / 4) as u64,
            cache_write: (written / 4) as u64,
        };
        Box::pin(async move {
            Ok(Response {
                id: "m".into(),
                model: "m".into(),
                content,
                stop,
                stop_details: None,
                usage,
                warnings: vec![],
            })
        })
    }
}

/// The exit of M5b: a 40-step run whose context would pass the stress line keeps every request
/// under it, and finishes.
#[tokio::test]
async fn a_long_run_stays_under_its_stress_line() {
    let root = std::env::temp_dir().join(format!("mnemorph-compact-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("world")).unwrap();
    let line = "x".repeat(40);
    for i in 0..40 {
        fs::write(
            root.join(format!("world/f{i}.txt")),
            format!("{line}\n").repeat(100),
        )
        .unwrap();
    }
    // A long run that works: it reads, and lands a note every tenth turn (forty turns with
    // nothing landed would rightly stall).
    let script: VecDeque<Vec<Block>> = (0..40)
        .map(|i| {
            let call = if i % 10 == 9 {
                Block::ToolUse {
                    id: format!("t{i}"),
                    name: "write".into(),
                    input: json!({"path": format!("notes{i}.md"), "content": "seen\n"}),
                }
            } else {
                Block::ToolUse {
                    id: format!("t{i}"),
                    name: "read".into(),
                    input: json!({"path": format!("f{i}.txt")}),
                }
            };
            vec![call]
        })
        .collect();
    let model = Arc::new(Sized {
        script: Mutex::new(script),
        sent: Mutex::new(Vec::new()),
        folded_seen: Mutex::new(false),
        cached: Mutex::new(std::collections::HashSet::new()),
        billed: Mutex::new(0),
    });
    let (body, cx) = solo(&root.join("world"), "m");
    let cx = cx
        .with_sandbox(Sandbox::None("compaction test".into()))
        .with_unguarded(true)
        .with_attic(
            mnemorph_tools::Attic::at(&root.join("attic"), &root.join("world")).expect("git"),
        );
    let setup = Setup {
        model: model.clone(),
        tools: registry(),
        cx,
        journal: Journal::new(root.join("run.jsonl")),
        ledger: None,
        prices: anthropic_prices().unwrap(),
        caller: Caller {
            session: "s".into(),
            run: "r".into(),
            body: "rimuru".into(),
            race: "rimuru".into(),
            commission: None,
            prefix: 0,
        },
    };
    let stress = 20_000;
    let limits = Limits {
        steps: 45,
        stress_tokens: stress,
        ..Limits::for_race(Race::Rimuru)
    };
    let mut run = Run::new(body, limits, setup);
    let mut quiet = |_| {};
    let ending = run.ask("read every file", &mut quiet).await;
    let sent = model.sent.lock().unwrap().clone();
    let largest = sent.iter().max().copied().unwrap();
    eprintln!(
        "{} requests, largest {largest} bytes, last {} bytes",
        sent.len(),
        sent.last().unwrap()
    );
    assert_eq!(
        ending,
        Ending::Answered("done".into()),
        "the run finished instead of hitting its stress line"
    );
    assert!(*model.folded_seen.lock().unwrap(), "it compacted");
    // A fold is the one rewrite allowed: it never reads as a moved prefix.
    let moved = Journal::<mnemorph_loop::Beat>::new(root.join("run.jsonl"))
        .read()
        .unwrap()
        .rows
        .into_iter()
        .any(|b| matches!(b, mnemorph_loop::Beat::Warning { text } if text.contains("cached prefix moved")));
    assert!(!moved, "a fold read as a moved prefix");
    assert!(
        largest < usize::try_from(stress * 4).unwrap(),
        "every request stayed under the line: {largest}"
    );
    let unfolded: usize = 36 * 4200;
    assert!(
        largest * 2 < unfolded,
        "far below the history it would have re-sent ({unfolded} bytes)"
    );
    the_history_is_read_from_the_cache(&model, &sent);
    let _ = fs::remove_dir_all(&root);
}

/// Each turn reads what the last one wrote; unmarked, every request would be billed whole.
fn the_history_is_read_from_the_cache(model: &Sized, sent: &[usize]) {
    let billed = *model.billed.lock().unwrap();
    let uncached = 100 * sent.iter().sum::<usize>();
    eprintln!(
        "billed {}% of the uncached input ({} of {} base-price bytes)",
        100 * billed / uncached,
        billed / 100,
        uncached / 100
    );
    assert!(10 * billed < 4 * uncached, "{billed} of {uncached}");
}

#[test]
fn folding_keeps_meaning_and_the_recent_results() {
    let big = "line\n".repeat(100);
    let thinking = Block::Opaque(json!({"type": "thinking", "thinking": "t", "signature": "s"}));
    let mut messages = vec![Message {
        role: Role::User,
        content: vec![Block::Text {
            text: "go".into(),
            cache: false,
        }],
    }];
    for i in 0..(KEEP_RECENT + 3) {
        messages.push(Message {
            role: Role::Assistant,
            content: vec![
                thinking.clone(),
                Block::ToolUse {
                    id: format!("t{i}"),
                    name: "read".into(),
                    input: json!({}),
                },
            ],
        });
        messages.push(Message {
            role: Role::User,
            content: vec![
                Block::ToolResult {
                    tool_use_id: format!("t{i}"),
                    content: big.clone(),
                    is_error: false,
                    cache: false,
                    references: Vec::new(),
                },
                Block::ToolResult {
                    tool_use_id: "small".into(),
                    content: "ok".into(),
                    is_error: false,
                    cache: false,
                    references: Vec::new(),
                },
            ],
        });
    }
    let point = fold_point(&messages);
    let sent = fold(&messages, point);
    assert_eq!(sent.len(), messages.len());
    for (i, (before, after)) in messages.iter().zip(&sent).enumerate() {
        if before.role == Role::Assistant {
            assert_eq!(
                before, after,
                "assistant turns — thinking and calls — are never touched"
            );
        }
        if let (Block::ToolResult { content: a, .. }, Block::ToolResult { content: b, .. }) = (
            &before.content.first().unwrap(),
            &after.content.first().unwrap(),
        ) {
            if i < point {
                assert!(b.starts_with("[folded: 100 lines"), "{b}");
            } else {
                assert_eq!(a, b, "the last {KEEP_RECENT} exchanges stay whole");
            }
        }
        if before.role == Role::User && before.content.len() > 1 {
            assert_eq!(
                before.content[1], after.content[1],
                "a small result is kept"
            );
        }
    }
}

/// A model whose reported occupancy follows a script — a provider's usage after a tool output
/// far larger than the ones before — and that notes whether a request went folded.
struct Jump {
    script: Mutex<VecDeque<(Vec<Block>, u64)>>,
    folded_seen: Mutex<bool>,
}

impl Model for Jump {
    fn provider(&self) -> &'static str {
        "jump"
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        _on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        let folded = request.messages.iter().flat_map(|m| &m.content).any(
            |b| matches!(b, Block::ToolResult { content, .. } if content.starts_with("[folded")),
        );
        *self.folded_seen.lock().unwrap() |= folded;
        let (content, occupied) = self.script.lock().unwrap().pop_front().unwrap_or_else(|| {
            (
                vec![Block::Text {
                    text: "done".into(),
                    cache: false,
                }],
                1_000,
            )
        });
        let stop = if content.iter().any(|b| matches!(b, Block::ToolUse { .. })) {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };
        Box::pin(async move {
            Ok(Response {
                id: "m".into(),
                model: "m".into(),
                content,
                stop,
                stop_details: None,
                usage: Usage {
                    input: occupied,
                    output: 20,
                    cache_read: 0,
                    cache_write: 0,
                },
                warnings: vec![],
            })
        })
    }
}

/// A jump from under the fold mark straight past the stress line folds before it checkpoints:
/// folding the older outputs brings the next request back under the line, and the run finishes.
#[tokio::test]
async fn a_jump_past_the_stress_line_folds_before_it_checkpoints() {
    let root = std::env::temp_dir().join(format!("mnemorph-jump-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("world")).unwrap();
    for i in 0..7 {
        fs::write(
            root.join(format!("world/f{i}.txt")),
            "y".repeat(60).repeat(100) + "\n",
        )
        .unwrap();
    }
    // Under 60% of an 8000-token line for six turns, then a jump past it; then under again.
    let occupied = [2_000, 2_500, 3_000, 3_500, 4_000, 4_500, 9_000];
    let script: VecDeque<(Vec<Block>, u64)> = occupied
        .iter()
        .enumerate()
        .map(|(i, &o)| {
            let call = Block::ToolUse {
                id: format!("t{i}"),
                name: "read".into(),
                input: json!({"path": format!("f{i}.txt")}),
            };
            (vec![call], o)
        })
        .collect();
    let model = Arc::new(Jump {
        script: Mutex::new(script),
        folded_seen: Mutex::new(false),
    });
    let (body, cx) = solo(&root.join("world"), "m");
    let cx = cx
        .with_sandbox(Sandbox::None("compaction test".into()))
        .with_unguarded(true)
        .with_attic(
            mnemorph_tools::Attic::at(&root.join("attic"), &root.join("world")).expect("git"),
        );
    let setup = Setup {
        model: model.clone(),
        tools: registry(),
        cx,
        journal: Journal::new(root.join("run.jsonl")),
        ledger: None,
        prices: anthropic_prices().unwrap(),
        caller: Caller {
            session: "s".into(),
            run: "r".into(),
            body: "rimuru".into(),
            race: "rimuru".into(),
            commission: None,
            prefix: 0,
        },
    };
    let limits = Limits {
        steps: 20,
        stress_tokens: 8_000,
        ..Limits::for_race(Race::Rimuru)
    };
    let mut run = Run::new(body, limits, setup);
    let mut quiet = |_| {};
    let ending = run.ask("read them all", &mut quiet).await;
    let warnings: Vec<String> = Journal::<mnemorph_loop::Beat>::new(root.join("run.jsonl"))
        .read()
        .unwrap()
        .rows
        .into_iter()
        .filter_map(|b| match b {
            mnemorph_loop::Beat::Warning { text } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(ending, Ending::Answered("done".into()), "{warnings:?}");
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("compacted at the stress line")),
        "{warnings:?}"
    );
    assert!(
        *model.folded_seen.lock().unwrap(),
        "the next request went folded"
    );
    let _ = fs::remove_dir_all(&root);
}
