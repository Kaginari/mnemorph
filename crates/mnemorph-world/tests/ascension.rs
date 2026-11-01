//! The M11 exit: replaying the trusted journals and the creatures' witness from the start
//! reproduces every rise and fall exactly — the same proposals at the same points, and, given the
//! same human applies, the same rows.
//!
//! The fixture is a world of three creatures whose runs are written as the harness writes them —
//! the usage witness and the gate verdicts, chained, under a temporary state directory — and
//! whose standing the human adopts. Over its record one Slime rises, slips into probation and
//! recovers, slips again and falls; the others never move.

use std::path::PathBuf;

use mnemorph_ledger::{Build, Journal, Timestamp, Usage, UsageRow};
use mnemorph_world::ascension::{self, Move};
use mnemorph_world::creatures::{self, Act, Row};
use mnemorph_world::queue::state_file_in;
use mnemorph_world::verdicts::{self, VerdictRow};

const HOUR: i64 = 3600;
/// November 2026.
const T0: i64 = 1_793_000_000;

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
    /// The next run's start.
    at: i64,
    n: u32,
}

impl World {
    /// A world of a Slime `sly` (`src/`), a Slime `mud` (`lib/`) and an Elf `lana`, adopted by
    /// the human.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("world");
        let state = dir.path().join("state");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        let world = world.canonicalize().unwrap();
        for (seat, card) in [
            ("rimuru/rimuru", "# rimuru\n\n- **Race:** rimuru\n"),
            (
                "slime/sly",
                "# sly\n\n- **Race:** slime\n- **Reports to:** rimuru\n- **Territory:** src/\n",
            ),
            (
                "slime/mud",
                "# mud\n\n- **Race:** slime\n- **Reports to:** rimuru\n- **Territory:** lib/\n",
            ),
            (
                "elf/lana",
                "# lana\n\n- **Race:** elf\n- **Reports to:** rimuru\n- **Territory:** src/, lib/\n",
            ),
        ] {
            let at = world.join(".mnemorph").join(seat);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("README.md"), card).unwrap();
        }
        let rows = creatures::adoption_in(Some(&state), &world);
        assert_eq!(rows.len(), 3, "the throne is implicit");
        creatures::record_human_in(Some(&state), &world, rows).unwrap();
        World {
            _dir: dir,
            root: world,
            state,
            at: T0,
            n: 0,
        }
    }

    /// One run of `body` as `race`, its landing passed or failed on the human's pins, an hour
    /// after the last.
    fn run(&mut self, body: &str, race: &str, pass: bool) {
        self.n += 1;
        let run = format!("{body}-{}", self.n);
        let ts = self.at;
        self.at += HOUR;
        Journal::<UsageRow>::chained(state_file_in(&self.state, &self.root, "usage"))
            .append(&UsageRow {
                ts: Timestamp(ts),
                session: "s".into(),
                run: run.clone(),
                body: body.into(),
                race: race.into(),
                commission: Some("rimuru".into()),
                model: "m".into(),
                provider: "p".into(),
                usage: Usage::default(),
                prefix: 0,
                wt: 0,
                usd: None,
                reported_cost: None,
                estimated: false,
                envelope: None,
                purpose: None,
            })
            .unwrap();
        verdicts::journal_at(state_file_in(&self.state, &self.root, "gate"))
            .append(&VerdictRow {
                ts: Timestamp(ts + 600),
                body: body.into(),
                verdict: if pass { "PASS" } else { "FAIL" }.into(),
                changed: vec!["src/a.rs".into()],
                failures: if pass {
                    Vec::new()
                } else {
                    vec!["verify: cargo test".into()]
                },
                unverified: Vec::new(),
                unchecked: false,
                verified_by: vec!["the human's pin".into()],
                entry: None,
                build: Build::default(),
                run: Some(run),
            })
            .unwrap();
    }

    fn runs(&mut self, body: &str, race: &str, passes: &[bool]) {
        for &p in passes {
            self.run(body, race, p);
        }
    }

    fn survey(&self) -> ascension::Survey {
        ascension::survey_in(&self.root, &self.state).unwrap()
    }

    fn roster(&self) -> Vec<Row> {
        creatures::rows_in(Some(&self.state), &self.root)
            .unwrap()
            .unwrap()
    }

    /// The race the trusted view runs `name` as.
    fn runs_as(&self, name: &str) -> mnemorph_world::races::Race {
        let cards = mnemorph_world::Roster::cards(&self.root);
        let view = creatures::trusted(
            cards,
            &creatures::witnessed_in(Some(&self.state), &self.root),
        );
        view.creatures[name].race
    }
}

/// What the human saw at one point: the clock, the witness's length, every proposal's creature,
/// move and id.
type Seen = (Timestamp, usize, Vec<(String, Move, String)>);

fn seen(w: &World) -> Seen {
    let s = w.survey();
    let clock = Timestamp(w.at - HOUR);
    (
        clock,
        w.roster().len(),
        s.proposals
            .iter()
            .map(|p| (p.creature.clone(), p.act, p.id.clone()))
            .collect(),
    )
}

/// The world's whole record, the human applying each proposal for `sly` as it appears: every
/// point seen, and the witness at the end.
fn scenario(w: &mut World) -> Vec<Seen> {
    let mut points = Vec::new();
    let pass = |n: usize| vec![true; n];
    // Twenty-one passes for sly; mud lands half; lana dispatches nothing.
    w.runs("sly", "slime", &pass(21));
    w.runs("mud", "slime", &[true, false].repeat(11));
    w.runs("lana", "elf", &pass(3));
    w.run("sly", "slime", true);
    points.push(seen(w));
    let s = w.survey();
    let rise = s.of("sly").expect("sly's rise").clone();
    assert_eq!((rise.act, rise.to.name()), (Move::Rise, "named-slime"));
    assert!(s.of("mud").is_none() && s.of("lana").is_none(), "{s:?}");
    // A proposal changes nothing by itself: the witness and the trusted race stay.
    let before = w.roster();
    for _ in 0..3 {
        let _ = w.survey();
    }
    assert_eq!(w.roster(), before);
    assert_eq!(w.runs_as("sly").name(), "slime");
    // The human applies it.
    ascension::apply_in(Some(&w.state), &w.root, "sly", Move::Rise, &rise.id).unwrap();
    let last = w.roster().pop().unwrap();
    assert_eq!(
        (
            last.act,
            last.race.as_str(),
            last.by.as_str(),
            last.proposal.as_deref()
        ),
        (Act::Rise, "named-slime", "human", Some(rise.id.as_str()))
    );
    assert_eq!(
        last.inputs_sha256.as_deref(),
        Some(rise.inputs_sha256.as_str())
    );
    assert_eq!(last.as_of, Some(rise.as_of));
    points.push(seen(w));
    // Under the fall bar: probation; then it recovers at the bar.
    w.runs(
        "sly",
        "named-slime",
        &[
            false, false, false, false, true, true, true, true, true, true, true,
        ],
    );
    points.push(seen(w));
    assert_eq!(w.survey().probation_of("sly").map(|p| p.reports), Some(0));
    w.runs("sly", "named-slime", &[false, true, true, true, true, true]);
    points.push(seen(w));
    let s = w.survey();
    assert!(s.probation.is_empty() && s.proposals.is_empty(), "{s:?}");
    // Slips again, and does not recover: the fall is proposed.
    w.runs("sly", "named-slime", &[false; 3]);
    points.push(seen(w));
    assert!(w.survey().probation_of("sly").is_some());
    w.runs("sly", "named-slime", &[false; 6]);
    points.push(seen(w));
    let fall = w.survey().of("sly").expect("sly's fall").clone();
    assert_eq!((fall.act, fall.to.name()), (Move::Fall, "slime"));
    assert!(fall.ended_probation);
    // Still named until the human applies it.
    assert_eq!(w.runs_as("sly").name(), "slime", "its card says slime");
    assert_eq!(
        w.roster().last().map(|r| r.race.clone()).as_deref(),
        Some("named-slime")
    );
    ascension::apply_in(Some(&w.state), &w.root, "sly", Move::Fall, &fall.id).unwrap();
    points.push(seen(w));
    w.runs("mud", "slime", &pass(2));
    points.push(seen(w));
    points
}

/// A row as a replay compares it: everything but the wall time and the build that wrote it.
fn kept(r: &Row) -> Row {
    Row {
        ts: Timestamp(0),
        build: Build::default(),
        ..r.clone()
    }
}

#[test]
fn replaying_the_journals_and_the_witness_reproduces_every_rise_and_fall() {
    let mut w = World::new();
    let points = scenario(&mut w);
    // Every rise and fall row recomputes from the journals and the rows before it.
    let again = ascension::replay_in(&w.root, &w.state).unwrap();
    let moves: Vec<(Act, &str, &str)> = again
        .iter()
        .map(|r| (r.act, r.creature.as_str(), r.to.as_str()))
        .collect();
    assert_eq!(
        moves,
        vec![
            (Act::Rise, "sly", "named-slime"),
            (Act::Fall, "sly", "slime")
        ]
    );
    // From the start: at every point the human saw, the journals as they stand now give the
    // same proposals.
    let rows = mnemorph_world::reports::fold_in(&w.root, &w.state)
        .unwrap()
        .rows;
    let roster = w.roster();
    for (clock, len, ids) in &points {
        let s = ascension::propose_at(&rows, &roster[..*len], *clock);
        let now: Vec<(String, Move, String)> = s
            .proposals
            .iter()
            .map(|p| (p.creature.clone(), p.act, p.id.clone()))
            .collect();
        assert_eq!(&now, ids, "at {clock}");
    }
    // The same record, lived again in another state directory with the same applies: the same
    // proposals at the same points, and the same rows.
    let mut twin = World::new();
    twin.root.clone_from(&w.root);
    let _ = std::fs::remove_dir_all(&twin.state);
    std::fs::create_dir_all(&twin.state).unwrap();
    let adopt = creatures::adoption_in(Some(&twin.state), &twin.root);
    creatures::record_human_in(Some(&twin.state), &twin.root, adopt).unwrap();
    assert_eq!(scenario(&mut twin), points);
    let a: Vec<Row> = w.roster().iter().map(kept).collect();
    let b: Vec<Row> = twin.roster().iter().map(kept).collect();
    assert_eq!(a, b);
}

#[test]
fn a_tampered_journal_or_report_ledger_proposes_nothing() {
    let mut w = World::new();
    w.runs("sly", "slime", &[true; 22]);
    assert!(w.survey().of("sly").is_some());
    // The report ledger kept beside the journals, edited: nothing is proposed beside it.
    let (_, kept) = mnemorph_world::reports::record_in(&w.root, &w.state).unwrap();
    let text = std::fs::read_to_string(&kept).unwrap();
    std::fs::write(
        &kept,
        text.replacen("\"gate\":\"pass\"", "\"gate\":\"fail\"", 1),
    )
    .unwrap();
    let e = ascension::survey_in(&w.root, &w.state).unwrap_err();
    assert!(e.contains("report ledger's chain is broken"), "{e}");
    let e = ascension::apply_in(Some(&w.state), &w.root, "sly", Move::Rise, "x").unwrap_err();
    assert!(e.contains("report ledger"), "{e}");
    mnemorph_world::reports::record_in(&w.root, &w.state).unwrap();
    assert!(w.survey().of("sly").is_some(), "rebuilt from the journals");
    // A gate verdict edited in its journal: the fold is unavailable, and so is every proposal.
    let gate = state_file_in(&w.state, &w.root, "gate");
    let text = std::fs::read_to_string(&gate).unwrap();
    std::fs::write(&gate, text.replacen("\"PASS\"", "\"FAIL\"", 1)).unwrap();
    let e = ascension::survey_in(&w.root, &w.state).unwrap_err();
    assert!(e.contains("the gate chain is broken"), "{e}");
    assert_eq!(w.roster().len(), 3, "nothing was written");
}

#[test]
fn an_apply_is_held_to_the_proposal_shown_and_to_a_state_directory_outside_the_world() {
    let mut w = World::new();
    w.runs("sly", "slime", &[true; 22]);
    let p = w.survey().of("sly").cloned().unwrap();
    // A move the law does not propose, or an id that moved, writes nothing.
    let e = ascension::apply_in(Some(&w.state), &w.root, "sly", Move::Fall, &p.id).unwrap_err();
    assert!(e.contains("proposes a rise for sly, not a fall"), "{e}");
    let e = ascension::apply_in(Some(&w.state), &w.root, "mud", Move::Rise, &p.id).unwrap_err();
    assert!(e.contains("no rise for mud"), "{e}");
    let e = ascension::apply_in(Some(&w.state), &w.root, "sly", Move::Rise, "000000000000")
        .unwrap_err();
    assert!(e.contains("the proposal moved since it was shown"), "{e}");
    let e = ascension::apply_in(None, &w.root, "sly", Move::Rise, &p.id).unwrap_err();
    assert!(e.contains("no home"), "{e}");
    let inside = w.root.join(".mnemorph/tmp/state");
    let e = ascension::apply_in(Some(&inside), &w.root, "sly", Move::Rise, &p.id).unwrap_err();
    assert!(e.contains("lies inside the world"), "{e}");
    assert!(!inside.exists());
    assert_eq!(w.roster().len(), 3, "nothing was written");
    // The row is chained: it walks whole with the rest.
    ascension::apply_in(Some(&w.state), &w.root, "sly", Move::Rise, &p.id).unwrap();
    let at = creatures::path_in(&w.state, &w.root);
    let report = mnemorph_ledger::chain::verify_against(&at, None).unwrap();
    assert!(report.holds() && report.rows == 4, "{report}");
    assert_eq!(
        w.runs_as("sly").name(),
        "slime",
        "its card says slime: the lower"
    );
}
