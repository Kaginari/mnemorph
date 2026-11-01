//! The first-run adoption: before the terminal is taken, the human is shown every creature whose
//! card no row of the creatures' witness holds ([`mnemorph_loop::world::creatures`]) — a cloned
//! repository, a world from before the witness — with the race, post, ground and chain its card
//! claims, and asked once to adopt them all. Only the word `adopt` adopts; anything else adopts
//! nothing, and those creatures stay strangers — their own seat alone — until
//! `mnemorphd roster adopt`. A card that differs from its row is shown with that way on, never
//! adopted here. Never asked inside a body's command, nor without a terminal.

use std::io::{self, Write};
use std::path::Path;

use mnemorph_loop::shown;
use mnemorph_loop::world::creatures::{self, Act, HUMAN};

/// What [`offer`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offered {
    /// Every card says what its row holds: nothing was asked.
    Nothing,
    /// The human adopted these creatures.
    Adopted(Vec<String>),
    /// The human said no, or could not be asked: these creatures stay strangers.
    Declined(Vec<String>),
}

/// [`offer_in`], under the human's state directory.
///
/// # Errors
///
/// `out` could not be written.
pub fn offer(
    world: &Path,
    out: &mut dyn Write,
    ask: &mut dyn FnMut(&str) -> Result<bool, String>,
) -> io::Result<Offered> {
    offer_in(creatures::state().as_deref(), world, out, ask)
}

/// Show `world`'s strangers on `out` and adopt them all when `ask` says yes; `ask` is put the one
/// question and answers it, an error being a no. The witness is kept under `state`.
///
/// # Errors
///
/// `out` could not be written.
pub fn offer_in(
    state: Option<&Path>,
    world: &Path,
    out: &mut dyn Write,
    ask: &mut dyn FnMut(&str) -> Result<bool, String>,
) -> io::Result<Offered> {
    let all = creatures::unadopted_in(state, world);
    if all.is_empty() {
        return Ok(Offered::Nothing);
    }
    let (strangers, differ): (Vec<_>, Vec<_>) = all.into_iter().partition(|u| u.row.is_none());
    for u in &differ {
        writeln!(
            out,
            "{}'s card differs from its row: it runs at the lower of the two — {}",
            shown(&u.card.name),
            creatures::hint(&u.card.name)
        )?;
    }
    if strangers.is_empty() {
        return Ok(Offered::Nothing);
    }
    writeln!(
        out,
        "These creatures' cards are in the world, where a body or a cloned repository could have \
         written them, and no row of the human's holds them: until adopted, each is a stranger — \
         its own seat alone, no dispatch, no outward act, no MCP door."
    )?;
    for u in &strangers {
        writeln!(out, "  {}", shown(&creatures::shown(&u.card)))?;
    }
    let names: Vec<String> = strangers.iter().map(|u| u.card.name.clone()).collect();
    let yes = std::env::var_os("MNEMORPH_BODY").is_none()
        && ask(&format!(
            "Adopt these {} creature(s) with exactly the standing shown?",
            names.len()
        ))
        .unwrap_or(false);
    if !yes {
        writeln!(
            out,
            "Nothing adopted: they stay strangers until `mnemorphd roster adopt`."
        )?;
        return Ok(Offered::Declined(names));
    }
    let rows = strangers
        .iter()
        .map(|u| creatures::row_of(Act::Adopt, &u.card, HUMAN))
        .collect();
    if let Err(e) = creatures::record_human_in(state, world, rows) {
        writeln!(out, "Nothing adopted: {}", shown(&e))?;
        return Ok(Offered::Declined(names));
    }
    writeln!(out, "Adopted.")?;
    Ok(Offered::Adopted(names))
}

/// The human's deliberate yes at the controlling terminal: the word `adopt`, typed; anything
/// else — Enter included — is a no. With no terminal, or piped input, no answer.
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
    write!(
        w,
        "{question} Type `adopt` to adopt, anything else declines: "
    )
    .and_then(|()| w.flush())
    .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    io::BufReader::new(tty)
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(answer.trim() == "adopt")
}
