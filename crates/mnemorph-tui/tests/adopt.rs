//! The first-run adoption: a fresh world asks nothing; a cloned one lists every stranger with the
//! standing its card claims and adopts them all on one deliberate yes, or none.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_loop::world::Roster;
use mnemorph_loop::world::creatures;
use mnemorph_loop::world::races::Race;
use mnemorph_tui::adopt::{Offered, offer_in};

struct World {
    root: PathBuf,
    world: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-tui-adopt-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let world = root.join("world");
        fs::create_dir_all(&world).unwrap();
        let w = World {
            world: world.canonicalize().unwrap(),
            root,
        };
        w.card("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n");
        w
    }

    fn card(&self, race: &str, seat: &str, text: &str) {
        let at = self.world.join(".mnemorph").join(race).join(seat);
        fs::create_dir_all(&at).unwrap();
        fs::write(at.join("README.md"), text).unwrap();
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn offer(&self, ask: &mut dyn FnMut(&str) -> Result<bool, String>) -> (Offered, String) {
        let mut out = Vec::new();
        let got = offer_in(Some(&self.state()), &self.world, &mut out, ask).unwrap();
        (got, String::from_utf8_lossy(&out).into_owned())
    }

    fn view(&self) -> Roster {
        Roster::load_in(Some(&self.state()), &self.world)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn path_of(state: &Path, world: &Path) -> PathBuf {
    creatures::path_in(state, world)
}

/// A fresh world — its throne alone — asks nothing: the throne is implicit.
#[test]
fn a_fresh_world_asks_nothing() {
    let w = World::new("fresh");
    let (got, text) = w.offer(&mut |_| panic!("a fresh world is asked nothing"));
    assert_eq!((got, text.as_str()), (Offered::Nothing, ""));
    assert_eq!(w.view().creatures["rimuru"].race, Race::Rimuru);
}

/// A cloned world's cards are listed with what each claims; only a yes adopts, and all at once.
#[test]
fn a_cloned_world_asks_once_and_adopts_what_it_showed() {
    let w = World::new("clone");
    w.card(
        "kijin",
        "forge",
        "# kijin-forge\n\n- **Race:** oni\n- **Reports to:** rimuru\n- **Post:** .github/workflows/\n",
    );
    w.card(
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n",
    );
    let mut asked = 0;
    let (got, text) = w.offer(&mut |_| {
        asked += 1;
        Ok(false)
    });
    assert_eq!(
        got,
        Offered::Declined(vec!["kijin-forge".into(), "slime-s".into()])
    );
    assert!(
        text.contains(
            "kijin-forge — oni, post kijin-forge, reports to rimuru, ground Post .github/workflows/"
        ) && text.contains("slime-s — slime, reports to orc-o, ground Territory src/")
            && text.contains("mnemorphd roster adopt"),
        "{text}"
    );
    assert_eq!(
        w.view().creatures["kijin-forge"].race,
        Race::Slime,
        "still a stranger"
    );
    // No terminal is no answer.
    let (got, _) = w.offer(&mut |_| Err("no terminal".into()));
    assert!(matches!(got, Offered::Declined(_)));
    assert!(!path_of(&w.state(), &w.world).exists(), "nothing written");
    let (got, _) = w.offer(&mut |_| {
        asked += 1;
        Ok(true)
    });
    assert_eq!(
        got,
        Offered::Adopted(vec!["kijin-forge".into(), "slime-s".into()])
    );
    assert_eq!(asked, 2, "one question each time");
    let v = w.view();
    assert_eq!(v.creatures["kijin-forge"].race, Race::Oni);
    assert_eq!(v.creatures["slime-s"].zone, ["src/"]);
    let (got, _) = w.offer(&mut |_| panic!("asked again"));
    assert_eq!(got, Offered::Nothing);
}

/// A card that differs from its row is shown with the way on, never adopted at the opening.
#[test]
fn a_mismatched_card_is_never_adopted_at_the_opening() {
    let w = World::new("differs");
    w.card(
        "kijin",
        "forge",
        "# kijin-forge\n\n- **Race:** kijin\n- **Reports to:** rimuru\n",
    );
    let (got, _) = w.offer(&mut |_| Ok(true));
    assert_eq!(got, Offered::Adopted(vec!["kijin-forge".into()]));
    w.card(
        "kijin",
        "forge",
        "# kijin-forge\n\n- **Race:** oni\n- **Reports to:** rimuru\n",
    );
    let (got, text) = w.offer(&mut |_| panic!("a mismatch is never asked here"));
    assert_eq!(got, Offered::Nothing);
    assert!(
        text.contains("kijin-forge's card differs from its row")
            && text.contains("mnemorphd roster adopt kijin-forge"),
        "{text}"
    );
    assert_eq!(w.view().creatures["kijin-forge"].race, Race::Kijin);
}
