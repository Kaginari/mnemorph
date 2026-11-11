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
use crate::races::learned::{
    CLASSES, EDIT_MISS, Guts, INPUT_VERSION, INPUTS, NO_ROWS, ROWS_PASSING, SINCE_CAP,
    SINCE_SHRANK, Score, WINDOW,
};

/// One run, as the learned guts reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The run's id.
    pub run: String,
    /// The race it ran as.
    pub race: String,
    /// Each act's readings, as its input version reads them. At version 2: the count of each
    /// class over the last [`WINDOW`] acts, this one included, in the order `fail`, `red-pin`,
    /// `positive`, `repeat`, `echo`, `thrash`; the edit misses over the same acts; the share of
    /// the statement rows passing, in hundredths; the turns since the failing rows shrank. At
    /// version 1, the six class counts alone.
    pub windows: Vec<Vec<u32>>,
    /// The input version the readings are for: runs of other versions are never fitted together.
    #[serde(default = "first_version")]
    pub inputs: u32,
    /// The gate passed it (`true`) or failed it (`false`); `None` when it could not judge it.
    pub landed: Option<bool>,
    /// What the run shares with others that a held-out score must not see in training — a
    /// bench's task: two runs of one task are not independent. `None` when nothing does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

fn first_version() -> u32 {
    1
}

impl Run {
    /// The run's readings as the model's inputs, when they are this build's version.
    #[must_use]
    pub fn current(&self) -> Option<Vec<[u32; INPUTS]>> {
        if self.inputs != INPUT_VERSION {
            return None;
        }
        self.windows
            .iter()
            .map(|w| <[u32; INPUTS]>::try_from(w.as_slice()).ok())
            .collect()
    }
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

/// Each act's class counts over the window, over a run's acts in order.
#[must_use]
pub fn class_windows(acts: &[&[Class]]) -> Vec<[u32; CLASSES]> {
    (0..acts.len())
        .map(|i| {
            let mut counts = [0; CLASSES];
            for act in &acts[i.saturating_sub(WINDOW - 1)..=i] {
                for class in *act {
                    counts[input(*class)] += 1;
                }
            }
            counts
        })
        .collect()
}

/// A run's readings from its guts rows in order, with their input version: this build's when
/// every row carries its readings, else version 1, the class counts alone. A world with no
/// statement rows reads the two rows inputs at [`NO_ROWS`], the middle of their ranges.
#[must_use]
pub fn windows(rows: &[&guts::GutsRow]) -> (u32, Vec<Vec<u32>>) {
    let classes: Vec<&[Class]> = rows.iter().map(|r| r.classes.as_slice()).collect();
    let counts = class_windows(&classes);
    if rows.is_empty() || rows.iter().any(|r| r.inputs != INPUT_VERSION) {
        return (1, counts.iter().map(|c| c.to_vec()).collect());
    }
    let read = (0..rows.len())
        .map(|i| {
            let mut w = [0u32; INPUTS];
            w[..CLASSES].copy_from_slice(&counts[i]);
            w[EDIT_MISS] = rows[i.saturating_sub(WINDOW - 1)..=i]
                .iter()
                .map(|r| u32::from(r.edit_miss))
                .sum();
            w[ROWS_PASSING] = rows[i]
                .rows_passing
                .map_or(NO_ROWS.0, |p| u32::from(p.min(100)));
            w[SINCE_SHRANK] = rows[i].since_shrank.map_or(NO_ROWS.1, |t| t.min(SINCE_CAP));
            w.to_vec()
        })
        .collect();
    (INPUT_VERSION, read)
}

/// `world`'s runs: every run the guts journal holds acts of and the report ledger settled.
///
/// # Errors
///
/// No state directory, or a journal whose chain does not hold.
pub fn runs(world: &Path) -> Result<Vec<Run>, String> {
    let rows = guts::held(world)?;
    let acts = guts::rows_by_run(&rows);
    let folded = crate::reports::fold(world).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in &folded.rows {
        let ReportRow::Run(r) = row else { continue };
        let Some(acts) = acts.get(r.run.as_str()) else {
            continue;
        };
        let (inputs, windows) = windows(acts);
        out.push(Run {
            run: r.run.clone(),
            race: r.race.clone(),
            windows,
            inputs,
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

/// A run's readings as the model's inputs, act by act.
type Readings = Vec<[u32; INPUTS]>;

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
    // Only runs read with this build's inputs: old and new readings never mix in one fit.
    let mut by_race: BTreeMap<&str, Vec<(&Run, Readings)>> = BTreeMap::new();
    for r in runs.iter().filter(|r| r.landed.is_some()) {
        if let Some(w) = r.current() {
            by_race
                .entry(fitted_race(&r.race))
                .or_default()
                .push((r, w));
        }
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
            let (held, trained): (Vec<_>, Vec<_>) =
                rs.iter().partition(|(r, _)| held_out(&unit(r)));
            // Every training act at once: one step per act, run after run, left a model that said
            // what the last runs said.
            let examples: Vec<([u32; INPUTS], bool)> = trained
                .iter()
                .flat_map(|(r, ws)| {
                    let landed = r.landed == Some(true);
                    ws.iter().map(move |w| (*w, landed))
                })
                .collect();
            model.fit_batch(&examples);
            let judged = &model;
            // The units the interval resamples: a run, or every run of a group together.
            let mut per_unit: BTreeMap<String, Vec<(f64, bool)>> = BTreeMap::new();
            for (r, ws) in &held {
                let landed = r.landed == Some(true);
                per_unit
                    .entry(unit(r))
                    .or_default()
                    .extend(ws.iter().map(|w| (judged.predict(w), landed)));
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
    weights: Vec<f64>,
    bias: f64,
    means: Vec<f64>,
    seen: u64,
    #[serde(default = "first_version")]
    inputs: u32,
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
    kept.races
        .into_iter()
        .map(|(race, k)| {
            // A model kept from other inputs reads other numbers: it is refused, never padded.
            let (Ok(weights), Ok(means)) = (
                <[f64; INPUTS]>::try_from(k.weights.as_slice()),
                <[f64; INPUTS]>::try_from(k.means.as_slice()),
            ) else {
                return Err(format!(
                    "{}: {race}'s model was kept from input version {}; this build reads version \
                     {INPUT_VERSION}: fit it again with `mnemorphd guts fit … --save`",
                    path.display(),
                    k.inputs
                ));
            };
            if k.inputs != INPUT_VERSION {
                return Err(format!(
                    "{}: {race}'s model was kept from input version {}; fit it again",
                    path.display(),
                    k.inputs
                ));
            }
            let mut g = Guts {
                weights,
                bias: k.bias,
                means,
                seen: k.seen,
            };
            // The red pin never learns, whatever a file says.
            g.weights[crate::races::learned::RED_PIN] = 0.0;
            Ok((race, g))
        })
        .collect()
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
                    weights: g.weights.to_vec(),
                    bias: g.bias,
                    means: g.means.to_vec(),
                    seen: g.seen,
                    inputs: INPUT_VERSION,
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

    /// A version-2 window: `n` failing acts, no edit miss, a world with no rows.
    fn fails(n: u32) -> Vec<u32> {
        let mut w = vec![0; INPUTS];
        w[0] = n;
        w[ROWS_PASSING] = NO_ROWS.0;
        w[SINCE_SHRANK] = NO_ROWS.1;
        w
    }

    fn guts_row(
        classes: &[Class],
        inputs: u32,
        edit_miss: bool,
        rows: Option<(u8, u32)>,
    ) -> guts::GutsRow {
        guts::GutsRow {
            ts: mnemorph_ledger::Timestamp(0),
            run: "r1".into(),
            body: "slime-a".into(),
            tool_use_id: "t".into(),
            tool: "edit".into(),
            classes: classes.to_vec(),
            build: mnemorph_ledger::Build::default(),
            inputs,
            edit_miss,
            rows_passing: rows.map(|r| r.0),
            since_shrank: rows.map(|r| r.1),
        }
    }

    /// The harness's readings: edit misses over the window, and the rows read as the act left
    /// them — or, with no rows, the middle of each range.
    #[test]
    fn a_runs_readings_carry_misses_and_rows() {
        let rows = [
            guts_row(&[Class::Fail], INPUT_VERSION, true, None),
            guts_row(&[], INPUT_VERSION, false, None),
            guts_row(&[], INPUT_VERSION, true, Some((80, 3))),
        ];
        let refs: Vec<&guts::GutsRow> = rows.iter().collect();
        let (version, w) = windows(&refs);
        assert_eq!(version, INPUT_VERSION);
        assert_eq!(w[1][EDIT_MISS], 1);
        assert_eq!(w[2][EDIT_MISS], 2);
        assert_eq!((w[1][ROWS_PASSING], w[1][SINCE_SHRANK]), NO_ROWS);
        assert_eq!((w[2][ROWS_PASSING], w[2][SINCE_SHRANK]), (80, 3));
        assert_eq!(w[2][0], 1, "the failing act is still in the window");
    }

    /// A run with a row from before the readings is read as version 1, the class counts alone,
    /// and a fit leaves it out rather than mix it with version 2.
    #[test]
    fn old_and_new_readings_never_mix() {
        let rows = [
            guts_row(&[Class::Fail], 0, false, None),
            guts_row(&[], INPUT_VERSION, true, None),
        ];
        let refs: Vec<&guts::GutsRow> = rows.iter().collect();
        let (version, w) = windows(&refs);
        assert_eq!((version, w[0].len()), (1, CLASSES));
        let old = Run {
            run: "r-old".into(),
            race: "slime".into(),
            windows: w,
            inputs: version,
            landed: Some(true),
            group: None,
        };
        assert!(old.current().is_none());
        let line = r#"{"run":"r0","race":"slime","windows":[[1,0,0,0,0,0]],"landed":false}"#;
        let from_record: Run = serde_json::from_str(line).unwrap();
        assert_eq!(
            from_record.inputs, 1,
            "a record row without a stamp is version 1"
        );
        assert!(fit(&[old, from_record], &BTreeMap::new()).is_empty());
    }

    /// A model kept from other inputs is refused, never padded or cut to fit.
    #[test]
    fn a_model_kept_from_other_inputs_is_refused() {
        let state = tempfile::tempdir().unwrap();
        let path = kept_at(state.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"races":{"slime":{"weights":[0,0,0,0,0,0],"bias":0,"means":[0,0,0,0,0,0],"seen":3}}}"#,
        )
        .unwrap();
        let e = load(state.path()).unwrap_err();
        assert!(e.contains("input version 1"), "{e}");
    }

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
        let w = class_windows(&acts);
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
            windows: vec![fails(if landed { 0 } else { 4 }); 6],
            inputs: INPUT_VERSION,
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
                windows: vec![fails(u32::from(i % 2 == 1)); 3],
                inputs: INPUT_VERSION,
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
            windows: vec![fails(0); 2],
            inputs: INPUT_VERSION,
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
                windows: vec![fails(if i % 3 == 0 { 0 } else { 3 }); 4],
                inputs: INPUT_VERSION,
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
