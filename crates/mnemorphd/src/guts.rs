//! `mnemorphd guts …`: the learned guts. `runs` prints a world's runs as the model reads them,
//! one JSON line each, from the harness's own journals; `fit` trains one model per race on runs
//! from any number of worlds, scores it on the runs it held out against their base rate, and
//! with `--save` keeps it under the human's state directory; `show` prints what is kept. The
//! model predicts and never acts.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use mnemorph_server::world::learned::{self, Run};

use super::{absolute, fail, refused};

/// `guts runs|fit|show`.
pub(crate) fn guts(args: &[String]) -> ExitCode {
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    let (mut world, mut save, mut words) = (None, false, Vec::new());
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--world" => {
                let Some(v) = it.next() else {
                    return refused(&"--world needs a value");
                };
                world = Some(absolute(&cwd, &PathBuf::from(v)));
            }
            "--save" => save = true,
            other if other.starts_with("--") => {
                return refused(&format!("unknown argument `{other}`; see --help"));
            }
            _ => words.push(w.as_str()),
        }
    }
    match words.as_slice() {
        ["runs"] => runs(&world.unwrap_or(cwd)),
        ["fit", files @ ..] if !files.is_empty() => fit(&cwd, files, save),
        ["watch", files @ ..] if !files.is_empty() => watch(&cwd, files),
        ["show"] => show(),
        _ => refused(
            &"guts needs runs, fit <runs.jsonl>... [--save], watch <runs.jsonl>..., or show; \
              see --help",
        ),
    }
}

fn runs(world: &std::path::Path) -> ExitCode {
    let world = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    match learned::runs(&world) {
        Ok(rs) => {
            for r in rs {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
            }
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e),
    }
}

/// The runs these files hold, one JSON line each; a line that is no run is named.
fn read_runs(cwd: &std::path::Path, files: &[&str]) -> Result<Vec<Run>, String> {
    let mut all: Vec<Run> = Vec::new();
    for f in files {
        let path = absolute(cwd, &PathBuf::from(f));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for (n, line) in text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            all.push(
                serde_json::from_str(line)
                    .map_err(|e| format!("{}:{}: {e}", path.display(), n + 1))?,
            );
        }
    }
    Ok(all)
}

/// `guts watch <runs.jsonl>...`: score again each kept unit that advises and is due, on the
/// labelled runs that came in since it was fitted. One that no longer gains over its reference
/// stops advising, and the advice record says so.
fn watch(cwd: &std::path::Path, files: &[&str]) -> ExitCode {
    let all = match read_runs(cwd, files) {
        Ok(all) => all,
        Err(e) => return fail(&e),
    };
    let Some(state) = learned::state() else {
        return fail("no state directory: nothing is kept");
    };
    match learned::watch(&state, &all) {
        Err(e) => fail(&e),
        Ok(found) if found.is_empty() => {
            println!(
                "no kept unit was due: none advises, or fewer than {} new labelled runs have \
                 come in since its last score",
                learned::RESCORE_EVERY
            );
            ExitCode::SUCCESS
        }
        Ok(found) => {
            for (race, scored) in found {
                let interval = scored.interval.map_or_else(
                    || "no interval".to_owned(),
                    |(lo, hi)| format!("90% {lo:.4} to {hi:.4}"),
                );
                println!(
                    "{race}: scored again on {} new labelled runs: gain over its reference \
                     {:+.4} ({interval}) — {}",
                    scored.runs,
                    scored.gain,
                    if scored.advises {
                        "still advises"
                    } else {
                        "stops advising"
                    }
                );
            }
            ExitCode::SUCCESS
        }
    }
}

fn fit(cwd: &std::path::Path, files: &[&str], save: bool) -> ExitCode {
    let all = match read_runs(cwd, files) {
        Ok(all) => all,
        Err(e) => return fail(&e),
    };
    // Old and new readings never mix: a run read with other inputs is left out, and counted.
    let current = mnemorph_server::world::races::learned::INPUT_VERSION;
    let older = all.iter().filter(|r| r.inputs != current).count();
    if older > 0 {
        println!(
            "{older} of {} runs were read with other inputs than this build's (version {current}) \
             and are left out",
            all.len()
        );
    }
    // From nothing: a model scored on runs it held out never saw them in an earlier fit either.
    let fits = learned::fit(&all, &BTreeMap::new());
    // Runs that share a group (a bench's task) are not independent: split by run, a task's other
    // runs teach the model what the held-out one will do. Both splits are said; the group's is
    // the one to trust.
    let grouped = all.iter().any(|r| r.group.is_some());
    // The control: the same fit on labels shuffled among runs. It can learn nothing true, so a
    // gain it shows means the split or the interval cannot be trusted.
    let control = learned::shuffled(&all);
    let by_group =
        grouped.then(|| learned::fit_split(&all, &BTreeMap::new(), learned::Split::Group));
    // Every held-out set this command scored on has judged one more fit. They are recorded
    // together: one command is one look, and its splits are not read against each other.
    let looks = looks_of(
        fits.values()
            .chain(by_group.iter().flat_map(BTreeMap::values)),
    );
    if grouped {
        println!("split by run:");
    }
    print_fits(&fits, "", &looks);
    let control_by_run = learned::fit(&control, &BTreeMap::new());
    print_control(&control_by_run);
    let control_by_group =
        grouped.then(|| learned::fit_split(&control, &BTreeMap::new(), learned::Split::Group));
    if let (Some(by_group), Some(control)) = (&by_group, &control_by_group) {
        println!("split by group, every run of a group on one side:");
        print_fits(by_group, " groups", &looks);
        print_control(control);
    }
    if save {
        let Some(state) = learned::state() else {
            return fail("no state directory: nothing can be kept");
        };
        // The unit kept is the one the bar is read on: the split by group's when the runs name
        // groups, else the split by run's, which can never earn advice.
        let (judged_on, split, controls) = match (&by_group, &control_by_group) {
            (Some(g), Some(c)) => (g, learned::Split::Group, c),
            _ => (&fits, learned::Split::Run, &control_by_run),
        };
        let mut units = BTreeMap::new();
        for (race, f) in judged_on {
            let looked = looks
                .as_ref()
                .ok()
                .and_then(|l| l.get(&learned::held_digest(&f.held_runs)));
            let short = f.short_of_the_bar(split, controls.get(race), looked);
            if short.is_empty() {
                println!("{race}: advises");
            } else {
                println!("{race}: does not advise — {}", short.join("; "));
            }
            let event = if short.is_empty() {
                "earned"
            } else {
                "not earned"
            };
            if let Err(e) = learned::note_advice(&state, race, event, &short.join("; ")) {
                return fail(&e);
            }
            units.insert(race.clone(), learned::Kept::of(f, &all, &short));
        }
        match learned::save(&state, &units) {
            Ok(path) => println!("kept at {}", path.display()),
            Err(e) => return fail(&e),
        }
    }
    ExitCode::SUCCESS
}

/// Each race's fit, one line: `unit` names what the held-out count after the runs counts.
fn print_fits(fits: &BTreeMap<String, learned::Fit>, unit: &str, looks: &Looks) {
    for (race, f) in fits {
        let verdict = match &f.score {
            None => "no run held out: not scored".to_owned(),
            Some(s) => format!(
                "{} acts held out, base rate {:.2}: Brier {:.4} against {:.4}, log loss {:.4} — {}",
                s.runs,
                s.base_rate,
                s.brier,
                s.base_brier,
                s.log_loss,
                judged(s, f.gain)
            ),
        };
        let held = if unit.is_empty() {
            format!("{} held out", f.held)
        } else {
            format!("{} held out in {}{unit}", f.held, f.held_groups)
        };
        println!("{race}: {} runs trained, {held}; {verdict}", f.trained);
        if let Some(score) = &f.score {
            println!("  {}", against_reference(f, score.brier, unit));
            println!("  {}", looked_at(&f.held_runs, looks));
        }
    }
}

/// The fit read against the reference, the base rate of the same 40-call stretch of the run on
/// the training runs, which the unit is also given as an input: the reference's Brier,
/// the gain over it with its 90% interval, and the held-out units where the guts is worse. It
/// clears the reference only when it is worse in none and the gain's lower bound is above 0.
fn against_reference(f: &learned::Fit, brier: f64, unit: &str) -> String {
    let Some(reference) = f.reference_brier else {
        return "against the base rate of the same stretch of the run: not scored".to_owned();
    };
    let units = if unit.is_empty() { "runs" } else { unit.trim() };
    let worse = &f.worse_than_reference_in;
    let where_worse = if worse.is_empty() {
        format!("no worse in any of the {} held-out {units}", f.held_groups)
    } else {
        let named: Vec<&str> = worse.iter().take(5).map(String::as_str).collect();
        format!(
            "worse in {} of {} held-out {units} ({}{})",
            worse.len(),
            f.held_groups,
            named.join(", "),
            if worse.len() > named.len() {
                ", …"
            } else {
                ""
            }
        )
    };
    let gain = reference - brier;
    let (interval, clears) = match f.over_reference {
        None => ("too few held out for an interval".to_owned(), false),
        Some((lo, hi)) => (
            format!("90% {lo:.4} to {hi:.4}"),
            lo > 0.0 && worse.is_empty(),
        ),
    };
    format!(
        "against the base rate of the same stretch of the run: its Brier {reference:.4}; gain over it \
         {gain:+.4} ({interval}); {where_worse} — {}",
        if clears {
            "clears the reference"
        } else {
            "does not clear the reference"
        }
    )
}

/// What each held-out set of this command had judged before it, by digest; or why that could
/// not be counted.
type Looks = Result<BTreeMap<String, learned::Judged>, String>;

/// Record that the held-out sets of these fits each judged one more fit ([`learned::judge_all`]).
fn looks_of<'a>(fits: impl Iterator<Item = &'a learned::Fit>) -> Looks {
    let sets: Vec<Vec<String>> = fits
        .filter(|f| f.score.is_some())
        .map(|f| f.held_runs.clone())
        .collect();
    let state = learned::state().ok_or_else(|| "no state directory".to_owned())?;
    learned::judge_all(&state, &sets)
}

/// How many fits this held-out set had judged before this command: the fit that decides a bar
/// wants a set that has judged nothing.
fn looked_at(held: &[String], looks: &Looks) -> String {
    let digest = learned::held_digest(held);
    let name: String = digest.chars().take(12).collect();
    match looks.as_ref().map(|l| l.get(&digest)) {
        Err(e) => format!(
            "held-out set {name}: the fits it has judged cannot be counted ({e}), so this fit \
             cannot decide a bar"
        ),
        Ok(None) => format!(
            "held-out set {name}: the fits it has judged were not counted, so this fit cannot \
             decide a bar"
        ),
        Ok(Some(j)) if j.fits_before == 0 && j.runs_seen_before == 0 => {
            format!("held-out set {name} has judged no fit before")
        }
        Ok(Some(j)) if j.fits_before == 0 => format!(
            "held-out set {name} has judged no fit before, but {} of its {} runs were held out \
             for earlier fits: it is not a fresh set",
            j.runs_seen_before, j.runs
        ),
        Ok(Some(j)) => format!(
            "held-out set {name} has judged {} fit{} before this one: this fit cannot decide a bar",
            j.fits_before,
            if j.fits_before == 1 { "" } else { "s" }
        ),
    }
}

/// The shuffled-label control, one line per race: it must show no gain.
fn print_control(fits: &BTreeMap<String, learned::Fit>) {
    for (race, f) in fits {
        let said = match f.gain {
            None => "too few held-out runs to judge".to_owned(),
            Some((lo, hi)) if lo > 0.0 => format!(
                "GAINS (90% {lo:.4} to {hi:.4}): the split or the interval cannot be trusted"
            ),
            Some((lo, hi)) => format!("no gain (90% {lo:.4} to {hi:.4}), as it must"),
        };
        println!("  {race} control, labels shuffled: {said}");
    }
}

fn show() -> ExitCode {
    let Some(state) = learned::state() else {
        return fail("no state directory: nothing is kept");
    };
    match learned::load(&state) {
        Ok(models) if models.is_empty() => {
            println!("no learned guts kept yet: `mnemorphd guts fit … --save`");
            ExitCode::SUCCESS
        }
        Ok(models) => {
            for (race, unit) in models {
                let g = &unit.model;
                let w: Vec<String> = g.weights.iter().map(|w| format!("{w:+.3}")).collect();
                println!(
                    "{race}: {} examples; bias {:+.3}; fail {} red-pin {} positive {} repeat {} echo {} \
                     thrash {} edit miss {} rows passing {} since shrank {} clock {}",
                    g.seen, g.bias, w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7], w[8], w[9]
                );
                match &unit.advice.why {
                    _ if unit.advice.advises => println!("  advises"),
                    Some(why) => println!("  does not advise: {why}"),
                    None => println!("  does not advise"),
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e),
    }
}

/// The verdict on a fit: "beats the base rate" only when the whole 90% interval of its gain,
/// resampled by held-out run, lies above 0. A base rate that every held-out act matched cannot be
/// beaten, so it says nothing either way.
fn judged(s: &mnemorph_server::world::races::learned::Score, gain: Option<(f64, f64)>) -> String {
    if s.base_brier <= f64::EPSILON {
        return "cannot judge: every held-out act ended the same way".to_owned();
    }
    match gain {
        None => "too few held-out runs to judge".to_owned(),
        Some((lo, hi)) if lo > 0.0 => {
            format!(
                "beats the base rate (gain {:.4}, 90% {lo:.4} to {hi:.4})",
                s.base_brier - s.brier
            )
        }
        Some((lo, hi)) if hi < 0.0 => {
            format!(
                "worse than the base rate (gain {:.4}, 90% {lo:.4} to {hi:.4})",
                s.base_brier - s.brier
            )
        }
        Some((lo, hi)) => format!(
            "no better than the base rate within noise (gain {:.4}, 90% {lo:.4} to {hi:.4})",
            s.base_brier - s.brier
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::judged;
    use mnemorph_server::world::races::learned::Score;

    /// A base rate every held-out act matched cannot be beaten, so it says nothing either way; a
    /// gain whose interval holds 0 is noise, not a win.
    #[test]
    fn the_verdict_says_beats_only_when_the_interval_does() {
        let all_landed = Score::of([(0.9, true), (0.8, true)]);
        assert_eq!(
            judged(&all_landed, Some((0.01, 0.02))),
            "cannot judge: every held-out act ended the same way"
        );
        let mixed = Score::of([(0.9, true), (0.2, false), (0.7, true), (0.4, false)]);
        assert!(judged(&mixed, Some((0.01, 0.2))).starts_with("beats the base rate"));
        assert!(judged(&mixed, Some((-0.05, 0.2))).starts_with("no better than the base rate"));
        assert!(judged(&mixed, Some((-0.2, -0.01))).starts_with("worse than the base rate"));
        assert_eq!(judged(&mixed, None), "too few held-out runs to judge");
    }
}
