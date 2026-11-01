//! `housekeep --heal`: the world repairs what is mechanical and says everything else. A seat with
//! no doc gets a retired stub — its memory stays visible, it claims nothing; a *Reports to* that
//! names no living creature is relinked to the nearest living ancestor of the race that may
//! command it. Each repair is one journal entry; nothing that needs judgment is touched, and
//! nothing is written through a link.

use std::fs;
use std::path::Path;

use mnemorph_races::Race;

use crate::log::{Entry, append};
use crate::roster::Roster;

/// One repair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// A seat with no doc got a retired stub.
    Stub {
        /// The seat, relative to the world.
        seat: String,
        /// The creature's name the stub gives it.
        name: String,
    },
    /// A *Reports to* naming no living creature was relinked.
    Relink {
        /// The creature whose doc changed.
        body: String,
        /// The name it reported to.
        from: String,
        /// The living creature it reports to now.
        to: String,
    },
}

impl Fix {
    /// The repair in words.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Fix::Stub { seat, name } => format!(
                "{seat} has no doc: a retired stub names it {name}, its memory kept, claiming nothing"
            ),
            Fix::Relink { body, from, to } => format!(
                "{body} reported to {from}, who is no living creature: it reports to {to} now"
            ),
        }
    }
}

/// What a heal found: the repairs, and what it leaves to judgment.
#[derive(Debug, Clone, Default)]
pub struct Heal {
    /// The mechanical repairs, made or to be made.
    pub fixes: Vec<Fix>,
    /// Everything else, said and left.
    pub left: Vec<String>,
}

/// Find what is broken in `world`'s roster; with `apply`, repair what is mechanical and journal
/// each repair.
///
/// # Errors
///
/// A doc that cannot be written.
pub fn heal(world: &Path, apply: bool) -> std::io::Result<Heal> {
    // The cards as they stand: a heal repairs their text, and grants nothing.
    let roster = Roster::cards(world);
    let mut found = Heal {
        fixes: Vec::new(),
        left: roster.problems.clone(),
    };
    for name in roster.loops() {
        found.left.push(format!(
            "{name} is in a loop of *Reports to*: break it by hand"
        ));
    }
    stubs(world, &roster, apply, &mut found)?;
    relinks(world, &roster, apply, &mut found)?;
    Ok(found)
}

/// Whether `path` is a link: a heal never writes through one.
fn linked(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// Every seat with no doc: a retired stub.
fn stubs(world: &Path, roster: &Roster, apply: bool, found: &mut Heal) -> std::io::Result<()> {
    for race in Race::ALL {
        let dir = world.join(".mnemorph").join(race.name());
        if linked(&dir) {
            found
                .left
                .push(format!("{} is a link: heal it by hand", dir.display()));
            continue;
        }
        let Ok(seats) = fs::read_dir(&dir) else {
            continue;
        };
        let mut seats: Vec<_> = seats.filter_map(Result::ok).map(|e| e.path()).collect();
        seats.sort();
        for seat in seats {
            let rel = seat
                .strip_prefix(world)
                .unwrap_or(&seat)
                .display()
                .to_string();
            let doc = seat.join("README.md");
            if linked(&seat) || linked(&doc) {
                found
                    .left
                    .push(format!("{rel} is, or holds, a link: heal it by hand"));
                continue;
            }
            if !seat.is_dir() || fs::symlink_metadata(&doc).is_ok() {
                continue;
            }
            let name = stub_name(race, &seat);
            // A name another doc holds — living, retired, or refused by the roster — is not free.
            let taken = roster.creatures.contains_key(&name)
                || roster
                    .problems
                    .iter()
                    .any(|p| p.contains(&format!("`{name}`")));
            if taken {
                found.left.push(format!(
                    "{rel} has no doc, and `{name}` is already another doc's: name it by hand"
                ));
                continue;
            }
            let fix = Fix::Stub {
                seat: format!("{rel}/"),
                name: name.clone(),
            };
            if apply {
                let parent = if race == Race::Rimuru {
                    String::new()
                } else {
                    "- **Reports to:** rimuru\n".to_owned()
                };
                let text = format!(
                    "# {name}\n\n- **Race:** {}\n{parent}- **Retired:** yes\n",
                    race.name()
                );
                // Created new: a link put in its place since the check is refused.
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&doc)
                    .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()))?;
                journal(world, &fix, found);
            }
            found.fixes.push(fix);
        }
    }
    Ok(())
}

/// The name a stub gives the seat at `seat` of `race`.
fn stub_name(race: Race, seat: &Path) -> String {
    if race == Race::Rimuru {
        return "rimuru".to_owned();
    }
    let short = seat
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("{}-{short}", race.name())
}

/// Every living creature whose *Reports to* names no living creature: relinked to the nearest
/// living ancestor of the race that may command it, never past the race law.
fn relinks(world: &Path, roster: &Roster, apply: bool, found: &mut Heal) -> std::io::Result<()> {
    for c in roster.living() {
        let Some(from) = c.reports_to.as_deref() else {
            continue;
        };
        if roster.creatures.get(from).is_some_and(|p| !p.retired) {
            continue;
        }
        let commands = |n: &String| {
            roster.creatures.get(n).is_some_and(|p| {
                !p.retired && *n != c.name && p.race.commands().contains(&c.race.base())
            })
        };
        let to = roster.chain(from).into_iter().find(commands).or_else(|| {
            let throne = "rimuru".to_owned();
            commands(&throne).then_some(throne)
        });
        let Some(to) = to else {
            found.left.push(format!(
                "{} reports to {from}, who is no living creature, and no living {} stands above it: relink it by hand",
                c.name,
                commander(c.race)
            ));
            continue;
        };
        let path = world.join(c.doc());
        if linked(&path) {
            found
                .left
                .push(format!("{} is a link: relink {} by hand", c.doc(), c.name));
            continue;
        }
        let fix = Fix::Relink {
            body: c.name.clone(),
            from: from.to_owned(),
            to: to.clone(),
        };
        if apply {
            let Some(&at) = c.lines.get("Reports to") else {
                continue;
            };
            let text = fs::read_to_string(&path)?;
            fs::write(&path, splice(&text, at, &format!("- **Reports to:** {to}")))?;
            journal(world, &fix, found);
        }
        found.fixes.push(fix);
    }
    Ok(())
}

/// The race that may command `race`, in words.
fn commander(race: Race) -> &'static str {
    match race.base() {
        Race::Slime => "orc",
        Race::Orc => "elf",
        _ => "rimuru",
    }
}

/// `text` with its line `at` (from 1) replaced by `with`, its own line ending kept, every other
/// byte as it was.
pub(crate) fn splice(text: &str, at: usize, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (n, line) in text.split_inclusive('\n').enumerate() {
        if n + 1 != at {
            out.push_str(line);
            continue;
        }
        out.push_str(with);
        if line.ends_with("\r\n") {
            out.push_str("\r\n");
        } else if line.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// One repair, one journal entry; an entry that cannot be written is said.
fn journal(world: &Path, fix: &Fix, found: &mut Heal) {
    let entry = Entry {
        body: "housekeep".into(),
        title: "heal".into(),
        lines: vec![("Fixed".into(), fix.line())],
    };
    if let Err(e) = append(world, &entry) {
        found.left.push(format!(
            "{}: the journal could not record it: {e}",
            fix.line()
        ));
    }
}
