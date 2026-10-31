//! Configuration: TOML layers whose every value remembers where it came from.
//!
//! Four layers, lowest first: the compiled **defaults**, the human's **global** file, the
//! project's **tracked** file (`.mnemorph/config.toml`, shared with everyone who clones it) and its
//! **local** file (`.mnemorph/config.local.toml`, the human's own). A higher layer wins a scalar
//! or a list; tables merge key by key, each leaf keeping its own origin; an empty list or table
//! never overrides a lower layer.
//!
//! The tracked file may only tighten the human gate. What loosens it — guard rules that ask
//! instead of refusing, a gate turned off, standing approvals, Oni envelopes, running unguarded —
//! and what reaches further — an MCP server, a provider, a spending cap — is honoured only from
//! the global file: the local file lives inside the world, where a body's command or a
//! repository's script may write it, so there those keys are holes too. Nothing else feeds them:
//! not the environment, not a flag. An envelope is bounded besides: it is
//! signed on a day, expires at most `oni.envelope_days` later (30 unless the human's own file
//! says, never more than a year), may cap its uses, and stays in the file once revoked, so the
//! record of what was allowed is never lost.
//!
//! The Kijin's plan step lives beside the envelopes it is read against: an outward act is typed,
//! and only a kind whose undo the harness derives can be covered; every other outward act is
//! destructive, which no envelope covers ([`plan`], [`Envelope::covers`]). Its receipt follows the
//! act: a typed act is `observed` only when a read-only probe sees its effect ([`PostJournal`]).
//! The act runner ([`Runner`], the Kijin's `outward` tool) is the one path a typed act takes:
//! post, plan, pin, gate, recheck, intent, act, result, probe, receipt.
//! An act it undoes ([`Runner::undo`]) is an [`Incident`] until four receipts close it — undone,
//! journal corrected, downstream told, a lesson — and while it is open the envelopes that read the
//! act are [`suspended`].
//!
//! Unknown keys are reported as holes and the session goes on; a file that does not parse, a
//! value of the wrong type, a floor rule asked to ask, a destructive envelope or a fallback cycle
//! is an error that names its file.

mod closure;
mod forge;
mod oni;
mod outward;
mod receipt;
mod runner;
mod write;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_ledger::Day;
use mnemorph_races::{ModelTier, Race};
use mnemorph_tools::{Place, Rule, Verdict, classify};
use toml::{Table, Value};

pub use closure::{Cause, Incident, Lesson, RECEIPTS, incidents, suspended};
pub use oni::{Admitted, EnvelopeGate, Revocation, excluded, guarded, secret_in, uses_path};
pub use outward::{
    Act, BODY_BYTES, COVERED_TEXT, Done, Malformed, NO_UNDO, NoUndo, Planned, TITLE_CHARS,
    Uncovered, plan,
};
pub use receipt::{
    Observer, PROBE_TIMEOUT, Pin, PostJournal, Probe, Receipt, Row, SEEN_BYTES, Seen, State, Tally,
    excerpt, post_journal, reconciling, redact,
};
pub use runner::{
    Hands, OUTWARD, OutwardTool, PAIR_ASKS, Paired, Refusal, Report, Request, Runner, Sandboxed,
    UNDO_ASKS,
};
pub use write::{revoke, set};

/// The compiled defaults: the lowest layer, and the reference `config explain` shows.
const DEFAULTS: &str = r#"
[gate]
enabled = true
timeout_secs = 300
verify_secs = 120

[tools.read]
range_over = 400

[stress]
escalate_at = 5
relieve_at = 7
max_consults = 3

[evolution]
mode = "auto"
min_net_wins = 2

[models]
provider = "anthropic"

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

/// How long each of the end gate's verify commands may run, in seconds, unless the configuration
/// says.
pub const VERIFY_SECS: u64 = 120;

/// The bounds of `gate.verify_secs`: at least long enough to start a test runner, and at most what
/// the `bash` tool grants any command.
pub const VERIFY_SECS_RANGE: std::ops::RangeInclusive<u64> =
    10..=mnemorph_tools::shell::MAX_TIMEOUT.as_secs();

/// How many Dark Elf reviewers a landing's challenge commissions at once, unless the configuration
/// says.
pub const CHALLENGE_REVIEWERS: usize = 3;
/// The most `gate.challenge_reviewers` may be: every speciality and the generalist.
const CHALLENGE_REVIEWERS_CEILING: i64 = 7;

/// `gate.challenge_reviewers` holds a count of reviewers from 1 to every speciality.
/// `evolution.min_net_wins`: a count of paired wins, at least 1.
fn net_wins(value: &Value, at: &Origin) -> Result<(), String> {
    if value
        .as_integer()
        .is_some_and(|n| (1..=i64::from(u32::MAX)).contains(&n))
    {
        Ok(())
    } else {
        Err(format!(
            "{at}: evolution.min_net_wins is a count of paired wins, at least 1"
        ))
    }
}

/// `tools.read.range_over`: a count of lines, 0 to read every file whole.
fn range_over(value: &Value, at: &Origin) -> Result<(), String> {
    if value
        .as_integer()
        .is_some_and(|n| (0..=RANGE_OVER_CEILING).contains(&n))
    {
        Ok(())
    } else {
        Err(format!(
            "{at}: tools.read.range_over is a count of lines from 0 (every file read whole) to {RANGE_OVER_CEILING}"
        ))
    }
}

fn reviewers(value: &Value, at: &Origin) -> Result<(), String> {
    if value
        .as_integer()
        .is_some_and(|n| (1..=CHALLENGE_REVIEWERS_CEILING).contains(&n))
    {
        Ok(())
    } else {
        Err(format!(
            "{at}: gate.challenge_reviewers is a count of reviewers from 1 to {CHALLENGE_REVIEWERS_CEILING}"
        ))
    }
}

/// How many more paired tasks an evolution must win than lose to be kept, unless the human sets
/// `evolution.min_net_wins`.
pub const MIN_NET_WINS: u32 = 2;

/// The most `tools.read.range_over` may be: past it, the rule never asks.
const RANGE_OVER_CEILING: i64 = 1_000_000;
/// The most restarts `mcp.restarts.max` may allow a server in one session.
const MCP_RESTARTS_CEILING: u64 = 10;
/// The fewest asks `mcp.prefix.asks` may choose a stable prefix over: fewer is a guess.
const MCP_PREFIX_ASKS_FLOOR: u64 = 10;

/// Whether only the human's global file may set `key`: everything that loosens the gate, an MCP
/// server, a provider (where a key is sent), every spending cap, every decision setting (who
/// decides, how sure it must be) and what the board reads. The
/// local file lives inside the world, where whatever runs there may write it.
#[must_use]
pub fn global_only(key: &str) -> bool {
    LOOSENING.contains(&key)
        || ["mcp.", "providers.", "budget.", "board."]
            .iter()
            .any(|p| key.starts_with(p))
}

/// Keys only the global file may set (a tracked one is a hole, and so is a local one): each one
/// loosens the human gate.
const LOOSENING: &[&str] = &[
    "gate.enabled",
    "gate.challenge_reviewers",
    "gate.ask_rules",
    "gate.approve",
    "oni.envelopes",
    "oni.envelope_days",
    "evolution.min_net_wins",
    "sandbox.allow_unguarded",
    "models.provider",
    "budget.session",
    "budget.day",
    "budget.month",
    "decide.enabled",
    "decide.via",
    "decide.model",
    "decide.llm",
    "decide.bench",
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
    ("gate.verify_secs", Kind::Int),
    ("gate.challenge_reviewers", Kind::Int),
    ("gate.ask_rules", Kind::StrList),
    ("gate.approve", Kind::StrList),
    ("sandbox.allow_unguarded", Kind::Bool),
    ("attic.keep_days", Kind::Int),
    ("attic.max_mb", Kind::Int),
    ("budget.session", Kind::Int),
    ("budget.day", Kind::Int),
    ("budget.month", Kind::Int),
    ("budget.month_usd", Kind::Number),
    ("budget.day_usd", Kind::Number),
    ("budget.session_usd", Kind::Number),
    ("stress.escalate_at", Kind::Int),
    ("stress.relieve_at", Kind::Int),
    ("stress.max_consults", Kind::Int),
    ("stress.context_tokens", Kind::Int),
    ("tools.read.range_over", Kind::Int),
    ("steps.solo", Kind::Int),
    ("steps.rimuru", Kind::Int),
    ("steps.elf", Kind::Int),
    ("steps.orc", Kind::Int),
    ("steps.slime", Kind::Int),
    ("steps.kijin", Kind::Int),
    ("steps.dark-elf", Kind::Int),
    ("steps.high-elf", Kind::Int),
    ("steps.high-orc", Kind::Int),
    ("steps.named-slime", Kind::Int),
    ("steps.oni", Kind::Int),
    ("steps.arch-dark-elf", Kind::Int),
    ("recall.meaning", Kind::Number),
    ("recall.relation", Kind::Number),
    ("recall.salience", Kind::Number),
    ("recall.confidence", Kind::Number),
    ("recall.freshness", Kind::Number),
    ("recall.dissent", Kind::Number),
    ("recall.strength", Kind::Number),
    ("recall.half_life_days", Kind::Number),
    ("recall.floor", Kind::Number),
    ("recall.saturation_short", Kind::Number),
    ("recall.saturation_mid", Kind::Number),
    ("recall.saturation_long", Kind::Number),
    ("evolution.mode", Kind::Str),
    ("evolution.min_net_wins", Kind::Int),
    ("models.provider", Kind::Str),
    ("models.default", Kind::Str),
    ("models.race.*", Kind::Str),
    ("models.tier.*", Kind::Str),
    ("models.office.*.*", Kind::Str),
    ("models.creature.*", Kind::Str),
    ("models.fallback.*", Kind::Str),
    ("providers.*.kind", Kind::Str),
    ("providers.*.base_url", Kind::Str),
    ("providers.*.key_env", Kind::Str),
    ("decide.enabled", Kind::Bool),
    ("decide.via", Kind::Str),
    ("decide.model", Kind::Str),
    ("decide.llm", Kind::Str),
    ("decide.bench", Kind::Bool),
    ("decide.route", Kind::Bool),
    ("decide.cutoff.measured", Kind::Number),
    ("decide.cutoff.stated", Kind::Number),
    ("oni.envelopes", Kind::Envelopes),
    ("oni.envelope_days", Kind::Int),
    ("mcp.top_k", Kind::Int),
    ("mcp.rerank.enabled", Kind::Bool),
    ("mcp.rerank.above", Kind::Int),
    ("mcp.rerank.margin_pct", Kind::Int),
    ("mcp.timeouts.start_secs", Kind::Int),
    ("mcp.timeouts.list_secs", Kind::Int),
    ("mcp.timeouts.call_secs", Kind::Int),
    ("mcp.restarts.max", Kind::Int),
    ("mcp.restarts.backoff_secs", Kind::Int),
    ("mcp.prefix.asks", Kind::Int),
    ("mcp.prefix.share_pct", Kind::Int),
    ("mcp.prefix.tools", Kind::Int),
    ("mcp.servers.*.command", Kind::Str),
    ("mcp.servers.*.args", Kind::StrList),
    ("mcp.servers.*.env", Kind::StrList),
    ("mcp.servers.*.races", Kind::StrList),
    ("mcp.servers.*.read_only", Kind::Bool),
    ("mcp.servers.*.class", Kind::Str),
    ("mcp.servers.*.purpose", Kind::Str),
    ("mcp.servers.*.tools.*.class", Kind::Str),
    ("mcp.servers.*.network", Kind::Bool),
    ("mcp.servers.*.timeout_secs", Kind::Int),
    ("board.bench_results", Kind::Str),
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

/// The session body's steps working solo, by default: an Elf's, an Orc's and a Slime's.
pub const SOLO_STEPS: u32 = 100;

/// The merged configuration.
#[derive(Debug, Clone, Default)]
pub struct Config {
    leaves: BTreeMap<String, Leaf>,
    holes: Vec<Hole>,
    /// The providers the human's own files define, known before any layer is merged, so a
    /// tracked model that names one as its backend is caught whichever file defines it.
    human_providers: std::collections::BTreeSet<String>,
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

/// Where one model's requests go: the provider, and the id it knows the model by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backend {
    /// The provider's name in the configuration.
    pub provider_name: String,
    /// The provider: its kind, URL and key variable.
    pub provider: Provider,
    /// The model's id at that provider.
    pub model: String,
}

/// How decisions are made, once the human switched them on: by the System One provider `via`
/// first (Jev, or a jeff server), and by the LLM `llm` when none is named, its key is not set, or
/// it fails.
#[derive(Debug, Clone, PartialEq)]
pub struct Decide {
    /// Whether decisions are on: the human turned them on (`decide.enabled`), off by default.
    pub enabled: bool,
    /// The System One provider tried first (`kind = "systemone"`), when one is configured.
    pub via: Option<String>,
    /// The model to ask at `via` (`jev-latest` at TypeSafe, `jeff-latest` at a jeff server);
    /// empty when there is no `via`.
    pub model: String,
    /// The LLM that answers when no System One is there: none named, its key not set, or it
    /// failed. Its answers are stated, held to `cutoff_stated`.
    pub llm: String,
    /// Whether Metis routes a commission to a tier by how demanding it is (`decide.route`): off
    /// unless the human turns it on, since it changes which model does the work.
    pub route: bool,
    /// Whether a piped test run's output is read by a decision instead of counted muted: on
    /// with decisions unless `decide.bench = false`.
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
    /// The world it is signed for, an absolute path: it admits acts only from there.
    pub world: Option<PathBuf>,
    /// The forge repository it is signed for, `owner/name`: it admits acts only on it.
    pub repo: Option<String>,
    /// The post it is signed for (`kijin-forge`, …): when set, only that post's acts.
    pub post: Option<String>,
    /// The body it is signed for, by name: when set, only that body's acts.
    pub body: Option<String>,
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
        Self::load_layers(paths, &[Layer::Global, Layer::Tracked, Layer::Local])
    }

    /// Read the defaults and the human's global file alone: where the gate reads Oni envelopes,
    /// since a file inside the world may be written by whatever runs there.
    ///
    /// # Errors
    ///
    /// As [`Self::load`], for the global file.
    pub fn load_global(paths: &Paths) -> Result<Self, String> {
        Self::load_layers(paths, &[Layer::Global])
    }

    fn load_layers(paths: &Paths, layers: &[Layer]) -> Result<Self, String> {
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
        let mut files = Vec::new();
        for &layer in layers {
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
            files.push((layer, path, table));
        }
        config.human_providers = std::iter::once(&defaults)
            .chain(
                files
                    .iter()
                    .filter(|(l, ..)| *l == Layer::Global)
                    .map(|(.., t)| t),
            )
            .filter_map(|t| t.get("providers").and_then(toml::Value::as_table))
            .flat_map(|t| t.keys().cloned())
            .collect();
        for (layer, path, table) in files {
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
            if origin.layer == Layer::Local && global_only(&key) {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "the local file lives inside the world, where a body or a repository's script can write it; this key widens the sandbox, the gate, a spending cap or where a key is sent, so it is honoured only from your global file".into(),
                });
                continue;
            }
            if origin.layer == Layer::Tracked && key.starts_with("mcp.") {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "an MCP server is a program started with its own reach, so it is honoured only from your global file".into(),
                });
                continue;
            }
            if origin.layer == Layer::Tracked && key.starts_with("board.") {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "the board reads the files this names, so it is honoured only from your global file".into(),
                });
                continue;
            }
            if origin.layer == Layer::Tracked && key.starts_with("providers.") {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "a provider decides where a key is sent, so it is honoured only from your global file".into(),
                });
                continue;
            }
            if origin.layer == Layer::Tracked
                && key.starts_with("models.")
                && value
                    .as_str()
                    .and_then(|v| v.split_once(':'))
                    .is_some_and(|(p, _)| self.human_providers.contains(p))
            {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "names a backend: where a model's requests and key go is honoured only from the local or global file".into(),
                });
                continue;
            }
            if origin.layer == Layer::Tracked
                && key.starts_with("budget.")
                && !key.ends_with("_usd")
            {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "a spending cap is the human's: a cloned repository neither raises nor sets one, so it is honoured only from your global file".into(),
                });
                continue;
            }
            if origin.layer == Layer::Tracked && LOOSENING.contains(&key.as_str()) {
                self.holes.push(Hole {
                    key,
                    origin: origin.clone(),
                    why: "loosens the human gate, so it is honoured only from your global file"
                        .into(),
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

    /// The budget caps, the race models and the default backend, checked.
    fn check_caps_and_models(&self, key: &str, leaf: &Leaf) -> Result<(), String> {
        let at = &leaf.origin;
        if matches!(key, "budget.session" | "budget.day" | "budget.month")
            && leaf.value.as_integer().is_some_and(|n| n < 1)
        {
            return Err(format!(
                "{at}: {key} is a count of weighted tokens of at least 1; to spend nothing, \
                 do not start a session"
            ));
        }
        if let Some(race) = key.strip_prefix("models.race.")
            && Race::from_name(race).is_none()
        {
            return Err(format!("{at}: models.race.{race}: no race `{race}`"));
        }
        if key == "models.provider" {
            let name = leaf.value.as_str().unwrap_or_default();
            match self.providers().get(name) {
                None => {
                    return Err(format!(
                        "{at}: models.provider names `{name}`, and no provider has that name"
                    ));
                }
                Some(p) if p.kind == "systemone" => {
                    return Err(format!(
                        "{at}: models.provider names `{name}`, a classifier; models need a provider of kind \"anthropic\" or \"openai\""
                    ));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        for (key, leaf) in &self.leaves {
            let at = &leaf.origin;
            match key.as_str() {
                k if k.starts_with("recall.") => recall_bound(k, &leaf.value, at)?,
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
                "gate.verify_secs"
                    if !leaf
                        .value
                        .as_integer()
                        .and_then(|s| u64::try_from(s).ok())
                        .is_some_and(|s| VERIFY_SECS_RANGE.contains(&s)) =>
                {
                    return Err(format!(
                        "{at}: gate.verify_secs is a count of seconds from {} to {}",
                        VERIFY_SECS_RANGE.start(),
                        VERIFY_SECS_RANGE.end()
                    ));
                }
                "gate.challenge_reviewers" => reviewers(&leaf.value, at)?,
                "tools.read.range_over" => range_over(&leaf.value, at)?,
                "evolution.min_net_wins" => net_wins(&leaf.value, at)?,
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
            self.check_caps_and_models(key, leaf)?;
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

    /// The human gate is on (only the global file can turn it off).
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

    /// How long each of the end gate's verify commands may run, in seconds. A tracked file may
    /// set it: a longer limit runs the same classified, sandboxed command, only for longer, and a
    /// command killed at the limit grounds nothing either way.
    #[must_use]
    pub fn verify_secs(&self) -> u64 {
        self.int("gate.verify_secs").unwrap_or(VERIFY_SECS)
    }

    /// How many Dark Elf reviewers one landing's challenge commissions at once; the rest wait
    /// queued for the next sweep. Local or global file only: fewer reviewers delay a review.
    #[must_use]
    pub fn challenge_reviewers(&self) -> usize {
        self.int("gate.challenge_reviewers")
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(CHALLENGE_REVIEWERS)
    }

    /// Commands may run with no kernel guard in force (global file only).
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

    /// A budget cap in weighted tokens — `session`, `day` or `month` — when the human set one.
    /// Weighted tokens count every call, priced or not, so no model slips under a cap.
    #[must_use]
    pub fn budget_wt(&self, period: &str) -> Option<u64> {
        self.int(&format!("budget.{period}"))
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

    /// The lines over which a read must name its range (a skeleton answers one that does not);
    /// 0 reads every file whole.
    #[must_use]
    pub fn read_range_over(&self) -> usize {
        self.int("tools.read.range_over")
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(mnemorph_tools::files::RANGE_OVER)
    }

    /// The steps of the session's own body, working solo: it does the work of the shortest chain
    /// it stands for — an Elf, an Orc and a Slime, 30 + 30 + 40 — so it is given their steps, not
    /// one body's. Spend is capped only by the `budget.*` caps the human sets; without them, the
    /// steps, the clock, the context line and the stall guards are what stop it.
    #[must_use]
    pub fn solo_steps(&self) -> u32 {
        self.int("steps.solo")
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .unwrap_or(SOLO_STEPS)
    }

    /// The steps a body of `race` may take in one commission: the human's `steps.<race>` (the
    /// race's name, `steps.named-slime` for a Named Slime), else the race law's
    /// ([`Race::steps`]). Passed down whole and never split, like the law's; zero is not a
    /// budget and falls back to the law.
    #[must_use]
    pub fn race_steps(&self, race: Race) -> u32 {
        self.int(&format!("steps.{}", race.name()))
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .unwrap_or_else(|| race.steps())
    }

    /// Evolution applies its plans itself, or proposes them.
    #[must_use]
    pub fn evolution_proposes(&self) -> bool {
        self.get("evolution.mode").and_then(|l| l.value.as_str()) == Some("propose")
    }

    /// How many more paired tasks an evolution must win than lose against doing nothing to be
    /// kept: `evolution.min_net_wins`, at least 1, 2 unless set.
    #[must_use]
    pub fn evolution_min_net_wins(&self) -> u32 {
        self.int("evolution.min_net_wins")
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n >= 1)
            .unwrap_or(MIN_NET_WINS)
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

    /// The model a race rides at a tier: its own model (`models.race.<race>`) at its default tier —
    /// a default, so a router may still move a body off it within the band — else the tier's.
    #[must_use]
    pub fn model_at(&self, race: Race, tier: ModelTier) -> String {
        self.get(&format!("models.race.{}", race.name()))
            .filter(|_| tier == race.default_tier())
            .and_then(|l| l.value.as_str())
            .map_or_else(|| self.tier_model(tier), str::to_owned)
    }

    /// The model configured for a tier: the tier as the human set it, else `models.default` —
    /// one model for everything — else the compiled tier.
    #[must_use]
    pub fn tier_model(&self, tier: ModelTier) -> String {
        let leaf = self.get(&format!("models.tier.{}", tier_name(tier)));
        let text = |l: &Leaf| l.value.as_str().map(str::to_owned);
        let human = |l: &&Leaf| matches!(l.origin.layer, Layer::Global | Layer::Local);
        leaf.filter(human)
            .and_then(text)
            .or_else(|| self.get("models.default").and_then(text))
            .or_else(|| leaf.and_then(text))
            .unwrap_or_default()
    }

    /// Where a model's requests go: `<provider>:<id>` when the part before the first `:` names a
    /// configured provider (`vllm:qwen3-coder`), else the model as written, through
    /// `models.provider` — the backend every model goes to by default. A model id may itself
    /// hold a `:` (`qwen3:32b`); it is split only on a provider's name.
    ///
    /// # Errors
    ///
    /// The provider is not configured, or it does not serve conversations (`kind = "systemone"`).
    pub fn backend(&self, model: &str) -> Result<Backend, String> {
        let providers = self.providers();
        let (name, id) = match model.split_once(':') {
            Some((p, id)) if providers.contains_key(p) => (p.to_owned(), id.to_owned()),
            _ => (
                self.get("models.provider")
                    .and_then(|l| l.value.as_str())
                    .unwrap_or("anthropic")
                    .to_owned(),
                model.to_owned(),
            ),
        };
        let provider = providers.get(&name).cloned().ok_or_else(|| {
            format!("model `{model}` goes to provider `{name}`, and no provider has that name")
        })?;
        if id.trim().is_empty() {
            return Err(format!("model `{model}` names no model"));
        }
        if provider.kind == "systemone" {
            return Err(format!(
                "model `{model}` goes to `{name}`, a classifier (`kind = \"systemone\"`): it makes decisions, not conversations"
            ));
        }
        Ok(Backend {
            provider_name: name,
            provider,
            model: id,
        })
    }

    /// The model for a body: the one the human chose ([`Self::model_pinned`]), else its race's
    /// default tier.
    #[must_use]
    pub fn model_for(&self, creature: &str, race: Race, office: Option<&str>) -> String {
        self.model_pinned(creature, race, office)
            .unwrap_or_else(|| self.model_at(race, race.default_tier()))
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
        let flag = |key: &str| self.get(key).and_then(|l| l.value.as_bool());
        let enabled = flag("decide.enabled").unwrap_or(false);
        let bench = enabled && flag("decide.bench").unwrap_or(true);
        let route = enabled && flag("decide.route").unwrap_or(false);
        Ok(Decide {
            enabled,
            route,
            via,
            model: text("decide.model").unwrap_or_default(),
            llm: text("decide.llm").unwrap_or_else(|| self.tier_model(ModelTier::Fast)),
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

    /// The directory of bench result rows the board shows (`board.bench_results`), from the
    /// human's global file alone; `None` when none is configured.
    #[must_use]
    pub fn bench_results(&self) -> Option<PathBuf> {
        self.get("board.bench_results")
            .filter(|l| l.origin.layer == Layer::Global)
            .and_then(|l| l.value.as_str())
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
    }

    /// The MCP servers the human's own files declare — each tool's class among them — and how many
    /// ranked tools an ask names.
    ///
    /// # Errors
    ///
    /// A server with no command, an unlawful name, an unknown race or class, a class that
    /// disagrees with `read_only`, an unlawful tool name, or a zero timeout.
    pub fn mcp(&self) -> Result<mnemorph_tools::mcp::Settings, String> {
        use mnemorph_tools::mcp::{self, Server, Settings};
        let mut names: Vec<&str> = self
            .leaves
            .keys()
            .filter_map(|k| k.strip_prefix("mcp.servers."))
            .filter_map(|k| k.split_once('.').map(|(n, _)| n))
            .collect();
        names.dedup();
        let list = |key: &str| -> Vec<String> {
            self.get(key)
                .and_then(|l| l.value.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let flag = |key: &str| self.get(key).and_then(|l| l.value.as_bool()) == Some(true);
        let mut servers = Vec::new();
        for name in names {
            let at = |f: &str| format!("mcp.servers.{name}.{f}");
            if !mcp::lawful_server_name(name) {
                return Err(format!(
                    "mcp.servers.{name}: a server's name is lower-case letters, digits and dashes, at most 24"
                ));
            }
            let Some(command) = self
                .get(&at("command"))
                .and_then(|l| l.value.as_str())
                .filter(|c| !c.is_empty())
            else {
                return Err(format!("mcp.servers.{name}: no command"));
            };
            let races = list(&at("races"))
                .iter()
                .map(|r| {
                    Race::from_name(r)
                        .ok_or_else(|| format!("mcp.servers.{name}.races: `{r}` is no race"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (class, tools) = self.mcp_classes(name)?;
            let timeout = match self.int(&at("timeout_secs")) {
                Some(0) => return Err(format!("mcp.servers.{name}.timeout_secs: at least 1")),
                Some(s) => Some(std::time::Duration::from_secs(s)),
                None => None,
            };
            servers.push(Server {
                name: name.to_owned(),
                command: command.to_owned(),
                args: list(&at("args")),
                env: list(&at("env")),
                races,
                class,
                tools,
                purpose: self
                    .get(&at("purpose"))
                    .and_then(|l| l.value.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                network: flag(&at("network")),
                timeout,
            });
        }
        let d = Settings::default();
        Ok(Settings {
            servers,
            top_k: match self.int("mcp.top_k") {
                Some(0) => return Err("mcp.top_k: at least 1".into()),
                n => n.and_then(|n| usize::try_from(n).ok()).unwrap_or(d.top_k),
            },
            rerank: self.mcp_rerank(&d.rerank)?,
            timeouts: mnemorph_tools::mcp::Timeouts {
                start: self.mcp_secs("mcp.timeouts.start_secs", d.timeouts.start)?,
                list: self.mcp_secs("mcp.timeouts.list_secs", d.timeouts.list)?,
                call: self.mcp_secs("mcp.timeouts.call_secs", d.timeouts.call)?,
            },
            restarts: self.mcp_restarts(d.restarts)?,
            prefix: self.mcp_prefix(d.prefix)?,
        })
    }

    /// How a failed MCP server is started again (`mcp.restarts.*`).
    fn mcp_restarts(
        &self,
        d: mnemorph_tools::mcp::Restarts,
    ) -> Result<mnemorph_tools::mcp::Restarts, String> {
        Ok(mnemorph_tools::mcp::Restarts {
            max: match self.int("mcp.restarts.max") {
                Some(n) if n > MCP_RESTARTS_CEILING => {
                    return Err(format!(
                        "mcp.restarts.max: a count of restarts from 0 to {MCP_RESTARTS_CEILING}"
                    ));
                }
                Some(n) => u32::try_from(n).unwrap_or(d.max),
                None => d.max,
            },
            backoff: self.mcp_secs("mcp.restarts.backoff_secs", d.backoff)?,
        })
    }

    /// What joins a creature's stable prefix (`mcp.prefix.*`).
    fn mcp_prefix(
        &self,
        d: mnemorph_tools::mcp::Prefix,
    ) -> Result<mnemorph_tools::mcp::Prefix, String> {
        Ok(mnemorph_tools::mcp::Prefix {
            asks: match self.int("mcp.prefix.asks") {
                Some(n) if n < MCP_PREFIX_ASKS_FLOOR => {
                    return Err(format!(
                        "mcp.prefix.asks: at least {MCP_PREFIX_ASKS_FLOOR} asks"
                    ));
                }
                n => n.and_then(|n| usize::try_from(n).ok()).unwrap_or(d.asks),
            },
            share_pct: match self.int("mcp.prefix.share_pct") {
                Some(n) if !(1..=100).contains(&n) => {
                    return Err("mcp.prefix.share_pct: a percent, from 1 to 100".into());
                }
                n => n.and_then(|n| u32::try_from(n).ok()).unwrap_or(d.share_pct),
            },
            tools: self
                .int("mcp.prefix.tools")
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(d.tools),
        })
    }

    /// A wait of whole seconds under `key`, at least one, else `default`.
    fn mcp_secs(
        &self,
        key: &str,
        default: std::time::Duration,
    ) -> Result<std::time::Duration, String> {
        match self.int(key) {
            Some(0) => Err(format!("{key}: at least 1 second")),
            Some(s) => Ok(std::time::Duration::from_secs(s)),
            None => Ok(default),
        }
    }

    /// When the fast tier reranks an ask's MCP shortlist (`mcp.rerank.*`), and on which model:
    /// the fast tier's, when it rides the session's default provider — the only client a run
    /// holds — else none, and the BM25 order stands.
    fn mcp_rerank(
        &self,
        d: &mnemorph_tools::mcp::Rerank,
    ) -> Result<mnemorph_tools::mcp::Rerank, String> {
        let margin_pct = match self.int("mcp.rerank.margin_pct") {
            Some(m) if m > 100 => {
                return Err("mcp.rerank.margin_pct: a percent, from 0 to 100".into());
            }
            Some(m) => u32::try_from(m).unwrap_or(d.margin_pct),
            None => d.margin_pct,
        };
        let fast = self.tier_model(ModelTier::Fast);
        let default_provider = self
            .get("models.provider")
            .and_then(|l| l.value.as_str())
            .unwrap_or("anthropic")
            .to_owned();
        let model = (!fast.is_empty())
            .then(|| self.backend(&fast).ok())
            .flatten()
            .filter(|b| b.provider_name == default_provider)
            .map(|b| b.model);
        Ok(mnemorph_tools::mcp::Rerank {
            enabled: self
                .get("mcp.rerank.enabled")
                .and_then(|l| l.value.as_bool())
                .unwrap_or(d.enabled),
            above: self
                .int("mcp.rerank.above")
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(d.above),
            margin_pct,
            model,
        })
    }

    /// The classes the global file declares for MCP server `name`: the server's (`read_only =
    /// true` is `read`) and each tool's.
    fn mcp_classes(
        &self,
        name: &str,
    ) -> Result<
        (
            Option<mnemorph_races::ToolClass>,
            std::collections::BTreeMap<String, mnemorph_races::ToolClass>,
        ),
        String,
    > {
        use mnemorph_tools::mcp;
        let at = |f: &str| format!("mcp.servers.{name}.{f}");
        let flag = |key: &str| self.get(key).and_then(|l| l.value.as_bool()) == Some(true);
        let class_at = |key: &str| -> Result<Option<mnemorph_races::ToolClass>, String> {
            match self.get(key).and_then(|l| l.value.as_str()) {
                None => Ok(None),
                Some(c) => mcp::class_named(c).map(Some).ok_or_else(|| {
                    format!(
                        "{key}: `{c}` is no class an MCP tool may hold — read, write, outward or destructive"
                    )
                }),
            }
        };
        let class = match (class_at(&at("class"))?, flag(&at("read_only"))) {
            (Some(c), true) if c != mnemorph_races::ToolClass::Read => {
                return Err(format!(
                    "mcp.servers.{name}: `read_only = true` and `class = \"{c}\"` disagree; keep one"
                ));
            }
            (Some(c), _) => Some(c),
            (None, true) => Some(mnemorph_races::ToolClass::Read),
            (None, false) => None,
        };
        let tools_at = format!("mcp.servers.{name}.tools.");
        let mut tools = std::collections::BTreeMap::new();
        for key in self.leaves.keys() {
            let Some(tool) = key
                .strip_prefix(&tools_at)
                .and_then(|k| k.strip_suffix(".class"))
            else {
                continue;
            };
            if !mcp::lawful_tool_name(tool) {
                return Err(format!(
                    "{key}: `{tool}` is no tool name — letters, digits, `_` and `-`, no `__`"
                ));
            }
            if let Some(c) = class_at(key)? {
                tools.insert(tool.to_owned(), c);
            }
        }
        Ok((class, tools))
    }

    /// The Oni envelopes the human signed (the global file alone), revoked and expired ones
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

    /// A number key's value — `recall.*` and the like — when a layer sets it: the caller's own
    /// default stands otherwise.
    #[must_use]
    pub fn number(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|l| {
            l.value.as_float().or_else(|| {
                l.value
                    .as_integer()
                    .and_then(|i| u32::try_from(i).ok())
                    .map(f64::from)
            })
        })
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

/// A `recall.*` value within what recall can score: finite; a saturation or a half-life above
/// zero (zero divides by zero); a weight, the dissent's boost and the floor at most 1; the floor
/// above zero, since a floor of zero lets every note match.
fn recall_bound(key: &str, value: &Value, at: &Origin) -> Result<(), String> {
    let v = value
        .as_float()
        .or_else(|| {
            value
                .as_integer()
                .and_then(|i| u32::try_from(i).ok())
                .map(f64::from)
        })
        .unwrap_or(f64::NAN);
    let name = key.trim_start_matches("recall.");
    let fits = v.is_finite()
        && if name.starts_with("saturation") || name == "half_life_days" {
            v > 0.0
        } else if name == "floor" {
            v > 0.0 && v <= 1.0
        } else {
            v <= 1.0
        };
    if fits {
        Ok(())
    } else {
        Err(format!(
            "{at}: {key} is {v}; recall's weights are at most 1, its floor above 0 and at most 1, \
             its saturations and half-life above 0"
        ))
    }
}

fn check_kind(key: &str, value: &Value, kind: Kind, origin: &Origin) -> Result<(), String> {
    let ok = match kind {
        Kind::Bool => value.is_bool(),
        Kind::Int => value.as_integer().is_some_and(|i| i >= 0),
        Kind::Number => {
            value.as_float().is_some_and(|f| f >= 0.0) || value.as_integer().is_some_and(|i| i >= 0)
        }
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
        // A revocation may come before the signing day: an envelope signed ahead can be withdrawn
        // before it ever takes force, and the record says so.
        let revoked = date(t, "revoked", name, at)?;
        let longest = signed.plus(days);
        let expires = date(t, "expires", name, at)?.unwrap_or(longest);
        if expires <= signed {
            return Err(format!(
                "{at}: envelope `{name}` expires on {expires}, not after it was signed on {signed}"
            ));
        }
        // A revoked envelope covers nothing, so a limit lowered after it was signed does not
        // judge it: the record stays, and the configuration still loads.
        if expires > longest && revoked.is_none() {
            return Err(format!(
                "{at}: envelope `{name}` expires on {expires}, more than {days} days after it was \
                 signed on {signed}; sign it again for at most {days} days, revoke it, or delete \
                 the record if it has ended — or raise `oni.envelope_days` in your own file"
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
        let (world, repo, post, body) = scope(t, name, at)?;
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
            world,
            repo,
            post,
            body,
        });
    }
    Ok(out)
}

/// What an envelope is signed for: its world, repository, post and body, each optional.
type Scope = (
    Option<PathBuf>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn scope(t: &Table, name: &str, at: &Origin) -> Result<Scope, String> {
    let text = |key: &str| -> Result<Option<String>, String> {
        match t.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| Some(s.to_owned()))
                .ok_or_else(|| format!("{at}: envelope `{name}`: `{key}` is a non-empty string")),
        }
    };
    let world = text("world")?.map(PathBuf::from);
    if world.as_ref().is_some_and(|w| !w.is_absolute()) {
        return Err(format!(
            "{at}: envelope `{name}`: `world` is the world's absolute path"
        ));
    }
    let repo = text("repo")?;
    if repo.as_ref().is_some_and(|r| {
        let mut parts = r.split('/');
        !(parts.next().is_some_and(|p| !p.is_empty())
            && parts.next().is_some_and(|p| !p.is_empty())
            && parts.next().is_none())
    }) {
        return Err(format!("{at}: envelope `{name}`: `repo` is `owner/name`"));
    }
    let post = text("post")?;
    let body = text("body")?;
    Ok((world, repo, post, body))
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
    // A `*` inside a word stands for a plain name (`feat/*` as `feat/x`): the classifier reads a
    // literal `*` in a refspec as a glob.
    let literal = |fill: &str| -> String {
        words
            .iter()
            .map(|w| {
                if *w == "*" {
                    fill.to_owned()
                } else {
                    w.replace('*', "x")
                }
            })
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
