//! The learned guts: one logistic unit over the guts classes of a body's last acts, giving the
//! chance its run lands. The first learned model of mnemorph-research's value track, and the one
//! whose maths is proven there (`value-function-math.md`, §3–§5):
//!
//! - **The model** (§3): `p = σ(b + Σ θ_c x_c)`, trained on the cross-entropy, whose gradient is
//!   `(p − y)·x_c`. A confident wrong guess moves the weights most: the model learns most from its
//!   own mistakes, and a failed run is as much a lesson as a landed one.
//! - **Rules that never learn** (§4): the red pin's weight is 0 and stays 0. A red test during
//!   TDD is the right process.
//! - **The inputs** (§5): each class's count over the last [`WINDOW`] acts, brought to `[0, 1]`,
//!   scaled to `[−1, 1]` and centred on a running mean.
//! - **The step** (§5): normalised, `η = μ / (ε + ‖x‖² + 1)` with `μ < 1`, so one update moves
//!   the prediction toward its label and never past it, and the error is clipped.
//!
//! Nothing here reads a model's words, raises a creature or stops a run: it predicts, and the
//! prediction is shown beside the hand-set guts until it proves better (shadow mode).

/// How many inputs: one per guts class, in the order `fail`, `red-pin`, `positive`, `repeat`,
/// `echo`, `thrash`.
pub const INPUTS: usize = 6;
/// The red pin's input, whose weight is pinned at 0.
pub const RED_PIN: usize = 1;
/// The acts a window counts.
pub const WINDOW: usize = 5;
/// The normalised step's size: under 1, a step never passes its label.
pub const MU: f64 = 0.5;
/// How fast the running means follow the inputs.
pub const ALPHA: f64 = 0.01;

/// The model: a weight per input, a bias, and each input's running mean.
#[derive(Debug, Clone, PartialEq)]
pub struct Guts {
    /// One weight per input; the red pin's is 0.
    pub weights: [f64; INPUTS],
    /// The bias: the log-odds of landing when every input sits at its mean.
    pub bias: f64,
    /// Each input's running mean, which centres it.
    pub means: [f64; INPUTS],
    /// The examples it has learned from.
    pub seen: u64,
}

impl Default for Guts {
    fn default() -> Self {
        Guts {
            weights: [0.0; INPUTS],
            bias: 0.0,
            // A quiet window, every class absent, scales to −1: the starting centre.
            means: [-1.0; INPUTS],
            seen: 0,
        }
    }
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

/// A window's counts, scaled to `[−1, 1]`.
fn scaled(counts: &[u32; INPUTS]) -> [f64; INPUTS] {
    let window = f64::from(u32::try_from(WINDOW).unwrap_or(u32::MAX));
    counts.map(|c| 2.0 * (f64::from(c) / window).clamp(0.0, 1.0) - 1.0)
}

impl Guts {
    /// The inputs the model reads for a window: scaled and centred, the red pin's included.
    #[must_use]
    pub fn inputs(&self, counts: &[u32; INPUTS]) -> [f64; INPUTS] {
        let u = scaled(counts);
        std::array::from_fn(|c| u[c] - self.means[c])
    }

    /// The chance a run whose last acts gave `counts` lands.
    #[must_use]
    pub fn predict(&self, counts: &[u32; INPUTS]) -> f64 {
        let x = self.inputs(counts);
        let z = self.bias + (0..INPUTS).map(|c| self.weights[c] * x[c]).sum::<f64>();
        sigmoid(z)
    }

    /// One step on one example: the window `counts` of a run that `landed` or not.
    pub fn learn(&mut self, counts: &[u32; INPUTS], landed: bool) {
        let x = self.inputs(counts);
        let y = if landed { 1.0 } else { 0.0 };
        let error = (self.predict(counts) - y).clamp(-1.0, 1.0);
        let norm = 1.0
            + (0..INPUTS)
                .filter(|c| *c != RED_PIN)
                .map(|c| x[c] * x[c])
                .sum::<f64>();
        let step = MU / (f64::EPSILON + norm);
        for c in (0..INPUTS).filter(|c| *c != RED_PIN) {
            self.weights[c] -= step * error * x[c];
        }
        self.bias -= step * error;
        for (mean, u) in self.means.iter_mut().zip(scaled(counts)) {
            *mean += ALPHA * (u - *mean);
        }
        self.seen += 1;
    }
}

/// The batch fit's passes over its examples.
pub const BATCH_PASSES: usize = 500;
/// The batch fit's step on the mean gradient: the inputs lie in `[−2, 2]`, so it stays stable.
pub const BATCH_STEP: f64 = 0.5;
/// The batch fit's pull of each weight toward 0: enough to keep a weight finite on separable
/// data, too little to hide a real signal.
pub const BATCH_L2: f64 = 0.01;

impl Guts {
    /// Fits the model to every example at once: full-batch gradient descent on the mean
    /// cross-entropy, with a light pull toward 0, inputs centred on the examples' own mean, the
    /// red pin's weight left at 0. Unlike [`Guts::learn`], the order of the examples does not
    /// matter: one step of 0.5 per act, run after run, left a model that said whatever the last
    /// runs had said.
    pub fn fit_batch(&mut self, examples: &[([u32; INPUTS], bool)]) {
        if examples.is_empty() {
            return;
        }
        let n = f64::from(u32::try_from(examples.len()).unwrap_or(u32::MAX));
        let mut means = [0.0; INPUTS];
        for (counts, _) in examples {
            for (m, u) in means.iter_mut().zip(scaled(counts)) {
                *m += u / n;
            }
        }
        self.means = means;
        let xs: Vec<([f64; INPUTS], f64)> = examples
            .iter()
            .map(|(c, landed)| (self.inputs(c), if *landed { 1.0 } else { 0.0 }))
            .collect();
        for _ in 0..BATCH_PASSES {
            let mut grad = [0.0; INPUTS];
            let mut grad_bias = 0.0;
            for (x, y) in &xs {
                let z = self.bias + (0..INPUTS).map(|c| self.weights[c] * x[c]).sum::<f64>();
                let error = sigmoid(z) - y;
                for c in 0..INPUTS {
                    grad[c] += error * x[c] / n;
                }
                grad_bias += error / n;
            }
            for c in (0..INPUTS).filter(|c| *c != RED_PIN) {
                self.weights[c] -= BATCH_STEP * (grad[c] + BATCH_L2 * self.weights[c]);
            }
            self.bias -= BATCH_STEP * grad_bias;
        }
        self.seen += examples.len() as u64;
    }
}

/// How well predictions held against what happened, beside the base rate they must beat.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    /// The predictions scored.
    pub runs: usize,
    /// The share that landed: what a model that knows nothing would say every time.
    pub base_rate: f64,
    /// The base rate's Brier score.
    pub base_brier: f64,
    /// The model's Brier score: the mean squared gap between its chance and what happened.
    pub brier: f64,
    /// The model's mean cross-entropy.
    pub log_loss: f64,
}

/// The 90% interval of the gain over the base rate (its Brier less the model's), resampling
/// whole runs, never acts: the acts of one run are not independent. `None` with fewer than two
/// runs. Deterministic: the same runs give the same interval.
#[must_use]
pub fn gain_interval(runs: &[Vec<(f64, bool)>]) -> Option<(f64, f64)> {
    if runs.len() < 2 {
        return None;
    }
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = |bound: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(state >> 33).unwrap_or(0) % bound
    };
    let mut gains: Vec<f64> = (0..1000)
        .map(|_| {
            let pairs: Vec<(f64, bool)> = (0..runs.len())
                .flat_map(|_| runs[next(runs.len())].iter().copied())
                .collect();
            let s = Score::of(pairs);
            s.base_brier - s.brier
        })
        .collect();
    gains.sort_by(f64::total_cmp);
    Some((gains[50], gains[949]))
}

impl Score {
    /// The score of `(chance, landed)` pairs.
    pub fn of(pairs: impl IntoIterator<Item = (f64, bool)>) -> Score {
        let pairs: Vec<(f64, f64)> = pairs
            .into_iter()
            .map(|(p, y)| (p.clamp(1e-9, 1.0 - 1e-9), if y { 1.0 } else { 0.0 }))
            .collect();
        let n = f64::from(u32::try_from(pairs.len().max(1)).unwrap_or(u32::MAX));
        let base_rate = pairs.iter().map(|(_, y)| y).sum::<f64>() / n;
        Score {
            runs: pairs.len(),
            base_rate,
            base_brier: pairs
                .iter()
                .map(|(_, y)| (base_rate - y).powi(2))
                .sum::<f64>()
                / n,
            brier: pairs.iter().map(|(p, y)| (p - y).powi(2)).sum::<f64>() / n,
            log_loss: pairs
                .iter()
                .map(|(p, y)| -(y * p.ln() + (1.0 - y) * (1.0 - p).ln()))
                .sum::<f64>()
                / n,
        }
    }
}
