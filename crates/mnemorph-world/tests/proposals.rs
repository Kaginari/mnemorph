//! Evolution proposals: a plan drawn is kept under the state directory with the bytes staged and
//! the world it was drawn against; it applies only while both still hold, and closes once
//! applied or rejected.

use std::path::PathBuf;

use mnemorph_world::evolution::{Arm, Staged, staging};
use mnemorph_world::proposals;

const DOC: &str = ".mnemorph/slime/sly/README.md";
const CARD: &str = "# sly\n\n- **Race:** slime\n\nKeeps the parser.\n";
const NEW: &str =
    "# sly\n\n- **Race:** slime\n\nKeeps the parser.\n\n## Traits\n\n- *trait* — exact\n";

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl World {
    /// A world of sly, its plan `sly-1` staged and drawn.
    fn drawn() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("world");
        std::fs::create_dir_all(root.join(".mnemorph/slime/sly")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join(DOC), CARD).unwrap();
        let at = staging(&root, "sly-1").join(DOC);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, NEW).unwrap();
        proposals::draw(
            &root,
            "sly-1",
            "sly",
            Arm::Mutate,
            &[Staged {
                path: DOC.into(),
                after: Some(NEW.as_bytes().to_vec()),
                creates: false,
            }],
        )
        .unwrap();
        World { _dir: dir, root }
    }
}

#[test]
fn a_drawn_plan_is_listed_shown_and_ready_as_drawn() {
    let w = World::drawn();
    let open = proposals::open(&w.root).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].creature, "sly");
    assert_eq!(open[0].file_set().replaces, vec![DOC.to_owned()]);
    let lines = proposals::lines(&w.root, &open);
    assert!(
        lines[0].starts_with("sly-1 — sly (mutate), 1 file(s): ready"),
        "{lines:?}"
    );
    let files = proposals::ready(&w.root, &open[0]).unwrap();
    assert_eq!(files[0].after.as_deref(), Some(NEW.as_bytes()));
    let shown = proposals::show(&w.root, &open[0]).unwrap();
    assert!(shown.contains(&"  + ## Traits".to_owned()), "{shown:?}");
    assert!(
        shown.contains(&"  + - *trait* — exact".to_owned()),
        "{shown:?}"
    );
    assert!(
        !shown.iter().any(|l| l.contains("Keeps the parser")),
        "the lines both hold are left unsaid: {shown:?}"
    );
    assert!(
        proposals::draw(&w.root, "sly-1", "sly", Arm::Mutate, &files)
            .unwrap_err()
            .contains("already a proposal")
    );
}

#[test]
fn a_proposal_the_world_moved_past_or_whose_staging_changed_is_refused() {
    let w = World::drawn();
    let p = proposals::get(&w.root, "sly-1").unwrap();
    std::fs::write(w.root.join(DOC), format!("{CARD}edited\n")).unwrap();
    let e = proposals::ready(&w.root, &p).unwrap_err();
    assert!(e.contains("the world moved since `sly-1` was drawn"), "{e}");
    std::fs::write(w.root.join(DOC), CARD).unwrap();
    assert!(proposals::ready(&w.root, &p).is_ok());
    // A body rewrote the staging after it was drawn.
    std::fs::write(
        staging(&w.root, "sly-1").join(DOC),
        "# sly\n\n- **Race:** orc\n",
    )
    .unwrap();
    let e = proposals::ready(&w.root, &p).unwrap_err();
    assert!(e.contains("not the bytes `sly-1` was drawn with"), "{e}");
}

#[test]
fn a_proposal_closes_once_applied_or_rejected() {
    let w = World::drawn();
    proposals::close(&w.root, "sly-1", "applied").unwrap();
    assert_eq!(proposals::open(&w.root).unwrap(), Vec::new());
    assert!(
        staging(&w.root, "sly-1").exists(),
        "an applied plan's staging is the swap's"
    );
    assert!(
        proposals::get(&w.root, "sly-1")
            .unwrap_err()
            .contains("no open proposal")
    );
    let w = World::drawn();
    proposals::close(&w.root, "sly-1", "rejected").unwrap();
    assert_eq!(proposals::open(&w.root).unwrap(), Vec::new());
    assert!(
        !staging(&w.root, "sly-1").exists(),
        "a rejected plan's staging goes"
    );
}

#[cfg(unix)]
#[test]
fn rejecting_follows_no_link_out_of_the_world() {
    let w = World::drawn();
    let dir = staging(&w.root, "sly-1");
    std::fs::remove_dir_all(&dir).unwrap();
    let elsewhere = w.root.join("../elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("keep"), "mine").unwrap();
    std::os::unix::fs::symlink(&elsewhere, &dir).unwrap();
    proposals::close(&w.root, "sly-1", "rejected").unwrap();
    assert!(
        elsewhere.join("keep").exists(),
        "nothing removed through the link"
    );
}

#[test]
fn the_proposals_live_outside_the_world_chained_and_anchored() {
    let w = World::drawn();
    let chained = mnemorph_world::chains::journals(&w.root);
    let j = chained
        .iter()
        .find(|j| j.name == "proposals")
        .expect("anchored with the others");
    assert!(!j.path.starts_with(&w.root));
    assert!(j.path.exists());
    // A row edited by hand breaks the chain: no proposal is read from it.
    let text = std::fs::read_to_string(&j.path).unwrap();
    std::fs::write(&j.path, text.replace("\"sly\"", "\"orc\"")).unwrap();
    let e = proposals::open(&w.root).unwrap_err();
    assert!(e.contains("do not verify"), "{e}");
}
