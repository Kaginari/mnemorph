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
        ["show"] => show(),
        _ => refused(&"guts needs runs, fit <runs.jsonl>... [--save], or show; see --help"),
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

fn fit(cwd: &std::path::Path, files: &[&str], save: bool) -> ExitCode {
    let mut all: Vec<Run> = Vec::new();
    for f in files {
        let path = absolute(cwd, &PathBuf::from(f));
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return fail(&format!("{}: {e}", path.display())),
        };
        for (n, line) in text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            match serde_json::from_str(line) {
                Ok(r) => all.push(r),
                Err(e) => return fail(&format!("{}:{}: {e}", path.display(), n + 1)),
            }
        }
    }
    // From nothing: a model scored on runs it held out never saw them in an earlier fit either.
    let fits = learned::fit(&all, &BTreeMap::new());
    for (race, f) in &fits {
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
        println!(
            "{race}: {} runs trained, {} held out; {verdict}",
            f.trained, f.held
        );
    }
    if save {
        let Some(state) = learned::state() else {
            return fail("no state directory: nothing can be kept");
        };
        let models = fits.into_iter().map(|(r, f)| (r, f.model)).collect();
        match learned::save(&state, &models) {
            Ok(path) => println!("kept at {}", path.display()),
            Err(e) => return fail(&e),
        }
    }
    ExitCode::SUCCESS
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
            for (race, g) in models {
                let w: Vec<String> = g.weights.iter().map(|w| format!("{w:+.3}")).collect();
                println!(
                    "{race}: {} examples; bias {:+.3}; fail {} red-pin {} positive {} repeat {} echo {} thrash {}",
                    g.seen, g.bias, w[0], w[1], w[2], w[3], w[4], w[5]
                );
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
