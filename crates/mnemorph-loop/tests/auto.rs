//! Evolution on its own: a due creature is drafted, proven and swapped in on trial; once its
//! window of 10 runs is full it is judged and settled by the rule — kept, or rolled back to the
//! doc it had. One judgement and one draft a tick; `propose` mode does nothing.

use std::path::PathBuf;

use mnemorph_ledger::evolution::{self as ledger, EVOLUTION_WINDOW, EvolutionRow, Judgement};
use mnemorph_ledger::{Build, Journal, Timestamp, Usage, UsageRow};
use mnemorph_loop::auto::{self, Ticker};
use mnemorph_loop::draft;
use mnemorph_loop::model::ScriptedModel;
use mnemorph_world::evolution::{Facts, Plan, ledger_of, stamp_for};
use mnemorph_world::evolve::{Due, desk};
use mnemorph_world::queue::state_file_in;
use mnemorph_world::verdicts::{self, VerdictRow};
use mnemorph_world::{creatures, proposals};

const DOC: &str = ".mnemorph/slime/sly/README.md";
const CARD: &str = "# sly\n\n- **Race:** slime\n- **Reports to:** rimuru\n- **Territory:** src/\n\nKeeps the parser.\n";

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
}

impl World {
    /// Rimuru and sly, adopted; sly stressed — its last 3 verdicts failed — its desk full.
    fn due() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("world");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        for (seat, card) in [
            ("rimuru/rimuru", "# rimuru\n\n- **Race:** rimuru\n"),
            ("slime/sly", CARD),
        ] {
            let at = root.join(".mnemorph").join(seat);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("README.md"), card).unwrap();
        }
        let state = mnemorph_tools::dirs::state_home().expect("the test env sets a state home");
        let rows = creatures::adoption_in(Some(&state), &root);
        creatures::record_human_in(Some(&state), &root, rows).unwrap();
        let w = World {
            _dir: dir,
            root,
            state,
        };
        for n in 0..3 {
            w.verdict(n, "FAIL");
        }
        for i in 0..8 {
            mnemorph_memory::propose(
                &w.root,
                "sly",
                &format!("lesson {i}: fractions must stay exact"),
                vec!["law".into()],
                None,
            )
            .unwrap();
        }
        w
    }

    fn verdict(&self, n: i64, word: &str) {
        let run = format!("sly-{n}");
        let ts = 1_793_000_000 + n * 3600;
        Journal::<UsageRow>::chained(state_file_in(&self.state, &self.root, "usage"))
            .append(&UsageRow {
                ts: Timestamp(ts),
                session: "s".into(),
                run: run.clone(),
                body: "sly".into(),
                race: "slime".into(),
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
                ts: Timestamp(ts + 60),
                body: "sly".into(),
                verdict: word.into(),
                changed: vec!["src/a.rs".into()],
                failures: vec!["duties done: the answer is empty; say what was done".into()],
                unverified: Vec::new(),
                unchecked: false,
                verified_by: vec!["the human's pin".into()],
                entry: None,
                build: Build::default(),
                run: Some(run),
            })
            .unwrap();
    }

    fn doc(&self) -> String {
        std::fs::read_to_string(self.root.join(DOC)).unwrap()
    }

    fn rows(&self) -> Vec<EvolutionRow> {
        ledger::read(&ledger_of(&self.root).unwrap(), None)
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.row)
            .collect()
    }

    /// The scripted drafter: one trait, citing the first note on sly's desk.
    fn drafter(&self) -> impl Fn(&Due) -> Result<draft::Draft, String> + '_ {
        move |due: &Due| {
            let id = desk(&self.root, "sly")[0].id.clone();
            let model = ScriptedModel::new([ScriptedModel::text(&format!(
                "@T keep fractions exact from={id}"
            ))]);
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            rt.block_on(draft::draft(
                &self.root,
                &model,
                "scripted",
                due,
                &Journal::chained(self.root.join("usage.jsonl")),
                "s",
                1_793_100_000,
            ))
        }
    }

    /// Runs `from..to` of sly's in the open window, each a task of its own, each stamped.
    fn runs(&self, from: usize, to: usize) {
        let at = ledger_of(&self.root).unwrap();
        for i in from..to {
            let s = stamp_for(&self.root, &at, "sly", &format!("held-out task {i}"))
                .expect("stamped in the window");
            s.record(&self.root, Facts::default()).unwrap();
        }
    }
}

fn never_judged(_: &str) -> Result<Judgement, String> {
    panic!("nothing waits to be judged")
}

#[allow(clippy::unnecessary_wraps, reason = "the shape a Ticker's proof takes")]
fn proven(_: &Plan) -> auto::Proof {
    Ok(vec!["proven".into()])
}

#[test]
fn a_due_creature_evolves_on_trial_and_is_kept_by_the_rule() {
    let w = World::due();
    let cited = desk(&w.root, "sly")[0].id.clone();
    let drafter = w.drafter();
    let lines = auto::tick(
        &w.root,
        &Ticker {
            draft: &drafter,
            judge: &never_judged,
            prove: &proven,
        },
    )
    .unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("sly evolved by mutate") && l.contains("on trial for 10 runs")),
        "{lines:?}"
    );
    assert!(
        w.doc().contains("- *trait* — keep fractions exact"),
        "swapped in"
    );
    assert!(
        proposals::open(&w.root).unwrap().is_empty(),
        "its proposal closed"
    );
    // On trial: no second evolution, no judgement, until its window is full.
    let no_draft = |_: &Due| -> Result<draft::Draft, String> { panic!("one evolution at a time") };
    let t = Ticker {
        draft: &no_draft,
        judge: &never_judged,
        prove: &proven,
    };
    w.runs(0, EVOLUTION_WINDOW - 1);
    assert_eq!(auto::tick(&w.root, &t).unwrap(), Vec::<String>::new());
    // The window full — its runs passed the gate, so sly is stressed no more — judged, and kept
    // by the rule.
    w.runs(EVOLUTION_WINDOW - 1, EVOLUTION_WINDOW);
    for n in 3..6 {
        w.verdict(n, "PASS");
    }
    let keep = |id: &str| -> Result<Judgement, String> {
        assert!(id.starts_with("sly-mutate-"));
        Ok(Judgement::Keep(Vec::new()))
    };
    let lines = auto::tick(
        &w.root,
        &Ticker {
            draft: &no_draft,
            judge: &keep,
            prove: &proven,
        },
    )
    .unwrap();
    assert!(
        lines.iter().any(|l| l.contains(": keep by the rule")),
        "{lines:?}"
    );
    assert!(w.doc().contains("keep fractions exact"), "kept");
    // The note its trait distils leaves the desk, asleep: the trait carries it now.
    let left = desk(&w.root, "sly");
    assert_eq!(left.len(), 7, "{left:?}");
    assert!(!left.iter().any(|n| n.id == cited), "{left:?}");
    assert!(
        w.rows()
            .iter()
            .any(|r| matches!(r, EvolutionRow::Settle(t) if t.by == "harness")),
        "settled by the harness"
    );
}

#[test]
fn an_evolution_that_does_not_beat_doing_nothing_is_rolled_back() {
    let w = World::due();
    let drafter = w.drafter();
    auto::tick(
        &w.root,
        &Ticker {
            draft: &drafter,
            judge: &never_judged,
            prove: &proven,
        },
    )
    .unwrap();
    w.runs(0, EVOLUTION_WINDOW);
    let rollback = |_: &str| -> Result<Judgement, String> { Ok(Judgement::Rollback(Vec::new())) };
    let no_draft = |_: &Due| -> Result<draft::Draft, String> {
        Err("sly is drafted again only when due again".into())
    };
    let lines = auto::tick(
        &w.root,
        &Ticker {
            draft: &no_draft,
            judge: &rollback,
            prove: &proven,
        },
    )
    .unwrap();
    assert!(
        lines.iter().any(|l| l.contains(": rollback by the rule")),
        "{lines:?}"
    );
    assert_eq!(w.doc(), CARD, "put back as it was");
    assert_eq!(
        desk(&w.root, "sly").len(),
        8,
        "rolled back, its notes stay on the desk"
    );
}

#[test]
fn a_judgement_still_pending_settles_nothing() {
    let w = World::due();
    let drafter = w.drafter();
    let t = Ticker {
        draft: &drafter,
        judge: &never_judged,
        prove: &proven,
    };
    auto::tick(&w.root, &t).unwrap();
    w.runs(0, EVOLUTION_WINDOW);
    // A cap stopped the controls: pending, said, nothing settled.
    let pending = |_: &str| -> Result<Judgement, String> {
        Ok(Judgement::Pending {
            runs: 10,
            controls: 4,
        })
    };
    let no_draft = |_: &Due| -> Result<draft::Draft, String> { Err("busy".into()) };
    let lines = auto::tick(
        &w.root,
        &Ticker {
            draft: &no_draft,
            judge: &pending,
            prove: &proven,
        },
    )
    .unwrap();
    assert!(
        lines.iter().any(|l| l.contains("left as it stands")),
        "{lines:?}"
    );
    assert!(
        !w.rows()
            .iter()
            .any(|r| matches!(r, EvolutionRow::Settle(_)))
    );
}

#[test]
fn a_draft_not_proven_is_left_for_the_human() {
    let w = World::due();
    let drafter = w.drafter();
    let unproven =
        |_: &Plan| -> Result<Vec<String>, Vec<String>> { Err(vec!["a new finding".into()]) };
    let lines = auto::tick(
        &w.root,
        &Ticker {
            draft: &drafter,
            judge: &never_judged,
            prove: &unproven,
        },
    )
    .unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("not proven — left for the human: a new finding")),
        "{lines:?}"
    );
    assert_eq!(w.doc(), CARD, "nothing swapped");
    assert_eq!(
        proposals::open(&w.root).unwrap().len(),
        1,
        "the proposal waits"
    );
    // A creature with a proposal waiting is not drafted again.
    let no_draft = |_: &Due| -> Result<draft::Draft, String> { panic!("its proposal waits") };
    assert_eq!(
        auto::tick(
            &w.root,
            &Ticker {
                draft: &no_draft,
                judge: &never_judged,
                prove: &proven,
            },
        )
        .unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn propose_mode_evolves_nothing_on_its_own_and_is_the_humans_alone() {
    let w = World::due();
    let global = w.root.join("../global.toml");
    std::fs::write(&global, "[evolution]\nmode = \"propose\"\n").unwrap();
    let paths = mnemorph_config::Paths::with_global(global, &w.root);
    let lines = auto::tick_in(&w.root, &paths, &|_| None, &|_| {}).unwrap();
    assert_eq!(
        lines,
        vec!["evolution.mode is `propose`: nothing evolves on its own"]
    );
    assert_eq!(w.doc(), CARD);
    // A world's file — a body's to write — never sets it.
    std::fs::write(
        w.root.join(".mnemorph/config.toml"),
        "[evolution]\nmode = \"propose\"\n",
    )
    .unwrap();
    let empty = w.root.join("../empty.toml");
    std::fs::write(&empty, "").unwrap();
    let c = mnemorph_config::Config::load(&mnemorph_config::Paths::with_global(empty, &w.root))
        .unwrap();
    assert!(!c.evolution_proposes(), "the world's word is a hole");
    assert!(
        c.holes().iter().any(|h| h.key == "evolution.mode"),
        "{:?}",
        c.holes()
    );
}
