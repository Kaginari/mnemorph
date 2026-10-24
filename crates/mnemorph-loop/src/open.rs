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
use crate::run::{Limits, Observer, Run, Setup};
use crate::session::Session;
use crate::solo::{configure, solo};

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
    /// The configuration's holes: keys it did not take, and why.
    pub holes: Vec<String>,
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
    let config = Config::load(&Paths::resolve(world))?;
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
    let mut opened = open_with(
        world,
        &config,
        Bound {
            model,
            model_id: &backend.model,
            attic: None,
        },
        gate,
        observer,
        env,
    )?;
    opened.model = named;
    opened.provider = backend.provider_name;
    Ok(opened)
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
    let Bound {
        model,
        model_id,
        attic,
    } = bound;
    let (body, cx) = solo(world, model_id);
    let (mut cx, limits) = configure(config, cx.with_gate(gate), Limits::for_race(body.race));
    if let Some(dir) = attic {
        let (keep_days, max_mb) = config.attic_limits();
        let kept = mnemorph_tools::Attic::at(dir, world)
            .ok_or_else(|| format!("{}: the attic needs git", dir.display()))?;
        cx = cx.with_attic(kept.with_limits(keep_days, max_mb));
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let session_id = format!("s{stamp}-{}", std::process::id());
    let run_id = format!("r{stamp}-{}", std::process::id());
    let instruments = world.join(".mnemorph/instruments");
    let setup = Setup {
        model: model.clone(),
        tools: registry(),
        cx,
        journal: Journal::new(instruments.join("loop").join(format!("{run_id}.jsonl"))),
        ledger: Some(Journal::new(instruments.join("usage.jsonl"))),
        prices: anthropic_prices().map_err(|e| format!("prices: {e}"))?,
        caller: Caller {
            session: session_id.clone(),
            run: run_id.clone(),
            body: body.name.clone(),
            race: body.race.name().to_owned(),
            commission: None,
            prefix: 0,
        },
    };
    let mut run = Run::new(body, limits, setup).with_observer(observer);
    if let Some(d) = decisions(config, world, model.clone(), env)? {
        run = run.with_decisions(d);
    }
    Ok(Opened {
        session: Session::new(run),
        world: world.to_path_buf(),
        model: model_id.to_owned(),
        provider: model.provider().to_owned(),
        session_id,
        run_id,
        holes: config
            .holes()
            .iter()
            .map(|h| format!("{} ({}): {}", h.key, h.origin, h.why))
            .collect(),
    })
}
