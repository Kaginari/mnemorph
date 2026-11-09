//! Slimes an Orc starts: a domain agent may start up to [`MAX_SUMMONED`] Slimes on parts of its own
//! domain, with no human asked, for the commission it is working. Each is recorded in the
//! creatures' witness as the harness's act, naming the Orc, so the human sees every one and may
//! retire any; and each is retired when the Orc's commission ends, its card included. The ground
//! it gets is the Orc's own, never a living Slime's: an Orc cannot hand out what it does not hold.

use std::fs;
use std::path::Path;

use crate::creatures::{self, Act, Ground, HARNESS};
use crate::doc::covers;
use crate::races::Race;
use crate::roster::Roster;

/// How many Slimes one Orc may have started and not yet released.
pub const MAX_SUMMONED: usize = 3;

/// The most seats one Orc's started Slimes are named over, released ones included.
const SEAT_NAMES: u32 = 10_000;

/// Who a summoned Slime's rows name: the harness, for the Orc that asked.
#[must_use]
pub fn by(orc: &str) -> String {
    format!("{HARNESS}: summoned by {orc}")
}

/// The living Slimes `orc` started, by name, as the witness holds them.
///
/// # Errors
///
/// The witness cannot be read.
pub fn summoned(world: &Path, orc: &str) -> Result<Vec<String>, String> {
    let rows = creatures::witnessed(world)?.unwrap_or_default();
    // The witness says who wrote a creature's last row after its act: `summon by …`.
    let by = format!(" by {}", by(orc));
    Ok(rows
        .into_iter()
        .filter(|(_, w)| w.by.ends_with(&by) && !w.retired)
        .map(|(name, _)| name)
        .collect())
}

/// A Slime an Orc started: its name, whether it was woken rather than born, and how many times
/// it has been started in all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    /// The Slime.
    pub name: String,
    /// It held this ground before and was released: woken, its memory its own still.
    pub woke: bool,
    /// How many times it has been started, this one included.
    pub times: usize,
}

/// Start a Slime for `orc` on `zone`, part of its own domain. A Slime the Orc started on exactly
/// this ground before, and that was released, is woken with what it learned; otherwise a new one
/// is born.
///
/// # Errors
///
/// `orc` is no living Orc; the zone is empty, reaches past the Orc's domain or overlaps a living
/// Slime's; the Orc already has [`MAX_SUMMONED`]; or the card or its row cannot be written.
pub fn summon(world: &Path, orc: &str, zone: &[String]) -> Result<Started, String> {
    let roster = Roster::load(world);
    let Some(o) = roster
        .creatures
        .get(orc)
        .filter(|c| !c.retired && c.race.base() == Race::Orc)
    else {
        return Err(format!(
            "`{orc}` is no living Orc: only an Orc starts a Slime"
        ));
    };
    let zone = checked_zone(&roster, &o.domain, zone)?;
    let live = summoned(world, orc)?;
    if live.len() >= MAX_SUMMONED {
        return Err(format!(
            "you have started {} Slimes ({}), the most at once: commission one of them",
            live.len(),
            live.join(", ")
        ));
    }
    for at in [world.join(".mnemorph"), world.join(".mnemorph/slime")] {
        if fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "{}: a link; nothing is written through one",
                at.display()
            ));
        }
    }
    let rows = creatures::witnessed(world)?.unwrap_or_default();
    let mark = format!(" by {}", by(orc));
    let asleep = rows
        .iter()
        .find(|(_, w)| w.retired && w.by.ends_with(&mark) && w.ground.zone == zone)
        .map(|(name, _)| name.clone());
    let (name, woke) = match asleep {
        Some(name) => (name, true),
        None => (fresh_name(world, &roster, orc)?, false),
    };
    let seat = name.strip_prefix("slime-").unwrap_or(&name);
    let dir = world.join(".mnemorph/slime").join(seat);
    let card = format!(
        "# {name}\n\n- **Race:** slime\n- **Reports to:** {orc}\n- **Territory:** {}\n\n\
         Started by {orc} for one commission, on part of its domain; retired when it ends.\n",
        zone.join(", ")
    );
    write_card(&dir, &card, woke)?;
    let ground = Ground {
        zone,
        ..Ground::default()
    };
    let row = creatures::row(
        Act::Summon,
        &name,
        Race::Slime,
        Some(orc),
        ground,
        false,
        &by(orc),
    );
    if let Err(e) = creatures::record(world, vec![row]) {
        // A card no row holds is a stranger: a new one is taken back, so nothing is half started.
        if !woke {
            let _ = fs::remove_dir_all(&dir);
        }
        return Err(e);
    }
    let times = times(world, &name);
    Ok(Started { name, woke, times })
}

/// `zone` as a Slime of the Orc with `domain` may keep it: plain paths of the domain, kept by no
/// living Slime, sorted.
fn checked_zone(
    roster: &Roster,
    domain: &[String],
    zone: &[String],
) -> Result<Vec<String>, String> {
    let mut zone: Vec<String> = zone.iter().map(|z| z.trim().to_owned()).collect();
    zone.retain(|z| !z.is_empty());
    zone.sort();
    zone.dedup();
    if zone.is_empty() {
        return Err("a Slime needs a zone: the paths of your domain it keeps".into());
    }
    for z in &zone {
        if z.starts_with('/') || z.split('/').any(|part| part == "..") {
            return Err(format!("`{z}` is no plain path under the world"));
        }
        if !domain.iter().any(|d| covers(d, z)) {
            return Err(format!(
                "`{z}` is not in your domain ({}): you start a Slime on your own ground only",
                domain.join(", ")
            ));
        }
        if let Some(s) = roster.living().find(|c| {
            c.race.base() == Race::Slime
                && c.zone.iter().any(|kept| covers(kept, z) || covers(z, kept))
        }) {
            return Err(format!(
                "`{z}` is kept by {}: commission it instead",
                s.name
            ));
        }
    }
    Ok(zone)
}

/// A name for a new Slime of `orc`'s: never one a creature or a seat has had.
fn fresh_name(world: &Path, roster: &Roster, orc: &str) -> Result<String, String> {
    let short = orc.strip_prefix("orc-").unwrap_or(orc);
    (1..=SEAT_NAMES)
        .map(|n| format!("{short}-{n}"))
        .find(|seat| {
            !roster.creatures.contains_key(&format!("slime-{seat}"))
                && fs::symlink_metadata(world.join(".mnemorph/slime").join(seat)).is_err()
        })
        .map(|seat| format!("slime-{seat}"))
        .ok_or_else(|| "no name is free".to_owned())
}

/// The card at `dir`: a new one never over another, a woken one written back as live.
fn write_card(dir: &Path, card: &str, woke: bool) -> Result<(), String> {
    if fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!(
            "{}: a link; nothing is written through one",
            dir.display()
        ));
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join("README.md");
    if woke {
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "{}: a link; nothing is written through one",
                path.display()
            ));
        }
        return fs::write(&path, card).map_err(|e| e.to_string());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, card.as_bytes()))
        .map_err(|e| e.to_string())
}

/// How many times `name` has been started, as the witness's rows count it.
#[must_use]
pub fn times(world: &Path, name: &str) -> usize {
    creatures::rows_in(crate::queue::state_dir().as_deref(), world)
        .ok()
        .flatten()
        .map_or(0, |rows| {
            rows.iter()
                .filter(|r| r.act == Act::Summon && r.name == name)
                .count()
        })
}

/// Retire every living Slime `orc` started: the end of its commission. The names released.
///
/// # Errors
///
/// The witness cannot be read or written, or a card cannot be marked.
pub fn release(world: &Path, orc: &str) -> Result<Vec<String>, String> {
    let names = summoned(world, orc)?;
    if names.is_empty() {
        return Ok(names);
    }
    let roster = Roster::cards(world);
    let mut rows = Vec::new();
    for name in &names {
        let Some(c) = roster.creatures.get(name) else {
            continue;
        };
        crate::relief::retire(world, c, &[])?;
        rows.push(creatures::row(
            Act::Release,
            name,
            Race::Slime,
            Some(orc),
            Ground::of(c),
            true,
            &by(orc),
        ));
    }
    creatures::record(world, rows)?;
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World {
        _dir: tempfile::TempDir,
        root: std::path::PathBuf,
    }

    fn doc(root: &Path, race: &str, short: &str, body: &str) {
        let dir = root.join(".mnemorph").join(race).join(short);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), body).unwrap();
    }

    /// An Orc over `src/`, with a Slime already keeping `src/wire/`, all adopted.
    fn world() -> World {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("w");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        doc(
            &root,
            "summoner",
            "summoner",
            "# summoner\n\n- **Race:** summoner\n",
        );
        doc(
            &root,
            "elf",
            "e",
            "# elf-e\n\n- **Race:** elf\n- **Reports to:** summoner\n- **Territory:** src/\n",
        );
        doc(
            &root,
            "orc",
            "o",
            "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** src/\n",
        );
        doc(
            &root,
            "slime",
            "w",
            "# slime-w\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/wire/\n",
        );
        creatures::record(&root, creatures::adoption(&root)).unwrap();
        World { _dir: dir, root }
    }

    #[test]
    fn an_orc_starts_a_slime_on_its_own_ground_and_it_is_trusted() {
        let w = world();
        let name = summon(&w.root, "orc-o", &["src/parse/".into()])
            .unwrap()
            .name;
        assert_eq!(name, "slime-o-1");
        let roster = Roster::load(&w.root);
        let s = &roster.creatures[&name];
        assert_eq!(s.race, Race::Slime);
        assert_eq!(s.reports_to.as_deref(), Some("orc-o"));
        assert_eq!(s.zone, ["src/parse/"]);
        let rows = creatures::witnessed(&w.root).unwrap().unwrap();
        assert_eq!(rows[&name].by, "summon by harness: summoned by orc-o");
        assert_eq!(summoned(&w.root, "orc-o").unwrap(), [name]);
    }

    #[test]
    fn ground_it_does_not_hold_or_a_slime_keeps_is_refused() {
        let w = world();
        let e = summon(&w.root, "orc-o", &["docs/".into()]).unwrap_err();
        assert!(e.contains("not in your domain"), "{e}");
        let e = summon(&w.root, "orc-o", &["src/wire/frame.rs".into()]).unwrap_err();
        assert!(e.contains("kept by slime-w"), "{e}");
        let e = summon(&w.root, "orc-o", &["src/../etc/".into()]).unwrap_err();
        assert!(e.contains("no plain path"), "{e}");
        let e = summon(&w.root, "slime-w", &["src/x/".into()]).unwrap_err();
        assert!(e.contains("no living Orc"), "{e}");
        assert_eq!(summoned(&w.root, "orc-o").unwrap(), [] as [String; 0]);
    }

    #[test]
    fn three_at_once_and_all_released_when_the_commission_ends() {
        let w = world();
        for part in ["a/", "b/", "c/"] {
            summon(&w.root, "orc-o", &[format!("src/{part}")]).unwrap();
        }
        let e = summon(&w.root, "orc-o", &["src/d/".into()]).unwrap_err();
        assert!(e.contains("the most at once"), "{e}");
        let released = release(&w.root, "orc-o").unwrap();
        assert_eq!(released.len(), 3);
        assert_eq!(summoned(&w.root, "orc-o").unwrap(), [] as [String; 0]);
        let roster = Roster::load(&w.root);
        assert!(
            released.iter().all(|n| roster.creatures[n].retired),
            "retired, card and row"
        );
        // Released, the Orc may start again: on new ground, under a name never used.
        let fresh = summon(&w.root, "orc-o", &["src/e/".into()]).unwrap();
        assert_eq!(fresh.name, "slime-o-4");
        assert!(!fresh.woke);
    }

    #[test]
    fn the_same_ground_wakes_the_same_slime_with_what_it_learned() {
        let w = world();
        let first = summon(&w.root, "orc-o", &["src/parse/".into()]).unwrap();
        let memory = w.root.join(".mnemorph/memory/creature/slime-o-1.jsonl");
        fs::create_dir_all(memory.parent().unwrap()).unwrap();
        fs::write(&memory, "{\"note\": \"the lexer is in parse/\"}\n").unwrap();
        release(&w.root, "orc-o").unwrap();
        let again = summon(&w.root, "orc-o", &["src/parse/".into()]).unwrap();
        assert_eq!(again.name, first.name, "the same Slime, not a new one");
        assert!(again.woke);
        assert_eq!(again.times, 2);
        assert!(memory.exists(), "its memory is its own, kept");
        let roster = Roster::load(&w.root);
        assert!(
            !roster.creatures[&again.name].retired,
            "live again, card and row"
        );
        assert_eq!(
            summoned(&w.root, "orc-o").unwrap(),
            std::slice::from_ref(&again.name)
        );
    }
}
