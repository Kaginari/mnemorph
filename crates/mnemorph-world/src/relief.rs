//! Relief: a strained creature's ground split in two young of its race — by its prefixes, or by
//! the directory tree under a one-prefix ground — or, when the ground cannot split, its seat
//! retired. The old seat is retired either way: it keeps its memory, claims nothing, and stays
//! visible; an Orc's living children report to the young that routes their ground. Every check
//! runs before any write, a failed write is undone, each relief is one journal entry, and nothing
//! is written through a link.

use std::fs;
use std::path::{Path, PathBuf};

use crate::doc::{Creature, covers};
use crate::heal::splice;
use crate::log::{Entry, append};
use crate::roster::Roster;
use mnemorph_races::Race;

/// What relief does to a creature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Its ground split in two young: their names, and the claims each takes.
    Split {
        /// The two young, each with its claims.
        young: [(String, Vec<String>); 2],
    },
    /// Its ground cannot split: the seat is retired.
    Retire,
}

/// The relief of one creature, planned or done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relief {
    /// The creature relieved.
    pub name: String,
    /// What relief does.
    pub act: Act,
    /// Its living children, each with the young it reports to after (an Orc's Slimes).
    pub children: Vec<(String, String)>,
    /// Open orders on the shared queue held by the old seat: re-route them by hand.
    pub orders: Vec<String>,
}

impl Relief {
    /// The relief in words.
    #[must_use]
    pub fn line(&self) -> String {
        let mut said = match &self.act {
            Act::Split { young } => format!(
                "{}'s ground splits: {} takes {}, {} takes {}; {} retires",
                self.name,
                young[0].0,
                young[0].1.join(", "),
                young[1].0,
                young[1].1.join(", "),
                self.name
            ),
            Act::Retire => format!("{}'s ground cannot split: its seat retires", self.name),
        };
        for (child, to) in &self.children {
            said.push_str("; ");
            said.push_str(child);
            said.push_str(" reports to ");
            said.push_str(to);
        }
        if !self.orders.is_empty() {
            said.push_str("; open orders it held, to re-route: ");
            said.push_str(&self.orders.join(", "));
        }
        said
    }
}

/// Plan `name`'s relief in `world` — every check made — and with `apply`, carry it out.
///
/// # Errors
///
/// No living Slime or Orc of that name, a young's name or seat already taken, a link where a doc
/// goes, or a doc that cannot be written.
pub fn relieve(world: &Path, name: &str, apply: bool) -> Result<Relief, String> {
    let roster = Roster::load(world);
    let c = roster
        .creatures
        .get(name)
        .filter(|c| !c.retired)
        .ok_or_else(|| format!("no living creature is named `{name}`"))?;
    let ground = match c.race.base() {
        Race::Slime => &c.zone,
        Race::Orc => &c.domain,
        _ => {
            return Err(format!(
                "relief splits Slimes and Orcs; {name} is a {}",
                c.race.name()
            ));
        }
    };
    let act = match halves(world, ground) {
        Some((a, b)) => Act::Split {
            young: [(format!("{name}-a"), a), (format!("{name}-b"), b)],
        },
        None => Act::Retire,
    };
    let children = heirs(&roster, c, &act);
    let orders = crate::queue::Queue::of(world)
        .open()
        .into_iter()
        .filter(|i| i.holder() == name)
        .map(|i| i.id)
        .collect();
    let relief = Relief {
        name: name.to_owned(),
        act,
        children,
        orders,
    };
    let seats = preflight(world, &roster, c, &relief)?;
    if apply {
        carry_out(world, &roster, c, &relief, &seats)?;
    }
    Ok(relief)
}

/// Each living child of `c` and the young it reports to after: the one whose ground covers the
/// child's, else the first. A retired seat's children stay as they are, for the heal.
fn heirs(roster: &Roster, c: &Creature, act: &Act) -> Vec<(String, String)> {
    let Act::Split { young } = act else {
        return Vec::new();
    };
    roster
        .living()
        .filter(|k| k.reports_to.as_deref() == Some(c.name.as_str()))
        .map(|k| {
            let ground: Vec<&String> = k.zone.iter().chain(&k.domain).collect();
            let to = young
                .iter()
                .find(|(_, claims)| {
                    !ground.is_empty()
                        && ground
                            .iter()
                            .all(|g| claims.iter().any(|claim| covers(claim, g)))
                })
                .map_or(&young[0].0, |(n, _)| n);
            (k.name.clone(), to.clone())
        })
        .collect()
}

/// A ground in two: its claims in halves when it has two or more — a claim another covers set
/// aside — or the entries under its one directory, descending while that directory holds a
/// single one. Hidden entries go with the first half; a link counts as a file.
fn halves(world: &Path, ground: &[String]) -> Option<(Vec<String>, Vec<String>)> {
    let mut claims: Vec<String> = ground
        .iter()
        .filter(|g| !ground.iter().any(|o| o != *g && covers(o, g)))
        .cloned()
        .collect();
    let mut hidden = Vec::new();
    while let [only] = claims.as_slice()
        && only.ends_with('/')
    {
        let dir = only.clone();
        let entries = fs::read_dir(world.join(dir.trim_start_matches("./"))).ok()?;
        let (shown, dotted): (Vec<_>, Vec<_>) = entries
            .filter_map(Result::ok)
            .map(|e| {
                let file = e.file_name().to_string_lossy().into_owned();
                let is_dir = fs::symlink_metadata(e.path()).is_ok_and(|m| m.is_dir());
                (
                    file.starts_with('.'),
                    format!("{dir}{file}{}", if is_dir { "/" } else { "" }),
                )
            })
            .partition(|(dot, _)| !dot);
        claims = shown.into_iter().map(|(_, p)| p).collect();
        hidden.extend(dotted.into_iter().map(|(_, p)| p));
        if claims.is_empty() {
            return None;
        }
    }
    claims.sort();
    if claims.len() < 2 {
        return None;
    }
    let b = claims.split_off(claims.len().div_ceil(2));
    let mut a = claims;
    a.extend(hidden);
    Some((a, b))
}

/// A claim as a doc writes it: a file at the root keeps its `./`, or it reads as no path.
fn written(claim: &str) -> String {
    if claim.contains('/') {
        claim.to_owned()
    } else {
        format!("./{claim}")
    }
}

/// Every check before any write: the old doc is no link, and each young's name and seat are
/// free. Returns the young's seats.
fn preflight(
    world: &Path,
    roster: &Roster,
    c: &Creature,
    relief: &Relief,
) -> Result<Vec<PathBuf>, String> {
    let linked = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
    if linked(&world.join(c.doc())) || linked(&world.join(&c.seat)) {
        return Err(format!("{} is a link: relieve {} by hand", c.doc(), c.name));
    }
    let Act::Split { young } = &relief.act else {
        return Ok(Vec::new());
    };
    let race_dir = world
        .join(&c.seat)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let short = Path::new(&c.seat)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut seats = Vec::new();
    for ((name, _), end) in young.iter().zip(["a", "b"]) {
        let seat = race_dir.join(format!("{short}-{end}"));
        if roster.creatures.contains_key(name) || fs::symlink_metadata(&seat).is_ok() {
            return Err(format!(
                "`{name}` or its seat already stands: relieve {} by hand",
                c.name
            ));
        }
        seats.push(seat);
    }
    Ok(seats)
}

/// Write the young's docs (undone if any write fails), relink the children, retire the old
/// seat, journal the act.
fn carry_out(
    world: &Path,
    roster: &Roster,
    c: &Creature,
    relief: &Relief,
    seats: &[PathBuf],
) -> Result<(), String> {
    if let Act::Split { young } = &relief.act
        && let Err(e) = write_young(c, young, seats)
    {
        for seat in seats {
            let _ = fs::remove_dir_all(seat);
        }
        return Err(e);
    }
    // Personal memory survives the split: each heir inherits the notes whose sources lie in its
    // ground; the old creature's file is kept whole.
    if let Act::Split { young } = &relief.act {
        mnemorph_memory::inherit(world, &c.name, young)
            .map_err(|e| format!("the young are seated, but the notes could not follow: {e}"))?;
    }
    for (child, to) in &relief.children {
        let Some(k) = roster.creatures.get(child) else {
            continue;
        };
        let Some(&at) = k.lines.get("Reports to") else {
            continue;
        };
        let path = world.join(k.doc());
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        fs::write(&path, splice(&text, at, &format!("- **Reports to:** {to}")))
            .map_err(|e| e.to_string())?;
    }
    let heirs: Vec<String> = match &relief.act {
        Act::Split { young } => young.iter().map(|(n, _)| n.clone()).collect(),
        Act::Retire => Vec::new(),
    };
    retire(world, c, &heirs)?;
    let entry = Entry {
        body: c.name.clone(),
        title: "relief".into(),
        lines: vec![("Act".into(), relief.line())],
    };
    append(world, &entry).map_err(|e| format!("relieved, but the journal could not record it: {e}"))
}

/// The young's docs: the old one's base race, parent, verify lines and Minds, half its ground each.
fn write_young(
    c: &Creature,
    young: &[(String, Vec<String>); 2],
    seats: &[PathBuf],
) -> Result<(), String> {
    let race = c.race.base().name();
    let field = if c.race.base() == Race::Slime {
        "Territory"
    } else {
        "Domain"
    };
    for ((name, claims), seat) in young.iter().zip(seats) {
        let mut text = format!("# {name}\n\n- **Race:** {race}\n");
        if let Some(p) = &c.reports_to {
            text.push_str("- **Reports to:** ");
            text.push_str(p);
            text.push('\n');
        }
        let claims: Vec<String> = claims.iter().map(|g| written(g)).collect();
        text.push_str("- **");
        text.push_str(field);
        text.push_str(":** ");
        text.push_str(&claims.join(", "));
        text.push('\n');
        for v in &c.verify {
            text.push_str("- **Verify:** `");
            text.push_str(v);
            text.push_str("`\n");
        }
        if !c.minds.is_empty() {
            text.push_str("- **Minds:** ");
            text.push_str(&c.minds.join(", "));
            text.push('\n');
        }
        text.push_str("- **Heir of:** ");
        text.push_str(&c.name);
        text.push_str("\n\nRelief split ");
        text.push_str(&c.name);
        text.push_str("'s ground.\n");
        fs::create_dir_all(seat).map_err(|e| e.to_string())?;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(seat.join("README.md"))
            .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Mark `c`'s seat retired, naming its heirs when it has some: its `Retired` line set, or one added
/// after its last header field, with a `Heirs` line beside it.
fn retire(world: &Path, c: &Creature, heirs: &[String]) -> Result<(), String> {
    let path = world.join(c.doc());
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut said = String::from("- **Retired:** yes");
    if !heirs.is_empty() {
        // The ending of the line the heirs follow: the Retired line's own, else the last field's.
        let at = c
            .lines
            .get("Retired")
            .copied()
            .unwrap_or_else(|| c.lines.values().copied().max().unwrap_or(1));
        let crlf = text
            .split_inclusive('\n')
            .nth(at.saturating_sub(1))
            .is_some_and(|l| l.ends_with("\r\n"));
        let ending = if crlf { "\r\n" } else { "\n" };
        said.push_str(ending);
        said.push_str("- **Heirs:** ");
        said.push_str(&heirs.join(", "));
    }
    if let Some(&at) = c.lines.get("Retired") {
        return fs::write(&path, splice(&text, at, &said)).map_err(|e| e.to_string());
    }
    let after = c.lines.values().copied().max().unwrap_or(1);
    let mut out = String::with_capacity(text.len() + 24);
    for (n, line) in text.split_inclusive('\n').enumerate() {
        out.push_str(line);
        if n + 1 == after {
            let ending = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
            if !line.ends_with('\n') {
                out.push_str(ending);
            }
            out.push_str(&said);
            out.push_str(ending);
        }
    }
    fs::write(&path, out).map_err(|e| e.to_string())
}
