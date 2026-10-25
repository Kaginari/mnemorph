//! The shapes the law demands: each breach a finding with its code, severity and place.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_world::{Roster, Severity, check};

struct World(PathBuf);

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-find-{name}-{}", std::process::id()));
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

/// A fixture with an overlap, a loop in the chain, a bond to nobody, a Slime with no Orc and
/// unowned soil reports each with its code, severity and `file:line`.
#[test]
fn every_breach_has_its_code_severity_and_place() {
    let w = World::new("all");
    w.doc(
        "orc",
        "o",
        "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** src/\n",
    )
    .doc(
        "elf",
        "e",
        "# elf-e\n\n- **Race:** elf\n- **Reports to:** orc-o\n- **Territory:** src/\n",
    )
    .doc(
        "slime",
        "a",
        "# slime-a\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n",
    )
    .doc(
        "slime",
        "b",
        "# slime-b\n\n- **Race:** slime\n- **Reports to:** elf-e\n- **Territory:** src/b/\n",
    )
    .doc(
        "slime",
        "c",
        "# slime-c\n\n- **Race:** slime\n- **Reports to:** orc-gone\n- **Territory:** lib/\n",
    );
    let roster = Roster::load(&w.0);
    assert!(roster.problems.is_empty(), "{:?}", roster.problems);
    let tracked = ["src/a.rs", "lib/x.rs", "etc/stray.txt", "etc/other.txt"].map(str::to_owned);
    let found = check(&roster, &tracked);
    let codes: Vec<(&str, Severity, &str)> = found
        .iter()
        .map(|f| (f.code, f.severity, f.at.as_str()))
        .collect();
    assert_eq!(
        codes,
        [
            (
                "above-cycle",
                Severity::Critical,
                ".mnemorph/elf/e/README.md:4"
            ),
            (
                "overlap",
                Severity::Critical,
                ".mnemorph/slime/a/README.md:5"
            ),
            (
                "slime-truth",
                Severity::High,
                ".mnemorph/slime/b/README.md:4"
            ),
            ("dead-bond", Severity::High, ".mnemorph/slime/c/README.md:4"),
            (
                "slime-truth",
                Severity::High,
                ".mnemorph/slime/c/README.md:4"
            ),
            ("unowned-soil", Severity::Medium, "etc/"),
        ],
        "{found:#?}"
    );
    let cycle = &found[0];
    assert_eq!(cycle.creatures, ["elf-e", "orc-o"]);
    assert!(found[1].message.contains("`src/`") && found[1].message.contains("`src/b/`"));
    assert!(found[5].message.starts_with("2 tracked file(s)"));
    assert!(found.iter().take(5).all(|f| f.severity.blocks()));
    assert!(!found[5].severity.blocks());
}

/// An Orc's domain and an Elf's lane nest the zones below them by design: no overlap.
#[test]
fn domains_and_lanes_never_overlap_the_zones_they_hold() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roster = Roster::load(&repo);
    let found = check(&roster, &[]);
    assert!(found.iter().all(|f| !f.severity.blocks()), "{found:#?}");
}

/// Before genesis there is no workspace Slime and no throne: root files, `tools/` and the plan
/// are unowned soil too. A report to a retired parent is a dead bond, and a loop through one is
/// none.
#[test]
fn compiled_soil_without_its_keeper_and_retired_parents() {
    let w = World::new("keeperless");
    w.doc(
        "orc",
        "w",
        "# orc-w\n\n- **Race:** orc\n- **Reports to:** elf-r\n- **Domain:** src/\n",
    )
    .doc(
        "elf",
        "r",
        "# elf-r\n\n- **Race:** elf\n- **Reports to:** elf-q\n- **Retired:** yes\n",
    )
    .doc(
        "elf",
        "q",
        "# elf-q\n\n- **Race:** elf\n- **Reports to:** elf-r\n",
    );
    let roster = Roster::load(&w.0);
    let tracked = [
        "Cargo.toml",
        "README.md",
        "tools/x.sh",
        ".mnemorph/plan.md",
        "src/a.rs",
    ]
    .map(str::to_owned);
    let found = check(&roster, &tracked);
    let codes: Vec<(&str, &str)> = found.iter().map(|f| (f.code, f.at.as_str())).collect();
    assert_eq!(
        codes,
        [
            ("dead-bond", ".mnemorph/elf/q/README.md:4"),
            ("dead-bond", ".mnemorph/orc/w/README.md:4"),
            ("unowned-soil", "./"),
            ("unowned-soil", ".mnemorph/"),
            ("unowned-soil", "tools/"),
        ],
        "{found:#?}"
    );
    assert!(found[1].message.ends_with("which is retired"));
    assert_eq!(found[2].paths, ["Cargo.toml", "README.md"]);
}

/// Prose is never a claim: a Slime's doc that names, in its prose, a path of the world its
/// Territory does not claim raises one low `prose-claim`, at that line. What it claims, what does
/// not exist, words that are no path, the harness's own `.mnemorph/` and its own fields raise none.
#[test]
fn a_path_named_in_prose_and_not_claimed_is_a_prose_claim() {
    let w = std::env::temp_dir().join(format!("mnemorph-prose-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&w);
    for dir in ["src/a", "lib", ".mnemorph/slime/s"] {
        std::fs::create_dir_all(w.join(dir)).unwrap();
    }
    std::fs::write(w.join("lib/util.rs"), "").unwrap();
    std::fs::write(w.join("Cargo.toml"), "").unwrap();
    std::fs::write(
        w.join(".mnemorph/slime/s/README.md"),
        "# slime-s\n\n- **Race:** slime\n- **Territory:** src/\n\n## Notes\n\
         Mine: `src/a/`, `src/a`. Not mine: `lib/util.rs`, and again `lib/util.rs`.\n\
         Gone: `old/x.rs`. A word: `cargo test`, `true`. Memory: `.mnemorph/slime/s/`.\n\
         The workspace's: `./Cargo.toml`.\n",
    )
    .unwrap();
    let roster = mnemorph_world::Roster::load(&w);
    let found = mnemorph_world::prose_claims(&roster, &w);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].code, "prose-claim");
    assert_eq!(found[0].severity, mnemorph_world::Severity::Low);
    assert_eq!(found[0].at, ".mnemorph/slime/s/README.md:7");
    assert_eq!(found[0].paths, ["lib/util.rs"]);
    let _ = std::fs::remove_dir_all(&w);
}

/// `heir-dead`: a retired creature's heir that is missing, or retired with no living line below
/// it. An heir relieved in turn lives on in its own living heirs.
#[test]
fn a_dead_line_of_heirs_is_heir_dead() {
    let w = std::env::temp_dir().join(format!("mnemorph-heirs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&w);
    let doc = |short: &str, body: &str| {
        let dir = w.join(".mnemorph/slime").join(short);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("README.md"), body).unwrap();
    };
    let slime = |name: &str, extra: &str| {
        format!("# {name}\n\n- **Race:** slime\n- **Territory:** src/{name}/\n{extra}")
    };
    // s's line lives through s-a, retired, whose heir s-a-a lives.
    doc(
        "s",
        &slime("slime-s", "- **Retired:** yes\n- **Heirs:** slime-s-a\n"),
    );
    doc(
        "s-a",
        &slime(
            "slime-s-a",
            "- **Retired:** yes\n- **Heirs:** slime-s-a-a\n",
        ),
    );
    doc("s-a-a", &slime("slime-s-a-a", "- **Heir of:** slime-s-a\n"));
    // t's heirs: one retired with none, one with no doc.
    doc(
        "t",
        &slime(
            "slime-t",
            "- **Retired:** yes\n- **Heirs:** slime-t-a, slime-gone\n",
        ),
    );
    doc("t-a", &slime("slime-t-a", "- **Retired:** yes\n"));
    // u split in two: u-a lives, u-b retired with nobody after it. Only u-b's half is unkept.
    doc(
        "u",
        "# slime-u\n\n- **Race:** slime\n- **Territory:** src/u/\n- **Retired:** yes\n- **Heirs:** slime-u-a, slime-u-b\n",
    );
    doc(
        "u-a",
        "# slime-u-a\n\n- **Race:** slime\n- **Territory:** src/u/a/\n",
    );
    doc(
        "u-b",
        "# slime-u-b\n\n- **Race:** slime\n- **Territory:** src/u/b/\n- **Retired:** yes\n",
    );
    // v's dead heir's ground was re-granted by hand to the living w: a stale line, low.
    doc(
        "v",
        "# slime-v\n\n- **Race:** slime\n- **Territory:** src/v/\n- **Retired:** yes\n- **Heirs:** slime-v-a\n",
    );
    doc(
        "v-a",
        "# slime-v-a\n\n- **Race:** slime\n- **Territory:** src/v/\n- **Retired:** yes\n",
    );
    doc(
        "w",
        "# slime-w\n\n- **Race:** slime\n- **Territory:** src/v/\n",
    );
    let roster = Roster::load(&w);
    let all: Vec<_> = check(&roster, &[])
        .into_iter()
        .filter(|f| f.code == "heir-dead")
        .collect();
    let of = |name: &str| -> Vec<_> {
        all.iter()
            .filter(|f| f.creatures == [name])
            .cloned()
            .collect()
    };
    let u = of("slime-u");
    assert_eq!(u.len(), 1, "{u:?}");
    assert_eq!(
        (u[0].severity, u[0].paths.clone()),
        (Severity::High, vec!["src/u/b/".to_owned()])
    );
    let v = of("slime-v");
    assert_eq!(v.len(), 1, "{v:?}");
    assert_eq!((v[0].severity, v[0].paths.len()), (Severity::Low, 0));
    assert!(v[0].message.contains("has a keeper"), "{v:?}");
    let dead = of("slime-t");
    assert_eq!(dead.len(), 2, "{dead:?}");
    assert!(dead.iter().all(|f| f.severity == Severity::High
        && f.creatures == ["slime-t"]
        && f.at == ".mnemorph/slime/t/README.md:6"));
    assert!(
        dead.iter().any(|f| f.paths == ["src/slime-t-a/"]),
        "{dead:?}"
    );
    assert!(dead.iter().any(|f| f.paths == ["src/slime-t/"]), "{dead:?}");
    assert!(
        dead.iter()
            .any(|f| f.message.contains("`slime-gone` has no doc"))
    );
    assert!(
        dead.iter()
            .any(|f| f.message.contains("`slime-t-a` is retired"))
    );
    let _ = std::fs::remove_dir_all(&w);
}

/// `orphan-mind`: a Mind no living creature wears and not marked shared — a directory with a
/// README or a `.md` file. A worn Mind, a shared one, and one worn only by a retired creature
/// (an orphan) are told apart.
#[test]
fn a_mind_nobody_wears_and_not_shared_is_an_orphan() {
    let w = std::env::temp_dir().join(format!("mnemorph-minds-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&w);
    let minds = w.join(".mnemorph/minds");
    std::fs::create_dir_all(minds.join("worn")).unwrap();
    std::fs::create_dir_all(minds.join("shared")).unwrap();
    std::fs::write(minds.join("worn/README.md"), "# worn\n").unwrap();
    std::fs::write(
        minds.join("shared/README.md"),
        "# shared\n\n- **Shared:** yes\n",
    )
    .unwrap();
    std::fs::write(minds.join("lonely.md"), "# lonely\n").unwrap();
    std::fs::write(minds.join("old.md"), "# old\n").unwrap();
    for (short, body) in [
        (
            "s",
            "# slime-s\n\n- **Race:** slime\n- **Territory:** src/\n- **Minds:** `worn`\n",
        ),
        (
            "r",
            "# slime-r\n\n- **Race:** slime\n- **Territory:** lib/\n- **Minds:** old\n- **Retired:** yes\n",
        ),
    ] {
        let dir = w.join(".mnemorph/slime").join(short);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("README.md"), body).unwrap();
    }
    let found = mnemorph_world::mind_findings(&Roster::load(&w), &w);
    let at: Vec<&str> = found.iter().map(|f| f.at.as_str()).collect();
    assert_eq!(
        at,
        [".mnemorph/minds/lonely.md", ".mnemorph/minds/old.md"],
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .all(|f| f.code == "orphan-mind" && f.severity == Severity::Medium)
    );
    let _ = std::fs::remove_dir_all(&w);
}
