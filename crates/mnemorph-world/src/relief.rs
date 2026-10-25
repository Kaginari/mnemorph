//! Relief: a strained creature's ground split in two young of its race — by its prefixes, or by
//! the directory tree under a one-prefix ground — or, when the ground cannot split, its seat
//! retired. The old seat is retired either way: it keeps its memory, claims nothing, and stays
//! visible. Each relief is one journal entry; nothing is written through a link.

use std::fs;
use std::path::Path;

use mnemorph_races::Race;

use crate::doc::Creature;
use crate::log::{Entry, append};
use crate::roster::Roster;

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
}

impl Relief {
    /// The relief in words.
    #[must_use]
    pub fn line(&self) -> String {
        match &self.act {
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
        }
    }
}

/// Plan `name`'s relief in `world`, and with `apply`, carry it out.
///
/// # Errors
///
/// No living Slime or Orc of that name, a young's seat already taken, a link where a doc goes, or
/// a doc that cannot be written.
pub fn relieve(world: &Path, name: &str, apply: bool) -> Result<Relief, String> {
    let roster = Roster::load(world);
    let c = roster
        .creatures
        .get(name)
        .filter(|c| !c.retired)
        .ok_or_else(|| format!("no living creature is named `{name}`"))?;
    let (field, ground) = match c.race.base() {
        Race::Slime => ("Territory", &c.zone),
        Race::Orc => ("Domain", &c.domain),
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
    let relief = Relief {
        name: name.to_owned(),
        act,
    };
    if apply {
        carry_out(world, &roster, c, field, &relief)?;
    }
    Ok(relief)
}

/// A ground in two: its claims in halves when it has two or more, or the entries under its one
/// directory when it has one and they are two or more.
fn halves(world: &Path, ground: &[String]) -> Option<(Vec<String>, Vec<String>)> {
    let mut claims: Vec<String> = ground.to_vec();
    if let [only] = ground
        && only.ends_with('/')
    {
        let entries = fs::read_dir(world.join(only.trim_start_matches("./"))).ok()?;
        claims = entries
            .filter_map(Result::ok)
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| {
                let path = format!("{only}{}", e.file_name().to_string_lossy());
                if e.path().is_dir() {
                    format!("{path}/")
                } else {
                    path
                }
            })
            .collect();
    }
    claims.sort();
    if claims.len() < 2 {
        return None;
    }
    let b = claims.split_off(claims.len().div_ceil(2));
    Some((claims, b))
}

/// Write the young's docs, retire the old seat, journal the act.
fn carry_out(
    world: &Path,
    roster: &Roster,
    c: &Creature,
    field: &str,
    relief: &Relief,
) -> Result<(), String> {
    let race = c.race.name();
    if let Act::Split { young } = &relief.act {
        for (name, claims) in young {
            if roster.creatures.contains_key(name) {
                return Err(format!(
                    "`{name}` is already a creature: relieve {} by hand",
                    c.name
                ));
            }
            let short = name.strip_prefix(&format!("{race}-")).unwrap_or(name);
            let seat = world.join(".mnemorph").join(race).join(short);
            if fs::symlink_metadata(&seat).is_ok() {
                return Err(format!(
                    "{} already stands: relieve {} by hand",
                    seat.display(),
                    c.name
                ));
            }
            let parent = c
                .reports_to
                .as_deref()
                .map(|p| format!("- **Reports to:** {p}\n"))
                .unwrap_or_default();
            let mut verify = String::new();
            for v in &c.verify {
                verify.push_str("- **Verify:** `");
                verify.push_str(v);
                verify.push_str("`\n");
            }
            let text = format!(
                "# {name}\n\n- **Race:** {race}\n{parent}- **{field}:** {}\n{verify}\nHeir of {}: relief split its ground.\n",
                claims.join(", "),
                c.name
            );
            fs::create_dir_all(&seat).map_err(|e| e.to_string())?;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(seat.join("README.md"))
                .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()))
                .map_err(|e| e.to_string())?;
        }
    }
    retire(world, c)?;
    let entry = Entry {
        body: c.name.clone(),
        title: "relief".into(),
        lines: vec![("Act".into(), relief.line())],
    };
    append(world, &entry).map_err(|e| format!("relieved, but the journal could not record it: {e}"))
}

/// Mark `c`'s seat retired: its `Retired` line set, or one added after its last header field.
fn retire(world: &Path, c: &Creature) -> Result<(), String> {
    let path = world.join(c.doc());
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("{} is a link: retire {} by hand", c.doc(), c.name));
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let at = c.lines.get("Retired").copied();
    let after = c.lines.values().copied().max().unwrap_or(1);
    let mut out = String::with_capacity(text.len() + 24);
    for (n, line) in text.split_inclusive('\n').enumerate() {
        let ending = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
        if Some(n + 1) == at {
            out.push_str("- **Retired:** yes");
            out.push_str(ending);
            continue;
        }
        out.push_str(line);
        if at.is_none() && n + 1 == after {
            if !line.ends_with('\n') {
                out.push_str(ending);
            }
            out.push_str("- **Retired:** yes");
            out.push_str(ending);
        }
    }
    fs::write(&path, out).map_err(|e| e.to_string())
}
