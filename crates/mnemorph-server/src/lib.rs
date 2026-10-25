//! The server: the client protocol over a pair of streams (stdio for the CLI), driving one
//! session. It renders nothing and decides nothing: it turns the run's beats into events, the
//! client's lines into asks, and the human gate into a request the client answers.
//!
//! Three lanes keep it honest. One writer thread owns the output, so lines never interleave or
//! reorder. One reader thread reads the input and answers the gate's waits itself, so a gate
//! blocking a run can never block the read that would release it. One driver task owns the
//! session and takes the human's lines strictly in the order they came.

pub mod protocol;

/// The world, for the commands that read it without a session (`onto owner`).
pub use mnemorph_loop::world;

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use mnemorph_loop::{Beat, Gate, GateAnswer, GateAsk, Observer, Opened, Seen, StreamEvent};
use serde_json::{Value, json};

use crate::protocol::{
    Event, GateReply, GateRequest, Hello, Incoming, PROTOCOL, REPLAY, RESULT_BYTES,
};

/// How long the gate waits for the human before it denies.
pub const GATE_WAIT: Duration = Duration::from_secs(600);

/// Opens the session once the client says hello, with the gate and observer the server made.
/// It is kept until it succeeds, so a failed `initialize` can be tried again.
pub type Opener = Box<dyn FnMut(Arc<dyn Gate>, Observer) -> Result<Opened, String> + Send>;

/// The core's side of one connection.
struct Core {
    /// The writer's lane; `None` tells it the connection is over.
    out: mpsc::Sender<Option<String>>,
    /// The events kept for a reconnect, and the last `seq` given: one lock, so `seq` reaches the
    /// wire in order.
    replay: Mutex<(u64, VecDeque<Value>)>,
    ids: Mutex<(String, String, String)>,
    asks: Mutex<HashMap<u64, mpsc::Sender<GateAnswer>>>,
    next_ask: AtomicU64,
    /// Lines sent and not yet taken to the end of their turn.
    outstanding: AtomicUsize,
    /// The client left: no gate waits, no queued line runs.
    closed: AtomicBool,
}

impl Core {
    fn send(&self, line: &Value) {
        let _ = self.out.send(Some(line.to_string()));
    }

    /// Answer a request; a notification (no id) gets no answer.
    fn reply(&self, id: Option<&Value>, result: &Value) {
        if let Some(id) = id {
            self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
        }
    }

    fn fail(&self, id: Option<&Value>, code: i64, message: &str) {
        if let Some(id) = id {
            self.send(
                &json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
            );
        }
    }

    /// Deny every act waiting for the human, with `why`.
    fn release(&self, why: &str) {
        let waiting: Vec<_> = self
            .asks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain()
            .collect();
        for (_, tx) in waiting {
            let _ = tx.send(GateAnswer::Deny(why.into()));
        }
    }

    /// The client left: release the gate and stop taking lines.
    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.release("the client left");
    }

    /// Number the event, keep it for a reconnect, and send it — all under one lock, so `seq`
    /// reaches the wire in the order it was given.
    fn emit(&self, event: &Event) {
        self.emit_as(event, None);
    }

    /// As [`Core::emit`], for a commissioned body below the session: its own body and run.
    fn emit_as(&self, event: &Event, below: Option<(&str, &str)>) {
        let (session, mut run, mut body) = self
            .ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some((b, r)) = below {
            b.clone_into(&mut body);
            r.clone_into(&mut run);
        }
        let mut params = serde_json::to_value(event).unwrap_or(Value::Null);
        if let Some(o) = params.as_object_mut() {
            o.remove("type");
            o.insert("session".into(), json!(session));
            o.insert("run".into(), json!(run));
            o.insert("body".into(), json!(body));
        }
        let mut replay = self.replay.lock().unwrap_or_else(PoisonError::into_inner);
        replay.0 += 1;
        if let Some(o) = params.as_object_mut() {
            o.insert("seq".into(), json!(replay.0));
        }
        let line = json!({"jsonrpc": "2.0", "method": format!("event/{}", event.kind()), "params": params});
        if replay.1.len() == REPLAY {
            replay.1.pop_front();
        }
        replay.1.push_back(line.clone());
        self.send(&line);
    }

    fn see(&self, seen: &Seen) {
        // A commissioned body's beat comes wrapped once per rank: the innermost names who.
        let mut below = None;
        let mut inner = seen;
        while let Seen::From { body, run, seen } = inner {
            below = Some((body.as_str(), run.as_str()));
            inner = seen;
        }
        for event in events(inner) {
            self.emit_as(&event, below);
        }
    }

    fn answered(&self, id: u64, msg: &Incoming) {
        let answer = match (&msg.result, &msg.error) {
            (Some(r), _) => match serde_json::from_value::<GateReply>(r.clone()) {
                Ok(GateReply { allow: true, .. }) => GateAnswer::Approve,
                Ok(GateReply { reason, .. }) => {
                    GateAnswer::Deny(reason.unwrap_or_else(|| "the human denied it".into()))
                }
                Err(e) => GateAnswer::Deny(format!("the client's answer did not read: {e}")),
            },
            _ => GateAnswer::Deny("the client could not ask the human".into()),
        };
        if let Some(waiting) = self
            .asks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id)
        {
            let _ = waiting.send(answer);
        }
    }
}

/// The gate a client answers: `gate/ask`, sent as a request; silence denies.
struct ClientGate(Arc<Core>);

impl Gate for ClientGate {
    fn ask(&self, ask: &GateAsk) -> GateAnswer {
        let core = &self.0;
        let id = core.next_ask.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = mpsc::channel();
        {
            let mut asks = core.asks.lock().unwrap_or_else(PoisonError::into_inner);
            // Checked under the lock `close` drains, so no ask slips in after it.
            if core.closed.load(Ordering::SeqCst) {
                return GateAnswer::Deny("the client left".into());
            }
            asks.insert(id, tx);
        }
        let request = GateRequest {
            race: ask.race.name().to_owned(),
            class: ask.class.to_string(),
            act: ask.act.clone(),
            why: ask.why.clone(),
            outside: ask
                .outside
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
        };
        core.send(&json!({"jsonrpc": "2.0", "id": format!("gate-{id}"), "method": "gate/ask", "params": request}));
        let answer = rx
            .recv_timeout(GATE_WAIT)
            .unwrap_or_else(|_| GateAnswer::Deny("the human did not answer in 10 minutes".into()));
        core.asks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id);
        answer
    }
}

/// The events one thing the run did becomes.
#[must_use]
pub fn events(seen: &Seen) -> Vec<Event> {
    match seen {
        Seen::Usage {
            usage,
            usd,
            wt,
            spent_wt,
            occupancy,
            stress_line,
        } => vec![Event::Usage {
            input: usage.input,
            output: usage.output,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
            usd: *usd,
            wt: *wt,
            spent_wt: *spent_wt,
            occupancy: *occupancy,
            stress_line: *stress_line,
        }],
        Seen::Beat(beat) => beat_events(beat),
        Seen::From { seen, .. } => events(seen),
    }
}

fn beat_events(beat: &Beat) -> Vec<Event> {
    match beat {
        Beat::Ask { text } => vec![Event::TurnStarted { text: text.clone() }],
        Beat::Model { content, stop } => {
            let text: String = content
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect();
            let mut out = vec![Event::Assistant {
                text,
                stop: stop.clone(),
            }];
            out.extend(content.iter().filter(|b| b["type"] == "tool_use").map(|b| {
                Event::ToolCall {
                    id: b["id"].as_str().unwrap_or_default().to_owned(),
                    name: b["name"].as_str().unwrap_or_default().to_owned(),
                    input: b["input"].clone(),
                }
            }));
            out
        }
        Beat::Result {
            tool_use_id,
            content,
            is_error,
        } => {
            let mut end = content.len().min(RESULT_BYTES);
            while !content.is_char_boundary(end) {
                end -= 1;
            }
            vec![Event::ToolResult {
                id: tool_use_id.clone(),
                content: content[..end].to_owned(),
                is_error: *is_error,
                cut: content.len() - end,
            }]
        }
        Beat::Note { text } => vec![Event::Note { text: text.clone() }],
        Beat::Warning { text } => vec![Event::Warning { text: text.clone() }],
        Beat::Gate { changed, failures } => vec![Event::GateVerdict {
            passed: failures.is_empty(),
            changed: changed.clone(),
            failures: failures.clone(),
        }],
        Beat::End { ending } => vec![Event::TurnEnded {
            ending: ending.clone(),
        }],
    }
}

/// Serve one connection: read JSON-RPC lines from `input`, write them to `output`, until the
/// input ends or the client asks to shut down. The session opens on `initialize`. On the way out
/// every act waiting at the gate is denied, the turn in flight is stopped, queued lines are
/// dropped, and the output is flushed before this returns.
///
/// # Errors
///
/// The runtime cannot start.
pub fn serve(
    input: impl BufRead + Send + 'static,
    output: impl Write + Send + 'static,
    version: &str,
    opener: Opener,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("runtime: {e}"))?;
    let (out, lines) = mpsc::channel::<Option<String>>();
    let writer = std::thread::spawn(move || {
        let mut output = output;
        while let Ok(Some(line)) = lines.recv() {
            if writeln!(output, "{line}")
                .and_then(|()| output.flush())
                .is_err()
            {
                break;
            }
        }
    });
    let core = Arc::new(Core {
        out,
        replay: Mutex::new((0, VecDeque::new())),
        ids: Mutex::new((String::new(), String::new(), "rimuru".into())),
        asks: Mutex::new(HashMap::new()),
        next_ask: AtomicU64::new(0),
        outstanding: AtomicUsize::new(0),
        closed: AtomicBool::new(false),
    });
    let inbox = read(input, Arc::clone(&core));
    runtime.block_on(main_lane(
        inbox,
        Arc::clone(&core),
        version.to_owned(),
        opener,
    ));
    // Every line is written; the writer ends even while the reader still waits on its input.
    let _ = core.out.send(None);
    let _ = writer.join();
    // The reader thread may still be blocked reading; it holds nothing that needs it.
    runtime.shutdown_background();
    Ok(())
}

/// The reader: answers the gate itself, and sends everything else, in order, to the main lane.
/// When the input ends, the client is gone: the gate is released at once.
fn read(
    input: impl BufRead + Send + 'static,
    core: Arc<Core>,
) -> tokio::sync::mpsc::UnboundedReceiver<Incoming> {
    let (requests, inbox) = tokio::sync::mpsc::unbounded_channel::<Incoming>();
    std::thread::spawn(move || {
        for line in input.lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let value = match serde_json::from_str::<Value>(&line) {
                Ok(v) => v,
                Err(e) => {
                    core.send(&json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("not JSON: {e}")}}));
                    continue;
                }
            };
            if !value.is_object() {
                let why = if value.is_array() {
                    "batches are not supported: send one message per line"
                } else {
                    "a message is a JSON object"
                };
                core.send(&json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": why}}));
                continue;
            }
            let msg: Incoming = match serde_json::from_value(value) {
                Ok(m) => m,
                Err(e) => {
                    core.send(&json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": format!("not a JSON-RPC message: {e}")}}));
                    continue;
                }
            };
            if msg.method.is_none() {
                // A response: only the gate's are expected.
                if let Some(id) = msg
                    .id
                    .as_ref()
                    .and_then(Value::as_str)
                    .and_then(|s| s.strip_prefix("gate-"))
                    .and_then(|n| n.parse().ok())
                {
                    core.answered(id, &msg);
                }
            } else if requests.send(msg).is_err() {
                break;
            }
        }
        core.close();
    });
    inbox
}

/// The main lane: the client's requests, one at a time.
async fn main_lane(
    mut inbox: tokio::sync::mpsc::UnboundedReceiver<Incoming>,
    core: Arc<Core>,
    version: String,
    mut session_maker: Opener,
) {
    let mut prompts: Option<tokio::sync::mpsc::UnboundedSender<String>> = None;
    let mut steer = None;
    let mut interrupt = None;
    let mut driver = None;
    while let Some(msg) = inbox.recv().await {
        let id = msg.id.as_ref();
        let method = msg.method.clone().unwrap_or_default();
        match method.as_str() {
            "initialize" => {
                if prompts.is_some() {
                    core.fail(id, -32600, "already initialized");
                    continue;
                }
                let seeing = Arc::clone(&core);
                let observer: Observer = Arc::new(move |seen: &Seen| seeing.see(seen));
                match session_maker(Arc::new(ClientGate(Arc::clone(&core))), observer) {
                    Ok(opened) => {
                        core.reply(id, &hello(&opened, &version));
                        *core.ids.lock().unwrap_or_else(PoisonError::into_inner) = (
                            opened.session_id.clone(),
                            opened.run_id.clone(),
                            "rimuru".into(),
                        );
                        interrupt = Some(opened.session.interrupt());
                        steer = Some(opened.session.steer());
                        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                        prompts = Some(tx);
                        driver = Some(tokio::spawn(drive(opened, rx, Arc::clone(&core))));
                    }
                    Err(e) => core.fail(id, -32001, &e),
                }
            }
            "session/prompt" => {
                let Some(tx) = &prompts else {
                    core.fail(id, -32002, "initialize first");
                    continue;
                };
                let Some(text) = msg.params["text"].as_str().filter(|t| !t.trim().is_empty())
                else {
                    core.fail(id, -32602, "session/prompt needs `text`");
                    continue;
                };
                let queued = core.outstanding.fetch_add(1, Ordering::SeqCst) > 0;
                if queued {
                    core.emit(&Event::Queued {
                        text: text.to_owned(),
                    });
                }
                let _ = tx.send(text.to_owned());
                core.reply(id, &json!({"queued": queued}));
            }
            "session/send" => match &steer {
                Some(lane) => steer_with(&core, id, lane, &msg.params),
                None => core.fail(id, -32002, "initialize first"),
            },
            "session/interrupt" => {
                if let Some(i) = &interrupt {
                    i.fire();
                }
                // An act waiting for the human is part of the turn Esc stops.
                core.release("the human interrupted");
                core.reply(id, &json!({}));
            }
            "session/events" => replay(&core, id, msg.params["since"].as_u64().unwrap_or(0)),
            "shutdown" => {
                core.reply(id, &json!({}));
                break;
            }
            _ => core.fail(id, -32601, &format!("no method `{method}`")),
        }
    }
    // The input ended or the client said goodbye: nothing more runs. Queued lines are dropped,
    // the gate is released, the turn in flight is stopped and allowed to record.
    core.close();
    if let Some(i) = &interrupt {
        i.fire();
    }
    drop(prompts);
    if let Some(d) = driver {
        let _ = d.await;
    }
}

/// `session/send`: a line for the running turn's next tool step; refused when nothing runs.
fn steer_with(core: &Core, id: Option<&Value>, lane: &mnemorph_loop::Steer, params: &Value) {
    let Some(text) = params["text"].as_str().filter(|t| !t.trim().is_empty()) else {
        return core.fail(id, -32602, "session/send needs `text`");
    };
    if core.outstanding.load(Ordering::SeqCst) == 0 {
        return core.fail(
            id,
            -32003,
            "nothing runs: a line sent steers a running turn — send it as a message instead",
        );
    }
    lane.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push_back(text.to_owned());
    core.reply(id, &json!({}));
}

/// `session/events`: every kept event after `since`, and where the kept ones begin — so a gap
/// is never silent.
fn replay(core: &Core, id: Option<&Value>, since: u64) {
    let replay = core.replay.lock().unwrap_or_else(PoisonError::into_inner);
    let oldest = replay.1.front().and_then(|e| e["params"]["seq"].as_u64());
    let events: Vec<Value> = replay
        .1
        .iter()
        .filter(|e| e["params"]["seq"].as_u64().is_some_and(|s| s > since))
        .cloned()
        .collect();
    let complete = oldest.is_none_or(|o| o <= since + 1);
    core.reply(
        id,
        &json!({"events": events, "oldest": oldest, "complete": complete}),
    );
}

fn hello(opened: &Opened, version: &str) -> Value {
    serde_json::to_value(Hello {
        protocol: PROTOCOL,
        version: version.to_owned(),
        world: opened.world.display().to_string(),
        model: opened.model.clone(),
        provider: opened.provider.clone(),
        session: opened.session_id.clone(),
        run: opened.run_id.clone(),
        holes: opened.holes.clone(),
        budget: opened.budget.map(|b| protocol::Budget {
            left_wt: b.wt,
            cap: b.cap.to_owned(),
        }),
        key: opened.key.clone(),
        sandbox: opened.sandbox.clone(),
    })
    .unwrap_or(Value::Null)
}

/// The session's lane: one line at a time, in the order they came, until the client leaves.
async fn drive(
    opened: Opened,
    mut lines: tokio::sync::mpsc::UnboundedReceiver<String>,
    core: Arc<Core>,
) {
    let mut session = opened.session;
    while let Some(line) = lines.recv().await {
        if core.closed.load(Ordering::SeqCst) {
            break;
        }
        let streaming = Arc::clone(&core);
        let mut on = move |e: StreamEvent| match e {
            StreamEvent::Text(text) => streaming.emit(&Event::TextDelta { text }),
            StreamEvent::ToolStart { id, name } => streaming.emit(&Event::ToolStart { id, name }),
        };
        session.drive(&line, &mut on).await;
        core.outstanding.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The opener for a world directory: its configuration, its backend, keys from this process's
/// environment.
pub fn opener(world: std::path::PathBuf) -> Opener {
    Box::new(move |gate, observer| {
        mnemorph_loop::open(&world, gate, observer, |var| std::env::var(var).ok())
    })
}
