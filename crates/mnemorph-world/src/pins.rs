//! The verify pins: the verify lines each creature's end gate may run, kept by the harness outside
//! the world at `$XDG_STATE_HOME/mnemorph/verify/<world>-<fnv64a>.jsonl`, chained by hash
//! ([`mnemorph_ledger::chain`]) and anchored with the world's other chained journals
//! ([`crate::chains`]).
//!
//! A creature's card lives in the world, and the world is the body's to write — and a cloned
//! repository's cards are a stranger's from the start — so a card's verify line is never trusted
//! because the card says it. The gate runs only the lines pinned here, and only while the card
//! still says the same: a card that differs, or a line no pin holds, runs nothing and fails the
//! landing closed, with the human's way on.
//!
//! Three writers, none of them a body's command:
//!
//! - the human's `mnemorphd verify pin <creature>`, asked at the terminal (or `--yes`, for a
//!   script the human runs — a benchmark harness), and the terminal app's first-run prompt: a
//!   [`Act::Pin`] row, the creature's whole set of lines;
//! - the harness, when an Orc codes a Slime's verify line with the `verify` tool
//!   ([`crate::verify::code`]): a [`Act::Code`] row that adds that one line — the Orc's word on its
//!   own Slime, through the harness, never through a file;
//! - the harness, when relief splits a creature: each young is pinned to the lines its parent's
//!   pin held ([`crate::relief`]).
//!
//! Every write is refused inside a body's command (`MNEMORPH_BODY` is set there), and the sandbox
//! keeps the state directory out of every command's reach.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mnemorph_ledger::chain::{self, Head};
use mnemorph_ledger::{Build, Journal, Timestamp};
use serde::{Deserialize, Serialize};

use crate::roster::Roster;

/// The journal's kind under the state directory, and its name among the world's chains.
pub const KIND: &str = "verify";

/// The human, as a pin's author.
pub const HUMAN: &str = "human";

/// How relief names itself as a pin's author: `relief of <parent>`. It pins a young only to the
/// lines the human pinned for its parent, so its pin is the human's word carried down.
pub const RELIEF: &str = "relief of ";

/// One pinned line and who pinned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    /// The command.
    pub line: String,
    /// Who pinned it: the human, `relief of <parent>`, or the Orc that coded it.
    pub by: String,
}

impl Pinned {
    /// Pinned by the human — directly, or carried down by relief — rather than by a body: only
    /// such a line is a test bench for the guts classes, and a pass on it is the human's check.
    #[must_use]
    pub fn human(&self) -> bool {
        self.by == HUMAN || self.by.starts_with(RELIEF)
    }

    /// Who pinned it, as a verdict says it: `the human's pin`, or `<orc>'s pin`.
    #[must_use]
    pub fn source(&self) -> String {
        if self.human() {
            "the human's pin".to_owned()
        } else {
            format!("{}'s pin", self.by)
        }
    }
}

/// Each creature's pinned lines.
pub type Pins = BTreeMap<String, Vec<Pinned>>;

/// What a row does to a creature's pinned lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Act {
    /// The whole set of lines, as given: what was pinned before is replaced.
    Pin,
    /// One more line, coded by the creature's Orc through the harness: added to the set.
    Code,
}

/// One row of the pins' witness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// When it was written.
    pub ts: Timestamp,
    /// What it does.
    pub act: Act,
    /// The creature whose gate runs the lines.
    pub creature: String,
    /// The lines.
    pub lines: Vec<String>,
    /// Who pinned them: the human, the Orc that coded the line, or `relief of <parent>`.
    pub by: String,
    /// The build that wrote the row.
    #[serde(default)]
    pub build: Build,
}

/// Where `world`'s pins are kept: `$XDG_STATE_HOME/mnemorph/verify/<world>-<fnv64a>.jsonl`, else
/// under `~/.local/state`; `None` with no home. The world is named by its real path, so every
/// spelling of it finds one witness.
#[must_use]
pub fn path_of(world: &Path) -> Option<PathBuf> {
    let real = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    crate::queue::state_file(&real, KIND)
}

/// The witness kept at `path`, chained.
#[must_use]
pub fn journal_at(path: impl Into<PathBuf>) -> Journal<Row> {
    Journal::chained(path)
}

/// Each creature's pinned lines, folded from the witness at `path`; the chain is walked first,
/// held to `anchor` when one was kept. A witness that does not walk whole is an error: a dropped
/// or forged pin is never read as none.
///
/// # Errors
///
/// The witness cannot be read, a link is broken, or a line does not parse.
pub fn fold_at(path: &Path, anchor: Option<&Head>) -> Result<Pins, String> {
    let report = chain::verify_against(path, anchor)
        .map_err(|e| format!("the verify pins cannot be read: {e}"))?;
    if !report.holds() {
        return Err(format!("the verify pins do not verify: {report}"));
    }
    let back = journal_at(path)
        .read()
        .map_err(|e| format!("the verify pins cannot be read: {e}"))?;
    if let Some(line) = back.skipped.first() {
        return Err(format!(
            "the verify pins do not verify: line {line} does not read"
        ));
    }
    Ok(fold(&back.rows))
}

/// The witness's rows folded, per creature: a pin replaces the set, a code adds to it; each line
/// keeps who first pinned it in its set.
#[must_use]
pub fn fold(rows: &[Row]) -> Pins {
    let mut out = Pins::new();
    for r in rows {
        let set = out.entry(r.creature.clone()).or_default();
        if r.act == Act::Pin {
            set.clear();
        }
        for line in &r.lines {
            let line = line.trim();
            if !line.is_empty() && !set.iter().any(|p| p.line == line) {
                set.push(Pinned {
                    line: line.to_owned(),
                    by: r.by.clone(),
                });
            }
        }
    }
    out
}

/// The harness's own processes: a pin asked under one of them is a body's, whatever its
/// environment says.
const HARNESS: [&str; 3] = ["mnemorph", "mnemorphd", "mnemorph-guard"];

/// Whether this process runs under the harness: some ancestor is `mnemorph`, `mnemorphd` or
/// `mnemorph-guard`. `MNEMORPH_BODY` is a body's to unset (`env -u`), its ancestry is not.
///
/// # Errors
///
/// The ancestry cannot be read: the caller refuses, never guesses.
pub fn under_harness() -> Result<Option<String>, String> {
    let mut pid = std::os::unix::process::parent_id();
    // A chain longer than this is no process tree.
    for _ in 0..4096 {
        if pid <= 1 {
            return Ok(None);
        }
        let (name, parent) = process(pid)?;
        if HARNESS.contains(&name.as_str()) {
            return Ok(Some(name));
        }
        pid = parent;
    }
    Err("the process ancestry does not end".to_owned())
}

/// Process `pid`'s name and parent, from `/proc/<pid>/stat`.
#[cfg(target_os = "linux")]
fn process(pid: u32) -> Result<(String, u32), String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|e| format!("process {pid}'s ancestry cannot be read: {e}"))?;
    // `<pid> (<comm>) <state> <ppid> …`: the name may hold spaces and parentheses, so it ends at
    // the last `)`.
    let (open, close) = (stat.find('('), stat.rfind(')'));
    let (Some(open), Some(close)) = (open, close) else {
        return Err(format!("process {pid}'s stat does not read"));
    };
    let name = stat[open + 1..close].to_owned();
    let parent = stat[close + 1..]
        .split_whitespace()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .ok_or_else(|| format!("process {pid}'s stat does not read"))?;
    Ok((name, parent))
}

/// Process `pid`'s name and parent, from `ps`.
#[cfg(not(target_os = "linux"))]
fn process(pid: u32) -> Result<(String, u32), String> {
    let out = std::process::Command::new("/bin/ps")
        .args(["-o", "ppid=,comm=", "-p", &pid.to_string()])
        .output()
        .map_err(|e| format!("process {pid}'s ancestry cannot be read: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let text = text.trim();
    let (parent, comm) = text
        .split_once(char::is_whitespace)
        .ok_or_else(|| format!("process {pid}'s ancestry cannot be read"))?;
    let name = Path::new(comm.trim())
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = parent
        .trim()
        .parse()
        .map_err(|_| format!("process {pid}'s ancestry cannot be read"))?;
    Ok((name, parent))
}

/// The state directory, refused when it lies inside `world`: a body writes the world, so pins
/// kept there would be the body's.
fn outside(world: &Path, at: &Path) -> Result<(), String> {
    let real = |p: &Path| {
        // The nearest part that exists, made real, and the rest as written.
        let mut head = p.to_path_buf();
        let mut tail = Vec::new();
        while !head.exists() {
            match (
                head.file_name().map(std::ffi::OsStr::to_owned),
                head.parent(),
            ) {
                (Some(n), Some(parent)) => {
                    tail.push(n);
                    head = parent.to_path_buf();
                }
                _ => break,
            }
        }
        let mut out = head.canonicalize().unwrap_or(head);
        for n in tail.into_iter().rev() {
            out.push(n);
        }
        out
    };
    if real(at).starts_with(real(world)) {
        return Err(format!(
            "the state directory ({}) lies inside the world: the verify pins are kept where no \
             body writes, so none is read or written there",
            at.display()
        ));
    }
    Ok(())
}

/// `row` appended to the witness at `path`, stamped with this binary's build. Refused inside a
/// body's command (`MNEMORPH_BODY` is set there): what a gate runs is never a body's to choose.
///
/// # Errors
///
/// A body asks, or the witness cannot be written.
pub fn record_at(path: &Path, mut row: Row) -> Result<(), String> {
    if let Some(body) = std::env::var_os("MNEMORPH_BODY") {
        return Err(format!(
            "{} runs this: a verify line is pinned by the human, or by the harness for an Orc, \
             never by a body's command",
            body.to_string_lossy()
        ));
    }

    row.build = Build::this();
    journal_at(path)
        .append(&row)
        .map_err(|e| format!("the verify pins cannot be written: {e}"))
}

/// `row` appended to `world`'s witness.
///
/// # Errors
///
/// No home, the state directory lies inside the world, a body asks, or the witness cannot be
/// written.
pub fn record(world: &Path, row: Row) -> Result<(), String> {
    let at = path_of(world)
        .ok_or("no home: the verify pins are kept under the human's state directory")?;
    outside(world, &at)?;
    record_at(&at, row)
}

/// The human's pin: [`record`], refused also when this process runs under the harness — a
/// command a session started, whatever its environment says (`under_harness`). The harness's own
/// rows (an Orc's coded line, relief's) are written in its process, by [`record`].
///
/// # Errors
///
/// As [`record`], and the ancestry is the harness's or cannot be read.
pub fn record_human(world: &Path, row: Row) -> Result<(), String> {
    if let Some(harness) = under_harness()? {
        return Err(format!(
            "this runs under {harness}: a verify line is pinned by the human, never by a command \
             a session started"
        ));
    }
    record(world, row)
}

/// A row now: `act` of `lines` on `creature`, by `by`.
#[must_use]
pub fn row(act: Act, creature: &str, lines: &[String], by: &str) -> Row {
    Row {
        ts: Timestamp::now(),
        act,
        creature: creature.to_owned(),
        lines: lines.iter().map(|l| l.trim().to_owned()).collect(),
        by: by.to_owned(),
        build: Build::default(),
    }
}

/// `world`'s pins as the state directory keeps them, held to the head anchored for them. `None`
/// with no home; an error when the witness does not walk whole.
///
/// # Errors
///
/// The witness cannot be trusted, or the state directory lies inside the world.
pub fn pinned(world: &Path) -> Result<Option<Pins>, String> {
    let Some(at) = path_of(world) else {
        return Ok(None);
    };
    outside(world, &at)?;
    let anchor = crate::chains::anchor_of(world, KIND);
    fold_at(&at, anchor.as_ref()).map(Some)
}

/// The way on, for a creature whose card the gate will not run.
#[must_use]
pub fn hint(creature: &str) -> String {
    format!("run `mnemorphd verify pin {creature}` to accept it")
}

/// The lines `creature`'s end gate may run, its card at `doc` reading `card`: the pinned lines,
/// while the card says the same. Pins held in `pins` (`None` with no home), or the error that
/// stopped them being read.
///
/// # Errors
///
/// The hole that fails the landing closed: the pins cannot be trusted or kept, the card holds a
/// line no pin holds, or it differs from the pin.
pub fn trusted_in(
    pins: &Result<Option<Pins>, String>,
    creature: &str,
    doc: &str,
    card: &[String],
) -> Result<Vec<Pinned>, String> {
    let card: BTreeSet<&str> = card
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let pins = match pins {
        Err(e) => {
            return Err(format!(
                "{e}: no verify line of {creature} runs until the witness holds again — `mnemorphd \
                 housekeep --verify` says where it breaks"
            ));
        }
        Ok(None) if card.is_empty() => return Ok(Vec::new()),
        Ok(None) => {
            return Err(format!(
                "the verify line in {doc} cannot be pinned with no home: the verify pins are kept \
                 under the human's state directory"
            ));
        }
        Ok(Some(p)) => p,
    };
    match pins.get(creature) {
        None if card.is_empty() => Ok(Vec::new()),
        None => Err(format!(
            "the verify line in {doc} is not pinned: {}",
            hint(creature)
        )),
        Some(lines) => {
            let held: BTreeSet<&str> = lines.iter().map(|p| p.line.as_str()).collect();
            if held == card {
                Ok(lines.clone())
            } else {
                Err(format!(
                    "the verify line in {doc} differs from the pinned one: {}",
                    hint(creature)
                ))
            }
        }
    }
}

/// [`trusted_in`], `world`'s pins read now.
///
/// # Errors
///
/// As [`trusted_in`].
pub fn trusted(
    world: &Path,
    creature: &str,
    doc: &str,
    card: &[String],
) -> Result<Vec<Pinned>, String> {
    trusted_in(&pinned(world), creature, doc, card)
}

/// A creature whose card the gate will not run as it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpinned {
    /// The creature.
    pub creature: String,
    /// Its card, relative to the world.
    pub doc: String,
    /// The card's verify lines.
    pub card: Vec<String>,
    /// What the pin holds, when one was made.
    pub pinned: Option<Vec<Pinned>>,
}

/// Every living creature of `roster` whose card holds verify lines the pins do not: the human's
/// list to pin. Empty when the pins cannot be read (the gate says why) or there is no home.
#[must_use]
pub fn unpinned(world: &Path, roster: &Roster) -> Vec<Unpinned> {
    let Ok(Some(pins)) = pinned(world) else {
        return Vec::new();
    };
    roster
        .creatures
        .values()
        .filter(|c| !c.retired)
        .filter(|c| {
            let ok = Ok(Some(pins.clone()));
            trusted_in(&ok, &c.name, &c.doc(), &c.verify).is_err()
        })
        .map(|c| Unpinned {
            creature: c.name.clone(),
            doc: c.doc(),
            card: c.verify.clone(),
            pinned: pins.get(&c.name).cloned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| (*s).to_owned()).collect()
    }

    fn texts(p: &[Pinned]) -> Vec<String> {
        p.iter().map(|p| p.line.clone()).collect()
    }

    fn human(l: &[&str]) -> Vec<Pinned> {
        l.iter()
            .map(|s| Pinned {
                line: (*s).to_owned(),
                by: HUMAN.to_owned(),
            })
            .collect()
    }

    /// Pins kept inside the world would be the body's: a state directory there is refused.
    #[test]
    fn a_state_directory_inside_the_world_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w");
        std::fs::create_dir_all(&world).unwrap();
        let inside = world.join(".mnemorph/tmp/state/mnemorph/verify/w.jsonl");
        assert!(
            outside(&world, &inside)
                .unwrap_err()
                .contains("inside the world")
        );
        let elsewhere = dir.path().join("state/mnemorph/verify/w.jsonl");
        assert_eq!(outside(&world, &elsewhere), Ok(()));
        // A link into the world is inside it too.
        std::os::unix::fs::symlink(&world, dir.path().join("link")).unwrap();
        assert!(outside(&world, &dir.path().join("link/state/x.jsonl")).is_err());
    }

    /// The test runner is no harness: its ancestry reads, and holds none of the harness's names.
    #[test]
    fn the_ancestry_reads() {
        assert_eq!(under_harness(), Ok(None));
    }

    /// Who pinned a line is kept: the human's (or relief's) lines are the human's check, an Orc's
    /// are its own.
    #[test]
    fn each_line_keeps_who_pinned_it() {
        let rows = vec![
            row(Act::Pin, "slime-a", &lines(&["cargo test"]), HUMAN),
            row(Act::Code, "slime-a", &lines(&["true"]), "orc-o"),
            row(Act::Pin, "slime-b", &lines(&["make"]), "relief of slime-x"),
        ];
        let f = fold(&rows);
        let a = &f["slime-a"];
        assert!(a[0].human() && !a[1].human());
        assert_eq!(a[1].source(), "orc-o's pin");
        assert!(f["slime-b"][0].human());
        assert_eq!(a[0].source(), "the human's pin");
    }

    #[test]
    fn a_pin_replaces_and_a_code_adds() {
        let rows = vec![
            row(Act::Pin, "slime-a", &lines(&["cargo test"]), HUMAN),
            row(Act::Code, "slime-a", &lines(&["make check"]), "orc-o"),
            row(Act::Code, "slime-a", &lines(&["make check"]), "orc-o"),
            row(Act::Pin, "rimuru", &lines(&["true", "false"]), HUMAN),
            row(Act::Pin, "rimuru", &lines(&["pytest -q"]), HUMAN),
        ];
        let f = fold(&rows);
        assert_eq!(texts(&f["slime-a"]), lines(&["cargo test", "make check"]));
        assert_eq!(texts(&f["rimuru"]), lines(&["pytest -q"]));
    }

    #[test]
    fn the_card_must_say_what_the_pin_holds() {
        let pins: Result<_, String> = Ok(Some(BTreeMap::from([(
            "rimuru".to_owned(),
            human(&["pytest -q"]),
        )])));
        let doc = ".mnemorph/rimuru/rimuru/README.md";
        assert_eq!(
            trusted_in(&pins, "rimuru", doc, &lines(&["pytest -q"])),
            Ok(human(&["pytest -q"]))
        );
        let e = trusted_in(&pins, "rimuru", doc, &lines(&["true"])).unwrap_err();
        assert!(e.contains("differs from the pinned one"), "{e}");
        assert!(e.contains("mnemorphd verify pin rimuru"), "{e}");
        // A line removed is a difference too: a pinned check cannot be dropped from the card.
        assert!(trusted_in(&pins, "rimuru", doc, &[]).is_err());
        let e = trusted_in(&pins, "slime-s", "s.md", &lines(&["true"])).unwrap_err();
        assert!(e.contains("is not pinned"), "{e}");
        assert_eq!(trusted_in(&pins, "slime-s", "s.md", &[]), Ok(Vec::new()));
        let broken: Result<Option<Pins>, String> =
            Err("the verify pins do not verify: broken at line 2".into());
        assert!(
            trusted_in(&broken, "rimuru", doc, &lines(&["pytest -q"]))
                .unwrap_err()
                .contains("broken at line 2")
        );
        assert!(trusted_in(&broken, "rimuru", doc, &[]).is_err());
        assert!(trusted_in(&Ok(None), "rimuru", doc, &lines(&["x"])).is_err());
        assert_eq!(trusted_in(&Ok(None), "rimuru", doc, &[]), Ok(Vec::new()));
    }
}
