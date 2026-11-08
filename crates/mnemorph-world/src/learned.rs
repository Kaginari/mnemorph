//! The learned guts' examples and models: each run's acts as the trusted guts journal holds them,
//! labelled with whether the gate passed the run, as the report ledger folds it. Both journals are
//! the harness's, under the human's state directory, where no body writes. The model itself is
//! [`crate::races::learned`]; this module only reads the record, splits it and keeps the fit.
//!
//! Every act of a run is an example, labelled with the run's outcome, so a run that failed teaches
//! at every act, as one that landed does. A run the gate could not judge — no check ran, or it
//! never reached the gate — has no label and teaches nothing: counting it as failed would teach
//! the model that every unchecked run fails. A bench, which knows each task's hidden tests, may
//! label its runs itself. A run is held out by a stable hash of its id, never by
//! chance, so a fit scored again scores the same runs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mnemorph_ledger::reports::{Gate, ReportRow};
use serde::{Deserialize, Serialize};

use crate::guts::{self, Class};
use crate::races::learned::{Guts, INPUTS, Score, WINDOW};

/// One run, as the learned guts reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The run's id.
    pub run: String,
    /// The race it ran as.
    pub race: String,
    /// Each act's window: the count of each class over the last [`WINDOW`] acts, this one
    /// included, in the order `fail`, `red-pin`, `positive`, `repeat`, `echo`, `thrash`.
    pub windows: Vec<[u32; INPUTS]>,
    /// The gate passed it (`true`) or failed it (`false`); `None` when it could not judge it.
    pub landed: Option<bool>,
}

fn input(class: Class) -> usize {
    match class {
        Class::Fail => 0,
        Class::RedPin => 1,
        Class::Positive => 2,
        Class::Repeat => 3,
        Class::Echo => 4,
        Class::Thrash => 5,
    }
}

/// Each act's window over a run's acts in order.
#[must_use]
pub fn windows(acts: &[&[Class]]) -> Vec<[u32; INPUTS]> {
    (0..acts.len())
        .map(|i| {
            let mut counts = [0; INPUTS];
            for act in &acts[i.saturating_sub(WINDOW - 1)..=i] {
                for class in *act {
                    counts[input(*class)] += 1;
                }
            }
            counts
        })
        .collect()
}

/// `world`'s runs: every run the guts journal holds acts of and the report ledger settled.
///
/// # Errors
///
/// No state directory, or a journal whose chain does not hold.
pub fn runs(world: &Path) -> Result<Vec<Run>, String> {
    let rows = guts::held(world)?;
    let acts = guts::by_run(&rows);
    let folded = crate::reports::fold(world).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in &folded.rows {
        let ReportRow::Run(r) = row else { continue };
        let Some(acts) = acts.get(r.run.as_str()) else {
            continue;
        };
        out.push(Run {
            run: r.run.clone(),
            race: r.race.clone(),
            windows: windows(acts),
            landed: match r.gate {
                Some(Gate::Pass) => Some(true),
                Some(Gate::Fail) => Some(false),
                Some(Gate::NotVerified) | None => None,
            },
        });
    }
    Ok(out)
}

/// A run is held out for scoring when its id hashes into the last [`HELD_OUT`] tenths.
pub const HELD_OUT: u64 = 3;

/// Whether `run` is held out of training, for scoring.
#[must_use]
pub fn held_out(run: &str) -> bool {
    mnemorph_wire::fnv64a(run.as_bytes()) % 10 < HELD_OUT
}

/// One race's fit: the model, its score on the held-out runs' acts, and how many runs each side
/// had.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    /// The model, trained on every act of the training runs, in their order.
    pub model: Guts,
    /// Its score on the held-out runs' acts; `None` with none held out.
    pub score: Option<Score>,
    /// Training runs.
    pub trained: usize,
    /// Held-out runs.
    pub held: usize,
}

/// A fit per race over `runs`, starting from `start`'s model of that race when it has one.
#[must_use]
pub fn fit(runs: &[Run], start: &BTreeMap<String, Guts>) -> BTreeMap<String, Fit> {
    let mut by_race: BTreeMap<&str, Vec<&Run>> = BTreeMap::new();
    for r in runs.iter().filter(|r| r.landed.is_some()) {
        by_race.entry(r.race.as_str()).or_default().push(r);
    }
    by_race
        .into_iter()
        .map(|(race, rs)| {
            let mut model = start.get(race).cloned().unwrap_or_default();
            let (held, trained): (Vec<&&Run>, Vec<&&Run>) =
                rs.iter().partition(|r| held_out(&r.run));
            for r in &trained {
                let landed = r.landed == Some(true);
                for w in &r.windows {
                    model.learn(w, landed);
                }
            }
            let judged = &model;
            let pairs: Vec<(f64, bool)> = held
                .iter()
                .flat_map(|r| {
                    let landed = r.landed == Some(true);
                    r.windows.iter().map(move |w| (judged.predict(w), landed))
                })
                .collect();
            let score = (!pairs.is_empty()).then(|| Score::of(pairs));
            let fit = Fit {
                model,
                score,
                trained: trained.len(),
                held: held.len(),
            };
            (race.to_owned(), fit)
        })
        .collect()
}

/// The kept models, one per race, as the human's state directory holds them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Kept {
    races: BTreeMap<String, KeptGuts>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct KeptGuts {
    weights: [f64; INPUTS],
    bias: f64,
    means: [f64; INPUTS],
    seen: u64,
}

/// The human's state directory, where the kept models live; `None` with no home.
#[must_use]
pub fn state() -> Option<PathBuf> {
    crate::queue::state_dir()
}

/// Where the kept models live: `$XDG_STATE_HOME/mnemorph/learned/guts.json`, outside every world.
#[must_use]
pub fn kept_at(state: &Path) -> PathBuf {
    state.join("mnemorph").join("learned").join("guts.json")
}

/// The models kept at `state`, by race: none when nothing was kept.
///
/// # Errors
///
/// The file is there and cannot be read.
pub fn load(state: &Path) -> Result<BTreeMap<String, Guts>, String> {
    let path = kept_at(state);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let kept: Kept = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(kept
        .races
        .into_iter()
        .map(|(race, k)| {
            let mut g = Guts {
                weights: k.weights,
                bias: k.bias,
                means: k.means,
                seen: k.seen,
            };
            // The red pin never learns, whatever a file says.
            g.weights[crate::races::learned::RED_PIN] = 0.0;
            (race, g)
        })
        .collect())
}

/// Keep `models` at `state`, replacing what was kept.
///
/// # Errors
///
/// The file cannot be written.
pub fn save(state: &Path, models: &BTreeMap<String, Guts>) -> Result<PathBuf, String> {
    let path = kept_at(state);
    let kept = Kept {
        races: models
            .iter()
            .map(|(race, g)| {
                let k = KeptGuts {
                    weights: g.weights,
                    bias: g.bias,
                    means: g.means,
                    seen: g.seen,
                };
                (race.clone(), k)
            })
            .collect(),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&kept).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_counts_the_last_five_acts() {
        let acts: Vec<&[Class]> = vec![
            &[Class::Fail],
            &[Class::Fail, Class::Repeat],
            &[],
            &[Class::Positive],
            &[Class::Fail],
            &[Class::Echo],
        ];
        let w = windows(&acts);
        assert_eq!(w.len(), 6);
        assert_eq!(w[1], [2, 0, 0, 1, 0, 0]);
        assert_eq!(w[4], [3, 0, 1, 1, 0, 0]);
        // The first act has left the window.
        assert_eq!(w[5], [2, 0, 1, 1, 1, 0]);
    }

    #[test]
    fn a_run_is_held_out_the_same_way_every_time() {
        let held: Vec<bool> = (0..200).map(|i| held_out(&format!("r{i}"))).collect();
        let again: Vec<bool> = (0..200).map(|i| held_out(&format!("r{i}"))).collect();
        assert_eq!(held, again);
        let n = held.iter().filter(|h| **h).count();
        assert!((40..=80).contains(&n), "{n} of 200 held out");
    }

    #[test]
    fn a_fit_is_per_race_and_kept_the_way_it_was_fitted() {
        let run = |i: usize, race: &str, landed: bool| Run {
            run: format!("r{i}"),
            race: race.into(),
            windows: vec![[if landed { 0 } else { 4 }, 0, 0, 0, 0, 0]; 6],
            landed: Some(landed),
        };
        let mut runs: Vec<Run> = (0..60)
            .map(|i| run(i, if i % 3 == 0 { "elf" } else { "slime" }, i % 2 == 0))
            .collect();
        // A run the gate could not judge teaches nothing.
        runs.push(Run {
            landed: None,
            ..run(99, "orc", false)
        });
        let fits = fit(&runs, &BTreeMap::new());
        assert_eq!(fits.keys().collect::<Vec<_>>(), ["elf", "slime"]);
        let slime = &fits["slime"];
        assert_eq!(slime.trained + slime.held, 40);
        let score = slime.score.as_ref().unwrap();
        assert!(score.brier < score.base_brier, "{score:?}");

        let state = tempfile::tempdir().unwrap();
        let models: BTreeMap<String, Guts> = fits
            .iter()
            .map(|(r, f)| (r.clone(), f.model.clone()))
            .collect();
        save(state.path(), &models).unwrap();
        let back = load(state.path()).unwrap();
        assert_eq!(
            back.keys().collect::<Vec<_>>(),
            models.keys().collect::<Vec<_>>()
        );
        for (race, g) in &models {
            let b = &back[race];
            let close = |x: f64, y: f64| (x - y).abs() < 1e-12;
            assert!(close(g.bias, b.bias) && g.seen == b.seen, "{race}");
            assert!(
                (0..INPUTS).all(|c| close(g.weights[c], b.weights[c])),
                "{race}"
            );
            assert!((0..INPUTS).all(|c| close(g.means[c], b.means[c])), "{race}");
        }
    }
}
