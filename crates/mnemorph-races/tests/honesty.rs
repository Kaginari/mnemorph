//! Honest bad news never lowers a score: a body that says `@S FAIL`, `@S ESCALATE` or raises an
//! `@?` hole scores, rises and falls exactly as one that stays silent over the same outcomes; and
//! `holeRate` counts the holes the harness finds, never the body's own.

#![allow(
    clippy::float_cmp,
    reason = "the law reads exact readings; so do its tests"
)]

use mnemorph_races::{
    GateWord, Outcome, Race, Recent, Said, Signals, falls, fold_reports, meter, rises, score,
};
use proptest::prelude::*;

fn gate() -> impl Strategy<Value = Option<GateWord>> {
    prop_oneof![
        Just(None),
        Just(Some(GateWord::Pass)),
        Just(Some(GateWord::Fail)),
        Just(Some(GateWord::NotVerified)),
    ]
}

fn said() -> impl Strategy<Value = Said> {
    prop_oneof![
        Just(Said::Done),
        Just(Said::Fail),
        Just(Said::Escalate),
        Just(Said::Other),
    ]
}

fn outcome() -> impl Strategy<Value = Outcome> {
    (said(), 0u32..3, gate(), any::<bool>(), 0u32..2).prop_map(
        |(said, holes_said, gate, first_try, holes_found)| Outcome {
            said,
            holes_said,
            gate,
            first_try,
            holes_found,
        },
    )
}

/// What the rest of a record holds, outside the reports' fold.
fn rest() -> impl Strategy<Value = Signals> {
    (0u32..40, 0.0..=100.0f64, 0.0..=100.0f64, 0.0..=100.0f64).prop_map(|(findings, a, b, c)| {
        Signals {
            findings,
            outward_acts: findings,
            plan_land_rate: a,
            reask_rate: b,
            gate_right: c,
            slime_health: a,
            outward_clean_rate: b,
            verify_green_rate: c,
            upheld_rate: a,
            precision: b,
            ask_done_rate: c,
            ..Signals::default()
        }
    })
}

/// The record a race is judged on: the reports' fold over the rest.
fn record(outcomes: &[Outcome], rest: Signals) -> Signals {
    let f = fold_reports(outcomes);
    Signals {
        reports: f.reports,
        land_rate: f.land_rate,
        gate_rate: f.gate_rate,
        hole_rate: f.hole_rate,
        ..rest
    }
}

proptest! {
    /// Silent and honest over the same outcomes: the honest record never scores, ranks, rises or
    /// falls worse — for every race, whatever the gate said and whatever the harness found.
    #[test]
    fn honest_bad_news_never_lowers_a_score(
        mut outcomes in prop::collection::vec(outcome(), 1..30),
        pick in any::<prop::sample::Index>(),
        word in prop_oneof![Just(Said::Fail), Just(Said::Escalate), Just(Said::Done)],
        holes in 0u32..4,
        rest in rest(),
    ) {
        let i = pick.index(outcomes.len());
        outcomes[i].said = Said::Done;
        outcomes[i].holes_said = 0;
        let silent = record(&outcomes, rest);
        outcomes[i].said = word;
        outcomes[i].holes_said = holes;
        let honest = record(&outcomes, rest);
        for race in Race::ALL {
            prop_assert!(score(race, &honest) >= score(race, &silent), "{race}");
            prop_assert_eq!(meter(race, &honest).band, meter(race, &silent).band);
            prop_assert!(!rises(race, &silent) || rises(race, &honest), "{race}");
            prop_assert_eq!(falls(race, &Recent(honest)), falls(race, &Recent(silent)));
        }
    }

    /// `holeRate` is the harness's: the body's own `@?` never moves it, a hole the harness finds
    /// always does.
    #[test]
    fn hole_rate_counts_the_harness_holes_not_the_bodys(
        mut outcomes in prop::collection::vec(outcome(), 1..30),
        pick in any::<prop::sample::Index>(),
        said in 0u32..5,
    ) {
        let i = pick.index(outcomes.len());
        outcomes[i].holes_found = 0;
        let before = fold_reports(&outcomes).hole_rate;
        outcomes[i].holes_said = said;
        prop_assert_eq!(fold_reports(&outcomes).hole_rate, before);
        outcomes[i].holes_found = 1;
        prop_assert!(fold_reports(&outcomes).hole_rate > before);
    }
}

#[test]
fn the_fold_reads_the_gate_and_the_harness() {
    let o = |gate, first_try, holes_found| Outcome {
        said: Said::Done,
        holes_said: 0,
        gate,
        first_try,
        holes_found,
    };
    let s = fold_reports(&[
        o(Some(GateWord::Pass), true, 0),
        o(Some(GateWord::Pass), false, 0),
        o(Some(GateWord::Fail), true, 1),
        o(None, false, 0),
    ]);
    assert_eq!(
        (s.reports, s.land_rate, s.gate_rate, s.hole_rate),
        (4, 50.0, 25.0, 25.0)
    );
    // An edited verify line is the harness's hole; a body that says `FAIL` over a landing the
    // gate passed still landed.
    let honest = Outcome {
        said: Said::Fail,
        holes_said: 2,
        ..o(Some(GateWord::Pass), true, 0)
    };
    assert_eq!(fold_reports(&[honest]).land_rate, 100.0);
    assert_eq!(fold_reports(&[honest]).hole_rate, 0.0);
    let none = fold_reports(&[]);
    assert_eq!((none.reports, none.land_rate), (0, 0.0));
}
