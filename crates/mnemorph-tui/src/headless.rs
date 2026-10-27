//! `mnemorph run`: one ask, without the terminal app, through the same session — the same
//! opening, the same loop, the same end gate. Progress goes to stderr, the answer to stdout; with
//! `--json`, every event goes to stdout as a JSON line (the client protocol's event, named by
//! `type`, with its `session`, `run`, `body` and `seq`), and a last `result` line says how it
//! ended. No one is there to approve a gated act: the gate denies it, said on stderr, unless the
//! human asked to be put the question on the terminal.

use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use mnemorph_loop::{Ending, GateAnswer, GateAsk, Observer, Opened, Seen, StreamEvent};
use mnemorph_server::SESSION_BODY;
use mnemorph_server::protocol::Event;
use serde_json::{Value, json};

use crate::blocks::summary;
use crate::safe::{clean, one_line};

/// What the session sends the printer.
#[derive(Debug)]
pub enum Out {
    /// An event of the session's own body (`below: None`), or of a commissioned body's: its name
    /// and run.
    Event {
        /// The event.
        event: Event,
        /// The body below the session, and its run.
        below: Option<(String, String)>,
    },
    /// A gated act the gate denied, in words for stderr.
    Denied(String),
}

/// The channel the session's observer, stream and gate send through.
#[must_use]
pub fn channel() -> (Sender<Out>, Receiver<Out>) {
    mpsc::channel()
}

/// The observer: what the run did, as the server's events.
#[must_use]
pub fn observer(tx: Sender<Out>) -> Observer {
    Arc::new(move |seen: &Seen| {
        let mut below = None;
        let mut inner = seen;
        while let Seen::From { body, run, seen } = inner {
            below = Some((body.clone(), run.clone()));
            inner = seen;
        }
        for event in mnemorph_server::events(inner) {
            let _ = tx.send(Out::Event {
                event,
                below: below.clone(),
            });
        }
    })
}

/// The gate of a run with no app: it denies every act, and says so — unless `tty`, when it puts
/// the question to the human on the controlling terminal, where silence still denies. It never
/// approves on its own.
pub struct Headless {
    tx: Sender<Out>,
    tty: bool,
}

/// The gate for `mnemorph run`.
#[must_use]
pub fn gate(tx: Sender<Out>, tty: bool) -> Arc<Headless> {
    Arc::new(Headless { tx, tty })
}

impl mnemorph_loop::Gate for Headless {
    fn ask(&self, ask: &GateAsk) -> GateAnswer {
        let answer = if self.tty {
            mnemorph_loop::Gate::ask(&mnemorph_tools::gate::Terminal::default(), ask)
        } else {
            GateAnswer::Deny(
                "mnemorph run has no human to approve it; ask in a session instead, or run with \
--gate-tty"
                    .into(),
            )
        };
        if let GateAnswer::Deny(why) = &answer {
            let _ = self.tx.send(Out::Denied(format!(
                "denied {} act `{}` — {}",
                ask.class,
                one_line(&ask.act),
                one_line(why)
            )));
        }
        answer
    }
}

/// The exit code of an ending: 0 answered (the end gate passed), 3 a checkpoint, else 1.
#[must_use]
pub fn code(ending: Option<&Ending>) -> u8 {
    match ending {
        Some(Ending::Answered(_)) => 0,
        Some(Ending::Checkpoint { .. }) => 3,
        _ => 1,
    }
}

/// What the stderr printer keeps between events.
struct Progress {
    /// The last character written to stderr by the streamed text.
    last: char,
    seq: u64,
    calls: u64,
    usage: [u64; 4],
    usd: Option<f64>,
    gate: Option<Value>,
}

/// Run `ask` on `opened`'s session, its observer, stream and gate sending to `tx`/`rx`, printing to
/// `out` and `err`. Returns the exit code.
///
/// # Errors
///
/// The runtime cannot start, or the session's task died.
pub fn run(
    opened: Opened,
    ask: &str,
    json: bool,
    (tx, rx): (Sender<Out>, Receiver<Out>),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("runtime: {e}"))?;
    let (session_id, run_id) = (opened.session_id.clone(), opened.run_id.clone());
    let mut session = opened.session;
    let ask = ask.to_owned();
    let streaming = tx;
    let task = runtime.spawn(async move {
        let mut on = move |e: StreamEvent| {
            let event = match e {
                StreamEvent::Text(text) => Event::TextDelta { text },
                StreamEvent::ToolStart { id, name } => Event::ToolStart { id, name },
                StreamEvent::ToolInput { .. } | StreamEvent::Thinking(_) => return,
            };
            let _ = streaming.send(Out::Event { event, below: None });
        };
        session.drive(&ask, &mut on).await
    });
    let mut p = Progress {
        last: '\n',
        seq: 0,
        calls: 0,
        usage: [0; 4],
        usd: Some(0.0),
        gate: None,
    };
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(msg) => print(&msg, json, &session_id, &run_id, &mut p, out, err),
            Err(RecvTimeoutError::Timeout) if task.is_finished() => break,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    let endings = runtime
        .block_on(task)
        .map_err(|e| format!("the session's task died: {e}"))?;
    // What the session sent before its task ended, and was not read yet.
    while let Ok(msg) = rx.try_recv() {
        print(&msg, json, &session_id, &run_id, &mut p, out, err);
    }
    let ending = endings.last();
    let words = ending.map_or_else(|| "failed: no turn ran".to_owned(), Ending::words);
    if p.last != '\n' {
        let _ = writeln!(err);
    }
    if json {
        let answer = match ending {
            Some(Ending::Answered(a)) => json!(a),
            _ => Value::Null,
        };
        let [input, output, cache_read, cache_write] = p.usage;
        let result = json!({
            "type": "result",
            "session": session_id,
            "run": run_id,
            "ending": words,
            "answered": matches!(ending, Some(Ending::Answered(_))),
            "answer": answer,
            "gate": p.gate,
            "usage": {
                "calls": p.calls, "input": input, "output": output,
                "cacheRead": cache_read, "cacheWrite": cache_write, "usd": p.usd,
            },
            "exit": code(ending),
        });
        let _ = writeln!(out, "{result}");
    } else if let Some(Ending::Answered(answer)) = ending {
        let answer = clean(answer);
        let _ = write!(out, "{answer}");
        if !answer.ends_with('\n') {
            let _ = writeln!(out);
        }
    }
    if !matches!(ending, Some(Ending::Answered(_))) {
        // The last line on stderr: how it ended, for a script to read.
        let _ = writeln!(err, "mnemorph: {}", one_line(&words));
    }
    let _ = out.flush();
    let _ = err.flush();
    drop(runtime);
    Ok(code(ending))
}

/// Print one message: a JSON line on stdout with `json`, and plain progress on stderr either way.
fn print(
    msg: &Out,
    json: bool,
    session: &str,
    run: &str,
    p: &mut Progress,
    out: &mut dyn Write,
    err: &mut dyn Write,
) {
    let (event, below) = match msg {
        Out::Denied(line) => {
            newline(p, err);
            let _ = writeln!(err, "mnemorph: {line}");
            return;
        }
        Out::Event { event, below } => (event, below.as_ref()),
    };
    if let Event::Usage {
        input,
        output,
        cache_read,
        cache_write,
        usd,
        ..
    } = event
    {
        p.calls += 1;
        for (sum, n) in p
            .usage
            .iter_mut()
            .zip([input, output, cache_read, cache_write])
        {
            *sum += n;
        }
        p.usd = p.usd.zip(*usd).map(|(a, b)| a + b);
    }
    if json {
        p.seq += 1;
        let mut line = serde_json::to_value(event).unwrap_or(Value::Null);
        if let Some(o) = line.as_object_mut() {
            let (body, run) = below.map_or((SESSION_BODY, run), |(b, r)| (b.as_str(), r.as_str()));
            o.insert("session".into(), json!(session));
            o.insert("run".into(), json!(run));
            o.insert("body".into(), json!(body));
            o.insert("seq".into(), json!(p.seq));
        }
        let _ = writeln!(out, "{line}");
    }
    let Some((body, _)) = below else {
        mine(event, p, err);
        return;
    };
    // Below the session: its acts and warnings, under its name; its text shows in its report.
    let body = one_line(body);
    match event {
        Event::ToolCall { name, input, .. } => {
            newline(p, err);
            let _ = writeln!(
                err,
                "  {body} ⏺ {} {}",
                one_line(name),
                summary(name, input)
            );
        }
        Event::ToolResult {
            content,
            is_error: true,
            ..
        } => {
            newline(p, err);
            let first = content.lines().next().unwrap_or("");
            let _ = writeln!(err, "  {body} ✗ {}", one_line(first));
        }
        Event::Warning { text } => {
            newline(p, err);
            let _ = writeln!(err, "  {body} ! {}", one_line(text));
        }
        _ => {}
    }
}

/// The session's own event, as stderr shows it.
fn mine(event: &Event, p: &mut Progress, err: &mut dyn Write) {
    match event {
        Event::TextDelta { text } => {
            let text = clean(text);
            if let Some(c) = text.chars().last() {
                p.last = c;
            }
            let _ = write!(err, "{text}");
        }
        Event::ToolCall { name, input, .. } => {
            newline(p, err);
            let _ = writeln!(err, "⏺ {} {}", one_line(name), summary(name, input));
        }
        Event::ToolResult {
            content,
            is_error: true,
            ..
        } => {
            newline(p, err);
            let first = content.lines().next().unwrap_or("");
            let _ = writeln!(err, "  ✗ {}", one_line(first));
        }
        Event::Warning { text } => {
            newline(p, err);
            let _ = writeln!(err, "! {}", one_line(text));
        }
        Event::GateVerdict {
            verdict,
            failures,
            unverified,
            passed,
            changed,
        } => {
            newline(p, err);
            let said: Vec<String> = failures
                .iter()
                .chain(unverified)
                .map(|f| one_line(f))
                .collect();
            if said.is_empty() {
                let _ = writeln!(err, "◆ end gate {}", one_line(verdict));
            } else {
                let _ = writeln!(err, "◆ end gate {}: {}", one_line(verdict), said.join("; "));
            }
            p.gate = Some(json!({
                "passed": passed, "verdict": verdict, "changed": changed,
                "failures": failures, "unverified": unverified,
            }));
        }
        _ => {}
    }
}

/// End the streamed text's line before a line of its own.
fn newline(p: &mut Progress, err: &mut dyn Write) {
    if p.last != '\n' {
        let _ = writeln!(err);
        p.last = '\n';
    }
}

/// `mnemorph status`: the lines for an opened session — what it would ride and what it found.
#[must_use]
pub fn status(opened: &Opened) -> Vec<String> {
    let mut lines = vec![
        ("mnemorph", env!("CARGO_PKG_VERSION").to_owned()),
        ("world", opened.world.display().to_string()),
        ("model", format!("{} via {}", opened.model, opened.provider)),
        // The key's variable and the file that names it; its value is never read back.
        (
            "key",
            opened
                .key
                .clone()
                .unwrap_or_else(|| "none — the provider takes no key".into()),
        ),
        ("sandbox", opened.sandbox.clone()),
        (
            "budget",
            opened.budget.as_ref().map_or_else(
                || "no caps set".into(),
                |b| format!("left {} wt ({})", b.wt, b.cap),
            ),
        ),
    ];
    lines.extend(opened.holes.iter().map(|h| ("hole", h.clone())));
    lines
        .into_iter()
        .map(|(k, v)| format!("{k:<9}{}", one_line(&v)))
        .collect()
}
