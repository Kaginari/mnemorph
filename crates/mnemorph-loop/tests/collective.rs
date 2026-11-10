//! The collective's own rules: who may summon, what a member is told, and the board.

use std::fs;

use mnemorph_loop::Ending;
use mnemorph_loop::collective::{
    BOARD, BOARD_BYTES, card, lawful, members, post, prompt, with_board,
};
use mnemorph_races::Race;

fn world(name: &str) -> std::path::PathBuf {
    let root =
        std::env::temp_dir().join(format!("mnemorph-collective-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    for (race, short, card) in [
        (
            "summoner",
            "summoner",
            "# summoner\n\n- **Race:** summoner\n",
        ),
        (
            "slime",
            "a",
            "# slime-a\n\n- **Race:** slime\n- **Reports to:** summoner\n- **Territory:** src/\n",
        ),
        (
            "dark-elf",
            "d",
            "# dark-elf-d\n\n- **Race:** dark-elf\n- **Reports to:** summoner\n",
        ),
        (
            "slime",
            "r",
            "# slime-r\n\n- **Race:** slime\n- **Reports to:** summoner\n- **Territory:** lib/\n- **Retired:** yes\n",
        ),
    ] {
        let dir = root.join(".mnemorph").join(race).join(short);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), card).unwrap();
    }
    mnemorph_world::creatures::record(&root, mnemorph_world::creatures::adoption(&root)).unwrap();
    root
}

#[test]
fn only_the_summoner_summons_and_only_the_living() {
    let w = world("law");
    let roster = mnemorph_world::Roster::load(&w);
    assert_eq!(lawful(&roster, "summoner", "slime-a"), Ok(()));
    let refused = lawful(&roster, "slime-a", "dark-elf-d").unwrap_err();
    assert!(refused.contains("only the Summoner summons"), "{refused}");
    assert!(lawful(&roster, "summoner", "summoner").is_err());
    assert!(lawful(&roster, "summoner", "slime-nobody").is_err());
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn the_summoner_is_told_its_members_and_how_the_collective_works() {
    let w = world("members");
    let found = members(&mnemorph_world::Roster::load(&w));
    let names: Vec<&str> = found.iter().map(|(n, _, _)| n.as_str()).collect();
    assert!(
        names.contains(&"slime-a") && names.contains(&"dark-elf-d"),
        "{names:?}"
    );
    assert!(!names.contains(&"summoner"), "{names:?}");
    let said = prompt(&found);
    assert!(said.contains("- slime-a (slime; writes src/)"), "{said}");
    assert!(said.contains("- dark-elf-d (dark-elf)"), "{said}");
    assert!(said.contains("Judge, never redo"), "{said}");
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn each_race_reads_its_part_of_the_collective() {
    assert!(card(Race::Slime).contains("No one orders you"));
    assert!(card(Race::DarkElf).contains("the challenge"));
    assert!(card(Race::Kijin).contains("outward"));
    assert!(card(Race::Orc).contains("No one orders you"));
}

#[test]
fn a_brief_carries_the_newest_of_the_board() {
    let w = world("board");
    assert_eq!(with_board(&w, "do it"), "do it");
    fs::create_dir_all(w.join(".mnemorph/tmp")).unwrap();
    fs::write(w.join(BOARD), "  \n").unwrap();
    assert_eq!(with_board(&w, "do it"), "do it");
    // Longer than a brief carries, a multi-byte character at the cut: the newest end, whole.
    let long = format!("{}é## Map\nsrc/x.py:1-9\n", "a".repeat(BOARD_BYTES));
    fs::write(w.join(BOARD), &long).unwrap();
    let told = with_board(&w, "do it");
    assert!(told.ends_with("Your part:\n\ndo it"), "{told}");
    assert!(told.contains("## Map\nsrc/x.py:1-9"), "{told}");
    assert!(told.len() < long.len() + 200, "{}", told.len());
    let _ = fs::remove_dir_all(&w);
}

#[test]
fn a_members_findings_are_posted_under_its_name() {
    let w = world("post");
    post(&w, "slime-a", &Ending::Escalate("stuck".into()));
    post(&w, "slime-a", &Ending::Answered("@S DONE\n@E".into()));
    assert!(!w.join(BOARD).exists());
    post(
        &w,
        "slime-a",
        &Ending::Answered("@S DONE\n@W src/x.py\n @F src/x.py:3 a trap\n@E".into()),
    );
    let board = fs::read_to_string(w.join(BOARD)).unwrap();
    assert_eq!(
        board,
        "\n## Found by slime-a\n@W src/x.py\n@F src/x.py:3 a trap\n"
    );
    let _ = fs::remove_dir_all(&w);
}
