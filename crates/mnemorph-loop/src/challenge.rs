//! Challenge routing at the gate: a landing that touches a Dark Elf surface opens a challenge,
//! and the throne's harness commissions the routed specialities to review it.
//!
//! - **Opening.** When the end gate passes a change, its changed paths are routed
//!   (`mnemorph_world::challenge::route`); a landing routed to any speciality opens a challenge in
//!   the harness's records, with the SHA-256 of its tree and its author.
//! - **Staffing.** Each routed speciality is reviewed by the living Dark Elf that bears its name
//!   (`dark-elf-harness`, the plain `dark-elf` for the generalist) — never the landing's author.
//!   One with no such body is `unstaffed`, said to the human, and the challenge stays pending.
//! - **The cap.** At most [`REVIEWERS`] reviewers (`gate.challenge_reviewers`) are commissioned
//!   per landing at one sweep, the specialities owning the most high-risk classes first; the rest
//!   wait `queued` for the next sweep, never dropped.
//! - **The reserve.** Each review is a slice of the session's challenge reserve
//!   ([`mnemorph_ledger::CHALLENGE_RESERVE_PCT`]); once it is spent, reviewers wait queued. What
//!   each review spent is booked against the challenge.
//! - **Findings.** A reviewer's report is read through [`mnemorph_proving::accept`]; a valid
//!   critical or high finding of its own speciality blocks the landing until it is fixed or the
//!   human waives the challenge.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mnemorph_proving::coverage::HIGH_RISK;
use mnemorph_races::{Race, Speciality};
use mnemorph_world::Roster;
use mnemorph_world::challenge::{Blocker, Challenges, Opening, Record, Staff, Status};
use sha2::{Digest as _, Sha256};

/// How many reviewers one landing's challenge commissions at one sweep, unless the configuration
/// says (`gate.challenge_reviewers`).
pub const REVIEWERS: usize = mnemorph_config::CHALLENGE_REVIEWERS;

/// The SHA-256 of a landing's tree: every written path, sorted, with its bytes as they stand now;
/// a removed path hashes as absent.
#[must_use]
pub fn tree_hash(world: &Path, written: &[String]) -> String {
    let mut paths: Vec<&String> = written.iter().collect();
    paths.sort();
    paths.dedup();
    let mut h = Sha256::new();
    for p in paths {
        h.update(p.as_bytes());
        h.update([0]);
        match std::fs::read(world.join(p)) {
            Ok(bytes) => {
                h.update([1]);
                h.update(Sha256::digest(&bytes));
            }
            Err(_) => h.update([2]),
        }
    }
    h.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// A landing id from a run id and a step: the characters a challenge id takes, the rest `-`.
#[must_use]
pub fn landing_id(run: &str, step: u32) -> String {
    let run: String = run
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .take(110)
        .collect();
    format!("{run}:{step}")
}

/// Opens the challenge of the landing `id`, by `by`, that changed `changed` in `world`: `None`
/// when no path touches a Dark Elf surface, else its status as opened.
///
/// # Errors
///
/// The records refused the opening; the reason, in words.
pub fn open(
    challenges: &Challenges,
    world: &Path,
    id: &str,
    by: &str,
    changed: &[PathBuf],
) -> Result<Option<Status>, String> {
    let written: Vec<String> = changed.iter().map(|p| p.display().to_string()).collect();
    let roster = Roster::load(world);
    let landing = mnemorph_world::challenge::route(&roster, id, &written);
    if landing.routed.is_empty() {
        return Ok(None);
    }
    let opening = Opening {
        tree: tree_hash(world, &landing.written),
        by: by.to_owned(),
    };
    challenges
        .open_at(&landing, &opening)
        .map(Some)
        .map_err(|e| e.to_string())
}

/// How many high-risk classes a speciality owns: its priority for a reviewer's seat.
fn risk(s: Speciality) -> usize {
    s.classes().iter().filter(|c| HIGH_RISK.contains(c)).count()
}

/// The body that reviews `s` on a landing by `author`: the living Dark Elf bearing its name,
/// never the author itself.
fn reviewer(roster: &Roster, s: Speciality, author: &str) -> Option<String> {
    roster
        .living()
        .find(|c| c.name == s.creature() && c.race.base() == Race::DarkElf && c.name != author)
        .map(|c| c.name.clone())
}

/// What one sweep does with a challenge's routed specialities that are not commissioned yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Commissioned now, by priority: the speciality and its body.
    pub commission: Vec<(Speciality, String)>,
    /// Past the cap: they wait for the next sweep.
    pub queued: Vec<Speciality>,
    /// No body of theirs is born in the world (or the only one is the landing's author).
    pub unstaffed: Vec<Speciality>,
}

/// The sweep's plan for `record`: each routed speciality not commissioned yet, by risk — the most
/// high-risk classes owned first, routing order among equals — staffed by its body up to `cap`,
/// the rest queued; one with no body unstaffed.
#[must_use]
pub fn plan(roster: &Roster, record: &Record, cap: usize) -> Plan {
    let commissioned = |s: Speciality| {
        record
            .staff
            .iter()
            .any(|(t, st)| *t == s && matches!(st, Staff::Commissioned { .. }))
    };
    let mut todo: Vec<Speciality> = record
        .landing
        .routed
        .iter()
        .map(|r| r.speciality)
        .filter(|s| !commissioned(*s))
        .collect();
    // Stable: routing order stands among specialities of equal risk.
    todo.sort_by_key(|s| std::cmp::Reverse(risk(*s)));
    let mut p = Plan::default();
    for s in todo {
        match reviewer(roster, s, &record.opening.by) {
            None => p.unstaffed.push(s),
            Some(body) if p.commission.len() < cap => p.commission.push((s, body)),
            Some(_) => p.queued.push(s),
        }
    }
    p
}

/// The ask a reviewer is commissioned with: the landing, its routed surfaces, its written paths,
/// and its playbook.
#[must_use]
pub fn ask(record: &Record, s: Speciality) -> String {
    let surfaces: Vec<String> = record
        .landing
        .routed(s)
        .map(|r| r.surfaces.iter().map(|x| format!("`{}`", x.at)).collect())
        .unwrap_or_default();
    let written: Vec<String> = record
        .landing
        .written
        .iter()
        .map(|p| format!("- `{p}`"))
        .collect();
    let mut text = format!(
        "Challenge the landing `{id}` by {by} (tree {tree}): you are routed as {s}{on}.\n\n\
         It wrote:\n{written}\n\n\
         Read each path as it stands and hunt your classes on it. File your coverage ledger for \
         landing `{id}` with the `coverage` tool; report each finding as an `@F` with its record.",
        id = record.landing.id,
        by = record.opening.by,
        tree = record.opening.tree.get(..12).unwrap_or("unrecorded"),
        on = if surfaces.is_empty() {
            String::new()
        } else {
            format!(" on {}", surfaces.join(", "))
        },
        written = written.join("\n"),
    );
    if let Some(book) = playbook(record, s) {
        text.push_str("\n\n");
        text.push_str(&book);
    }
    text
}

/// The playbook a reviewer reads for this landing: the hook where `mnemorph_races::brief()`
/// (`for_landing(speciality, surfaces)`, the speciality's playbooks for the routed surfaces) is
/// called once the compiled playbooks land; until then the ask carries none.
fn playbook(_record: &Record, _s: Speciality) -> Option<String> {
    None
}

/// The findings of `answer`, a reviewer's report for `s`, that block: valid on the landing's
/// tree, its own speciality's, critical or high. Seals come with verification; this is where
/// they hook in.
#[must_use]
pub fn blockers(world: &Path, s: Speciality, answer: &str) -> Vec<Blocker> {
    let Some(report) = mnemorph_wire::Report::parse(answer) else {
        return Vec::new();
    };
    let accepted = mnemorph_proving::accept(report, &mnemorph_proving::Tree::new(world));
    accepted
        .valid()
        .iter()
        .filter(|v| v.record().speciality == s.creature())
        .filter(|v| v.severity() >= mnemorph_proving::Severity::High)
        .map(|v| Blocker {
            speciality: s.creature().to_owned(),
            fingerprint: v.fingerprint().to_owned(),
            severity: v.severity().word().to_owned(),
            claim: v.record().claim.clone(),
        })
        .collect()
}

/// One line for the human on a challenge that still holds its landing, or was just settled.
#[must_use]
pub fn said(record: &Record) -> String {
    let id = &record.landing.id;
    let staffed = |want: fn(&Staff) -> bool| -> Vec<&str> {
        record
            .staff
            .iter()
            .filter(|(_, st)| want(st))
            .map(|(s, _)| s.creature())
            .collect()
    };
    let unstaffed = staffed(|st| matches!(st, Staff::Unstaffed));
    let queued = staffed(|st| matches!(st, Staff::Queued { .. }));
    let mut line = match &record.status {
        Status::Pending { awaiting } => {
            let names: Vec<&str> = awaiting.iter().map(|s| s.creature()).collect();
            format!(
                "- `{id}` by {}: challenge pending — awaiting {}",
                record.opening.by,
                names.join(", ")
            )
        }
        Status::Blocked { findings } => {
            let f: Vec<String> = findings.iter().map(ToString::to_string).collect();
            format!(
                "- `{id}` by {}: blocked — {}",
                record.opening.by,
                f.join("; ")
            )
        }
        Status::Cleared => format!("- `{id}` by {}: challenge cleared", record.opening.by),
        Status::Waived { reason } => format!("- `{id}`: waived by the human — {reason}"),
    };
    if record.status.open() {
        if !unstaffed.is_empty() {
            let _ = write!(
                line,
                "; unstaffed (no body born): {} — birth it, or waive",
                unstaffed.join(", ")
            );
        }
        if !queued.is_empty() {
            let _ = write!(line, "; queued: {}", queued.join(", "));
        }
    }
    line
}

/// The block the throne's answer carries for the challenges its sweep moved; `open` when any
/// still holds its landing.
#[must_use]
pub fn notice(lines: &[String], open: bool) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let mut text = format!("Dark Elf challenges:\n{}", lines.join("\n"));
    if open {
        text.push_str(
            "\nA landing whose challenge is pending or blocked does not count as landed until \
             every routed Dark Elf has filed a valid coverage ledger and nothing blocks; the human \
             may waive one with a reason (`mnemorphd challenge waive <landing> --why <reason>`, or \
             `/waive <landing> <reason>`).",
        );
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemorph_world::challenge::route;

    fn world(name: &str, elves: &[&str]) -> PathBuf {
        let w = std::env::temp_dir().join(format!(
            "mnemorph-loop-challenge-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&w);
        for e in elves {
            let dir = w.join(".mnemorph/dark-elf").join(e);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("README.md"),
                format!("# {e}\n\n- **Race:** dark-elf\n- **Reports to:** rimuru\n"),
            )
            .unwrap();
        }
        std::fs::create_dir_all(&w).unwrap();
        w
    }

    fn record(w: &Path, by: &str, written: &[&str]) -> Record {
        let written: Vec<String> = written.iter().map(|s| (*s).to_owned()).collect();
        Record {
            landing: route(&Roster::load(w), "l:1", &written),
            opening: Opening {
                tree: "ab".repeat(32),
                by: by.to_owned(),
            },
            staff: Vec::new(),
            status: Status::Cleared,
        }
    }

    /// Every hunter and the generalist routed, three born: the cap seats the riskiest three
    /// staffed, queues the rest, and names the unborn.
    #[test]
    fn the_cap_seats_the_riskiest_and_queues_the_rest() {
        let w = world(
            "cap",
            &[
                "dark-elf-harness",
                "dark-elf-agent",
                "dark-elf-supply",
                "dark-elf-law",
                "dark-elf",
            ],
        );
        let r = record(
            &w,
            "orc-o",
            &[
                "crates/mnemorph-tools/src/files.rs",
                "crates/mnemorph-wire/src/report.rs",
                "Cargo.lock",
                "crates/mnemorph-config/src/lib.rs",
                "crates/mnemorph-ledger/src/lib.rs",
            ],
        );
        let p = plan(&Roster::load(&w), &r, 3);
        let seated: Vec<Speciality> = p.commission.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            seated,
            [Speciality::Harness, Speciality::Supply, Speciality::Agent]
        );
        assert_eq!(p.commission[0].1, "dark-elf-harness");
        assert_eq!(p.queued, [Speciality::Law, Speciality::General]);
        assert_eq!(p.unstaffed, [Speciality::Secrets]);
        // A commissioned speciality is not planned again.
        let mut r = r;
        r.staff.push((
            Speciality::Harness,
            Staff::Commissioned {
                body: "dark-elf-harness".into(),
                by: "rimuru".into(),
                wt: 1,
            },
        ));
        let p = plan(&Roster::load(&w), &r, 3);
        assert_eq!(
            p.commission.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            [Speciality::Supply, Speciality::Agent, Speciality::Law]
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    /// A landing's author never reviews it, even when it bears the routed speciality's name.
    #[test]
    fn the_author_never_reviews_its_own_landing() {
        let w = world("author", &["dark-elf-harness"]);
        let r = record(
            &w,
            "dark-elf-harness",
            &["crates/mnemorph-tools/src/files.rs"],
        );
        let p = plan(&Roster::load(&w), &r, 3);
        assert!(p.commission.is_empty(), "{p:?}");
        assert_eq!(p.unstaffed, [Speciality::Harness]);
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn ids_hashes_asks_and_lines() {
        assert_eq!(landing_id("slime s/1", 4), "slime-s-1:4");
        let w = world("hash", &[]);
        std::fs::write(w.join("a.txt"), "one").unwrap();
        let a = tree_hash(&w, &["a.txt".into(), "gone.txt".into()]);
        assert_eq!(a.len(), 64);
        assert_eq!(a, tree_hash(&w, &["gone.txt".into(), "a.txt".into()]));
        std::fs::write(w.join("a.txt"), "two").unwrap();
        assert_ne!(a, tree_hash(&w, &["a.txt".into(), "gone.txt".into()]));
        let mut r = record(&w, "orc-o", &["Cargo.lock"]);
        let text = ask(&r, Speciality::Supply);
        assert!(text.contains("landing `l:1`") && text.contains("`**/Cargo.lock`"));
        assert!(text.contains("- `Cargo.lock`") && text.contains("coverage"));
        r.status = Status::Pending {
            awaiting: vec![Speciality::Supply],
        };
        r.staff.push((Speciality::Supply, Staff::Unstaffed));
        let line = said(&r);
        assert!(line.contains("awaiting dark-elf-supply"), "{line}");
        assert!(line.contains("unstaffed (no body born): dark-elf-supply"));
        r.staff[0].1 = Staff::Queued { why: "cap".into() };
        assert!(said(&r).contains("queued: dark-elf-supply"));
        r.status = Status::Waived {
            reason: "a revert".into(),
        };
        assert!(said(&r).contains("waived by the human — a revert"));
        r.status = Status::Cleared;
        assert!(said(&r).contains("cleared"));
        r.status = Status::Blocked {
            findings: vec![Blocker {
                speciality: "dark-elf-supply".into(),
                fingerprint: "f".into(),
                severity: "high".into(),
                claim: "c".into(),
            }],
        };
        assert!(said(&r).contains("blocked — #f high"));
        assert!(notice(&[], true).is_none());
        assert!(
            notice(&["x".into()], true)
                .unwrap()
                .contains("challenge waive")
        );
        assert!(!notice(&["x".into()], false).unwrap().contains("waive"));
        assert_eq!(blockers(&w, Speciality::Supply, "no wire here"), []);
        let _ = std::fs::remove_dir_all(&w);
    }
}
