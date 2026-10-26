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

use crate::kit::pixels::{self, Caps, Choice, Replies};

/// The longest the probe waits for the terminal.
pub const TIMEOUT: Duration = Duration::from_millis(200);

/// The queries, DA1 last.
pub const QUERIES: &str = "\x1b[>0q\x1b[16t\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c";

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
        c
    };
    caps.notes.extend(note);
    caps
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

    #[test]
    fn images_off_asks_the_terminal_nothing() {
        let caps = probe(&|k: &str| (k == "MNEMORPH_TUI_IMAGES").then(|| "off".to_owned()));
        assert_eq!(caps.protocol, Protocol::Blocks);
        assert_eq!(caps.choice, Choice::Off);
        assert_eq!(caps.raw, "");
    }
}
