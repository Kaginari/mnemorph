//! An evolution's do-nothing controls, run on demand, and its judgement.
//!
//! No scheduler runs the controls beside the window: the human asks for them with
//! `mnemorphd evolution judge <swap>`. Each window run still waiting on its control is run again —
//! the same ask, by the same body, under the same build — in a scratch copy of the world whose
//! named files are put back as they were before the swap (the bytes kept beside the ledger), and
//! appended as a control row stamped with the before state it found and left. Then the swap is
//! judged ([`mnemorph_ledger::evolution`]). Keep or rollback is only proposed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_config::{Config, Paths};
use mnemorph_ledger::evolution::state_now;
use mnemorph_ledger::evolution::{self as ledger, Judgement};
use mnemorph_ledger::{Build, Journal};
use mnemorph_provider::{Caller, anthropic_prices};
use mnemorph_races::Race;
use mnemorph_tools::{Context, NoHuman, registry};
use mnemorph_world::Roster;
use mnemorph_world::evolution::{Facts, Stamp, fingerprint, restore_before};

use crate::beats::Beat;
use crate::gate::{Author, EndGate};
use crate::model::Model;
use crate::run::{Ending, Limits, Run, Setup};
use crate::solo::{configure, solo};

/// What runs the controls: the model, its id, the configuration, and how a control's tools'
/// context is made ready over its scratch world (the sandbox, the attic).
pub struct Controller<'a> {
    /// The model.
    pub model: Arc<dyn Model>,
    /// The id it answers to.
    pub model_id: String,
    /// The world's configuration.
    pub config: &'a Config,
    /// The context made ready over the scratch world, given its root.
    pub ready: fn(Context, &Path) -> Context,
}

/// The directories a scratch copy of the world leaves out: build output and the instruments.
const UNCOPIED: [&str; 2] = ["target", "node_modules"];

/// Copies `from` into `to`: links as links, never followed; build output, the instruments and the
/// scratch left behind.
fn copy_world(from: &Path, to: &Path, top: bool) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name();
        let name_s = name.to_string_lossy();
        if top && UNCOPIED.contains(&name_s.as_ref()) {
            continue;
        }
        let kind = e.file_type()?;
        let dest = to.join(&name);
        if kind.is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(fs::read_link(e.path())?, &dest)?;
        } else if kind.is_dir() {
            if from.file_name().is_some_and(|n| n == ".mnemorph")
                && matches!(name_s.as_ref(), "instruments" | "tmp")
            {
                continue;
            }
            copy_world(&e.path(), &dest, false)?;
        } else {
            fs::copy(e.path(), &dest)?;
        }
    }
    Ok(())
}

/// The gate's facts from a run's journal: its last verdict, a pass on the first try, and the
/// holes it found (*tests intact*).
fn facts_of(journal: &Journal<Beat>) -> Facts {
    use mnemorph_ledger::evolution::Gate;
    let rows = journal.read().map(|r| r.rows).unwrap_or_default();
    let mut verdicts = Vec::new();
    let mut holes = 0u32;
    for b in rows {
        if let Beat::Gate {
            failures, verdict, ..
        } = b
        {
            holes = holes.saturating_add(
                u32::try_from(
                    failures
                        .iter()
                        .filter(|f| f.starts_with("tests intact:"))
                        .count(),
                )
                .unwrap_or(u32::MAX),
            );
            verdicts.push(match verdict.as_str() {
                "PASS" => Gate::Pass,
                "NOT_VERIFIED" => Gate::NotVerified,
                _ => Gate::Fail,
            });
        }
    }
    let gate = verdicts.last().copied();
    Facts {
        gate,
        first_try: verdicts.len() == 1 && gate == Some(Gate::Pass),
        holes_found: holes,
    }
}

/// The body `name`'s run over the scratch world `world`: Rimuru solo, or a living creature with
/// its own prompt, soil and verify lines; its gate denies every act that asks.
fn body_run(
    world: &Path,
    root: &Path,
    name: &str,
    c: &Controller<'_>,
    journal: Journal<Beat>,
) -> Result<Run, String> {
    let roster = Roster::load(world);
    let (body, cx, gate) = if name == "rimuru" {
        let (body, cx) = solo(world, &c.model_id);
        let verify = roster
            .creatures
            .get(name)
            .map(|cr| cr.verify.clone())
            .unwrap_or_default();
        let gate = EndGate {
            verify,
            verify_secs: c.config.verify_secs(),
            author: None,
        };
        (body, cx, gate)
    } else {
        let creature = roster
            .creatures
            .get(name)
            .filter(|cr| !cr.retired)
            .ok_or_else(|| format!("{name} is no living creature of the world"))?;
        let cx = crate::commission::context_for(world, &roster, name)
            .ok_or_else(|| format!("{name} has no soil"))?;
        let body = crate::run::Body {
            name: name.to_owned(),
            race: creature.race,
            model: c.model_id.clone(),
            system: crate::commission::prompt(world, &roster, name, "human"),
        };
        let gate = EndGate {
            verify: creature.verify.clone(),
            verify_secs: c.config.verify_secs(),
            author: Some(Author {
                name: name.to_owned(),
                roster: roster.clone(),
            }),
        };
        (body, cx, gate)
    };
    let race = body.race;
    let (cx, limits) = configure(
        c.config,
        cx.with_gate(Arc::new(NoHuman)),
        Limits::for_race(race),
    );
    let cx = (c.ready)(cx, root);
    let mut tools = registry();
    tools.push(Box::new(crate::wear_tool::WearTool));
    let setup = Setup {
        model: Arc::clone(&c.model),
        tools,
        cx,
        journal,
        ledger: None,
        prices: anthropic_prices().map_err(|e| format!("prices: {e}"))?,
        caller: Caller {
            session: "evolution-control".into(),
            run: format!("control-{name}"),
            body: name.to_owned(),
            race: if race == Race::Rimuru {
                "rimuru".into()
            } else {
                race.name().into()
            },
            commission: None,
            prefix: 0,
        },
    };
    Ok(Run::new(body, limits, setup)
        .with_gate(gate)
        // A control is stamped by its runner, never by the scratch world's own ledger.
        .with_evolution_ledger(root.join("no-ledger.jsonl")))
}

/// Runs, for swap `id` on the ledger at `at`, every control its window still waits on, and
/// appends each as a control row. Returns one line per control run.
///
/// # Errors
///
/// The swap cannot be judged, it was swapped in under another build, a window run kept no ask, a
/// scratch world cannot be made or put back to the before state, or a row cannot be written.
pub async fn run_controls(
    world: &Path,
    at: &Path,
    id: &str,
    c: &Controller<'_>,
) -> Result<Vec<String>, String> {
    let read = ledger::read(at, None).map_err(|e| format!("the ledger cannot be read: {e}"))?;
    let (swap, waiting) = ledger::awaiting(&read, id).map_err(|r| format!("refused: {r}"))?;
    if waiting.is_empty() {
        return Ok(Vec::new());
    }
    if swap.build != Build::this() {
        return Err(format!(
            "swap `{id}` was made by build {}; this one is {}: its controls run under its own build",
            swap.build,
            Build::this()
        ));
    }
    let mut lines = Vec::new();
    for (n, w) in waiting.iter().enumerate() {
        let ask = w
            .ask
            .as_deref()
            .filter(|a| fingerprint(a) == w.task)
            .ok_or_else(|| {
                format!(
                    "task {}: its ask was not kept, so no control can run",
                    w.task
                )
            })?;
        let root =
            std::env::temp_dir().join(format!("mnemorph-control-{id}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let scratch = root.join(world.file_name().unwrap_or_else(|| "world".as_ref()));
        let ran = control(world, at, swap, w.body.as_str(), ask, &root, &scratch, c).await;
        let _ = fs::remove_dir_all(&root);
        let short: String = w.task.chars().take(12).collect();
        match ran? {
            Some(f) => lines.push(format!(
                "control of task {short} by {}: {}",
                w.body,
                f.gate.map_or("nothing landed", |g| match g {
                    ledger::Gate::Pass => "pass",
                    ledger::Gate::Fail => "fail",
                    ledger::Gate::NotVerified => "not verified",
                })
            )),
            None => lines.push(format!(
                "control of task {short} by {}: interrupted, not recorded",
                w.body
            )),
        }
    }
    Ok(lines)
}

/// One control: the scratch world made and put back to the before state, the ask run, the row
/// written. `None` when the run was interrupted.
#[allow(clippy::too_many_arguments, reason = "one control's whole setting")]
async fn control(
    world: &Path,
    at: &Path,
    swap: &ledger::Swap,
    body: &str,
    ask: &str,
    root: &Path,
    scratch: &Path,
    c: &Controller<'_>,
) -> Result<Option<Facts>, String> {
    copy_world(world, scratch, true).map_err(|e| format!("the scratch world: {e}"))?;
    restore_before(at, swap, scratch).map_err(|e| format!("the before state: {e}"))?;
    let state = state_now(scratch, swap).map_err(|e| format!("the before state: {e}"))?;
    if state != swap.before_state() {
        return Err("the scratch world does not hold the named files as they were before".into());
    }
    let journal: Journal<Beat> = Journal::new(root.join("control.jsonl"));
    let mut run = body_run(scratch, root, body, c, journal.clone())?;
    let stamp = Stamp {
        ledger: at.to_path_buf(),
        swap: swap.clone(),
        body: body.to_owned(),
        ask: ask.to_owned(),
        state,
        control: true,
    };
    let mut quiet = |_| {};
    if matches!(run.ask(ask, &mut quiet).await, Ending::Interrupted) {
        return Ok(None);
    }
    let facts = facts_of(&journal);
    stamp
        .record(scratch, facts)
        .map_err(|e| format!("the control row: {e}"))?;
    Ok(Some(facts))
}

/// What judging on demand came to: the controls run, then the judgement.
pub struct Judged {
    /// One line per control run.
    pub controls: Vec<String>,
    /// The judgement, a proposal.
    pub judgement: Judgement,
}

/// `mnemorphd evolution judge <id>`: runs the controls swap `id` waits on in `world` — through the
/// configured model, its key read from `env` — then judges it from the world's ledger, held to its
/// anchored head, on `evolution.min_net_wins`. No model is reached when no control waits.
///
/// # Errors
///
/// The configuration does not load, there is no home for the ledger, the model cannot be reached,
/// or a control cannot run.
pub fn judge(
    world: &Path,
    id: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Judged, String> {
    let config = Config::load(&Paths::resolve(world))?;
    let at: PathBuf = mnemorph_world::evolution::ledger_of(world)
        .ok_or("no home to keep the evolution ledger under")?;
    let read = ledger::read(&at, None).map_err(|e| format!("the ledger cannot be read: {e}"))?;
    let waits = ledger::awaiting(&read, id).is_ok_and(|(_, w)| !w.is_empty());
    let controls = if waits {
        let (model, backend, _) = crate::open::model_of(&config, env)?;
        let c = Controller {
            model,
            model_id: backend.model,
            config: &config,
            ready: |cx, _| cx,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("the runtime: {e}"))?;
        rt.block_on(run_controls(world, &at, id, &c))?
    } else {
        Vec::new()
    };
    let judgement = mnemorph_world::evolution::judge(world, id, config.evolution_min_net_wins())
        .map_err(|e| format!("the ledger cannot be read: {e}"))?;
    Ok(Judged {
        controls,
        judgement,
    })
}
