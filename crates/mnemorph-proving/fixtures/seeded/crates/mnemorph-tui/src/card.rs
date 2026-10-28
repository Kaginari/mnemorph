//! The approval card: what the human reads before saying yes.

use std::io::{self, Write};

/// An act waiting for the human's approval.
pub struct Card {
    /// The act's title, as the body named it.
    pub title: String,
    /// The command line it will run.
    pub command: String,
}

/// Draws the card: the title in bold, the command under it.
pub fn render_card(card: &Card, out: &mut impl Write) -> io::Result<()> {
    writeln!(out, "\x1b[1m{}\x1b[0m", card.title)?;
    writeln!(out, "  {}", card.command)?;
    writeln!(out, "  approve? [y/N]")
}
