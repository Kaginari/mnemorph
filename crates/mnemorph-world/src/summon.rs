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

/// Start a Slime for `orc` on `zone`, part of its own domain: its name.
///
/// # Errors
///
/// `orc` is no living Orc; the zone is empty, reaches past the Orc's domain or overlaps a living
/// Slime's; the Orc already has [`MAX_SUMMONED`]; or the card or its row cannot be written.
pub fn summon(world: &Path, orc: &str, zone: &[String]) -> Result<String, String> {
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
        if !o.domain.iter().any(|d| covers(d, z)) {
            return Err(format!(
                "`{z}` is not in your domain ({}): you start a Slime on your own ground only",
                o.domain.join(", ")
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
    let live = summoned(world, orc)?;
    if live.len() >= MAX_SUMMONED {
        return Err(format!(
            "you have started {} Slimes ({}), the most at once: commission one of them",
            live.len(),
            live.join(", ")
        ));
    }
    let short = orc.strip_prefix("orc-").unwrap_or(orc);
    let (name, seat) = (1..=SEAT_NAMES)
        .map(|n| {
            let seat = format!("{short}-{n}");
            (format!("slime-{seat}"), seat)
        })
        .find(|(name, seat)| {
            !roster.creatures.contains_key(name)
                && fs::symlink_metadata(world.join(".mnemorph/slime").join(seat)).is_err()
        })
        .ok_or("no name is free")?;
    let dir = world.join(".mnemorph/slime").join(&seat);
    for at in [world.join(".mnemorph"), world.join(".mnemorph/slime")] {
        if fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "{}: a link; nothing is written through one",
                at.display()
            ));
        }
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let card = format!(
        "# {name}\n\n- **Race:** slime\n- **Reports to:** {orc}\n- **Territory:** {}\n\n\
         Started by {orc} for one commission, on part of its domain; retired when it ends.\n",
        zone.join(", ")
    );
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("README.md"))
        .and_then(|mut f| std::io::Write::write_all(&mut f, card.as_bytes()))
        .map_err(|e| e.to_string())?;
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
        // A card no row holds is a stranger: taken back, so nothing is left half started.
        let _ = fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok(name)
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
            "rimuru",
            "rimuru",
            "# rimuru\n\n- **Race:** rimuru\n",
        );
        doc(
            &root,
            "elf",
            "e",
            "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n- **Territory:** src/\n",
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
        let name = summon(&w.root, "orc-o", &["src/parse/".into()]).unwrap();
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
        // Released, the Orc may start again, under a name never used.
        let again = summon(&w.root, "orc-o", &["src/a/".into()]).unwrap();
        assert_eq!(again, "slime-o-4");
    }
}
