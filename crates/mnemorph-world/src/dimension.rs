//! Relations between worlds: the registry of the worlds on this machine, the sightings the
//! harness records when a body reads a manifest naming another world, the named-twice proposal
//! with its synonym check and its expiry, the relations only the human's yes states, and the
//! gluing of two worlds' relations on the pair they share.
//!
//! The relations a world states are kept outside it, beside its sightings, at
//! `$XDG_STATE_HOME/mnemorph/dimension/<world-id>/relations.jsonl`: chained by hash, anchored as
//! `relations` with the world's other chained journals ([`crate::chains`]), written only by the
//! human's `mnemorphd onto relate`, `unrelate` and `inverse`. The world's `.mnemorph/dimension.toml`
//! is a copy written after each, for readers: what it states beyond them is a proposal, flagged
//! `untrusted-relation`, never a relation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use mnemorph_ledger::chain;
use mnemorph_ledger::{Build, Journal, Timestamp};
use mnemorph_wire::fnv64a;
use serde::{Deserialize, Serialize};

use crate::findings::{Finding, Severity};

/// The dimension manifest: the relations this world states, the throne's soil.
pub const MANIFEST: &str = ".mnemorph/dimension.toml";

/// The largest manifest read, this world's or a neighbour's.
const DIMENSION_CAP: u64 = 64 * 1024;

/// The longest remote kept: a longer one is no remote.
const REMOTE_CAP: usize = 512;

/// Sessions of this world a learned relation may go unseen before its removal is proposed.
pub const STALE_AFTER: u64 = 30;

/// The containment at which a new type is offered as an existing one first.
pub const SYNONYM: f64 = 0.8;

/// The pairs a new type must name before the synonym check runs.
pub const SYNONYM_PAIRS: usize = 3;

/// The manifests a read is looked through for other worlds, by file name.
pub const MANIFESTS: &[&str] = &["Cargo.toml", "package.json", "go.mod", ".gitmodules"];

/// The largest manifest looked through: a bigger one is no manifest a body reads whole.
const MANIFEST_CAP: u64 = 1 << 20;

/// Types whose inverse is not `<type>-by`.
const INVERSES: &[(&str, &str)] = &[
    ("deploys", "deployed-by"),
    ("depends-on", "depended-on-by"),
    ("vendors", "vendored-by"),
];

// ---------------------------------------------------------------------------------------------
// The registry

/// One row of the registry: a session opened in a world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opened {
    /// The world's canonical path.
    pub world: String,
    /// Its id across the state directory: `<name>-<fnv64a of the path>`.
    pub id: String,
    /// Its `origin` remote, normalised (`host/owner/repo`, no credentials), if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// When, RFC 3339 in UTC.
    pub at: String,
}

/// A world on this machine, folded from the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    /// Its canonical path.
    pub world: PathBuf,
    /// Its id.
    pub id: String,
    /// Its latest remote, normalised.
    pub remote: Option<String>,
    /// When a session first opened in it.
    pub first: String,
    /// When the latest did.
    pub last: String,
    /// How many sessions opened in it: the clock a relation's expiry is read on.
    pub sessions: u64,
}

/// Where the registry lives: `$XDG_STATE_HOME/mnemorph/worlds.jsonl`, else under
/// `~/.local/state`; `None` with no home.
#[must_use]
pub fn registry() -> Option<PathBuf> {
    crate::queue::state_dir().map(|s| s.join("mnemorph/worlds.jsonl"))
}

/// A world's id across the state directory: `<name>-<fnv64a of its path>`, as every record kept
/// for it there is named.
#[must_use]
pub fn world_id(world: &Path) -> String {
    let h = fnv64a(world.as_os_str().as_encoded_bytes());
    let name = world
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("{name}-{h:016x}")
}

/// Record a session opening in `world` in the registry at `file`, with its remote.
///
/// # Errors
///
/// The registry cannot be written.
pub fn register_at(file: &Path, world: &Path, remote: Option<String>) -> std::io::Result<()> {
    let world = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    Journal::<Opened>::new(file).append(&Opened {
        world: world.display().to_string(),
        id: world_id(&world),
        remote,
        at: Timestamp::now().to_string(),
    })
}

/// Record a session opening in `world` in the human's registry, with its `origin` remote; a
/// machine with no home keeps none.
///
/// # Errors
///
/// The registry cannot be written.
pub fn register(world: &Path) -> std::io::Result<()> {
    match registry() {
        Some(file) => register_at(&file, world, remote_of(world)),
        None => Ok(()),
    }
}

/// A world's `origin` remote, normalised, as git reads it; `None` without one.
#[must_use]
pub fn remote_of(world: &Path) -> Option<String> {
    let out = mnemorph_sandbox::git()
        .arg("-C")
        .arg(world)
        .args(["config", "--get", "remote.origin.url"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    // A world's git config is its own to write: bounded, and only ever a hint for matching.
    let text = String::from_utf8_lossy(&out.stdout);
    let text = text.trim();
    if text.len() > REMOTE_CAP || text.chars().any(char::is_control) {
        return None;
    }
    remote(text)
}

/// The world `id` names among `known`, matched on the whole id: for the human's commands, which
/// alone turn an id back into a path.
#[must_use]
pub fn resolve<'a>(known: &'a [Known], id: &str) -> Option<&'a Known> {
    known.iter().find(|k| k.id == id)
}

/// Where the sightings of `world` live, beside the registry at `registry`:
/// `<state>/mnemorph/dimension/<world-id>/sightings.jsonl` — the harness's, outside every world.
#[must_use]
pub fn sightings_file(registry: &Path, world: &Path) -> PathBuf {
    let world = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    registry
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("dimension")
        .join(world_id(&world))
        .join("sightings.jsonl")
}

/// The relations' trusted journal's name among the world's chains.
pub const RELATIONS: &str = "relations";

/// Where the relations `world` states are kept, under the human's state directory `state`:
/// `<state>/mnemorph/dimension/<world-id>/relations.jsonl`, beside its sightings, the world named
/// by its real path.
#[must_use]
pub fn relations_in(state: &Path, world: &Path) -> PathBuf {
    let world = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    state
        .join("mnemorph/dimension")
        .join(world_id(&world))
        .join("relations.jsonl")
}

/// [`relations_in`], beside the registry at `registry` (`<state>/mnemorph/worlds.jsonl`).
#[must_use]
pub fn relations_file(registry: &Path, world: &Path) -> PathBuf {
    relations_in(&state_of(registry), world)
}

/// The human's state directory the registry at `registry` lives under.
fn state_of(registry: &Path) -> PathBuf {
    registry
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// What a row of the relations does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Act {
    /// A relation stated: the human's yes.
    Relate,
    /// A relation revoked.
    Unrelate,
    /// A type's inverse declared, or undeclared.
    Inverse,
}

/// One row of a world's trusted relations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationRow {
    /// When it was written.
    pub ts: Timestamp,
    /// What it does.
    pub act: Act,
    /// The other world's canonical path; empty for an inverse.
    #[serde(default)]
    pub world: String,
    /// The type.
    #[serde(rename = "type")]
    pub kind: String,
    /// The other world's remote, normalised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// The inverse declared; `None` undeclares it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse: Option<String>,
    /// Who wrote it: the human.
    pub by: String,
    /// The build that wrote it.
    #[serde(default)]
    pub build: Build,
}

/// The relations a world states, folded from its rows in order.
#[must_use]
pub fn fold_relations(rows: &[RelationRow]) -> Manifest {
    let mut m = Manifest::default();
    for r in rows {
        match r.act {
            Act::Relate => {
                if !m.states(&r.world, &r.kind) {
                    m.relations.push(Relation {
                        world: r.world.clone(),
                        kind: r.kind.clone(),
                        remote: r.remote.clone(),
                    });
                }
            }
            Act::Unrelate => m
                .relations
                .retain(|x| !(x.world == r.world && x.kind == r.kind)),
            Act::Inverse => match &r.inverse {
                Some(i) => {
                    m.inverse.insert(r.kind.clone(), i.clone());
                }
                None => {
                    m.inverse.remove(&r.kind);
                }
            },
        }
    }
    m
}

/// Whether `rel` is tracked by git in `root` and unchanged from HEAD: a manifest a body only
/// wrote is no evidence.
fn committed(root: &Path, rel: &str) -> bool {
    let git = |args: &[&str]| {
        mnemorph_sandbox::git()
            .arg("-C")
            .arg(root)
            .args(args)
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    let head = git(&["rev-parse", "--verify", "--quiet", &format!("HEAD:{rel}")]);
    let now = git(&["hash-object", "--no-filters", "--", rel]);
    matches!((head, now), (Some(h), Some(n)) if !h.is_empty() && h == n)
}

/// The worlds the registry at `file` knows, in the order first seen.
#[must_use]
pub fn known(file: &Path) -> Vec<Known> {
    let rows = Journal::<Opened>::new(file)
        .read()
        .map(|r| r.rows)
        .unwrap_or_default();
    let mut out: Vec<Known> = Vec::new();
    for row in rows {
        let path = PathBuf::from(&row.world);
        if let Some(k) = out.iter_mut().find(|k| k.world == path) {
            k.last = row.at;
            k.sessions += 1;
            if row.remote.is_some() {
                k.remote = row.remote;
            }
        } else {
            out.push(Known {
                world: path,
                id: row.id,
                remote: row.remote,
                first: row.at.clone(),
                last: row.at,
                sessions: 1,
            });
        }
    }
    out
}

/// A remote as worlds are matched on it: `host/owner/repo`, lowercase, with no scheme, no
/// credentials, no `.git` and no trailing `/`. `None` for a relative or local one.
#[must_use]
pub fn remote(url: &str) -> Option<String> {
    let url = url.trim();
    let url = url.strip_prefix("git+").unwrap_or(url);
    let rest = if let Some(r) = url.strip_prefix("github:") {
        format!("github.com/{r}")
    } else if let Some((_, r)) = url.split_once("://") {
        r.to_owned()
    } else if let Some((host, path)) = url.split_once(':') {
        // scp-like: `git@host:owner/repo`.
        if host.contains('/') || path.starts_with("//") {
            return None;
        }
        format!("{host}/{path}")
    } else {
        // A bare module path, as go.mod names one: `github.com/owner/repo`.
        url.to_owned()
    };
    // Credentials never reach a record.
    let rest = match rest.split_once('/') {
        Some((auth, path)) => {
            let host = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
            format!("{host}/{path}")
        }
        None => return None,
    };
    let rest = rest.split(['#', '?']).next().unwrap_or_default();
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let host = rest.split('/').next().unwrap_or_default();
    if !host.contains('.') || rest.split('/').filter(|s| !s.is_empty()).count() < 3 {
        return None;
    }
    Some(rest.to_lowercase())
}

// ---------------------------------------------------------------------------------------------
// The board's list

/// What the human did to the board's list by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hand {
    /// `mnemorphd board add <dir>`: the world is listed though no session opened in it.
    Add,
    /// `mnemorphd board forget <dir>`: the world is no longer listed.
    Forget,
}

/// One row of the board's list: the human's hand on it. The registry stays the sessions' alone,
/// so a world added by hand never moves a relation's session clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handled {
    /// Added or forgotten.
    pub act: Hand,
    /// The world's canonical path.
    pub world: String,
    /// Its id.
    pub id: String,
    /// When, RFC 3339 in UTC.
    pub at: String,
}

/// A world the board lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Its canonical path.
    pub world: PathBuf,
    /// Its id.
    pub id: String,
    /// Its latest remote, normalised.
    pub remote: Option<String>,
    /// When the latest session opened in it; `None` for a world only added by hand.
    pub last: Option<String>,
    /// How many sessions opened in it.
    pub sessions: u64,
}

/// Where the board's list lives, beside the registry at `registry`:
/// `<state>/mnemorph/board/worlds.jsonl` — the human's hand, outside every world.
#[must_use]
pub fn hand_file(registry: &Path) -> PathBuf {
    registry
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("board/worlds.jsonl")
}

/// The worlds the board lists: every world of the registry at `registry` and every world added
/// at `hand`, but a world forgotten and neither added again nor opened in a session since. A
/// world whose directory vanished stays listed: it is the board's to show it missing, never to
/// drop it.
#[must_use]
pub fn listed(registry: &Path, hand: &Path) -> Vec<Listed> {
    // Each world, and when it was forgotten if its latest hand was a forget.
    let mut out: Vec<(Listed, Option<Timestamp>)> = known(registry)
        .into_iter()
        .map(|k| {
            let listed = Listed {
                world: k.world,
                id: k.id,
                remote: k.remote,
                last: Some(k.last),
                sessions: k.sessions,
            };
            (listed, None)
        })
        .collect();
    let rows = Journal::<Handled>::new(hand)
        .read()
        .map(|r| r.rows)
        .unwrap_or_default();
    for row in rows {
        let path = PathBuf::from(&row.world);
        let at = Timestamp::parse(&row.at).unwrap_or(Timestamp(i64::MAX));
        match (out.iter_mut().find(|(l, _)| l.world == path), row.act) {
            (Some((_, forgot)), Hand::Add) => *forgot = None,
            (Some((_, forgot)), Hand::Forget) => *forgot = Some(at),
            (None, Hand::Add) => out.push((
                Listed {
                    world: path,
                    id: row.id,
                    remote: None,
                    last: None,
                    sessions: 0,
                },
                None,
            )),
            (None, Hand::Forget) => {}
        }
    }
    out.into_iter()
        .filter(|(l, forgot)| {
            forgot.is_none_or(|f| {
                l.last
                    .as_deref()
                    .and_then(Timestamp::parse)
                    .is_some_and(|last| last > f)
            })
        })
        .map(|(l, _)| l)
        .collect()
}

/// Write the human's `act` on `world` into the board's list at `hand`, and return the world as
/// listed. Refused inside a body's command (`MNEMORPH_BODY` is set there): what the board shows
/// is the human's to choose. `add` needs a world (a directory holding `.mnemorph/`); `forget` needs a world the board lists
/// (the registry at `registry` is read for it), a vanished one included.
///
/// # Errors
///
/// A body asks, the world is not one the act applies to, or the list cannot be written.
pub fn hand(hand: &Path, registry: &Path, act: Hand, world: &Path) -> Result<Listed, String> {
    if let Some(body) = std::env::var_os("MNEMORPH_BODY") {
        return Err(format!(
            "{} runs this: the board's worlds are added and forgotten by the human alone",
            body.to_string_lossy()
        ));
    }
    let world = match act {
        Hand::Add => match world.canonicalize() {
            Ok(w) if w.is_dir() && !live(&w) => {
                return Err(format!(
                    "{} holds no .mnemorph/: it is no world yet, and the board would list it as \
                     missing; open a session in it with `mnemorph` and it joins the board",
                    w.display()
                ));
            }
            Ok(w) if w.is_dir() => w,
            Ok(w) => return Err(format!("{} is not a directory", w.display())),
            Err(e) => return Err(format!("{}: {e}", world.display())),
        },
        Hand::Forget => world.canonicalize().unwrap_or_else(|_| world.to_path_buf()),
    };
    let found = listed(registry, hand)
        .into_iter()
        .find(|l| l.world == world);
    let out = match (act, found) {
        (Hand::Forget, None) => return Err(format!("{} is not on the board", world.display())),
        (_, Some(l)) => l,
        (Hand::Add, None) => Listed {
            id: world_id(&world),
            world: world.clone(),
            remote: None,
            last: None,
            sessions: 0,
        },
    };
    Journal::<Handled>::new(hand)
        .append(&Handled {
            act,
            world: world.display().to_string(),
            id: world_id(&world),
            at: Timestamp::now().to_string(),
        })
        .map_err(|e| format!("the board's list cannot be written: {e}"))?;
    Ok(out)
}

/// Whether `world` is a live world: a directory (a link to one is followed, so a link whose
/// target is gone is missing) holding a `.mnemorph/` directory of its own — never a link out.
#[must_use]
pub fn live(world: &Path) -> bool {
    std::fs::metadata(world).is_ok_and(|m| m.is_dir())
        && std::fs::symlink_metadata(world.join(".mnemorph")).is_ok_and(|m| m.is_dir())
}

/// Where the board keeps when each missing world was first seen missing, beside the registry at
/// `registry`: `<state>/mnemorph/board/missing.json` — the human's state, outside every world.
#[must_use]
pub fn missing_file(registry: &Path) -> PathBuf {
    registry
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("board/missing.json")
}

/// The worlds the board lists, split: the live ones, and the missing ones with when each was
/// first seen missing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Standing {
    /// The live worlds.
    pub live: Vec<Listed>,
    /// The missing worlds, each with when it was first seen missing.
    pub missing: Vec<(Listed, Timestamp)>,
}

/// The `since` each world id was first seen missing, as kept at `file`; empty when it does not
/// read.
fn read_missing(file: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Keep `since` at `file` whole: a fresh `0600` file renamed over it.
fn write_missing(file: &Path, since: &BTreeMap<String, String>) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        o.mode(0o600);
    }
    let mut f = o.open(&tmp)?;
    std::io::Write::write_all(&mut f, serde_json::to_string(since)?.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, file)
}

/// The worlds the board lists at `registry` and `hand`, live and missing. With `record`, a world
/// newly missing is kept as missing since `now`, and one that came back is dropped from the record,
/// so it is live again.
#[must_use]
pub fn standing(registry: &Path, hand: &Path, now: Timestamp, record: bool) -> Standing {
    let file = missing_file(registry);
    let kept = read_missing(&file);
    let mut since = BTreeMap::new();
    let mut out = Standing::default();
    for l in listed(registry, hand) {
        if live(&l.world) {
            out.live.push(l);
            continue;
        }
        let at = kept
            .get(&l.id)
            .and_then(|t| Timestamp::parse(t))
            .unwrap_or(now);
        since.insert(l.id.clone(), at.to_string());
        out.missing.push((l, at));
    }
    if record && since != kept {
        let _ = write_missing(&file, &since);
    }
    out
}

/// Forget the missing worlds the board lists at `registry` and `hand` — those missing for at
/// least `after` seconds by `now`, or every one with `None` — and return them. Only the board's
/// list is written (a `forget` row each): no directory, journal or other state is touched. With
/// `dry_run`, nothing is written at all.
///
/// # Errors
///
/// A body asks, or the board's list cannot be written.
pub fn prune(
    registry: &Path,
    hand: &Path,
    after: Option<i64>,
    now: Timestamp,
    dry_run: bool,
) -> Result<Vec<(Listed, Timestamp)>, String> {
    if let Some(body) = std::env::var_os("MNEMORPH_BODY") {
        return Err(format!(
            "{} runs this: the board's worlds are forgotten by the human alone",
            body.to_string_lossy()
        ));
    }
    let gone: Vec<(Listed, Timestamp)> = standing(registry, hand, now, !dry_run)
        .missing
        .into_iter()
        .filter(|(_, since)| after.is_none_or(|a| now.0.saturating_sub(since.0) >= a))
        .collect();
    if dry_run || gone.is_empty() {
        return Ok(gone);
    }
    let journal = Journal::<Handled>::new(hand);
    for (l, _) in &gone {
        journal
            .append(&Handled {
                act: Hand::Forget,
                world: l.world.display().to_string(),
                id: l.id.clone(),
                at: now.to_string(),
            })
            .map_err(|e| format!("the board's list cannot be written: {e}"))?;
    }
    let file = missing_file(registry);
    let mut since = read_missing(&file);
    for (l, _) in &gone {
        since.remove(&l.id);
    }
    write_missing(&file, &since).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(gone)
}

// ---------------------------------------------------------------------------------------------
// Sightings

/// One cross-world reference the harness saw a body read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sighting {
    /// The world it was seen from, by id.
    pub from: String,
    /// The world it names, by id: never its path, which only the human's commands resolve.
    pub to: String,
    /// The relation's type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Where it was read: `file:line`, relative to the world.
    pub anchor: String,
    /// The body that read it.
    pub body: String,
    /// The session ordinal of `from` it was seen in.
    pub session: u64,
}

/// What a manifest line names.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Named {
    /// A path, as written, relative to the manifest's directory or absolute.
    Path(String),
    /// A remote, normalised.
    Remote(String),
}

/// The cross-world references a manifest names: each line, the type and what it names.
fn references(file: &str, text: &str) -> Vec<(usize, &'static str, Named)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let line = line.trim();
        match file {
            "Cargo.toml" => {
                for (key, value) in toml_pairs(line) {
                    match key {
                        "path" => out.push((n, "depends-on", Named::Path(value))),
                        "git" => {
                            if let Some(r) = remote(&value) {
                                out.push((n, "depends-on", Named::Remote(r)));
                            }
                        }
                        _ => {}
                    }
                }
            }
            "package.json" => {
                let Some(value) = line
                    .split_once("\":")
                    .map(|(_, v)| v.trim().trim_end_matches(',').trim_matches('"'))
                else {
                    continue;
                };
                if let Some(p) = ["file:", "link:", "portal:"]
                    .iter()
                    .find_map(|s| value.strip_prefix(s))
                {
                    out.push((n, "depends-on", Named::Path(p.to_owned())));
                } else if (value.starts_with("git") || value.contains("://"))
                    && let Some(r) = remote(value)
                {
                    out.push((n, "depends-on", Named::Remote(r)));
                }
            }
            "go.mod" => {
                if let Some((_, to)) = line.split_once("=>") {
                    let to = to.split_whitespace().next().unwrap_or_default();
                    if to.starts_with('.') || to.starts_with('/') {
                        out.push((n, "depends-on", Named::Path(to.to_owned())));
                    }
                } else {
                    let line = line.strip_prefix("require").unwrap_or(line).trim();
                    let mut words = line.split_whitespace();
                    if let (Some(module), Some(version)) = (words.next(), words.next())
                        && version.starts_with('v')
                        && let Some(r) = remote(module)
                    {
                        out.push((n, "depends-on", Named::Remote(r)));
                    }
                }
            }
            ".gitmodules" => {
                for (key, value) in toml_pairs(line) {
                    if key == "url"
                        && let Some(r) = remote(&value)
                    {
                        out.push((n, "vendors", Named::Remote(r)));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The `key = "value"` pairs on one line of TOML or git config, inline tables included.
fn toml_pairs(line: &str) -> Vec<(&str, String)> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq]
            .rsplit(|c: char| c == '{' || c == ',' || c.is_whitespace())
            .find(|s| !s.is_empty())
            .unwrap_or_default();
        let after = rest[eq + 1..].trim_start();
        // An inline table: its pairs are read in turn.
        if let Some(inner) = after.strip_prefix('{') {
            rest = inner;
            continue;
        }
        let (value, tail) = if let Some(q) = after.strip_prefix('"') {
            match q.find('"') {
                Some(end) => (q[..end].to_owned(), &q[end + 1..]),
                None => break,
            }
        } else {
            let end = after.find([',', '}']).unwrap_or(after.len());
            (after[..end].trim().to_owned(), &after[end..])
        };
        out.push((key, value));
        rest = tail;
    }
    out
}

/// A path with its `.` and `..` folded, never touching the disk.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The sightings a body's read of `read` in `world` makes, against the worlds `known` names: none
/// unless it is a manifest inside the world, read by the harness from where the body read it.
/// A path is matched on the deepest known world whose root holds it — never `world` itself — and
/// a remote on a known world's remote. Only a manifest git tracks, unchanged from HEAD, counts.
/// Nothing outside `world` is opened.
#[must_use]
pub fn sightings_of(world: &Path, known: &[Known], body: &str, read: &str) -> Vec<Sighting> {
    let Ok(root) = world.canonicalize() else {
        return Vec::new();
    };
    let Some(rel) = crate::own::normal(read) else {
        return Vec::new();
    };
    let Some(name) = Path::new(&rel).file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    if !MANIFESTS.contains(&name) {
        return Vec::new();
    }
    // Through its links, the file must still be the world's.
    let Ok(file) = root.join(&rel).canonicalize() else {
        return Vec::new();
    };
    if !file.starts_with(&root)
        || std::fs::metadata(&file).map_or(true, |m| !m.is_file() || m.len() > MANIFEST_CAP)
    {
        return Vec::new();
    }
    if !committed(&root, &rel) {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Vec::new();
    };
    let dir = root.join(&rel);
    let dir = dir.parent().unwrap_or(&root);
    let session = known
        .iter()
        .find(|k| k.world == root)
        .map_or(0, |k| k.sessions);
    let from = world_id(&root);
    let mut out: Vec<Sighting> = Vec::new();
    for (line, kind, named) in references(name, &text) {
        let to = match &named {
            Named::Path(p) => {
                let target = lexical(&dir.join(p));
                known
                    .iter()
                    .filter(|k| target.starts_with(&k.world))
                    .max_by_key(|k| k.world.components().count())
                    .filter(|k| k.world != root)
            }
            Named::Remote(r) => known.iter().find(|k| {
                k.world != root
                    && k.remote
                        .as_deref()
                        .is_some_and(|kr| kr == r || r.starts_with(&format!("{kr}/")))
            }),
        };
        let Some(to) = to else { continue };
        let s = Sighting {
            from: from.clone(),
            to: to.id.clone(),
            kind: kind.to_owned(),
            anchor: format!("{rel}:{line}"),
            body: body.to_owned(),
            session,
        };
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// Record the sightings a body's read of `read` makes, against the registry at `registry`: each
/// once per session. How many were new.
///
/// # Errors
///
/// The sightings cannot be written.
pub fn sight(world: &Path, registry: &Path, body: &str, read: &str) -> std::io::Result<usize> {
    let seen = sightings_of(world, &known(registry), body, read);
    if seen.is_empty() {
        return Ok(0);
    }
    let journal = Journal::<Sighting>::new(sightings_file(registry, world));
    let had = journal.read().map(|r| r.rows).unwrap_or_default();
    let mut n = 0;
    for s in seen.into_iter().filter(|s| !had.contains(s)) {
        journal.append(&s)?;
        n += 1;
    }
    Ok(n)
}

/// The sightings of `world` kept beside the registry at `registry`, as written.
#[must_use]
pub fn load_sightings(registry: &Path, world: &Path) -> Vec<Sighting> {
    Journal::<Sighting>::new(sightings_file(registry, world))
        .read()
        .map(|r| r.rows)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// The manifest

/// The relations' findings `onto check` reports: each learned relation unseen too long, and with
/// `dimension` each obstruction against a neighbour's manifest, with the glues.
///
/// # Errors
///
/// The world's manifest does not read, or a neighbour's cannot be glued.
pub fn relation_findings(
    world: &Path,
    dimension: bool,
) -> Result<(Vec<Finding>, Vec<Glue>), String> {
    let registry = registry();
    let known = registry.as_deref().map(known).unwrap_or_default();
    let now = known
        .iter()
        .find(|k| k.world == world)
        .map_or(0, |k| k.sessions);
    let manifest = Manifest::trusted(registry.as_deref(), world)?;
    let sightings = registry
        .as_deref()
        .map(|r| load_sightings(r, world))
        .unwrap_or_default();
    let mut findings = stale_findings(&proposals(&sightings, &manifest, now));
    findings.extend(untrusted_relations(world, &manifest));
    if !dimension {
        return Ok((findings, Vec::new()));
    }
    let (glues, f) = glue(world, &known, registry.as_deref())?;
    findings.extend(f);
    Ok((findings, glues))
}

/// Whether `kind` is a relation type: lowercase ASCII words joined by `-`, at most 64 bytes.
#[must_use]
pub fn valid_kind(kind: &str) -> bool {
    !kind.is_empty()
        && kind.len() <= 64
        && !kind.starts_with('-')
        && !kind.ends_with('-')
        && kind.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

/// One relation this world states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    /// The other world: its canonical path on this machine.
    pub world: String,
    /// Its type, read from this world's side.
    #[serde(rename = "type")]
    pub kind: String,
    /// The other world's remote, normalised, when it has one: how it is known elsewhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

/// The dimension manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Declared inverses: `type = "inverse"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inverse: BTreeMap<String, String>,
    /// The relations stated.
    #[serde(default, rename = "relation", skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,
}

/// An `untrusted-relation` finding for each relation, or inverse, the in-world copy states that
/// the trusted relations do not: a proposal, the human's `onto relate` (or `onto inverse`) its
/// yes. A copy that does not read proposes nothing.
#[must_use]
pub fn untrusted_relations(world: &Path, trusted: &Manifest) -> Vec<Finding> {
    let Ok(said) = Manifest::read_file(world) else {
        return Vec::new();
    };
    let finding = |message: String| Finding {
        code: "untrusted-relation",
        severity: Severity::Medium,
        at: MANIFEST.into(),
        paths: vec![MANIFEST.into()],
        creatures: vec![crate::queue::THRONE.into()],
        message,
    };
    let mut out: Vec<Finding> = said
        .relations
        .iter()
        .filter(|r| !trusted.states(&r.world, &r.kind))
        .map(|r| {
            finding(format!(
                "`{}` towards {} is stated only in the in-world {MANIFEST}, never trusted: \
                 `mnemorphd onto relate {} {}` states it",
                r.kind,
                r.world.escape_debug(),
                r.world.escape_debug(),
                r.kind
            ))
        })
        .collect();
    out.extend(
        said.inverse
            .iter()
            .filter(|(k, v)| trusted.inverse.get(*k) != Some(*v))
            .map(|(k, v)| {
                finding(format!(
                    "the inverse `{k}` ↔ `{v}` is declared only in the in-world {MANIFEST}, never \
                     trusted: `mnemorphd onto inverse {k} {v}` declares it"
                ))
            }),
    );
    out
}

impl Manifest {
    /// The relations the world at `world` states, as the human's state directory keeps them; none
    /// with no home.
    ///
    /// # Errors
    ///
    /// They cannot be read, or their chain does not walk whole.
    pub fn load(world: &Path) -> Result<Manifest, String> {
        Manifest::trusted(registry().as_deref(), world)
    }

    /// [`Manifest::load`], beside the registry at `registry` (`None`: no home, nothing stated).
    ///
    /// # Errors
    ///
    /// As [`Manifest::load`].
    pub fn trusted(registry: Option<&Path>, world: &Path) -> Result<Manifest, String> {
        let Some(registry) = registry else {
            return Ok(Manifest::default());
        };
        let path = relations_file(registry, world);
        crate::pins::outside_as(world, &path, "the relations are")?;
        let anchor = crate::chains::anchor_under(&state_of(registry), world, RELATIONS);
        let report = chain::verify_against(&path, anchor.as_ref())
            .map_err(|e| format!("the relations cannot be read: {e}"))?;
        if !report.holds() {
            return Err(format!("the relations do not verify: {report}"));
        }
        let back = Journal::<RelationRow>::chained(&path)
            .read()
            .map_err(|e| format!("the relations cannot be read: {e}"))?;
        if let Some(line) = back.skipped.first() {
            return Err(format!(
                "the relations do not verify: line {line} does not read"
            ));
        }
        Ok(fold_relations(&back.rows))
    }

    /// The in-world copy of the world at `world`, `.mnemorph/dimension.toml`: shown and proposed
    /// from, never trusted. An absent one is empty.
    ///
    /// # Errors
    ///
    /// It exists and does not read.
    pub fn read_file(world: &Path) -> Result<Manifest, String> {
        let path = world.join(MANIFEST);
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Manifest::default()),
            Err(e) => return Err(format!("{MANIFEST}: {e}")),
        };
        // A FIFO, a device or a huge file is no manifest: never waited on, never read whole.
        if !meta.is_file() {
            return Err(format!("{MANIFEST}: not a regular file"));
        }
        if meta.len() > DIMENSION_CAP {
            return Err(format!("{MANIFEST}: over {DIMENSION_CAP} bytes"));
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{MANIFEST}: {e}"))?;
        let m: Manifest = toml::from_str(&text).map_err(|e| format!("{MANIFEST}: {e}"))?;
        let kinds = m
            .relations
            .iter()
            .map(|r| &r.kind)
            .chain(m.inverse.keys())
            .chain(m.inverse.values());
        for k in kinds {
            if !valid_kind(k) {
                return Err(format!(
                    "{MANIFEST}: `{}` is no relation type: lowercase words joined by `-`",
                    k.escape_debug()
                ));
            }
        }
        Ok(m)
    }

    /// Write it to the world at `world` as its in-world copy, through no link.
    ///
    /// # Errors
    ///
    /// It cannot be written.
    pub fn save(&self, world: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::beneath::write(world, MANIFEST, text.as_bytes(), false)
            .map_err(|e| format!("{MANIFEST}: {e}"))
    }

    /// Whether it states `kind` towards `world`.
    #[must_use]
    pub fn states(&self, world: &str, kind: &str) -> bool {
        self.relations
            .iter()
            .any(|r| r.world == world && r.kind == kind)
    }

    /// Whether it states `kind` towards the world whose id is `id`.
    #[must_use]
    pub fn states_id(&self, id: &str, kind: &str) -> bool {
        self.relations
            .iter()
            .any(|r| r.kind == kind && world_id(Path::new(&r.world)) == id)
    }

    /// The inverse of a type: declared here, else compiled, else `<type>-by` (and back).
    #[must_use]
    pub fn inverse_of(&self, kind: &str) -> String {
        if let Some(i) = self.inverse.get(kind) {
            return i.clone();
        }
        if let Some((t, _)) = self.inverse.iter().find(|(_, i)| *i == kind) {
            return t.clone();
        }
        if let Some((a, b)) = INVERSES.iter().find(|(a, b)| *a == kind || *b == kind) {
            return if *a == kind { (*b).into() } else { (*a).into() };
        }
        kind.strip_suffix("-by")
            .map_or_else(|| format!("{kind}-by"), str::to_owned)
    }

    /// The types it states towards `world`.
    fn types_towards(&self, world: &str) -> BTreeSet<String> {
        self.relations
            .iter()
            .filter(|r| r.world == world)
            .map(|r| r.kind.clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Proposals

/// A change to the manifest offered to the human. Proposing writes nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Proposal {
    /// A relation named twice, not yet stated.
    Add {
        /// The other world, by id; the human's command shows its path.
        world: String,
        /// The type sighted.
        kind: String,
        /// An existing type offered first, when the new one is most likely its synonym.
        offered: Option<String>,
        /// The types the pair already carries.
        carries: Vec<String>,
        /// The bodies that sighted it.
        bodies: Vec<String>,
        /// The sessions it was sighted in.
        sessions: Vec<u64>,
        /// The anchors it was sighted at.
        anchors: Vec<String>,
    },
    /// A learned relation unseen for [`STALE_AFTER`] sessions.
    Remove {
        /// The other world.
        world: String,
        /// The type.
        kind: String,
        /// The session it was last sighted in.
        last: u64,
        /// The session now.
        now: u64,
    },
}

impl Proposal {
    /// The proposal as a diff of the manifest.
    #[must_use]
    pub fn diff(&self) -> String {
        let (sign, world, kind) = match self {
            Proposal::Add { world, kind, .. } => ('+', world, kind),
            Proposal::Remove { world, kind, .. } => ('-', world, kind),
        };
        format!(
            "--- a/{MANIFEST}\n+++ b/{MANIFEST}\n{sign}[[relation]]\n{sign}world = \"{world}\"\n{sign}type = \"{kind}\"\n"
        )
    }

    /// Why it is proposed, in one line.
    #[must_use]
    pub fn why(&self) -> String {
        match self {
            Proposal::Add {
                offered,
                carries,
                bodies,
                anchors,
                sessions,
                ..
            } => {
                use std::fmt::Write as _;
                let mut s = format!(
                    "named twice: in sessions {}, by {}, from {}",
                    sessions
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    bodies.join(", "),
                    anchors.join(", ")
                );
                if let Some(t) = offered {
                    let _ = write!(
                        s,
                        "; most pairs of this new type already carry `{t}`: state `{t}` instead?"
                    );
                } else if !carries.is_empty() {
                    let _ = write!(s, "; the pair already carries {}", carries.join(", "));
                }
                s
            }
            Proposal::Remove { last, now, .. } => {
                format!(
                    "last sighted in session {last}, unseen for {} sessions",
                    now - last
                )
            }
        }
    }
}

/// Whether two sightings name a relation twice: different bodies from different anchors, in
/// different sessions — a body and the child it commissions share a session, so one session is
/// never two voices.
fn named_twice(seen: &[&Sighting]) -> bool {
    seen.iter().enumerate().any(|(i, a)| {
        seen[i + 1..]
            .iter()
            .any(|b| a.body != b.body && a.anchor != b.anchor && a.session != b.session)
    })
}

/// What the ledger proposes to the human: each relation named twice that the manifest does not
/// state, and each learned relation unseen for [`STALE_AFTER`] sessions, `now` being this world's
/// session count.
#[must_use]
pub fn proposals(sightings: &[Sighting], manifest: &Manifest, now: u64) -> Vec<Proposal> {
    let mut by_pair: BTreeMap<(&str, &str), Vec<&Sighting>> = BTreeMap::new();
    for s in sightings {
        by_pair
            .entry((s.to.as_str(), s.kind.as_str()))
            .or_default()
            .push(s);
    }
    let stated_types: BTreeSet<&str> = manifest.relations.iter().map(|r| r.kind.as_str()).collect();
    let mut out = Vec::new();
    for (&(to, kind), seen) in &by_pair {
        if manifest.states_id(to, kind) || !named_twice(seen) {
            continue;
        }
        let mut offered = None;
        if !stated_types.contains(kind) {
            let pairs: BTreeSet<&str> = by_pair
                .keys()
                .filter(|(_, k)| *k == kind)
                .map(|(t, _)| *t)
                .collect();
            if pairs.len() >= SYNONYM_PAIRS {
                offered = stated_types
                    .iter()
                    .map(|t| {
                        let carried = pairs.iter().filter(|p| manifest.states_id(p, t)).count();
                        #[allow(clippy::cast_precision_loss, reason = "counts of worlds are small")]
                        (carried as f64 / pairs.len() as f64, *t)
                    })
                    .filter(|(c, _)| *c >= SYNONYM)
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, t)| t.to_owned());
            }
        }
        let unique = |f: fn(&Sighting) -> &str| -> Vec<String> {
            seen.iter()
                .map(|s| f(s).to_owned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        out.push(Proposal::Add {
            world: to.to_owned(),
            kind: kind.to_owned(),
            carries: manifest
                .relations
                .iter()
                .filter(|r| world_id(Path::new(&r.world)) == to)
                .map(|r| r.kind.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            offered,
            bodies: unique(|s| s.body.as_str()),
            sessions: seen
                .iter()
                .map(|s| s.session)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            anchors: unique(|s| s.anchor.as_str()),
        });
    }
    for r in &manifest.relations {
        // Only a learned relation carries its way back: one never sighted was the human's alone.
        let id = world_id(Path::new(&r.world));
        let Some(last) = by_pair
            .get(&(id.as_str(), r.kind.as_str()))
            .and_then(|seen| seen.iter().map(|s| s.session).max())
        else {
            continue;
        };
        if now.saturating_sub(last) >= STALE_AFTER {
            out.push(Proposal::Remove {
                world: r.world.clone(),
                kind: r.kind.clone(),
                last,
                now,
            });
        }
    }
    out
}

/// A `stale-relation` finding for each removal proposed.
#[must_use]
pub fn stale_findings(proposals: &[Proposal]) -> Vec<Finding> {
    proposals
        .iter()
        .filter_map(|p| match p {
            Proposal::Remove { world, kind, .. } => Some(Finding {
                code: "stale-relation",
                severity: Severity::Low,
                at: MANIFEST.into(),
                paths: vec![MANIFEST.into()],
                creatures: vec![crate::queue::THRONE.into()],
                message: format!("`{kind}` towards {world}: {}", p.why()),
            }),
            Proposal::Add { .. } => None,
        })
        .collect()
}

/// The relations are written by the human alone: refused inside a body's command
/// (`MNEMORPH_BODY` is set there) and under any of the harness's processes, whatever the
/// environment says ([`crate::pins::under_harness`]); the sandbox keeps the state directory out
/// of every command's reach besides.
fn refuse_a_body() -> Result<(), String> {
    if let Some(body) = std::env::var_os("MNEMORPH_BODY") {
        return Err(format!(
            "{} runs this: relations between worlds are the human's to state",
            body.to_string_lossy()
        ));
    }
    if let Some(harness) = crate::pins::under_harness()? {
        return Err(format!(
            "this runs under {harness}: relations between worlds are the human's to state, never \
             a command a session started"
        ));
    }
    Ok(())
}

/// `row` appended to the relations of `world` kept beside `registry`, their in-world copy then
/// written again from them. A copy that cannot be written is said on stderr: it grants nothing.
fn write_row(registry: &Path, world: &Path, mut row: RelationRow) -> Result<(), String> {
    refuse_a_body()?;
    row.build = Build::this();
    Journal::<RelationRow>::chained(relations_file(registry, world))
        .append(&row)
        .map_err(|e| format!("the relations cannot be written: {e}"))?;
    if let Err(e) = Manifest::trusted(Some(registry), world).and_then(|m| m.save(world)) {
        eprintln!("mnemorph: the in-world copy of the relations could not be written: {e}");
    }
    Ok(())
}

fn row(act: Act, world: String, kind: &str) -> RelationRow {
    RelationRow {
        ts: Timestamp::now(),
        act,
        world,
        kind: kind.to_owned(),
        remote: None,
        inverse: None,
        by: crate::queue::HUMAN.to_owned(),
        build: Build::default(),
    }
}

/// The registry, or why there is none.
fn home() -> Result<PathBuf, String> {
    registry().ok_or_else(|| {
        "no home: the relations are kept under the human's state directory".to_owned()
    })
}

/// Write the human's yes: `kind` towards the world at `other` joins the relations of `world`.
/// Whether it was new.
///
/// # Errors
///
/// A body asks, there is no home, or the relations do not read or cannot be written.
pub fn relate(world: &Path, other: &Path, kind: &str) -> Result<bool, String> {
    relate_at(&home()?, world, other, kind)
}

/// [`relate`], beside the registry at `registry`.
///
/// # Errors
///
/// As [`relate`].
pub fn relate_at(registry: &Path, world: &Path, other: &Path, kind: &str) -> Result<bool, String> {
    refuse_a_body()?;
    let other = other.canonicalize().unwrap_or_else(|_| other.to_path_buf());
    let key = other.display().to_string();
    if Manifest::trusted(Some(registry), world)?.states(&key, kind) {
        return Ok(false);
    }
    let mut r = row(Act::Relate, key, kind);
    r.remote = remote_of(&other);
    write_row(registry, world, r)?;
    Ok(true)
}

/// Revoke `kind` towards the world at `other` from the relations of `world`. Whether it was
/// stated.
///
/// # Errors
///
/// A body asks, there is no home, or the relations do not read or cannot be written.
pub fn unrelate(world: &Path, other: &Path, kind: &str) -> Result<bool, String> {
    unrelate_at(&home()?, world, other, kind)
}

/// [`unrelate`], beside the registry at `registry`.
///
/// # Errors
///
/// As [`unrelate`].
pub fn unrelate_at(
    registry: &Path,
    world: &Path,
    other: &Path,
    kind: &str,
) -> Result<bool, String> {
    refuse_a_body()?;
    let other = other.canonicalize().unwrap_or_else(|_| other.to_path_buf());
    let key = other.display().to_string();
    if !Manifest::trusted(Some(registry), world)?.states(&key, kind) {
        return Ok(false);
    }
    write_row(registry, world, row(Act::Unrelate, key, kind))?;
    Ok(true)
}

/// Declare `inverse` as the inverse of `kind` in the relations of `world`, or with `None` take the
/// declaration back. Whether anything changed.
///
/// # Errors
///
/// A body asks, or the relations do not read or cannot be written.
pub fn declare_inverse_at(
    registry: &Path,
    world: &Path,
    kind: &str,
    inverse: Option<&str>,
) -> Result<bool, String> {
    refuse_a_body()?;
    if Manifest::trusted(Some(registry), world)?
        .inverse
        .get(kind)
        .map(String::as_str)
        == inverse
    {
        return Ok(false);
    }
    let mut r = row(Act::Inverse, String::new(), kind);
    r.inverse = inverse.map(str::to_owned);
    write_row(registry, world, r)?;
    Ok(true)
}

// ---------------------------------------------------------------------------------------------
// Gluing

/// Two worlds' manifests glued on their pair, read from this world's side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glue {
    /// The neighbour.
    pub neighbour: String,
    /// What both state.
    pub shared: Vec<String>,
    /// What only this world states.
    pub ours: Vec<String>,
    /// What only the neighbour states, through inverses.
    pub theirs: Vec<String>,
}

impl Glue {
    /// Whether the two cannot both hold: both state something, and nothing in common.
    #[must_use]
    pub fn obstructed(&self) -> bool {
        self.shared.is_empty() && !self.ours.is_empty() && !self.theirs.is_empty()
    }
}

/// Glue the relations of `world` with each neighbour's — every world it states a relation
/// towards, and every world `known` that states one towards it — reading the neighbours'
/// relations as the state directory beside `registry` keeps them and nothing else of theirs,
/// never writing them. The glues, and a `dimension-obstruction` finding for each pair that cannot
/// hold.
///
/// # Errors
///
/// This world's relations do not read.
pub fn glue(
    world: &Path,
    known: &[Known],
    registry: Option<&Path>,
) -> Result<(Vec<Glue>, Vec<Finding>), String> {
    let root = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    let ours = Manifest::trusted(registry, &root)?;
    let me = root.display().to_string();
    let my_remote = known
        .iter()
        .find(|k| k.world == root)
        .and_then(|k| k.remote.clone())
        .or_else(|| remote_of(&root));
    let mut neighbours: BTreeSet<String> = ours.relations.iter().map(|r| r.world.clone()).collect();
    neighbours.extend(
        known
            .iter()
            .filter(|k| k.world != root)
            .map(|k| k.world.display().to_string()),
    );
    let mut glues = Vec::new();
    let mut findings = Vec::new();
    for n in neighbours {
        let path = PathBuf::from(&n);
        // A neighbour's relations that do not read are reported and read as stating nothing,
        // and never fail this world's own check.
        let theirs = match Manifest::trusted(registry, &path) {
            Ok(m) => m,
            Err(e) => {
                findings.push(Finding {
                    code: "unreadable-neighbour",
                    severity: Severity::Low,
                    at: MANIFEST.into(),
                    paths: Vec::new(),
                    creatures: vec![crate::queue::THRONE.into()],
                    message: format!("{n}: {}", e.escape_debug()),
                });
                Manifest::default()
            }
        };
        let names_me = |r: &Relation| {
            lexical(&path.join(&r.world)) == root
                || (my_remote.is_some() && r.remote == my_remote)
                || r.world == me
        };
        let r_b: BTreeSet<String> = theirs
            .relations
            .iter()
            .filter(|r| names_me(r))
            .map(|r| theirs.inverse_of(&r.kind))
            .collect();
        let r_a = ours.types_towards(&n);
        if r_a.is_empty() && r_b.is_empty() {
            continue;
        }
        let g = Glue {
            neighbour: n.clone(),
            shared: r_a.intersection(&r_b).cloned().collect(),
            ours: r_a.difference(&r_b).cloned().collect(),
            theirs: r_b.difference(&r_a).cloned().collect(),
        };
        if g.obstructed() {
            findings.push(Finding {
                code: "dimension-obstruction",
                severity: Severity::High,
                at: MANIFEST.into(),
                paths: vec![MANIFEST.into()],
                creatures: vec![crate::queue::THRONE.into()],
                message: format!(
                    "{n}: this world states {}, it states {} — nothing in common",
                    g.ours.join(", "),
                    g.theirs.join(", ")
                ),
            });
        }
        glues.push(g);
    }
    Ok((glues, findings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remotes_normalise_every_way_of_writing_one() {
        for url in [
            "https://github.com/Kaginari/Mnemorph.git",
            "git@github.com:Kaginari/mnemorph.git",
            "ssh://git@github.com/kaginari/mnemorph",
            "git+https://token:x@github.com/kaginari/mnemorph/",
            "github:kaginari/mnemorph",
            "github.com/kaginari/mnemorph",
        ] {
            assert_eq!(
                remote(url).as_deref(),
                Some("github.com/kaginari/mnemorph"),
                "{url}"
            );
        }
        assert_eq!(remote("../sibling.git"), None);
        assert_eq!(remote("/srv/repo"), None);
        assert_eq!(remote("serde"), None);
    }

    #[test]
    fn manifests_name_paths_and_remotes() {
        let cargo = "[dependencies]\nlib = { path = \"../lib\", version = \"1\" }\nx = { git = \"https://github.com/o/x\" }\n[lib]\npath = \"src/lib.rs\"\n";
        let refs = references("Cargo.toml", cargo);
        assert_eq!(refs[0], (2, "depends-on", Named::Path("../lib".into())));
        assert_eq!(
            refs[1],
            (3, "depends-on", Named::Remote("github.com/o/x".into()))
        );
        assert_eq!(refs[2], (5, "depends-on", Named::Path("src/lib.rs".into())));
        let pkg = "{\n  \"dependencies\": {\n    \"a\": \"file:../a\",\n    \"b\": \"github:o/b\",\n    \"c\": \"^1.0.0\"\n  }\n}\n";
        let refs = references("package.json", pkg);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].2, Named::Path("../a".into()));
        let gomod =
            "module x\nrequire (\n\tgithub.com/o/y v1.2.0\n)\nreplace github.com/o/z => ../z\n";
        let refs = references("go.mod", gomod);
        assert_eq!(refs[0].2, Named::Remote("github.com/o/y".into()));
        assert_eq!(refs[1].2, Named::Path("../z".into()));
        let subs = "[submodule \"v\"]\n\tpath = vendor/v\n\turl = git@github.com:o/v.git\n";
        let refs = references(".gitmodules", subs);
        assert_eq!(
            refs,
            vec![(3, "vendors", Named::Remote("github.com/o/v".into()))]
        );
    }

    #[test]
    fn inverses_are_declared_compiled_or_by() {
        let mut m = Manifest::default();
        assert_eq!(m.inverse_of("deploys"), "deployed-by");
        assert_eq!(m.inverse_of("deployed-by"), "deploys");
        assert_eq!(m.inverse_of("reads-secrets-from"), "reads-secrets-from-by");
        assert_eq!(m.inverse_of("reads-secrets-from-by"), "reads-secrets-from");
        m.inverse.insert("feeds".into(), "eats".into());
        assert_eq!(m.inverse_of("eats"), "feeds");
    }
}
