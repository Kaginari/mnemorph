//! Dispatch, every rule of the flowchart, on this repository's own chain — and every refusal
//! naming the creature the order should go to instead.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_races::Refusal;
use mnemorph_wire::Hole;
use mnemorph_world::{Ask, Hop, Roster, dispatch};

fn repo() -> Roster {
    Roster::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn ask(plan: &[String]) -> Ask<'_> {
    Ask {
        plan,
        slice_wt: 60_000,
        left_wt: 200_000,
    }
}

#[test]
fn one_order_down_the_chain() {
    let r = repo();
    let none: [String; 0] = [];
    let plan = ["orc-law".to_owned()];
    assert_eq!(
        dispatch(&r, "rimuru", "elf-engine", &ask(&none)),
        Ok(Hop::Direct("elf-engine".into()))
    );
    assert_eq!(
        dispatch(&r, "elf-engine", "orc-law", &ask(&plan)),
        Ok(Hop::Direct("orc-law".into()))
    );
    // slime-wire's doc carries its verify line: its Orc may commission it.
    assert_eq!(
        dispatch(&r, "orc-law", "slime-wire", &ask(&none)),
        Ok(Hop::Direct("slime-wire".into()))
    );
}

#[test]
fn every_refusal_names_its_lawful_route() {
    let r = repo();
    let none: [String; 0] = [];
    let refused = |from: &str, to: &str, a: &Ask<'_>| dispatch(&r, from, to, a).unwrap_err();
    let e = refused("rimuru", "slime-wire", &ask(&none));
    assert_eq!(e.why, Some(Refusal::NotOneHop));
    assert_eq!(
        e.next,
        "commission elf-engine, the next rank toward slime-wire"
    );
    // Up or across, the order goes to the dispatcher's own commander.
    let e = refused("slime-wire", "orc-law", &ask(&none));
    assert_eq!(
        (e.why, e.next.as_str()),
        (Some(Refusal::NotOneHop), "report it to orc-law")
    );
    let e = refused("orc-law", "slime-loop", &ask(&none));
    assert_eq!(e.why, Some(Refusal::NotYourLine));
    assert_eq!(e.next, "ask orc-engine, whose chain holds slime-loop");
    let e = refused("elf-engine", "orc-law", &ask(&none));
    assert_eq!(e.why, Some(Refusal::NoPlanItem));
    assert_eq!(e.next, "open a plan item that names orc-law");
    let e = refused(
        "orc-law",
        "slime-wire",
        &Ask {
            plan: &none,
            slice_wt: 300_000,
            left_wt: 200_000,
        },
    );
    assert_eq!(e.why, Some(Refusal::Budget));
    // The wire's own typed hole, its addressee beside it.
    assert_eq!(e.next, "@? budget: need ~300000 wt, have 200000 wt");
    assert_eq!(
        Hole::parse(e.next.strip_prefix("@? ").unwrap()),
        Hole::Budget {
            need: 300_000,
            have: 200_000
        }
    );
    assert_eq!(e.whom.as_deref(), Some("elf-engine"));
    assert!(e.to_string().starts_with("refused: the slice does not fit"));
    // Sideways, Orc to Orc: lawful without lineage.
    assert_eq!(
        dispatch(&r, "orc-law", "orc-engine", &ask(&none)),
        Ok(Hop::Direct("orc-engine".into()))
    );
    let e = refused("orc-law", "slime-nobody", &ask(&none));
    assert_eq!(e.why, None);
    assert!(e.next.contains("Rimuru routes its birth"));
}

struct World(PathBuf);

impl World {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("mnemorph-dispatch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
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

/// A Slime with no verify line — or a blank one — waits for its test; a High Elf reaches another
/// lane's Orc only through that lane's Elf, and a plain Elf not at all; a refusal never names a
/// retired creature; a rank skipped in the docs is said as such.
#[test]
fn verify_first_lanes_retired_and_skipped_ranks() {
    let w = World::new();
    w.doc("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n")
        .doc("high-elf", "h", "# high-elf-h\n\n- **Race:** high-elf\n- **Reports to:** rimuru\n")
        .doc("elf", "e", "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n")
        .doc("elf", "f", "# elf-f\n\n- **Race:** elf\n- **Reports to:** rimuru\n")
        .doc("orc", "o", "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n")
        .doc("orc", "gone", "# orc-gone\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Retired:** yes\n")
        .doc("orc", "skip", "# orc-skip\n\n- **Race:** orc\n- **Reports to:** rimuru\n")
        .doc("slime", "s", "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n")
        .doc("slime", "b", "# slime-b\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Verify:**\n- **Territory:** lib/\n")
        .doc("slime", "orphan", "# slime-orphan\n\n- **Race:** slime\n- **Reports to:** orc-gone\n- **Verify:** `true`\n- **Territory:** etc/\n");
    let r = Roster::load(&w.0);
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    let none: [String; 0] = [];
    let e = dispatch(&r, "orc-o", "slime-s", &ask(&none)).unwrap_err();
    assert_eq!(e.why, Some(Refusal::VerifyFirst));
    assert_eq!(
        e.next,
        "code the verify line on slime-s's doc (.mnemorph/slime/s/README.md)"
    );
    let e = dispatch(&r, "orc-o", "slime-b", &ask(&none)).unwrap_err();
    assert_eq!(
        e.why,
        Some(Refusal::VerifyFirst),
        "a blank verify line is none"
    );
    let plan = ["orc-o".to_owned()];
    assert_eq!(
        dispatch(&r, "high-elf-h", "orc-o", &ask(&plan)),
        Ok(Hop::Via {
            elf: "elf-e".into(),
            target: "orc-o".into()
        })
    );
    // A plain Elf has no such road: another lane's Orc is not its line.
    let e = dispatch(&r, "elf-f", "orc-o", &ask(&plan)).unwrap_err();
    assert_eq!(e.why, Some(Refusal::NotYourLine));
    assert_eq!(
        (e.next.as_str(), e.whom.as_deref()),
        ("ask elf-e, whose chain holds orc-o", Some("elf-e"))
    );
    assert_eq!(
        dispatch(&r, "elf-e", "orc-o", &ask(&plan)),
        Ok(Hop::Direct("orc-o".into()))
    );
    // A retired commander is never the route.
    let e = dispatch(&r, "orc-o", "slime-orphan", &ask(&none)).unwrap_err();
    assert_eq!(e.why, None);
    assert!(
        e.next.starts_with("`orc-gone` is no living creature"),
        "{}",
        e.next
    );
    let e = dispatch(&r, "elf-e", "slime-orphan", &ask(&none)).unwrap_err();
    assert!(
        e.next.starts_with("`orc-gone` is no living creature"),
        "{}",
        e.next
    );
    // An Orc whose doc reports straight to the throne skips its Elf.
    let e = dispatch(&r, "rimuru", "orc-skip", &ask(&none)).unwrap_err();
    assert_eq!(
        e.next,
        "orc-skip reports to rimuru directly, skipping a rank: Rimuru routes its lane"
    );
}
