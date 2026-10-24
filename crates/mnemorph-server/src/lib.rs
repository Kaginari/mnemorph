//! The server: the client protocol over a pair of streams (stdio for the CLI), driving one
//! session. It renders nothing and decides nothing: it turns the run's beats into events, the
//! client's lines into asks, and the human gate into a request the client answers.
//!
//! Three lanes keep it honest. One writer thread owns the output, so lines never interleave or
//! reorder. One reader thread reads the input and answers the gate's waits itself, so a gate
//! blocking a run can never block the read that would release it. One driver task owns the
//! session and takes the human's lines strictly in the order they came.

pub mod protocol;

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
pub type Opener = Box<dyn FnOnce(Arc<dyn Gate>, Observer) -> Result<Opened, String> + Send>;

/// The core's side of one connection.
struct Core {
    out: mpsc::Sender<String>,
    seq: AtomicU64,
    ids: Mutex<(String, String, String)>,
    replay: Mutex<VecDeque<Value>>,
    asks: Mutex<HashMap<u64, mpsc::Sender<GateAnswer>>>,
    next_ask: AtomicU64,
    busy: AtomicBool,
}

impl Core {
    fn send(&self, line: &Value) {
        let _ = self.out.send(line.to_string());
    }

    fn reply(&self, id: &Value, result: &Value) {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }

    fn fail(&self, id: &Value, code: i64, message: &str) {
        self.send(
            &json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        );
    }

    /// Number the event, keep it for a reconnect, and send it.
    fn emit(&self, event: &Event) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let (session, run, body) = self
            .ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let mut params = serde_json::to_value(event).unwrap_or(Value::Null);
        if let Some(o) = params.as_object_mut() {
            o.remove("type");
            o.insert("session".into(), json!(session));
            o.insert("run".into(), json!(run));
            o.insert("body".into(), json!(body));
            o.insert("seq".into(), json!(seq));
        }
        let line = json!({"jsonrpc": "2.0", "method": format!("event/{}", event.kind()), "params": params});
        {
            let mut replay = self.replay.lock().unwrap_or_else(PoisonError::into_inner);
            if replay.len() == REPLAY {
                replay.pop_front();
            }
            replay.push_back(line.clone());
        }
        self.send(&line);
    }

    fn see(&self, seen: &Seen) {
        for event in events(seen) {
            self.emit(&event);
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
        core.asks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, tx);
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
/// input ends or the client asks to shut down. The session opens on `initialize`.
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
    let (out, lines) = mpsc::channel::<String>();
    let writer = std::thread::spawn(move || {
        let mut output = output;
        for line in lines {
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
        seq: AtomicU64::new(0),
        ids: Mutex::new((String::new(), String::new(), "rimuru".into())),
        replay: Mutex::new(VecDeque::new()),
        asks: Mutex::new(HashMap::new()),
        next_ask: AtomicU64::new(0),
        busy: AtomicBool::new(false),
    });
    let inbox = read(input, Arc::clone(&core));
    runtime.block_on(main_lane(
        inbox,
        Arc::clone(&core),
        version.to_owned(),
        opener,
    ));
    drop(core);
    let _ = writer.join();
    Ok(())
}

/// The reader: answers the gate itself, and sends everything else, in order, to the main lane.
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
            match serde_json::from_str::<Incoming>(&line) {
                Ok(msg) if msg.method.is_none() => {
                    if let Some(id) = msg
                        .id
                        .as_ref()
                        .and_then(Value::as_str)
                        .and_then(|s| s.strip_prefix("gate-"))
                        .and_then(|n| n.parse().ok())
                    {
                        core.answered(id, &msg);
                    }
                }
                Ok(msg) => {
                    if requests.send(msg).is_err() {
                        break;
                    }
                }
                Err(e) => core.send(&json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("not JSON-RPC: {e}")}})),
            }
        }
    });
    inbox
}

/// The main lane: the client's requests, one at a time.
async fn main_lane(
    mut inbox: tokio::sync::mpsc::UnboundedReceiver<Incoming>,
    core: Arc<Core>,
    version: String,
    session_maker: Opener,
) {
    let mut pending = Some(session_maker);
    let mut prompts: Option<tokio::sync::mpsc::UnboundedSender<String>> = None;
    let mut interrupt = None;
    let mut driver = None;
    while let Some(msg) = inbox.recv().await {
        let id = msg.id.clone().unwrap_or(Value::Null);
        let method = msg.method.clone().unwrap_or_default();
        match method.as_str() {
            "initialize" => {
                let Some(make) = pending.take() else {
                    core.fail(&id, -32600, "already initialized");
                    continue;
                };
                let seeing = Arc::clone(&core);
                let observer: Observer = Arc::new(move |seen: &Seen| seeing.see(seen));
                match make(Arc::new(ClientGate(Arc::clone(&core))), observer) {
                    Ok(opened) => {
                        core.reply(&id, &hello(&opened, &version));
                        *core.ids.lock().unwrap_or_else(PoisonError::into_inner) = (
                            opened.session_id.clone(),
                            opened.run_id.clone(),
                            "rimuru".into(),
                        );
                        interrupt = Some(opened.session.interrupt());
                        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                        prompts = Some(tx);
                        driver = Some(tokio::spawn(drive(opened, rx, Arc::clone(&core))));
                    }
                    Err(e) => core.fail(&id, -32001, &e),
                }
            }
            "session/prompt" => {
                let Some(tx) = &prompts else {
                    core.fail(&id, -32002, "initialize first");
                    continue;
                };
                let Some(text) = msg.params["text"].as_str().filter(|t| !t.trim().is_empty())
                else {
                    core.fail(&id, -32602, "session/prompt needs `text`");
                    continue;
                };
                let queued = core.busy.swap(true, Ordering::SeqCst);
                if queued {
                    core.emit(&Event::Queued {
                        text: text.to_owned(),
                    });
                }
                let _ = tx.send(text.to_owned());
                core.reply(&id, &json!({"queued": queued}));
            }
            "session/interrupt" => {
                if let Some(i) = &interrupt {
                    i.fire();
                }
                core.reply(&id, &json!({}));
            }
            "session/events" => {
                let since = msg.params["since"].as_u64().unwrap_or(0);
                let events: Vec<Value> = core
                    .replay
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .iter()
                    .filter(|e| e["params"]["seq"].as_u64().is_some_and(|s| s > since))
                    .cloned()
                    .collect();
                core.reply(&id, &json!({"events": events}));
            }
            "shutdown" => {
                core.reply(&id, &json!({}));
                break;
            }
            _ if msg.id.is_some() => core.fail(&id, -32601, &format!("no method `{method}`")),
            _ => {}
        }
    }
    // The input ended or the client said goodbye: stop the turn in flight, let it record.
    if let Some(i) = &interrupt {
        i.fire();
    }
    drop(prompts);
    if let Some(d) = driver {
        let _ = d.await;
    }
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
    })
    .unwrap_or(Value::Null)
}

/// The session's lane: one line at a time, in the order they came.
async fn drive(
    opened: Opened,
    mut prompts: tokio::sync::mpsc::UnboundedReceiver<String>,
    core: Arc<Core>,
) {
    let mut session = opened.session;
    while let Some(line) = prompts.recv().await {
        let streaming = Arc::clone(&core);
        let mut on = move |e: StreamEvent| match e {
            StreamEvent::Text(text) => streaming.emit(&Event::TextDelta { text }),
            StreamEvent::ToolStart { id, name } => streaming.emit(&Event::ToolStart { id, name }),
        };
        session.drive(&line, &mut on).await;
        if prompts.is_empty() {
            core.busy.store(false, Ordering::SeqCst);
        }
    }
}

/// The opener for a world directory: its configuration, its backend, keys from this process's
/// environment.
pub fn opener(world: std::path::PathBuf) -> Opener {
    Box::new(move |gate, observer| {
        mnemorph_loop::open(&world, gate, observer, |var| std::env::var(var).ok())
    })
}
