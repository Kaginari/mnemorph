//! The runtime: the session hosted in-process, the terminal owned and always given back, and
//! the loop — receive a message, update, run the commands, draw when dirty — with frames
//! coalesced to at most [`FPS`] a second, however fast the model streams.

use std::io::Write as _;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use mnemorph_loop::{Interrupt, Observer, Opened, Seen, Steer, StreamEvent};
use mnemorph_server::protocol::Event;
use ratatui::crossterm::event::{self as term, Event as TermEvent, KeyEventKind};
use ratatui::crossterm::{execute, terminal};

use crate::app::{self, Cmd, Header, Model, Msg};
use crate::gate::TuiGate;

/// The most frames drawn a second.
pub const FPS: u64 = 30;

/// When the next frame may be drawn: at most one every `1/FPS` seconds.
#[derive(Debug, Clone, Copy)]
pub struct FrameClock {
    last: Option<Instant>,
    every: Duration,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self {
            last: None,
            every: Duration::from_millis(1000 / FPS),
        }
    }
}

impl FrameClock {
    /// How long to wait before drawing at `now`; zero draws now.
    #[must_use]
    pub fn wait(&self, now: Instant) -> Duration {
        self.last.map_or(Duration::ZERO, |l| {
            (l + self.every).saturating_duration_since(now)
        })
    }

    /// A frame was drawn at `now`.
    pub fn drew(&mut self, now: Instant) {
        self.last = Some(now);
    }
}

/// The observer that turns what the run did into messages, as the server turns it into events:
/// a commissioned body's beat is named by the innermost body it came from.
#[must_use]
pub fn observer(tx: Sender<Msg>) -> Observer {
    Arc::new(move |seen: &Seen| {
        let mut below = None;
        let mut inner = seen;
        while let Seen::From { body, seen, .. } = inner {
            below = Some(body.clone());
            inner = seen;
        }
        for event in mnemorph_server::events(inner) {
            let _ = tx.send(Msg::Event {
                event,
                below: below.clone(),
            });
        }
    })
}

/// The gate whose cards reach the loop through `tx`.
#[must_use]
pub fn gate(tx: Sender<Msg>) -> Arc<TuiGate> {
    Arc::new(TuiGate::new(tx))
}

/// The session, driven on a runtime of its own: lines go in, messages come out.
pub struct Host {
    lines: tokio::sync::mpsc::UnboundedSender<String>,
    interrupt: Arc<Interrupt>,
    steer: Steer,
    runtime: tokio::runtime::Runtime,
}

impl Host {
    /// Drive `opened`'s session, its messages sent to `tx`.
    ///
    /// # Errors
    ///
    /// The runtime cannot start.
    pub fn start(opened: Opened, tx: Sender<Msg>) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("runtime: {e}"))?;
        let interrupt = opened.session.interrupt();
        let steer = opened.session.steer();
        let (lines, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let world = opened.world.clone();
        let mut session = opened.session;
        runtime.spawn(async move {
            while let Some(line) = rx.recv().await {
                let streaming = tx.clone();
                let mut on = move |e: StreamEvent| {
                    let event = match e {
                        StreamEvent::Text(text) => Event::TextDelta { text },
                        StreamEvent::ToolStart { id, name } => Event::ToolStart { id, name },
                    };
                    let _ = streaming.send(Msg::Event { event, below: None });
                };
                session.drive(&line, &mut on).await;
                let _ = tx.send(Msg::Done { held: held(&world) });
            }
        });
        Ok(Self {
            lines,
            interrupt,
            steer,
            runtime,
        })
    }

    /// Run a command; `false` when it is the one to leave.
    pub fn run(&self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Ask(line) => {
                let _ = self.lines.send(line);
            }
            Cmd::Steer(line) => self
                .steer
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push_back(line),
            Cmd::Interrupt => self.interrupt.fire(),
            Cmd::Answer(pending, answer) => {
                let _ = pending.answer(answer);
            }
            Cmd::Quit => return false,
        }
        true
    }

    /// Stop: the turn in flight is interrupted and given a moment to record.
    pub fn stop(self) {
        self.interrupt.fire();
        drop(self.lines);
        self.runtime.shutdown_timeout(Duration::from_secs(1));
    }
}

/// The orders held on `world`'s shared work queue.
fn held(world: &Path) -> u32 {
    use mnemorph_loop::world::queue::State;
    let open = mnemorph_loop::world::Queue::of(world).open();
    u32::try_from(open.iter().filter(|i| i.state == State::Held).count()).unwrap_or(u32::MAX)
}

/// The terminal, given back on drop — a panic included, since unwinding drops it too.
struct Screen(ratatui::DefaultTerminal);

impl Screen {
    fn enter() -> std::io::Result<Self> {
        // ratatui's init also hooks panics to restore the terminal before the message prints.
        let terminal = ratatui::try_init()?;
        execute!(std::io::stdout(), term::EnableBracketedPaste)?;
        if terminal::supports_keyboard_enhancement().unwrap_or(false) {
            execute!(
                std::io::stdout(),
                term::PushKeyboardEnhancementFlags(
                    term::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            )?;
        }
        Ok(Self(terminal))
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let mut out = std::io::stdout();
        if terminal::supports_keyboard_enhancement().unwrap_or(false) {
            let _ = execute!(out, term::PopKeyboardEnhancementFlags);
        }
        let _ = execute!(out, term::DisableBracketedPaste);
        ratatui::restore();
        let _ = out.flush();
    }
}

/// Read the terminal's input on a thread of its own, as messages.
fn read_input(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        while let Ok(ev) = term::read() {
            let msg = match ev {
                TermEvent::Key(k) if k.kind != KeyEventKind::Release => Msg::Key(k),
                TermEvent::Paste(s) => Msg::Paste(s),
                TermEvent::Resize(w, h) => Msg::Resize(w, h),
                _ => continue,
            };
            if tx.send(msg).is_err() {
                break;
            }
        }
    });
}

/// Run the UI over an opened session until the human leaves.
///
/// # Errors
///
/// The terminal cannot be set up or drawn to.
pub fn run(model: Model, host: &Host, tx: Sender<Msg>, rx: &Receiver<Msg>) -> std::io::Result<()> {
    let mut screen = Screen::enter()?;
    let mut model = model;
    if let Ok(size) = screen.0.size() {
        model.size = (size.width, size.height);
    }
    read_input(tx);
    let result = event_loop(&mut model, host, &mut screen.0, rx);
    // An approval still open is denied: the human left without answering it.
    if let Some((card, _)) = model.gate.take() {
        card.answer(mnemorph_loop::GateAnswer::Deny("the human left".into()));
    }
    drop(screen);
    result
}

fn event_loop(
    model: &mut Model,
    host: &Host,
    terminal: &mut ratatui::DefaultTerminal,
    rx: &Receiver<Msg>,
) -> std::io::Result<()> {
    let mut clock = FrameClock::default();
    let mut dirty = true;
    let mut last_tick = Instant::now();
    loop {
        let now = Instant::now();
        if dirty && clock.wait(now).is_zero() {
            terminal.draw(|f| app::view(model, f))?;
            clock.drew(now);
            dirty = false;
        }
        // Sleep until a message, the next frame owed, or the model's next tick.
        let mut wait = model
            .next_tick()
            .map(|t| (last_tick + t).saturating_duration_since(now));
        if dirty {
            let frame = clock.wait(now);
            wait = Some(wait.map_or(frame, |w| w.min(frame)));
        }
        let first = match wait {
            Some(w) => rx.recv_timeout(w),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let mut msgs = match first {
            Ok(m) => vec![m],
            Err(RecvTimeoutError::Timeout) => Vec::new(),
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        // Everything already waiting is taken before the next frame: a burst of deltas is one
        // frame.
        msgs.extend(rx.try_iter());
        if model.next_tick().is_some_and(|t| last_tick.elapsed() >= t) {
            msgs.push(Msg::Tick(last_tick.elapsed()));
        }
        for msg in msgs {
            if matches!(msg, Msg::Tick(_)) {
                last_tick = Instant::now();
            }
            dirty = true;
            for cmd in app::update(model, msg) {
                if !host.run(cmd) {
                    return Ok(());
                }
            }
        }
        if model.next_tick().is_none() {
            last_tick = Instant::now();
        }
    }
}

/// The world as the header shows it: the home directory as `~`.
#[must_use]
pub fn display_world(world: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| world.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => world.display().to_string(),
    }
}

/// The header of an opened session.
#[must_use]
pub fn header(opened: &Opened, home: Option<&Path>) -> Header {
    Header {
        body: mnemorph_server::SESSION_BODY.into(),
        model: opened.model.clone(),
        world: display_world(&opened.world, home),
    }
}

/// A message channel for the loop.
#[must_use]
pub fn channel() -> (Sender<Msg>, Receiver<Msg>) {
    mpsc::channel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_come_at_most_thirty_a_second() {
        let mut c = FrameClock::default();
        let t0 = Instant::now();
        assert!(c.wait(t0).is_zero());
        c.drew(t0);
        assert_eq!(c.wait(t0), Duration::from_millis(33));
        assert_eq!(
            c.wait(t0 + Duration::from_millis(20)),
            Duration::from_millis(13)
        );
        assert!(c.wait(t0 + Duration::from_millis(40)).is_zero());
    }

    #[test]
    fn the_home_directory_reads_as_a_tilde() {
        let home = Path::new("/home/u");
        assert_eq!(
            display_world(Path::new("/home/u/sqlglot"), Some(home)),
            "~/sqlglot"
        );
        assert_eq!(display_world(Path::new("/srv/w"), Some(home)), "/srv/w");
    }
}
