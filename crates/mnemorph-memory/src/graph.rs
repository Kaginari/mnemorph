//! The claim graph: every note a fixed id, its corrections a chain of versions, and what it rests
//! on — other claims, facts of the ontology, files with their hashes. When something it rests on
//! moves, everything downstream needs review: marked, never deleted, never retired, until a route
//! grounds it again. And each claim a strength, read from its route, its age, how many times it
//! was stood behind and the health of what it rests on — never stronger than its weakest source.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path};

use crate::note::{By, Change, Dependency, Note, Status, fold};
use crate::store::{Tier, append, read};

/// What moved: the origin of an invalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved<'a> {
    /// A claim, by id: superseded, struck by an attributable failure, or retired.
    Claim(&'a str),
    /// A fact of the ontology, by id: contested.
    Fact(&'a str),
    /// A creature, by name: rebirthed, rewritten or retired.
    Creature(&'a str),
}

/// The SHA-256 of a world file's bytes, its first 16 hex digits: `None` when it is not a readable
/// file inside the world.
#[must_use]
pub fn file_hash(world: &Path, path: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    let rel = inside(world, path)?;
    let bytes = std::fs::read(world.join(&rel)).ok()?;
    Some(crate::store::hex16(&Sha256::digest(&bytes)))
}

/// A path as the graph keys it: relative to the world, `./` dropped; `None` for one outside it.
fn inside(world: &Path, path: &str) -> Option<String> {
    let p = Path::new(path);
    let rel = if p.is_absolute() {
        p.strip_prefix(world).ok()?
    } else {
        p
    };
    let mut out = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => out.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.is_empty()).then(|| out.join("/"))
}

/// The files `paths` name, each with its content hash now: what a claim resting on them records
/// when it is proposed or grounded. A path that is no readable file inside the world is left out.
#[must_use]
pub fn files(world: &Path, paths: &[String]) -> Vec<Dependency> {
    let mut out: Vec<Dependency> = Vec::new();
    for p in paths {
        let (Some(path), Some(hash)) = (inside(world, p), file_hash(world, p)) else {
            continue;
        };
        let dep = Dependency::File { path, hash };
        if !out.contains(&dep) {
            out.push(dep);
        }
    }
    out
}

/// Every tier a claim may live in: shared memory and each creature's personal tier. The short
/// tier is a cache, resting on nothing.
fn tiers(world: &Path) -> Vec<Tier> {
    let mut out = vec![Tier::Shared];
    if let Ok(rd) = std::fs::read_dir(world.join(".mnemorph/memory/creature")) {
        let mut names: Vec<String> = rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort();
        out.extend(names.into_iter().map(Tier::Personal));
    }
    out
}

/// Mark for review every live claim whose dependencies `direct` names as moved, and every claim
/// resting on one of those, transitively: each id visited once, so a cycle ends. A copy already
/// under review gets no second event; its dependents are still walked. `origin`, the claim that
/// moved, is never marked by its own move. Returns the ids marked, nearest first.
fn propagate(
    world: &Path,
    origin: Option<&str>,
    direct: impl Fn(&Note) -> Option<String>,
) -> std::io::Result<Vec<String>> {
    walk(world, origin, direct, true)
}

/// [`propagate`], walking downstream only when `downstream` is set: off, only the claims resting
/// directly on what moved are marked — the unit test that shows the walk is load-bearing.
fn walk(
    world: &Path,
    origin: Option<&str>,
    direct: impl Fn(&Note) -> Option<String>,
    downstream: bool,
) -> std::io::Result<Vec<String>> {
    let held: Vec<(Tier, BTreeMap<String, Note>)> = tiers(world)
        .into_iter()
        .map(|t| read(world, &t).map(|r| (t, fold(&r.rows))))
        .collect::<std::io::Result<_>>()?;
    let live = |n: &Note| matches!(n.status, Status::Proposed | Status::Grounded);
    let mut seen: BTreeSet<String> = origin.into_iter().map(str::to_owned).collect();
    let mut queue: VecDeque<(String, String)> = VecDeque::new();
    for (_, notes) in &held {
        for n in notes.values().filter(|n| live(n)) {
            if let Some(cause) = direct(n)
                && seen.insert(n.id.clone())
            {
                queue.push_back((n.id.clone(), cause));
            }
        }
    }
    let mut marked = Vec::new();
    while let Some((id, cause)) = queue.pop_front() {
        let mut wrote = false;
        for (tier, notes) in &held {
            let Some(n) = notes.get(&id).filter(|n| live(n)) else {
                continue;
            };
            if n.review.is_none() {
                append(
                    world,
                    tier,
                    &id,
                    By::Record,
                    Some(format!("review: {cause}")),
                    Change::Review {
                        cause: cause.clone(),
                    },
                )?;
            }
            wrote = true;
        }
        if wrote {
            marked.push(id.clone());
        }
        // Downstream: whatever rests on this claim needs review too.
        if !downstream {
            continue;
        }
        for (_, notes) in &held {
            for n in notes.values().filter(|n| live(n)) {
                let rests = n
                    .depends_on
                    .iter()
                    .any(|d| matches!(d, Dependency::Claim { id: c } if *c == id));
                if rests && seen.insert(n.id.clone()) {
                    queue.push_back((n.id.clone(), format!("rests on `{id}`, which needs review")));
                }
            }
        }
    }
    Ok(marked)
}

/// A claim or a fact moved, for `why`: every claim resting on it needs review, and every claim
/// resting on those, transitively. Returns the ids marked.
///
/// # Errors
///
/// A note file cannot be read or written.
pub fn invalidate(world: &Path, moved: Moved<'_>, why: &str) -> std::io::Result<Vec<String>> {
    let (origin, key) = match moved {
        Moved::Claim(id) => (Some(id), Dependency::Claim { id: id.to_owned() }),
        Moved::Fact(id) => (None, Dependency::Fact { id: id.to_owned() }),
        Moved::Creature(name) => (
            None,
            Dependency::Creature {
                name: name.to_owned(),
            },
        ),
    };
    propagate(world, origin, |n| {
        n.depends_on.contains(&key).then(|| why.to_owned())
    })
}

/// A landing changed `changed`, under the verdict `cited`: every claim resting on one of those
/// files whose bytes no longer hash as they did — changed or gone — needs review, and everything
/// downstream of it. A claim that recorded the new hash already, grounded by this very landing,
/// stands. Returns the ids marked.
///
/// # Errors
///
/// A note file cannot be read or written.
pub fn landed(world: &Path, changed: &[String], cited: &str) -> std::io::Result<Vec<String>> {
    let now: BTreeMap<String, Option<String>> = changed
        .iter()
        .filter_map(|p| inside(world, p).map(|rel| (rel, file_hash(world, p))))
        .collect();
    if now.is_empty() {
        return Ok(Vec::new());
    }
    propagate(world, None, |n| {
        n.depends_on.iter().find_map(|d| match d {
            Dependency::File { path, hash } => now
                .get(path)
                .filter(|h| h.as_deref() != Some(hash.as_str()))
                .map(|_| format!("`{path}` changed in {cited}")),
            _ => None,
        })
    })
}

/// How far a claim stands, from weakest to strongest: a hypothesis — said, not grounded, or
/// resting on one — a claim derived from grounded claims, a claim a route observed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    /// Said, never grounded — or resting on something that was not.
    #[default]
    Hypothesis,
    /// Grounded, and resting on other grounded claims: no stronger than they are.
    Derived,
    /// Grounded by a route, resting on no other claim.
    Observed,
}

impl Class {
    /// Its word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Class::Hypothesis => "hypothesis",
            Class::Derived => "derived",
            Class::Observed => "observed",
        }
    }
}

/// A claim's strength, in `[0, 1]`, with what made it: computed at recall, never stored.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Strength {
    /// The strength, in `[0, 1]`: 0 for a hypothesis, at most 0.25 under review, at least 0.5 for
    /// a grounded claim in good health.
    pub value: f64,
    /// Its class.
    pub class: Class,
    /// It needs review.
    pub review: bool,
    /// What its route is worth, `r`.
    pub route: f64,
    /// How many times more than once it was grounded, by distinct proofs, at most 3.
    pub corroborations: usize,
    /// Its age since last grounded, as `2^(−days/365)`.
    pub age: f64,
    /// The share of the claims it rests on that stand grounded and need no review, `h`.
    pub health: f64,
    /// The weaker claim that capped it, if one did.
    pub capped_by: Option<String>,
}

impl Strength {
    /// The breakdown, in one line, such as
    /// `observed 0.93 = 0.5 + 0.5·(0.6·r0.90 + 0.2·c0.00 + 0.1·a0.99 + 0.1·h1.00)`.
    #[must_use]
    pub fn explain(&self) -> String {
        if self.class == Class::Hypothesis && self.route == 0.0 {
            return "hypothesis 0.00: not grounded".to_owned();
        }
        #[allow(clippy::cast_precision_loss, reason = "at most 3")]
        let c = self.corroborations as f64 / 3.0;
        let head = if self.review { "0.25·" } else { "0.5 + 0.5·" };
        let mut s = format!(
            "{}{} {:.2} = {head}(0.6·r{:.2} + 0.2·c{c:.2} + 0.1·a{:.2} + 0.1·h{:.2})",
            self.class.word(),
            if self.review { ", needs review," } else { "" },
            self.value,
            self.route,
            self.age,
            self.health,
        );
        if let Some(by) = &self.capped_by {
            s.push_str(", capped by `");
            s.push_str(by);
            s.push('`');
        }
        s
    }
}

/// What a grounding's route is worth: the human's word most, then a check that ran over the diff,
/// then a second creature's independent evidence, then a commissioner's verdict. A revision the
/// gate grounded is an observed failure, worth a check; a schema or a waking rests on what it
/// distilled or on the human.
#[must_use]
pub fn route_weight(proof: Option<&str>) -> f64 {
    let p = proof.unwrap_or_default();
    let word = p.split_once(": ").map_or(p, |(w, _)| w);
    match word {
        "confirmed" => 1.0,
        "checked" => 0.9,
        "corroborated" => 0.75,
        "accepted" | "distilled" | "abstracted" => 0.6,
        _ if p.starts_with("woken by the human") => 1.0,
        // A gate's verdict: a failure observed.
        _ if p.starts_with("log.md") => 0.9,
        _ => 0.6,
    }
}

/// How fast a grounding's age discounts it, in days: slowly — a claim stops being believed because
/// the world corrected it, not because time passed.
pub const AGE_HALF_LIFE_DAYS: f64 = 365.0;

/// Every note's strength in `notes`, at `now`. A claim never stands stronger than a claim it rests
/// on — a weaker source never yields a stronger claim — and a cycle is cut where it closes.
#[must_use]
pub fn strengths<'a>(
    notes: impl IntoIterator<Item = &'a Note>,
    now: u64,
) -> BTreeMap<String, Strength> {
    let by_id: BTreeMap<&str, &Note> = notes.into_iter().fold(BTreeMap::new(), |mut m, n| {
        m.entry(n.id.as_str()).or_insert(n);
        m
    });
    let mut memo = BTreeMap::new();
    let ids: Vec<&str> = by_id.keys().copied().collect();
    for id in ids {
        let mut visiting = BTreeSet::new();
        eval(id, &by_id, &mut memo, &mut visiting, now);
    }
    memo
}

fn eval(
    id: &str,
    by_id: &BTreeMap<&str, &Note>,
    memo: &mut BTreeMap<String, Strength>,
    visiting: &mut BTreeSet<String>,
    now: u64,
) -> Option<Strength> {
    if let Some(s) = memo.get(id) {
        return Some(s.clone());
    }
    let n = by_id.get(id)?;
    if !visiting.insert(id.to_owned()) {
        return None;
    }
    let s = if n.status == Status::Grounded {
        let route = route_weight(n.proof.as_deref());
        let corroborations = n.groundings.len().saturating_sub(1).min(3);
        #[allow(clippy::cast_precision_loss, reason = "ages in days are small")]
        let days = now.saturating_sub(n.grounded_at.unwrap_or(n.born)) as f64 / 86_400_000.0;
        let age = (-days / AGE_HALF_LIFE_DAYS).exp2();
        let mut rests = 0usize;
        let mut healthy = 0usize;
        let mut cap: Option<(f64, Class, String)> = None;
        for d in &n.depends_on {
            let Dependency::Claim { id: dep } = d else {
                continue;
            };
            rests += 1;
            // A claim out of sight — another creature's personal note — caps nothing, and is not
            // known to be healthy.
            let Some(ds) = eval(dep, by_id, memo, visiting, now) else {
                continue;
            };
            if ds.class != Class::Hypothesis && !ds.review {
                healthy += 1;
            }
            if cap.as_ref().is_none_or(|(v, _, _)| ds.value < *v) {
                cap = Some((ds.value, ds.class, dep.clone()));
            }
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "counts of dependencies are small"
        )]
        let health = if rests == 0 {
            1.0
        } else {
            healthy as f64 / rests as f64
        };
        #[allow(clippy::cast_precision_loss, reason = "at most 3")]
        let c = corroborations as f64 / 3.0;
        let quality = 0.6 * route + 0.2 * c + 0.1 * age + 0.1 * health;
        let review = n.review.is_some();
        let own = if review {
            0.25 * quality
        } else {
            0.5 + 0.5 * quality
        };
        let mut class = if rests == 0 {
            Class::Observed
        } else {
            Class::Derived
        };
        let mut value = own;
        let mut capped_by = None;
        if let Some((v, c, by)) = cap {
            class = class.min(c);
            if v < own {
                value = v;
                capped_by = Some(by);
            }
        }
        if value <= 0.0 {
            class = Class::Hypothesis;
        }
        Strength {
            value,
            class,
            review,
            route,
            corroborations,
            age,
            health,
            capped_by,
        }
    } else {
        Strength {
            value: 0.0,
            class: Class::Hypothesis,
            review: n.review.is_some(),
            route: 0.0,
            corroborations: 0,
            age: 0.0,
            health: 0.0,
            capped_by: None,
        }
    };
    visiting.remove(id);
    memo.insert(id.to_owned(), s.clone());
    Some(s)
}

/// The claims touching `owner` — its own tier, and shared claims it owns — that need review, each
/// with its text and why: what `memory status` lists for the human, who may ground one again.
///
/// # Errors
///
/// A note file cannot be read.
pub fn under_review(world: &Path, owner: &str) -> std::io::Result<Vec<(String, String, String)>> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for tier in [Tier::Personal(owner.to_owned()), Tier::Shared] {
        for n in fold(&read(world, &tier)?.rows).into_values() {
            let Some(cause) = &n.review else {
                continue;
            };
            if n.owner.as_deref() != Some(owner) || out.iter().any(|(id, _, _)| *id == n.id) {
                continue;
            }
            out.push((n.id.clone(), n.text.clone(), cause.clone()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write::propose_resting;

    /// The walk downstream is what marks a claim resting on a claim resting on what moved: turned
    /// off, the far end is never marked — the transitive case fails without it.
    #[test]
    fn without_the_walk_downstream_the_far_end_is_never_marked() {
        let w = std::env::temp_dir().join(format!("mnemorph-graph-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        let claim = |text: &str, on: Vec<Dependency>| {
            propose_resting(&w, "slime-s", text, vec![], None, vec![], on)
                .unwrap()
                .note
        };
        let base = claim("the base holds", vec![]);
        let near = claim(
            "near rests on it",
            vec![Dependency::Claim { id: base.clone() }],
        );
        let far = claim(
            "far rests on near",
            vec![Dependency::Claim { id: near.clone() }],
        );
        let key = Dependency::Claim { id: base.clone() };
        let direct = |n: &Note| n.depends_on.contains(&key).then(|| "moved".to_owned());
        let off = walk(&w, Some(&base), direct, false).unwrap();
        assert_eq!(off, vec![near.clone()], "off: the far end stays unmarked");
        let on = propagate(&w, Some(&base), direct).unwrap();
        assert_eq!(on, vec![near, far], "on: the walk reaches it");
        let _ = std::fs::remove_dir_all(&w);
    }
}
