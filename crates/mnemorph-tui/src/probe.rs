//! The probe: what the terminal can draw, asked once at start-up — before the alternate screen,
//! within [`TIMEOUT`], never blocking the start.
//!
//! It reads the window's size in cells and pixels (`TIOCGWINSZ`), then sends four queries in
//! raw mode — XTVERSION (the terminal's name), `CSI 16 t` (a cell's size in pixels), Kitty's
//! graphics query, and the primary device attributes (DA1) last. Every terminal answers DA1, and
//! answers in order: its reply marks the end of everything the terminal chose to answer, and the
//! probe stops reading there. A terminal that has not answered by the deadline is left behind;
//! whatever half-reply it sent is flushed from the input, so it never reaches the UI as keys.
//!
//! No thread is left reading: the probe waits with `poll` on `/dev/tty` itself, on the calling
//! thread, so the UI's reader is the only one once it returns. [`kit::pixels`] decides what the
//! replies mean; `--probe` prints them.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::kit::pixels::{self, Caps, Choice, Replies};

/// The longest the probe waits for the terminal.
pub const TIMEOUT: Duration = Duration::from_millis(200);

/// The queries, DA1 last.
pub const QUERIES: &str =
    "\x1b[>0q\x1b[16t\x1b[14t\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[?1;1;0S\x1b[c";

/// A terminal to ask: send bytes, then wait up to a while for some back.
pub trait Tty {
    /// Send `bytes`.
    ///
    /// # Errors
    ///
    /// The terminal cannot be written.
    fn send(&mut self, bytes: &[u8]) -> std::io::Result<()>;

    /// Wait at most `left` for bytes; `None` when none came.
    ///
    /// # Errors
    ///
    /// The terminal cannot be read.
    fn wait(&mut self, left: Duration) -> std::io::Result<Option<Vec<u8>>>;
}

/// Ask `tty` the [`QUERIES`] and read until DA1's reply or `timeout`: the replies, and whether
/// the time ran out first.
pub fn exchange(tty: &mut impl Tty, timeout: Duration) -> (String, bool) {
    let deadline = Instant::now() + timeout;
    if tty.send(QUERIES.as_bytes()).is_err() {
        return (String::new(), true);
    }
    let mut got = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return (String::from_utf8_lossy(&got).into_owned(), true);
        }
        match tty.wait(left) {
            Ok(Some(bytes)) => {
                got.extend_from_slice(&bytes);
                // A reply in pieces is read whole: never more than a few hundred bytes.
                if got.len() > 64 * 1024 {
                    return (String::from_utf8_lossy(&got).into_owned(), true);
                }
                let text = String::from_utf8_lossy(&got);
                if pixels::attributes(&text).is_some() {
                    return (text.into_owned(), false);
                }
            }
            Ok(None) | Err(_) => return (String::from_utf8_lossy(&got).into_owned(), true),
        }
    }
}

/// How long after a probe that timed out its replies may still arrive.
pub const LATE: Duration = Duration::from_secs(2);

/// The replies of a probe that timed out, arriving late: the UI's input reads them as keys —
/// XTVERSION's `ESC P` as Alt+P, Kitty's `ESC _` as Alt+_, their `ESC \` as Alt+\, the rest as
/// plain characters, a `1` among them, the approval card's "yes". For [`LATE`] after such a
/// probe, keys shaped like a reply are dropped, and no key but Esc and Ctrl+C — the denial —
/// reaches an open card.
#[derive(Debug, Clone, Copy, Default)]
pub struct Late {
    until: Option<Instant>,
    inside: bool,
}

impl Late {
    /// The window after a probe that gave up at `at`.
    #[must_use]
    pub fn after(at: Instant) -> Self {
        Self {
            until: Some(at + LATE),
            inside: false,
        }
    }

    /// Whether late replies may still arrive at `at`.
    #[must_use]
    pub fn active(&self, at: Instant) -> bool {
        self.until.is_some_and(|u| at < u)
    }

    /// Whether `key`, arriving at `at`, is dropped: part of a late reply, or any key while a
    /// card is open (`card`) in the window.
    pub fn drops(&mut self, key: &KeyEvent, at: Instant, card: bool) -> bool {
        if !self.active(at) {
            self.inside = false;
            return false;
        }
        // Every shape below names its modifiers too: a reply arrives as plain characters (Shift
        // at most) and Alt+P, Alt+_, Alt+[ and Alt+\ — never with Control. Ctrl+C and Esc are
        // always the human's: they stop, quit and deny a card.
        if key.modifiers.contains(KeyModifiers::CONTROL) || key.code == KeyCode::Esc {
            return false;
        }
        let rest = key.modifiers.difference(KeyModifiers::SHIFT);
        let plain = rest.is_empty();
        let alt = rest == KeyModifiers::ALT;
        let reply = match key.code {
            // A string reply opens (DCS, APC) …
            KeyCode::Char('P' | 'p' | '_') if alt => {
                self.inside = true;
                true
            }
            // … and closes (ST).
            KeyCode::Char('\\') if alt => {
                self.inside = false;
                true
            }
            KeyCode::Char('[') if alt => true,
            KeyCode::Char(_) if plain && self.inside => true,
            // The shapes of `CSI ? … c`, `CSI 6 ; h ; w t`, `CSI 4 ; a ; b t` and
            // `CSI ? 1 ; 0 ; n S` read as characters.
            KeyCode::Char(c) if plain => {
                c.is_ascii_digit() || matches!(c, ';' | '?' | '[' | 'c' | 't' | 'S')
            }
            _ => false,
        };
        // No other key reaches an open card in the window: only the human's denial does.
        reply || card
    }
}

/// What the terminal can draw, as `MNEMORPH_TUI_IMAGES` (read through `env`) allows: probed on
/// the controlling terminal when the output is one, else half blocks.
#[must_use]
pub fn probe(env: &impl Fn(&str) -> Option<String>) -> Caps {
    let (choice, note) = Choice::read(env("MNEMORPH_TUI_IMAGES").as_deref());
    let tty = if choice == Choice::Off {
        None
    } else {
        unix::Terminal::open()
    };
    let mut caps = if let Some(mut tty) = tty {
        let window = tty.window();
        let (raw, timed_out) = tty.raw(|t| exchange(t, TIMEOUT));
        let mut caps = pixels::decide(Replies::parse(&raw), window, choice, env);
        caps.raw = pixels::escaped(&raw);
        caps.timed_out = timed_out;
        caps
    } else {
        let mut c = Caps::blocks(if choice == Choice::Off {
            "images off"
        } else {
            "the output is not a terminal"
        });
        c.choice = choice;
        c.glyphs = pixels::glyphs(None, env);
        c
    };
    caps.notes.extend(note);
    caps
}

/// A cell's size in pixels now, from the window's size alone (`TIOCGWINSZ`, no query): read again
/// on a resize, since a font zoom changes it. `None` when the terminal gives no pixels.
#[must_use]
pub fn cell_now() -> Option<(u16, u16)> {
    unix::Terminal::open()?.window()?.cell()
}

#[cfg(unix)]
mod unix {
    use std::os::fd::OwnedFd;
    use std::time::Duration;

    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    use rustix::fs::{Mode, OFlags, open};
    use rustix::termios::{
        OptionalActions, QueueSelector, isatty, tcflush, tcgetattr, tcgetwinsize, tcsetattr,
    };

    use super::Tty;
    use crate::kit::pixels::Window;

    /// The controlling terminal, opened for the probe.
    pub struct Terminal(OwnedFd);

    impl Terminal {
        /// `/dev/tty`, when the output is a terminal and it opens.
        pub fn open() -> Option<Self> {
            if !isatty(std::io::stdout()) {
                return None;
            }
            open(
                "/dev/tty",
                OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .ok()
            .map(Self)
        }

        /// Its window, as `TIOCGWINSZ` gives it.
        pub fn window(&self) -> Option<Window> {
            let w = tcgetwinsize(&self.0).ok()?;
            Some(Window {
                cols: w.ws_col,
                rows: w.ws_row,
                xpx: w.ws_xpixel,
                ypx: w.ws_ypixel,
            })
        }

        /// Run `f` with the terminal in raw mode, its modes given back after, and any reply
        /// still arriving when `f` gave up flushed from the input.
        pub fn raw(&mut self, f: impl FnOnce(&mut Self) -> (String, bool)) -> (String, bool) {
            let Ok(saved) = tcgetattr(&self.0) else {
                return (String::new(), true);
            };
            let mut raw = saved.clone();
            raw.make_raw();
            if tcsetattr(&self.0, OptionalActions::Now, &raw).is_err() {
                return (String::new(), true);
            }
            let out = f(self);
            if out.1 {
                let _ = tcflush(&self.0, QueueSelector::IFlush);
            }
            let _ = tcsetattr(&self.0, OptionalActions::Now, &saved);
            out
        }
    }

    impl Tty for Terminal {
        fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
            let mut rest = bytes;
            while !rest.is_empty() {
                let n = rustix::io::write(&self.0, rest)?;
                rest = &rest[n..];
            }
            Ok(())
        }

        fn wait(&mut self, left: Duration) -> std::io::Result<Option<Vec<u8>>> {
            let mut fds = [PollFd::new(&self.0, PollFlags::IN)];
            let t = Timespec {
                tv_sec: i64::try_from(left.as_secs()).unwrap_or(0),
                tv_nsec: i64::from(left.subsec_nanos()),
            };
            if poll(&mut fds, Some(&t))? == 0 {
                return Ok(None);
            }
            let mut buf = [0u8; 1024];
            let n = rustix::io::read(&self.0, &mut buf)?;
            Ok((n > 0).then(|| buf[..n].to_vec()))
        }
    }
}

#[cfg(not(unix))]
mod unix {
    use super::Tty;
    use crate::kit::pixels::Window;

    /// No terminal to probe off Unix: half blocks.
    pub struct Terminal;

    impl Terminal {
        pub fn open() -> Option<Self> {
            None
        }
        pub fn window(&self) -> Option<Window> {
            None
        }
        pub fn raw(&mut self, f: impl FnOnce(&mut Self) -> (String, bool)) -> (String, bool) {
            f(self)
        }
    }

    impl Tty for Terminal {
        fn send(&mut self, _: &[u8]) -> std::io::Result<()> {
            Ok(())
        }
        fn wait(&mut self, _: std::time::Duration) -> std::io::Result<Option<Vec<u8>>> {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::pixels::Protocol;

    /// A terminal that answers with `replies`, one piece a wait, then stays silent.
    struct Fake {
        sent: Vec<u8>,
        replies: Vec<&'static str>,
        silent_for: Duration,
    }

    impl Tty for Fake {
        fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
            self.sent.extend_from_slice(bytes);
            Ok(())
        }
        fn wait(&mut self, left: Duration) -> std::io::Result<Option<Vec<u8>>> {
            if self.replies.is_empty() {
                std::thread::sleep(left.min(self.silent_for));
                return Ok(None);
            }
            Ok(Some(self.replies.remove(0).as_bytes().to_vec()))
        }
    }

    #[test]
    fn the_replies_end_at_the_device_attributes() {
        let mut t = Fake {
            sent: Vec::new(),
            replies: vec![
                "\x1bP>|xterm.js(6.1.0-beta.304)\x1b\\",
                "\x1b[6;17;8t\x1b[?1;",
                "2;4c",
                "never read",
            ],
            silent_for: Duration::ZERO,
        };
        let (raw, timed_out) = exchange(&mut t, TIMEOUT);
        assert!(!timed_out);
        assert!(raw.ends_with("\x1b[?1;2;4c"), "{raw:?}");
        assert_eq!(t.sent, QUERIES.as_bytes());
        assert!(QUERIES.ends_with("\x1b[c"), "DA1 last");
        let caps = pixels::decide(Replies::parse(&raw), None, Choice::Auto, &|_| None);
        assert_eq!(caps.protocol, Protocol::Iterm);
    }

    #[test]
    fn a_silent_terminal_is_given_up_on_in_time() {
        let mut t = Fake {
            sent: Vec::new(),
            replies: vec!["\x1bP>|half"],
            silent_for: Duration::from_secs(10),
        };
        let t0 = Instant::now();
        let (raw, timed_out) = exchange(&mut t, Duration::from_millis(60));
        let took = t0.elapsed();
        assert!(timed_out);
        assert!(took < Duration::from_secs(2), "{took:?}");
        assert_eq!(raw, "\x1bP>|half");
        let caps = pixels::decide(Replies::parse(&raw), None, Choice::Auto, &|_| None);
        assert_eq!(
            caps.protocol,
            Protocol::Blocks,
            "half blocks after a timeout"
        );
    }

    /// The keys the UI's input reads for `reply`, as crossterm parses an escape and a character.
    fn keys(reply: &str) -> Vec<KeyEvent> {
        let mut out = Vec::new();
        let mut chars = reply.chars();
        while let Some(c) = chars.next() {
            out.push(if c == '\x1b' {
                match chars.next() {
                    Some(n) => KeyEvent::new(KeyCode::Char(n), KeyModifiers::ALT),
                    None => KeyEvent::from(KeyCode::Esc),
                }
            } else {
                KeyEvent::from(KeyCode::Char(c))
            });
        }
        out
    }

    #[test]
    fn late_replies_are_dropped_and_the_card_takes_no_key() {
        let t0 = Instant::now();
        let mut late = Late::after(t0);
        let soon = t0 + Duration::from_millis(500);
        // Kitty's reply carries a `1`: every key of it dropped, on a card or not.
        for reply in [
            "\x1b_Gi=31;OK\x1b\\",
            "\x1bP>|xterm.js(6.1.0-beta.304)\x1b\\",
            "\x1b[6;17;8t",
            "\x1b[?1;2;4c",
            "\x1b[?1;0;256S",
            "\x1b[4;1575;882t",
            "?1;2c",
            "6;17;8t",
        ] {
            for k in keys(reply) {
                assert!(late.drops(&k, soon, false), "{reply:?}: {k:?} passed");
            }
        }
        // A word typed in the window passes, but no key reaches an open card.
        let a = KeyEvent::from(KeyCode::Char('a'));
        assert!(!late.drops(&a, soon, false));
        assert!(late.drops(&a, soon, true));
        assert!(late.drops(&KeyEvent::from(KeyCode::Char('1')), soon, true));
        // After the window, everything passes: a `1` answers a card again.
        let later = t0 + LATE + Duration::from_millis(1);
        assert!(!late.drops(&KeyEvent::from(KeyCode::Char('1')), later, true));
        // Ctrl+C and Esc always pass, on a card or not: they stop, quit and deny.
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let ctrl_t = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        let esc = KeyEvent::from(KeyCode::Esc);
        for card in [false, true] {
            assert!(!late.drops(&ctrl_c, soon, card));
            assert!(!late.drops(&ctrl_t, soon, card));
            assert!(!late.drops(&esc, soon, card));
        }
        // Inside a string reply too.
        assert!(late.drops(
            &KeyEvent::new(KeyCode::Char('_'), KeyModifiers::ALT),
            soon,
            false
        ));
        assert!(!late.drops(&ctrl_c, soon, false));
        assert!(late.drops(
            &KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::ALT),
            soon,
            false
        ));
        // A plain `c` or `t` is still a reply's shape; Alt+c is not.
        assert!(late.drops(&KeyEvent::from(KeyCode::Char('c')), soon, false));
        assert!(late.drops(&KeyEvent::from(KeyCode::Char('t')), soon, false));
        assert!(!late.drops(
            &KeyEvent::new(KeyCode::Char('c'), KeyModifiers::ALT),
            soon,
            false
        ));
        // No probe timed out: nothing is ever dropped.
        let mut none = Late::default();
        for k in keys("\x1b_Gi=31;OK\x1b\\") {
            assert!(!none.drops(&k, soon, true));
        }
    }

    #[test]
    fn images_off_asks_the_terminal_nothing() {
        let caps = probe(&|k: &str| (k == "MNEMORPH_TUI_IMAGES").then(|| "off".to_owned()));
        assert_eq!(caps.protocol, Protocol::Blocks);
        assert_eq!(caps.choice, Choice::Off);
        assert_eq!(caps.raw, "");
    }
}
