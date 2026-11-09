//! A staged plan proven before it is swapped: written over a scratch copy of the world — the
//! overlay — where the ontology must find nothing the live world does not, and the creature's
//! pinned verify lines must pass in the sandbox. The live world is never touched.

use std::path::PathBuf;

use mnemorph_loop::prove::{self, Prover};
use mnemorph_tools::Sandbox;
use mnemorph_world::evolution::{Arm, Plan, Staged};
use mnemorph_world::pins;

const DOC: &str = ".mnemorph/slime/sly/README.md";
const HEAD: &str =
    "# sly\n\n- **Race:** slime\n- **Reports to:** summoner\n- **Territory:** src/\n";

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl World {
    /// Summoner and a Slime `sly` over `src/`, its verify line `verify` pinned by the human.
    fn new(verify: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("world");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("src/lib.rs"), "fn f() {}\n").unwrap();
        for (seat, card) in [
            (
                "summoner/summoner",
                "# summoner\n\n- **Race:** summoner\n".to_owned(),
            ),
            (
                "slime/sly",
                format!("{HEAD}- **Verify:** `{verify}`\n\nKeeps the parser.\n"),
            ),
        ] {
            let at = root.join(".mnemorph").join(seat);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("README.md"), card).unwrap();
        }
        pins::record(
            &root,
            pins::row(pins::Act::Pin, "sly", &[verify.to_owned()], pins::HUMAN),
        )
        .unwrap();
        World { _dir: dir, root }
    }

    fn live(&self) -> String {
        std::fs::read_to_string(self.root.join(DOC)).unwrap()
    }
}

fn plan(files: Vec<(&str, Option<String>, bool)>) -> Plan {
    Plan {
        id: "sly-mutate-1".into(),
        creature: "sly".into(),
        bodies: vec!["sly".into()],
        arm: Arm::Mutate,
        files: files
            .into_iter()
            .map(|(path, after, creates)| Staged {
                path: path.to_owned(),
                after: after.map(String::into_bytes),
                creates,
            })
            .collect(),
        proven_on: Vec::new(),
    }
}

fn prover() -> Prover {
    Prover {
        sandbox: Sandbox::None("the proof's judgement is under test".into()),
        unguarded: true,
        verify_secs: 30,
    }
}

fn doc(verify: &str, body: &str) -> String {
    format!("{HEAD}- **Verify:** `{verify}`\n\n{body}")
}

#[test]
fn a_plan_whose_verify_lines_pass_over_the_overlay_and_adds_no_finding_is_proven() {
    let verify = "grep -q '^- [*]trait' .mnemorph/slime/sly/README.md";
    let w = World::new(verify);
    let before = w.live();
    let p = plan(vec![(
        DOC,
        Some(doc(
            verify,
            "Keeps the parser.\n\n## Traits\n\n- *trait* — exact\n",
        )),
        false,
    )]);
    // The line fails on the live doc, which holds no trait: it is the overlay it ran over.
    let lines = prove::prove(&w.root, &p, &[], &prover()).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        lines.iter().any(|l| l.contains("no new finding")),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("sly") && l.contains(verify) && l.contains("passed")),
        "{lines:?}"
    );
    assert_eq!(w.live(), before, "the live world is never touched");
}

#[test]
fn a_verify_line_that_fails_over_the_overlay_refuses_the_plan() {
    let verify = "grep -qx 'Keeps the parser.' .mnemorph/slime/sly/README.md";
    let w = World::new(verify);
    let p = plan(vec![(DOC, Some(doc(verify, "Keeps the lexer.\n")), false)]);
    let why = prove::prove(&w.root, &p, &[], &prover()).unwrap_err();
    assert!(
        why.iter()
            .any(|y| y.contains(verify) && y.contains("failed")),
        "{why:?}"
    );
}

#[test]
fn a_plan_that_adds_a_finding_over_the_overlay_is_refused() {
    let w = World::new("true");
    // A second Slime over sly's own ground: an overlap the live world does not have.
    let p = plan(vec![(
        ".mnemorph/slime/twin/README.md",
        Some(
            "# twin\n\n- **Race:** slime\n- **Reports to:** summoner\n- **Territory:** src/\n"
                .into(),
        ),
        true,
    )]);
    let why = prove::prove(&w.root, &p, &[], &prover()).unwrap_err();
    assert!(
        why.iter()
            .any(|y| y.contains("new finding") && y.contains("overlap")),
        "{why:?}"
    );
    assert!(!w.root.join(".mnemorph/slime/twin").exists());
}

#[test]
fn a_verify_line_no_pin_holds_is_never_run_and_refuses_the_plan() {
    let w = World::new("true");
    let marker = w.root.join("../ran");
    let line = format!("touch {}", marker.display());
    let p = plan(vec![(DOC, Some(doc(&line, "Keeps the parser.\n")), false)]);
    let why = prove::prove(&w.root, &p, &[], &prover()).unwrap_err();
    assert!(
        why.iter().any(|y| y.contains("mnemorphd verify pin sly")),
        "{why:?}"
    );
    assert!(!marker.exists(), "a line the card alone says never runs");
}

#[cfg(unix)]
#[test]
fn a_link_on_a_plans_path_is_refused_and_nothing_written_through_it() {
    let w = World::new("true");
    let elsewhere = w.root.join("../elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, w.root.join(".mnemorph/minds")).unwrap();
    let p = plan(vec![(
        ".mnemorph/minds/careful.md",
        Some("# Careful\n".into()),
        true,
    )]);
    let why = prove::prove(&w.root, &p, &[], &prover()).unwrap_err();
    assert!(why.iter().any(|y| y.contains("link")), "{why:?}");
    assert!(std::fs::read_dir(&elsewhere).unwrap().next().is_none());
}
