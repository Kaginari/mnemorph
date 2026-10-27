//! `mnemorph run --gate-tty`: a gated act put to the human on the controlling terminal, with the
//! approval card's protections. Everything a model chose — the act, the reasons, the paths, the
//! body's name — is escaped by the core's own rule ([`mnemorph_loop::shown`]), so no carriage
//! return or escape code can redraw what is being approved; the act is shown whole. Input typed
//! before the card was on the screen is thrown away, so only an answer typed after reading it
//! counts; only `y` or `yes` approves, and anything else, an end of input, an error or silence
//! until the deadline denies.

use std::fmt::Write as _;
use std::io::{self, BufRead as _, Write as _};
use std::sync::Arc;
use std::time::Duration;

use mnemorph_loop::{GateAnswer, GateAsk, shown};
use mnemorph_tools::Pending;

/// The terminal the card is put on.
pub trait Tty: Send {
    /// Write the card, whole, and flush it.
    ///
    /// # Errors
    ///
    /// The terminal could not be written.
    fn show(&mut self, card: &str) -> io::Result<()>;
    /// Wait until the card has reached the terminal, then throw away every key typed so far.
    ///
    /// # Errors
    ///
    /// The terminal could not be drained or flushed.
    fn settle(&mut self) -> io::Result<()>;
    /// Read one line of the human's answer; 0 at the end of input.
    ///
    /// # Errors
    ///
    /// The terminal could not be read.
    fn read_line(&mut self, line: &mut String) -> io::Result<usize>;
}

/// The card for `ask`, every part a model chose escaped.
#[must_use]
pub fn card(ask: &GateAsk, deadline: Duration) -> String {
    let body = if ask.body.is_empty() {
        "rimuru"
    } else {
        ask.body.as_str()
    };
    let mut card = format!(
        "\n── {} ({}) wants to run a {} act ──\n  {}\n",
        shown(body),
        ask.race,
        ask.class,
        shown(&ask.act)
    );
    for why in &ask.why {
        let _ = writeln!(card, "  · {}", shown(why));
    }
    for path in &ask.outside {
        let _ = writeln!(
            card,
            "  ⚠ approving lets it write {}",
            shown(&path.display().to_string())
        );
    }
    if let Some(plan) = &ask.fingerprint {
        let _ = writeln!(card, "  bound to {}", plan.short());
    }
    let _ = write!(
        card,
        "Approve? type y and Enter — anything else denies (denied in {}s): ",
        deadline.as_secs()
    );
    card
}

/// Whether a line the human typed approves: `y` or `yes`, nothing else.
#[must_use]
pub fn approves(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Put `ask` to the human on `tty` and wait for the answer until `deadline`; `pending` may be
/// answered elsewhere first (an interrupt denies it).
pub fn put(
    mut tty: impl Tty + 'static,
    ask: &GateAsk,
    deadline: Duration,
    pending: &Arc<Pending>,
) -> GateAnswer {
    if let Err(e) = tty.show(&card(ask, deadline)) {
        return GateAnswer::Deny(format!("the terminal could not be written: {e}"));
    }
    // Keys typed before the card reached the screen are no answer to it.
    if let Err(e) = tty.settle() {
        return GateAnswer::Deny(format!(
            "the terminal's typeahead could not be cleared: {e}"
        ));
    }
    let answering = Arc::clone(pending);
    // A reader still waiting after the deadline answers into a taken slot, and is ignored.
    std::thread::spawn(move || {
        let mut line = String::new();
        let answer = match tty.read_line(&mut line) {
            Ok(0) => GateAnswer::Deny("the terminal's input ended: denied".into()),
            Ok(_) if approves(&line) => GateAnswer::Approve,
            Ok(_) => GateAnswer::Deny("the human said no".into()),
            Err(e) => GateAnswer::Deny(format!("the terminal could not be read: {e}")),
        };
        let _ = answering.answer(answer);
    });
    pending.wait(deadline)
}

/// The controlling terminal, `/dev/tty`.
#[cfg(unix)]
pub struct DevTty {
    out: std::fs::File,
    input: io::BufReader<std::fs::File>,
}

#[cfg(unix)]
impl DevTty {
    /// Open the controlling terminal.
    ///
    /// # Errors
    ///
    /// There is none, or it cannot be opened.
    pub fn open() -> io::Result<Self> {
        let out = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")?;
        let input = io::BufReader::new(out.try_clone()?);
        Ok(Self { out, input })
    }
}

#[cfg(unix)]
impl Tty for DevTty {
    fn show(&mut self, card: &str) -> io::Result<()> {
        self.out.write_all(card.as_bytes())?;
        self.out.flush()
    }

    fn settle(&mut self) -> io::Result<()> {
        rustix::termios::tcdrain(&self.out)?;
        rustix::termios::tcflush(&self.out, rustix::termios::QueueSelector::IFlush)?;
        Ok(())
    }

    fn read_line(&mut self, line: &mut String) -> io::Result<usize> {
        self.input.read_line(line)
    }
}
