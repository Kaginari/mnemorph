//! CI as an instrument: `kijin-ci`'s standing read. The forge's latest runs are read through the
//! harness's read-only probe and appended to `.mnemorph/instruments/ci.jsonl` — a row for a run
//! not seen, or one whose state moved — so every creature checks the real CI state, not a doc's
//! word for it. Reading runs nothing and changes nothing on the forge. The instruments are the
//! world's and a body may write them, so what is read back is said as last read, and grants
//! nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemorph_ledger::Journal;
use serde::{Deserialize, Serialize};

use crate::Probe;
use crate::forge::probed_within;

/// The instrument, relative to the world.
pub const INSTRUMENT: &str = ".mnemorph/instruments/ci.jsonl";

/// How many of the forge's newest runs one read takes.
const RUNS: &str = "30";

/// The fields read of each run.
const FIELDS: &str = "databaseId,headBranch,headSha,workflowName,status,conclusion,url,createdAt";

/// One CI run as the forge said it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiRun {
    /// The forge's id of the run.
    pub database_id: u64,
    /// The branch it ran on.
    pub head_branch: String,
    /// The commit it ran on.
    pub head_sha: String,
    /// The workflow.
    pub workflow_name: String,
    /// `queued`, `in_progress` or `completed`.
    pub status: String,
    /// `success`, `failure`, `cancelled`, …; empty until it completes.
    #[serde(default)]
    pub conclusion: String,
    /// Where to read it.
    pub url: String,
    /// When it was created, as the forge wrote it.
    pub created_at: String,
}

impl CiRun {
    fn word(&self) -> &str {
        if self.conclusion.is_empty() {
            &self.status
        } else {
            &self.conclusion
        }
    }

    fn state(&self) -> (u64, String, String) {
        (
            self.database_id,
            self.status.clone(),
            self.conclusion.clone(),
        )
    }
}

fn journal(world: &Path) -> Journal<CiRun> {
    Journal::new(world.join(INSTRUMENT))
}

/// `owner/name` from a remote's URL (https, ssh or scp form); `None` when it names no such pair.
#[must_use]
pub fn repo_of(url: &str) -> Option<String> {
    let path = match url.split_once("://") {
        Some((_, rest)) => rest.split_once('/')?.1,
        None => url.split_once(':')?.1,
    };
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let (owner, name) = path.split_once('/')?;
    let plain = |s: &str| {
        !s.is_empty()
            && !s.starts_with('-')
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (plain(owner) && plain(name)).then(|| format!("{owner}/{name}"))
}

/// The forge's newest runs of `repo` read through `probe`, each new one or one whose state moved
/// appended to the instrument; how many were.
///
/// # Errors
///
/// The forge cannot be read, its answer does not parse, or the instrument cannot be written:
/// nothing is written then.
pub fn refresh(world: &Path, repo: &str, probe: &dyn Probe) -> Result<usize, String> {
    let argv: Vec<String> = [
        "gh", "run", "list", "--repo", repo, "--limit", RUNS, "--json", FIELDS,
    ]
    .map(str::to_owned)
    .to_vec();
    let (mut probes, mut seen) = (Vec::new(), Vec::new());
    let out = probed_within(probe, &argv, crate::PROBE_TIMEOUT, &mut probes, &mut seen)?;
    let mut runs: Vec<CiRun> = serde_json::from_slice(&out)
        .map_err(|e| format!("the forge's runs cannot be read: {e}"))?;
    // Oldest first, so the instrument reads in the order things happened.
    runs.reverse();
    let j = journal(world);
    let known: BTreeSet<(u64, String, String)> = j
        .read()
        .map_err(|e| format!("{INSTRUMENT}: {e}"))?
        .rows
        .iter()
        .map(CiRun::state)
        .collect();
    let new: Vec<&CiRun> = runs
        .iter()
        .filter(|r| !known.contains(&r.state()))
        .collect();
    if let Some(dir) = world.join(INSTRUMENT).parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    for r in &new {
        j.append(*r).map_err(|e| format!("{INSTRUMENT}: {e}"))?;
    }
    Ok(new.len())
}

/// One line for the newest run of each branch and workflow, as last read: failures first, then
/// the newest. Nothing when the instrument holds none; the forge is not reached.
#[must_use]
pub fn lines(world: &Path) -> Vec<String> {
    let rows = journal(world).read().map(|r| r.rows).unwrap_or_default();
    let mut newest: BTreeMap<(String, String), CiRun> = BTreeMap::new();
    for r in rows {
        let key = (r.head_branch.clone(), r.workflow_name.clone());
        if newest
            .get(&key)
            .is_none_or(|n| r.database_id >= n.database_id)
        {
            newest.insert(key, r);
        }
    }
    let mut runs: Vec<CiRun> = newest.into_values().collect();
    runs.sort_by(|a, b| {
        (a.word() != "failure")
            .cmp(&(b.word() != "failure"))
            .then_with(|| b.database_id.cmp(&a.database_id))
    });
    runs.iter()
        .take(8)
        .map(|r| {
            format!(
                "{} {}: {} at {} ({}), as last read",
                r.head_branch,
                r.workflow_name,
                r.word(),
                r.head_sha.chars().take(7).collect::<String>(),
                r.created_at
            )
        })
        .collect()
}

/// [`refresh`] through the harness's own probe, of `repo` or the repository `world`'s `origin`
/// names: the repository read and how many rows were written.
///
/// # Errors
///
/// No repository can be named, or as [`refresh`].
pub fn refresh_now(world: &Path, repo: Option<&str>) -> Result<(String, usize), String> {
    let repo = match repo {
        Some(r) => repo_of(&format!("https://forge/{r}"))
            .ok_or_else(|| format!("`{r}` is no repository: owner/name"))?,
        None => origin(world).ok_or(
            "no repository to read: the world has no `origin` on a forge; name one with --repo",
        )?,
    };
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/".into());
    let probe = crate::Observer::new(mnemorph_tools::Sandbox::detect(), home);
    let wrote = refresh(world, &repo, &probe)?;
    Ok((repo, wrote))
}

/// `owner/name` of `world`'s `origin`, read by the harness's own git.
fn origin(world: &Path) -> Option<String> {
    let out = mnemorph_tools::git()
        .arg("-C")
        .arg(world)
        .args(["remote", "get-url", "origin"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    repo_of(String::from_utf8_lossy(&out.stdout).trim())
}
