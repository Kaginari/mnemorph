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
        Box::new(crate::index::Index),
        Box::new(files::Write),
        Box::new(files::Edit),
        Box::new(files::MultiEdit),
        Box::new(files::Move),
        Box::new(files::Delete),
        Box::new(crate::patch::Patch),
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
// Independent facts about one call, each read on its own: no state machine hides in them.
#[allow(clippy::struct_excessive_bools)]
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
    /// The command was killed at its deadline.
    pub timed_out: bool,
    /// Lines the call added to the files it wrote, counted as a multiset against what they held
    /// before: a line moved within a file is neither added nor removed. Zero for other calls.
    pub lines_added: u32,
    /// Lines the call removed from the files it wrote, counted the same way.
    pub lines_removed: u32,
    /// An edit whose `old_string` does not occur in the file: the body edited text it had not read,
    /// or, in a multiedit, text an earlier edit of the same call had already changed.
    pub edit_miss: bool,
    /// The human gate denied the act: the human's choice, never the body's failure.
    pub denied: bool,
    /// The shelf refused the act: the class the body's race does not hold.
    pub unheld: Option<ToolClass>,
    /// The MCP tool the call reached, by the name a body calls it: what the world's usage record
    /// learns from.
    pub mcp: Option<String>,
}

impl Outcome {
    /// The shelf's refusal of an act its race does not hold, the class kept for the loop.
    #[must_use]
    pub fn unheld(refusal: mnemorph_races::ShelfRefusal) -> Self {
        let mut o = Self::error(refusal.to_string());
        o.meta.unheld = Some(refusal.class);
        o
    }

    /// A result the call produced, of `class`.
    #[must_use]
    pub fn ok(content: impl Into<String>, class: ToolClass) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            meta: Meta {
                class: Some(class),
                ..Meta::default()
            },
        }
    }

    /// A refusal or a failure, in words that name the lawful route.
    #[must_use]
    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            meta: Meta::default(),
        }
    }
}

/// What a refusal outside the soil says next, for a path relative to the world: who owns it and
/// the ways on — written where the roster is known.
pub type OwnerOf = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Where a body works: who it is, the world it may read, the soil it may write, the gate it
/// asks, and what it already read.
pub struct Context {
    race: Race,
    world: PathBuf,
    soil: Vec<PathBuf>,
    /// The body's own seat, absolute: the one place under `.mnemorph` its plan is written.
    seat: Option<PathBuf>,
    /// Who owns what, for a refusal to name: set where the world's roster is known.
    owner_of: Option<OwnerOf>,
    /// Claims inside the soil that are not the body's: the longest claim that covers a path
    /// decides, and on a tie the exclusion wins.
    except: Vec<PathBuf>,
    /// Whether the world's root files are the body's (the workspace).
    root_files: bool,
    /// The body working here, by name: what an approval card names.
    body: String,
    /// Whether the soil was handed by the world (a commissioned creature): then commands are
    /// granted exactly what it admits, not the whole world.
    narrowed: bool,
    /// Build output granted besides the soil (see [`Self::with_build`]).
    build: Vec<PathBuf>,
    /// The creature's scratch nest, when it was handed one.
    nest: Option<PathBuf>,
    home: PathBuf,
    /// The directories outside the world a deploy may write (see [`Self::with_deploy_roots`]).
    deploy_roots: Vec<PathBuf>,
    gate: Arc<dyn Gate>,
    sandbox: Sandbox,
    asked_rules: BTreeSet<Rule>,
    unguarded: bool,
    attic: Option<Arc<crate::attic::Attic>>,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    deadline: Option<std::time::Instant>,
    pub(crate) seen: HashMap<PathBuf, Seen>,
    /// Skeletons made this session, by content and language.
    pub(crate) skeletons: HashMap<u64, crate::index::Skeleton>,
    /// Lines over which a read must name its range (`tools.read.range_over`); 0 never asks.
    range_over: usize,
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
    /// The window was answered once already with a pointer to the earlier result: asked again, it
    /// is shown, not pointed at a second time.
    pub(crate) pointed: bool,
}

impl Context {
    /// Forget which windows were read: the conversation no longer holds their text (a fold or a
    /// compaction took it out), so a re-read must show it again, never point back at it.
    pub fn forget_reads(&mut self) {
        self.seen.clear();
    }

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
            seat: None,
            owner_of: None,
            except: Vec::new(),
            root_files: false,
            narrowed: false,
            body: String::new(),
            build: Vec::new(),
            nest: None,
            home,
            deploy_roots: Vec::new(),
            gate: Arc::new(NoHuman),
            sandbox: Sandbox::detect(),
            asked_rules: BTreeSet::new(),
            unguarded: false,
            attic: None,
            cancel: None,
            deadline: None,
            seen: HashMap::new(),
            skeletons: HashMap::new(),
            range_over: files::RANGE_OVER,
        }
    }

    /// Ask for a range when a read names none on a file over `lines` lines (or over
    /// [`files::BYTES_PER_LINE`] bytes a line of it); 0 reads every file whole.
    #[must_use]
    pub fn with_range_over(mut self, lines: usize) -> Self {
        self.range_over = lines;
        self
    }

    /// Lines over which a read must name its range; 0 never asks.
    #[must_use]
    pub fn range_over(&self) -> usize {
        self.range_over
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
        // A claim reached through a link is no claim: resolved, it would be the link's target.
        self.soil = write
            .iter()
            .filter(|s| !linked(&self.world, s.as_ref()))
            .map(|s| real(&self.world.join(s.as_ref())))
            .collect();
        self.except = except
            .iter()
            .map(|s| real(&self.world.join(s.as_ref())))
            .collect();
        self.root_files = root_files;
        self.narrowed = true;
        // The scratch nest, read before any link is followed; else a fresh private directory.
        let all = self.world.join(".mnemorph/tmp");
        self.nest = write
            .iter()
            .map(|s| self.world.join(s.as_ref()))
            .find(|s| s.starts_with(&all) && *s != all)
            .filter(|s| !linked(&self.world, s.strip_prefix(&self.world).unwrap_or(s)))
            .map(|s| real(&s))
            .or_else(private_scratch);
        self
    }

    /// The build output a commissioned creature may write besides its soil: paths git ignores
    /// that the world gives it or nobody — never another's, never through a link.
    #[must_use]
    pub fn with_build(mut self, build: &[impl AsRef<Path>]) -> Self {
        self.build = build
            .iter()
            .filter(|b| !linked(&self.world, b.as_ref()))
            .map(|b| self.world.join(b.as_ref()))
            .collect();
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
        // Build output, as the world handed it: existing, and never a link.
        let built: Vec<PathBuf> = self
            .build
            .iter()
            .filter(|b| {
                b.symlink_metadata()
                    .is_ok_and(|m| !m.file_type().is_symlink())
            })
            .cloned()
            .collect();
        let built_real: Vec<PathBuf> = built.iter().filter_map(|b| b.canonicalize().ok()).collect();
        let records = [self.world.join(".mnemorph"), self.world.join(".git")];
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
        if self.narrowed {
            return self
                .nest
                .clone()
                .unwrap_or_else(|| self.world.join(".mnemorph/tmp/.none"));
        }
        self.world.join(".mnemorph/tmp")
    }

    /// Whether this body could have written `path` (absolute). Under a kernel guard a commissioned
    /// body's commands write only what it is granted — its soil, its build output, its scratch,
    /// its plan — and its file tools no more, so a change anywhere else is another writer's: a
    /// second session on the world. With no guard, or the whole world as soil, writers cannot be
    /// told apart, and every change may be its.
    #[must_use]
    pub fn could_have_written(&self, path: &Path) -> bool {
        self.write_grant().could_have_written(path)
    }

    /// What this body is granted, kept apart from the context: taken before a commission runs,
    /// the put-back after it reads the grant the body ran with.
    #[must_use]
    pub fn write_grant(&self) -> Grant {
        // Seatbelt guards only on macOS: anywhere else it is no guard at all.
        let guarded = match self.sandbox {
            Sandbox::None(_) => false,
            Sandbox::Seatbelt(_) => cfg!(target_os = "macos"),
            Sandbox::Guard(_) => true,
        };
        Grant {
            confined: self.narrowed && guarded,
            world: self.world.clone(),
            soil: self.soil.clone(),
            except: self.except.clone(),
            root_files: self.root_files,
            seat: self.seat.clone(),
            build: self.build.clone(),
            nest: self.nest.clone(),
        }
    }

    /// Whether the soil admits `path`: the longest covering claim decides, a tie to the
    /// exclusion; a root file by `root_files`, unless a claim excludes it.
    fn admits(&self, path: &Path) -> bool {
        admits(&self.world, &self.soil, &self.except, self.root_files, path)
    }

    /// The body's own seat, relative to the world: its `plan.md` there is the one record the
    /// file tools let it write.
    #[must_use]
    pub fn with_seat(mut self, seat: impl AsRef<Path>) -> Self {
        self.seat = Some(real(&self.world.join(seat.as_ref())));
        self
    }

    /// Who owns a path, so a write outside the soil is refused with its owner named and the
    /// ways on: route, widen, birth.
    #[must_use]
    pub fn with_owner_of(mut self, owner_of: OwnerOf) -> Self {
        self.owner_of = Some(owner_of);
        self
    }

    /// The body working here, by name.
    #[must_use]
    pub fn with_body(mut self, name: &str) -> Self {
        name.clone_into(&mut self.body);
        self
    }

    /// The body working here, by name; empty when none was named.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// What a commissioned body takes from the one that commissioned it: the human's gate, the
    /// sandbox, the home, the guard rules that ask, whether commands may run unguarded, the
    /// attic and the read's range rule. Its soil is its own.
    #[must_use]
    pub fn inherit(mut self, from: &Context) -> Self {
        self.home.clone_from(&from.home);
        self.deploy_roots.clone_from(&from.deploy_roots);
        self.gate = Arc::clone(&from.gate);
        self.sandbox.clone_from(&from.sandbox);
        self.asked_rules.clone_from(&from.asked_rules);
        self.unguarded = from.unguarded;
        self.attic.clone_from(&from.attic);
        self.range_over = from.range_over;
        self
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
    /// global configuration. Without it, no guard means no command.
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

    /// As [`Self::with_cancel`], in place.
    pub fn set_cancel(&mut self, flag: Arc<std::sync::atomic::AtomicBool>) {
        self.cancel = Some(flag);
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

    /// The directories outside the world a deploy Kijin may write: a write outside the world
    /// anywhere else, or into a dotfile of the home, is no post's act.
    #[must_use]
    pub fn with_deploy_roots(mut self, roots: &[impl AsRef<Path>]) -> Self {
        self.deploy_roots = roots.iter().map(|r| real(r.as_ref())).collect();
        self
    }

    /// The directories outside the world a deploy Kijin may write.
    #[must_use]
    pub fn deploy_roots(&self) -> &[PathBuf] {
        &self.deploy_roots
    }

    /// The human approval gate the tools ask; the loop asks it too, for what the end gate leaves
    /// to the human.
    #[must_use]
    pub fn gate(&self) -> &dyn Gate {
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

    /// As [`Self::writable`], but the last component is the entry itself, never what a link
    /// there points to: what a delete removes, or a move renames.
    pub(crate) fn writable_entry(&self, text: &str) -> Result<PathBuf, String> {
        let lexical =
            join(Some(&self.world), text).ok_or_else(|| format!("`{text}` is not a path"))?;
        self.writable_at(&lexical)
    }

    /// As [`Self::writable_entry`], for a path already in hand (a walk's entry): never through a
    /// lossy string.
    pub(crate) fn writable_at(&self, lexical: &Path) -> Result<PathBuf, String> {
        let text = lexical.display();
        let (Some(dir), Some(name)) = (lexical.parent(), lexical.file_name()) else {
            return Err(format!("`{text}` names no entry"));
        };
        let path = real(dir).join(name);
        if !path.starts_with(&self.world) || path == self.world {
            return Err(format!(
                "`{text}` is outside the world ({}); tools stay inside it",
                path.display()
            ));
        }
        self.in_soil(path)
    }

    /// Whether another's spot lies strictly inside `dir`: such a folder is never moved whole.
    pub(crate) fn holds_spot(&self, dir: &Path) -> bool {
        self.except.iter().any(|e| e.starts_with(dir) && e != dir)
    }

    /// As [`Self::inside`], and the path must lie in the body's soil.
    pub(crate) fn writable(&self, text: &str) -> Result<PathBuf, String> {
        let path = self.inside(text)?;
        self.in_soil(path)
    }

    /// The soil's judgement on a resolved path: git's files and the records are never a
    /// tool's, and the rest must be admitted.
    fn in_soil(&self, path: PathBuf) -> Result<PathBuf, String> {
        // One record is a body's to write: the plan in its own seat, which the dispatch law reads.
        // Only `plan.md`, directly inside the seat the body was given.
        let own_plan = path.file_name().is_some_and(|f| f == "plan.md")
            && self.seat.is_some()
            && path.parent() == self.seat.as_deref();
        if path.starts_with(self.world.join(".mnemorph").join("minds")) {
            return Err(format!(
                "`{}` is a Mind's: Minds are the human's, adopted and retired with `mnemorphd \
                 mind`, and a doc changed since adoption no longer loads; propose the change in \
                 your report",
                self.shown(&path)
            ));
        }
        if crate::tripwire::protects(&self.world, &path) && !own_plan {
            return Err(format!(
                "`{}` is protected: git's executable configuration and the world's records are not written by tools",
                self.shown(&path)
            ));
        }
        if path.starts_with(self.world.join(".git")) {
            return Err(format!(
                "`{}` is git's: landing is the harness's, never a tool's",
                self.shown(&path)
            ));
        }
        if self.admits(&path) {
            Ok(path)
        } else {
            let soil: Vec<String> = self.soil.iter().map(|s| self.shown(s)).collect();
            let shown = self.shown(&path);
            Err(format!(
                "`{shown}` is outside your soil; you may write under: {}{}",
                soil.join(", "),
                ways_on(self.owner_of.as_ref(), &shown)
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
        return Outcome::unheld(refusal);
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

/// What git ignores in `world`, relative to it: the candidates for build output. The world
/// decides which of them a creature may write ([`Context::with_build`]).
#[must_use]
pub fn ignored(world: &Path) -> Vec<PathBuf> {
    // Outside the sandbox: a clone's own configuration must not run anything (fsmonitor, hooks).
    let out = mnemorph_sandbox::git()
        .args(["-c", "core.fsmonitor=", "-c", "core.hooksPath=/dev/null"])
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
        .map(|p| PathBuf::from(p.trim_end_matches('/')))
        .collect()
}

/// Whether `rel` reaches through a link anywhere between the world and itself.
fn linked(world: &Path, rel: &Path) -> bool {
    let mut at = world.to_path_buf();
    rel.components().any(|c| {
        at.push(c);
        at.symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
    })
}

/// A fresh directory only this creature uses, outside the world: owner-only, never a name another
/// already holds.
fn private_scratch() -> Option<PathBuf> {
    use std::os::unix::fs::DirBuilderExt as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for _ in 0..8 {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "mnemorph-scratch-{}-{stamp:x}-{n}",
            std::process::id()
        ));
        if std::fs::DirBuilder::new().mode(0o700).create(&dir).is_ok() {
            return Some(dir);
        }
    }
    None
}

/// The ways on from a write outside the soil, when the context knows them.
fn ways_on(owner_of: Option<&OwnerOf>, shown: &str) -> String {
    owner_of
        .map(|f| f(shown))
        .filter(|w| !w.is_empty())
        .map_or_else(String::new, |w| format!(". {w}"))
}

/// A body's grant: what its commands and its file tools may write, under the guard it ran with.
#[derive(Debug, Clone)]
pub struct Grant {
    /// A commissioned body under a kernel guard: only then can writers be told apart.
    confined: bool,
    world: PathBuf,
    soil: Vec<PathBuf>,
    except: Vec<PathBuf>,
    root_files: bool,
    seat: Option<PathBuf>,
    build: Vec<PathBuf>,
    nest: Option<PathBuf>,
}

impl Grant {
    /// Whether the body could have written `path` (absolute). Confined, only its soil, its own
    /// plan, its build output and its scratch; otherwise anything, since writers cannot be told
    /// apart.
    #[must_use]
    pub fn could_have_written(&self, path: &Path) -> bool {
        if !self.confined {
            return true;
        }
        // The grant's paths are resolved; so is the one asked about (macOS's `/var` is a link).
        let path = &real(path);
        let admits = admits(&self.world, &self.soil, &self.except, self.root_files, path);
        let own_plan = path.file_name().is_some_and(|f| f == "plan.md")
            && self.seat.is_some()
            && path.parent() == self.seat.as_deref();
        admits
            || own_plan
            || self.build.iter().any(|b| path.starts_with(b))
            || self.nest.as_ref().is_some_and(|n| path.starts_with(n))
    }
}

/// Whether a soil admits `path`: the longest covering claim decides, a tie to the exclusion; a
/// root file by `root_files`, unless a claim excludes it.
fn admits(
    world: &Path,
    soil: &[PathBuf],
    except: &[PathBuf],
    root_files: bool,
    path: &Path,
) -> bool {
    let best = |claims: &[PathBuf]| {
        claims
            .iter()
            .filter(|c| path.starts_with(c))
            .map(|c| c.components().count())
            .max()
    };
    let root = (root_files && path.parent() == Some(world)).then_some(0);
    match (best(soil).max(root), best(except)) {
        (Some(w), Some(e)) => w > e,
        (Some(_), None) => true,
        (None, _) => false,
    }
}
