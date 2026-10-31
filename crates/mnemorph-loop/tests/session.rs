//! The session: queued lines, Esc, and a conversation that stays valid whatever the order.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use mnemorph_ledger::{Journal, Usage};
use mnemorph_loop::model::Completion;
use mnemorph_loop::{Ending, Interrupt, Limits, Model, Run, Session, Setup, solo, validate};
use mnemorph_provider::{
    Block, Caller, Event, Message, Request, Response, Role, StopReason, anthropic_prices,
};
use mnemorph_races::Race;
use mnemorph_tools::{Sandbox, registry};
use proptest::prelude::*;
use serde_json::json;

/// What the chaos model does on one call.
#[derive(Debug, Clone)]
enum Act {
    Answer,
    Tools(usize),
    /// Fire Esc and never answer: the interrupt must cut the call.
    FireInCall,
    /// Answer with two writes and fire Esc: neither may run.
    FireBeforeRunning,
    /// An empty reply.
    Empty,
    /// A reply cut by the output ceiling.
    Cut,
}

struct Chaos {
    script: Mutex<VecDeque<Act>>,
    interrupt: OnceLock<Arc<Interrupt>>,
    requests: Mutex<Vec<Request>>,
    ids: AtomicUsize,
}

impl Chaos {
    fn new(script: Vec<Act>) -> Self {
        Self {
            script: Mutex::new(script.into()),
            interrupt: OnceLock::new(),
            requests: Mutex::new(Vec::new()),
            ids: AtomicUsize::new(0),
        }
    }

    fn id(&self) -> String {
        format!("t{}", self.ids.fetch_add(1, Ordering::SeqCst))
    }
}

fn response(content: Vec<Block>, stop: StopReason) -> Response {
    Response {
        id: "m".into(),
        model: "m".into(),
        content,
        stop,
        stop_details: None,
        usage: Usage {
            input: 10,
            output: 5,
            cache_read: 0,
            cache_write: 0,
        },
        reported_cost: None,
        warnings: vec![],
    }
}

impl Model for Chaos {
    fn provider(&self) -> &'static str {
        "chaos"
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        _on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        self.requests.lock().unwrap().push(request.clone());
        let act = self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Act::Answer);
        let tool = |name: &str, input: serde_json::Value| Block::ToolUse {
            id: self.id(),
            name: name.into(),
            input,
        };
        match act {
            Act::Answer => Box::pin(async {
                Ok(response(
                    vec![Block::Text {
                        text: "ok".into(),
                        cache: false,
                    }],
                    StopReason::EndTurn,
                ))
            }),
            Act::Tools(n) => {
                let calls = (0..n)
                    .map(|_| tool("glob", json!({"pattern": "*.none"})))
                    .collect();
                Box::pin(async move { Ok(response(calls, StopReason::ToolUse)) })
            }
            Act::FireInCall => {
                self.interrupt.get().unwrap().fire();
                Box::pin(std::future::pending())
            }
            Act::Empty => Box::pin(async { Ok(response(vec![], StopReason::EndTurn)) }),
            Act::Cut => Box::pin(async {
                Ok(response(
                    vec![Block::Text {
                        text: "half".into(),
                        cache: false,
                    }],
                    StopReason::MaxTokens,
                ))
            }),
            Act::FireBeforeRunning => {
                self.interrupt.get().unwrap().fire();
                let calls = vec![
                    tool("write", json!({"path": "never-a.txt", "content": "x"})),
                    tool("write", json!({"path": "never-b.txt", "content": "x"})),
                ];
                Box::pin(async move { Ok(response(calls, StopReason::ToolUse)) })
            }
        }
    }
}

fn session(root: &PathBuf, model: &Arc<Chaos>) -> Session {
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root.join("world")).unwrap();
    let (body, cx) = solo(&root.join("world"), "m");
    let cx = cx
        .with_sandbox(Sandbox::None("session tests run unguarded".into()))
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
    let run = Run::new(body, Limits::for_race(Race::Rimuru), setup);
    model.interrupt.set(run.interrupt()).unwrap();
    Session::new(run)
}

fn root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mnemorph-session-{name}-{}", std::process::id()))
}

fn user(text: &str) -> Message {
    Message {
        role: Role::User,
        content: vec![Block::Text {
            text: text.into(),
            cache: false,
        }],
    }
}

#[test]
fn the_validator_names_what_is_broken() {
    let call = Message {
        role: Role::Assistant,
        content: vec![Block::ToolUse {
            id: "a".into(),
            name: "glob".into(),
            input: json!({}),
        }],
    };
    let result = |id: &str| Message {
        role: Role::User,
        content: vec![Block::ToolResult {
            tool_use_id: id.into(),
            content: "r".into(),
            is_error: false,
            cache: false,
            references: Vec::new(),
        }],
    };
    assert!(validate(&[user("hi"), call.clone(), result("a")]).is_ok());
    assert!(validate(&[]).unwrap_err().contains("empty"));
    assert!(
        validate(std::slice::from_ref(&call))
            .unwrap_err()
            .contains("start with the user")
    );
    assert!(
        validate(&[user("a"), user("b")])
            .unwrap_err()
            .contains("in a row")
    );
    assert!(
        validate(&[user("hi"), call.clone(), user("no result")])
            .unwrap_err()
            .contains("not answered")
    );
    assert!(
        validate(&[user("hi"), call.clone(), result("zzz")])
            .unwrap_err()
            .contains("no open call")
    );
    let late = Message {
        role: Role::User,
        content: vec![
            Block::Text {
                text: "x".into(),
                cache: false,
            },
            Block::ToolResult {
                tool_use_id: "a".into(),
                content: "r".into(),
                is_error: false,
                cache: false,
                references: Vec::new(),
            },
        ],
    };
    assert!(
        validate(&[user("hi"), call, late])
            .unwrap_err()
            .contains("results come first")
    );
}

#[tokio::test]
async fn queued_lines_are_answered_in_order() {
    let r = root("queue");
    let model = Arc::new(Chaos::new(vec![Act::Answer, Act::Answer, Act::Answer]));
    let mut s = session(&r, &model);
    s.queue()
        .lock()
        .unwrap()
        .extend(["second".to_owned(), "third".to_owned()]);
    let mut quiet = |_| {};
    let endings = s.drive("first", &mut quiet).await;
    assert_eq!(endings.len(), 3);
    let asks: Vec<String> = model
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(
            |r| match r.messages.last().unwrap().content.last().unwrap() {
                Block::Text { text, .. } => text.clone(),
                _ => String::new(),
            },
        )
        .collect();
    assert_eq!(asks, ["first", "second", "third"]);
    let _ = fs::remove_dir_all(&r);
}

/// Esc during a model call drops it — nothing half-streamed enters the conversation — and the
/// session pivots to the next queued line.
#[tokio::test]
async fn esc_in_a_call_pivots_to_the_queue() {
    let r = root("esc-call");
    let model = Arc::new(Chaos::new(vec![Act::FireInCall, Act::Answer]));
    let mut s = session(&r, &model);
    s.queue()
        .lock()
        .unwrap()
        .push_back("instead, do this".into());
    let mut quiet = |_| {};
    let endings = s.drive("a long job", &mut quiet).await;
    assert_eq!(
        endings,
        [Ending::Interrupted, Ending::Answered("ok".into())]
    );
    let second = &model.requests.lock().unwrap()[1];
    validate(&second.messages).unwrap();
    let _ = fs::remove_dir_all(&r);
}

/// Esc between tool calls: none of them runs, each is answered as interrupted.
#[tokio::test]
async fn esc_before_the_acts_runs_none_of_them() {
    let r = root("esc-act");
    let model = Arc::new(Chaos::new(vec![Act::FireBeforeRunning, Act::Answer]));
    let mut s = session(&r, &model);
    s.queue().lock().unwrap().push_back("never mind".into());
    let mut quiet = |_| {};
    let endings = s.drive("write two files", &mut quiet).await;
    assert_eq!(endings[0], Ending::Interrupted);
    assert!(!r.join("world/never-a.txt").exists() && !r.join("world/never-b.txt").exists());
    let second = &model.requests.lock().unwrap()[1];
    validate(&second.messages).unwrap();
    let answered = second.messages.iter().flat_map(|m| &m.content).filter(|b| matches!(b, Block::ToolResult { content, is_error: true, .. } if content.contains("interrupted"))).count();
    assert_eq!(answered, 2);
    let _ = fs::remove_dir_all(&r);
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        3 => Just(Act::Answer),
        3 => (1usize..3).prop_map(Act::Tools),
        1 => Just(Act::FireInCall),
        1 => Just(Act::FireBeforeRunning),
        1 => Just(Act::Empty),
        1 => Just(Act::Cut),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Whatever the order of answers, tool calls, interrupts and queued lines, every request the
    /// model receives is a conversation a provider accepts, and no interrupted act ran.
    #[test]
    fn every_request_is_a_valid_conversation(script in prop::collection::vec(act(), 0..14), queued in 0usize..4) {
        let r = root(&format!("prop-{}", std::process::id() as usize ^ script.len() ^ (queued << 8)));
        let model = Arc::new(Chaos::new(script));
        let mut s = session(&r, &model);
        s.queue().lock().unwrap().extend((0..queued).map(|i| format!("queued {i}")));
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        let mut quiet = |_| {};
        let endings = rt.block_on(s.drive("start", &mut quiet));
        // The run validates before each request, so a broken conversation would end the run
        // here instead of reaching the model: no ending may say so.
        for e in &endings {
            prop_assert!(!matches!(e, Ending::Failed(why) if why.contains("malformed")), "{e:?}");
        }
        prop_assert_eq!(endings.len(), queued + 1, "every line is answered or interrupted");
        for (i, req) in model.requests.lock().unwrap().iter().enumerate() {
            prop_assert!(validate(&req.messages).is_ok(), "request {i}: {:?}", validate(&req.messages));
        }
        prop_assert!(!r.join("world/never-a.txt").exists());
        let _ = fs::remove_dir_all(&r);
    }
}
