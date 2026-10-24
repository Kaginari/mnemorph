//! Configuration: TOML layers whose every value remembers where it came from.
//!
//! Four layers, lowest first: the compiled **defaults**, the human's **global** file, the
//! project's **tracked** file (`.mnemorph/config.toml`, shared with everyone who clones it) and its
//! **local** file (`.mnemorph/config.local.toml`, the human's own). A higher layer wins a scalar
//! or a list; tables merge key by key, each leaf keeping its own origin; an empty list or table
//! never overrides a lower layer.
//!
//! The tracked file may only tighten the human gate. What loosens it — guard rules that ask
//! instead of refusing, a gate turned off, standing approvals, Oni envelopes — is honoured only
//! from the local or global file. Nothing else feeds those keys: not the environment, not a flag,
//! and not a body, which cannot write `.mnemorph/` at all. An envelope is bounded besides: it is
//! signed on a day, expires at most `oni.envelope_days` later (30 unless the human's own file
//! says, never more than a year), may cap its uses, and stays in the file once revoked, so the
//! record of what was allowed is never lost.
//!
//! Unknown keys are reported as holes and the session goes on; a file that does not parse, a
//! value of the wrong type, a floor rule asked to ask, a destructive envelope or a fallback cycle
//! is an error that names its file.

mod write;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_ledger::Day;
use mnemorph_races::{ModelTier, Race};
use mnemorph_tools::{Place, Rule, Verdict, classify};
use toml::{Table, Value};

pub use write::{revoke, set};

/// The compiled defaults: the lowest layer, and the reference `config explain` shows.
const DEFAULTS: &str = r#"
[gate]
enabled = true
timeout_secs = 300

[stress]
escalate_at = 5
relieve_at = 7
max_consults = 3

[evolution]
mode = "auto"

[models.tier]
frontier = "claude-opus-5-5"
strong = "claude-sonnet-5-5"
fast = "claude-haiku-5-5"

[providers.anthropic]
kind = "anthropic"
base_url = "https://api.anthropic.com"
key_env = "ANTHROPIC_API_KEY"

[decide.cutoff]
measured = 0.6
stated = 0.9
"#;

/// Keys only the local or global file may set: each one loosens the human gate.
const LOOSENING: &[&str] = &[
    "gate.enabled",
    "gate.ask_rules",
    "gate.approve",
    "oni.envelopes",
    "oni.envelope_days",
    "sandbox.allow_unguarded",
    "decide.via",
    "decide.cutoff.measured",
    "decide.cutoff.stated",
    "decide.route",
];

/// The kind of value a known key holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bool,
    Int,
    Number,
    Str,
    StrList,
    Envelopes,
}

/// Every key the configuration knows; `*` stands for one segment of the human's choosing.
const SCHEMA: &[(&str, Kind)] = &[
    ("gate.enabled", Kind::Bool),
    ("gate.timeout_secs", Kind::Int),
    ("gate.ask_rules", Kind::StrList),
    ("gate.approve", Kind::StrList),
    ("sandbox.allow_unguarded", Kind::Bool),
    ("attic.keep_days", Kind::Int),
    ("attic.max_mb", Kind::Int),
    ("budget.month_usd", Kind::Number),
    ("budget.day_usd", Kind::Number),
    ("budget.session_usd", Kind::Number),
    ("stress.escalate_at", Kind::Int),
    ("stress.relieve_at", Kind::Int),
    ("stress.max_consults", Kind::Int),
    ("stress.context_tokens", Kind::Int),
    ("evolution.mode", Kind::Str),
    ("models.tier.*", Kind::Str),
    ("models.office.*.*", Kind::Str),
    ("models.creature.*", Kind::Str),
    ("models.fallback.*", Kind::Str),
    ("providers.*.kind", Kind::Str),
    ("providers.*.base_url", Kind::Str),
    ("providers.*.key_env", Kind::Str),
    ("decide.via", Kind::Str),
    ("decide.model", Kind::Str),
    ("decide.llm", Kind::Str),
    ("decide.bench", Kind::Bool),
    ("decide.route", Kind::Bool),
    ("decide.cutoff.measured", Kind::Number),
    ("decide.cutoff.stated", Kind::Number),
    ("oni.envelopes", Kind::Envelopes),
    ("oni.envelope_days", Kind::Int),
];

/// A configuration layer, lowest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// Compiled into the binary.
    Default,
    /// The human's own, for every world.
    Global,
    /// The project's, tracked in git: everyone who clones it gets it.
    Tracked,
    /// The project's, on this machine only.
    Local,
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Layer::Default => "default",
            Layer::Global => "global",
            Layer::Tracked => "tracked",
            Layer::Local => "local",
        })
    }
}

/// Where a value came from: its layer and the exact file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// The layer.
    pub layer: Layer,
    /// The file; `None` for the compiled defaults.
    pub file: Option<PathBuf>,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.file {
            Some(file) => write!(f, "{} ({})", self.layer, file.display()),
            None => write!(f, "{} (built in)", self.layer),
        }
    }
}

/// The files each layer reads, resolved once and shared by every reader and writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// The global file.
    pub global: PathBuf,
    /// The project's tracked file.
    pub tracked: PathBuf,
    /// The project's local file.
    pub local: PathBuf,
}

impl Paths {
    /// The files for `world`: `$XDG_CONFIG_HOME/mnemorph/config.toml` (or `config` under the
    /// home from the platform, never a literal `~`), then the world's two.
    #[must_use]
    pub fn resolve(world: &Path) -> Self {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::home_dir().map(|h| h.join(".config")))
            .unwrap_or_else(|| PathBuf::from("/nonexistent"));
        Self::with_global(config_home.join("mnemorph/config.toml"), world)
    }

    /// The world's two files beside an explicit global one.
    #[must_use]
    pub fn with_global(global: PathBuf, world: &Path) -> Self {
        let dir = world.join(".mnemorph");
        Self {
            global,
            tracked: dir.join("config.toml"),
            local: dir.join("config.local.toml"),
        }
    }

    fn of(&self, layer: Layer) -> Option<&Path> {
        match layer {
            Layer::Default => None,
            Layer::Global => Some(&self.global),
            Layer::Tracked => Some(&self.tracked),
            Layer::Local => Some(&self.local),
        }
    }
}

/// A value and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Leaf {
    /// The value.
    pub value: Value,
    /// Its origin.
    pub origin: Origin,
}

/// A key the configuration did not take, and why — reported, never fatal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    /// The dotted key.
    pub key: String,
    /// The file it was found in.
    pub origin: Origin,
    /// Why it was not taken.
    pub why: String,
}

/// The merged configuration.
#[derive(Debug, Clone, Default)]
pub struct Config {
    leaves: BTreeMap<String, Leaf>,
    holes: Vec<Hole>,
}

/// One line of `config explain`.
#[derive(Debug, Clone, PartialEq)]
pub struct Explained {
    /// The dotted key.
    pub key: String,
    /// The value, as TOML writes it.
    pub value: String,
    /// Where it came from.
    pub origin: Origin,
}

/// Which model rides a body, and which provider serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    /// `anthropic`, `openai` (any OpenAI-compatible server), or `systemone` (TypeSafe's API or
    /// a jeff server: typed decisions, not conversations).
    pub kind: String,
    /// The server's base URL.
    pub base_url: String,
    /// The environment variable holding its key (never the key itself).
    pub key_env: Option<String>,
}

/// How decisions are made: a System One provider when `via` names one, else the LLM `llm`.
#[derive(Debug, Clone, PartialEq)]
pub struct Decide {
    /// The System One provider tried first (`kind = "systemone"`), when one is configured.
    pub via: Option<String>,
    /// The model to ask at `via` (`jev-latest` at TypeSafe, `jeff-latest` at a jeff server);
    /// empty when there is no `via`.
    pub model: String,
    /// The LLM that answers when there is no `via`, or it fails.
    pub llm: String,
    /// Whether Metis routes a commission to a tier by how demanding it is (`decide.route`): off
    /// unless the human turns it on, since it changes which model does the work.
    pub route: bool,
    /// Whether a piped test run's output is read by a decision instead of counted muted: as
    /// `decide.bench` says, else on exactly when a System One provider is named, so no LLM is
    /// asked unless the human chose it.
    pub bench: bool,
    /// The least confidence a System One answer needs before a caller acts on it.
    pub cutoff_measured: f64,
    /// The same for an LLM's answer, whose probabilities are its own guess.
    pub cutoff_stated: f64,
}

/// How many days an envelope may last, and lasts when it names no `expires`, unless the human's
/// own file sets `oni.envelope_days`.
pub const ENVELOPE_DAYS: i64 = 30;

/// The longest `oni.envelope_days` may be: no setting brings back an envelope that never ends.
pub const ENVELOPE_DAYS_CEILING: i64 = 365;

/// Standing permissions the human signed for Oni: outward acts that run without a prompt, for a
/// bounded time and, when the human says so, a bounded number of acts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Its name, for the ledger and `status`.
    pub name: String,
    /// The command patterns it covers; `*` at the end of a word matches the rest.
    pub allow: Vec<String>,
    /// The day the human signed it; it covers nothing before.
    pub signed: Day,
    /// The first day it covers nothing: its own `expires`, at most `oni.envelope_days` after
    /// `signed`, and exactly that when it names none.
    pub expires: Day,
    /// How many acts it may cover in all, counted by the ledger; `None` caps only by time.
    pub uses: Option<u64>,
    /// The day the human revoked it. A revoked envelope stays in the file and covers nothing.
    pub revoked: Option<Day>,
}

/// Why an envelope does not cover an act on a given day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lapse {
    /// Signed for a later day.
    NotYet(Day),
    /// Expired on this day.
    Expired(Day),
    /// Revoked on this day.
    Revoked(Day),
    /// Every one of this many uses is spent.
    Spent(u64),
}

impl fmt::Display for Lapse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lapse::NotYet(day) => write!(f, "signed for {day}, not yet in force"),
            Lapse::Expired(day) => write!(f, "expired on {day}"),
            Lapse::Revoked(day) => write!(f, "revoked on {day}"),
            Lapse::Spent(uses) => write!(f, "all {uses} uses spent"),
        }
    }
}

impl Envelope {
    /// Whether the envelope is in force on `today`, after `used` acts it already covered (the
    /// ledger's count). Whether a command matches `allow` is the gate's question; this is only
    /// the envelope's own time and count, read at the moment of use, never only at plan time.
    ///
    /// # Errors
    ///
    /// The [`Lapse`] that stops it: revoked, not yet signed, expired or spent, in that order.
    pub fn admits(&self, today: Day, used: u64) -> Result<(), Lapse> {
        if let Some(day) = self.revoked {
            return Err(Lapse::Revoked(day));
        }
        if today < self.signed {
            return Err(Lapse::NotYet(self.signed));
        }
        if today >= self.expires {
            return Err(Lapse::Expired(self.expires));
        }
        match self.uses {
            Some(cap) if used >= cap => Err(Lapse::Spent(cap)),
            _ => Ok(()),
        }
    }
}

impl Config {
    /// Read every layer for these paths. A missing file is an empty layer.
    ///
    /// # Errors
    ///
    /// A file that does not parse, a known key with the wrong type, a floor rule asked to ask,
    /// an envelope covering a destructive act, a fallback cycle — each naming its file.
    pub fn load(paths: &Paths) -> Result<Self, String> {
        let mut config = Self::default();
        let defaults: Table =
            toml::from_str(DEFAULTS).map_err(|e| format!("the built-in defaults: {e}"))?;
        config.layer(
            &defaults,
            &Origin {
                layer: Layer::Default,
                file: None,
            },
        )?;
        for layer in [Layer::Global, Layer::Tracked, Layer::Local] {
            let Some(path) = paths.of(layer) else {
                continue;
            };
            let text = match fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            };
            let table: Table =
                toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            config.layer(
                &table,
                &Origin {
                    layer,
                    file: Some(path.to_path_buf()),
                },
            )?;
        }
        config.validate()?;
        Ok(config)
    }

    fn layer(&mut self, table: &Table, origin: &Origin) -> Result<(), String> {
        let mut leaves = Vec::new();
        flatten(table, "", &mut leaves);
        for (key, value) in leaves {
            let Some(kind) = kind_of(&key) else {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "not a key Mnemorph knows".into(),
                });
                continue;
            };
            check_kind(&key, &value, kind, origin)?;
            if origin.layer == Layer::Tracked && key.starts_with("providers.") {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "a provider decides where a key is sent, so it is honoured only from the local or global file".into(),
                });
                continue;
            }
            if origin.layer == Layer::Tracked && LOOSENING.contains(&key.as_str()) {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "loosens the human gate, so it is honoured only from the local or global file".into(),
                });
                continue;
            }
            if value.as_array().is_some_and(Vec::is_empty) {
                continue;
            }
            self.leaves.insert(
                key,
                Leaf {
                    value,
                    origin: origin.clone(),
                },
            );
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        for (key, leaf) in &self.leaves {
            let at = &leaf.origin;
            match key.as_str() {
                "gate.ask_rules" => {
                    for name in strings(&leaf.value) {
                        let rule = Rule::from_name(name).ok_or_else(|| {
                            let known: Vec<&str> = Rule::ALL.iter().map(|r| r.name()).collect();
                            format!(
                                "{at}: gate.ask_rules: no guard rule `{name}`; the rules are {}",
                                known.join(", ")
                            )
                        })?;
                        if rule.floor() {
                            return Err(format!(
                                "{at}: gate.ask_rules: `{name}` is on the floor and always refuses"
                            ));
                        }
                    }
                }
                "evolution.mode" if !matches!(leaf.value.as_str(), Some("auto" | "propose")) => {
                    return Err(format!("{at}: evolution.mode is `auto` or `propose`"));
                }
                "oni.envelopes" => {
                    envelopes(&leaf.value, at, self.envelope_days())?;
                }
                "oni.envelope_days"
                    if !leaf
                        .value
                        .as_integer()
                        .is_some_and(|d| (1..=ENVELOPE_DAYS_CEILING).contains(&d)) =>
                {
                    return Err(format!(
                        "{at}: oni.envelope_days is a count of days from 1 to {ENVELOPE_DAYS_CEILING}"
                    ));
                }
                _ => {}
            }
            if let Some(rest) = key.strip_prefix("models.office.") {
                let (race, office) = rest.split_once('.').unwrap_or((rest, ""));
                if Race::from_name(race).is_none() {
                    return Err(format!("{at}: models.office.{race}: no race `{race}`"));
                }
                if let Some((name, function)) = office_names(office)
                    && self
                        .leaves
                        .contains_key(&format!("models.office.{race}.{name}"))
                    && self
                        .leaves
                        .contains_key(&format!("models.office.{race}.{function}"))
                {
                    return Err(format!(
                        "{at}: models.office.{race} names one office twice, as {name} and {function}: keep one"
                    ));
                }
                if office_names(office).is_none() {
                    return Err(format!(
                        "{at}: models.office.{race}.{office}: the offices are mneme (read), melete (judge) and aoede (write)"
                    ));
                }
            }
            if let Some(tier) = key.strip_prefix("models.tier.")
                && !matches!(tier, "frontier" | "strong" | "fast")
            {
                return Err(format!(
                    "{at}: models.tier.{tier}: the tiers are frontier, strong and fast"
                ));
            }
        }
        for model in self
            .leaves
            .keys()
            .filter_map(|k| k.strip_prefix("models.fallback."))
        {
            self.fallback_chain(model)?;
        }
        Ok(())
    }

    /// The keys the configuration did not take.
    #[must_use]
    pub fn holes(&self) -> &[Hole] {
        &self.holes
    }

    /// A value and its origin.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Leaf> {
        self.leaves.get(key)
    }

    /// Every value under `prefix` (all of them for `""`) with its origin: scalars, lists, and
    /// each leaf of a table.
    #[must_use]
    pub fn explain(&self, prefix: &str) -> Vec<Explained> {
        self.leaves
            .iter()
            .filter(|(k, _)| {
                prefix.is_empty() || *k == prefix || k.starts_with(&format!("{prefix}."))
            })
            .map(|(k, l)| Explained {
                key: k.clone(),
                value: l.value.to_string(),
                origin: l.origin.clone(),
            })
            .collect()
    }

    /// The human gate is on (only the local or global file can turn it off).
    #[must_use]
    pub fn gate_enabled(&self) -> bool {
        self.get("gate.enabled")
            .and_then(|l| l.value.as_bool())
            .unwrap_or(true)
    }

    /// How long the gate waits before denying.
    #[must_use]
    pub fn gate_timeout_secs(&self) -> u64 {
        self.int("gate.timeout_secs").unwrap_or(300)
    }

    /// Commands may run with no kernel guard in force (local or global file only).
    #[must_use]
    pub fn allow_unguarded(&self) -> bool {
        self.get("sandbox.allow_unguarded")
            .and_then(|l| l.value.as_bool())
            .unwrap_or(false)
    }

    /// How long the attic keeps snapshots, in days, and how large it may grow, in megabytes.
    #[must_use]
    pub fn attic_limits(&self) -> (u64, u64) {
        (
            self.int("attic.keep_days").unwrap_or(14),
            self.int("attic.max_mb").unwrap_or(2048),
        )
    }

    /// The guard rules the human turned into questions.
    #[must_use]
    pub fn asked_rules(&self) -> Vec<Rule> {
        self.get("gate.ask_rules")
            .map(|l| strings(&l.value).filter_map(Rule::from_name).collect())
            .unwrap_or_default()
    }

    /// A budget cap in dollars: `month`, `day` or `session`.
    #[must_use]
    pub fn budget_usd(&self, period: &str) -> Option<f64> {
        self.get(&format!("budget.{period}_usd")).and_then(|l| {
            l.value.as_float().or_else(|| {
                l.value
                    .as_integer()
                    .and_then(|i| u32::try_from(i).ok())
                    .map(f64::from)
            })
        })
    }

    /// A stress threshold: `escalate_at`, `relieve_at` or `max_consults`.
    #[must_use]
    pub fn stress(&self, name: &str) -> Option<u64> {
        self.int(&format!("stress.{name}"))
    }

    /// The context stress line in tokens: absolute, the same whatever the model.
    #[must_use]
    pub fn context_stress_tokens(&self) -> u64 {
        self.int("stress.context_tokens").unwrap_or(150_000)
    }

    /// Evolution applies its plans itself, or proposes them.
    #[must_use]
    pub fn evolution_proposes(&self) -> bool {
        self.get("evolution.mode").and_then(|l| l.value.as_str()) == Some("propose")
    }

    /// The model the human chose for a body, if they chose one: the creature's own, else its
    /// race's for this office (named as `mneme`, `melete`, `aoede` or as `read`, `judge`,
    /// `write`). `None` when the body would ride its race's tier — the only choice a router may
    /// make.
    #[must_use]
    pub fn model_pinned(&self, creature: &str, race: Race, office: Option<&str>) -> Option<String> {
        let by = |key: String| {
            self.get(&key)
                .and_then(|l| l.value.as_str())
                .map(str::to_owned)
        };
        by(format!("models.creature.{creature}")).or_else(|| {
            let (name, function) = office.and_then(office_names)?;
            by(format!("models.office.{}.{name}", race.name()))
                .or_else(|| by(format!("models.office.{}.{function}", race.name())))
        })
    }

    /// The model configured for a tier.
    #[must_use]
    pub fn tier_model(&self, tier: ModelTier) -> String {
        self.get(&format!("models.tier.{}", tier_name(tier)))
            .and_then(|l| l.value.as_str())
            .map(str::to_owned)
            .unwrap_or_default()
    }

    /// The model for a body: the one the human chose ([`Self::model_pinned`]), else its race's
    /// default tier.
    #[must_use]
    pub fn model_for(&self, creature: &str, race: Race, office: Option<&str>) -> String {
        self.model_pinned(creature, race, office)
            .unwrap_or_else(|| self.tier_model(race.default_tier()))
    }

    /// The models to try in order, starting with `model`: each one's configured next.
    ///
    /// # Errors
    ///
    /// A cycle in the chain.
    pub fn fallback_chain(&self, model: &str) -> Result<Vec<String>, String> {
        let mut chain = vec![model.to_owned()];
        let mut at = model.to_owned();
        while let Some(next) = self
            .get(&format!("models.fallback.{at}"))
            .and_then(|l| l.value.as_str())
        {
            if chain.iter().any(|m| m == next) {
                let origin = &self
                    .get(&format!("models.fallback.{at}"))
                    .map(|l| l.origin.clone());
                return Err(format!(
                    "{}: models.fallback loops: {} → {next}",
                    origin
                        .as_ref()
                        .map_or_else(String::new, ToString::to_string),
                    chain.join(" → ")
                ));
            }
            chain.push(next.to_owned());
            next.clone_into(&mut at);
        }
        Ok(chain)
    }

    /// How decisions are made. `decide.llm` defaults to the fast tier's model.
    ///
    /// # Errors
    ///
    /// `decide.via` names no provider, or one that is not `kind = "systemone"`, or comes without
    /// `decide.model`; a cut-off is not between 0 and 1. Each error names the file the value came
    /// from.
    pub fn decide(&self) -> Result<Decide, String> {
        let text = |key: &str| {
            self.get(key)
                .and_then(|l| l.value.as_str())
                .map(str::to_owned)
        };
        let number = |key: &str| {
            self.get(key).map(|l| {
                l.value
                    .as_float()
                    .or_else(|| {
                        l.value
                            .as_integer()
                            .and_then(|i| i32::try_from(i).ok())
                            .map(f64::from)
                    })
                    .unwrap_or(f64::NAN)
            })
        };
        let origin = |key: &str| {
            self.get(key)
                .map_or_else(String::new, |l| l.origin.to_string())
        };
        let via = text("decide.via");
        if let Some(name) = &via {
            match self.providers().get(name) {
                None => {
                    return Err(format!(
                        "{}: decide.via names `{name}`, and no provider has that name",
                        origin("decide.via")
                    ));
                }
                Some(p) if p.kind != "systemone" => {
                    return Err(format!(
                        "{}: decide.via names `{name}`, a provider of kind `{}`; decisions need `kind = \"systemone\"`",
                        origin("decide.via"),
                        p.kind
                    ));
                }
                Some(_) if text("decide.model").is_none() => {
                    return Err(format!(
                        "{}: decide.via names `{name}` without decide.model: name the model to ask (`jev-latest` at TypeSafe, `jeff-latest` at a jeff server)",
                        origin("decide.via")
                    ));
                }
                Some(_) => {}
            }
        }
        let cutoff = |key: &str| -> Result<f64, String> {
            let v = number(key).unwrap_or_default();
            if !(0.0..=1.0).contains(&v) {
                return Err(format!(
                    "{}: {key} is {v}; a cut-off is a confidence, from 0 to 1",
                    origin(key)
                ));
            }
            Ok(v)
        };
        let bench = self
            .get("decide.bench")
            .and_then(|l| l.value.as_bool())
            .unwrap_or(via.is_some());
        let route = self
            .get("decide.route")
            .and_then(|l| l.value.as_bool())
            .unwrap_or(false);
        Ok(Decide {
            route,
            via,
            model: text("decide.model").unwrap_or_default(),
            llm: text("decide.llm")
                .or_else(|| text("models.tier.fast"))
                .unwrap_or_default(),
            bench,
            cutoff_measured: cutoff("decide.cutoff.measured")?,
            cutoff_stated: cutoff("decide.cutoff.stated")?,
        })
    }

    /// The configured providers, by name.
    #[must_use]
    pub fn providers(&self) -> BTreeMap<String, Provider> {
        let mut out = BTreeMap::new();
        for key in self
            .leaves
            .keys()
            .filter_map(|k| k.strip_prefix("providers."))
        {
            let Some((name, _)) = key.split_once('.') else {
                continue;
            };
            if out.contains_key(name) {
                continue;
            }
            let field = |f: &str| {
                self.get(&format!("providers.{name}.{f}"))
                    .and_then(|l| l.value.as_str())
                    .map(str::to_owned)
            };
            out.insert(
                name.to_owned(),
                Provider {
                    kind: field("kind").unwrap_or_default(),
                    base_url: field("base_url").unwrap_or_default(),
                    key_env: field("key_env"),
                },
            );
        }
        out
    }

    /// The Oni envelopes the human signed (local or global file only), revoked and expired ones
    /// included: [`Envelope::admits`] says which are in force.
    #[must_use]
    pub fn envelopes(&self) -> Vec<Envelope> {
        self.get("oni.envelopes")
            .and_then(|l| envelopes(&l.value, &l.origin, self.envelope_days()).ok())
            .unwrap_or_default()
    }

    /// Revoke the envelope `name` on `day`, in the human's file that holds it: the envelope stays
    /// written, with `revoked = <day>`, and covers nothing from then on. Returns that file.
    ///
    /// # Errors
    ///
    /// No envelope is signed, none is called `name`, it is already revoked, or the file cannot be
    /// written — each named.
    pub fn revoke_envelope(&self, name: &str, day: Day) -> Result<PathBuf, String> {
        let envelope = self
            .envelopes()
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| format!("no envelope `{name}` is signed"))?;
        if let Some(on) = envelope.revoked {
            return Err(format!("envelope `{name}` was already revoked on {on}"));
        }
        let file = self
            .get("oni.envelopes")
            .and_then(|l| l.origin.file.clone())
            .ok_or_else(|| format!("envelope `{name}` lives in no file"))?;
        revoke(&file, name, day)?;
        Ok(file)
    }

    /// The longest an Oni envelope may last, in days, and how long one lasts that names no
    /// `expires`: `oni.envelope_days` from the human's own file, else [`ENVELOPE_DAYS`].
    #[must_use]
    pub fn envelope_days(&self) -> i64 {
        self.get("oni.envelope_days")
            .and_then(|l| l.value.as_integer())
            .filter(|d| (1..=ENVELOPE_DAYS_CEILING).contains(d))
            .unwrap_or(ENVELOPE_DAYS)
    }

    fn int(&self, key: &str) -> Option<u64> {
        self.get(key)
            .and_then(|l| l.value.as_integer())
            .and_then(|i| u64::try_from(i).ok())
    }
}

/// An office as configuration may name it, `(name, function)`: its Muse or what it does.
fn office_names(word: &str) -> Option<(&'static str, &'static str)> {
    [("mneme", "read"), ("melete", "judge"), ("aoede", "write")]
        .into_iter()
        .find(|(name, function)| word == *name || word == *function)
}

fn tier_name(tier: ModelTier) -> &'static str {
    match tier {
        ModelTier::Frontier => "frontier",
        ModelTier::Strong => "strong",
        ModelTier::Fast => "fast",
    }
}

/// Flatten tables into dotted keys; arrays and scalars are leaves.
fn flatten(table: &Table, prefix: &str, out: &mut Vec<(String, Value)>) {
    for (k, v) in table {
        let key = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            Value::Table(t) => flatten(t, &key, out),
            other => out.push((key, other.clone())),
        }
    }
}

fn kind_of(key: &str) -> Option<Kind> {
    let parts: Vec<&str> = key.split('.').collect();
    SCHEMA.iter().find_map(|(pattern, kind)| {
        let pat: Vec<&str> = pattern.split('.').collect();
        (pat.len() == parts.len() && pat.iter().zip(&parts).all(|(p, k)| *p == "*" || p == k))
            .then_some(*kind)
    })
}

fn check_kind(key: &str, value: &Value, kind: Kind, origin: &Origin) -> Result<(), String> {
    let ok = match kind {
        Kind::Bool => value.is_bool(),
        Kind::Int => value.as_integer().is_some_and(|i| i >= 0),
        Kind::Number => value.is_float() || value.as_integer().is_some_and(|i| i >= 0),
        Kind::Str => value.is_str(),
        Kind::StrList => value
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_str)),
        Kind::Envelopes => value
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_table)),
    };
    if ok {
        Ok(())
    } else {
        let want = match kind {
            Kind::Bool => "true or false",
            Kind::Int => "a whole number of zero or more",
            Kind::Number => "a number of zero or more",
            Kind::Str => "a string",
            Kind::StrList => "a list of strings",
            Kind::Envelopes => "a list of tables (`[[oni.envelopes]]`)",
        };
        Err(format!(
            "{origin}: {key} is {}, it must be {want}",
            value.type_str()
        ))
    }
}

fn strings(value: &Value) -> impl Iterator<Item = &str> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

/// Read and vet the envelopes: a name no other envelope has, a list of patterns, none bare and
/// none destructive; the day it was signed, an expiry after it and at most `days` on, a use cap
/// of at least one.
fn envelopes(value: &Value, at: &Origin, days: i64) -> Result<Vec<Envelope>, String> {
    let place = Place::new("/world", "/home");
    let mut out = Vec::new();
    for (i, t) in value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_table)
        .enumerate()
    {
        let name = t
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{at}: oni.envelopes[{i}] needs a `name`"))?;
        let allow: Vec<String> = t
            .get("allow")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if allow.is_empty() {
            return Err(format!(
                "{at}: envelope `{name}` allows nothing; give it `allow = [\"…\"]`"
            ));
        }
        for pattern in &allow {
            vet(name, pattern, &place, at)?;
        }
        let signed = date(t, "signed", name, at)?.ok_or_else(|| {
            format!(
                "{at}: envelope `{name}` needs `signed = YYYY-MM-DD`, the day you signed it; it \
                 lasts {days} days unless `expires` says sooner"
            )
        })?;
        let longest = signed.plus(days);
        let expires = date(t, "expires", name, at)?.unwrap_or(longest);
        if expires <= signed {
            return Err(format!(
                "{at}: envelope `{name}` expires on {expires}, not after it was signed on {signed}"
            ));
        }
        if expires > longest {
            return Err(format!(
                "{at}: envelope `{name}` expires on {expires}, more than {days} days after it was \
                 signed on {signed}; sign it again when it ends, or raise `oni.envelope_days` in \
                 your own file"
            ));
        }
        let uses = match t.get("uses") {
            None => None,
            Some(v) => Some(
                v.as_integer()
                    .and_then(|n| u64::try_from(n).ok())
                    .filter(|&n| n >= 1)
                    .ok_or_else(|| {
                        format!("{at}: envelope `{name}`: `uses` is a count of at least 1")
                    })?,
            ),
        };
        // A revocation may come before the signing day: an envelope signed ahead can be withdrawn
        // before it ever takes force, and the record says so.
        let revoked = date(t, "revoked", name, at)?;
        if out.iter().any(|e: &Envelope| e.name == name) {
            return Err(format!(
                "{at}: two envelopes are called `{name}`; a name is how one is revoked, so each is \
                 its own"
            ));
        }
        out.push(Envelope {
            name: name.to_owned(),
            allow,
            signed,
            expires,
            uses,
            revoked,
        });
    }
    Ok(out)
}

/// An envelope's date: a bare TOML date (`2026-10-24`), never a time or a string.
fn date(t: &Table, key: &str, name: &str, at: &Origin) -> Result<Option<Day>, String> {
    let Some(value) = t.get(key) else {
        return Ok(None);
    };
    let day = match value {
        Value::Datetime(dt) if dt.time.is_none() && dt.offset.is_none() => dt
            .date
            .and_then(|d| Day::new(i64::from(d.year), d.month, d.day)),
        _ => None,
    };
    day.map(Some).ok_or_else(|| {
        format!("{at}: envelope `{name}`: `{key}` is a date, written bare: `{key} = 2026-10-24`")
    })
}

/// What a standalone `*` in an envelope may stand for, at its worst: the arguments that make a
/// command destructive.
const WORST: &[&str] = &[
    "--force",
    "-f",
    "--force-with-lease",
    "--delete",
    "-d",
    "--mirror",
    "--tags",
    "+main",
    ":main",
    "-X DELETE",
    "--method=delete",
    "delete",
    "release delete",
    "release create",
    "secret set",
    "repo delete",
    "publish",
    "push",
    "--hard",
    "rm -rf ~",
    "-i",
];

/// An envelope pattern covers what its `*` admits at worst: a word that is a bare `*` is tried
/// as each destructive argument; a `*` inside a word (`feat/*`) cannot become one.
fn vet(name: &str, pattern: &str, place: &Place, at: &Origin) -> Result<(), String> {
    if pattern.split_whitespace().all(|w| w == "*") {
        return Err(format!(
            "{at}: envelope `{name}`: a bare `*` covers everything; name the command"
        ));
    }
    let words: Vec<&str> = pattern.split_whitespace().collect();
    let literal = |fill: &str| -> String {
        words
            .iter()
            .map(|w| if *w == "*" { fill } else { w })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut tries = vec![literal("x")];
    if words.contains(&"*") {
        tries.extend(WORST.iter().map(|w| literal(w)));
    }
    for command in tries {
        match classify(&command, place) {
            Verdict::Run {
                class: mnemorph_races::ToolClass::Destructive,
                ..
            } => {
                return Err(format!(
                    "{at}: envelope `{name}`: `{pattern}` admits `{command}`, which is destructive, and destructive acts always ask"
                ));
            }
            Verdict::Refuse { reason, .. } => {
                return Err(format!(
                    "{at}: envelope `{name}`: `{pattern}` admits `{command}`, which the guard refuses: {reason}"
                ));
            }
            Verdict::Run { .. } => {}
        }
    }
    Ok(())
}
