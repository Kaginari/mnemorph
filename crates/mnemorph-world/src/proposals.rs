//! Evolution proposals: the plans drawn and staged for the human, each kept by the harness
//! outside the world — `$XDG_STATE_HOME/mnemorph/proposals/<world>-<fnv64a>.jsonl`, chained by hash
//! and anchored with the world's other chained journals ([`crate::chains`]). A proposal is the
//! plan's file set with each file's digest as the world held it when drawn and as it was staged;
//! the staging is the world's, and a body could rewrite it, so the digests here are what the
//! human applies. A proposal the world moved past since it was drawn, or whose staging is not
//! the bytes drawn, is refused. Applying one swaps it in ([`crate::evolution::swap`]); rejecting
//! one closes it. Both are the human's.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_ledger::chain;
use mnemorph_ledger::evolution::{self as ledger, Arm, FileChange};
use mnemorph_ledger::{Build, Journal, Timestamp};
use serde::{Deserialize, Serialize};

use crate::evolution::{FileSet, Staged, soil, staged, staging};

/// The state directory's kind for the proposals: `proposals/`.
pub const KIND: &str = "proposals";

/// One row of the proposals journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "row", rename_all = "camelCase")]
pub enum Row {
    /// A plan drawn and staged.
    Drawn {
        /// When.
        ts: Timestamp,
        /// The plan's id: its staging folder's name.
        id: String,
        /// The creature that evolves.
        creature: String,
        /// The arm.
        arm: Arm,
        /// Every file it changes, its digest as the world held it and as staged.
        files: Vec<FileChange>,
        /// The build that drew it.
        #[serde(default)]
        build: Build,
    },
    /// A proposal closed: `applied` or `rejected`.
    Closed {
        /// When.
        ts: Timestamp,
        /// The proposal's id.
        id: String,
        /// `applied` or `rejected`.
        how: String,
        /// The build that closed it.
        #[serde(default)]
        build: Build,
    },
}

/// A proposal still open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Its id.
    pub id: String,
    /// The creature that evolves.
    pub creature: String,
    /// The arm.
    pub arm: Arm,
    /// Its files, as drawn.
    pub files: Vec<FileChange>,
}

impl Proposal {
    /// The file set the swap reads its staging against.
    #[must_use]
    pub fn file_set(&self) -> FileSet {
        let mut set = FileSet::default();
        for f in &self.files {
            match (&f.before, &f.after) {
                (None, _) => set.creates.push(f.path.clone()),
                (Some(_), None) => set.removes.push(f.path.clone()),
                (Some(_), Some(_)) => set.replaces.push(f.path.clone()),
            }
        }
        set
    }
}

/// Where `world`'s proposals are kept under the state directory `state`.
#[must_use]
pub fn path_in(state: &Path, world: &Path) -> PathBuf {
    crate::queue::state_file_in(state, world, KIND)
}

fn state_of(state: Option<&Path>) -> Result<PathBuf, String> {
    state
        .map(Path::to_path_buf)
        .or_else(crate::queue::state_dir)
        .ok_or_else(|| {
            "no home: the evolution proposals are kept under the human's state directory".to_owned()
        })
}

fn path_of(state: &Path, world: &Path) -> Result<PathBuf, String> {
    let at = path_in(state, world);
    crate::pins::outside_as(world, &at, "the evolution proposals are")?;
    Ok(at)
}

/// The rows of `world`'s proposals under `state`, the chain walked whole and held to the head
/// anchored for it.
fn rows(state: &Path, world: &Path) -> Result<Vec<Row>, String> {
    let path = path_of(state, world)?;
    let path = path.as_path();
    let anchor = crate::chains::anchor_under(state, world, KIND);
    let report = chain::verify_against(path, anchor.as_ref())
        .map_err(|e| format!("the evolution proposals cannot be read: {e}"))?;
    if !report.holds() {
        return Err(format!("the evolution proposals do not verify: {report}"));
    }
    let back = Journal::<Row>::chained(path)
        .read()
        .map_err(|e| format!("the evolution proposals cannot be read: {e}"))?;
    if let Some(line) = back.skipped.first() {
        return Err(format!(
            "the evolution proposals do not verify: line {line} does not read"
        ));
    }
    Ok(back.rows)
}

/// The plan `id`, staged as `files`, drawn for `creature` by `arm`: each file's digest as the
/// world holds it now and as staged, kept as a proposal.
///
/// # Errors
///
/// No home, the id is already a proposal, the soil cannot be read, or the row cannot be written.
pub fn draw(
    world: &Path,
    id: &str,
    creature: &str,
    arm: Arm,
    files: &[Staged],
) -> Result<Proposal, String> {
    draw_in(None, world, id, creature, arm, files)
}

/// [`draw`], the human's state directory at `state` (`None`: the one this process has).
///
/// # Errors
///
/// As [`draw`].
pub fn draw_in(
    state: Option<&Path>,
    world: &Path,
    id: &str,
    creature: &str,
    arm: Arm,
    files: &[Staged],
) -> Result<Proposal, String> {
    let state = state_of(state)?;
    let at = path_of(&state, world)?;
    if rows(&state, world)?
        .iter()
        .any(|r| matches!(r, Row::Drawn { id: i, .. } if i == id))
    {
        return Err(format!("`{id}` is already a proposal"));
    }
    let now = soil(world).map_err(|e| format!("the soil cannot be read: {e}"))?;
    let files: Vec<FileChange> = files
        .iter()
        .map(|f| FileChange {
            path: f.path.clone(),
            before: now.get(&f.path).cloned(),
            after: f.after.as_deref().map(ledger::digest),
        })
        .collect();
    Journal::chained(&at)
        .append(&Row::Drawn {
            ts: Timestamp::now(),
            id: id.to_owned(),
            creature: creature.to_owned(),
            arm,
            files: files.clone(),
            build: Build::this(),
        })
        .map_err(|e| format!("the proposal cannot be kept: {e}"))?;
    Ok(Proposal {
        id: id.to_owned(),
        creature: creature.to_owned(),
        arm,
        files,
    })
}

/// Every proposal still open, oldest first.
///
/// # Errors
///
/// No home, or the journal does not verify.
pub fn open(world: &Path) -> Result<Vec<Proposal>, String> {
    let state = state_of(None)?;
    let mut out: BTreeMap<String, (usize, Proposal)> = BTreeMap::new();
    for (n, r) in rows(&state, world)?.into_iter().enumerate() {
        match r {
            Row::Drawn {
                id,
                creature,
                arm,
                files,
                ..
            } => {
                out.insert(
                    id.clone(),
                    (
                        n,
                        Proposal {
                            id,
                            creature,
                            arm,
                            files,
                        },
                    ),
                );
            }
            Row::Closed { id, .. } => {
                out.remove(&id);
            }
        }
    }
    let mut v: Vec<(usize, Proposal)> = out.into_values().collect();
    v.sort_by_key(|(n, _)| *n);
    Ok(v.into_iter().map(|(_, p)| p).collect())
}

/// The open proposal `id`.
///
/// # Errors
///
/// As [`open`], or no open proposal is `id`.
pub fn get(world: &Path, id: &str) -> Result<Proposal, String> {
    open(world)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| {
            format!("no open proposal `{id}`: `mnemorphd evolution list` says which are")
        })
}

/// The proposal's files as staged, held to it: refused when the world moved past it since it
/// was drawn — a named file the world no longer holds as it did — or the staging is not the
/// bytes drawn.
///
/// # Errors
///
/// Why the proposal cannot be applied as it stands.
pub fn ready(world: &Path, p: &Proposal) -> Result<Vec<Staged>, String> {
    let now = soil(world).map_err(|e| format!("the soil cannot be read: {e}"))?;
    for f in &p.files {
        if now.get(&f.path) != f.before.as_ref() {
            return Err(format!(
                "{}: the world moved since `{}` was drawn — draft it again",
                f.path, p.id
            ));
        }
    }
    let files = staged(world, &p.id, &p.file_set())?;
    for s in &files {
        let drawn = p
            .files
            .iter()
            .find(|f| f.path == s.path)
            .and_then(|f| f.after.as_ref());
        if s.after.as_deref().map(ledger::digest).as_ref() != drawn {
            return Err(format!(
                "{}: the staging is not the bytes `{}` was drawn with",
                s.path, p.id
            ));
        }
    }
    Ok(files)
}

/// The proposal `id` closed `how` — `applied` or `rejected`; a rejected one's staging removed,
/// never followed through a link.
///
/// # Errors
///
/// As [`get`], or the row cannot be written.
pub fn close(world: &Path, id: &str, how: &str) -> Result<(), String> {
    let at = path_of(&state_of(None)?, world)?;
    get(world, id)?;
    Journal::chained(&at)
        .append(&Row::Closed {
            ts: Timestamp::now(),
            id: id.to_owned(),
            how: how.to_owned(),
            build: Build::this(),
        })
        .map_err(|e| format!("the proposal cannot be closed: {e}"))?;
    if how == "rejected" {
        let dir = staging(world, id);
        if fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
            let _ = fs::remove_dir_all(&dir);
        }
    }
    Ok(())
}

/// One line per open proposal: its id, creature, arm and files, and whether it applies as it
/// stands.
#[must_use]
pub fn lines(world: &Path, open: &[Proposal]) -> Vec<String> {
    open.iter()
        .map(|p| {
            let state = match ready(world, p) {
                Ok(_) => {
                    "ready: `mnemorphd evolution show` it, then `apply` or `reject`".to_owned()
                }
                Err(e) => e,
            };
            format!(
                "{} — {} ({}), {} file(s): {state}",
                p.id,
                p.creature,
                crate::evolve::arm_word(p.arm),
                p.files.len()
            )
        })
        .collect()
}

/// What applying `p` would change, file by file: each line it takes away `- `, each it brings
/// `+ ` — the staging read as [`ready`] reads it, so a proposal that would be refused shows why.
///
/// # Errors
///
/// As [`ready`].
pub fn show(world: &Path, p: &Proposal) -> Result<Vec<String>, String> {
    let files = ready(world, p)?;
    let mut out = vec![format!(
        "{} — {} ({}), drawn against the world as it stands",
        p.id,
        p.creature,
        crate::evolve::arm_word(p.arm)
    )];
    for f in &files {
        let before = if f.creates {
            String::new()
        } else {
            crate::regular::text(&world.join(&f.path)).unwrap_or_default()
        };
        let after = f
            .after
            .as_deref()
            .map(|b| String::from_utf8_lossy(b).into_owned());
        let verb = match (&after, f.creates) {
            (None, _) => "removed",
            (Some(_), true) => "created",
            (Some(_), false) => "replaced",
        };
        out.push(format!("{} — {verb}", f.path));
        out.extend(diff(&before, after.as_deref().unwrap_or("")));
    }
    Ok(out)
}

/// The lines `after` takes away from `before` and brings, in order: a longest common
/// subsequence, the lines both hold left unsaid.
fn diff(before: &str, after: &str) -> Vec<String> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push(format!("  + {}", b[j]));
            j += 1;
        } else {
            out.push(format!("  - {}", a[i]));
            i += 1;
        }
    }
    out
}
