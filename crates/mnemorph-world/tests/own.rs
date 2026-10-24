//! Ownership: one function, the first rule that matches — on this repository's own world and on
//! worlds made to break it.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_world::{Owner, Roster, own};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn creature(name: &str) -> Owner {
    Owner::Creature(name.into())
}

#[test]
fn this_repositorys_world_reads_whole() {
    let roster = Roster::load(&repo());
    assert!(roster.problems.is_empty(), "{:?}", roster.problems);
    assert!(roster.creatures.len() >= 20, "{}", roster.creatures.len());
    assert_eq!(roster.loops(), [] as [std::string::String; 0]);
    assert_eq!(
        roster.chain("slime-wire"),
        ["orc-law", "elf-engine", "rimuru"]
    );
    assert!(roster.is_above("rimuru", "slime-wire"));
    assert!(!roster.is_above("slime-wire", "orc-law"));
}

#[test]
fn every_path_of_this_world_has_the_owner_its_docs_say() {
    let roster = Roster::load(&repo());
    let cases = [
        ("crates/mnemorph-wire/src/lib.rs", creature("slime-wire")),
        ("crates/mnemorph-loop/src/run.rs", creature("slime-loop")),
        // No Slime's zone and no Orc's domain covers the config crate: unowned soil, which
        // Rimuru must route.
        ("crates/mnemorph-config/src/lib.rs", Owner::Unowned),
        ("Cargo.toml", creature("slime-workspace")),
        ("tools/check-layers.mjs", creature("slime-workspace")),
        (".github/workflows/ci.yml", creature("kijin-ci")),
        (".mnemorph/slime/wire/README.md", creature("slime-wire")),
        (".mnemorph/shared/queue.jsonl", Owner::Harness),
        (".mnemorph/instruments/usage.jsonl", Owner::Harness),
        (".mnemorph/log.md", Owner::Harness),
        (".mnemorph/law.md", Owner::Human),
        (".mnemorph/dimension.toml", creature("rimuru")),
    ];
    for (path, owner) in cases {
        assert_eq!(own(&roster, path), owner, "{path}");
    }
}

struct World(PathBuf);

impl World {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("mnemorph-own-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        World(root)
    }

    fn doc(&self, race: &str, short: &str, body: &str) -> &Self {
        let dir = self.0.join(".mnemorph").join(race).join(short);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), body).unwrap();
        self
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Lesson: a creature claiming the world directory itself swallowed every seat, the shared
/// queue and scratch. The order is the rule: harness soil, scratch and seats are asked first.
#[test]
fn a_creature_claiming_the_records_owns_none_of_the_seats_the_queue_or_scratch() {
    let w = World::new("greedy");
    w.doc("orc", "boss", "# orc-boss\n\n- **Race:** orc\n- **Reports to:** rimuru\n- **Domain:** src/\n")
        .doc("slime", "greedy", "# slime-greedy\n\n- **Race:** slime\n- **Reports to:** orc-boss\n- **Territory:** .mnemorph/\n")
        .doc("slime", "small", "# slime-small\n\n- **Race:** slime\n- **Reports to:** orc-boss\n- **Territory:** src/small/\n");
    let roster = Roster::load(&w.0);
    assert!(roster.problems.is_empty(), "{:?}", roster.problems);
    assert_eq!(
        own(&roster, ".mnemorph/slime/small/README.md"),
        creature("slime-small")
    );
    assert_eq!(
        own(&roster, ".mnemorph/orc/boss/README.md"),
        creature("orc-boss")
    );
    assert_eq!(own(&roster, ".mnemorph/shared/queue.jsonl"), Owner::Harness);
    assert_eq!(
        own(&roster, ".mnemorph/tmp/slime-small/x"),
        creature("slime-small")
    );
    assert_eq!(own(&roster, ".mnemorph/notes.md"), creature("slime-greedy"));
    assert_eq!(own(&roster, "src/small/a.rs"), creature("slime-small"));
    assert_eq!(own(&roster, "src/other.rs"), creature("orc-boss"));
    assert_eq!(own(&roster, "lib/x.rs"), Owner::Unowned);
}

/// Retired creatures own nothing; a tie on one prefix breaks by name; a loop is found.
#[test]
fn retired_ties_and_loops() {
    let w = World::new("edges");
    w.doc(
        "slime",
        "a",
        "# slime-a\n\n- **Race:** slime\n- **Reports to:** slime-b\n- **Territory:** src/\n",
    )
    .doc(
        "slime",
        "b",
        "# slime-b\n\n- **Race:** slime\n- **Reports to:** slime-a\n- **Territory:** src/\n",
    )
    .doc(
        "slime",
        "old",
        "# slime-old\n\n- **Race:** slime\n- **Territory:** lib/\n- **Retired:** yes\n",
    );
    let roster = Roster::load(&w.0);
    assert_eq!(
        own(&roster, "src/x.rs"),
        creature("slime-a"),
        "a tie breaks by name, the same every time"
    );
    assert_eq!(own(&roster, "lib/x.rs"), Owner::Unowned);
    assert_eq!(
        own(&roster, ".mnemorph/slime/old/README.md"),
        Owner::Unowned
    );
    assert_eq!(roster.loops(), ["slime-a", "slime-b"]);
}

/// A doc that cannot be read is reported, never guessed; an Orc's ground is its Domain.
#[test]
fn a_doc_that_does_not_read_is_reported() {
    let w = World::new("bad");
    w.doc(
        "orc",
        "x",
        "# orc-x\n\n- **Race:** orc\n- **Territory:** src/\n",
    )
    .doc("slime", "y", "no heading\n")
    .doc("slime", "z", "# slime-z\n\n- **Race:** wizard\n");
    let roster = Roster::load(&w.0);
    assert!(roster.creatures.is_empty());
    let problems = roster.problems.join("\n");
    assert!(
        problems.contains("names its ground as Domain"),
        "{problems}"
    );
    assert!(problems.contains("no `# name` heading"), "{problems}");
    assert!(problems.contains("no race `wizard`"), "{problems}");
    let w = World::new("prose");
    w.doc(
        "slime",
        "p",
        "# slime-p\n\n- **Race:** slime\n- **Territory:** every file at the root\n",
    );
    let problems = Roster::load(&w.0).problems.join("\n");
    assert!(
        problems.contains("`every` is not one"),
        "prose is never a claim: {problems}"
    );
}
