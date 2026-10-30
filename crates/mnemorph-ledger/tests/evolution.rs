//! An evolution judged after the freeze: the post-swap window, the do-nothing control, the exact
//! file set, the held-out tasks, and replay from the chained ledger.

use std::fs;
use std::path::Path;

use mnemorph_ledger::evolution::{
    self, Arm, EVOLUTION_WINDOW, EvolutionRow, FileChange, Gate, Judgement, Refusal, Run, Swap,
};
use mnemorph_ledger::{Build, Journal};
use mnemorph_races::Kind;

const MIND: &str = ".mnemorph/slime/wire/README.md";

fn build() -> Build {
    Build {
        version: "1.2.0".into(),
        commit: "a1b2c3d4e5".into(),
    }
}

fn change(path: &str, before: Option<&str>, after: Option<&str>) -> FileChange {
    FileChange {
        path: path.into(),
        before: before.map(|b| evolution::digest(b.as_bytes())),
        after: after.map(|a| evolution::digest(a.as_bytes())),
    }
}

fn swap(id: &str) -> Swap {
    let files = vec![change(MIND, Some("old doc"), Some("new doc"))];
    Swap {
        id: id.into(),
        creature: "slime-wire".into(),
        bodies: vec!["slime-wire".into()],
        arm: Arm::Mutate,
        build: build(),
        diff: files.clone(),
        files,
        proven_on: vec!["staged-1".into()],
    }
}

fn run(s: &Swap, task: &str, control: bool, pass: bool) -> EvolutionRow {
    EvolutionRow::Run(Run {
        swap: Some(s.id.clone()),
        build: s.build.clone(),
        body: "slime-wire".into(),
        task: task.into(),
        control,
        state: if control {
            s.before_state()
        } else {
            s.after_state()
        },
        gate: Some(if pass { Gate::Pass } else { Gate::Fail }),
        first_try: pass,
        holes_found: 0,
    })
}

fn ledger(path: &Path, rows: &[EvolutionRow]) -> Journal<EvolutionRow> {
    let j = Journal::chained(path);
    for r in rows {
        j.append(r).unwrap();
    }
    j
}

/// A full window: `evolved[i]` and `nothing[i]` are task `i`'s gate words with and without.
fn window(s: &Swap, evolved: &[bool], nothing: &[bool]) -> Vec<EvolutionRow> {
    let mut rows = Vec::new();
    for (i, (e, n)) in evolved.iter().zip(nothing).enumerate() {
        rows.push(run(s, &format!("held-{i}"), false, *e));
        rows.push(run(s, &format!("held-{i}"), true, *n));
    }
    rows
}

fn judged(rows: &[EvolutionRow], id: &str) -> Judgement {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evolution.jsonl");
    ledger(&path, rows);
    evolution::judge(&evolution::read(&path).unwrap(), id)
}

#[test]
fn the_window_excludes_rows_before_the_swap() {
    let s = swap("e1");
    // Before the swap: the staging's own runs, stamped early, and plain runs — every one failing.
    let mut rows: Vec<EvolutionRow> = (0..EVOLUTION_WINDOW)
        .map(|i| run(&s, &format!("held-{i}"), false, false))
        .collect();
    rows.push(EvolutionRow::Run(Run {
        swap: None,
        ..match run(&s, "held-0", false, false) {
            EvolutionRow::Run(r) => r,
            EvolutionRow::Swap(_) => unreachable!(),
        }
    }));
    let swapped_at = u64::try_from(rows.len()).unwrap() + 1;
    rows.push(EvolutionRow::Swap(s.clone()));
    // After it, under another build: not its window either.
    rows.push(EvolutionRow::Run(Run {
        build: Build::default(),
        ..match run(&s, "held-0", false, false) {
            EvolutionRow::Run(r) => r,
            EvolutionRow::Swap(_) => unreachable!(),
        }
    }));
    let half: Vec<bool> = (0..EVOLUTION_WINDOW).map(|i| i % 2 == 0).collect();
    rows.extend(window(&s, &[true; EVOLUTION_WINDOW], &half));
    let Judgement::Keep(lines) = judged(&rows, "e1") else {
        panic!("{}", judged(&rows, "e1"));
    };
    assert!(
        (lines[0].reading - 100.0).abs() < 1e-9,
        "only post-swap runs: {}",
        lines[0]
    );
    assert!(lines.iter().all(|l| l.from > Some(swapped_at + 1)));
    assert!(
        lines[0].to_string().starts_with(
            "keep: landRate 1.00 > 0.50 (over 10 runs beside do-nothing); ledger head "
        ),
        "{}",
        lines[0]
    );
}

#[test]
fn an_evolution_equal_to_doing_nothing_rolls_back() {
    let s = swap("e1");
    // Tasks where doing nothing is the right output pass both ways; the rest split alike.
    let same: Vec<bool> = (0..EVOLUTION_WINDOW).map(|i| i < 7).collect();
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(window(&s, &same, &same));
    let Judgement::Rollback(lines) = judged(&rows, "e1") else {
        panic!("{}", judged(&rows, "e1"));
    };
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].kind, Kind::Rollback);
    assert!(
        lines[0]
            .to_string()
            .starts_with("rollback: landRate 0.70 <= 0.70 (over 10 runs beside do-nothing)"),
        "{}",
        lines[0]
    );
    assert!(lines[0].holds());
    // Worse than doing nothing, or more holes: rolled back too.
    let mut worse = vec![EvolutionRow::Swap(s.clone())];
    worse.extend(window(&s, &[false; EVOLUTION_WINDOW], &same));
    assert!(matches!(judged(&worse, "e1"), Judgement::Rollback(_)));
    let mut holes = vec![EvolutionRow::Swap(s.clone())];
    for (i, row) in window(&s, &[true; EVOLUTION_WINDOW], &same)
        .into_iter()
        .enumerate()
    {
        holes.push(match row {
            EvolutionRow::Run(r) if i == 0 => EvolutionRow::Run(Run {
                holes_found: 1,
                ..r
            }),
            other => other,
        });
    }
    let Judgement::Rollback(lines) = judged(&holes, "e1") else {
        panic!("{}", judged(&holes, "e1"));
    };
    assert_eq!(lines[0].meter, "holeRate");
}

#[test]
fn a_contaminated_swap_is_refused() {
    let id = "e1";
    let with_diff = |diff: Vec<FileChange>| Swap { diff, ..swap(id) };
    let refused = |s: Swap| {
        let mut rows = vec![EvolutionRow::Swap(s.clone())];
        rows.extend(window(
            &s,
            &[true; EVOLUTION_WINDOW],
            &[false; EVOLUTION_WINDOW],
        ));
        judged(&rows, id)
    };
    let named = swap(id).files;
    let mut extra = named.clone();
    extra.push(change(".mnemorph/orc/gate/README.md", Some("a"), Some("b")));
    assert_eq!(
        refused(with_diff(extra)),
        Judgement::Refused(Refusal::Contaminated(".mnemorph/orc/gate/README.md".into()))
    );
    assert_eq!(
        refused(with_diff(Vec::new())),
        Judgement::Refused(Refusal::Missing(MIND.into()))
    );
    // A file the swap creates must not have existed.
    let created = Swap {
        files: vec![change(MIND, None, Some("new doc"))],
        diff: vec![change(MIND, Some("was there"), Some("new doc"))],
        ..swap(id)
    };
    assert_eq!(
        refused(created),
        Judgement::Refused(Refusal::Mismatch(MIND.into()))
    );
    assert_eq!(
        refused(Swap {
            files: Vec::new(),
            diff: Vec::new(),
            ..swap(id)
        }),
        Judgement::Refused(Refusal::NoFiles)
    );
    // A window run that found the files other than the swap left them: refused.
    let s = swap(id);
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(window(
        &s,
        &[true; EVOLUTION_WINDOW],
        &[false; EVOLUTION_WINDOW],
    ));
    if let EvolutionRow::Run(r) = &mut rows[3] {
        r.state = evolution::state([(MIND, Some("edited since"))]);
    }
    assert_eq!(judged(&rows, id), Judgement::Refused(Refusal::Drifted(4)));
    // A control that did not run without the change: refused.
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(window(
        &s,
        &[true; EVOLUTION_WINDOW],
        &[false; EVOLUTION_WINDOW],
    ));
    if let EvolutionRow::Run(r) = &mut rows[2] {
        r.state = s.after_state();
    }
    assert_eq!(
        judged(&rows, id),
        Judgement::Refused(Refusal::ControlState(3))
    );
}

#[test]
fn the_file_set_is_read_from_the_soil() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".mnemorph/slime/wire")).unwrap();
    fs::write(root.join(MIND), "old doc").unwrap();
    fs::write(root.join("notes.txt"), "a").unwrap();
    let before = evolution::snapshot(root).unwrap();
    fs::write(root.join(MIND), "new doc").unwrap();
    let mut s = swap("e1");
    s.diff = evolution::diff(&before, &evolution::snapshot(root).unwrap());
    assert_eq!(s.check(), Ok(()));
    assert_eq!(evolution::state_now(root, &s).unwrap(), s.after_state());
    fs::write(root.join("notes.txt"), "b").unwrap();
    s.diff = evolution::diff(&before, &evolution::snapshot(root).unwrap());
    assert_eq!(s.check(), Err(Refusal::Contaminated("notes.txt".into())));
}

#[test]
fn a_held_out_task_is_used_once() {
    let s = swap("e1");
    let full = |s: &Swap| window(s, &[true; EVOLUTION_WINDOW], &[false; EVOLUTION_WINDOW]);
    // A task the plan was proven on.
    let mut rows = vec![EvolutionRow::Swap(Swap {
        proven_on: vec!["held-3".into()],
        ..s.clone()
    })];
    rows.extend(full(&s));
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Refused(Refusal::Leaked("held-3".into()))
    );
    // A task another swap's window used first.
    let other = swap("e0");
    let mut rows = vec![
        EvolutionRow::Swap(other.clone()),
        run(&other, "held-5", false, true),
        EvolutionRow::Swap(s.clone()),
    ];
    rows.extend(full(&s));
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Refused(Refusal::Reused("held-5".into()))
    );
    // A task twice in the window.
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(full(&s));
    rows.insert(3, run(&s, "held-0", false, true));
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Refused(Refusal::Reused("held-0".into()))
    );
}

#[test]
fn a_window_without_its_controls_is_pending() {
    let s = swap("e1");
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(window(&s, &[true; 4], &[false; 4]));
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Pending {
            runs: 4,
            controls: 4
        }
    );
    let rows: Vec<EvolutionRow> = std::iter::once(EvolutionRow::Swap(s.clone()))
        .chain((0..EVOLUTION_WINDOW).map(|i| run(&s, &format!("held-{i}"), false, true)))
        .collect();
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Pending {
            runs: EVOLUTION_WINDOW,
            controls: 0
        }
    );
    assert_eq!(
        judged(&rows, "e9"),
        Judgement::Refused(Refusal::NoSwap("e9".into()))
    );
}

#[test]
fn replaying_the_ledger_reproduces_the_judgement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evolution.jsonl");
    let s = swap("e1");
    let mut rows = vec![
        run(&s, "held-0", false, false),
        EvolutionRow::Swap(s.clone()),
    ];
    let nothing: Vec<bool> = (0..EVOLUTION_WINDOW).map(|i| i % 3 == 0).collect();
    rows.extend(window(&s, &[true; EVOLUTION_WINDOW], &nothing));
    let j = ledger(&path, &rows);
    let first = evolution::judge(&evolution::read(&path).unwrap(), "e1");
    let Judgement::Keep(lines) = &first else {
        panic!("{first}");
    };
    let to = lines[0].to.unwrap();
    let head = lines[0].head.clone().unwrap();
    assert!(
        lines
            .iter()
            .all(|l| l.holds() && l.head.as_deref() == Some(&head))
    );
    // The ledger grows on: more runs of the swap, another swap.
    let next = swap("e2");
    for r in [
        run(&s, "held-11", false, false),
        run(&s, "held-11", true, true),
        EvolutionRow::Swap(next.clone()),
        run(&next, "held-12", false, true),
    ] {
        j.append(&r).unwrap();
    }
    let again = evolution::judge(&evolution::read(&path).unwrap(), "e1");
    assert_eq!(again, first, "a full window is judged once and for all");
    assert_eq!(evolution::replay(&path, "e1", to).unwrap(), first);
    // The named head is the digest of the window's last row, as the chain walks it.
    let prefix = evolution::read_to(&path, to).unwrap();
    assert_eq!(prefix.chain.head.unwrap().digest, head);
    assert_eq!(first.to_string().lines().count(), 2);
    // An edited row breaks the chain: nothing is judged on it.
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replacen("\"pass\"", "\"fail\"", 1)).unwrap();
    assert!(matches!(
        evolution::replay(&path, "e1", to).unwrap(),
        Judgement::Refused(Refusal::ChainBroken(_))
    ));
}
