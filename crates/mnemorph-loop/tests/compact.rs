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

/// A model whose reported input is the size of what it was sent, as a real one's is.
struct Sized {
    script: Mutex<VecDeque<Vec<Block>>>,
    sent: Mutex<Vec<usize>>,
    folded_seen: Mutex<bool>,
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
            input: (bytes / 4) as u64,
            output: 20,
            cache_read: 0,
            cache_write: 0,
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
    assert!(
        largest < usize::try_from(stress * 4).unwrap(),
        "every request stayed under the line: {largest}"
    );
    let unfolded: usize = 36 * 4200;
    assert!(
        largest * 2 < unfolded,
        "far below the history it would have re-sent ({unfolded} bytes)"
    );
    let _ = fs::remove_dir_all(&root);
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
                },
                Block::ToolResult {
                    tool_use_id: "small".into(),
                    content: "ok".into(),
                    is_error: false,
                    cache: false,
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
