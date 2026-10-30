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
    let state = if control {
        s.before_state()
    } else {
        s.after_state()
    };
    EvolutionRow::Run(Run {
        swap: Some(s.id.clone()),
        build: s.build.clone(),
        body: "slime-wire".into(),
        task: task.into(),
        control,
        state_after: state.clone(),
        state,
        gate: Some(if pass { Gate::Pass } else { Gate::Fail }),
        first_try: pass,
        holes_found: 0,
        ask: None,
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
    evolution::judge(&evolution::read(&path, None).unwrap(), id, MIN)
}

/// The keep bar the tests judge on: `evolution.min_net_wins` unless the human sets it.
const MIN: u32 = 2;

fn unrun(row: EvolutionRow) -> Run {
    match row {
        EvolutionRow::Run(r) => r,
        EvolutionRow::Swap(_) => unreachable!(),
    }
}

#[test]
fn the_window_excludes_rows_before_the_swap() {
    let s = swap("e1");
    // Before the swap: the staging's own runs, stamped early, and plain runs — every one failing.
    let mut rows: Vec<EvolutionRow> = (0..EVOLUTION_WINDOW)
        .map(|i| run(&s, &format!("old-{i}"), false, false))
        .collect();
    rows.push(EvolutionRow::Run(Run {
        swap: None,
        ..unrun(run(&s, "old-10", false, false))
    }));
    let swapped_at = u64::try_from(rows.len()).unwrap() + 1;
    rows.push(EvolutionRow::Swap(s.clone()));
    // After it, under another build: not its window either.
    rows.push(EvolutionRow::Run(Run {
        build: Build::default(),
        ..unrun(run(&s, "other-0", false, false))
    }));
    let half: Vec<bool> = (0..EVOLUTION_WINDOW).map(|i| i % 2 == 0).collect();
    rows.extend(window(&s, &[true; EVOLUTION_WINDOW], &half));
    let Judgement::Keep(lines) = judged(&rows, "e1") else {
        panic!("{}", judged(&rows, "e1"));
    };
    assert!(
        (lines[0].reading - 5.0).abs() < 1e-9,
        "only post-swap runs: {}",
        lines[0]
    );
    assert!(lines.iter().all(|l| l.from > Some(swapped_at + 1)));
    assert!(
        lines[0]
            .to_string()
            .starts_with("keep: netWins 5 >= 2 (over 10 runs beside do-nothing); ledger head "),
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
            .starts_with("rollback: netWins 0 < 2 (over 10 runs beside do-nothing)"),
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
        run(&s, "old-0", false, false),
        EvolutionRow::Swap(s.clone()),
    ];
    let nothing: Vec<bool> = (0..EVOLUTION_WINDOW).map(|i| i % 3 == 0).collect();
    rows.extend(window(&s, &[true; EVOLUTION_WINDOW], &nothing));
    let j = ledger(&path, &rows);
    let first = evolution::judge(&evolution::read(&path, None).unwrap(), "e1", MIN);
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
    let anchor = j.head().unwrap();
    let again = evolution::judge(&evolution::read(&path, anchor.as_ref()).unwrap(), "e1", MIN);
    assert_eq!(again, first, "a full window is judged once and for all");
    assert_eq!(
        evolution::replay(&path, "e1", to, anchor.as_ref(), MIN).unwrap(),
        first
    );
    // The named head is the digest of the window's last row, as the chain walks it.
    let prefix = evolution::read_to(&path, to, anchor.as_ref()).unwrap();
    let last = prefix.entries.last().unwrap();
    assert_eq!((last.at, last.digest.as_deref()), (to, Some(head.as_str())));
    assert_eq!(first.to_string().lines().count(), 2);
    // An edited row breaks the chain: nothing is judged on it.
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replacen("\"pass\"", "\"fail\"", 1)).unwrap();
    assert!(matches!(
        evolution::replay(&path, "e1", to, None, MIN).unwrap(),
        Judgement::Refused(Refusal::ChainBroken(_))
    ));
}

#[test]
fn it_keeps_on_two_net_paired_wins_and_rolls_back_on_one() {
    let s = swap("e1");
    let nothing = [false, false, true, true, true, true, true, true, true, true];
    // One task won, none lost: one net win.
    let one = [true; EVOLUTION_WINDOW];
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(window(&s, &one, &nothing));
    let mut only_one = nothing;
    only_one[1] = true;
    let mut rows_one = vec![EvolutionRow::Swap(s.clone())];
    rows_one.extend(window(&s, &one, &only_one));
    let Judgement::Rollback(lines) = judged(&rows_one, "e1") else {
        panic!("{}", judged(&rows_one, "e1"));
    };
    assert!(
        lines[0].to_string().starts_with("rollback: netWins 1 < 2"),
        "{}",
        lines[0]
    );
    // Two won: kept.
    let Judgement::Keep(lines) = judged(&rows, "e1") else {
        panic!("{}", judged(&rows, "e1"));
    };
    assert!(
        lines[0].to_string().starts_with("keep: netWins 2 >= 2"),
        "{}",
        lines[0]
    );
    // Two won and one lost elsewhere: one net, rolled back.
    let mut lost = one;
    lost[5] = false;
    let mut rows_lost = vec![EvolutionRow::Swap(s.clone())];
    rows_lost.extend(window(&s, &lost, &nothing));
    assert!(matches!(judged(&rows_lost, "e1"), Judgement::Rollback(_)));
    // The human's bar is the one judged on.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evolution.jsonl");
    ledger(&path, &rows_one);
    assert!(matches!(
        evolution::judge(&evolution::read(&path, None).unwrap(), "e1", 1),
        Judgement::Keep(_)
    ));
}

#[test]
fn a_control_counts_only_after_the_swap_and_by_the_same_body() {
    let s = swap("e1");
    let evolved: Vec<EvolutionRow> = (0..EVOLUTION_WINDOW)
        .map(|i| run(&s, &format!("held-{i}"), false, true))
        .collect();
    let controls: Vec<EvolutionRow> = (0..EVOLUTION_WINDOW)
        .map(|i| run(&s, &format!("held-{i}"), true, false))
        .collect();
    // Controls run before the swap: none counts.
    let mut rows = controls.clone();
    rows.push(EvolutionRow::Swap(s.clone()));
    rows.extend(evolved.clone());
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Pending {
            runs: EVOLUTION_WINDOW,
            controls: 0
        }
    );
    // Controls run by another body: none counts.
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(evolved.clone());
    rows.extend(controls.iter().cloned().map(|c| {
        EvolutionRow::Run(Run {
            body: "slime-other".into(),
            ..unrun(c)
        })
    }));
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Pending {
            runs: EVOLUTION_WINDOW,
            controls: 0
        }
    );
    // After the swap, by the same body: judged.
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(evolved);
    rows.extend(controls);
    assert!(matches!(judged(&rows, "e1"), Judgement::Keep(_)));
}

#[test]
fn a_task_any_earlier_run_carries_is_refused() {
    let s = swap("e1");
    // An unstamped run of the task, long before the swap.
    let mut rows = vec![
        EvolutionRow::Run(Run {
            swap: None,
            ..unrun(run(&s, "held-4", false, true))
        }),
        EvolutionRow::Swap(s.clone()),
    ];
    rows.extend(window(
        &s,
        &[true; EVOLUTION_WINDOW],
        &[false; EVOLUTION_WINDOW],
    ));
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Refused(Refusal::Reused("held-4".into()))
    );
}

#[test]
fn a_mid_run_edit_refuses_its_row() {
    let s = swap("e1");
    let edited = evolution::state([(MIND, Some("edited mid-run"))]);
    let full = || {
        let mut rows = vec![EvolutionRow::Swap(s.clone())];
        rows.extend(window(
            &s,
            &[true; EVOLUTION_WINDOW],
            &[false; EVOLUTION_WINDOW],
        ));
        rows
    };
    let mut rows = full();
    if let EvolutionRow::Run(r) = &mut rows[5] {
        r.state_after.clone_from(&edited);
    }
    assert_eq!(judged(&rows, "e1"), Judgement::Refused(Refusal::Drifted(6)));
    let mut rows = full();
    if let EvolutionRow::Run(r) = &mut rows[6] {
        r.state_after.clone_from(&edited);
    }
    assert_eq!(
        judged(&rows, "e1"),
        Judgement::Refused(Refusal::ControlState(7))
    );
}

#[test]
fn an_unchained_or_rechained_ledger_is_never_judged() {
    let s = swap("e1");
    let mut rows = vec![EvolutionRow::Swap(s.clone())];
    rows.extend(window(
        &s,
        &[true; EVOLUTION_WINDOW],
        &[false; EVOLUTION_WINDOW],
    ));
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.jsonl");
    let j: Journal<EvolutionRow> = Journal::new(&plain);
    for r in &rows {
        j.append(r).unwrap();
    }
    assert_eq!(
        evolution::judge(&evolution::read(&plain, None).unwrap(), "e1", MIN),
        Judgement::Refused(Refusal::Unchained(None))
    );
    // A body that could write the file chains it again from the start, a control rewritten: the
    // head anchored outside the file no longer holds.
    let path = dir.path().join("evolution.jsonl");
    let anchor = ledger(&path, &rows).head().unwrap().unwrap();
    let forged = dir.path().join("forged.jsonl");
    let mut changed = rows.clone();
    if let EvolutionRow::Run(r) = &mut changed[2] {
        r.gate = Some(Gate::Pass);
    }
    ledger(&forged, &changed);
    fs::copy(&forged, &path).unwrap();
    assert!(matches!(
        evolution::judge(&evolution::read(&path, None).unwrap(), "e1", MIN),
        Judgement::Keep(_)
    ));
    assert!(matches!(
        evolution::judge(&evolution::read(&path, Some(&anchor)).unwrap(), "e1", MIN),
        Judgement::Refused(Refusal::ChainBroken(_))
    ));
}
