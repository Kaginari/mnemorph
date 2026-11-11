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
    /// What the run shares with others that a held-out score must not see in training — a
    /// bench's task: two runs of one task are not independent. `None` when nothing does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
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
            group: None,
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

/// How a fit splits its runs into training and held out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    /// Run by run: each run held out by the hash of its id.
    Run,
    /// Group by group: every run of a group held out together, by the hash of the group — a run
    /// with no group is its own — and the gain's interval resampled by group.
    Group,
}

/// The control: `runs` with their labels shuffled among the labelled runs of each race, each run
/// keeping its acts. A fit on them can learn nothing true, so a gain it shows is the split leaking
/// or the interval lying. Deterministic: the same runs shuffle the same way.
#[must_use]
pub fn shuffled(runs: &[Run]) -> Vec<Run> {
    let mut out = runs.to_vec();
    let mut by_race: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, r) in out.iter().enumerate().filter(|(_, r)| r.landed.is_some()) {
        by_race
            .entry(fitted_race(&r.race).to_owned())
            .or_default()
            .push(i);
    }
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    for idx in by_race.values() {
        let mut labels: Vec<Option<bool>> = idx.iter().map(|i| out[*i].landed).collect();
        for k in (1..labels.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = usize::try_from(state >> 33).unwrap_or(0) % (k + 1);
            labels.swap(k, j);
        }
        for (i, l) in idx.iter().zip(labels) {
            out[*i].landed = l;
        }
    }
    out
}

/// The race a run is fitted as: the throne's runs from builds before its rename say `rimuru`.
#[must_use]
pub fn fitted_race(race: &str) -> &str {
    if race == "rimuru" { "summoner" } else { race }
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
    /// Held-out groups: as many as the held-out runs when split by run.
    pub held_groups: usize,
    /// The 90% interval of the gain over the base rate on the held-out runs, resampled by run,
    /// or by group when split by group.
    pub gain: Option<(f64, f64)>,
}

/// A fit per race over `runs`, split by run, starting from `start`'s model of that race when it
/// has one.
#[must_use]
pub fn fit(runs: &[Run], start: &BTreeMap<String, Guts>) -> BTreeMap<String, Fit> {
    fit_split(runs, start, Split::Run)
}

/// A fit per race over `runs`, split as `split` says.
#[must_use]
pub fn fit_split(
    runs: &[Run],
    start: &BTreeMap<String, Guts>,
    split: Split,
) -> BTreeMap<String, Fit> {
    let mut by_race: BTreeMap<&str, Vec<&Run>> = BTreeMap::new();
    for r in runs.iter().filter(|r| r.landed.is_some()) {
        by_race.entry(fitted_race(&r.race)).or_default().push(r);
    }
    let unit = |r: &Run| -> String {
        match split {
            Split::Run => r.run.clone(),
            Split::Group => r.group.clone().unwrap_or_else(|| r.run.clone()),
        }
    };
    by_race
        .into_iter()
        .map(|(race, rs)| {
            let mut model = start.get(race).cloned().unwrap_or_default();
            let (held, trained): (Vec<&&Run>, Vec<&&Run>) =
                rs.iter().partition(|r| held_out(&unit(r)));
            // Every training act at once: one step per act, run after run, left a model that said
            // what the last runs said.
            let examples: Vec<([u32; INPUTS], bool)> = trained
                .iter()
                .flat_map(|r| {
                    let landed = r.landed == Some(true);
                    r.windows.iter().map(move |w| (*w, landed))
                })
                .collect();
            model.fit_batch(&examples);
            let judged = &model;
            // The units the interval resamples: a run, or every run of a group together.
            let mut per_unit: BTreeMap<String, Vec<(f64, bool)>> = BTreeMap::new();
            for r in &held {
                let landed = r.landed == Some(true);
                per_unit
                    .entry(unit(r))
                    .or_default()
                    .extend(r.windows.iter().map(|w| (judged.predict(w), landed)));
            }
            let per_unit: Vec<Vec<(f64, bool)>> = per_unit.into_values().collect();
            let pairs: Vec<(f64, bool)> = per_unit.iter().flatten().copied().collect();
            let score = (!pairs.is_empty()).then(|| Score::of(pairs));
            let fit = Fit {
                model,
                score,
                gain: crate::races::learned::gain_interval(&per_unit),
                trained: trained.len(),
                held: held.len(),
                held_groups: per_unit.len(),
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
            group: None,
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

    /// Split by group, a task's runs are all trained on or all held out, and the interval counts
    /// tasks, not runs; split by run, the same runs scatter across both sides.
    #[test]
    fn a_group_is_held_out_whole() {
        let runs: Vec<Run> = (0..120)
            .map(|i| Run {
                run: format!("r{i}"),
                race: "slime".into(),
                windows: vec![[u32::from(i % 2 == 1), 0, 0, 0, 0, 0]; 3],
                landed: Some(i % 2 == 0),
                group: Some(format!("task-{}", i % 12)),
            })
            .collect();
        let by_group = &fit_split(&runs, &BTreeMap::new(), Split::Group)["slime"];
        let held_tasks: std::collections::BTreeSet<_> = runs
            .iter()
            .filter(|r| held_out(r.group.as_deref().unwrap()))
            .map(|r| r.group.clone())
            .collect();
        assert_eq!(by_group.held_groups, held_tasks.len());
        assert_eq!(by_group.held, 10 * held_tasks.len());
        let by_run = &fit_split(&runs, &BTreeMap::new(), Split::Run)["slime"];
        assert_eq!(by_run.held_groups, by_run.held);
    }

    /// The throne was `rimuru` before its rename: its runs fit with the Summoner's.
    #[test]
    fn the_thrones_old_name_fits_with_its_new_one() {
        let run = |i: usize, race: &str| Run {
            run: format!("r{i}"),
            race: race.into(),
            windows: vec![[0; INPUTS]; 2],
            landed: Some(i.is_multiple_of(2)),
            group: None,
        };
        let runs: Vec<Run> = (0..20)
            .map(|i| run(i, if i < 10 { "rimuru" } else { "summoner" }))
            .collect();
        let fits = fit(&runs, &BTreeMap::new());
        assert_eq!(fits.keys().collect::<Vec<_>>(), ["summoner"]);
        assert_eq!(fits["summoner"].trained + fits["summoner"].held, 20);
    }

    /// Shuffled, each race keeps its labels, only their runs change; a model fit on them gains
    /// nothing, where one fit on the true labels does.
    #[test]
    fn a_shuffled_control_shows_no_gain() {
        let runs: Vec<Run> = (0..200)
            .map(|i| Run {
                run: format!("r{i}"),
                race: "slime".into(),
                windows: vec![[if i % 3 == 0 { 0 } else { 3 }, 0, 0, 0, 0, 0]; 4],
                landed: Some(i % 3 == 0),
                group: None,
            })
            .collect();
        let control = shuffled(&runs);
        let count = |rs: &[Run]| rs.iter().filter(|r| r.landed == Some(true)).count();
        assert_eq!(count(&control), count(&runs));
        assert_eq!(
            control,
            shuffled(&runs),
            "the same runs shuffle the same way"
        );
        let real = fit(&runs, &BTreeMap::new())["slime"].gain.unwrap();
        let none = fit(&control, &BTreeMap::new())["slime"].gain.unwrap();
        assert!(real.0 > 0.0, "{real:?}");
        assert!(none.0 <= 0.0, "{none:?}");
    }
}
