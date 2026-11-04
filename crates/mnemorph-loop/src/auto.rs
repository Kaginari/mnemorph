//! Evolution on its own, with `evolution.mode = "auto"`: a trial, then keep or roll back by the
//! rule. One tick, run by the harness after a session (`mnemorphd evolution auto`):
//!
//! 1. **Settle.** A swap whose window of 10 runs is full is judged — its do-nothing controls run
//!    first, priced against the human's caps — and settled by the rule: kept on its net wins and
//!    no more holes than doing nothing, otherwise rolled back, its named files put back as they
//!    were before it. A judgement still pending or refused is left as it stands, said.
//! 2. **Evolve.** One creature the trigger finds due, by mutate or rebirth, with no swap still
//!    unsettled and no proposal waiting, is drafted, proven over an overlay and swapped in. A
//!    draft not proven stays a proposal for the human.
//!
//! At most one judgement and one draft a tick, so a tick's spend is bounded. With
//! `evolution.mode = "propose"` nothing is done: every plan waits for the human.

use std::path::Path;

use mnemorph_ledger::evolution::{self as ledger, EvolutionRow, Judgement};
use mnemorph_world::evolution::{Arm, Plan, Settled};
use mnemorph_world::evolve::Due;
use mnemorph_world::proposals;

use crate::draft::Draft;

/// A plan proven over an overlay: what was proven, or every reason it was not.
pub type Proof = Result<Vec<String>, Vec<String>>;

/// What a tick reaches out with: the drafter, the judge and the proof, each given so a test can
/// stand in for the model.
pub struct Ticker<'a> {
    /// Drafts the plan of a creature due.
    pub draft: &'a dyn Fn(&Due) -> Result<Draft, String>,
    /// Runs the controls swap `id` waits on, then judges it.
    pub judge: &'a dyn Fn(&str) -> Result<Judgement, String>,
    /// Proves a plan over an overlay of the world.
    pub prove: &'a dyn Fn(&Plan) -> Proof,
}

/// One tick over `world`: what it settled and what it evolved, one line each.
///
/// # Errors
///
/// The evolution ledger cannot be read, or the trigger cannot read its journals.
pub fn tick(world: &Path, t: &Ticker<'_>) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    settle_one(world, t, &mut lines)?;
    evolve_one(world, t, &mut lines)?;
    Ok(lines)
}

fn read(world: &Path) -> Result<Option<ledger::Ledger>, String> {
    let Some(at) = mnemorph_world::evolution::ledger_of(world) else {
        return Ok(None);
    };
    ledger::read(&at, None)
        .map(Some)
        .map_err(|e| format!("the evolution ledger cannot be read: {e}"))
}

/// The swaps not yet settled, oldest first.
fn unsettled(read: &ledger::Ledger) -> Vec<&ledger::Swap> {
    read.entries
        .iter()
        .filter_map(|e| match &e.row {
            EvolutionRow::Swap(s) if ledger::settled(read, &s.id).is_none() => Some(s),
            _ => None,
        })
        .collect()
}

fn settle_one(world: &Path, t: &Ticker<'_>, lines: &mut Vec<String>) -> Result<(), String> {
    let Some(read) = read(world)? else {
        return Ok(());
    };
    let Some(swap) = unsettled(&read)
        .into_iter()
        .find(|s| ledger::window_full(&read, &s.id))
    else {
        return Ok(());
    };
    let id = swap.id.clone();
    let (outcome, judged) = match (t.judge)(&id) {
        Ok(j @ Judgement::Keep(_)) => (Settled::Keep, j),
        Ok(j @ Judgement::Rollback(_)) => (Settled::Rollback, j),
        Ok(j) => {
            lines.push(format!("swap {id}: {j} — left as it stands"));
            return Ok(());
        }
        Err(e) => {
            lines.push(format!("swap {id} not judged: {e}"));
            return Ok(());
        }
    };
    let why = judged.to_string();
    match mnemorph_world::evolution::settle(world, &id, outcome, "harness", &why) {
        Ok(_) => lines.push(format!(
            "swap {id} for {}: {} by the rule — {}",
            swap.creature,
            outcome.word(),
            why.lines().next().unwrap_or_default()
        )),
        Err(e) => lines.push(format!("swap {id} not settled: {e}")),
    }
    Ok(())
}

fn evolve_one(world: &Path, t: &Ticker<'_>, lines: &mut Vec<String>) -> Result<(), String> {
    let survey = mnemorph_world::evolve::survey(world)?;
    let read = read(world)?;
    let busy: Vec<String> = read
        .as_ref()
        .map(|r| {
            unsettled(r)
                .into_iter()
                .flat_map(|s| s.bodies.iter().chain([&s.creature]).cloned())
                .collect()
        })
        .unwrap_or_default();
    let waiting: Vec<String> = proposals::open(world)
        .map(|o| o.into_iter().map(|p| p.creature).collect())
        .unwrap_or_default();
    let Some(due) = survey.due.iter().find(|d| {
        d.arm != Arm::Split && !busy.contains(&d.creature) && !waiting.contains(&d.creature)
    }) else {
        return Ok(());
    };
    let d = match (t.draft)(due) {
        Ok(d) => d,
        Err(e) => {
            lines.push(format!("{} not drafted: {e}", due.creature));
            return Ok(());
        }
    };
    let p = proposals::get(world, &d.id)?;
    let files = proposals::ready(world, &p)?;
    let plan = Plan {
        id: p.id.clone(),
        creature: p.creature.clone(),
        bodies: vec![p.creature.clone()],
        arm: p.arm,
        files,
        proven_on: Vec::new(),
    };
    if let Err(why) = (t.prove)(&plan) {
        lines.push(format!(
            "{} drafted `{}`, not proven — left for the human: {}",
            d.creature,
            d.id,
            why.join("; ")
        ));
        return Ok(());
    }
    match mnemorph_world::evolution::swap(world, &plan) {
        Ok(s) => {
            proposals::close(world, &s.id, "applied")?;
            lines.push(format!(
                "{} evolved by {}: swap {} on trial for {} runs, then kept or rolled back by the \
                 rule — `mnemorphd evolution rollback {}` puts it back now",
                d.creature,
                mnemorph_world::evolve::arm_word(d.arm),
                s.id,
                ledger::EVOLUTION_WINDOW,
                s.id
            ));
        }
        Err(e) => lines.push(format!(
            "{} drafted `{}`, not swapped — left for the human: {e}",
            d.creature, d.id
        )),
    }
    Ok(())
}

/// [`tick`] with the world's configuration: the configured model drafts and runs the controls,
/// its key read from `env`; nothing is done unless `evolution.mode` is `auto`.
///
/// # Errors
///
/// The configuration does not load, or as [`tick`].
pub fn tick_now(
    world: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    say: &dyn Fn(&str),
) -> Result<Vec<String>, String> {
    tick_in(world, &mnemorph_config::Paths::resolve(world), env, say)
}

/// [`tick_now`], the configuration read from `paths`. `evolution.mode` is the human's global
/// file's alone: it lets the harness swap without asking, so a world's file setting it is a hole.
///
/// # Errors
///
/// As [`tick_now`].
pub fn tick_in(
    world: &Path,
    paths: &mnemorph_config::Paths,
    env: &dyn Fn(&str) -> Option<String>,
    say: &dyn Fn(&str),
) -> Result<Vec<String>, String> {
    let config = mnemorph_config::Config::load(paths)?;
    let state = mnemorph_tools::dirs::state_home()
        .ok_or("no home: evolution's journals are kept under the state directory")?;
    let lock = mnemorph_world::queue::state_file_in(&state, world, "evolution-auto")
        .with_extension("lock");
    if let Some(dir) = lock.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock)
        .map_err(|e| format!("{}: {e}", lock.display()))?;
    // Held until the tick returns: two ticks never judge or swap at once.
    if file.try_lock().is_err() {
        return Ok(vec!["an evolution tick is running already".to_owned()]);
    }
    if config.evolution_proposes() {
        return Ok(vec![
            "evolution.mode is `propose`: nothing evolves on its own".to_owned(),
        ]);
    }
    let tracked = mnemorph_world_tracked(world);
    let draft = |due: &Due| crate::draft::draft_now(world, &due.creature, env);
    let judge = |id: &str| crate::evolution::judge(world, id, env, say).map(|j| j.judgement);
    let prove = |plan: &Plan| crate::prove::prove_now(world, plan, &tracked);
    tick(
        world,
        &Ticker {
            draft: &draft,
            judge: &judge,
            prove: &prove,
        },
    )
}

/// The files git tracks in `world`, as `onto check` reads them; none when it is no repository.
fn mnemorph_world_tracked(world: &Path) -> Vec<String> {
    // The harness's own git: no key, and nothing the world's `.git/config` names is run.
    mnemorph_tools::git()
        .arg("-C")
        .arg(world)
        .args(["ls-files", "-z"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
