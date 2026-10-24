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
    /// Claims inside the soil that are not the body's: the longest claim that covers a path
    /// decides, and on a tie the exclusion wins.
    except: Vec<PathBuf>,
    /// Whether the world's root files are the body's (the workspace).
    root_files: bool,
    /// Whether the soil was handed by the world (a commissioned creature): then commands are
    /// granted exactly what it admits, not the whole world.
    narrowed: bool,
    home: PathBuf,
    gate: Arc<dyn Gate>,
    sandbox: Sandbox,
    asked_rules: BTreeSet<Rule>,
    unguarded: bool,
    attic: Option<Arc<crate::attic::Attic>>,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    deadline: Option<std::time::Instant>,
    pub(crate) seen: HashMap<PathBuf, Seen>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("race", &self.race)
            .field("world", &self.world)
            .field("soil", &self.soil)
            .field("except", &self.except)
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
            except: Vec::new(),
            root_files: false,
            narrowed: false,
            home,
            gate: Arc::new(NoHuman),
            sandbox: Sandbox::detect(),
            asked_rules: BTreeSet::new(),
            unguarded: false,
            attic: None,
            cancel: None,
            deadline: None,
            seen: HashMap::new(),
        }
    }

    /// The body's soil as the world hands it: claims it writes, claims inside them it does not,
    /// and whether the root files are its own (paths relative to the world).
    #[must_use]
    pub fn with_soil(
        mut self,
        write: &[impl AsRef<Path>],
        except: &[impl AsRef<Path>],
        root_files: bool,
    ) -> Self {
        self.soil = write
            .iter()
            .map(|s| real(&self.world.join(s.as_ref())))
            .collect();
        self.except = except
            .iter()
            .map(|s| real(&self.world.join(s.as_ref())))
            .collect();
        self.root_files = root_files;
        self.narrowed = true;
        self
    }

    /// What a command may write, for the kernel guard: the whole world for a solo body; for a
    /// commissioned creature, exactly the existing paths its soil admits, plus what git ignores
    /// (build output, never a landing). The guard can allow a folder but not except a spot inside
    /// it, so a claim that holds another's spot is granted part by part around that spot — and a
    /// new file directly beside such a spot is the write tool's to make.
    #[must_use]
    pub fn grants(&self) -> Vec<PathBuf> {
        if !self.narrowed {
            return vec![self.world.clone()];
        }
        let mut out = Vec::new();
        for w in &self.soil {
            self.grant(w, &mut out);
        }
        if self.root_files
            && let Ok(entries) = std::fs::read_dir(&self.world)
        {
            for e in entries.filter_map(Result::ok) {
                let p = e.path();
                if p.is_file() && self.admits(&p) {
                    out.push(p);
                }
            }
        }
        // Build output, never the records, git's own files, or anything that holds or sits in
        // another's spot: an ignored `.mnemorph/instruments/` is the harness's ledger, not output.
        let records = [self.world.join(".mnemorph"), self.world.join(".git")];
        let built: Vec<PathBuf> = ignored(&self.world)
            .into_iter()
            .filter(|p| {
                !records.iter().any(|r| p.starts_with(r) || r.starts_with(p))
                    && !self
                        .except
                        .iter()
                        .any(|e| e.starts_with(p) || p.starts_with(e))
            })
            .collect();
        let built_real: Vec<PathBuf> = built.iter().filter_map(|b| b.canonicalize().ok()).collect();
        out.extend(built);
        // The guard resolves every rule to the file it names: a symlink granted as an entry would
        // grant its target. So each grant is resolved here, and kept only if what it resolves to
        // is still inside the world, outside the records, and the creature's own.
        let mut out: Vec<PathBuf> = out
            .into_iter()
            .filter_map(|g| {
                let real = g.canonicalize().ok()?;
                // A folder that holds another's spot is never granted whole.
                let holds = self
                    .except
                    .iter()
                    .any(|e| e.starts_with(&real) && *e != real)
                    || records.iter().any(|r| r.starts_with(&real));
                let built = built_real.iter().any(|b| real.starts_with(b))
                    && !records.iter().any(|r| real.starts_with(r))
                    && !self.except.iter().any(|e| real.starts_with(e));
                (real.starts_with(&self.world) && !holds && (self.admits(&real) || built))
                    .then_some(real)
            })
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Grant `path` whole when no exclusion lies inside it; else each of its parts around them.
    fn grant(&self, path: &Path, out: &mut Vec<PathBuf>) {
        if !path.exists() {
            return;
        }
        if !self.except.iter().any(|e| e.starts_with(path) && e != path) {
            if self.admits(path) {
                out.push(path.to_path_buf());
            }
            return;
        }
        if let Ok(entries) = std::fs::read_dir(path) {
            for e in entries.filter_map(Result::ok) {
                self.grant(&e.path(), out);
            }
        }
    }

    /// Where a command keeps its temporary files: the creature's own scratch nest when the
    /// world handed its soil, else the world's scratch.
    #[must_use]
    pub fn scratch(&self) -> PathBuf {
        let all = self.world.join(".mnemorph/tmp");
        if self.narrowed {
            // The nest, resolved: a nest that is a link elsewhere is no nest.
            return self
                .soil
                .iter()
                .find(|s| s.starts_with(&all) && *s != &all)
                .filter(|nest| nest.canonicalize().map_or(true, |r| r == **nest))
                .cloned()
                // No nest of its own, or one that links elsewhere: a private directory outside
                // the world, never a scratch others share.
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!("mnemorph-scratch-{}", std::process::id()))
                });
        }
        all
    }

    /// Whether the soil admits `path`: the longest covering claim decides, a tie to the
    /// exclusion; a root file by `root_files`, unless a claim excludes it.
    fn admits(&self, path: &Path) -> bool {
        let best = |claims: &[PathBuf]| {
            claims
                .iter()
                .filter(|c| path.starts_with(c))
                .map(|c| c.components().count())
                .max()
        };
        let root = (self.root_files && path.parent() == Some(self.world.as_path())).then_some(0);
        match (best(&self.soil).max(root), best(&self.except)) {
            (Some(w), Some(e)) => w > e,
            (Some(_), None) => true,
            (None, _) => false,
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

    /// Let commands run when no kernel guard is in force — the human's own choice, from their
    /// local or global configuration. Without it, no guard means no command.
    #[must_use]
    pub fn with_unguarded(mut self, allowed: bool) -> Self {
        self.unguarded = allowed;
        self
    }

    /// The sandbox commands run in.
    #[must_use]
    pub fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }

    /// Keep what commands and writes take away in this attic.
    #[must_use]
    pub fn with_attic(mut self, attic: crate::attic::Attic) -> Self {
        self.attic = Some(Arc::new(attic));
        self
    }

    /// The attic, if the context keeps one.
    #[must_use]
    pub fn attic(&self) -> Option<&crate::attic::Attic> {
        self.attic.as_deref()
    }

    /// Stop a running command as soon as this flag is set — the human's Esc.
    #[must_use]
    pub fn with_cancel(mut self, flag: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    /// No command runs past this instant — the run's clock.
    pub fn set_deadline(&mut self, deadline: Option<std::time::Instant>) {
        self.deadline = deadline;
    }

    /// Whether the human asked to stop.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|f| f.load(std::sync::atomic::Ordering::SeqCst))
    }

    /// The instant no command may run past, if any.
    #[must_use]
    pub fn deadline(&self) -> Option<std::time::Instant> {
        self.deadline
    }

    /// Whether commands may run with no kernel guard in force.
    #[must_use]
    pub fn allows_unguarded(&self) -> bool {
        self.unguarded
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
        if self.admits(&path) {
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

/// What git ignores in `world`, resolved: build output and caches, writable by any body because
/// they are never a landing (the end gate's snapshot skips them too).
fn ignored(world: &Path) -> Vec<PathBuf> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(world)
        .args([
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ])
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| world.join(p.trim_end_matches('/')))
        .filter(|p| p.exists())
        .collect()
}
