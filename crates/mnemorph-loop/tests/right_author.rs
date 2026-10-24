//! The end gate's right author: a commissioned creature lands only on its own soil, and a
//! critical or high finding blocks a landing it touches — not one that removes it.

use std::fs;
use std::path::PathBuf;

use mnemorph_loop::commission::context_for;
use mnemorph_loop::world::Roster;
use mnemorph_loop::{Author, EndGate, snapshot};
use mnemorph_tools::{Sandbox, registry};

struct World(PathBuf);

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-author-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for dir in ["src", "lib"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        let w = World(root);
        w.doc(
            "orc",
            "o",
            "# orc-o\n\n- **Race:** orc\n- **Domain:** src/ lib/\n",
        );
        w.doc(
            "slime",
            "a",
            "# slime-a\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n",
        );
        w.doc(
            "slime",
            "b",
            "# slime-b\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** lib/\n",
        );
        w
    }

    fn doc(&self, race: &str, short: &str, body: &str) {
        let dir = self.0.join(".mnemorph").join(race).join(short);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), body).unwrap();
    }

    /// Judge what changed since `before`, as slime-a's landing.
    fn judge(&self, before: &mnemorph_loop::Tree, roster: &Roster) -> Vec<String> {
        let gate = EndGate {
            verify: Vec::new(),
            author: Some(Author {
                name: "slime-a".into(),
                roster: roster.clone(),
            }),
        };
        let mut cx = context_for(&self.0, roster, "slime-a")
            .unwrap()
            .with_sandbox(Sandbox::None("the gate's judgement is under test".into()));
        gate.judge(before, "done", &registry(), &mut cx).failures
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_creature_lands_only_on_its_own_soil() {
    let w = World::new("soil");
    let roster = Roster::load(&w.0);
    let before = snapshot(&w.0);
    fs::write(w.0.join("src/mine.rs"), "mine\n").unwrap();
    assert_eq!(w.judge(&before, &roster), Vec::<String>::new());
    fs::write(w.0.join("lib/theirs.rs"), "theirs\n").unwrap();
    fs::write(w.0.join("NOTES.md"), "stray\n").unwrap();
    let failures = w.judge(&before, &roster);
    assert_eq!(
        failures,
        [
            "right author: `NOTES.md` is nobody's (the workspace NOTES.md), not slime-a's — Rimuru routes its birth",
            "right author: `lib/theirs.rs` is slime-b's (zone lib/), not slime-a's — ask slime-b",
        ]
    );
}

#[test]
fn a_finding_blocks_the_landing_it_touches_not_the_one_that_ends_it() {
    let w = World::new("finding");
    // slime-c claims inside slime-a's zone: a critical overlap on src/.
    w.doc(
        "slime",
        "c",
        "# slime-c\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/c/\n",
    );
    let roster = Roster::load(&w.0);
    let before = snapshot(&w.0);
    fs::write(w.0.join("src/mine.rs"), "mine\n").unwrap();
    let failures = w.judge(&before, &roster);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with("right author: a critical `overlap` finding touches this landing"),
        "{failures:?}"
    );
    // The landing that narrows the overlap away is judged on the tree it leaves.
    w.doc(
        "slime",
        "c",
        "# slime-c\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** etc/\n",
    );
    assert_eq!(w.judge(&before, &roster), Vec::<String>::new());
}
