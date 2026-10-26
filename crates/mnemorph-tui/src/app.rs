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

use crate::activity::Activity;
use crate::approval::{Approval, Decision};
use crate::gate::Card;
use crate::kit::spinner::{FRAME_MS, Spinner};
use crate::kit::textarea::{Outcome, TextArea};
use crate::kit::toast::Toast;
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
    /// A piece of the session's reasoning, as it streams; empty when its text is hidden.
    Thinking(String),
    /// A piece of a started call's input, as raw JSON.
    ToolInput {
        /// The call.
        id: String,
        /// The piece.
        json: String,
    },
    /// The session's model turn, whole, carried this reasoning in its blocks.
    Reasoned(String),
    /// The classifier read a command's class, off the UI thread.
    Classed {
        /// The call.
        id: String,
        /// Its class.
        class: String,
    },
    /// A `write` call names its path: the UI looks whether it exists, off the session's task.
    Writes {
        /// The call.
        id: String,
        /// Its path, as the model wrote it.
        path: String,
    },
    /// Whether a `write` call's path existed when the UI looked.
    Prior {
        /// The call.
        id: String,
        /// Whether its path existed.
        existed: bool,
    },
    /// The world's survey: its memory's standing, its creatures' races; `seq` is the
    /// [`Cmd::Survey`] it answers, so a reading that comes back late never overwrites a newer one.
    Surveyed {
        /// The survey asked for.
        seq: u64,
        /// What it read.
        survey: Survey,
    },
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
    /// Look whether a `write` call's path exists, off the UI thread and the session's task.
    Look {
        /// The call.
        id: String,
        /// Its path, as the model wrote it: joined to the world unless absolute.
        path: String,
    },
    /// Read the world's memory standing and roster, off the UI thread; its answer carries `seq`.
    Survey(u64),
    /// Read a `bash` call's class, off the UI thread: the classifier looks at paths on disk.
    Classify {
        /// The call.
        id: String,
        /// Its command.
        command: String,
    },
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
    /// The world's surveys asked for and applied.
    surveys: Surveys,
    /// The turn as the status keeps it.
    pub activity: Activity,
    /// The spinner's frames since the session began: the shimmer's crest.
    pub frames: usize,
    /// The toast floating at the top right, while it shows.
    pub toast: Option<Toast>,
    /// The session's id, as the header names it.
    pub session_id: String,
    /// How long the sidebar's portrait has played: it moves only while a body works and the
    /// portrait is on screen.
    pub portrait_clock: Duration,
}

/// The surveys: the next one's number, and the newest whose reading was applied.
#[derive(Debug, Clone, Copy, Default)]
struct Surveys {
    next: u64,
    applied: Option<u64>,
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
    /// The sidebar's first column, its width (none while closed) and its first row.
    pub side_x: u16,
    /// The sidebar's width.
    pub side_w: u16,
    /// The sidebar's first row.
    pub side_y: u16,
}

/// The commands typed with a slash.
pub const COMMANDS: &str = "/send <text> steer the running turn · /theme dark|light · /clear · /quit · ctrl+o sidebar · alt+↑↓ pick a card · ctrl+t expand it · a click on a sidebar title folds it";

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
            surveys: Surveys::default(),
            activity: Activity::default(),
            frames: 0,
            toast: None,
            session_id: String::new(),
            portrait_clock: Duration::ZERO,
        }
    }

    /// Whether the sidebar shows its portrait: open, wide enough, tall enough.
    #[must_use]
    pub fn portrait_shown(&self) -> bool {
        let inner = self.size.0.saturating_sub(4);
        self.sidebar.width(inner) > 0
            && usize::from(self.size.1.saturating_sub(3)) >= crate::sidebar::HEAD_FROM
    }

    /// Whether the portrait plays: a body works, and the portrait is on screen.
    #[must_use]
    pub fn portrait_plays(&self) -> bool {
        self.busy() && self.portrait_shown()
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
        if self.viewport.animating() || self.toast.is_some() {
            Some(Duration::from_millis(16))
        } else if self.portrait_plays() {
            // The portrait's own pace, 16 frames a second; the frame cap still holds.
            Some(Duration::from_millis(1000 / 16))
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

    /// The approval card's size: a modal over the main area.
    #[must_use]
    pub fn card_area(&self) -> (usize, usize) {
        // The conversation's label takes the main area's first row.
        let (w, h) = self.main_area();
        crate::approval::modal(w, h.saturating_sub(1))
    }

    /// A new survey of the world, numbered after every one before it.
    pub fn survey(&mut self) -> Cmd {
        let seq = self.surveys.next;
        self.surveys.next += 1;
        Cmd::Survey(seq)
    }

    /// Write a line of its own into the transcript.
    pub fn info(&mut self, text: impl Into<String>) {
        self.transcript.push(Entry::Info(text.into()));
    }
}

/// Apply `msg` to `m`; the commands it returns are the runtime's to run.
pub fn update(m: &mut Model, msg: Msg) -> Vec<Cmd> {
    let cmds = apply(m, msg);
    settle_actor(m);
    cmds
}

/// The portrait follows the body working now; it starts its loop from the first frame only when
/// that body really changes, never on an event of the one already shown.
fn settle_actor(m: &mut Model) {
    let (now, _) = crate::activity::actor(m);
    if m.activity.actor.as_deref() != Some(now.as_str()) {
        m.activity.actor = Some(now);
        m.portrait_clock = Duration::ZERO;
    }
}

fn apply(m: &mut Model, msg: Msg) -> Vec<Cmd> {
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
        Msg::Tick(dt) => tick(m, dt),
        Msg::Event { event, below } => session_event(m, event, below),
        Msg::Below { event, chain } => {
            fold(m, &event, &chain);
            Vec::new()
        }
        Msg::Thinking(piece) => {
            m.transcript.thinking(&piece);
            Vec::new()
        }
        Msg::ToolInput { id, json } => {
            m.transcript.tool_input(&id, &json);
            Vec::new()
        }
        Msg::Reasoned(text) => {
            m.transcript.reasoned(&text);
            Vec::new()
        }
        Msg::Classed { id, class } => {
            m.transcript.classed(&id, &class);
            Vec::new()
        }
        Msg::Recalled(n) => {
            m.sidebar.recalled = Some(n);
            Vec::new()
        }
        Msg::Writes { id, path } => vec![Cmd::Look { id, path }],
        Msg::Prior { id, existed } => {
            m.sidebar.files.prior(&id, existed);
            Vec::new()
        }
        Msg::Surveyed { seq, survey } => {
            // Readings run unordered on the runtime: only one newer than the last applied lands.
            if m.surveys.applied.is_none_or(|a| seq > a) {
                m.surveys.applied = Some(seq);
                m.sidebar.surveyed(survey);
            }
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
            if !m.busy() {
                m.activity.since = None;
            }
            m.transcript.held = held;
            // A finished line may have grounded notes or born creatures: read the world again.
            vec![m.survey()]
        }
        Msg::Crashed(why) => {
            m.outstanding = 0;
            m.activity.since = None;
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

/// Time passed: the clock, the spinner and its shimmer, the springs, the toast, the card's
/// deadline.
fn tick(m: &mut Model, dt: Duration) -> Vec<Cmd> {
    m.viewport.set_max(m.measured.get());
    m.viewport.update(ViewportMsg::Tick(dt.as_secs_f64()));
    m.transcript.clock += dt;
    if m.portrait_plays() {
        m.portrait_clock += dt;
    }
    m.spin += dt;
    while m.spin >= Duration::from_millis(FRAME_MS) {
        m.spin -= Duration::from_millis(FRAME_MS);
        m.spinner.update();
        m.frames = m.frames.wrapping_add(1);
    }
    if let Some(toast) = &mut m.toast {
        toast.tick(dt.as_secs_f64());
        if !toast.alive() {
            m.toast = None;
        }
    }
    expire(m, Instant::now())
}

/// An event of the session's, or of a body `below` it.
fn session_event(m: &mut Model, event: Event, below: Option<String>) -> Vec<Cmd> {
    let mine = below.is_none();
    let ended = mine && matches!(event, Event::TurnEnded { .. });
    if mine {
        status(m, &event);
    }
    let chain: Vec<String> = below.into_iter().collect();
    fold(m, &event, &chain);
    // An approval still open when its turn ends is a question no one asks any more.
    if ended {
        return deny_all(m, "the turn ended before the human answered");
    }
    match event {
        Event::ToolCall { id, name, input } if mine && name == "bash" => input
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(|c| Cmd::Classify {
                id,
                command: c.to_owned(),
            })
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// The session's own event, for the status: a turn that starts restarts the count, one that
/// ends lets the next restart it; the end gate's verdict floats a toast.
fn status(m: &mut Model, event: &Event) {
    match event {
        Event::TurnStarted { .. } => {
            if m.activity.since.is_none() || m.activity.ended {
                m.activity.since = Some(m.transcript.clock);
                m.activity.base_tokens = m.transcript.usage.tokens;
            }
            m.activity.ended = false;
        }
        Event::TurnEnded { .. } => {
            m.activity.ended = true;
        }
        Event::GateVerdict { verdict, .. } => {
            let t = &m.theme;
            let (word, hue) = match verdict.as_str() {
                "PASS" => ("pass", t.readable(t.races.elf)),
                "NOT_VERIFIED" => ("not verified", t.p.gold),
                _ => ("fail", t.p.alarm),
            };
            m.toast = Some(Toast::new(format!("◆ end gate {word}"), hue));
        }
        _ => {}
    }
}

/// An event into the transcript and the sidebar; `chain` names the bodies it came through.
fn fold(m: &mut Model, event: &Event, chain: &[String]) {
    m.transcript.fold(event, chain.last().map(String::as_str));
    m.sidebar.fold(event, chain);
}

/// The card fitted again to the main area: the terminal's size or the sidebar changed.
fn refit(m: &mut Model) {
    let (mw, mh) = m.card_area();
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
            if let Some(section) = crate::view::sidebar_title_at(m, e.column, e.row) {
                m.sidebar.fold_section(section);
                return;
            }
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
        cards
            .into_iter()
            .rev()
            .find(|&i| m.transcript.entries().get(i).is_some_and(Entry::foldable))
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
        let (w, h) = m.card_area();
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
    let (w, h) = m.card_area();
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
    } else {
        // The status shows from this frame on, before the first token.
        m.activity.since = Some(m.transcript.clock);
        m.activity.base_tokens = m.transcript.usage.tokens;
        m.activity.ended = false;
    }
    m.outstanding += 1;
    vec![Cmd::Ask(line)]
}

/// Draw the model.
pub fn view(m: &Model, f: &mut Frame<'_>) {
    crate::view::draw(m, f);
}
