//! The runtime: the session hosted in-process, the terminal owned and always given back, and
//! the loop — receive a message, update, run the commands, draw when dirty — with frames
//! coalesced to at most [`FPS`] a second, however fast the model streams.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use mnemorph_loop::{Beat, Interrupt, Observer, Opened, Seen, Steer, StreamEvent};
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

/// The observer that turns what the run did into messages, as the server turns it into events: a
/// commissioned body's beat comes with the chain of bodies it came through. It also tells the
/// sidebar what recall gave the session's ask, and each `write`'s path — the UI looks at it off
/// this task ([`Cmd::Look`]): the observer runs on the session's task and never touches the disk.
#[must_use]
pub fn observer(tx: Sender<Msg>) -> Observer {
    Arc::new(move |seen: &Seen| {
        let mut chain = Vec::new();
        let mut inner = seen;
        while let Seen::From { body, seen, .. } = inner {
            chain.push(body.clone());
            inner = seen;
        }
        match inner {
            Seen::Beat(Beat::Recall { anchors, .. }) if chain.is_empty() => {
                let _ = tx.send(Msg::Recalled(anchors.len()));
            }
            Seen::Beat(Beat::Model { content, .. }) => {
                for (id, path) in content.iter().filter_map(written) {
                    let _ = tx.send(Msg::Writes { id, path });
                }
            }
            _ => {}
        }
        for event in mnemorph_server::events(inner) {
            let msg = if chain.is_empty() {
                Msg::Event { event, below: None }
            } else {
                Msg::Below {
                    event,
                    chain: chain.clone(),
                }
            };
            let _ = tx.send(msg);
        }
    })
}

/// A journaled block that is a `write` call: its id and path.
fn written(block: &serde_json::Value) -> Option<(String, String)> {
    let s = |k: &str| block.get(k).and_then(serde_json::Value::as_str);
    if s("type")? != "tool_use" || s("name")? != "write" {
        return None;
    }
    let path = block.get("input")?.get("path")?.as_str()?;
    Some((s("id")?.to_owned(), path.to_owned()))
}

/// The gate whose cards reach the loop through `tx`.
#[must_use]
pub fn gate(tx: Sender<Msg>) -> Arc<TuiGate> {
    Arc::new(TuiGate::new(tx))
}

/// The session, driven on a runtime of its own: lines go in, messages come out.
pub struct Host {
    tx: Sender<Msg>,
    world: PathBuf,
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
        let home = opened.world;
        let done = tx.clone();
        let deltas = tx.clone();
        let driver = runtime.spawn(async move {
            while let Some(line) = rx.recv().await {
                let streaming = deltas.clone();
                let mut on = move |e: StreamEvent| {
                    let event = match e {
                        StreamEvent::Text(text) => Event::TextDelta { text },
                        StreamEvent::ToolStart { id, name } => Event::ToolStart { id, name },
                    };
                    let _ = streaming.send(Msg::Event { event, below: None });
                };
                session.drive(&line, &mut on).await;
                let _ = done.send(Msg::Done { held: held(&world) });
            }
        });
        supervise(runtime.handle(), driver, tx.clone());
        Ok(Self {
            tx,
            world: home,
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
                if self.lines.send(line).is_err() {
                    let _ = self.tx.send(Msg::Crashed("the session is gone".into()));
                }
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
            Cmd::Look { id, path } => {
                let (tx, world) = (self.tx.clone(), self.world.clone());
                // A blocking thread of the runtime's: a hung mount holds it, never the session.
                self.runtime.spawn_blocking(move || {
                    let existed = world.join(&path).symlink_metadata().is_ok();
                    let _ = tx.send(Msg::Prior { id, existed });
                });
            }
            Cmd::Survey(seq) => {
                let (tx, world) = (self.tx.clone(), self.world.clone());
                self.runtime.spawn_blocking(move || {
                    let survey = crate::sidebar::survey(&world);
                    let _ = tx.send(Msg::Surveyed { seq, survey });
                });
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

/// Watch the session's task: when it ends by a panic, the UI is told, so it never waits on a
/// turn that will not finish.
pub fn supervise(rt: &tokio::runtime::Handle, task: tokio::task::JoinHandle<()>, tx: Sender<Msg>) {
    rt.spawn(async move {
        if let Err(e) = task.await
            && e.is_panic()
        {
            let p = e.into_panic();
            let why = p
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "a panic".into());
            let _ = tx.send(Msg::Crashed(format!("the session stopped: {why}")));
        }
    });
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
        enter_with(
            || {
                let original = std::panic::take_hook();
                let terminal = ratatui::try_init()?;
                // A panic on the UI thread gives the terminal back before its message prints.
                // One on another thread — the session's — is the UI's to report (`supervise`):
                // the screen stays, and its message would only scribble over it.
                let ui = std::thread::current().id();
                std::panic::set_hook(Box::new(move |info| {
                    if std::thread::current().id() == ui {
                        give_back();
                        original(info);
                    }
                }));
                Ok(Self(terminal))
            },
            Self::modes,
        )
    }

    /// The terminal's modes: pasted text whole, the mouse's wheel and clicks, keys told apart.
    fn modes() -> std::io::Result<()> {
        let mut out = std::io::stdout();
        execute!(out, term::EnableBracketedPaste, term::EnableMouseCapture)?;
        if terminal::supports_keyboard_enhancement().unwrap_or(false) {
            execute!(
                out,
                term::PushKeyboardEnhancementFlags(
                    term::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            )?;
            PUSHED.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Whether the keyboard enhancement flags are pushed: popped once, by whichever gives the
/// terminal back first — the panic hook or the screen's drop.
static PUSHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Unset the modes the UI set, into `out`: the keyboard flags popped if still pushed, the mouse
/// and bracketed paste turned off. Safe to run twice: only the pop is once-only, and turning a
/// mode off again does nothing.
///
/// # Errors
///
/// `out` cannot be written.
pub fn release(out: &mut impl std::io::Write) -> std::io::Result<()> {
    if PUSHED.swap(false, std::sync::atomic::Ordering::SeqCst) {
        ratatui::crossterm::queue!(out, term::PopKeyboardEnhancementFlags)?;
    }
    ratatui::crossterm::queue!(out, term::DisableMouseCapture, term::DisableBracketedPaste)?;
    out.flush()
}

/// The terminal given back whole: its modes, then raw mode and the alternate screen — before a
/// panic's message prints, and on the screen's drop.
fn give_back() {
    let mut out = std::io::stdout();
    let _ = release(&mut out);
    ratatui::restore();
    let _ = out.flush();
}

impl Drop for Screen {
    fn drop(&mut self) {
        give_back();
    }
}

/// Take the terminal with `take`, then set its modes with `modes`. The taken screen is owned
/// before the modes are set: when setting them fails, it is dropped — the terminal given back —
/// before the error returns.
///
/// # Errors
///
/// `take` or `modes` failed.
pub fn enter_with<S>(
    take: impl FnOnce() -> std::io::Result<S>,
    modes: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<S> {
    let screen = take()?;
    modes()?;
    Ok(screen)
}

/// Read the terminal's input on a thread of its own, as messages.
fn read_input(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        while let Ok(ev) = term::read() {
            let msg = match ev {
                TermEvent::Key(k) if k.kind != KeyEventKind::Release => {
                    Msg::KeyAt(k, Instant::now())
                }
                TermEvent::Paste(s) => Msg::Paste(s),
                TermEvent::Mouse(m) => Msg::Mouse(m),
                TermEvent::Resize(w, h) => Msg::Resize(w, h),
                _ => continue,
            };
            if tx.send(msg).is_err() {
                break;
            }
        }
    });
}

/// The signals that end the UI as a quit does. In raw mode Ctrl+C is a key, not SIGINT: a SIGINT
/// or SIGQUIT comes from outside, as SIGTERM and SIGHUP do.
#[cfg(unix)]
pub const QUIT_SIGNALS: [i32; 4] = [
    signal_hook::consts::SIGTERM,
    signal_hook::consts::SIGHUP,
    signal_hook::consts::SIGINT,
    signal_hook::consts::SIGQUIT,
];

/// The quit signals end the UI as a quit does: the gate denied, the terminal given back.
#[cfg(unix)]
fn signals(tx: Sender<Msg>) {
    let Ok(mut signals) = signal_hook::iterator::Signals::new(QUIT_SIGNALS) else {
        return;
    };
    std::thread::spawn(move || {
        if signals.forever().next().is_some() {
            let _ = tx.send(Msg::Terminate);
        }
    });
}

#[cfg(not(unix))]
fn signals(_tx: Sender<Msg>) {}

/// Run the UI over an opened session until the human leaves.
///
/// # Errors
///
/// The terminal cannot be set up or drawn to.
pub fn run(model: Model, host: &Host, tx: Sender<Msg>, rx: &Receiver<Msg>) -> std::io::Result<()> {
    // Before the screen is taken: a signal from then on finds the terminal given back.
    signals(tx.clone());
    let mut screen = Screen::enter()?;
    let mut model = model;
    if let Ok(size) = screen.0.size() {
        model.size = (size.width, size.height);
    }
    read_input(tx);
    let result = event_loop(&mut model, host, &mut screen.0, rx);
    // However the loop ended, every approval still open or waiting is denied: the human left
    // without answering it.
    for cmd in app::leave(&mut model) {
        host.run(cmd);
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
    fn a_panicking_session_is_reported_not_waited_on() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = channel();
        let task = rt.spawn(async { panic!("boom") });
        supervise(rt.handle(), task, tx);
        rt.block_on(async { tokio::time::sleep(Duration::from_millis(50)).await });
        let msg = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            matches!(&msg, Msg::Crashed(why) if why.contains("boom")),
            "{msg:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn every_quit_signal_is_handled() {
        use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
        for sig in [SIGTERM, SIGHUP, SIGINT, SIGQUIT] {
            assert!(QUIT_SIGNALS.contains(&sig), "{sig}");
        }
    }

    #[test]
    fn a_mode_that_fails_after_the_terminal_is_taken_gives_it_back() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Taken<'a>(&'a AtomicBool);
        impl Drop for Taken<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let given_back = AtomicBool::new(false);
        let r = enter_with(
            || Ok(Taken(&given_back)),
            || Err(std::io::Error::other("bracketed paste refused")),
        );
        assert!(r.is_err());
        assert!(given_back.load(Ordering::SeqCst), "the terminal was kept");
        let ok = AtomicBool::new(false);
        let screen = enter_with(|| Ok(Taken(&ok)), || Ok(())).unwrap();
        assert!(!ok.load(Ordering::SeqCst), "given back while in use");
        drop(screen);
    }

    #[test]
    fn releasing_turns_every_mode_off_and_pops_the_keys_once() {
        PUSHED.store(true, std::sync::atomic::Ordering::SeqCst);
        let mut first = Vec::new();
        release(&mut first).unwrap();
        let first = String::from_utf8(first).unwrap();
        assert!(
            first.contains("\x1b[<1u"),
            "the keyboard flags popped: {first:?}"
        );
        assert!(first.contains("\x1b[?1000l"), "the mouse off: {first:?}");
        assert!(
            first.contains("\x1b[?2004l"),
            "bracketed paste off: {first:?}"
        );
        // The hook, then the drop: the second release turns the modes off again, pops nothing.
        let mut second = Vec::new();
        release(&mut second).unwrap();
        let second = String::from_utf8(second).unwrap();
        assert!(!second.contains("\x1b[<1u"), "{second:?}");
        assert!(second.contains("\x1b[?1000l") && second.contains("\x1b[?2004l"));
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
