//! The evolution ledger's home and its writer: `$XDG_STATE_HOME/mnemorph/evolution/<world>-<fnv64a>.jsonl`,
//! outside the world and out of every command's reach, written by the harness alone, chained by
//! hash and anchored with the world's other chained journals ([`crate::chains`]). An evolution is
//! judged here from that ledger, held to its newest anchored head
//! ([`mnemorph_ledger::evolution`]).
//!
//! - **The swap** ([`swap`]). A plan the human accepted — a Mind or a creature doc rewritten,
//!   added or removed — is written into the world here and only here: the soil under
//!   `.mnemorph/` is hashed before and after ([`soil`]), the named files' bytes before are kept
//!   beside the ledger for the controls ([`restore_before`]), and one `swap` row names every file
//!   with its digest before and after and the build. A plan whose diff is not exactly its named
//!   files is put back and writes no row.
//! - **The runs** ([`stamp_for`], [`Stamp::record`]). Each ask of a body a swap names, while its
//!   window is open, is stamped with the swap's id, the build, the task's fingerprint (the SHA-256
//!   of the ask) and the named files' state as the run found and left them, and appended as a
//!   `run` row when it ends. A task the ledger already holds is never stamped again: it could not
//!   be held out.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use mnemorph_ledger::evolution::{self, EvolutionRow, FileChange, Ledger, Run};
/// The arm, the gate's word, the swap row and the judgement, as the writer's callers name them.
pub use mnemorph_ledger::evolution::{Arm, Gate, Judgement, Swap};
use mnemorph_ledger::{Build, Journal};

/// Where a world's evolution ledger is kept:
/// `$XDG_STATE_HOME/mnemorph/evolution/<world>-<fnv64a>.jsonl`, else under `~/.local/state`;
/// `None` when there is no home.
#[must_use]
pub fn ledger_of(world: &Path) -> Option<PathBuf> {
    crate::queue::state_file(world, "evolution")
}

/// The world's evolution ledger, chained, for the harness to append swaps and runs to; `None`
/// when there is no home.
#[must_use]
pub fn journal(world: &Path) -> Option<Journal<EvolutionRow>> {
    ledger_of(world).map(Journal::chained)
}

/// The chained evolution ledger at `path`: what [`swap_at`] writes to when the ledger is not the
/// world's own.
#[must_use]
pub fn journal_at(path: &Path) -> Journal<EvolutionRow> {
    Journal::chained(path)
}

/// Judges the swap `id` of `world` from its ledger, held to the newest head anchored for it.
///
/// # Errors
///
/// There is no home to keep the ledger under, or it cannot be read.
pub fn judge(world: &Path, id: &str, min_net_wins: u32) -> io::Result<Judgement> {
    let path =
        ledger_of(world).ok_or_else(|| io::Error::other("no home to keep the ledger under"))?;
    let anchor = crate::chains::anchor_of(world, "evolution");
    Ok(evolution::judge(
        &evolution::read(&path, anchor.as_ref())?,
        id,
        min_net_wins,
    ))
}

/// The parts of `.mnemorph/` a swap never touches and the soil hash leaves out: what runs write
/// while they work.
const RESTLESS: [&str; 3] = ["instruments", "memory", "tmp"];

/// The soil a swap is checked over: every file under the world's `.mnemorph/` — the creature docs
/// and the Minds — but the instruments, memory and scratch, by world path, with its SHA-256. A
/// link is hashed as its target's name, never followed.
///
/// # Errors
///
/// A file under it cannot be read.
pub fn soil(world: &Path) -> io::Result<BTreeMap<String, String>> {
    let records = world.join(".mnemorph");
    let mut out = BTreeMap::new();
    let entries = match fs::read_dir(&records) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };
    for e in entries {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if RESTLESS.contains(&name.as_str()) {
            continue;
        }
        let kind = e.file_type()?;
        if kind.is_dir() {
            for (p, d) in evolution::snapshot(&e.path())? {
                out.insert(format!(".mnemorph/{name}/{p}"), d);
            }
        } else if kind.is_symlink() {
            let to = fs::read_link(e.path())?;
            let mut bytes = b"link:".to_vec();
            bytes.extend_from_slice(to.to_string_lossy().as_bytes());
            out.insert(format!(".mnemorph/{name}"), evolution::digest(&bytes));
        } else {
            out.insert(
                format!(".mnemorph/{name}"),
                evolution::digest(&fs::read(e.path())?),
            );
        }
    }
    Ok(out)
}

/// One file of a plan: its world path and its bytes after the swap; `None` removes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    /// The path, relative to the world, `/`-separated, under `.mnemorph/`.
    pub path: String,
    /// Its bytes after the swap; `None` when the swap removes it.
    pub after: Option<Vec<u8>>,
}

/// An evolution the human accepted, to be swapped in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The swap's id: letters, digits, `.`, `_` and `-`.
    pub id: String,
    /// The creature that evolves.
    pub creature: String,
    /// The bodies whose runs judge it: the creature, or a split's young.
    pub bodies: Vec<String>,
    /// The arm.
    pub arm: Arm,
    /// Every file it changes.
    pub files: Vec<Staged>,
    /// The fingerprints of the tasks it was proven on.
    pub proven_on: Vec<String>,
}

/// The id a swap may carry, or why not.
fn valid_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && !id.starts_with('.')
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "`{id}` is no swap id: letters, digits, `.`, `_` and `-`, at most 64, not led by `.`"
        ))
    }
}

/// A plan's path, or why it is none: relative, plain, under `.mnemorph/`, and in the soil.
fn valid_path(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    let plain = p.components().all(|c| matches!(c, Component::Normal(_)));
    let under = path
        .strip_prefix(".mnemorph/")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|top| !top.is_empty() && !RESTLESS.contains(&top));
    if plain && under && !path.contains('\\') && !path.ends_with('/') {
        Ok(())
    } else {
        Err(format!(
            "{path}: a swap writes creature docs and Minds under `.mnemorph/`, never its \
             instruments, memory or scratch"
        ))
    }
}

/// Where the named files' bytes before swap `id` are kept, beside the ledger at `ledger`.
#[must_use]
pub fn kept_before(ledger: &Path, id: &str) -> PathBuf {
    let stem = ledger
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    ledger
        .with_file_name(format!("{stem}.swaps"))
        .join(id)
        .join("before")
}

/// Puts `swap`'s named files under `root` back as they were before it, from the bytes kept beside
/// the ledger at `ledger`: a file the swap created is removed.
///
/// # Errors
///
/// A kept file cannot be read, or a named file cannot be written or removed.
pub fn restore_before(ledger: &Path, swap: &Swap, root: &Path) -> io::Result<()> {
    let kept = kept_before(ledger, &swap.id);
    for f in &swap.files {
        let at = root.join(&f.path);
        if f.before.is_some() {
            let bytes = fs::read(kept.join(&f.path))?;
            if let Some(dir) = at.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&at, bytes)?;
        } else {
            match fs::remove_file(&at) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

/// Swaps `plan` into `world` and writes its `swap` row to the world's evolution ledger.
///
/// # Errors
///
/// What [`swap_at`] refuses on, or there is no home to keep the ledger under.
pub fn swap(world: &Path, plan: &Plan) -> Result<Swap, String> {
    let ledger = journal(world).ok_or("no home to keep the evolution ledger under")?;
    swap_at(world, &ledger, plan)
}

/// [`swap`], on the ledger given. The plan is refused when its id is taken or malformed, a path
/// leaves the soil or is named twice, a body it names is still in another swap's window, or the
/// soil's diff is not exactly its named files — then the world is put back and no row is written.
///
/// # Errors
///
/// Why the plan is refused, or the soil, the kept bytes or the ledger cannot be read or written.
pub fn swap_at(world: &Path, ledger: &Journal<EvolutionRow>, plan: &Plan) -> Result<Swap, String> {
    valid_id(&plan.id)?;
    if plan.files.is_empty() {
        return Err("the plan names no file".into());
    }
    if plan.bodies.is_empty() {
        return Err("the plan names no body to judge it".into());
    }
    let mut named = std::collections::BTreeSet::new();
    for f in &plan.files {
        valid_path(&f.path)?;
        if !named.insert(f.path.as_str()) {
            return Err(format!("{}: named twice", f.path));
        }
    }
    let read = read_ledger(ledger.path())?;
    if read
        .entries
        .iter()
        .any(|e| matches!(&e.row, EvolutionRow::Swap(s) if s.id == plan.id))
    {
        return Err(format!("the ledger already holds a swap `{}`", plan.id));
    }
    if let Some((b, s)) = plan
        .bodies
        .iter()
        .find_map(|b| evolution::open_for(&read, b).map(|s| (b, s)))
    {
        return Err(format!(
            "{b} is still judged on swap `{}`: one evolution at a time",
            s.id
        ));
    }
    let before = soil(world).map_err(|e| format!("the soil cannot be read: {e}"))?;
    let kept = kept_before(ledger.path(), &plan.id);
    if kept.exists() {
        return Err(format!("{} already exists", kept.display()));
    }
    for f in &plan.files {
        if before.contains_key(&f.path) {
            let to = kept.join(&f.path);
            fs::create_dir_all(to.parent().unwrap_or(&kept))
                .and_then(|()| fs::copy(world.join(&f.path), &to).map(|_| ()))
                .map_err(|e| format!("{}: its bytes before cannot be kept: {e}", f.path))?;
        }
    }
    let files: Vec<FileChange> = plan
        .files
        .iter()
        .map(|f| FileChange {
            path: f.path.clone(),
            before: before.get(&f.path).cloned(),
            after: f.after.as_deref().map(evolution::digest),
        })
        .collect();
    let swap = Swap {
        id: plan.id.clone(),
        creature: plan.creature.clone(),
        bodies: plan.bodies.clone(),
        arm: plan.arm,
        build: Build::this(),
        files,
        diff: Vec::new(),
        proven_on: plan.proven_on.clone(),
    };
    let wrote = write_plan(world, plan);
    let after = soil(world);
    let checked = match (wrote, after) {
        (Ok(()), Ok(after)) => {
            let swap = Swap {
                diff: evolution::diff(&before, &after),
                ..swap
            };
            swap.check().map(|()| swap).map_err(|r| r.to_string())
        }
        (Err(e), _) | (_, Err(e)) => Err(format!("the plan could not be written: {e}")),
    };
    let put_back = |why: String| {
        let _ = restore_before(ledger.path(), &swap_named(plan, &before), world);
        let _ = fs::remove_dir_all(kept.parent().unwrap_or(&kept));
        Err(format!("refused, and the world put back: {why}"))
    };
    let swap = match checked {
        Ok(s) => s,
        Err(why) => return put_back(why),
    };
    if let Err(e) = ledger.append(&EvolutionRow::Swap(swap.clone())) {
        return put_back(format!("the ledger cannot be written: {e}"));
    }
    Ok(swap)
}

/// The plan's files as a swap names them, for putting the world back.
fn swap_named(plan: &Plan, before: &BTreeMap<String, String>) -> Swap {
    Swap {
        id: plan.id.clone(),
        creature: plan.creature.clone(),
        bodies: plan.bodies.clone(),
        arm: plan.arm,
        build: Build::this(),
        files: plan
            .files
            .iter()
            .map(|f| FileChange {
                path: f.path.clone(),
                before: before.get(&f.path).cloned(),
                after: None,
            })
            .collect(),
        diff: Vec::new(),
        proven_on: Vec::new(),
    }
}

fn write_plan(world: &Path, plan: &Plan) -> io::Result<()> {
    for f in &plan.files {
        let at = world.join(&f.path);
        match &f.after {
            Some(bytes) => {
                if let Some(dir) = at.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::write(&at, bytes)?;
            }
            None => match fs::remove_file(&at) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            },
        }
    }
    Ok(())
}

/// The ledger at `path`, read without its anchor: what the writer reads to stamp and to refuse.
fn read_ledger(path: &Path) -> Result<Ledger, String> {
    evolution::read(path, None).map_err(|e| format!("the evolution ledger cannot be read: {e}"))
}

/// A task's fingerprint: the SHA-256 of the ask, trimmed.
#[must_use]
pub fn fingerprint(ask: &str) -> String {
    evolution::digest(ask.trim().as_bytes())
}

/// A run in a swap's window, stamped when it starts and written when it ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    /// The ledger it is written to.
    pub ledger: PathBuf,
    /// The swap whose window it is in.
    pub swap: Swap,
    /// The body that runs it.
    pub body: String,
    /// The ask.
    pub ask: String,
    /// The named files' state as the run found them.
    pub state: String,
    /// The do-nothing control of a window run.
    pub control: bool,
}

/// What the harness saw of a run: the gate's last word, a first-try pass, the holes it found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Facts {
    /// The gate's word on the landing; `None` when no gate judged one.
    pub gate: Option<Gate>,
    /// The gate passed on its first try.
    pub first_try: bool,
    /// Holes the harness found.
    pub holes_found: u32,
}

/// The stamp for an ask of `body` in `world`, over the ledger at `ledger`: `None` when no swap's
/// window is open for the body, the ask's task is already on the ledger, or the ledger cannot be
/// read — then the run is no evolution's to judge.
#[must_use]
pub fn stamp_for(world: &Path, ledger: &Path, body: &str, ask: &str) -> Option<Stamp> {
    let read = read_ledger(ledger).ok()?;
    let swap = evolution::open_for(&read, body)?;
    if evolution::seen(&read, &fingerprint(ask)) {
        return None;
    }
    let state = evolution::state_now(world, swap).ok()?;
    Some(Stamp {
        ledger: ledger.to_path_buf(),
        swap: swap.clone(),
        body: body.to_owned(),
        ask: ask.to_owned(),
        state,
        control: false,
    })
}

impl Stamp {
    /// Appends the run's row: its stamps, the state it left the named files in under `world`, and
    /// the harness's facts.
    ///
    /// # Errors
    ///
    /// A named file cannot be read, or the ledger cannot be written.
    pub fn record(&self, world: &Path, facts: Facts) -> io::Result<()> {
        let state_after = evolution::state_now(world, &self.swap)?;
        Journal::<EvolutionRow>::chained(&self.ledger).append(&EvolutionRow::Run(Run {
            swap: Some(self.swap.id.clone()),
            build: Build::this(),
            body: self.body.clone(),
            task: fingerprint(&self.ask),
            control: self.control,
            state: self.state.clone(),
            state_after,
            gate: facts.gate,
            first_try: facts.first_try,
            holes_found: facts.holes_found,
            ask: Some(self.ask.clone()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ledger_lives_outside_the_world_and_is_anchored_with_the_others() {
        let world = Path::new("/srv/worlds/jura");
        let Some(at) = ledger_of(world) else {
            return;
        };
        assert!(!at.starts_with(world), "{}", at.display());
        assert!(at.to_string_lossy().contains("mnemorph/evolution/jura-"));
        assert!(
            crate::chains::journals(world)
                .iter()
                .any(|j| j.name == "evolution" && j.path == at)
        );
        assert!(journal(world).is_some_and(|j| j.is_chained()));
    }
}
