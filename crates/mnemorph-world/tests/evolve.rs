//! When a creature is due to evolve, and by which arm: stressed — its last three trusted gate
//! verdicts failed, its strain reached the council's line, or a run met the doom checkpoint — and
//! its desk full — more than seven live notes in its personal memory —
//! read over the harness's journals, the witness and the world. Proposed only: nothing is drafted
//! or swapped.

use std::path::PathBuf;

use mnemorph_ledger::{Build, Journal, Timestamp, Usage, UsageRow};
use mnemorph_world::creatures;
use mnemorph_world::evolution::{Arm, EvolutionRow, Swap};
use mnemorph_world::evolve::{self, Due};
use mnemorph_world::queue::state_file_in;
use mnemorph_world::strain::{self, Act, Row};
use mnemorph_world::verdicts::{self, VerdictRow};

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
    n: u32,
}

impl World {
    /// A world of a Slime `sly` over `src/`, adopted by the human, its doc's extra lines `extra`.
    fn new(extra: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("world");
        let state = dir.path().join("state");
        std::fs::create_dir_all(world.join("src")).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        let world = world.canonicalize().unwrap();
        for (seat, card) in [
            (
                "summoner/summoner",
                "# summoner\n\n- **Race:** summoner\n".to_owned(),
            ),
            (
                "slime/sly",
                format!(
                    "# sly\n\n- **Race:** slime\n- **Reports to:** summoner\n- **Territory:** src/\n{extra}"
                ),
            ),
        ] {
            let at = world.join(".mnemorph").join(seat);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("README.md"), card).unwrap();
        }
        let rows = creatures::adoption_in(Some(&state), &world);
        creatures::record_human_in(Some(&state), &world, rows).unwrap();
        World {
            _dir: dir,
            root: world,
            state,
            n: 0,
        }
    }

    /// One run of sly's, landing `changed`, judged `word` with `failures`.
    fn run(&mut self, word: &str, changed: &str, failures: &[&str]) {
        self.n += 1;
        let run = format!("sly-{}", self.n);
        let ts = 1_793_000_000 + i64::from(self.n) * 3600;
        Journal::<UsageRow>::chained(state_file_in(&self.state, &self.root, "usage"))
            .append(&UsageRow {
                ts: Timestamp(ts),
                session: "s".into(),
                run: run.clone(),
                body: "sly".into(),
                race: "slime".into(),
                commission: Some("summoner".into()),
                model: "m".into(),
                provider: "p".into(),
                usage: Usage::default(),
                prefix: 0,
                wt: 0,
                usd: None,
                reported_cost: None,
                upstream: None,
                message_id: None,
                estimated: false,
                envelope: None,
                purpose: None,
            })
            .unwrap();
        verdicts::journal_at(state_file_in(&self.state, &self.root, "gate"))
            .append(&VerdictRow {
                ts: Timestamp(ts + 60),
                body: "sly".into(),
                verdict: word.into(),
                changed: vec![changed.into()],
                failures: failures.iter().map(|f| (*f).to_owned()).collect(),
                unverified: Vec::new(),
                unchecked: false,
                verified_by: vec!["the human's pin".into()],
                entry: None,
                build: Build::default(),
                run: Some(run),
            })
            .unwrap();
    }

    fn fails(&mut self, n: usize, changed: &[&str], failure: &str) {
        for i in 0..n {
            self.run("FAIL", changed[i % changed.len()], &[failure]);
        }
    }

    /// `n` notes in sly's personal memory, each about one of `sources` in turn.
    fn notes(&self, n: usize, sources: &[&str]) {
        for i in 0..n {
            let source = sources
                .get(i % sources.len().max(1))
                .map(|s| (*s).to_owned());
            mnemorph_memory::propose(
                &self.root,
                "sly",
                &format!("lesson {i}: the parser keeps fractions"),
                vec!["law".into()],
                source,
            )
            .unwrap();
        }
    }

    /// `act` of sly in the trusted strain journal, as the harness writes it.
    fn saw(&self, act: Act) {
        strain::journal_at(strain::path_in(&self.state, &self.root))
            .append(&Row {
                ts: Timestamp(1_793_000_000),
                body: "sly".into(),
                act,
                build: Build::default(),
            })
            .unwrap();
    }

    fn survey(&self) -> evolve::Survey {
        evolve::survey_in(&self.root, &self.state).unwrap()
    }
}

const DUTIES: &str = "duties done: the answer is empty; say what was done";

fn due(s: &evolve::Survey) -> Option<&Due> {
    s.due.iter().find(|d| d.creature == "sly")
}

#[test]
fn a_stressed_creature_with_a_full_desk_is_due_to_mutate() {
    let mut w = World::new("");
    w.fails(3, &["src/a.rs"], DUTIES);
    w.notes(8, &[]);
    let s = w.survey();
    let d = due(&s).unwrap_or_else(|| panic!("{s:?}"));
    assert_eq!(d.arm, Arm::Mutate);
    assert!(
        d.why
            .iter()
            .any(|y| y.contains("last 3 gate verdicts failed")),
        "{d:?}"
    );
    assert!(d.why.iter().any(|y| y.contains("8 notes")), "{d:?}");
    let lines = evolve::lines_of(&s);
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("sly (slime): due to mutate") && l.contains("proposed only")),
        "{lines:?}"
    );
}

#[test]
fn stress_or_a_full_desk_alone_is_not_enough() {
    // A full desk, but its last verdict passed.
    let mut w = World::new("");
    w.fails(3, &["src/a.rs"], DUTIES);
    w.run("PASS", "src/a.rs", &[]);
    w.notes(9, &[]);
    assert!(due(&w.survey()).is_none());
    // Stressed, but seven notes are no full desk.
    let mut w = World::new("");
    w.fails(3, &["src/a.rs"], DUTIES);
    w.notes(7, &[]);
    assert!(due(&w.survey()).is_none());
    // A verdict no check could run for is the machine's: it neither fails nor passes.
    let mut w = World::new("");
    w.fails(2, &["src/a.rs"], DUTIES);
    w.run("NOT_VERIFIED, unrun", "src/a.rs", &[]);
    w.notes(8, &[]);
    assert!(due(&w.survey()).is_none(), "two fails and an unrun");
    w.run("FAIL", "src/a.rs", &[DUTIES]);
    assert!(due(&w.survey()).is_some(), "the unrun skipped: three fails");
}

#[test]
fn a_gate_failed_on_its_own_verify_lines_or_a_doc_naming_files_gone_is_reborn() {
    let mut w = World::new("");
    w.fails(3, &["src/a.rs"], "traits hold: `cargo test` failed:\nboom");
    w.notes(8, &[]);
    let s = w.survey();
    let d = due(&s).unwrap();
    assert_eq!(d.arm, Arm::Rebirth);
    assert!(
        d.why.iter().any(|y| y.contains("its own verify lines")),
        "{d:?}"
    );
    let mut w = World::new("\nIt keeps `src/parser/gone.rs` in order.\n");
    w.fails(3, &["src/a.rs"], DUTIES);
    w.notes(8, &[]);
    let d = w.survey().due.into_iter().next().unwrap();
    assert_eq!(d.arm, Arm::Rebirth);
    assert!(
        d.why.iter().any(|y| y.contains("`src/parser/gone.rs`")),
        "{d:?}"
    );
}

#[test]
fn a_large_zone_whose_trouble_clusters_in_two_parts_splits() {
    let mut w = World::new("");
    for part in ["lexer", "parser"] {
        std::fs::create_dir_all(w.root.join("src").join(part)).unwrap();
        for i in 0..12 {
            std::fs::write(w.root.join(format!("src/{part}/f{i}.rs")), "fn f() {}\n").unwrap();
        }
    }
    w.fails(4, &["src/lexer/f1.rs", "src/parser/f2.rs"], DUTIES);
    w.notes(8, &["src/lexer/f3.rs", "src/parser/f4.rs"]);
    let d = w.survey().due.into_iter().next().unwrap();
    assert_eq!(d.arm, Arm::Split, "{d:?}");
    assert!(
        d.why
            .iter()
            .any(|y| y.contains("24 files") && y.contains("src/lexer") && y.contains("src/parser")),
        "{d:?}"
    );
    // The two parts it names are relief's to split around, one to each young — the human's.
    assert_eq!(
        d.parts,
        vec!["src/lexer".to_owned(), "src/parser".to_owned()]
    );
    assert_eq!(
        d.relief_command().as_deref(),
        Some("mnemorphd relief sly --around src/lexer,src/parser")
    );
    let lines = evolve::lines_of(&w.survey());
    assert!(
        lines
            .iter()
            .any(|l| l.contains("a split is the human's")
                && l.contains("--around src/lexer,src/parser")),
        "{lines:?}"
    );
    let r = mnemorph_world::relief::relieve_by(
        Some(&w.state),
        &w.root,
        "sly",
        Some(&[d.parts[0].clone(), d.parts[1].clone()]),
        false,
    )
    .unwrap();
    assert!(
        r.line().contains("sly-a takes src/lexer/") && r.line().contains("sly-b takes src/parser/"),
        "{}",
        r.line()
    );
    // The same trouble in a small zone mutates.
    let mut w = World::new("");
    std::fs::create_dir_all(w.root.join("src/lexer")).unwrap();
    std::fs::create_dir_all(w.root.join("src/parser")).unwrap();
    w.fails(4, &["src/lexer/f1.rs", "src/parser/f2.rs"], DUTIES);
    w.notes(8, &["src/lexer/f3.rs", "src/parser/f4.rs"]);
    assert_eq!(w.survey().due[0].arm, Arm::Mutate);
}

#[test]
fn a_creature_whose_swap_window_is_open_cools() {
    let mut w = World::new("");
    w.fails(3, &["src/a.rs"], DUTIES);
    w.notes(8, &[]);
    Journal::<EvolutionRow>::chained(state_file_in(&w.state, &w.root, "evolution"))
        .append(&EvolutionRow::Swap(Swap {
            id: "ev1".into(),
            creature: "sly".into(),
            bodies: vec!["sly".into()],
            arm: Arm::Mutate,
            build: Build::this(),
            files: Vec::new(),
            diff: Vec::new(),
            proven_on: Vec::new(),
        }))
        .unwrap();
    let s = w.survey();
    assert!(due(&s).is_none(), "{s:?}");
    assert_eq!(s.cooling, vec![("sly".to_owned(), "ev1".to_owned())]);
    let lines = evolve::lines_of(&s);
    assert!(
        lines
            .iter()
            .any(|l| l.contains("sly: cooling") && l.contains("ev1")),
        "{lines:?}"
    );
}

#[test]
fn strain_at_the_councils_line_or_a_doom_checkpoint_stresses_a_creature_as_verdicts_do() {
    // The council's line, with no failed verdict at all.
    let w = World::new("");
    w.notes(8, &[]);
    w.saw(Act::Strain { by: 3 });
    assert!(due(&w.survey()).is_none(), "under the line");
    w.saw(Act::Strain { by: 2 });
    let s = w.survey();
    let d = due(&s).unwrap_or_else(|| panic!("{s:?}"));
    assert_eq!(d.arm, Arm::Mutate);
    assert!(d.why.iter().any(|y| y.contains("strain 5")), "{d:?}");
    // A clean landing clears it.
    w.saw(Act::Clear);
    assert!(due(&w.survey()).is_none(), "cleared");
    // A doom checkpoint, until a clean landing.
    w.saw(Act::Doom);
    let s = w.survey();
    let d = due(&s).unwrap_or_else(|| panic!("{s:?}"));
    assert!(d.why.iter().any(|y| y.contains("doom checkpoint")), "{d:?}");
    w.saw(Act::Clear);
    assert!(due(&w.survey()).is_none());
    // Stress alone is not enough: the desk must be full too.
    let w = World::new("");
    w.notes(7, &[]);
    w.saw(Act::Strain { by: 9 });
    assert!(due(&w.survey()).is_none());
}

#[test]
fn strain_a_body_wrote_in_the_world_counts_for_nothing() {
    let w = World::new("");
    w.notes(8, &[]);
    let at = w.root.join(".mnemorph/instruments");
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(
        at.join("council.jsonl"),
        "{\"kind\":\"strain\",\"body\":\"sly\",\"by\":99}\n",
    )
    .unwrap();
    assert!(
        due(&w.survey()).is_none(),
        "the council's tally is the world's"
    );
}

#[test]
fn a_strain_journal_that_does_not_verify_is_refused_never_read_as_calm() {
    let w = World::new("");
    w.notes(8, &[]);
    w.saw(Act::Strain { by: 2 });
    let at = strain::path_in(&w.state, &w.root);
    let text = std::fs::read_to_string(&at).unwrap();
    std::fs::write(&at, text.replace("\"by\":2", "\"by\":9")).unwrap();
    let e = evolve::survey_in(&w.root, &w.state).unwrap_err();
    assert!(e.contains("strain journal"), "{e}");
}

/// One world, however it is spelled: a body's session and the survey may reach it by different
/// paths (macOS's `/var` is a link), and strain written by one must be read by the other.
#[cfg(unix)]
#[test]
fn a_world_reached_through_a_link_keeps_one_strain_journal() {
    let w = World::new("");
    let link = w.root.with_file_name("linked-world");
    std::os::unix::fs::symlink(&w.root, &link).unwrap();
    assert_eq!(
        strain::path_in(&w.state, &link),
        strain::path_in(&w.state, &w.root)
    );
    let chained = mnemorph_world::chains::journals_in(&w.state, &link);
    let held = chained.iter().find(|j| j.name == strain::KIND).unwrap();
    assert_eq!(held.path, strain::path_in(&w.state, &w.root));
    let held = chained.iter().find(|j| j.name == "proposals").unwrap();
    assert_eq!(
        held.path,
        mnemorph_world::proposals::path_in(&w.state, &w.root)
    );
}
