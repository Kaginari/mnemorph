//! `housekeep --heal`: the world repairs what is mechanical and says everything else. A seat with
//! no doc gets a retired stub — its memory stays visible, it claims nothing; a *Reports to* that
//! names no living creature is relinked to the named creature's nearest living ancestor, or the
//! throne. Each repair is one journal entry; nothing that needs judgment is touched.

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
            Fix::Stub { seat, name } => {
                format!(
                    "{seat} has no doc: a retired stub names it {name}, its memory kept, claiming nothing"
                )
            }
            Fix::Relink { body, from, to } => {
                format!(
                    "{body} reported to {from}, who is no living creature: it reports to {to} now"
                )
            }
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
    let roster = Roster::load(world);
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

/// Every seat with no doc: a retired stub.
fn stubs(world: &Path, roster: &Roster, apply: bool, found: &mut Heal) -> std::io::Result<()> {
    for race in Race::ALL {
        let dir = world.join(".mnemorph").join(race.name());
        let Ok(seats) = fs::read_dir(&dir) else {
            continue;
        };
        let mut seats: Vec<_> = seats.filter_map(Result::ok).map(|e| e.path()).collect();
        seats.sort();
        for seat in seats
            .into_iter()
            .filter(|s| s.is_dir() && !s.join("README.md").exists())
        {
            let short = seat
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = if race == Race::Rimuru {
                "rimuru".to_owned()
            } else {
                format!("{}-{short}", race.name())
            };
            let rel = seat
                .strip_prefix(world)
                .unwrap_or(&seat)
                .display()
                .to_string();
            if roster.creatures.contains_key(&name) {
                found.left.push(format!(
                    "{rel} has no doc, and `{name}` is already another seat's: name it by hand"
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
                fs::write(
                    seat.join("README.md"),
                    format!(
                        "# {name}\n\n- **Race:** {}\n{parent}- **Retired:** yes\n",
                        race.name()
                    ),
                )?;
                journal(world, &fix);
            }
            found.fixes.push(fix);
        }
    }
    Ok(())
}

/// Every living creature whose *Reports to* names no living creature: relinked.
fn relinks(world: &Path, roster: &Roster, apply: bool, found: &mut Heal) -> std::io::Result<()> {
    for c in roster.living() {
        let Some(from) = c.reports_to.as_deref() else {
            continue;
        };
        if roster.creatures.get(from).is_some_and(|p| !p.retired) {
            continue;
        }
        // The named creature's nearest living ancestor, or the throne.
        let to = roster
            .chain(from)
            .into_iter()
            .find(|n| {
                roster
                    .creatures
                    .get(n)
                    .is_some_and(|p| !p.retired && *n != c.name)
            })
            .or_else(|| {
                roster
                    .creatures
                    .get("rimuru")
                    .filter(|r| !r.retired)
                    .map(|_| "rimuru".to_owned())
            });
        let Some(to) = to.filter(|t| *t != c.name) else {
            found.left.push(format!("{} reports to {from}, who is no living creature, and no living ancestor or throne stands above: relink it by hand", c.name));
            continue;
        };
        let fix = Fix::Relink {
            body: c.name.clone(),
            from: from.to_owned(),
            to: to.clone(),
        };
        if apply {
            let path = world.join(c.doc());
            let text = fs::read_to_string(&path)?;
            let relinked: Vec<String> = text
                .lines()
                .map(|l| {
                    if l.trim().starts_with("- **Reports to:**") {
                        format!("- **Reports to:** {to}")
                    } else {
                        l.to_owned()
                    }
                })
                .collect();
            fs::write(&path, relinked.join("\n") + "\n")?;
            journal(world, &fix);
        }
        found.fixes.push(fix);
    }
    Ok(())
}

/// One repair, one journal entry.
fn journal(world: &Path, fix: &Fix) {
    let entry = Entry {
        body: "housekeep".into(),
        title: "heal".into(),
        lines: vec![("Fixed".into(), fix.line())],
    };
    if let Err(e) = append(world, &entry) {
        eprintln!("mnemorph: the heal could not reach the journal: {e}");
    }
}
