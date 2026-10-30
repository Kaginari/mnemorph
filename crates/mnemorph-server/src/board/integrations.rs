//! Integrations and settings, read-only, as `GET /api/integrations[?world=<id>]` shows them: the
//! providers, the models each role rides and their prices, each key's *source* — the variable
//! that holds it, and whether it is set — never its value; the configuration's holes; the MCP
//! servers the human's global file declares; the effective configuration, each key with its
//! origin and whether only the global file may set it; and the board's own port.
//!
//! Secrets never cross. What is shown is an allowlist of fields; every string the configuration
//! holds passes [`redact`], which drops a value whose key names a secret, that carries credentials
//! in a URL, a secret `NAME=value`, or a word shaped like a token; a variable's name is shown only
//! when it is one. Last, the answer's bytes are searched for the value of every secret variable
//! in the board's environment, and an answer that holds one is withheld whole.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mnemorph_loop::config::{self, Config, Origin, Paths};
use mnemorph_loop::is_secret;
use serde::Serialize;
use toml::Value;
use ts_rs::TS;

use super::overview::Sourced;

/// Where a key is read from, never what it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct KeySource {
    /// The environment variable that holds it; `null` when none is named, or when what is named
    /// is not a variable's name (then hidden: it may be a key written in its place).
    pub variable: Option<String>,
    /// `set`, `unset`, `none` (no variable named) or `hidden`.
    pub state: String,
}

/// A configured provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderView {
    /// Its name in the configuration.
    pub name: String,
    /// `anthropic`, `openai` or `systemone`.
    pub kind: String,
    /// Its base URL; `null` when hidden (it carried credentials).
    pub base_url: Option<String>,
    /// Where its key is read from.
    pub key: KeySource,
    /// Where it was defined.
    pub origin: String,
}

/// A model's prices, in US dollars per million tokens.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PriceView {
    /// The model id.
    pub model: String,
    /// Uncached input.
    pub input: f64,
    /// Output.
    pub output: f64,
    /// Cache reads.
    pub cache_read: f64,
    /// Cache writes (5 minutes).
    pub cache_write: f64,
}

/// A model the configuration names, and where it goes.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ModelView {
    /// The key that names it: `models.tier.frontier`, `models.race.slime`, …
    pub key: String,
    /// The model as configured; `null` when hidden.
    pub model: Option<String>,
    /// The provider its requests go to; `null` when it resolves to none.
    pub provider: Option<String>,
    /// Its prices from the shipped table; `null` when it is unpriced — never free.
    pub price: Option<PriceView>,
    /// Why it resolves to no provider.
    pub problem: Option<String>,
    /// Where it was set.
    pub origin: String,
}

/// A key the configuration did not take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct HoleView {
    /// The dotted key; `null` when hidden.
    pub key: Option<String>,
    /// The file it was found in.
    pub origin: String,
    /// Why it was not taken.
    pub why: String,
}

/// An MCP server the global file declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct McpServerView {
    /// Its name.
    pub name: String,
    /// The program; `null` when hidden.
    pub command: Option<String>,
    /// Its arguments, a hidden one as `null`.
    pub args: Vec<Option<String>>,
    /// The variables passed to it by name, and whether each is set.
    pub env: Vec<KeySource>,
    /// The races its tools are offered to: its scope.
    pub races: Vec<String>,
    /// Its tools only read.
    pub read_only: bool,
    /// Sockets allowed in its sandbox.
    pub network: bool,
    /// How long a call may take, in seconds.
    #[ts(type = "number")]
    pub timeout_secs: u64,
}

/// The MCP servers, and how a large catalog is offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct McpView {
    /// The servers.
    pub servers: Vec<McpServerView>,
    /// Past this many tools in a race's scope, the rest are deferred.
    #[ts(type = "number")]
    pub defer_above: u64,
    /// Ranked tools that ride the frame when deferred.
    #[ts(type = "number")]
    pub top_k: u64,
    /// The most schema tokens a frame spends on MCP tools.
    #[ts(type = "number")]
    pub schema_tokens: u64,
}

/// One key of the effective configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Setting {
    /// The dotted key.
    pub key: String,
    /// Its value as TOML writes it; `null` when hidden.
    pub value: Option<String>,
    /// Its layer: `default`, `global`, `tracked` or `local`.
    pub layer: String,
    /// Its file, or `built in`.
    pub file: String,
    /// Whether only the human's global file may set it.
    pub global_only: bool,
}

/// The configuration, read.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ConfigView {
    /// The providers.
    pub providers: Vec<ProviderView>,
    /// The models each key names.
    pub models: Vec<ModelView>,
    /// The holes.
    pub holes: Vec<HoleView>,
    /// The MCP servers; `null` value with why when they do not hold together.
    pub mcp: Sourced<McpView>,
    /// Every key, with its origin.
    pub effective: Vec<Setting>,
}

/// The board's own settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardSettings {
    /// The port it listens on.
    pub port: u16,
    /// The addresses it is bound to: loopback only.
    pub hosts: Vec<String>,
}

/// `GET /api/integrations`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BoardIntegrations {
    /// The world whose files are layered over the global one, by id; `null` for the global file
    /// alone.
    pub world: Option<String>,
    /// The human's global file.
    pub global: String,
    /// The configuration; `null` with why when it does not read.
    pub config: Sourced<ConfigView>,
    /// The shipped price table: a model not in it is unpriced.
    pub prices: Vec<PriceView>,
    /// The board's own settings.
    pub board: BoardSettings,
}

/// Whether `word` is shaped like a key or a token: a known prefix, or a long run of key
/// characters mixing letters and digits.
fn token_like(word: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "sk-",
        "sk_",
        "pk-",
        "rk-",
        "ghp_",
        "gho_",
        "ghs_",
        "ghu_",
        "github_pat_",
        "glpat-",
        "xox",
        "akia",
        "asia",
        "aiza",
        "ya29.",
        "hf_",
        "eyj",
        "npm_",
        "pypi-",
    ];
    let lower = word.to_ascii_lowercase();
    if word.len() >= 8 && PREFIXES.iter().any(|p| lower.starts_with(p)) {
        return true;
    }
    let keyish = word
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_+=/.".contains(c));
    let digits = word.chars().any(|c| c.is_ascii_digit());
    let letters = word.chars().any(|c| c.is_ascii_alphabetic());
    let mixed = word.chars().any(|c| c.is_ascii_uppercase())
        && word.chars().any(|c| c.is_ascii_lowercase());
    // A path or a model id has slashes, dots and dashes between short words; a key is one run.
    let longest = word
        .split(['/', '.', '-', ':'])
        .map(str::len)
        .max()
        .unwrap_or(0);
    keyish && digits && letters && (longest >= 32 || (mixed && longest >= 20))
}

/// `value`, a string the configuration holds at `key`, if it may be shown: `None` when the key
/// names a secret, or the value carries credentials or anything shaped like a key.
#[must_use]
pub fn redact(key: &str, value: &str) -> Option<String> {
    let leaf = key.rsplit('.').next().unwrap_or(key);
    if is_secret(leaf, value) || is_secret(key, value) && !key.ends_with(".key_env") {
        return None;
    }
    // Words are runs of what a key, a URL or an assignment is made of: quotes, backticks and
    // brackets around one never hide it.
    for word in value.split(|c: char| !(c.is_ascii_alphanumeric() || "-_+=/.:@%~".contains(c))) {
        if word.is_empty() {
            continue;
        }
        if let Some((name, v)) = word.split_once('=')
            && (is_secret(name.trim_start_matches('-'), v) || token_like(v))
        {
            return None;
        }
        if token_like(word) || is_secret("", word) {
            return None;
        }
        // A word like `user:pass@host`, credentials with no scheme.
        if let Some((before, _)) = word.split_once('@')
            && before.contains(':')
            && !before.contains('/')
        {
            return None;
        }
    }
    Some(value.to_owned())
}

/// Whether `name` is an environment variable's name.
fn variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 64
        && !token_like(name)
}

/// The source `name` names: the variable, and whether it is set in `env` — its value never read
/// past that.
fn source(name: Option<&str>, set: &dyn Fn(&str) -> bool) -> KeySource {
    match name {
        None => KeySource {
            variable: None,
            state: "none".to_owned(),
        },
        Some(n) if !variable_name(n) => KeySource {
            variable: None,
            state: "hidden".to_owned(),
        },
        Some(n) => KeySource {
            variable: Some(n.to_owned()),
            state: if set(n) { "set" } else { "unset" }.to_owned(),
        },
    }
}

fn origin(o: &Origin) -> String {
    o.to_string()
}

/// The first line of a configuration error, redacted: a parse error quotes the line it stopped
/// at, which may hold a key.
pub(super) fn first_line(e: &str) -> String {
    let line = e.lines().next().unwrap_or_default();
    redact("", line).unwrap_or_else(|| "the configuration does not read".to_owned())
}

/// A leaf's value as TOML writes it, if every string in it may be shown.
fn shown(key: &str, value: &Value) -> Option<String> {
    match value {
        Value::Boolean(_) | Value::Integer(_) | Value::Float(_) | Value::Datetime(_) => {
            Some(value.to_string())
        }
        Value::String(s) if key.ends_with(".key_env") => {
            variable_name(s).then(|| value.to_string())
        }
        Value::String(s) => redact(key, s).map(|_| value.to_string()),
        Value::Array(items) => {
            for item in items {
                shown(key, item)?;
            }
            Some(value.to_string())
        }
        // Tables (the Oni envelopes): `mnemorphd envelope list` shows them.
        Value::Table(t) => Some(format!(
            "{} entries — see `mnemorphd envelope list`",
            t.len()
        )),
    }
}

/// The models the configuration names, where each goes and what it costs.
fn models(c: &Config, prices: &config_prices::Table) -> Vec<ModelView> {
    c.explain("models")
        .into_iter()
        .filter(|e| e.key != "models.provider")
        .filter_map(|e| {
            let leaf = c.get(&e.key)?;
            let name = leaf.value.as_str()?;
            let Some(model) = redact(&e.key, name) else {
                return Some(ModelView {
                    key: e.key,
                    model: None,
                    provider: None,
                    price: None,
                    problem: None,
                    origin: origin(&e.origin),
                });
            };
            let (provider, id, problem) = match c.backend(&model) {
                Ok(b) => (Some(b.provider_name), b.model, None),
                Err(why) => (None, model.clone(), Some(first_line(&why))),
            };
            Some(ModelView {
                key: e.key,
                price: prices.get(&id).cloned(),
                model: Some(model),
                provider,
                problem,
                origin: origin(&e.origin),
            })
        })
        .collect()
}

/// The shipped price table, keyed by model.
mod config_prices {
    use super::PriceView;
    use std::collections::BTreeMap;

    pub type Table = BTreeMap<String, PriceView>;

    /// The shipped table; empty when it does not parse (a build defect its own test catches).
    pub fn table() -> Table {
        mnemorph_loop::anthropic_prices()
            .map(|p| {
                p.models
                    .into_iter()
                    .map(|(model, price)| {
                        let view = PriceView {
                            model: model.clone(),
                            input: price.input,
                            output: price.output,
                            cache_read: price.cache_read,
                            cache_write: price.cache_write,
                        };
                        (model, view)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The MCP servers `c` declares.
fn mcp(c: &Config, set: &dyn Fn(&str) -> bool) -> Sourced<McpView> {
    const SOURCE: &str = "mcp.servers in your global file";
    let settings = match c.mcp() {
        Ok(s) => s,
        Err(e) => return Sourced::unknown(SOURCE, true, first_line(&e)),
    };
    let servers = settings
        .servers
        .iter()
        .map(|s| {
            let at = |f: &str| format!("mcp.servers.{}.{f}", s.name);
            // An argument after a flag that names a secret is the secret.
            let mut after_secret = false;
            let args = s
                .args
                .iter()
                .map(|a| {
                    let hide = after_secret;
                    let flag = a.trim_start_matches('-');
                    after_secret = a.starts_with('-') && !a.contains('=') && is_secret(flag, "");
                    if hide { None } else { redact(&at("args"), a) }
                })
                .collect();
            McpServerView {
                name: s.name.clone(),
                command: redact(&at("command"), &s.command),
                args,
                env: s.env.iter().map(|n| source(Some(n), set)).collect(),
                races: s.races.iter().map(|r| r.name().to_owned()).collect(),
                read_only: s.read_only,
                network: s.network,
                timeout_secs: s.timeout.as_secs(),
            }
        })
        .collect();
    let n = |v: usize| u64::try_from(v).unwrap_or(u64::MAX);
    Sourced::known(
        McpView {
            servers,
            defer_above: n(settings.defer_above),
            top_k: n(settings.top_k),
            schema_tokens: settings.schema_tokens,
        },
        SOURCE,
        true,
    )
}

/// `c`, shown: every field allowlisted, every string redacted.
fn view(c: &Config, set: &dyn Fn(&str) -> bool) -> ConfigView {
    let providers = c
        .providers()
        .into_iter()
        .map(|(name, p)| ProviderView {
            origin: c
                .get(&format!("providers.{name}.kind"))
                .or_else(|| c.get(&format!("providers.{name}.base_url")))
                .map(|l| origin(&l.origin))
                .unwrap_or_default(),
            base_url: redact(&format!("providers.{name}.base_url"), &p.base_url),
            key: source(p.key_env.as_deref(), set),
            kind: redact("kind", &p.kind).unwrap_or_default(),
            name,
        })
        .collect();
    let holes = c
        .holes()
        .iter()
        .map(|h| HoleView {
            key: redact("", &h.key),
            origin: origin(&h.origin),
            why: first_line(&h.why),
        })
        .collect();
    let effective = c
        .explain("")
        .into_iter()
        .map(|e| {
            let value = c.get(&e.key).and_then(|l| shown(&e.key, &l.value));
            Setting {
                global_only: config::global_only(&e.key),
                layer: e.origin.layer.to_string(),
                file: e
                    .origin
                    .file
                    .as_ref()
                    .map_or_else(|| "built in".to_owned(), |f| f.display().to_string()),
                key: e.key,
                value,
            }
        })
        .collect();
    ConfigView {
        providers,
        models: models(c, &config_prices::table()),
        holes,
        mcp: mcp(c, set),
        effective,
    }
}

/// The integrations: the configuration of `world` (its files over `global`), or of `global` alone;
/// `set` says whether a variable is set, and is never asked its value.
#[must_use]
pub fn integrations(
    global: &Path,
    world: Option<(&str, &Path)>,
    board: BoardSettings,
    set: &dyn Fn(&str) -> bool,
) -> BoardIntegrations {
    let (paths, layers) = match world {
        Some((_, dir)) => (Paths::with_global(global.to_path_buf(), dir), true),
        // No world: the global file alone (the world's files are never read).
        None => (
            Paths::with_global(global.to_path_buf(), Path::new("/nonexistent")),
            false,
        ),
    };
    let loaded = if layers {
        Config::load(&paths)
    } else {
        Config::load_global(&paths)
    };
    let source = if layers {
        "the global file, then the world's tracked and local files"
    } else {
        "the global file"
    };
    let config = match loaded {
        Ok(c) => Sourced::known(view(&c, set), source, !layers),
        Err(e) => Sourced::unknown(source, !layers, first_line(&e)),
    };
    BoardIntegrations {
        world: world.map(|(id, _)| id.to_owned()),
        global: global.display().to_string(),
        config,
        prices: config_prices::table().into_values().collect(),
        board,
    }
}

/// The values of the secret variables in `env` worth searching for: long enough that finding one
/// is not chance. A name or value that is not UTF-8 is read lossily: never a panic, and a value
/// still searched for as it would be shown.
#[must_use]
pub fn secret_values(
    env: impl Iterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) -> BTreeSet<String> {
    env.map(|(k, v)| {
        (
            k.to_string_lossy().into_owned(),
            v.to_string_lossy().into_owned(),
        )
    })
    .filter(|(k, v)| is_secret(k, v) && v.len() >= 8)
    .map(|(_, v)| v)
    .collect()
}

/// `s` percent-encoded as a URL's query or path would carry it: every byte but the unreserved.
fn url_encoded(s: &str) -> String {
    use std::fmt::Write as _;
    s.bytes()
        .fold(String::with_capacity(s.len()), |mut out, b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                out.push(char::from(b));
            } else {
                let _ = write!(out, "%{b:02X}");
            }
            out
        })
}

/// Every form a secret may take in an answer: as it is, escaped as a JSON string writes it (a `"`
/// or a `\` in it changes its bytes), and percent-encoded.
fn forms(secret: &str) -> Vec<String> {
    let mut out = vec![secret.to_owned()];
    if let Ok(json) = serde_json::to_string(secret) {
        out.push(json[1..json.len() - 1].to_owned());
    }
    out.push(url_encoded(secret));
    out.sort();
    out.dedup();
    out
}

/// Whether `bytes` hold any of `secrets`, in any of the forms an answer may carry it.
#[must_use]
pub fn leaks(bytes: &[u8], secrets: &BTreeSet<String>) -> bool {
    secrets.iter().flat_map(|s| forms(s)).any(|f| {
        let f = f.as_bytes();
        !f.is_empty() && bytes.windows(f.len()).any(|w| w == f)
    })
}

/// The human's global file, as every reader resolves it.
#[must_use]
pub fn global_file() -> PathBuf {
    Paths::resolve(Path::new("/nonexistent")).global
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_drops_what_names_or_looks_like_a_secret() {
        for (key, value) in [
            ("providers.or.api_key", "anything"),
            ("x.token", "abc"),
            ("providers.or.base_url", "https://user:pw@example.com/v1"),
            (
                "mcp.servers.gh.command",
                "env GITHUB_TOKEN=ghp_abc123 gh-mcp",
            ),
            (
                "mcp.servers.gh.args",
                "ghp_16C7e42F292c6912E7710c838347Ae178B4a",
            ),
            ("mcp.servers.x.args", "--key=sk-or-v1-0123456789abcdef"),
            (
                "",
                "gate.ask_rules: no guard rule `ghp_16C7e42F292c6912E7710c838347Ae178B4a`;",
            ),
            (
                "models.default",
                "sk-or-v1-0123456789abcdef0123456789abcdef",
            ),
            (
                "a.b",
                "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
            ),
        ] {
            assert_eq!(redact(key, value), None, "{key} = {value}");
        }
        for (key, value) in [
            ("models.tier.fast", "deepseek/deepseek-v4-flash-0731"),
            (
                "models.tier.strong",
                "openrouter:anthropic/claude-sonnet-5-5",
            ),
            ("providers.or.base_url", "https://openrouter.ai/api/v1"),
            ("mcp.servers.search.command", "web-search-mcp"),
            ("board.bench_results", "/home/h/kaginari/bench/results"),
        ] {
            assert_eq!(redact(key, value).as_deref(), Some(value), "{key}");
        }
    }

    #[test]
    fn a_key_source_names_a_variable_or_hides_what_is_not_one() {
        let set = |n: &str| n == "OPENROUTER_API_KEY";
        assert_eq!(
            source(Some("OPENROUTER_API_KEY"), &set),
            KeySource {
                variable: Some("OPENROUTER_API_KEY".into()),
                state: "set".into()
            }
        );
        assert_eq!(source(Some("OTHER_KEY"), &set).state, "unset");
        assert_eq!(source(None, &set).state, "none");
        let inline = source(Some("sk-or-v1-0123456789abcdef0123456789abcdef"), &set);
        assert_eq!((inline.variable, inline.state.as_str()), (None, "hidden"));
    }

    #[test]
    fn an_answer_holding_a_secret_is_found() {
        let os = |k: &str, v: &str| (std::ffi::OsString::from(k), std::ffi::OsString::from(v));
        let secrets = secret_values(
            [
                os("OPENROUTER_API_KEY", "sk-or-fake-0123"),
                os("HOME", "/home/h"),
                os("SHORT_TOKEN", "abc"),
            ]
            .into_iter(),
        );
        assert_eq!(secrets.len(), 1);
        assert!(leaks(br#"{"x":"sk-or-fake-0123"}"#, &secrets));
        assert!(!leaks(br#"{"x":"/home/h"}"#, &secrets));
    }

    #[test]
    fn a_secret_is_found_escaped_as_json_or_url_encoded() {
        let secret = r#"pa"ss\word-FAKE/+= 99"#;
        let secrets: BTreeSet<String> = [secret.to_owned()].into();
        let body =
            serde_json::to_vec(&serde_json::json!({ "why": format!("x {secret} y") })).unwrap();
        assert!(
            !body.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "escaped in JSON"
        );
        assert!(leaks(&body, &secrets));
        let url = format!("{{\"source\":\"https://h/?k={}\"}}", url_encoded(secret));
        assert!(leaks(url.as_bytes(), &secrets));
        assert!(!leaks(br#"{"why":"nothing"}"#, &secrets));
    }

    #[cfg(unix)]
    #[test]
    fn a_value_that_is_not_utf8_is_read_lossily_never_a_panic() {
        use std::os::unix::ffi::OsStringExt as _;
        let bad = std::ffi::OsString::from_vec(b"tok\xffen-FAKE-0123".to_vec());
        let secrets = secret_values([(std::ffi::OsString::from("API_TOKEN"), bad)].into_iter());
        assert_eq!(secrets.len(), 1);
        assert!(secrets.iter().next().unwrap().contains("en-FAKE-0123"));
    }
}
