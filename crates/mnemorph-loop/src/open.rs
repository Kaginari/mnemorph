//! Opening a session from a world's configuration: the model through the human's backend, the
//! tools, the kernel guard, the journal, the ledger and the decisions — everything a client needs
//! to talk to one body, built in one place so every client gets the same one.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mnemorph_config::{Config, Paths};
use mnemorph_ledger::Journal;
use mnemorph_provider::{
    Anthropic, Caller, HttpTransport, OpenAiCompatible, Policy, anthropic_prices,
};
use mnemorph_races::Race;
use mnemorph_tools::gate::Gate;
use mnemorph_tools::registry;

use crate::decisions::decisions;
use crate::model::{AnthropicModel, Model, OpenAiModel};
use crate::run::{Body, Limits, Observer, Run, Setup};
use crate::session::Session;
use crate::solo::{configure, solo};

/// The session's run, given what it opens with and answers to: the journal's last entries — a
/// fresh run's alone; a resumed one was told them when it began — the commissioner over its
/// lanes, the human's caps and the decider.
fn equip(
    mut run: Run,
    world: &Path,
    commissioner: Arc<crate::commission::Commissioner>,
    dispatches: bool,
    caps: Option<mnemorph_ledger::Caps>,
    decided: Option<crate::Decisions>,
) -> Run {
    if run.steps() == 0 && run.messages().is_empty() {
        run = run.with_opening(mnemorph_world::start::recent(
            world,
            OPENING_ENTRIES,
            OPENING_CAP,
        ));
    }
    let roster = mnemorph_world::Roster::load(world);
    run = run.with_manifest(mnemorph_world::minds::manifest(world, &roster, "rimuru"));
    // The throne sends the Dark Elves a landing's challenge routes, lanes or none.
    run = run.with_challenger(Arc::clone(&commissioner));
    if dispatches {
        run = run.with_commissioner(commissioner);
    }
    if let Some(caps) = caps {
        run = run.with_caps(caps);
    }
    if let Some(d) = decided {
        run = run.with_decisions(d);
    }
    run
}

/// How many of the journal's last entries a fresh session opens with: told what earlier sessions
/// learned by the harness itself, never by a file the host may or may not load.
const OPENING_ENTRIES: usize = 5;
/// The most of them it carries, in characters.
const OPENING_CAP: usize = 8_000;

/// A session, opened, and what a client shows about it.
pub struct Opened {
    /// The session.
    pub session: Session,
    /// The world it works in.
    pub world: PathBuf,
    /// The model, as the configuration names it.
    pub model: String,
    /// The provider that serves it.
    pub provider: String,
    /// The session's id.
    pub session_id: String,
    /// The run's id: its journal is `.mnemorph/instruments/loop/<run>.jsonl`.
    pub run_id: String,
    /// The configuration's holes — keys it did not take, and why — under a cap, the usage
    /// ledger's lines that cannot be read, each outward act a probe left in `reconcile`, and each open
    /// incident with the receipts it is missing.
    pub holes: Vec<String>,
    /// What the session may spend under the human's caps, when they set any.
    pub budget: Option<crate::budget::Left>,
    /// Where the provider's key comes from — the variable and the file that names it, never the
    /// value; `None` when the provider takes no key.
    pub key: Option<String>,
    /// The sandbox actually in force for the session's commands, or `none — <reason>`.
    pub sandbox: String,
    /// The MCP tools the session's body is offered, one priced line each, then the frame's
    /// total: what `status` shows. Empty when no server serves its race.
    pub mcp: Vec<String>,
}

/// Which run a session opens on.
enum Start<'a> {
    /// A new run.
    Fresh,
    /// A stopped run, by id or `last`, picked up from its journal.
    Resume(&'a str),
    /// A stopped run already found, checked and held.
    Held(String, crate::resume::Hold),
}

/// Open a solo session in `world`: its configuration loaded, its model reached through the
/// backend the human chose, its key read from the environment variable the provider names
/// (`env`), the human's gate answering every act that asks.
///
/// # Errors
///
/// The configuration does not load, the model's backend is unknown, its key variable is not set,
/// or the HTTP stack cannot start.
pub fn open(
    world: &Path,
    gate: Arc<dyn Gate>,
    observer: Observer,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Opened, String> {
    open_from(world, Start::Fresh, gate, observer, env)
}

/// [`open`] on a stopped run — its id, or `last` for the newest that stopped — picked up from its
/// journal: the same configuration, provider, sandbox, end gate, observer and commissioner as a
/// fresh session's, only the run rebuilt rather than new. It keeps its model unless the
/// configuration now names another; then the switch is said to the body as a context update.
///
/// # Errors
///
/// What [`open`] refuses on, and: no such run, its journal does not read whole, it answered or
/// failed, or another live session holds it.
pub fn resume(
    world: &Path,
    run: &str,
    gate: Arc<dyn Gate>,
    observer: Observer,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Opened, String> {
    open_from(world, Start::Resume(run), gate, observer, env)
}

fn open_from(
    world: &Path,
    start: Start<'_>,
    gate: Arc<dyn Gate>,
    observer: Observer,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Opened, String> {
    // A run with nothing to resume is refused before any key is asked for.
    let start = match start {
        Start::Resume(which) => {
            let (run, hold, _) = crate::resume::take(world, which)?;
            Start::Held(run, hold)
        }
        other => other,
    };
    let config = Config::load(&Paths::resolve(world))?;
    let (model, backend, named) = model_of(&config, &env)?;
    let mut opened = build(
        world,
        &config,
        Bound {
            model,
            model_id: &backend.model,
            attic: None,
        },
        start,
        gate,
        observer,
        env,
    )?;
    opened.key = backend.provider.key_env.as_ref().map(|var| {
        let field = format!("providers.{}.key_env", backend.provider_name);
        match config.get(&field) {
            Some(leaf) => format!("{var}, named by {field} in {}", leaf.origin),
            None => var.clone(),
        }
    });
    opened.model = named;
    opened.provider = backend.provider_name;
    Ok(opened)
}

/// The session's model, reached through the backend the human chose, its key read from the
/// variable the provider names: the model, its backend and the name the configuration gives it.
///
/// # Errors
///
/// The model's backend is unknown, its key variable is not set, or the HTTP stack cannot start.
pub fn model_of(
    config: &Config,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(Arc<dyn Model>, mnemorph_config::Backend, String), String> {
    let named = config.model_for("rimuru", Race::Rimuru, None);
    let backend = config.backend(&named)?;
    let key = match &backend.provider.key_env {
        Some(var) => Some(env(var).ok_or_else(|| {
            format!(
                "{var} is not set: provider `{}` reads its key from it",
                backend.provider_name
            )
        })?),
        None => None,
    };
    let transport =
        Arc::new(HttpTransport::new(Duration::from_secs(10)).map_err(|e| format!("HTTP: {e}"))?);
    let model: Arc<dyn Model> = match backend.provider.kind.as_str() {
        "anthropic" => Arc::new(AnthropicModel(Anthropic::new(
            transport,
            &backend.provider.base_url,
            key.ok_or_else(|| {
                format!(
                    "provider `{}` needs a key: name its variable with key_env",
                    backend.provider_name
                )
            })?,
            Policy::default(),
        ))),
        "openai" => Arc::new(OpenAiModel {
            client: OpenAiCompatible::new(
                transport,
                &backend.provider.base_url,
                key,
                Policy::default(),
            ),
            name: backend.provider_name.clone(),
        }),
        other => {
            return Err(format!(
                "provider `{}` is of kind `{other}`, which holds no conversation",
                backend.provider_name
            ));
        }
    };
    Ok((model, backend, named))
}

/// What [`open_with`] is given rather than built: the model, and where the attic lives.
pub struct Bound<'a> {
    /// The model.
    pub model: Arc<dyn Model>,
    /// The id it answers to.
    pub model_id: &'a str,
    /// The attic's directory; `None` for the human's state directory.
    pub attic: Option<&'a Path>,
}

/// The session's own end gate, as a commissioned body's: the verify lines of its card, each given
/// the configuration's `gate.verify_secs`, and no test deleted or skipped; the whole repository is
/// its soil, so no author is checked.
fn session_gate(world: &Path, name: &str, config: &Config) -> crate::gate::EndGate {
    crate::gate::EndGate {
        verify: mnemorph_world::Roster::load(world)
            .creatures
            .get(name)
            .map(|c| c.verify.clone())
            .unwrap_or_default(),
        verify_secs: config.verify_secs(),
        author: None,
    }
}

/// [`open`] with the model given: the configuration still sets the guard, the limits and the
/// decisions.
///
/// # Errors
///
/// The decisions' settings are invalid or their classifier's key is not set.
pub fn open_with(
    world: &Path,
    config: &Config,
    bound: Bound<'_>,
    gate: Arc<dyn Gate>,
    observer: Observer,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Opened, String> {
    build(world, config, bound, Start::Fresh, gate, observer, env)
}

/// [`resume`] with the model given, as [`open_with`] is [`open`].
///
/// # Errors
///
/// What [`open_with`] and [`resume`] refuse on.
pub fn resume_with(
    world: &Path,
    config: &Config,
    bound: Bound<'_>,
    run: &str,
    gate: Arc<dyn Gate>,
    observer: Observer,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Opened, String> {
    build(
        world,
        config,
        bound,
        Start::Resume(run),
        gate,
        observer,
        env,
    )
}

/// The one opening every session goes through, fresh or resumed.
// One straight sequence, each step reading the last: split, it would pass the same state around.
#[allow(clippy::too_many_lines)]
fn build(
    world: &Path,
    config: &Config,
    bound: Bound<'_>,
    start: Start<'_>,
    gate: Arc<dyn Gate>,
    observer: Observer,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Opened, String> {
    let Bound {
        model,
        model_id,
        attic,
    } = bound;
    let (session_id, fresh_run) = ids();
    // A resumed run is found, checked and held before anything is prepared: a refusal leaves the
    // world as it was.
    let (run_id, hold, resuming) = held_run(world, start, fresh_run)?;
    prepare(world);
    let (mut body, cx) = solo(world, model_id);
    // A world with a throne and Elves below it: Rimuru may hand a lane's work to its Elf.
    let lanes = lanes(world);
    if !lanes.is_empty() {
        body.system.push_str(&dispatch_prompt(&lanes));
        let roster = mnemorph_world::Roster::load(world);
        body.system
            .push_str(&crate::commission::cards_below(world, &roster, &body.name));
    }
    let (mut cx, mut limits) = configure(config, cx.with_gate(gate), Limits::for_race(body.race));
    limits.steps = session_steps(config, &lanes, limits.steps);
    if let Some(dir) = attic {
        cx = cx.with_attic(attic_of(config, dir, world)?);
    }
    let instruments = world.join(".mnemorph/instruments");
    // The caps against what this world already spent. A cap that cannot be read opens no
    // session: spend it cannot see would be spend it cannot stop.
    let usage = usage_ledger(world);
    let (caps, capped, unread, budget) = read_caps(config, &usage, &session_id)?;
    let mut tools = registry();
    tools.push(Box::new(crate::wear_tool::WearTool));
    if !lanes.is_empty() {
        tools.push(Box::new(crate::commission::CommissionTool));
        tools.push(Box::new(crate::queue_tool::QueueTool));
    }
    // The MCP servers the human declared: the body's share decided once, before the run is made.
    let (mcp, mcp_lines, mcp_holes) = mcp_share(config, body.race, &cx, &mut tools)?;
    let prices = anthropic_prices().map_err(|e| format!("prices: {e}"))?;
    // A dispatching session's envelope is what the human's caps leave: its slices carve from it.
    if !lanes.is_empty() {
        limits.budget_wt = budget.map(|b| b.wt);
        limits.budget_cap = budget.map(|b| b.cap);
    }
    // One decider for the session's run and its commissioner's routes.
    let decided = decisions(config, world, model.clone(), env)?;
    let commissioner = Arc::new(crate::commission::Commissioner {
        world: cx.world().to_path_buf(),
        model: model.clone(),
        prices: prices.clone(),
        ledger: Some(usage.clone()),
        journals: instruments.join("loop"),
        session: session_id.clone(),
        caps: capped.then_some(caps),
        stress_tokens: Some(config.context_stress_tokens()),
        verify_secs: Some(config.verify_secs()),
        weights: Some(crate::solo::recall_weights(config)),
        busy: std::sync::Mutex::default(),
        observer: Some(Arc::clone(&observer)),
        queue: Some(mnemorph_world::Queue::of(cx.world())),
        router: Some(router(config, decided.clone(), model.provider())),
        slice_floor: crate::commission::SLICE_FLOOR,
        council: council_keys(config),
        challenges: None,
        reviewers: config.challenge_reviewers(),
        mcp,
    });
    let setup = Setup {
        model: model.clone(),
        tools,
        cx,
        journal: Journal::new(instruments.join("loop").join(format!("{run_id}.jsonl"))),
        ledger: Some(usage),
        prices,
        caller: Caller {
            session: session_id.clone(),
            run: run_id.clone(),
            body: body.name.clone(),
            race: body.race.name().to_owned(),
            commission: None,
            prefix: 0,
        },
    };
    let gate = session_gate(world, &body.name, config);
    let run = if resuming {
        resumed(body, limits, setup, model_id)?
    } else {
        Run::new(body, limits, setup)
    }
    .with_observer(observer)
    .with_gate(gate);
    let run = equip(
        run,
        world,
        commissioner,
        !lanes.is_empty(),
        capped.then_some(caps),
        decided,
    );
    Ok(Opened {
        session: hold.into_iter().fold(Session::new(run), Session::holding),
        world: world.to_path_buf(),
        model: model_id.to_owned(),
        provider: model.provider().to_owned(),
        session_id,
        run_id,
        holes: holes(config, unread, world)
            .into_iter()
            .chain(mcp_holes)
            .collect(),
        budget,
        key: None,
        // The network is opened only for an approved outward act, so the standing state is off.
        sandbox: mnemorph_tools::Sandbox::detect().status(false),
        mcp: mcp_lines,
    })
}

/// The session's MCP servers, when the human declared any, and the body's share of them put on
/// its shelf: the priced lines for `status`, and what was refused.
#[allow(clippy::type_complexity)]
fn mcp_share(
    config: &Config,
    race: mnemorph_races::Race,
    cx: &mnemorph_tools::Context,
    tools: &mut Vec<Box<dyn mnemorph_tools::Tool>>,
) -> Result<
    (
        Option<Arc<mnemorph_tools::mcp::Mcp>>,
        Vec<String>,
        Vec<String>,
    ),
    String,
> {
    let settings = config.mcp()?;
    if settings.servers.is_empty() {
        return Ok((None, Vec::new(), Vec::new()));
    }
    let mcp = mnemorph_tools::mcp::Mcp::new(settings);
    let offer = mcp.offer(race, "", cx, &reserved(tools));
    let lines = offer.lines();
    let holes = offer.holes.clone();
    tools.extend(offer.tools);
    Ok((Some(mcp), lines, holes))
}

/// Every name an MCP tool may not take: the tools already on the shelf, and every tool the loop
/// or the act runner hands any body.
pub(crate) fn reserved(
    tools: &[Box<dyn mnemorph_tools::Tool>],
) -> std::collections::BTreeSet<String> {
    tools
        .iter()
        .map(|t| t.name().to_owned())
        .chain(
            [
                crate::commission::COMMISSION,
                crate::queue_tool::QUEUE,
                crate::wear_tool::WEAR,
                crate::verify_tool::VERIFY,
                crate::coverage_tool::COVERAGE,
                crate::verdict_tool::VERDICT,
                mnemorph_config::OUTWARD,
            ]
            .map(str::to_owned),
        )
        .chain(
            mnemorph_tools::registry()
                .iter()
                .map(|t| t.name().to_owned()),
        )
        .collect()
}

/// The session's attic at `dir`, kept within the configuration's limits.
fn attic_of(config: &Config, dir: &Path, world: &Path) -> Result<mnemorph_tools::Attic, String> {
    let (keep_days, max_mb) = config.attic_limits();
    let kept = mnemorph_tools::Attic::at(dir, world)
        .ok_or_else(|| format!("{}: the attic needs git", dir.display()))?;
    Ok(kept.with_limits(keep_days, max_mb))
}

/// What the session opens saying it cannot do or still waits on: the configuration's holes, the
/// caps it could not read, the outward acts waiting for the human, the open incidents.
fn holes(config: &Config, unread: Vec<String>, world: &Path) -> Vec<String> {
    config
        .holes()
        .iter()
        .map(|h| format!("{} ({}): {}", h.key, h.origin, h.why))
        .chain(unread)
        // An outward act whose effect a probe did not see waits for the human.
        .chain(mnemorph_config::reconciling(world))
        // An incident stays open until its four receipts are recorded.
        .chain(mnemorph_config::incidents(world))
        .collect()
}

/// The world made ready for a session: the report folder a check writes into, ignored locally —
/// a check never writes the tree — and git merging note files by union, so two machines that
/// both appended notes never conflict.
fn prepare(world: &Path) {
    if let Err(e) = mnemorph_world::report::prepare(world) {
        eprintln!("mnemorph: the report folder could not be prepared: {e}");
    }
    if let Err(e) = mnemorph_memory::union_merge(world) {
        eprintln!("mnemorph: the notes' union merge could not be set: {e}");
    }
}

/// The session's run id, its hold on it, and whether it is resumed: a fresh run's own, held, or
/// the stopped run `start` names, found, checked and held.
fn held_run(
    world: &Path,
    start: Start<'_>,
    fresh: String,
) -> Result<(String, Option<crate::resume::Hold>, bool), String> {
    match start {
        Start::Fresh => {
            // A run no lock holds would be resumable while it lives: no session opens on it.
            let hold = crate::resume::hold(world, &fresh).map_err(|e| {
                format!("the session's run cannot be held, so no session opens: {e}")
            })?;
            Ok((fresh, Some(hold), false))
        }
        Start::Resume(which) => {
            let (run, hold, _) = crate::resume::take(world, which)?;
            Ok((run, Some(hold), true))
        }
        Start::Held(run, hold) => Ok((run, Some(hold), true)),
    }
}

/// A stopped run rebuilt from its journal, on the model it rode — or on `model_id`, when the
/// configuration names another now, said to the body as a context update.
fn resumed(body: Body, limits: Limits, setup: Setup, model_id: &str) -> Result<Run, String> {
    let run_id = setup.caller.run.clone();
    let mut run = Run::resume(body, limits, setup)
        .map_err(|e| format!("run `{run_id}`: its journal cannot be read: {e}"))?;
    run.switch_model(model_id, "the configuration names it now");
    Ok(run)
}

/// What the human is told of ledger lines that cannot be read: their spend is counted high, as
/// `unseen` weighted tokens against every cap, until they are repaired or removed.
pub(crate) fn unreadable_lines(path: &Path, lines: &[usize], unseen: u64) -> String {
    format!(
        "the usage ledger {} has {} unreadable line(s) ({lines:?}): the caps count them as {unseen} \
         wt, each as large as the largest call, against each cap at most its hidden {}% — repair \
         or remove them to count them true",
        path.display(),
        lines.len(),
        mnemorph_ledger::UNSEEN_RESERVE_PCT
    )
}

/// The Elves that report to a living throne, with their lanes: whom Rimuru may commission.
fn lanes(world: &Path) -> Vec<(String, Vec<String>)> {
    let roster = mnemorph_world::Roster::load(world);
    if !roster.living().any(|c| c.name == "rimuru") {
        return Vec::new();
    }
    roster
        .living()
        .filter(|c| c.race.base() == Race::Elf && c.reports_to.as_deref() == Some("rimuru"))
        .map(|c| (c.name.clone(), c.lane.clone()))
        .collect()
}

/// What Rimuru is told of the lanes it may hand work to.
fn dispatch_prompt(lanes: &[(String, Vec<String>)]) -> String {
    let list: Vec<String> = lanes
        .iter()
        .map(|(elf, lane)| {
            if lane.is_empty() {
                format!("- {elf}")
            } else {
                format!("- {elf}: {}", lane.join(", "))
            }
        })
        .collect();
    format!(
        "\n\nYou are not alone here: this world has creatures. Work that belongs to a lane goes to its \
Elf with the commission \
tool: it plans, its Orcs judge, its Slimes write, each under its own gate, and the report comes back \
as the tool's result. Small asks you may still do yourself. When a report carries `@? triage #<new> …`, \
an Elf's new fact resembles older ones: label it in your answer, on a line of its own — \
`@V #<new> support|counter|refines #<id>`, or `@V #<new> distinct`. The lanes:\n{}",
        list.join("\n")
    )
}

/// The council's keys, from the configuration's `stress.*`, else their defaults.
fn council_keys(config: &Config) -> crate::council::Thresholds {
    let d = crate::council::Thresholds::default();
    crate::council::Thresholds {
        escalate_at: config.stress("escalate_at").unwrap_or(d.escalate_at),
        relieve_at: config.stress("relieve_at").unwrap_or(d.relieve_at),
        max_consults: config.stress("max_consults").unwrap_or(d.max_consults),
    }
}

/// What chooses each court's model: the configuration, Metis's decider when routing is on, and
/// the session's provider, as its client names it.
fn router(
    config: &Config,
    decisions: Option<crate::decisions::Decisions>,
    provider: &str,
) -> crate::commission::Router {
    crate::commission::Router {
        config: Arc::new(config.clone()),
        decisions,
        provider: provider.to_owned(),
    }
}

/// The world's usage ledger, chained — every row names the digest of the one before, so an edit
/// of the spend is found — and witnessed under the human's state directory, so the tripwire puts
/// it back with a parallel run's rows kept.
pub(crate) fn usage_ledger(world: &Path) -> Journal<mnemorph_ledger::UsageRow> {
    let usage = Journal::chained(world.join(".mnemorph/instruments/usage.jsonl"));
    match mnemorph_tools::usage_witness(world) {
        Some(witness) => usage.witnessed_at(witness),
        None => usage,
    }
}

/// The human's caps, and what this world already spent against them: whether any is set, the
/// ledger lines that could not be read, and what the session may spend. A ledger that cannot be
/// read opens no capped session.
pub(crate) fn read_caps(
    config: &Config,
    usage: &Journal<mnemorph_ledger::UsageRow>,
    session_id: &str,
) -> Result<
    (
        crate::budget::Caps,
        bool,
        Vec<String>,
        Option<crate::budget::Left>,
    ),
    String,
> {
    let caps = crate::budget::Caps {
        session: config.budget_wt("session"),
        day: config.budget_wt("day"),
        month: config.budget_wt("month"),
    };
    let capped = caps != crate::budget::Caps::default();
    let mut unread = Vec::new();
    let (spent, unseen) = if capped {
        let back = usage
            .read()
            .map_err(|e| format!("the usage ledger: {e}; the budget caps cannot be read"))?;
        let unseen = mnemorph_ledger::unseen_wt(&back.rows, back.skipped.len());
        if !back.skipped.is_empty() {
            unread.push(unreadable_lines(usage.path(), &back.skipped, unseen));
        }
        (back.rows, unseen)
    } else {
        (Vec::new(), 0)
    };
    let budget = crate::budget::left(
        caps,
        &spent,
        session_id,
        mnemorph_ledger::Timestamp::now(),
        unseen,
    );
    Ok((caps, capped, unread, budget))
}

/// The session body's steps: working solo — no Elf to dispatch to — the throne does the chain's
/// work and gets the chain's steps; dispatching, it keeps its own.
fn session_steps<T>(config: &Config, lanes: &[T], own: u32) -> u32 {
    if lanes.is_empty() {
        config.solo_steps()
    } else {
        own
    }
}

/// A new session's id and its first run's: the clock in milliseconds and the process.
fn ids() -> (String, String) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let pid = std::process::id();
    (format!("s{stamp}-{pid}"), format!("r{stamp}-{pid}"))
}
