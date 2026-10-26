//! The application in the Elm architecture, as Bubble Tea has it: one [`Model`], the [`Msg`]s
//! that reach it, a pure [`update`] that changes it and returns [`Cmd`]s — work the runtime does
//! off the UI thread, whose results come back as messages — and a [`view`] that draws it.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use mnemorph_loop::GateAnswer;
use mnemorph_server::protocol::Event;
use mnemorph_tools::gate::Pending;
use ratatui::Frame;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::sync::Arc;

use crate::approval::{Approval, Decision};
use crate::gate::Card;
use crate::kit::spinner::{FRAME_MS, Spinner};
use crate::kit::textarea::{Outcome, TextArea};
use crate::kit::viewport::{Viewport, ViewportMsg};
use crate::sidebar::{Sidebar, Survey};
use crate::theme::{Theme, Variant};
use crate::transcript::{Entry, Transcript};

/// What reaches the model.
#[derive(Debug, Clone)]
pub enum Msg {
    /// A key, pressed now.
    Key(KeyEvent),
    /// A key, stamped when the terminal read it: the input thread's keys.
    KeyAt(KeyEvent, Instant),
    /// Text pasted whole.
    Paste(String),
    /// The mouse: the wheel scrolls, a click expands or folds a card.
    Mouse(MouseEvent),
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
    /// Something a commissioned body did; `chain` names the bodies it came through, the session's
    /// own commission first.
    Below {
        /// The event.
        event: Event,
        /// The bodies, outermost first.
        chain: Vec<String>,
    },
    /// Recall gave the session's ask this many notes.
    Recalled(usize),
    /// Whether a `write` call's path existed before it ran.
    Prior {
        /// The call.
        id: String,
        /// Whether its path existed.
        existed: bool,
    },
    /// The world's survey: its memory's standing, its creatures' races.
    Surveyed(Survey),
    /// An act waits for the human.
    Gate(Card),
    /// The session finished a line the human sent; `held` orders wait on the shared queue.
    Done {
        /// The orders held.
        held: u32,
    },
    /// The session's task ended without finishing its line: it panicked or is gone.
    Crashed(String),
    /// The process was told to end (SIGTERM, SIGHUP): leave, the terminal restored.
    Terminate,
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
    /// Read the world's memory standing and roster, off the UI thread.
    Survey,
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
    /// The act on the card.
    pub gate: Option<Approval>,
    /// Acts asking after it, first come first shown.
    pub waiting: VecDeque<Card>,
    /// Lines sent and not yet finished.
    pub outstanding: usize,
    /// The terminal's size.
    pub size: (u16, u16),
    /// Time toward the spinner's next frame.
    spin: Duration,
    /// How far up the transcript goes, as the last frame measured it.
    pub(crate) measured: Cell<usize>,
    /// The sidebar.
    pub sidebar: Sidebar,
    /// Where the last frame drew the transcript, for the mouse.
    pub(crate) hits: RefCell<Hits>,
}

/// Where the last frame drew the transcript: its column span, its first row and each row's
/// entry — what a click lands on.
#[derive(Debug, Clone, Default)]
pub(crate) struct Hits {
    /// The transcript's first column.
    pub x: u16,
    /// Its width.
    pub width: u16,
    /// Its first row.
    pub y: u16,
    /// The entry on each row from `y`, `None` where none is.
    pub owners: Vec<Option<usize>>,
    /// The rows the transcript's window had.
    pub rows: usize,
}

/// The commands typed with a slash.
pub const COMMANDS: &str = "/send <text> steer the running turn · /theme dark|light · /clear · /quit · ctrl+o sidebar · alt+↑↓ pick a card · ctrl+t expand it";

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
            waiting: VecDeque::new(),
            outstanding: 0,
            size,
            spin: Duration::ZERO,
            measured: Cell::new(usize::MAX),
            sidebar: Sidebar::default(),
            hits: RefCell::new(Hits::default()),
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
        } else if self.gate.is_some() {
            // The card's countdown, and its deadline.
            Some(Duration::from_secs(1))
        } else {
            None
        }
    }

    /// The transcript column's width and height at the terminal's size.
    #[must_use]
    pub fn main_area(&self) -> (usize, usize) {
        let inner = self.size.0.saturating_sub(4);
        let w = inner - self.sidebar.width(inner);
        (usize::from(w), usize::from(self.size.1.saturating_sub(2)))
    }

    /// Write a line of its own into the transcript.
    pub fn info(&mut self, text: impl Into<String>) {
        self.transcript.push(Entry::Info(text.into()));
    }
}

/// Apply `msg` to `m`; the commands it returns are the runtime's to run.
pub fn update(m: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::Key(key) => keys(m, key, Instant::now()),
        Msg::KeyAt(key, at) => keys(m, key, at),
        Msg::Paste(text) => {
            match &mut m.gate {
                // A paste into the card is never an answer: it arms the card again.
                Some(a) => a.pasted(Instant::now()),
                None => m.input.insert(&text),
            }
            Vec::new()
        }
        Msg::Mouse(e) => {
            mouse(m, e);
            Vec::new()
        }
        Msg::Resize(w, h) => {
            m.size = (w, h);
            refit(m);
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
            expire(m, Instant::now())
        }
        Msg::Event { event, below } => {
            let ended = below.is_none() && matches!(event, Event::TurnEnded { .. });
            let chain: Vec<String> = below.into_iter().collect();
            fold(m, &event, &chain);
            // An approval still open when its turn ends is a question no one asks any more.
            if ended {
                return deny_all(m, "the turn ended before the human answered");
            }
            Vec::new()
        }
        Msg::Below { event, chain } => {
            fold(m, &event, &chain);
            Vec::new()
        }
        Msg::Recalled(n) => {
            m.sidebar.recalled = Some(n);
            Vec::new()
        }
        Msg::Prior { id, existed } => {
            m.sidebar.files.prior(&id, existed);
            Vec::new()
        }
        Msg::Surveyed(s) => {
            m.sidebar.surveyed(s);
            Vec::new()
        }
        Msg::Gate(card) => {
            // Every approval starts unchosen and unarmed: a key typed as it appears approves
            // nothing.
            m.waiting.push_back(card);
            next_gate(m);
            m.viewport.update(ViewportMsg::Bottom);
            Vec::new()
        }
        Msg::Done { held } => {
            m.outstanding = m.outstanding.saturating_sub(1);
            m.transcript.held = held;
            // A finished line may have grounded notes or born creatures: read the world again.
            vec![Cmd::Survey]
        }
        Msg::Crashed(why) => {
            m.outstanding = 0;
            m.transcript.push(Entry::Ending(format!("failed: {why}")));
            deny_all(m, "the session stopped")
        }
        Msg::Terminate => {
            let mut cmds = deny_all(m, "the terminal UI was told to end");
            cmds.push(Cmd::Quit);
            cmds
        }
    }
}

/// An event into the transcript and the sidebar; `chain` names the bodies it came through.
fn fold(m: &mut Model, event: &Event, chain: &[String]) {
    m.transcript.fold(event, chain.last().map(String::as_str));
    m.sidebar.fold(event, chain);
}

/// The card fitted again to the main area: the terminal's size or the sidebar changed.
fn refit(m: &mut Model) {
    let (mw, mh) = m.main_area();
    if let Some(a) = &mut m.gate {
        a.fit(mw, mh);
    }
}

/// The human leaves: the act on the card and every act waiting after it are denied — on any way
/// out of the loop, an error included.
pub fn leave(m: &mut Model) -> Vec<Cmd> {
    deny_all(m, "the human left")
}

/// The mouse: the wheel scrolls the transcript on its spring; a left click on a card gives it the
/// focus and expands or folds it. While an approval waits, a click answers nothing.
fn mouse(m: &mut Model, e: MouseEvent) {
    m.viewport.set_max(m.measured.get());
    match e.kind {
        MouseEventKind::ScrollUp => m.viewport.update(ViewportMsg::Up(WHEEL)),
        MouseEventKind::ScrollDown => m.viewport.update(ViewportMsg::Down(WHEEL)),
        MouseEventKind::Down(MouseButton::Left) if m.gate.is_none() => {
            let hit = {
                let h = m.hits.borrow();
                let inside = e.column >= h.x && e.column < h.x + h.width && e.row >= h.y;
                inside
                    .then(|| h.owners.get(usize::from(e.row - h.y)).copied().flatten())
                    .flatten()
            };
            if let Some(i) = hit
                && m.transcript.cards().any(|c| c == i)
            {
                m.transcript.focus(Some(i));
                m.transcript.toggle(i);
            }
        }
        _ => {}
    }
}

/// Lines one notch of the wheel scrolls.
pub const WHEEL: usize = 3;

/// The focus moves to the card before (`up`) or after the focused one — from none, the newest
/// — and the transcript scrolls to show it; past the newest, the focus leaves the cards.
fn step_focus(m: &mut Model, up: bool) {
    let now = m.transcript.focused();
    let next = if up {
        match now {
            Some(i) => m.transcript.cards().rev().find(|&c| c < i).or(Some(i)),
            None => m.transcript.cards().next_back(),
        }
    } else {
        now.and_then(|i| m.transcript.cards().find(|&c| c > i))
    };
    m.transcript.focus(next);
    match next {
        Some(i) => show(m, i),
        None => m.viewport.update(ViewportMsg::Bottom),
    }
}

/// Scroll so entry `i` is in view: its bottom at the window's foot when it was below, its top at
/// the window's head when it was above or is taller than the window.
fn show(m: &mut Model, i: usize) {
    let (w, _) = m.main_area();
    let rows = m.hits.borrow().rows.max(1);
    let ctx = crate::view::ctx(m, w);
    let (h, below) = m.transcript.place(i, &ctx);
    let at = m.viewport.target();
    let to = if h > rows || below + h > at + rows {
        (below + h).saturating_sub(rows)
    } else if below < at {
        below
    } else {
        at
    };
    m.viewport.set_max(m.measured.get());
    m.viewport.update(ViewportMsg::To(to));
}

/// Expand or fold the focused card; with none focused, the newest that has something folded,
/// which takes the focus.
fn toggle_focused(m: &mut Model) {
    let target = m.transcript.focused().or_else(|| {
        let cards: Vec<usize> = m.transcript.cards().collect();
        cards.into_iter().rev().find(|&i| {
            matches!(m.transcript.entries().get(i), Some(crate::transcript::Entry::Tool(c)) if crate::blocks::foldable(c))
        })
    });
    if let Some(i) = target {
        m.transcript.focus(Some(i));
        m.transcript.toggle(i);
        show(m, i);
    }
}

/// The card leaves at its deadline: the core has denied the act by then.
fn expire(m: &mut Model, now: Instant) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    while m.gate.as_ref().is_some_and(|a| now >= a.card.deadline)
        && let Some(a) = m.gate.take()
    {
        m.transcript
            .push(Entry::Warning("denied: no answer in time".into()));
        cmds.push(deny(&a.card, "no answer in time"));
        next_gate(m);
    }
    cmds
}

/// The next act waiting, onto the card, when the card is free.
fn next_gate(m: &mut Model) {
    if m.gate.is_none()
        && let Some(card) = m.waiting.pop_front()
    {
        // Every approval starts unchosen and unarmed: a key typed as it appears approves
        // nothing.
        let (w, h) = m.main_area();
        m.gate = Some(Approval::new(card, w, h));
    }
}

/// Deny the act on the card and every act waiting after it.
fn deny_all(m: &mut Model, why: &str) -> Vec<Cmd> {
    m.gate
        .take()
        .map(|a| a.card)
        .into_iter()
        .chain(m.waiting.drain(..))
        .map(|c| deny(&c, why))
        .collect()
}

fn deny(card: &Card, why: &str) -> Cmd {
    Cmd::Answer(Arc::clone(&card.pending), GateAnswer::Deny(why.into()))
}

fn keys(m: &mut Model, key: KeyEvent, at: Instant) -> Vec<Cmd> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && key.code == KeyCode::Char('c') {
        // Ctrl+C always does something: it denies an open approval and stops the turn, or quits.
        if m.gate.is_some() {
            let mut cmds = deny_all(m, "the human stopped the turn");
            cmds.push(Cmd::Interrupt);
            return cmds;
        }
        return vec![if m.busy() { Cmd::Interrupt } else { Cmd::Quit }];
    }
    if m.gate.is_some() {
        return gate_keys(m, key, at);
    }
    m.viewport.set_max(m.measured.get());
    let page = usize::from(m.size.1 / 2).max(1);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char('o') if ctrl => {
            m.sidebar.open = !m.sidebar.open;
            refit(m);
        }
        KeyCode::Char('t') if ctrl => toggle_focused(m),
        KeyCode::Up if alt => step_focus(m, true),
        KeyCode::Down if alt => step_focus(m, false),
        KeyCode::Esc => {
            m.transcript.focus(None);
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

/// A key while an approval waits: the card decides, or not yet.
fn gate_keys(m: &mut Model, key: KeyEvent, at: Instant) -> Vec<Cmd> {
    let (w, h) = m.main_area();
    let Some(a) = &mut m.gate else {
        return Vec::new();
    };
    match a.key(key, at, w, h) {
        Decision::Open => Vec::new(),
        Decision::Answered(answer) => {
            let cmd = Cmd::Answer(Arc::clone(&a.card.pending), answer);
            m.gate = None;
            next_gate(m);
            vec![cmd]
        }
    }
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
