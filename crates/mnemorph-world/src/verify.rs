//! An Orc codes its Slime's verify line: the test before the change, which the dispatch law asks
//! before an Orc commissions a Slime. Creature docs are no body's to write with the file tools —
//! a body that rewrote its own checks could cheat its trust — so the harness writes this one line,
//! for an Orc, on the doc of a Slime that reports to it, and journals it.

use std::fs;
use std::path::Path;

use mnemorph_races::Race;

use crate::heal::splice;
use crate::log::{Entry, append};
use crate::roster::Roster;

/// The longest verify command, in characters.
pub const MAX_VERIFY: usize = 300;

/// Code `slime`'s verify line as `command`, for `orc`: the line set if the doc has one, else
/// added after its last header field. Returns the line as written.
///
/// # Errors
///
/// `orc` is no living Orc, `slime` no living Slime reporting to it, the command is empty, too long,
/// or spans lines or holds a backtick, the doc is a link, or it cannot be written.
pub fn code(world: &Path, orc: &str, slime: &str, command: &str) -> Result<String, String> {
    let roster = Roster::load(world);
    let o = roster
        .creatures
        .get(orc)
        .filter(|c| !c.retired && c.race.base() == Race::Orc)
        .ok_or_else(|| {
            format!("{orc} is no living Orc: only an Orc codes a Slime's verify line")
        })?;
    let s = roster
        .creatures
        .get(slime)
        .filter(|c| !c.retired && c.race.base() == Race::Slime)
        .ok_or_else(|| format!("`{slime}` is no living Slime"))?;
    if s.reports_to.as_deref() != Some(o.name.as_str()) {
        return Err(format!(
            "{slime} does not report to {orc}: its own Orc codes its verify line"
        ));
    }
    let command = command.trim();
    if command.is_empty() || command.chars().count() > MAX_VERIFY {
        return Err(format!(
            "a verify line is one command of 1 to {MAX_VERIFY} characters"
        ));
    }
    if command.contains(['\n', '\r', '`']) {
        return Err("a verify line is one line, without a backtick".into());
    }
    let path = world.join(s.doc());
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!(
            "{} is a link: its verify line is set by hand",
            s.doc()
        ));
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let line = format!("- **Verify:** `{command}`");
    let next = if let Some(&at) = s.lines.get("Verify") {
        splice(&text, at, &line)
    } else {
        let after = s.lines.values().copied().max().unwrap_or(1);
        let mut out = String::with_capacity(text.len() + line.len() + 1);
        for (n, l) in text.split_inclusive('\n').enumerate() {
            out.push_str(l);
            if n + 1 == after {
                let ending = if l.ends_with("\r\n") { "\r\n" } else { "\n" };
                if !l.ends_with('\n') {
                    out.push_str(ending);
                }
                out.push_str(&line);
                out.push_str(ending);
            }
        }
        out
    };
    fs::write(&path, next).map_err(|e| e.to_string())?;
    let entry = Entry {
        body: orc.to_owned(),
        title: "verify".into(),
        lines: vec![(
            "Coded".into(),
            format!("{slime}'s verify line: `{command}`"),
        )],
    };
    if let Err(e) = append(world, &entry) {
        return Err(format!(
            "the line is written, but the journal could not record it: {e}"
        ));
    }
    Ok(line)
}
