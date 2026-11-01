//! An Orc codes its Slime's verify line: the test before the change, which the dispatch law asks
//! before an Orc commissions a Slime. Creature docs are no body's to write with the file tools —
//! a body that rewrote its own checks could cheat its trust — so the harness writes this one line,
//! for an Orc, on the doc of a Slime that reports to it, pins it ([`crate::pins`]), and journals
//! it.

use std::fs;
use std::path::Path;

use mnemorph_races::Race;

use crate::doc::parse;
use crate::log::{Entry, append};
use crate::roster::Roster;

/// The longest verify command, in characters.
pub const MAX_VERIFY: usize = 300;

/// Add `command` to `slime`'s verify lines, for `orc`: added after the doc's last header field,
/// never replacing one — a line someone else set stays — and nothing at all when the doc already
/// carries it. Returns the line.
///
/// # Errors
///
/// `orc` is no living Orc, `slime` no living Slime reporting to it, the command is empty, too long,
/// or holds a control character or a backtick, the seat or the doc is a link, or the doc cannot be
/// read, parsed or written.
pub fn code(world: &Path, orc: &str, slime: &str, command: &str) -> Result<String, String> {
    code_in(
        crate::queue::state_dir().as_deref(),
        world,
        orc,
        slime,
        command,
    )
}

/// [`code`], the trusted roster read against the creatures' witness under `state`.
///
/// # Errors
///
/// As [`code`].
pub fn code_in(
    state: Option<&Path>,
    world: &Path,
    orc: &str,
    slime: &str,
    command: &str,
) -> Result<String, String> {
    let roster = Roster::load_in(state, world);
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
    if command
        .chars()
        .any(|c| c.is_control() || matches!(c, '`' | '\u{2028}' | '\u{2029}' | '\u{85}'))
    {
        return Err("a verify line is one line, without a control character or a backtick".into());
    }
    let seat = world.join(&s.seat);
    let path = seat.join("README.md");
    let linked = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
    // Every step from the world down to the doc: a link anywhere is refused.
    let rel = Path::new(&s.seat).join("README.md");
    let mut at = world.to_path_buf();
    let through_link = rel.components().any(|c| {
        at.push(c);
        linked(&at)
    });
    if through_link || linked(&seat) || linked(&path) {
        return Err(format!(
            "{} is, or sits in, a link: its verify line is set by hand",
            s.doc()
        ));
    }
    // The doc as read now, parsed now: the line goes after the fields this text holds.
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let read = parse(&text, Path::new(&s.seat))?;
    // The pin first: the Orc's word on its own Slime, kept where no body writes. A line the doc
    // carries is pinned too — a stranger's line becomes trusted only by this Orc's coding it.
    crate::pins::record(
        world,
        crate::pins::row(crate::pins::Act::Code, slime, &[command.to_owned()], orc),
    )
    .map_err(|e| format!("the line is not pinned, so it was not written: {e}"))?;
    if read.verify.iter().any(|v| v == command) {
        return Ok(format!("- **Verify:** `{command}`"));
    }
    let line = format!("- **Verify:** `{command}`");
    let after = read.lines.values().copied().max().unwrap_or(1);
    let mut next = String::with_capacity(text.len() + line.len() + 2);
    for (n, l) in text.split_inclusive('\n').enumerate() {
        next.push_str(l);
        if n + 1 == after {
            let ending = if l.ends_with("\r\n") { "\r\n" } else { "\n" };
            if !l.ends_with('\n') {
                next.push_str(ending);
            }
            next.push_str(&line);
            next.push_str(ending);
        }
    }
    // Written new beside the doc, then renamed over it: a link put in its place is replaced, never
    // followed.
    let temp = seat.join(".README.md.verify");
    let _ = fs::remove_file(&temp);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut f| std::io::Write::write_all(&mut f, next.as_bytes()))
        .and_then(|()| fs::rename(&temp, &path))
        .map_err(|e| {
            let _ = fs::remove_file(&temp);
            e.to_string()
        })?;
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

/// A verify line for the session's own card, when it has none: without one the end gate runs no
/// check, and no claim of a solo session is ever grounded. Only a command the harness itself picks
/// from the project's manifest — never a model's words: an ontology fact is creatures' text, and a
/// suggested line is one the human may paste. Only ever suggested: the card is the human's.
///
/// A Cargo project is suggested whole, `cargo test --workspace`: every member's tests, and no name
/// read from its manifest. A compiled project's cold build can outrun the gate's limit:
/// [`missing_session_verify`] says so.
#[must_use]
pub fn suggest(world: &Path) -> Option<String> {
    detected(world).map(|d| d.command)
}

/// A suggested command, and whether the project builds before it tests: a cold build may outrun
/// the end gate's limit on each verify command.
struct Detected {
    command: String,
    compiled: bool,
}

/// The test command a project's manifest names, by the first manifest found.
fn detected(world: &Path) -> Option<Detected> {
    let has = |f: &str| world.join(f).is_file();
    let plain = |command: &str| Detected {
        command: command.to_owned(),
        compiled: false,
    };
    if has("Cargo.toml") {
        return Some(Detected {
            command: "cargo test --workspace".into(),
            compiled: true,
        });
    }
    if has("pyproject.toml") || has("setup.py") || has("pytest.ini") || has("tox.ini") {
        return Some(plain("python3 -m pytest -q"));
    }
    if has("package.json") {
        // A suggestion only: a `"test"` script named in the manifest is enough to read.
        let text = crate::regular::text(&world.join("package.json"))?;
        // npm's placeholder script always fails: no test to suggest.
        if !(text.contains("\"scripts\"") && text.contains("\"test\""))
            || text.contains("no test specified")
        {
            return None;
        }
        let runner = if has("pnpm-lock.yaml") {
            "pnpm"
        } else if has("yarn.lock") {
            "yarn"
        } else {
            "npm"
        };
        return Some(Detected {
            command: format!("{runner} test"),
            compiled: has("tsconfig.json"),
        });
    }
    if has("go.mod") {
        return Some(Detected {
            command: "go test ./...".into(),
            compiled: true,
        });
    }
    let make = crate::regular::text(&world.join("Makefile"))?;
    make.lines()
        .any(|l| l.starts_with("test:"))
        .then(|| plain("make test"))
}

/// The session's card has no verify line: what the end gate cannot then do, and the line to add —
/// with, for a compiled project, the limit its cold build may need raised.
#[must_use]
pub fn missing_session_verify(world: &Path, roster: &Roster) -> Option<String> {
    let card = roster.creatures.get("rimuru")?;
    if !card.verify.is_empty() {
        return None;
    }
    let found = detected(world);
    let advice = found.as_ref().map_or_else(
        || "add `- **Verify:** `<your test command>`` to it".to_owned(),
        |d| format!("add `- **Verify:** `{}`` to it", d.command),
    );
    let slow = if found.is_some_and(|d| d.compiled) {
        "; each verify command has `gate.verify_secs` to finish (120 s unless set, at most 600), \
         and a cold build of this project may need more: if the gate says NOT_VERIFIED, raise it \
         in .mnemorph/config.toml"
    } else {
        ""
    };
    Some(format!(
        "rimuru's card (.mnemorph/rimuru/rimuru/README.md) has no verify line: the session's end \
         gate runs no check, so no claim of a solo session is ever grounded — {advice}{slow}"
    ))
}
