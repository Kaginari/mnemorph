//! A staged plan proven before it is swapped. The plan is written over a scratch copy of the
//! world — the overlay — never the world itself: there the ontology must find nothing the live
//! world does not, and every creature doc the plan writes must have its verify lines pinned by
//! the human and passing in the sandbox. A plan that fails any of it is refused whole, with every
//! reason; the live world is never touched.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use mnemorph_tools::{Context, Sandbox};
use mnemorph_world::evolution::{Plan, Staged};
use mnemorph_world::{Facts, Roster};

use crate::evolution::copy_world;
use crate::gate::EndGate;

/// How the proof runs verify lines: the sandbox, whether the human lets a command run with no
/// guard in force, and how long each line may run.
#[derive(Debug, Clone)]
pub struct Prover {
    /// The sandbox the verify lines run in.
    pub sandbox: Sandbox,
    /// The human's `sandbox.allow_unguarded`.
    pub unguarded: bool,
    /// `gate.verify_secs`.
    pub verify_secs: u64,
}

/// `plan` proven over an overlay of `world`, whose tracked files are `tracked`: one line per
/// thing proven, or every reason it is not.
///
/// # Errors
///
/// Every reason the plan is not proven: the overlay cannot be made, a path of it leaves the world
/// or crosses a link, the ontology finds what the live world does not, or a doc it writes has a
/// verify line no pin holds, that fails or gives no result.
pub fn prove(
    world: &Path,
    plan: &Plan,
    tracked: &[String],
    p: &Prover,
) -> Result<Vec<String>, Vec<String>> {
    let root =
        std::env::temp_dir().join(format!("mnemorph-proof-{}-{}", plan.id, std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let scratch = root.join(world.file_name().unwrap_or_else(|| "world".as_ref()));
    let proven = over(world, &scratch, plan, tracked, p);
    let _ = fs::remove_dir_all(&root);
    proven
}

/// [`prove`] with the world's own configuration: the sandbox this machine has, the human's
/// `sandbox.allow_unguarded` and `gate.verify_secs`.
///
/// # Errors
///
/// As [`prove`], or the configuration cannot be read.
pub fn prove_now(
    world: &Path,
    plan: &Plan,
    tracked: &[String],
) -> Result<Vec<String>, Vec<String>> {
    let config = mnemorph_config::Config::load(&mnemorph_config::Paths::resolve(world))
        .map_err(|e| vec![format!("the configuration: {e}")])?;
    let p = Prover {
        sandbox: Sandbox::detect(),
        unguarded: config.allow_unguarded(),
        verify_secs: config.verify_secs(),
    };
    prove(world, plan, tracked, &p)
}

fn over(
    world: &Path,
    scratch: &Path,
    plan: &Plan,
    tracked: &[String],
    p: &Prover,
) -> Result<Vec<String>, Vec<String>> {
    let one = |e: String| vec![e];
    copy_world(world, scratch, true).map_err(|e| one(format!("the overlay: {e}")))?;
    let scratch = scratch
        .canonicalize()
        .map_err(|e| one(format!("the overlay: {e}")))?;
    for f in &plan.files {
        put(&scratch, f).map_err(one)?;
    }
    let mut lines = Vec::new();
    let mut why = Vec::new();
    match (
        findings(world, world, tracked),
        findings(&scratch, world, tracked),
    ) {
        (Ok(live), Ok(overlay)) => {
            let new: Vec<&String> = overlay.difference(&live).collect();
            if new.is_empty() {
                lines.push(format!(
                    "the ontology over the overlay: no new finding ({} before, {} after)",
                    live.len(),
                    overlay.len()
                ));
            }
            why.extend(new.into_iter().map(|f| {
                format!(
                    "the ontology over the overlay: a new finding: {}",
                    f.replace(&scratch.display().to_string(), &world.display().to_string())
                )
            }));
        }
        (Err(e), _) | (_, Err(e)) => why.push(format!("the ontology cannot be read: {e}")),
    }
    let cards = Roster::cards(&scratch);
    for f in plan.files.iter().filter(|f| f.after.is_some()) {
        let Some(c) = cards.creatures.values().find(|c| c.doc() == f.path) else {
            continue;
        };
        let gate = EndGate::pinned(world, &c.name, &f.path, &c.verify, p.verify_secs, None);
        if gate.verify.is_empty() && gate.held.is_empty() {
            lines.push(format!(
                "{}'s doc names no verify line: none to run",
                c.name
            ));
            continue;
        }
        let mut cx = Context::new(c.race, &scratch, &[] as &[&str])
            .with_sandbox(p.sandbox.clone())
            .with_unguarded(p.unguarded);
        let failed = gate.run_alone(&mnemorph_tools::registry(), &mut cx);
        if failed.is_empty() {
            for line in &gate.verify {
                lines.push(format!(
                    "{}'s verify line `{line}` passed over the overlay",
                    c.name
                ));
            }
        }
        why.extend(failed.into_iter().map(|e| format!("{}: {e}", c.name)));
    }
    if why.is_empty() { Ok(lines) } else { Err(why) }
}

/// Every finding of `at`'s shape, its cards read against `trust`'s witness, and every doc that
/// does not read: each as one line, its `file:line` cut to the file, so a doc grown longer
/// above a finding does not make it new.
fn findings(at: &Path, trust: &Path, tracked: &[String]) -> Result<BTreeSet<String>, String> {
    let roster = Roster::shown(at, trust);
    let (found, _) = mnemorph_world::onto_check(at, &roster, &Facts::load(at), tracked, false)?;
    let shown = at.display().to_string();
    let mut out: BTreeSet<String> = found
        .iter()
        .map(|f| {
            let file =
                f.at.rsplit_once(':')
                    .filter(|(_, n)| n.chars().all(|c| c.is_ascii_digit()))
                    .map_or(f.at.as_str(), |(file, _)| file);
            format!("{} {} at {file}: {}", f.severity.word(), f.code, f.message)
        })
        .collect();
    out.extend(
        roster
            .problems
            .iter()
            .map(|p| format!("a doc that does not read: {}", p.replace(&shown, "<world>"))),
    );
    Ok(out)
}

/// `f` written into the overlay `scratch`: its path plain and relative, no link on the way to it
/// or at it — the copy keeps the world's links as links — a missing folder made.
fn put(scratch: &Path, f: &Staged) -> Result<(), String> {
    let rel = Path::new(&f.path);
    let parts: Vec<Component<'_>> = rel.components().collect();
    if parts.is_empty() || !parts.iter().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("{}: no plain path under the world", f.path));
    }
    let mut at = scratch.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        at.push(part);
        let last = i + 1 == parts.len();
        match fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "{}: a link on the plan's path; the proof follows none",
                    shown(scratch, &at)
                ));
            }
            Ok(m) if !last && !m.is_dir() => {
                return Err(format!(
                    "{}: a file where the plan's path needs a folder",
                    shown(scratch, &at)
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound && !last => {
                fs::create_dir(&at).map_err(|e| format!("{}: {e}", shown(scratch, &at)))?;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", shown(scratch, &at))),
        }
    }
    match fs::remove_file(&at) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{}: {e}", f.path)),
    }
    if let Some(bytes) = &f.after {
        use std::io::Write as _;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&at)
            .and_then(|mut file| file.write_all(bytes))
            .map_err(|e| format!("{}: {e}", f.path))?;
    }
    Ok(())
}

fn shown(scratch: &Path, at: &Path) -> String {
    at.strip_prefix(scratch)
        .map_or_else(|_| at.to_path_buf(), PathBuf::from)
        .display()
        .to_string()
}
