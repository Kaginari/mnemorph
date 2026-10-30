//! The evolution writer: a swap writes one stamped swap row and keeps the bytes before; runs in
//! its window write run rows; a plan whose diff is not exactly its files writes nothing.

use std::fs;
use std::path::PathBuf;

use mnemorph_ledger::evolution::{self, EVOLUTION_WINDOW, EvolutionRow, Gate};
use mnemorph_ledger::{Build, Journal};
use mnemorph_world::evolution::{
    Arm, Facts, Plan, Staged, fingerprint, kept_before, restore_before, stamp_for, swap_at,
};

const MIND: &str = ".mnemorph/minds/careful/README.md";

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "mnemorph-world-evolution-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let mind = root.join("world").join(MIND);
        fs::create_dir_all(mind.parent().unwrap()).unwrap();
        fs::write(&mind, "# Careful\n\nRead before writing.\n").unwrap();
        let log = root.join("world/.mnemorph/instruments/loop/r.jsonl");
        fs::create_dir_all(log.parent().unwrap()).unwrap();
        fs::write(log, "{}\n").unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn ledger(&self) -> Journal<EvolutionRow> {
        Journal::chained(self.root.join("state/evolution.jsonl"))
    }

    fn rows(&self) -> Vec<EvolutionRow> {
        self.ledger().read().map(|r| r.rows).unwrap_or_default()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn plan(id: &str, files: Vec<Staged>) -> Plan {
    Plan {
        id: id.into(),
        creature: "rimuru".into(),
        bodies: vec!["rimuru".into()],
        arm: Arm::Mutate,
        files,
        proven_on: vec!["staged-1".into()],
    }
}

fn staged(path: &str, after: Option<&str>) -> Staged {
    Staged {
        path: path.into(),
        after: after.map(|a| a.as_bytes().to_vec()),
    }
}

#[test]
fn a_swap_writes_one_stamped_row_naming_every_file_before_and_after() {
    let w = World::new("swap");
    let old = fs::read(w.world().join(MIND)).unwrap();
    let new = "# Careful\n\nRead before writing. Write once.\n";
    let added = ".mnemorph/minds/careful/checklist.md";
    let s = swap_at(
        &w.world(),
        &w.ledger(),
        &plan(
            "careful-1",
            vec![staged(MIND, Some(new)), staged(added, Some("- read\n"))],
        ),
    )
    .unwrap();
    assert_eq!(fs::read_to_string(w.world().join(MIND)).unwrap(), new);
    let rows = w.rows();
    assert_eq!(rows, vec![EvolutionRow::Swap(s.clone())]);
    assert_eq!(s.build, Build::this(), "stamped with the build");
    let mind = s.files.iter().find(|f| f.path == MIND).unwrap();
    assert_eq!(mind.before, Some(evolution::digest(&old)));
    assert_eq!(mind.after, Some(evolution::digest(new.as_bytes())));
    let created = s.files.iter().find(|f| f.path == added).unwrap();
    assert_eq!(created.before, None, "a created file had no digest before");
    assert_eq!(s.diff, {
        let mut f = s.files.clone();
        f.sort();
        f
    });
    assert!(s.check().is_ok());
    // The bytes before are kept beside the ledger, outside the world, for the controls.
    let kept = kept_before(w.ledger().path(), "careful-1");
    assert!(!kept.starts_with(w.world()));
    assert_eq!(fs::read(kept.join(MIND)).unwrap(), old);
    let elsewhere = w.root.join("copy");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(elsewhere.join("x"), "x").unwrap();
    let copy_mind = elsewhere.join(MIND);
    fs::create_dir_all(copy_mind.parent().unwrap()).unwrap();
    fs::write(&copy_mind, new).unwrap();
    fs::write(elsewhere.join(added), "- read\n").unwrap();
    restore_before(w.ledger().path(), &s, &elsewhere).unwrap();
    assert_eq!(fs::read(&copy_mind).unwrap(), old);
    assert!(!elsewhere.join(added).exists(), "a created file is removed");
    assert_eq!(
        evolution::state_now(&elsewhere, &s).unwrap(),
        s.before_state()
    );
}

#[test]
fn a_plan_is_refused_before_anything_is_written() {
    let w = World::new("refused");
    let world = w.world();
    let before = fs::read(world.join(MIND)).unwrap();
    for (p, why) in [
        (plan("../up", vec![staged(MIND, Some("x"))]), "swap id"),
        (plan("p", Vec::new()), "names no file"),
        (
            plan("p", vec![staged("src/lib.rs", Some("x"))]),
            "creature docs and Minds",
        ),
        (
            plan("p", vec![staged(".mnemorph/instruments/x", Some("x"))]),
            "instruments",
        ),
        (
            plan("p", vec![staged(".mnemorph/minds/../../x", Some("x"))]),
            "creature docs and Minds",
        ),
        (
            plan("p", vec![staged(MIND, Some("a")), staged(MIND, Some("b"))]),
            "named twice",
        ),
    ] {
        let e = swap_at(&world, &w.ledger(), &p).unwrap_err();
        assert!(e.contains(why), "{why}: {e}");
    }
    // The same bytes before and after: the soil shows no change, so the named file is missing.
    let same = String::from_utf8(before.clone()).unwrap();
    let e = swap_at(
        &world,
        &w.ledger(),
        &plan("same", vec![staged(MIND, Some(&same))]),
    )
    .unwrap_err();
    assert!(e.contains("the same before and after"), "{e}");
    assert_eq!(fs::read(world.join(MIND)).unwrap(), before);
    assert!(w.rows().is_empty(), "a refused plan writes no row");
    assert!(!kept_before(w.ledger().path(), "same").exists());
}

/// A swap whose files are already being judged, or whose id is taken, is refused.
#[test]
fn one_evolution_at_a_time() {
    let w = World::new("once");
    swap_at(
        &w.world(),
        &w.ledger(),
        &plan("a", vec![staged(MIND, Some("one\n"))]),
    )
    .unwrap();
    let e = swap_at(
        &w.world(),
        &w.ledger(),
        &plan("a", vec![staged(MIND, Some("two\n"))]),
    )
    .unwrap_err();
    assert!(e.contains("already holds"), "{e}");
    let e = swap_at(
        &w.world(),
        &w.ledger(),
        &plan("b", vec![staged(MIND, Some("two\n"))]),
    )
    .unwrap_err();
    assert!(e.contains("one evolution at a time"), "{e}");
}

fn run(w: &World, ask: &str, gate: Gate) {
    let stamp = stamp_for(&w.world(), w.ledger().path(), "rimuru", ask);
    if let Some(s) = stamp {
        s.record(
            &w.world(),
            Facts {
                gate: Some(gate),
                first_try: gate == Gate::Pass,
                holes_found: 0,
            },
        )
        .unwrap();
    }
}

#[test]
fn runs_after_the_swap_write_stamped_run_rows_until_the_window_is_full() {
    let w = World::new("runs");
    assert!(
        stamp_for(&w.world(), w.ledger().path(), "rimuru", "before any swap").is_none(),
        "no swap, no stamp"
    );
    let s = swap_at(
        &w.world(),
        &w.ledger(),
        &plan("c", vec![staged(MIND, Some("new\n"))]),
    )
    .unwrap();
    assert!(stamp_for(&w.world(), w.ledger().path(), "another", "x").is_none());
    for i in 0..EVOLUTION_WINDOW + 3 {
        run(&w, &format!("task {i}"), Gate::Pass);
    }
    // A task already on the ledger is never stamped again.
    run(&w, "task 0", Gate::Fail);
    let runs: Vec<_> = w
        .rows()
        .into_iter()
        .filter_map(|r| match r {
            EvolutionRow::Run(r) => Some(r),
            EvolutionRow::Swap(_) => None,
        })
        .collect();
    assert_eq!(runs.len(), EVOLUTION_WINDOW, "the window closes at 10");
    let first = &runs[0];
    assert_eq!(first.swap.as_deref(), Some("c"));
    assert_eq!(first.build, Build::this());
    assert_eq!(first.task, fingerprint("task 0"));
    assert_eq!(first.ask.as_deref(), Some("task 0"));
    assert_eq!(first.state, s.after_state());
    assert_eq!(first.state_after, s.after_state());
    assert!(!first.control);
    assert_eq!(first.gate, Some(Gate::Pass));
    // A Mind edited mid-run shows on the row it left.
    let w2 = World::new("drift");
    let s2 = swap_at(
        &w2.world(),
        &w2.ledger(),
        &plan("d", vec![staged(MIND, Some("new\n"))]),
    )
    .unwrap();
    let stamp = stamp_for(&w2.world(), w2.ledger().path(), "rimuru", "edit").unwrap();
    fs::write(w2.world().join(MIND), "edited mid-run\n").unwrap();
    stamp.record(&w2.world(), Facts::default()).unwrap();
    let Some(EvolutionRow::Run(r)) = w2.rows().pop() else {
        panic!("a run row")
    };
    assert_eq!(r.state, s2.after_state());
    assert_ne!(r.state_after, s2.after_state());
}
