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
    let q = Queue::at(&w.0.join("queue.jsonl"));
    let item = q
        .add(
            &Roster::load(&w.0),
            Some("slime-s"),
            "orc-o",
            "add the tab test",
            0,
        )
        .unwrap();
    assert_eq!((item.state, item.rev), (State::Held, 1));
    let claimed = q
        .move_item(&r, "harness", &item.id, &Move::Claim { as_body: None })
        .unwrap();
    assert_eq!((claimed.state, claimed.rev), (State::Claimed, 2));
    assert_eq!(
        q.move_item(&r, "harness", &item.id, &Move::Claim { as_body: None }),
        Err(Refusal::State {
            now: State::Claimed,
            wanted: "be claimed"
        })
    );
    q.move_item(&r, "slime-s", &item.id, &Move::Requeue)
        .unwrap();
    q.move_item(&r, "harness", &item.id, &Move::Claim { as_body: None })
        .unwrap();
    let done = q
        .move_item(&r, "orc-o", &item.id, &Move::Close { drop: false })
        .unwrap();
    assert_eq!((done.state, done.rev), (State::Done, 5));
    assert_eq!(q.open(), Vec::new());
    assert_eq!(done.notes.last().unwrap(), "done, closed by orc-o");
}

/// A body moves the work it holds; anyone up the holder's chain may too; a sibling may not; the
/// human stands over every body; work held by no one is the throne's.
#[test]
fn the_rank_law() {
    let w = World::new("rank");
    let r = Roster::load(&w.0);
    let q = Queue::at(&w.0.join("queue.jsonl"));
    let item = q
        .add(&Roster::load(&w.0), Some("slime-s"), "orc-o", "x", 0)
        .unwrap();
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
    let throne = q.add(&Roster::load(&w.0), None, "human", "y", 0).unwrap();
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
    let q = Queue::at(&w.0.join("queue.jsonl"));
    let item = q
        .add(&Roster::load(&w.0), Some("slime-s"), "orc-o", "x", 0)
        .unwrap();
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
        q.move_item(&r, "harness", &item.id, &Move::Claim { as_body: None }),
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
    let q = Queue::at(&w.0.join("queue.jsonl"));
    let item = q
        .add(&Roster::load(&w.0), Some("slime-s"), "orc-o", "x", 0)
        .unwrap();
    let as_orc = Move::Claim {
        as_body: Some("orc-p".into()),
    };
    assert!(q.move_item(&r, "orc-o", &item.id, &as_orc).is_err());
    assert!(
        q.move_item(&r, "human", &item.id, &as_orc).is_ok(),
        "the human gives work to any body"
    );
}

/// Ties of rank run in order of arrival, whatever their ids.
#[test]
fn ties_run_in_order_of_arrival() {
    let w = World::new("arrival");
    let r = Roster::load(&w.0);
    let q = Queue::at(&w.0.join("queue.jsonl"));
    let ids: Vec<String> = (0..6)
        .map(|n| q.add(&r, None, "human", &format!("job {n}"), 0).unwrap().id)
        .collect();
    let late = q.add(&r, None, "human", "urgent", -1).unwrap().id;
    let open: Vec<String> = q.open().into_iter().map(|i| i.id).collect();
    assert_eq!(open[0], late);
    assert_eq!(open[1..], ids[..]);
}

/// Orders go down the ranks: a body orders only bodies below it; the human orders anyone.
#[test]
fn orders_go_down_the_ranks() {
    let w = World::new("down");
    let r = Roster::load(&w.0);
    let q = Queue::at(&w.0.join("queue.jsonl"));
    assert!(
        q.add(&r, Some("slime-s"), "elf-e", "x", 0).is_ok(),
        "an elf over its slime"
    );
    assert_eq!(
        q.add(&r, Some("elf-e"), "slime-s", "x", 0),
        Err(Refusal::Upward {
            by: "slime-s".into(),
            to: "elf-e".into()
        })
    );
    assert!(
        q.add(&r, Some("orc-p"), "orc-o", "x", 0).is_err(),
        "a sibling"
    );
    assert!(
        q.add(&r, None, "orc-o", "x", 0).is_err(),
        "the throne's work is the human's to add"
    );
    assert!(q.add(&r, Some("elf-e"), "human", "x", 0).is_ok());
}

/// No creature takes the human's or the harness's name: both stand over every body.
#[test]
fn the_human_and_the_harness_are_reserved_names() {
    let w = World::new("reserved");
    w.doc(
        "orc",
        "h",
        "# human\n\n- **Race:** orc\n- **Reports to:** elf-e\n",
    );
    let r = Roster::load(&w.0);
    assert!(r.living().all(|c| c.name != "human"));
    assert!(
        r.problems.iter().any(|p| p.contains("`human` is reserved")),
        "{:?}",
        r.problems
    );
}

/// A link in the queue's place is never written through.
#[test]
fn the_queue_is_not_written_through_a_link() {
    let w = World::new("link");
    let r = Roster::load(&w.0);
    let target = w.0.join("elsewhere.jsonl");
    std::os::unix::fs::symlink(&target, w.0.join("queue.jsonl")).unwrap();
    let q = Queue::at(&w.0.join("queue.jsonl"));
    assert!(matches!(
        q.add(&r, None, "human", "x", 0),
        Err(Refusal::Io(_))
    ));
    assert!(!target.exists());
}

/// A body never closes or drops its own order; a body above it may.
#[test]
fn a_body_does_not_close_its_own_order() {
    let w = World::new("own-close");
    let r = Roster::load(&w.0);
    let q = Queue::at(&w.0.join("queue.jsonl"));
    let item = q.add(&r, Some("slime-s"), "orc-o", "x", 0).unwrap();
    for drop in [false, true] {
        assert_eq!(
            q.move_item(&r, "slime-s", &item.id, &Move::Close { drop }),
            Err(Refusal::NotYours {
                holder: "slime-s".into()
            })
        );
    }
    assert!(
        q.move_item(
            &r,
            "slime-s",
            &item.id,
            &Move::Touch {
                note: "seen".into()
            }
        )
        .is_ok()
    );
    assert!(
        q.move_item(&r, "orc-o", &item.id, &Move::Close { drop: true })
            .is_ok()
    );
}
