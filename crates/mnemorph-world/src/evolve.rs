//! When a creature is due to evolve, and by which arm ([the book](../../../docs/02-races.md)):
//! **stressed** and its **desk full**, read over the harness's trusted journals, the creatures'
//! witness and the world. Proposed only: this module drafts and swaps nothing — a plan is staged
//! and swapped in by the human (`mnemorphd evolution swap`).
//!
//! - **Stressed:** its last [`STRESS`] gate verdicts, in the trusted gate journal, all failed. A
//!   verdict no check could run for (`unrun`, `unpinned`) is the machine's: skipped. Strain and the
//!   doom checkpoint are journaled only inside the world, where a body writes; they count once a
//!   trusted journal carries them.
//! - **The desk:** its live personal notes — proposed or grounded, never dormant or tombstoned —
//!   more than [`DESK`]. The notes are kept inside the world, so a body could add some: the desk
//!   alone never makes a creature due, only beside the trusted stress, and only a proposal.
//! - **The arm**, in order: **split** when its zone is large ([`LARGE_FILES`] files or
//!   [`LARGE_LINES`] lines) and its trouble — the paths its failed landings changed and its notes'
//!   sources — clusters in two parts; **rebirth** when a failed verdict names its own verify lines
//!   (`traits hold`) or its doc names a path that is gone; **mutate** otherwise.
//! - **Cooling:** a creature whose swap's window is still open in the evolution ledger is not due:
//!   it evolves at most once a window (10 runs).

use std::collections::BTreeMap;
use std::path::Path;

use mnemorph_ledger::evolution::{self, Arm, Ledger};
use mnemorph_races::Race;

use crate::creatures::{self, Standing};
use crate::doc::Creature;
use crate::roster::Roster;
use crate::verdicts::VerdictRow;

/// The failed verdicts in a row that make a creature stressed.
pub const STRESS: usize = 3;
/// The notes a full desk holds more than.
pub const DESK: usize = 7;
/// A zone of this many files is large.
pub const LARGE_FILES: usize = 24;
/// A zone of this many lines is large.
pub const LARGE_LINES: usize = 1500;
/// The most of one file read to count its lines.
const LINE_CAP: u64 = 1024 * 1024;

/// A creature due to evolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    /// The creature.
    pub creature: String,
    /// Its race.
    pub race: Race,
    /// The arm the rules pick.
    pub arm: Arm,
    /// Why: its stress, its desk, the arm's reason.
    pub why: Vec<String>,
    /// For a split, the two parts of its zone its trouble clusters in — one for each young —
    /// as relief's `--around` takes them; empty for another arm.
    pub parts: Vec<String>,
}

impl Due {
    /// The relief that splits it around its two parts, for the human to run; `None` for another
    /// arm.
    #[must_use]
    pub fn relief_command(&self) -> Option<String> {
        (self.arm == Arm::Split && self.parts.len() == 2).then(|| {
            format!(
                "mnemorphd relief {} --around {},{}",
                self.creature, self.parts[0], self.parts[1]
            )
        })
    }
}

/// What the rules read for the world.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Survey {
    /// Each creature due, by name.
    pub due: Vec<Due>,
    /// Each creature stressed with a full desk whose swap's window is still open: the swap's id.
    pub cooling: Vec<(String, String)>,
}

/// `world`'s survey, from the trusted journals and the witness under the state directory `state`.
///
/// # Errors
///
/// The gate journal or the evolution ledger cannot be trusted, or there is no witness.
pub fn survey_in(world: &Path, state: &Path) -> Result<Survey, String> {
    let folded = crate::reports::fold_in(world, state).map_err(|e| e.to_string())?;
    let witnessed = creatures::witnessed_in(Some(state), world);
    if let Err(why) = &witnessed {
        return Err(why.clone());
    }
    let roster = creatures::trusted(Roster::cards(world), &witnessed);
    let ledger = ledger_in(world, state)?;
    let strain = crate::strain::read_in(state, world)?;
    let mut out = Survey::default();
    for (name, c) in &roster.creatures {
        if c.retired
            || c.race.base() == Race::Rimuru
            || !matches!(roster.standing.get(name), Some(Standing::Trusted))
        {
            continue;
        }
        let stress = strain.get(name).copied().unwrap_or_default();
        let Some(d) = due_of(world, c, folded.verdicts(), stress) else {
            continue;
        };
        match evolution::open_for(&ledger, name) {
            Some(swap) => out.cooling.push((name.clone(), swap.id.clone())),
            None => out.due.push(d),
        }
    }
    Ok(out)
}

/// [`survey_in`], under the human's state directory.
///
/// # Errors
///
/// As [`survey_in`], or there is no home.
pub fn survey(world: &Path) -> Result<Survey, String> {
    let state = crate::queue::state_dir().ok_or("no home: no state directory to read from")?;
    survey_in(world, &state)
}

/// The evolution ledger of `world` under `state`, held to the head anchored there; empty when
/// none was written.
fn ledger_in(world: &Path, state: &Path) -> Result<Ledger, String> {
    let path = crate::queue::state_file_in(state, world, "evolution");
    let anchor = crate::chains::anchor_under(state, world, "evolution");
    let ledger = evolution::read(&path, anchor.as_ref())
        .map_err(|e| format!("the evolution ledger cannot be read: {e}"))?;
    if let Some((line, why)) = &ledger.chain.broken {
        return Err(format!(
            "the evolution ledger's chain is broken at line {line}: {why}"
        ));
    }
    Ok(ledger)
}

/// Whether `c` is due, and by which arm, over the trusted `verdicts`.
fn due_of(
    world: &Path,
    c: &Creature,
    verdicts: &[VerdictRow],
    stress: crate::strain::Stress,
) -> Option<Due> {
    // Its verdicts a check ran for, newest last.
    let judged: Vec<&VerdictRow> = verdicts
        .iter()
        .filter(|v| v.body == c.name && crate::reports::failed(v).is_some())
        .collect();
    let last = &judged[judged.len().saturating_sub(STRESS)..];
    let failing =
        last.len() == STRESS && last.iter().all(|v| crate::reports::failed(v) == Some(true));
    let mut why = Vec::new();
    if failing {
        why.push(format!("its last {STRESS} gate verdicts failed"));
    }
    if stress.strain >= crate::strain::LINE {
        why.push(format!(
            "its strain {} reached the council's line",
            stress.strain
        ));
    }
    if stress.doomed {
        why.push("a run of its ended at the doom checkpoint".to_owned());
    }
    if why.is_empty() {
        return None;
    }
    let notes: Vec<Option<String>> = desk(world, &c.name).into_iter().map(|n| n.source).collect();
    if notes.len() <= DESK {
        return None;
    }
    why.push(format!(
        "its desk holds {} notes (more than {DESK})",
        notes.len()
    ));
    let failed: Vec<&VerdictRow> = judged
        .iter()
        .copied()
        .filter(|v| crate::reports::failed(v) == Some(true))
        .collect();
    let mut parts = Vec::new();
    let arm = if let Some((reason, two)) = split(world, c, &failed, &notes) {
        why.push(reason);
        parts = two.to_vec();
        Arm::Split
    } else if let Some(reason) = reborn(world, c, last) {
        why.push(reason);
        Arm::Rebirth
    } else {
        why.push(
            "no part of its zone stands out and its doc still holds: its lessons become traits"
                .into(),
        );
        Arm::Mutate
    };
    Some(Due {
        creature: c.name.clone(),
        race: c.race,
        arm,
        why,
        parts,
    })
}

/// One note on a creature's desk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeskNote {
    /// Its id.
    pub id: String,
    /// What it says.
    pub text: String,
    /// The path it is about, when it names one.
    pub source: Option<String>,
}

/// `name`'s desk: its live personal notes — proposed or grounded — in id order. Kept inside the
/// world: a body could add to it.
#[must_use]
pub fn desk(world: &Path, name: &str) -> Vec<DeskNote> {
    use mnemorph_memory::{Status, Tier, fold, read};
    let Ok(read) = read(world, &Tier::Personal(name.to_owned())) else {
        return Vec::new();
    };
    fold(&read.rows)
        .into_values()
        .filter(|n| matches!(n.status, Status::Proposed | Status::Grounded))
        .map(|n| DeskNote {
            id: n.id,
            text: n.text,
            source: n.source,
        })
        .collect()
}

/// Why `c` splits: its zone is large and its trouble clusters in two parts — each holding a
/// quarter of it, the two three quarters, of at least four paths; `None` otherwise.
fn split(
    world: &Path,
    c: &Creature,
    failed: &[&VerdictRow],
    notes: &[Option<String>],
) -> Option<(String, [String; 2])> {
    let (files, lines) = size(world, &c.zone);
    if files < LARGE_FILES && lines < LARGE_LINES {
        return None;
    }
    let mut parts: BTreeMap<String, usize> = BTreeMap::new();
    let trouble = failed
        .iter()
        .flat_map(|v| v.changed.iter().map(String::as_str))
        .chain(notes.iter().filter_map(Option::as_deref));
    let mut total = 0;
    for path in trouble {
        let Some(part) = part_of(&c.zone, path) else {
            continue;
        };
        total += 1;
        *parts.entry(part).or_default() += 1;
    }
    let mut ranked: Vec<(String, usize)> = parts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let [(a, na), (b, nb), ..] = ranked.as_slice() else {
        return None;
    };
    let clusters = total >= 4 && na * 4 >= total && nb * 4 >= total && (na + nb) * 4 >= total * 3;
    clusters.then(|| {
        (
            format!(
                "its zone is large ({files} files, {lines} lines) and its trouble clusters in {a} \
                 ({na}) and {b} ({nb}) of {total}"
            ),
            [a.clone(), b.clone()],
        )
    })
}

/// The part of the zone `path` lies in: with several claims, the claim; with one, the directory
/// one level under it. `None` outside the zone, or directly in a lone claim's root.
fn part_of(zone: &[String], path: &str) -> Option<String> {
    let claim = zone.iter().find(|z| crate::doc::covers(z, path))?;
    if zone.len() > 1 {
        return Some(claim.trim_end_matches('/').to_owned());
    }
    let rest = path.strip_prefix(claim.as_str())?;
    let (dir, _) = rest.split_once('/')?;
    Some(format!("{claim}{dir}"))
}

/// The regular files under `zone` and their lines, counted until both bars are reached; links are
/// never followed, and `.git` and `.mnemorph` never walked.
fn size(world: &Path, zone: &[String]) -> (usize, usize) {
    let (mut files, mut lines) = (0, 0);
    let mut stack: Vec<std::path::PathBuf> = zone.iter().map(|z| world.join(z)).collect();
    while let Some(p) = stack.pop() {
        if files >= LARGE_FILES && lines >= LARGE_LINES {
            break;
        }
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if meta.is_dir() {
            if p.file_name()
                .is_some_and(|n| n == ".git" || n == ".mnemorph")
            {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&p) {
                stack.extend(entries.filter_map(Result::ok).map(|e| e.path()));
            }
        } else if meta.is_file() {
            files += 1;
            if let Ok(Some(bytes)) = crate::regular::read(&p, LINE_CAP) {
                lines += bytes.split(|b| *b == b'\n').count() - 1;
            }
        }
    }
    (files, lines)
}

/// Why `c` is reborn: a failed verdict of `last` names its own verify lines, or its doc names a
/// path that is gone; `None` otherwise.
fn reborn(world: &Path, c: &Creature, last: &[&VerdictRow]) -> Option<String> {
    if last
        .iter()
        .any(|v| v.failures.iter().any(|f| f.starts_with("traits hold:")))
    {
        return Some("the gate failed on its own verify lines".into());
    }
    let doc = crate::regular::text(&world.join(c.doc())).unwrap_or_default();
    let gone: Vec<String> = named_paths(&doc)
        .into_iter()
        .filter(|p| std::fs::symlink_metadata(world.join(p)).is_err())
        .collect();
    (!gone.is_empty()).then(|| {
        format!(
            "its doc names what is gone: {}",
            gone.iter()
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// The world paths `doc` names in backticks: no space, a `/` inside, no glob, no scheme, nothing
/// under `.mnemorph/` and nothing climbing out.
fn named_paths(doc: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, piece) in doc.split('`').enumerate() {
        let p = piece.trim();
        let path_like = i % 2 == 1
            && !p.is_empty()
            && !p.contains(char::is_whitespace)
            && p.trim_end_matches('/').contains('/')
            && !p.contains(['*', '?', '[', '<', '{'])
            && !p.contains("://")
            && !p.starts_with(".mnemorph")
            && !p.starts_with('/')
            && !p.split('/').any(|c| c == "..");
        if path_like && !out.iter().any(|o| o == p) {
            out.push(p.to_owned());
        }
    }
    out
}

/// The survey in lines, for `status` and `mnemorphd evolution due`: one per creature due or
/// cooling; nothing when none is.
#[must_use]
pub fn lines_of(s: &Survey) -> Vec<String> {
    let mut out: Vec<String> = s
        .due
        .iter()
        .map(|d| match d.relief_command() {
            Some(relief) => format!(
                "{} ({}): due to split — {} — a split is the human's: `{relief}` shows it, \
                 `--apply` does it",
                d.creature,
                d.race.name(),
                d.why.join("; ")
            ),
            None => format!(
                "{} ({}): due to {} — {} — proposed only: nothing is drafted or swapped",
                d.creature,
                d.race.name(),
                arm_word(d.arm),
                d.why.join("; ")
            ),
        })
        .collect();
    out.extend(s.cooling.iter().map(|(c, id)| {
        format!("{c}: cooling — swap `{id}`'s window is open; it evolves at most once a window")
    }));
    out
}

/// The arm's word.
#[must_use]
pub fn arm_word(arm: Arm) -> &'static str {
    match arm {
        Arm::Split => "split",
        Arm::Mutate => "mutate",
        Arm::Rebirth => "rebirth",
    }
}

#[cfg(test)]
mod tests {
    use super::{named_paths, part_of};

    #[test]
    fn a_doc_names_paths_in_backticks_only() {
        let doc = "keeps `src/a.rs` and `src/parser/` but not `cargo test`, `*.rs`, \
                   `https://x/y`, `.mnemorph/x/y`, `../out/x`, `/etc/passwd` or src/b.rs";
        assert_eq!(named_paths(doc), ["src/a.rs", "src/parser/"]);
    }

    #[test]
    fn a_part_is_a_claim_or_the_directory_under_a_lone_one() {
        let one = vec!["src/".to_owned()];
        assert_eq!(
            part_of(&one, "src/lexer/a.rs").as_deref(),
            Some("src/lexer")
        );
        assert_eq!(part_of(&one, "src/a.rs"), None, "in the root: no part");
        assert_eq!(part_of(&one, "lib/a.rs"), None, "outside");
        let two = vec!["src/".to_owned(), "lib/".to_owned()];
        assert_eq!(part_of(&two, "lib/x/a.rs").as_deref(), Some("lib"));
    }
}
