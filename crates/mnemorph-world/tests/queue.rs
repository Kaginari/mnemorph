//! The shared work queue: snapshots with revisions, a fold that refuses a line that does not
//! follow, the rank law, and the states an item moves through.

use std::fs;
use std::path::PathBuf;

use mnemorph_world::Roster;
use mnemorph_world::queue::{Item, Move, Queue, Refusal, State};

struct World(PathBuf);

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-queue-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let w = World(root);
        w.doc("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n");
        w.doc(
            "elf",
            "e",
            "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n",
        );
        w.doc(
            "orc",
            "o",
            "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n",
        );
        w.doc(
            "orc",
            "p",
            "# orc-p\n\n- **Race:** orc\n- **Reports to:** elf-e\n",
        );
        w.doc(
            "slime",
            "s",
            "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n",
        );
        w
    }

    fn doc(&self, race: &str, short: &str, body: &str) {
        let dir = self.0.join(".mnemorph").join(race).join(short);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), body).unwrap();
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn an_item_moves_held_claimed_done() {
    let w = World::new("states");
    let r = Roster::load(&w.0);
    let q = Queue::of(&w.0);
    let item = q
        .add(Some("slime-s"), "orc-o", "add the tab test", 0)
        .unwrap();
    assert_eq!((item.state, item.rev), (State::Held, 1));
    let claimed = q
        .move_item(&r, "slime-s", &item.id, &Move::Claim { as_body: None })
        .unwrap();
    assert_eq!((claimed.state, claimed.rev), (State::Claimed, 2));
    assert_eq!(
        q.move_item(&r, "slime-s", &item.id, &Move::Claim { as_body: None }),
        Err(Refusal::State {
            now: State::Claimed,
            wanted: "be claimed"
        })
    );
    q.move_item(&r, "slime-s", &item.id, &Move::Requeue)
        .unwrap();
    q.move_item(&r, "slime-s", &item.id, &Move::Claim { as_body: None })
        .unwrap();
    let done = q
        .move_item(&r, "slime-s", &item.id, &Move::Close { drop: false })
        .unwrap();
    assert_eq!((done.state, done.rev), (State::Done, 5));
    assert_eq!(q.open(), Vec::new());
    assert_eq!(done.notes.last().unwrap(), "done, closed by slime-s");
}

/// A body moves the work it holds; anyone up the holder's chain may too; a sibling may not; the
/// human stands over every body; work held by no one is the throne's.
#[test]
fn the_rank_law() {
    let w = World::new("rank");
    let r = Roster::load(&w.0);
    let q = Queue::of(&w.0);
    let item = q.add(Some("slime-s"), "orc-o", "x", 0).unwrap();
    let touch = || Move::Touch {
        note: "seen".into(),
    };
    for mover in ["slime-s", "orc-o", "elf-e", "rimuru", "human"] {
        assert!(
            q.move_item(&r, mover, &item.id, &touch()).is_ok(),
            "{mover}"
        );
    }
    assert_eq!(
        q.move_item(&r, "orc-p", &item.id, &touch()),
        Err(Refusal::NotYours {
            holder: "slime-s".into()
        })
    );
    let throne = q.add(None, "human", "y", 0).unwrap();
    assert!(
        q.move_item(&r, "elf-e", &throne.id, &touch()).is_err(),
        "unheld work is the throne's"
    );
    assert!(q.move_item(&r, "rimuru", &throne.id, &touch()).is_ok());
}

/// Two sessions racing for one move both append; the fold takes the first, the second reads back
/// refused.
#[test]
fn a_race_is_won_by_the_first_line() {
    let w = World::new("race");
    let q = Queue::of(&w.0);
    let item = q.add(Some("slime-s"), "orc-o", "x", 0).unwrap();
    // Another session lands rev 2 first.
    let other = Item {
        rev: 2,
        state: State::Dropped,
        ..item.clone()
    };
    let mut lines = fs::read_to_string(q.path()).unwrap();
    lines += &serde_json::to_string(&other).unwrap();
    lines.push('\n');
    fs::write(q.path(), lines).unwrap();
    let r = Roster::load(&w.0);
    // This session read rev 1 before the other landed: its rev 2 line is refused by the fold.
    let stale = Item {
        rev: 2,
        state: State::Claimed,
        ..item.clone()
    };
    let mut lines = fs::read_to_string(q.path()).unwrap();
    lines += &serde_json::to_string(&stale).unwrap();
    lines.push('\n');
    fs::write(q.path(), lines).unwrap();
    assert_eq!(
        q.items()[&item.id].state,
        State::Dropped,
        "the first rev 2 stands"
    );
    assert!(matches!(
        q.move_item(&r, "slime-s", &item.id, &Move::Claim { as_body: None }),
        Err(Refusal::State {
            now: State::Dropped,
            ..
        })
    ));
}

/// Work reaches a body claimed in its name: a body never claims an order for another.
#[test]
fn a_body_never_claims_for_another() {
    let w = World::new("claim-as");
    let r = Roster::load(&w.0);
    let q = Queue::of(&w.0);
    let item = q.add(Some("slime-s"), "orc-o", "x", 0).unwrap();
    let as_orc = Move::Claim {
        as_body: Some("orc-p".into()),
    };
    assert!(q.move_item(&r, "orc-o", &item.id, &as_orc).is_err());
    assert!(
        q.move_item(&r, "human", &item.id, &as_orc).is_ok(),
        "the human gives work to any body"
    );
}
