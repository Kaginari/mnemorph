//! The learned guts against the maths it rests on (mnemorph-research, `value-function-math.md`):
//! the gradient of the cross-entropy is (p − y)·x (§3), the red pin's weight stays 0 (§4), a
//! normalised step never overshoots (§5), and a model fitted on runs that differ beats the base
//! rate they share.

use mnemorph_races::learned::{Guts, INPUTS, RED_PIN, Score};
use proptest::prelude::*;

/// Window counts: `fail` acts out of the last five, the rest quiet.
fn fails(n: u32) -> [u32; INPUTS] {
    let mut c = [0; INPUTS];
    c[0] = n;
    c
}

fn loss(p: f64, y: f64) -> f64 {
    -(y * p.ln() + (1.0 - y) * (1.0 - p).ln())
}

#[test]
fn the_gradient_is_the_error_times_the_input() {
    let g = Guts {
        weights: [0.3, 0.0, -0.4, 0.2, 0.1, -0.2, -0.3, 0.5, -0.1],
        bias: 0.15,
        ..Guts::default()
    };
    let counts = [2, 0, 1, 1, 0, 3, 1, 40, 7];
    let x = g.inputs(&counts);
    let p = g.predict(&counts);
    for y in [0.0, 1.0] {
        for (c, xc) in x.iter().enumerate() {
            let h = 1e-6;
            let mut up = g.clone();
            up.weights[c] += h;
            let mut down = g.clone();
            down.weights[c] -= h;
            let numeric =
                (loss(up.predict(&counts), y) - loss(down.predict(&counts), y)) / (2.0 * h);
            let analytic = (p - y) * xc;
            assert!(
                (numeric - analytic).abs() < 1e-6,
                "input {c}, y {y}: {numeric} against {analytic}"
            );
        }
    }
}

#[test]
fn the_red_pin_never_learns() {
    let mut g = Guts::default();
    let mut counts = [0; INPUTS];
    counts[RED_PIN] = 4;
    for i in 0..500 {
        g.learn(&counts, i % 3 == 0);
    }
    assert!(g.weights[RED_PIN].abs() < f64::EPSILON, "{:?}", g.weights);
}

proptest! {
    /// One normalised step moves the prediction toward the label and never past it.
    #[test]
    fn a_step_never_overshoots(
        counts in proptest::array::uniform9(0u32..=5),
        landed in any::<bool>(),
        w in proptest::array::uniform9(-2.0f64..2.0),
        b in -2.0f64..2.0,
    ) {
        let mut g = Guts {
            weights: w,
            bias: b,
            ..Guts::default()
        };
        g.weights[RED_PIN] = 0.0;
        let y = if landed { 1.0 } else { 0.0 };
        let before = g.predict(&counts);
        let means = g.means;
        g.learn(&counts, landed);
        // The same example, read against the means it was learned with.
        let mut again = g.clone();
        again.means = means;
        let after = again.predict(&counts);
        prop_assert!((after - y).abs() <= (before - y).abs() + 1e-12, "{before} -> {after}, y {y}");
    }
}

#[test]
fn runs_that_failed_teach_as_much_as_runs_that_landed() {
    // Runs that failed came with failing acts; runs that landed, with few. The model learns that
    // from both: trained on the two, it beats the base rate on runs it never saw.
    let train: Vec<([u32; INPUTS], bool)> = (0..400u32)
        .map(|i| {
            let landed = i % 2 == 0;
            let n = if landed { i % 2 } else { 3 + i % 3 };
            (fails(n.min(5)), landed)
        })
        .collect();
    let mut g = Guts::default();
    for (c, y) in &train {
        g.learn(c, *y);
    }
    let held: Vec<([u32; INPUTS], bool)> = vec![
        (fails(0), true),
        (fails(1), true),
        (fails(4), false),
        (fails(5), false),
    ];
    let score = Score::of(held.iter().map(|(c, y)| (g.predict(c), *y)));
    assert!(score.brier < score.base_brier, "{score:?}");
    assert!(
        g.weights[0] < 0.0,
        "failing acts lower the chance: {:?}",
        g.weights
    );
}

#[test]
fn a_model_that_saw_only_failures_learns_no_difference() {
    // Every label the same: the loss is least at a constant, so no input gains weight it could
    // tell runs apart with. Failures teach when there is something to tell them from.
    let mut g = Guts::default();
    for i in 0..400 {
        g.learn(&fails(i % 6), false);
    }
    let spread = g.predict(&fails(0)) - g.predict(&fails(5));
    assert!(spread.abs() < 0.05, "{spread}");
    assert!(g.predict(&fails(2)) < 0.2);
}

#[test]
fn the_score_names_the_base_rate_it_must_beat() {
    let s = Score::of([(0.9, true), (0.2, false), (0.7, true), (0.4, false)]);
    assert_eq!(s.runs, 4);
    assert!((s.base_rate - 0.5).abs() < 1e-12);
    assert!((s.base_brier - 0.25).abs() < 1e-12);
    assert!((s.brier - (0.01 + 0.04 + 0.09 + 0.16) / 4.0).abs() < 1e-12);
}

/// The batch fit keeps a signal however the runs are ordered: failing runs' windows full of
/// failures, landed runs' quiet, and the last fifty runs all landed. One step of 0.5 per act ended
/// saying whatever those last runs said.
#[test]
fn the_batch_fit_keeps_the_signal_whatever_the_order() {
    use mnemorph_races::learned::{Guts, INPUTS};
    let failing: [u32; INPUTS] = [4, 0, 0, 1, 0, 0, 2, 50, 10];
    let quiet: [u32; INPUTS] = [0, 0, 2, 0, 0, 0, 0, 50, 10];
    let mut examples = Vec::new();
    for _ in 0..30 {
        examples.push((failing, false));
        examples.push((quiet, true));
    }
    for _ in 0..50 {
        examples.push((quiet, true));
    }
    let mut ordered = Guts::default();
    ordered.fit_batch(&examples);
    let mut reversed = Guts::default();
    examples.reverse();
    reversed.fit_batch(&examples);
    assert!(
        ordered.predict(&failing) < 0.2,
        "{}",
        ordered.predict(&failing)
    );
    assert!(ordered.predict(&quiet) > 0.8, "{}", ordered.predict(&quiet));
    for c in 0..INPUTS {
        assert!(
            (ordered.weights[c] - reversed.weights[c]).abs() < 1e-9,
            "weight {c}"
        );
    }
    assert!(
        ordered.weights[1].abs() < f64::EPSILON,
        "the red pin stays at 0"
    );
}

/// The gain's interval resamples whole runs: a model that says the base rate every time gains
/// nothing, and its interval holds 0.
#[test]
fn a_model_that_knows_nothing_gains_nothing() {
    use mnemorph_races::learned::gain_interval;
    let runs: Vec<Vec<(f64, bool)>> = (0..10)
        .map(|i| vec![(0.6, i % 5 != 0), (0.6, i % 5 != 0)])
        .collect();
    let (lo, hi) = gain_interval(&runs).unwrap();
    assert!(lo <= 0.0 && hi >= -0.05, "{lo} {hi}");
    assert!(gain_interval(&runs[..1]).is_none());
}

/// A batch fit on no examples leaves the model as it was: nothing to learn from is no change.
#[test]
fn a_batch_fit_on_nothing_changes_nothing() {
    use mnemorph_races::learned::Guts;
    let mut g = Guts::default();
    g.fit_batch(&[]);
    assert_eq!(g, Guts::default());
}

/// Each reading is brought to `[0, 1]` by its own range (§5): a class or an edit miss by the
/// window, the rows' share by 100, the turns since the failing rows shrank by their cap. A world
/// with no rows reads the middle of both rows ranges, which scales to 0: no evidence either way.
#[test]
fn every_reading_scales_by_its_own_range() {
    use mnemorph_races::learned::{EDIT_MISS, NO_ROWS, ROWS_PASSING, SINCE_CAP, SINCE_SHRANK};
    let centred = Guts {
        means: [0.0; INPUTS],
        ..Guts::default()
    };
    let mut w = [0u32; INPUTS];
    w[EDIT_MISS] = 5;
    w[ROWS_PASSING] = NO_ROWS.0;
    w[SINCE_SHRANK] = NO_ROWS.1;
    let x = centred.inputs(&w);
    assert!(
        (x[EDIT_MISS] - 1.0).abs() < 1e-12,
        "five misses in five acts"
    );
    assert!(
        x[ROWS_PASSING].abs() < 1e-12 && x[SINCE_SHRANK].abs() < 1e-12,
        "{x:?}"
    );
    w[ROWS_PASSING] = 100;
    w[SINCE_SHRANK] = SINCE_CAP * 3;
    let x = centred.inputs(&w);
    assert!((x[ROWS_PASSING] - 1.0).abs() < 1e-12 && (x[SINCE_SHRANK] - 1.0).abs() < 1e-12);
}

/// The reference counts calls and nothing else. Fitted where failed runs run long, it says a late
/// act lands less often than an early one: what a learned guts must beat before it advises.
#[test]
fn the_reference_learns_only_the_clock() {
    use mnemorph_races::learned::Reference;
    // Landed runs end by call 40; failed runs go on to 200.
    let examples: Vec<(u32, bool)> = (1..=40)
        .map(|c| (c, true))
        .chain((1..=200).map(|c| (c, false)))
        .collect();
    let mut clock = Reference::default();
    clock.fit_batch(&examples);
    assert!(
        clock.predict(10) > clock.predict(150) + 0.1,
        "{} against {}",
        clock.predict(10),
        clock.predict(150)
    );
    assert!(clock.predict(150) < 0.25, "{}", clock.predict(150));
    assert_eq!(
        clock.predict(200),
        clock.predict(5000),
        "past the cap, later is no later"
    );
    // Nothing to fit changes nothing.
    let mut untouched = Reference::default();
    untouched.fit_batch(&[]);
    assert_eq!(untouched, Reference::default());
}

/// The gain over the reference is paired: each resample scores both models on the same units.
/// A model that says what the reference says gains exactly nothing; one that is right where the
/// reference only guesses gains with its whole interval above 0.
#[test]
fn a_gain_over_the_reference_is_paired_by_unit() {
    use mnemorph_races::learned::gain_over;
    let same: Vec<Vec<(f64, f64, bool)>> =
        (0..10).map(|i| vec![(0.6, 0.6, i % 2 == 0); 4]).collect();
    let (lo, hi) = gain_over(&same).unwrap();
    assert!(lo.abs() < 1e-12 && hi.abs() < 1e-12, "{lo} to {hi}");
    let better: Vec<Vec<(f64, f64, bool)>> = (0..10)
        .map(|i| {
            let landed = i % 2 == 0;
            vec![(if landed { 0.9 } else { 0.1 }, 0.5, landed); 4]
        })
        .collect();
    let (lo, _) = gain_over(&better).unwrap();
    assert!(lo > 0.0, "{lo}");
    assert_eq!(
        gain_over(&better[..1]),
        None,
        "one unit cannot be resampled"
    );
    // Units that hold no act score nothing: the gain is 0, never a division by nothing.
    assert_eq!(gain_over(&[Vec::new(), Vec::new()]), Some((0.0, 0.0)));
}
