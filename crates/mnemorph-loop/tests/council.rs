//! The council: strain across a body's runs, warrants at `escalate_at`, a forced escalation
//! after `max_consults`, relief at `relieve_at`; a clean landing clears it.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_loop::council::{Sitting, Thresholds, sit, tally};

fn world(name: &str) -> PathBuf {
    let w = std::env::temp_dir().join(format!("mnemorph-council-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    for (race, short, body) in [
        ("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n"),
        (
            "elf",
            "e",
            "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n",
        ),
        (
            "orc",
            "o",
            "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** src/\n",
        ),
        (
            "slime",
            "s",
            "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/a/, src/b/\n",
        ),
    ] {
        let dir = w.join(".mnemorph").join(race).join(short);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), body).unwrap();
    }
    w
}

fn fail(w: &Path, failed: u64) -> Sitting {
    sit(w, Thresholds::default(), "slime-s", failed, false, true)
}

#[test]
fn strain_warrants_then_escalates_and_a_clean_landing_clears() {
    let w = world("warrants");
    assert_eq!(fail(&w, 1), Sitting::Quiet); // 2
    assert_eq!(
        sit(&w, Thresholds::default(), "slime-s", 3, false, false),
        Sitting::Quiet,
        "an interrupted run adds nothing"
    );
    assert_eq!(tally(&w, "slime-s"), (2, 0));
    assert_eq!(fail(&w, 2), Sitting::Warrant("council: slime-s strain 5 — answer as its manager: @INSTRUCT slime-s: <how>, @SPLIT slime-s, or @ESCALATE".into()));
    // A clean landing clears strain and warrants alike.
    assert_eq!(
        sit(&w, Thresholds::default(), "slime-s", 0, true, true),
        Sitting::Quiet
    );
    assert_eq!(tally(&w, "slime-s"), (0, 0));
    // Three warrants: the third forces the escalation.
    let keys = Thresholds {
        escalate_at: 1,
        relieve_at: 99,
        max_consults: 3,
    };
    let s = |w: &Path| sit(w, keys, "slime-s", 0, false, true);
    assert!(matches!(s(&w), Sitting::Warrant(_)));
    assert!(matches!(s(&w), Sitting::Warrant(_)));
    assert!(matches!(s(&w), Sitting::Escalate(why) if why.contains("after 3 warrants")));
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn relief_acts_at_its_threshold_and_resets_the_trigger() {
    let w = world("relief");
    assert!(matches!(fail(&w, 3), Sitting::Quiet)); // 4
    assert!(matches!(fail(&w, 0), Sitting::Warrant(_))); // 5
    let relieved = fail(&w, 1); // 7
    assert!(
        matches!(&relieved, Sitting::Relieved(line) if line.contains("slime-s-a takes src/a/")),
        "{relieved:?}"
    );
    assert_eq!(
        tally(&w, "slime-s"),
        (0, 0),
        "every relief resets its own trigger"
    );
    let roster = mnemorph_loop::world::Roster::load(&w);
    assert!(roster.creatures["slime-s"].retired);
    let _ = fs::remove_dir_all(&w);
}

/// A DONE clears, whatever failed on the way; a jump past relief with no warrant served is a
/// warrant first; a relief that cannot act escalates and keeps the tally.
#[test]
fn the_council_asks_before_it_relieves_and_keeps_what_relief_could_not_clear() {
    let w = world("guards");
    assert!(matches!(fail(&w, 2), Sitting::Quiet));
    assert_eq!(
        sit(&w, Thresholds::default(), "slime-s", 4, true, true),
        Sitting::Quiet
    );
    assert_eq!(
        tally(&w, "slime-s"),
        (0, 0),
        "a DONE clears, red-green on the way or not"
    );
    assert!(
        matches!(fail(&w, 9), Sitting::Warrant(_)),
        "relief waits for a warrant"
    );
    assert!(matches!(fail(&w, 0), Sitting::Relieved(_)));
    // An Elf cannot be relieved: the council escalates, and the strain stays.
    let elf = |w: &Path| sit(w, Thresholds::default(), "elf-e", 4, false, true);
    assert!(matches!(elf(&w), Sitting::Warrant(_)));
    assert!(matches!(elf(&w), Sitting::Escalate(why) if why.contains("relief could not act")));
    assert_eq!(tally(&w, "elf-e").0, 10);
    let _ = fs::remove_dir_all(&w);
}
