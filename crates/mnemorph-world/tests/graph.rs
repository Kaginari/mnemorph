//! The closed graph and each body's soil, on this repository's own world: the soil a body is
//! handed and the owner `own` names never disagree.

use std::path::{Path, PathBuf};
use std::process::Command;

use mnemorph_world::{Bond, Graph, Owner, Roster, Rule, explain, own, soil};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_graph_closes_to_its_fixed_point() {
    let roster = Roster::load(&repo());
    let g = Graph::of(&roster);
    // Asserted: the Orc above its Slime, by the truth sub-bond.
    assert!(g.has("orc-law", Bond::Truth, "slime-wire"));
    // Inferred: the sub-bond is `above`; `above` is transitive; `below` is its inverse.
    assert!(g.has("orc-law", Bond::Above, "slime-wire"));
    assert!(g.has("elf-engine", Bond::Verdict, "orc-law"));
    assert!(g.has("rimuru", Bond::Above, "slime-wire"));
    assert!(g.has("slime-wire", Bond::Below, "rimuru"));
    assert!(g.has("rimuru", Bond::Reports, "elf-engine"));
    // Classes: a creature is of its race, and a race is a creature.
    assert!(g.has("slime-wire", Bond::IsA, "slime"));
    assert!(g.has("slime-wire", Bond::IsA, "Creature"));
    assert!(g.has("high-orc", Bond::IsA, "Creature"));
    assert!(g.has("slime-wire", Bond::Owns, "crates/mnemorph-wire/"));
    assert!(g.has("orc-engine", Bond::Routes, "crates/mnemorph-config/"));
    assert!(g.has("kijin-ci", Bond::Posts, ".github/workflows/"));
    assert!(!g.has("slime-wire", Bond::Above, "orc-law"));
    // Closing again adds nothing: it is a fixed point.
    assert_eq!(Graph::of(&roster), g);
}

#[test]
fn explain_names_the_rule_and_the_claim() {
    let roster = Roster::load(&repo());
    let e = explain(&roster, "crates/mnemorph-wire/src/lib.rs");
    assert_eq!(
        (e.owner, e.rule, e.claim.as_deref()),
        (
            Owner::Creature("slime-wire".into()),
            Rule::Zone,
            Some("crates/mnemorph-wire/")
        )
    );
    let e = explain(&roster, "crates/mnemorph-config/src/lib.rs");
    assert_eq!(
        (e.rule, e.claim.as_deref()),
        (Rule::Domain, Some("crates/mnemorph-config/"))
    );
    assert_eq!(explain(&roster, "Cargo.toml").rule, Rule::Workspace);
    assert_eq!(explain(&roster, "../x").rule, Rule::Outside);
    assert_eq!(
        explain(&roster, ".github/ISSUE_TEMPLATE/bug.yml").rule,
        Rule::Unclaimed
    );
}

/// Every tracked file, and the places the rules are about: for every living creature, its soil
/// admits a path exactly when `own` names it — by any rule but a domain, which routes and never
/// writes.
#[test]
fn every_soil_admits_exactly_what_own_gives_it_to_write() {
    let roster = Roster::load(&repo());
    let out = Command::new("git")
        .arg("-C")
        .arg(repo())
        .args(["ls-files"])
        .output()
        .unwrap();
    let mut paths: Vec<String> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(paths.len() > 100, "{}", paths.len());
    paths.extend(
        [
            ".mnemorph/tmp/slime-wire/x",
            ".mnemorph/tmp/rimuru/x",
            ".mnemorph/tmp/evolution/1/x",
            ".mnemorph/shared/queue.jsonl",
            ".mnemorph/plan.md",
            ".mnemorph/config.local.toml",
            ".mnemorph/dark-elf/witness/x",
            ".mnemorph",
            "tools/new.mjs",
            "NEW.md",
        ]
        .map(str::to_owned),
    );
    for c in roster.living() {
        let s = soil(&roster, &c.name).unwrap();
        for p in &paths {
            let e = explain(&roster, p);
            let writes = e.owner == Owner::Creature(c.name.clone()) && e.rule != Rule::Domain;
            assert_eq!(s.admits(p), writes, "{} on {p}: {e:?}, soil {s:?}", c.name);
        }
    }
    assert_eq!(
        own(&roster, "packages/cli/src/app.tsx"),
        Owner::Creature("slime-cli".into())
    );
}
