//! The application in the Elm architecture, as Bubble Tea has it: one [`Model`], the [`Msg`]s
//! that reach it, a pure [`update`] that changes it and returns [`Cmd`]s — work the runtime does
//! off the UI thread, whose results come back as messages — and a [`view`] that draws it.

use std::cell::Cell;
use std::time::Duration;

use mnemorph_loop::GateAnswer;
use mnemorph_server::protocol::Event;
use mnemorph_tools::gate::Pending;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;

use crate::gate::Card;
use crate::kit::spinner::{FRAME_MS, Spinner};
use crate::kit::textarea::{Outcome, TextArea};
use crate::kit::viewport::{Viewport, ViewportMsg};
use crate::theme::{Theme, Variant};
use crate::transcript::{Entry, Transcript};

/// What reaches the model.
#[derive(Debug, Clone)]
pub enum Msg {
    /// A key.
    Key(KeyEvent),
    /// Text pasted whole.
    Paste(String),
    /// The terminal's new size.
    Resize(u16, u16),
    /// Time passed: spinners turn, springs move.
    Tick(Duration),
    /// Something the session did; `below` names the commissioned body it came from.
    Event {
        /// The event.
        event: Event,
        /// The body below the session that did it, if any.
        below: Option<String>,
    },
    /// An act waits for the human.
    Gate(Card),
    /// The session finished a line the human sent; `held` orders wait on the shared queue.
    Done {
        /// The orders held.
        held: u32,
    },
}

/// Work for the runtime.
#[derive(Debug, Clone)]
pub enum Cmd {
    /// Ask the session a line: it runs now, or after the lines before it.
    Ask(String),
    /// Send a line to the running turn's next tool step.
    Steer(String),
    /// Stop the turn in progress.
    Interrupt,
    /// Answer an act waiting at the gate.
    Answer(Arc<Pending>, GateAnswer),
    /// Leave.
    Quit,
}

/// What the header names.
#[derive(Debug, Clone, Default)]
pub struct Header {
    /// The session's own body.
    pub body: String,
    /// The model.
    pub model: String,
    /// The world, `~` for the home directory.
    pub world: String,
}

/// The model.
#[derive(Debug)]
pub struct Model {
    /// The theme worn.
    pub theme: Theme,
    /// What the header names.
    pub header: Header,
    /// The conversation.
    pub transcript: Transcript,
    /// The input.
    pub input: TextArea,
    /// The transcript's scroll.
    pub viewport: Viewport,
    /// The spinner of calls in flight.
    pub spinner: Spinner,
    /// The act waiting for the human, and the choice made on it so far.
    pub gate: Option<(Card, Option<bool>)>,
    /// Lines sent and not yet finished.
    pub outstanding: usize,
    /// The terminal's size.
    pub size: (u16, u16),
    /// Time toward the spinner's next frame.
    spin: Duration,
    /// How far up the transcript goes, as the last frame measured it.
    pub(crate) measured: Cell<usize>,
}

/// The commands typed with a slash.
pub const COMMANDS: &str =
    "/send <text> steer the running turn · /theme dark|light · /clear · /quit";

impl Model {
    /// A model for a session of `header`, in `variant`, at `size`.
    #[must_use]
    pub fn new(header: Header, variant: Variant, size: (u16, u16)) -> Self {
        Self {
            theme: Theme::new(variant),
            header,
            transcript: Transcript::default(),
            input: TextArea::default(),
            viewport: Viewport::default(),
            spinner: Spinner::default(),
            gate: None,
            outstanding: 0,
            size,
            spin: Duration::ZERO,
            measured: Cell::new(usize::MAX),
        }
    }

    /// Whether a turn runs.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.outstanding > 0
    }

    /// When the model next wants a tick: soon while the scroll moves, at the spinner's pace
    /// while work runs, never while idle — an idle UI wakes for nothing.
    #[must_use]
    pub fn next_tick(&self) -> Option<Duration> {
        if self.viewport.animating() {
            Some(Duration::from_millis(16))
        } else if self.busy() || self.transcript.running() {
            Some(Duration::from_millis(FRAME_MS))
        } else {
            None
        }
    }

    /// Write a line of its own into the transcript.
    pub fn info(&mut self, text: impl Into<String>) {
        self.transcript.push(Entry::Info(text.into()));
    }
}

/// Apply `msg` to `m`; the commands it returns are the runtime's to run.
pub fn update(m: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Key(key) => keys(m, key),
        Msg::Paste(text) => {
            if m.gate.is_none() {
                m.input.insert(&text);
            }
            Vec::new()
        }
        Msg::Resize(w, h) => {
            m.size = (w, h);
            Vec::new()
        }
        Msg::Tick(dt) => {
            m.viewport.set_max(m.measured.get());
            m.viewport.update(ViewportMsg::Tick(dt.as_secs_f64()));
            m.spin += dt;
            while m.spin >= Duration::from_millis(FRAME_MS) {
                m.spin -= Duration::from_millis(FRAME_MS);
                m.spinner.update();
            }
            Vec::new()
        }
        Msg::Event { event, below } => {
            let ended = below.is_none() && matches!(event, Event::TurnEnded { .. });
            m.transcript.fold(&event, below.as_deref());
            // An approval still open when its turn ends is a question no one asks any more.
            if ended && let Some((card, _)) = m.gate.take() {
                return vec![deny(&card, "the turn ended before the human answered")];
            }
            Vec::new()
        }
        Msg::Gate(card) => {
            // Every approval starts unchosen: a key typed as it appears approves nothing.
            m.gate = Some((card, None));
            m.viewport.update(ViewportMsg::Bottom);
            Vec::new()
        }
        Msg::Done { held } => {
            m.outstanding = m.outstanding.saturating_sub(1);
            m.transcript.held = held;
            Vec::new()
        }
    }
}

fn deny(card: &Card, why: &str) -> Cmd {
    Cmd::Answer(Arc::clone(&card.pending), GateAnswer::Deny(why.into()))
}

fn keys(m: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && key.code == KeyCode::Char('c') {
        // Ctrl+C always does something: it denies an open approval and stops the turn, or quits.
        if let Some((card, _)) = m.gate.take() {
            return vec![deny(&card, "the human stopped the turn"), Cmd::Interrupt];
        }
        return vec![if m.busy() { Cmd::Interrupt } else { Cmd::Quit }];
    }
    if m.gate.is_some() {
        return gate_keys(m, key);
    }
    m.viewport.set_max(m.measured.get());
    let page = usize::from(m.size.1 / 2).max(1);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Esc => {
            m.viewport.update(ViewportMsg::Bottom);
            return if m.busy() {
                vec![Cmd::Interrupt]
            } else {
                Vec::new()
            };
        }
        KeyCode::PageUp => m.viewport.update(ViewportMsg::Up(page)),
        KeyCode::PageDown => m.viewport.update(ViewportMsg::Down(page)),
        KeyCode::Up if shift => m.viewport.update(ViewportMsg::Up(3)),
        KeyCode::Down if shift => m.viewport.update(ViewportMsg::Down(3)),
        KeyCode::Home if ctrl => m.viewport.update(ViewportMsg::Top),
        KeyCode::End if ctrl => m.viewport.update(ViewportMsg::Bottom),
        KeyCode::Char('d') if ctrl && m.input.text().is_empty() => return vec![Cmd::Quit],
        _ => {
            if let Outcome::Submit(line) = m.input.update(key) {
                return submit(m, line);
            }
        }
    }
    Vec::new()
}

/// An approval is chosen, then confirmed: `y` or `n`, then Enter; Esc denies at once.
fn gate_keys(m: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    let Some((card, choice)) = &mut m.gate else {
        return Vec::new();
    };
    match key.code {
        KeyCode::Char('y' | 'Y') => *choice = Some(true),
        KeyCode::Char('n' | 'N') => *choice = Some(false),
        KeyCode::Esc => {
            let cmd = deny(card, "the human said no");
            m.gate = None;
            return vec![cmd];
        }
        KeyCode::Enter => match choice {
            Some(true) => {
                let cmd = Cmd::Answer(Arc::clone(&card.pending), GateAnswer::Approve);
                m.gate = None;
                return vec![cmd];
            }
            Some(false) => {
                let cmd = deny(card, "the human said no");
                m.gate = None;
                return vec![cmd];
            }
            None => {}
        },
        _ => {}
    }
    Vec::new()
}

/// A line the human sent: a command, or an ask that runs now or waits its turn.
fn submit(m: &mut Model, line: String) -> Vec<Cmd> {
    m.viewport.update(ViewportMsg::Bottom);
    if let Some(rest) = line.strip_prefix('/') {
        let (name, arg) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let arg = arg.trim();
        match name {
            "quit" | "exit" => return vec![Cmd::Quit],
            "help" => m.info(COMMANDS),
            "theme" => match arg {
                "dark" => m.theme = Theme::new(Variant::Dark),
                "light" => m.theme = Theme::new(Variant::Light),
                _ => m.info("/theme dark or /theme light"),
            },
            "clear" => m.transcript = m.transcript.cleared(),
            "send" if arg.is_empty() => m.info("/send <text>: steer the running turn"),
            "send" if !m.busy() => {
                m.info("nothing runs: a line sent steers a running turn — send it as a message");
            }
            "send" => {
                m.info(format!("sent: {arg}"));
                return vec![Cmd::Steer(arg.to_owned())];
            }
            _ => m.info(format!("no command /{name}; {COMMANDS}")),
        }
        return Vec::new();
    }
    if m.busy() {
        m.transcript
            .fold(&Event::Queued { text: line.clone() }, None);
    }
    m.outstanding += 1;
    vec![Cmd::Ask(line)]
}

/// Draw the model.
pub fn view(m: &Model, f: &mut Frame<'_>) {
    crate::view::draw(m, f);
}
