//! A world's ontology, as `GET /api/w/<id>/facts` shows it: the facts by status — settled, agreed,
//! single, contested, countered, the partition the facts table reads (`FactStatus`) — each with
//! who supports it and on what anchor, who counters it and why; what `mnemorphd onto check` finds;
//! and who owns each path a fact is about. And the human's act on a fact: **settle**, with a proof
//! — a passing end-gate verdict that landed on the fact's path, or the human's own statement.
//!
//! The facts journal and the creature docs are read from a capped copy of the world
//! ([`super::snapshot`]); the check's other reads open only regular files under a cap
//! (`mnemorph_world::regular`). All of it lives inside the world, so it is shown and grants
//! nothing — but for the gate's verdicts, which the harness keeps under the state directory,
//! chained, and which alone may prove a settle besides the human.

use std::collections::BTreeSet;
use std::path::Path;

use mnemorph_loop::world::facts::{Event, FACTS, Fact};
use mnemorph_loop::world::ledger::Journal;
use mnemorph_loop::world::{FactStatus, Facts, Owner, Roster, explain, onto_check, verdicts};
use serde::Serialize;
use ts_rs::TS;

use super::memory::seen;
use super::snapshot;

/// The most facts one page holds.
pub const PAGE_CAP: u32 = 500;
/// The facts a page holds unless asked otherwise.
pub const PAGE: u32 = 100;
/// The most findings listed.
pub const FINDINGS_CAP: usize = 500;
/// The most passing verdicts offered as a settle's proof.
const VERDICTS_CAP: usize = 50;
/// The most characters of a human's statement.
pub const STATEMENT_CAP: usize = 1000;

/// A fact's status, by the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum BoardFactStatus {
    /// A proof settled it; final until a newer counter.
    Settled,
    /// Named twice, one dissenter in three tolerated.
    Agreed,
    /// One voice.
    Single,
    /// A counter and support neither side carries: a check settles it, never recency.
    Contested,
    /// The counters carry it.
    Countered,
}

impl From<FactStatus> for BoardFactStatus {
    fn from(s: FactStatus) -> Self {
        match s {
            FactStatus::Settled => Self::Settled,
            FactStatus::Agreed => Self::Agreed,
            FactStatus::Single => Self::Single,
            FactStatus::Contested => Self::Contested,
            FactStatus::Countered => Self::Countered,
        }
    }
}

/// How many facts stand in each status: every live fact in exactly one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FactCounts {
    /// Settled.
    pub settled: u32,
    /// Agreed.
    pub agreed: u32,
    /// Single.
    pub single: u32,
    /// Contested.
    pub contested: u32,
    /// Countered.
    pub countered: u32,
}

/// What a voice rests on: a `file:line`, a directory, a commit, a journal entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FactAnchor {
    /// Where it points.
    pub at: String,
    /// Whether what it points at moved since it was said.
    pub stale: bool,
}

/// One voice on a fact: a supporter with its anchor, or a counter with its reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FactSide {
    /// The creature.
    pub creature: String,
    /// The anchor it gave, if any (a supporter's).
    pub anchor: Option<FactAnchor>,
    /// Why it counters (a counter's).
    pub reason: Option<String>,
}

/// Who owns a path, as `mnemorphd onto owner` says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PathOwner {
    /// The path.
    pub path: String,
    /// A creature's name, `the harness`, `the human` or `nobody`.
    pub owner: String,
    /// By which rule, and the claim that matched.
    pub why: String,
}

/// One live fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardFact {
    /// Its id.
    pub id: String,
    /// The path it is about.
    pub about: String,
    /// What it says.
    pub text: String,
    /// Its status.
    pub status: BoardFactStatus,
    /// Who supports it, each with the anchor it gave.
    pub supporters: Vec<FactSide>,
    /// Who counters it, each with why.
    pub counters: Vec<FactSide>,
    /// Its latest evidence.
    pub anchor: Option<FactAnchor>,
    /// Who settled it and on what proof, while it stands settled.
    pub settled: Option<String>,
    /// Who owns the path it is about.
    pub owner: PathOwner,
    /// The SHA-256 of its id and text: a settle names it, so the server settles the fact the
    /// human read.
    pub seen: String,
}

/// One finding of `onto check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FactFinding {
    /// `low`, `medium`, `high` or `critical`.
    pub severity: String,
    /// Its code: `contested-fact`, `overlap`, ….
    pub code: String,
    /// Where it stands.
    pub at: String,
    /// What it says.
    pub message: String,
}

/// A passing end-gate verdict, offered as a settle's proof: kept by the harness, chained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ProofVerdict {
    /// Its number in the gate's journal, from 1: what a settle names.
    pub n: u32,
    /// When (RFC 3339, UTC).
    pub at: String,
    /// The body whose landing it judged.
    pub body: String,
    /// The paths the landing changed.
    pub changed: Vec<String>,
}

/// `GET /api/w/<id>/facts`: the partition's counts, a page of facts, the check's findings, the
/// paths' owners, and the verdicts a settle may cite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardFacts {
    /// How many live facts stand in each status.
    pub counts: FactCounts,
    /// The facts of this page, by status then path.
    pub facts: Vec<BoardFact>,
    /// How many facts the filter matches in all.
    pub total: u32,
    /// Where this page starts.
    pub offset: u32,
    /// How many a page holds.
    pub limit: u32,
    /// Whether `onto check` fails: a critical or high finding, or a doc that does not read.
    pub failed: bool,
    /// Its findings, most severe first (at most 500).
    pub findings: Vec<FactFinding>,
    /// How many findings in all.
    pub found: u32,
    /// What does not read in the roster: each makes the check fail.
    pub problems: Vec<String>,
    /// Who owns each path a fact is about.
    pub owners: Vec<PathOwner>,
    /// The latest passing end-gate verdicts, newest first: what a settle may cite.
    pub verdicts: Vec<ProofVerdict>,
    /// Whether the world is a git repository: without one, unowned soil is not checked.
    pub git: bool,
    /// Where it was read.
    pub source: String,
}

/// What a page of facts is filtered by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// One status.
    pub status: Option<BoardFactStatus>,
    /// The first fact of the page.
    pub offset: u32,
    /// How many.
    pub limit: u32,
}

impl Filter {
    /// The filter a query string asks for.
    ///
    /// # Errors
    ///
    /// An unknown parameter, or a value that is not one of its words.
    pub fn parse(query: Option<&str>) -> Result<Self, String> {
        let mut f = Filter {
            limit: PAGE,
            ..Filter::default()
        };
        for (k, v) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
            match (&*k, &*v) {
                ("status", s) => {
                    f.status = Some(match s {
                        "settled" => BoardFactStatus::Settled,
                        "agreed" => BoardFactStatus::Agreed,
                        "single" => BoardFactStatus::Single,
                        "contested" => BoardFactStatus::Contested,
                        "countered" => BoardFactStatus::Countered,
                        _ => return Err(format!("`status` is no fact status: {s}")),
                    });
                }
                ("offset" | "limit", n) => {
                    let n: u32 = n
                        .parse()
                        .map_err(|_| format!("`{k}` is a whole number, not {n}"))?;
                    if k == "offset" {
                        f.offset = n;
                    } else {
                        f.limit = n.clamp(1, PAGE_CAP);
                    }
                }
                _ => return Err(format!("`{k}={v}` is no filter of the ontology page")),
            }
        }
        Ok(f)
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Who owns `path`, as `mnemorphd onto owner` says it.
fn owner(roster: &Roster, path: &str) -> PathOwner {
    let e = explain(roster, path);
    let owner = match &e.owner {
        Owner::Creature(name) => name.clone(),
        Owner::Harness => "the harness".into(),
        Owner::Human => "the human".into(),
        Owner::Unowned => "nobody".into(),
    };
    let why = e.claim.map_or_else(
        || e.rule.words().to_owned(),
        |c| format!("{} {c}", e.rule.words()),
    );
    PathOwner {
        path: path.to_owned(),
        owner,
        why,
    }
}

/// The anchor `at` with whether it moved, against the world.
fn anchor(world: &Path, a: &mnemorph_loop::world::facts::Anchor) -> FactAnchor {
    let stale = a
        .digest
        .as_ref()
        .is_some_and(|d| mnemorph_loop::world::facts::digest(world, &a.at).as_ref() != Some(d));
    FactAnchor {
        at: a.at.clone(),
        stale,
    }
}

/// The fact `f` as the page shows it: its voices from `events`, the ids merged into it included.
fn shown(world: &Path, roster: &Roster, facts: &Facts, events: &[Event], f: &Fact) -> BoardFact {
    let ids: BTreeSet<&str> = facts
        .facts
        .values()
        .filter(|o| o.merged_into.as_deref() == Some(f.id.as_str()))
        .map(|o| o.id.as_str())
        .chain([f.id.as_str()])
        .collect();
    let mut anchors: std::collections::BTreeMap<&str, FactAnchor> =
        std::collections::BTreeMap::new();
    let mut settled = None;
    for e in events {
        match e {
            Event::Support {
                id,
                by,
                source: Some(a),
                ..
            } if ids.contains(id.as_str()) => {
                anchors.insert(by.as_str(), anchor(world, a));
            }
            Event::Settle { id, by, proof } if ids.contains(id.as_str()) => {
                settled = Some(format!("{by}: {proof}"));
            }
            _ => {}
        }
    }
    BoardFact {
        id: f.id.clone(),
        about: f.about.clone(),
        text: f.text.clone(),
        status: f.status().into(),
        supporters: f
            .supporters
            .iter()
            .map(|s| FactSide {
                creature: s.clone(),
                anchor: anchors.get(s.as_str()).cloned(),
                reason: None,
            })
            .collect(),
        counters: f
            .counters
            .iter()
            .map(|c| FactSide {
                creature: c.clone(),
                anchor: None,
                reason: f.reasons.get(c).cloned(),
            })
            .collect(),
        anchor: f.source.as_ref().map(|a| anchor(world, a)),
        settled: if f.settled { settled } else { None },
        owner: owner(roster, &f.about),
        seen: seen(&f.id, &f.text),
    }
}

/// The passing verdicts of the gate on `world`, newest first, with their numbers.
fn passing(world: &Path) -> Vec<ProofVerdict> {
    verdicts::read(world)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter(|(_, r)| r.verdict == "PASS")
        .map(|(i, r)| ProofVerdict {
            n: count(i + 1),
            at: r.ts.to_string(),
            body: r.body,
            changed: r.changed,
        })
        .rev()
        .take(VERDICTS_CAP)
        .collect()
}

/// The facts of the copy at `path`, and every event of their journal.
fn read(path: &Path) -> Result<(Facts, Vec<Event>), String> {
    let back = Journal::<Event>::new(path.join(FACTS))
        .read()
        .map_err(|e| format!("{FACTS}: {e}"))?;
    Ok((Facts::fold(&back.rows), back.rows))
}

/// The ontology of the world at `world`, filtered and paged by `filter`.
///
/// # Errors
///
/// Why it cannot be read: the copy refused a file, the facts journal or the manifest of
/// relations does not read.
pub fn facts(world: &Path, filter: &Filter) -> Result<BoardFacts, String> {
    let snap = snapshot::ontology(world)?;
    let (facts, events) = read(snap.path())?;
    let roster = Roster::load(snap.path());
    let tracked = crate::tracked(world);
    let git = tracked.is_some();
    let (findings, _) = onto_check(world, &roster, &facts, &tracked.unwrap_or_default(), false)?;
    let failed = !roster.problems.is_empty() || findings.iter().any(|f| f.severity.blocks());
    let mut live: Vec<&Fact> = facts.live().collect();
    live.sort_by(|a, b| {
        BoardFactStatus::from(a.status())
            .cmp(&b.status().into())
            .then_with(|| a.about.cmp(&b.about))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut counts = FactCounts::default();
    for f in &live {
        let n = match f.status() {
            FactStatus::Settled => &mut counts.settled,
            FactStatus::Agreed => &mut counts.agreed,
            FactStatus::Single => &mut counts.single,
            FactStatus::Contested => &mut counts.contested,
            FactStatus::Countered => &mut counts.countered,
        };
        *n += 1;
    }
    let kept: Vec<&Fact> = live
        .iter()
        .copied()
        .filter(|f| {
            filter
                .status
                .is_none_or(|s| s == BoardFactStatus::from(f.status()))
        })
        .collect();
    let paths: BTreeSet<&str> = live.iter().map(|f| f.about.as_str()).collect();
    Ok(BoardFacts {
        counts,
        total: count(kept.len()),
        facts: kept
            .into_iter()
            .skip(filter.offset as usize)
            .take(filter.limit as usize)
            .map(|f| shown(world, &roster, &facts, &events, f))
            .collect(),
        offset: filter.offset,
        limit: filter.limit,
        failed,
        found: count(findings.len()),
        findings: findings
            .into_iter()
            .take(FINDINGS_CAP)
            .map(|f| FactFinding {
                severity: f.severity.word().to_owned(),
                code: f.code.to_owned(),
                at: f.at,
                message: f.message,
            })
            .collect(),
        problems: roster.problems.clone(),
        owners: paths.into_iter().map(|p| owner(&roster, p)).collect(),
        verdicts: passing(world),
        git,
        source: format!("{FACTS} (as `mnemorphd onto check` reads it)"),
    })
}

/// A settle's proof, as the human gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
    /// A passing end-gate verdict, by its number in the gate's journal.
    Verdict(u32),
    /// The human's own statement.
    Statement(String),
}

/// Settle the fact `id` of `world`, which the human read as `seen_as`, on `proof`: checked again
/// on a fresh copy — the fact is live and its text the one read — and the proof held to its rule,
/// then filed by the CLI's own function, `by: human (board)`.
///
/// # Errors
///
/// `(status, why)`: 400 for a statement empty or too long, 404 for no such fact, 409 when it
/// changed since it was read, 422 for a verdict that proves nothing of it or a write the world
/// refuses, 503 when the ontology cannot be read.
pub fn settle(
    world: &Path,
    id: &str,
    seen_as: &str,
    proof: &Proof,
) -> Result<String, (u16, String)> {
    let snap = snapshot::ontology(world).map_err(|e| (503, e))?;
    let (facts, _) = read(snap.path()).map_err(|e| (503, e))?;
    drop(snap);
    let Some(f) = facts.live().find(|f| f.id == id) else {
        return Err((404, format!("no fact `#{id}` in this world's ontology")));
    };
    if seen(&f.id, &f.text) != seen_as {
        return Err((
            409,
            format!("#{id} changed since it was read: read it again before you settle it"),
        ));
    }
    let said = match proof {
        Proof::Statement(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Err((
                    400,
                    "a settle needs a proof: your statement is empty".into(),
                ));
            }
            if s.chars().count() > STATEMENT_CAP {
                return Err((
                    400,
                    format!("a statement is at most {STATEMENT_CAP} characters"),
                ));
            }
            format!("the human's word (board): {s}")
        }
        Proof::Verdict(n) => {
            let rows =
                verdicts::read(world).map_err(|e| (503, format!("the gate's verdicts: {e}")))?;
            let row = (*n as usize)
                .checked_sub(1)
                .and_then(|i| rows.get(i))
                .ok_or_else(|| (422, format!("the gate gave no verdict {n}")))?;
            if row.verdict != "PASS" {
                return Err((
                    422,
                    format!(
                        "verdict {n} is `{}`: only a pass proves a fact",
                        row.verdict
                    ),
                ));
            }
            let about = f.about.trim_end_matches('/');
            let landed = row
                .changed
                .iter()
                .any(|p| about.is_empty() || p == about || p.starts_with(&format!("{about}/")));
            if !landed {
                return Err((
                    422,
                    format!(
                        "verdict {n} judged a landing that changed nothing at `{}`: it proves \
                         nothing of #{id}",
                        f.about
                    ),
                ));
            }
            format!(
                "the gate's verdict {n}: PASS on {}'s landing ({})",
                row.body, row.ts
            )
        }
    };
    super::knowledge::no_links(world, &[FACTS.to_owned()]).map_err(|e| (422, e))?;
    Facts::settle(world, id, "human (board)", &said)
        .map(|id| format!("settled #{id}"))
        .map_err(|e| (422, e))
}
