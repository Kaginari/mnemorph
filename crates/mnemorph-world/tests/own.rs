//! Ownership: one function, the first rule that matches — on this repository's own world and on
//! worlds made to break it.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_world::{Owner, Roster, Rule, explain, own, soil};

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
        // No Slime's zone covers these: they fall to the Orc whose domain does.
        ("crates/mnemorph-config/src/lib.rs", creature("orc-engine")),
        ("packages/theme/src/tokens.css", creature("orc-clients")),
        (".cargo/config.toml", creature("slime-workspace")),
        (".github/ISSUE_TEMPLATE/bug.yml", Owner::Unowned),
        ("Cargo.toml", creature("slime-workspace")),
        ("tools/check-layers.mjs", creature("slime-workspace")),
        (".github/workflows/ci.yml", creature("kijin-ci")),
        (".mnemorph/slime/wire/README.md", creature("slime-wire")),
        (".mnemorph/shared/queue.jsonl", Owner::Harness),
        (".mnemorph/instruments/usage.jsonl", Owner::Harness),
        (".mnemorph/log.md", Owner::Harness),
        (".mnemorph/law.md", Owner::Human),
        (".mnemorph/config.toml", Owner::Human),
        (".mnemorph/dimension.toml", creature("rimuru")),
        (".mnemorph/plan.md", creature("rimuru")),
        (".mnemorph/planet.md", Owner::Unowned),
        (".mnemorph/tmp/rimuru/x", creature("rimuru")),
        (".mnemorph/tmp/evolution/7/README.md", Owner::Harness),
        // A path is read segment by segment: `..` never carries a claim out of its zone.
        ("crates/mnemorph-wire/../../.mnemorph/law.md", Owner::Human),
        (
            "crates/mnemorph-wire/../mnemorph-config/src/lib.rs",
            creature("orc-engine"),
        ),
        (
            ".mnemorph/shared/../slime/wire/README.md",
            creature("slime-wire"),
        ),
        ("crates//mnemorph-wire/./src/lib.rs", creature("slime-wire")),
        ("crates/mnemorph-wire/", creature("slime-wire")),
        ("../elsewhere/x.rs", Owner::Unowned),
        ("/.mnemorph/law.md", Owner::Unowned),
        ("", Owner::Unowned),
        (".", Owner::Unowned),
        (".mnemorph", Owner::Unowned),
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

/// The rules between them: a nested zone, a post over a zone and the workspace, an Elf's lane
/// owning nothing, an ascended race's seat, the one proving ground, a header that ends at any
/// heading, and a file claim.
#[test]
fn nested_posts_lanes_ascended_and_files() {
    let w = World::new("rules");
    w.doc("slime", "outer", "# slime-outer\n\n- **Race:** slime\n- **Territory:** src/\n")
        .doc("slime", "inner", "# slime-inner\n\n- **Race:** slime\n- **Territory:** src/inner/\n### Later\n- **Territory:** lib/\n")
        .doc("kijin", "release", "# kijin-release\n\n- **Race:** kijin\n- **Post:** ./CHANGELOG.md src/release/\n")
        .doc("elf", "plan", "# elf-plan\n\n- **Race:** elf\n- **Territory:** docs/\n")
        .doc("high-orc", "big", "# high-orc-big\n\n- **Race:** high-orc\n- **Domain:** docs/\n")
        .doc("dark-elf", "shade", "# dark-elf-shade\n\n- **Race:** dark-elf\n")
        .doc("dark-elf", "abyss", "# dark-elf-abyss\n\n- **Race:** dark-elf\n");
    let roster = Roster::load(&w.0);
    assert!(roster.problems.is_empty(), "{:?}", roster.problems);
    assert_eq!(own(&roster, "src/a.rs"), creature("slime-outer"));
    assert_eq!(
        own(&roster, "src/inner/a.rs"),
        creature("slime-inner"),
        "the longest zone wins"
    );
    assert_eq!(
        own(&roster, "lib/a.rs"),
        Owner::Unowned,
        "a field under a later heading is prose"
    );
    assert_eq!(
        own(&roster, "src/release/x.toml"),
        creature("kijin-release"),
        "a post before a zone"
    );
    assert_eq!(
        own(&roster, "CHANGELOG.md"),
        creature("kijin-release"),
        "a post before the workspace"
    );
    assert_eq!(
        own(&roster, "CHANGELOG.md.bak"),
        Owner::Unowned,
        "a file claim is the file alone"
    );
    assert_eq!(
        own(&roster, "docs/x.md"),
        creature("high-orc-big"),
        "an Elf's lane owns nothing"
    );
    assert_eq!(
        own(&roster, ".mnemorph/high-orc/big/README.md"),
        creature("high-orc-big")
    );
    assert_eq!(
        own(&roster, ".mnemorph/dark-elf/witness/x"),
        creature("dark-elf-abyss")
    );
    // No throne doc: the throne's files are nobody's until it has one.
    assert_eq!(own(&roster, ".mnemorph/plan.md"), Owner::Unowned);
}

/// A claim is a path inside the world, never one that climbs out or starts at the root.
#[test]
fn a_claim_outside_the_world_is_no_claim() {
    for bad in ["../other/", "/abs/", "/", "./", "src/../../x/", "every"] {
        let w = World::new("outside");
        w.doc(
            "slime",
            "x",
            &format!("# slime-x\n\n- **Race:** slime\n- **Territory:** {bad}\n"),
        );
        let roster = Roster::load(&w.0);
        assert!(roster.creatures.is_empty(), "`{bad}` was taken as a claim");
        assert!(
            roster.problems[0].contains("is not one"),
            "{:?}",
            roster.problems
        );
    }
    let w = World::new("unicode");
    w.doc(
        "slime",
        "fr",
        "# slime-fr\n\n- **Race:** slime\n- **Territory:** docs/français/\n",
    );
    assert_eq!(
        own(&Roster::load(&w.0), "docs/français/a.md"),
        creature("slime-fr")
    );
}

/// A doc that cannot be read is reported, and its change moves the fingerprint.
#[test]
fn a_doc_that_cannot_be_read_is_reported_and_fingerprinted() {
    let w = World::new("binary");
    w.doc(
        "slime",
        "ok",
        "# slime-ok\n\n- **Race:** slime\n- **Territory:** src/\n",
    );
    let before = Roster::load(&w.0).fingerprint;
    let doc = w.0.join(".mnemorph/slime/bad/README.md");
    fs::create_dir_all(doc.parent().unwrap()).unwrap();
    fs::write(&doc, [0xff, 0xfe, 0x00]).unwrap();
    let roster = Roster::load(&w.0);
    assert_ne!(roster.fingerprint, before);
    assert!(
        roster
            .problems
            .iter()
            .any(|p| p.contains("the doc cannot be read")),
        "{:?}",
        roster.problems
    );
    fs::write(&doc, [0xff, 0xfe, 0x01]).unwrap();
    assert_ne!(Roster::load(&w.0).fingerprint, roster.fingerprint);
}

/// The soil handed to a creature claiming the records gives away what the earlier rules give
/// away: it agrees with `own` on every place the rules are about.
#[test]
fn a_greedy_creatures_soil_leaves_the_seats_the_queue_and_scratch() {
    let w = World::new("greedy-soil");
    w.doc(
        "orc",
        "boss",
        "# orc-boss\n\n- **Race:** orc\n- **Domain:** src/\n",
    )
    .doc(
        "slime",
        "greedy",
        "# slime-greedy\n\n- **Race:** slime\n- **Territory:** .mnemorph/ src/\n",
    )
    .doc(
        "slime",
        "small",
        "# slime-small\n\n- **Race:** slime\n- **Territory:** src/small/\n",
    )
    .doc(
        "kijin",
        "release",
        "# kijin-release\n\n- **Race:** kijin\n- **Post:** ./CHANGELOG.md src/small/release/\n",
    )
    .doc(
        "slime",
        "workspace",
        "# slime-workspace\n\n- **Race:** slime\n- **Territory:** tools/\n",
    );
    let roster = Roster::load(&w.0);
    let greedy = soil(&roster, "slime-greedy").unwrap();
    for p in [
        ".mnemorph/shared/queue.jsonl",
        ".mnemorph/slime/small/README.md",
        ".mnemorph/tmp/slime-small/x",
        ".mnemorph/law.md",
    ] {
        assert!(!greedy.admits(p), "{p}");
    }
    assert!(greedy.admits(".mnemorph/notes.md"));
    assert!(greedy.admits(".mnemorph/tmp/slime-greedy/x"));
    let paths = [
        ".mnemorph/notes.md",
        ".mnemorph/shared/q",
        ".mnemorph/slime/small/README.md",
        ".mnemorph/slime/greedy/README.md",
        ".mnemorph/tmp/slime-small/x",
        ".mnemorph/tmp/slime-greedy/x",
        ".mnemorph/tmp/other/x",
        ".mnemorph/log.md",
        ".mnemorph/plan.md",
        "src/a.rs",
        "src/small/a.rs",
        "src/small/release/r.toml",
        "CHANGELOG.md",
        "README.md",
        "tools/x.mjs",
        ".mnemorph",
    ];
    for c in roster.living() {
        let s = soil(&roster, &c.name).unwrap();
        for p in paths {
            let e = explain(&roster, p);
            let writes = e.owner == Owner::Creature(c.name.clone()) && e.rule != Rule::Domain;
            assert_eq!(s.admits(p), writes, "{} on {p}: {e:?}, soil {s:?}", c.name);
        }
    }
}
