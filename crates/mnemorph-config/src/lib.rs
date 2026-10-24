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
//! and not a body, which cannot write `.mnemorph/` at all.
//!
//! Unknown keys are reported as holes and the session goes on; a file that does not parse, a
//! value of the wrong type, a floor rule asked to ask, a destructive envelope or a fallback cycle
//! is an error that names its file.

mod write;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_races::{ModelTier, Race};
use mnemorph_tools::{Place, Rule, Verdict, classify};
use toml::{Table, Value};

pub use write::set;

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
"#;

/// Keys only the local or global file may set: each one loosens the human gate.
const LOOSENING: &[&str] = &[
    "gate.enabled",
    "gate.ask_rules",
    "gate.approve",
    "oni.envelopes",
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
    ("budget.month_usd", Kind::Number),
    ("budget.day_usd", Kind::Number),
    ("budget.session_usd", Kind::Number),
    ("stress.escalate_at", Kind::Int),
    ("stress.relieve_at", Kind::Int),
    ("stress.max_consults", Kind::Int),
    ("evolution.mode", Kind::Str),
    ("models.tier.*", Kind::Str),
    ("models.office.*.*", Kind::Str),
    ("models.creature.*", Kind::Str),
    ("models.fallback.*", Kind::Str),
    ("providers.*.kind", Kind::Str),
    ("providers.*.base_url", Kind::Str),
    ("providers.*.key_env", Kind::Str),
    ("oni.envelopes", Kind::Envelopes),
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
    /// `anthropic` or `openai` (any OpenAI-compatible server).
    pub kind: String,
    /// The server's base URL.
    pub base_url: String,
    /// The environment variable holding its key (never the key itself).
    pub key_env: Option<String>,
}

/// Standing permissions the human signed for Oni: outward acts that run without a prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Its name, for the ledger and `status`.
    pub name: String,
    /// The command patterns it covers; `*` at the end of a word matches the rest.
    pub allow: Vec<String>,
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
                    envelopes(&leaf.value, at)?;
                }
                _ => {}
            }
            if let Some(rest) = key.strip_prefix("models.office.") {
                let race = rest.split('.').next().unwrap_or("");
                if Race::from_name(race).is_none() {
                    return Err(format!("{at}: models.office.{race}: no race `{race}`"));
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

    /// Evolution applies its plans itself, or proposes them.
    #[must_use]
    pub fn evolution_proposes(&self) -> bool {
        self.get("evolution.mode").and_then(|l| l.value.as_str()) == Some("propose")
    }

    /// The model for a body: the creature's own, else its race's for this office, else its
    /// race's default tier.
    #[must_use]
    pub fn model_for(&self, creature: &str, race: Race, office: Option<&str>) -> String {
        let by = |key: String| {
            self.get(&key)
                .and_then(|l| l.value.as_str())
                .map(str::to_owned)
        };
        by(format!("models.creature.{creature}"))
            .or_else(|| office.and_then(|o| by(format!("models.office.{}.{o}", race.name()))))
            .or_else(|| by(format!("models.tier.{}", tier_name(race.default_tier()))))
            .unwrap_or_default()
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

    /// The Oni envelopes the human signed (local or global file only).
    #[must_use]
    pub fn envelopes(&self) -> Vec<Envelope> {
        self.get("oni.envelopes")
            .and_then(|l| envelopes(&l.value, &l.origin).ok())
            .unwrap_or_default()
    }

    fn int(&self, key: &str) -> Option<u64> {
        self.get(key)
            .and_then(|l| l.value.as_integer())
            .and_then(|i| u64::try_from(i).ok())
    }
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

/// Read and vet the envelopes: a name, a list of patterns, none bare and none destructive.
fn envelopes(value: &Value, at: &Origin) -> Result<Vec<Envelope>, String> {
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
        out.push(Envelope {
            name: name.to_owned(),
            allow,
        });
    }
    Ok(out)
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
