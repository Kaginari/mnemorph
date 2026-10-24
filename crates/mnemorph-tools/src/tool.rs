//! What every tool shares: one trait, one registry, one way to be called.
//!
//! A tool is declared once — its `Tool` impl and its line in [`registry`] — and its schema,
//! class and shelf seat come from there. Results carry structured metadata (`class`,
//! `unchanged`, `wrote`) that consumers read instead of parsing the text.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_races::{Race, ToolClass, check_class};
use mnemorph_sandbox::Sandbox;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::classify::Rule;
use crate::classify::paths::{join, real};
use crate::files;
use crate::gate::{Gate, NoHuman};
use crate::shell;

/// One tool a body can call.
pub trait Tool: Send + Sync {
    /// The name the model calls it by.
    fn name(&self) -> &'static str;
    /// What it does, for the model.
    fn description(&self) -> &'static str;
    /// Its JSON Schema (an object schema).
    fn schema(&self) -> Value;
    /// The lowest class a call runs at, which decides the shelf it sits on. A call may rise
    /// above it: `bash` classifies each command.
    fn class(&self) -> ToolClass;
    /// A valid call, kept beside the schema so the two cannot drift.
    fn example(&self) -> Value;
    /// Run it.
    fn run(&self, args: Value, cx: &mut Context) -> Outcome;
}

/// Every tool, in the order the model sees them.
#[must_use]
pub fn registry() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(files::Read),
        Box::new(files::Write),
        Box::new(files::Edit),
        Box::new(files::Glob),
        Box::new(files::Grep),
        Box::new(shell::Bash),
    ]
}

/// What a call returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The text the model reads.
    pub content: String,
    /// The call failed; `content` says why and what to do instead.
    pub is_error: bool,
    /// What happened, for the harness.
    pub meta: Meta,
}

/// Structured facts about a call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meta {
    /// The class the call ran at; `None` when it did not run.
    pub class: Option<ToolClass>,
    /// The read returned a pointer to an earlier, still-true result instead of the text.
    pub unchanged: bool,
    /// Files the call wrote.
    pub wrote: Vec<PathBuf>,
    /// The exit code of a command; `None` for other tools, or a command killed by a signal.
    pub exit: Option<i32>,
}

impl Outcome {
    pub(crate) fn ok(content: impl Into<String>, class: ToolClass) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            meta: Meta {
                class: Some(class),
                ..Meta::default()
            },
        }
    }

    pub(crate) fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            meta: Meta::default(),
        }
    }
}

/// Where a body works: who it is, the world it may read, the soil it may write, the gate it
/// asks, and what it already read.
pub struct Context {
    race: Race,
    world: PathBuf,
    soil: Vec<PathBuf>,
    home: PathBuf,
    gate: Arc<dyn Gate>,
    sandbox: Sandbox,
    asked_rules: BTreeSet<Rule>,
    pub(crate) seen: HashMap<PathBuf, Seen>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("race", &self.race)
            .field("world", &self.world)
            .field("soil", &self.soil)
            .field("home", &self.home)
            .field("sandbox", &self.sandbox)
            .finish_non_exhaustive()
    }
}

/// The last window of a file a body read, and the content it saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Seen {
    pub(crate) window: (usize, usize),
    pub(crate) hash: u64,
}

impl Context {
    /// A body of `race` in `world` that may write under each prefix of `soil` (paths relative to
    /// the world are joined onto it). Its gate denies until [`Self::with_gate`] attaches a human.
    pub fn new(race: Race, world: impl AsRef<Path>, soil: &[impl AsRef<Path>]) -> Self {
        let world = real(world.as_ref());
        let soil = soil.iter().map(|s| real(&world.join(s.as_ref()))).collect();
        let home =
            std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), |h| real(Path::new(&h)));
        Self {
            race,
            world,
            soil,
            home,
            gate: Arc::new(NoHuman),
            sandbox: Sandbox::detect(),
            asked_rules: BTreeSet::new(),
            seen: HashMap::new(),
        }
    }

    /// Ask this gate for outward and destructive acts.
    #[must_use]
    pub fn with_gate(mut self, gate: Arc<dyn Gate>) -> Self {
        self.gate = gate;
        self
    }

    /// Resolve `~` and `$HOME` to this directory instead of the process's home.
    #[must_use]
    pub fn with_home(mut self, home: impl AsRef<Path>) -> Self {
        self.home = real(home.as_ref());
        self
    }

    /// Contain commands with this sandbox instead of the one detected for the machine.
    #[must_use]
    pub fn with_sandbox(mut self, sandbox: Sandbox) -> Self {
        self.sandbox = sandbox;
        self
    }

    /// Turn these guard rules from "refuse" into "ask the human". Rules on the floor stay
    /// refusals whatever the list says.
    #[must_use]
    pub fn with_asked_rules(mut self, rules: impl IntoIterator<Item = Rule>) -> Self {
        self.asked_rules = rules.into_iter().filter(|r| !r.floor()).collect();
        self
    }

    /// Whether a refusal under `rule` becomes a question for the human.
    #[must_use]
    pub fn asks(&self, rule: Rule) -> bool {
        !rule.floor() && self.asked_rules.contains(&rule)
    }

    /// The sandbox commands run in.
    #[must_use]
    pub fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }

    /// The body's race.
    #[must_use]
    pub fn race(&self) -> Race {
        self.race
    }

    /// The home `~` resolves to.
    #[must_use]
    pub fn home(&self) -> &Path {
        &self.home
    }

    pub(crate) fn gate(&self) -> &dyn Gate {
        self.gate.as_ref()
    }

    /// The world's root, resolved.
    #[must_use]
    pub fn world(&self) -> &Path {
        &self.world
    }

    /// Resolve a path argument and check it lies inside the world. Symlinks are resolved first,
    /// the parent of a file not yet written included, so no link leads out.
    pub(crate) fn inside(&self, text: &str) -> Result<PathBuf, String> {
        let lexical =
            join(Some(&self.world), text).ok_or_else(|| format!("`{text}` is not a path"))?;
        let path = real(&lexical);
        if path.starts_with(&self.world) {
            Ok(path)
        } else {
            Err(format!(
                "`{text}` is outside the world ({}); tools stay inside it",
                path.display()
            ))
        }
    }

    /// As [`Self::inside`], and the path must lie in the body's soil.
    pub(crate) fn writable(&self, text: &str) -> Result<PathBuf, String> {
        let path = self.inside(text)?;
        if crate::tripwire::protects(&self.world, &path) {
            return Err(format!(
                "`{}` is protected: git's executable configuration and the world's records are not written by tools",
                self.shown(&path)
            ));
        }
        if self.soil.iter().any(|s| path.starts_with(s)) {
            Ok(path)
        } else {
            let soil: Vec<String> = self.soil.iter().map(|s| self.shown(s)).collect();
            Err(format!(
                "`{}` is outside your soil; you may write under: {}",
                self.shown(&path),
                soil.join(", ")
            ))
        }
    }

    /// A path as the model should see it: relative to the world.
    pub(crate) fn shown(&self, path: &Path) -> String {
        match path.strip_prefix(&self.world) {
            Ok(rel) if rel.as_os_str().is_empty() => ".".into(),
            Ok(rel) => rel.display().to_string(),
            Err(_) => path.display().to_string(),
        }
    }
}

/// Call `name` with the raw JSON the model sent. Bad JSON, unknown fields and missing fields are
/// named errors returned to the model; the tool does not run on input it cannot read.
pub fn call(tools: &[Box<dyn Tool>], name: &str, raw_args: &str, cx: &mut Context) -> Outcome {
    let Some(tool) = tools.iter().find(|t| t.name() == name) else {
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        return Outcome::error(format!(
            "there is no tool `{name}`; the tools are: {}",
            names.join(", ")
        ));
    };
    if let Err(refusal) = check_class(cx.race, tool.class()) {
        return Outcome::error(refusal.to_string());
    }
    match serde_json::from_str::<Value>(raw_args) {
        Ok(args) => tool.run(args, cx),
        Err(e) => Outcome::error(format!("the arguments to {name} are not valid JSON: {e}")),
    }
}

/// Parse a tool's arguments, naming the tool in the error.
pub(crate) fn args<T: DeserializeOwned>(name: &str, value: Value) -> Result<T, Outcome> {
    serde_json::from_value(value).map_err(|e| {
        Outcome::error(format!(
            "the arguments to {name} do not fit its schema: {e}"
        ))
    })
}
