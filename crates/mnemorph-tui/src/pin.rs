//! The first-run pin: before the terminal is taken, the human is shown every creature whose card
//! holds verify lines the pins do not ([`mnemorph_loop::world::pins`]) — a fresh world, a cloned
//! repository, a card edited since — and asked once to pin them all. A `y` pins exactly the lines
//! shown; anything else pins nothing, and the end gate fails closed on those creatures' landings
//! until `mnemorphd verify pin` is run. Never asked inside a body's command, nor without a
//! terminal: no silent trust either way.

use std::io::{self, Write};
use std::path::Path;

use mnemorph_loop::shown;
use mnemorph_loop::world::Roster;
use mnemorph_loop::world::pins::{self, Act, HUMAN};

/// What [`offer`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offered {
    /// Every card's lines are pinned already: nothing was asked.
    Nothing,
    /// The human pinned these creatures.
    Pinned(Vec<String>),
    /// The human said no, or could not be asked: these creatures stay unpinned.
    Declined(Vec<String>),
}

/// Show `world`'s unpinned verify lines on `out` and pin them all when `ask` says yes. `ask` is
/// put the one question and answers it; an error is a no.
///
/// # Errors
///
/// `out` could not be written.
pub fn offer(
    world: &Path,
    out: &mut dyn Write,
    ask: &mut dyn FnMut(&str) -> Result<bool, String>,
) -> io::Result<Offered> {
    let list = pins::unpinned(world, &Roster::load(world));
    if list.is_empty() {
        return Ok(Offered::Nothing);
    }
    writeln!(
        out,
        "The end gate runs only verify lines the human has pinned; a card lives in the world, \
         where a body or a cloned repository could have written it."
    )?;
    for u in &list {
        let why = if u.pinned.is_some() {
            "differs from its pin"
        } else {
            "not pinned"
        };
        writeln!(out, "  {} ({}) — {why}:", shown(&u.creature), shown(&u.doc))?;
        for line in &u.card {
            writeln!(out, "    `{}`", shown(line))?;
        }
        for p in u.pinned.iter().flatten() {
            writeln!(out, "    pinned `{}` by {}", shown(&p.line), shown(&p.by))?;
        }
        if u.card.is_empty() {
            writeln!(out, "    (no line: its gate would run no check)")?;
        }
    }
    let names: Vec<String> = list.iter().map(|u| u.creature.clone()).collect();
    let yes = std::env::var_os("MNEMORPH_BODY").is_none()
        && ask("Pin these lines, so the gate runs them on every landing?").unwrap_or(false);
    if !yes {
        writeln!(
            out,
            "Nothing pinned: their landings fail closed until `mnemorphd verify pin <creature>`."
        )?;
        return Ok(Offered::Declined(names));
    }
    let mut pinned = Vec::new();
    for u in &list {
        match pins::record_human(world, pins::row(Act::Pin, &u.creature, &u.card, HUMAN)) {
            Ok(()) => pinned.push(u.creature.clone()),
            Err(e) => writeln!(out, "  {} not pinned: {}", shown(&u.creature), shown(&e))?,
        }
    }
    if pinned.len() < names.len() {
        let rest = names.into_iter().filter(|n| !pinned.contains(n)).collect();
        return Ok(Offered::Declined(rest));
    }
    writeln!(out, "Pinned.")?;
    Ok(Offered::Pinned(pinned))
}

/// The human's `y` at the controlling terminal; with none, or piped input, no answer.
///
/// # Errors
///
/// There is no terminal, or it could not be read.
pub fn at_terminal(question: &str) -> Result<bool, String> {
    use std::io::{BufRead as _, IsTerminal as _};
    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|e| format!("no terminal: {e}"))?;
    if !tty.is_terminal() {
        return Err("never piped".to_owned());
    }
    let mut w = tty.try_clone().map_err(|e| e.to_string())?;
    write!(w, "{question} [y/N] ")
        .and_then(|()| w.flush())
        .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    io::BufReader::new(tty)
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    let a = answer.trim();
    Ok(a.eq_ignore_ascii_case("y") || a.eq_ignore_ascii_case("yes"))
}
