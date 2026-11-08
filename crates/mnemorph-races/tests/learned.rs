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
        weights: [0.3, 0.0, -0.4, 0.2, 0.1, -0.2],
        bias: 0.15,
        ..Guts::default()
    };
    let counts = [2, 0, 1, 1, 0, 3];
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
        counts in proptest::array::uniform6(0u32..=5),
        landed in any::<bool>(),
        w in proptest::array::uniform6(-2.0f64..2.0),
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
