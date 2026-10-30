//! A world's memory, as `GET /api/w/<id>/memory` shows it: every note by creature, scope and
//! standing, with the route and proof its standing rests on and its place in the claim graph —
//! its version, what it rests on, whether it waits for review — the notes the gate retired, and
//! the events whose author or proof does not resolve. And the human's two routes: **confirm** (the
//! confirmed route, `mnemorphd memory confirm`) and **wake** (`mnemorphd memory wake`).
//!
//! Every read is the CLI's own function over a capped copy of the world's memory
//! ([`super::snapshot`]): memory lives inside the world, which a body can write, so it is shown and
//! grants nothing. An act calls the CLI's function on the world itself, after the server checks
//! again what the page showed: the note still reads as the human saw it, and the files it writes
//! are no links.

use std::collections::BTreeMap;
use std::path::Path;

use mnemorph_loop::world::memory::{
    self as m, Dependency, Note, Status, Tier, audit::SHARED, pathology,
};
use serde::Serialize;
use ts_rs::TS;

use super::overview::Memory;
use super::snapshot::{self, Snapshot};

/// The most notes one page holds.
pub const PAGE_CAP: u32 = 500;
/// The notes a page holds unless asked otherwise.
pub const PAGE: u32 = 100;
/// The most characters of a note's text shown.
pub const TEXT_CAP: usize = 2000;
/// The most earlier versions shown of a note.
const VERSIONS: usize = 10;
/// The most unproven events listed per creature.
const UNPROVEN_CAP: usize = 200;

/// Where a note lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum NoteScope {
    /// A creature's own tier.
    Personal,
    /// Shared memory: the only notes other bodies recall.
    Shared,
}

/// A note's standing now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum NoteStanding {
    /// Said, not proved.
    Proposed,
    /// Proved by a route.
    Grounded,
    /// Retired by the gate: kept dormant for the human to review, and to wake when it was wrong.
    Retired,
    /// Put to sleep otherwise: kept, not recalled.
    Dormant,
    /// Gone from recall for good.
    Tombstoned,
}

/// One earlier version of a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct NoteVersion {
    /// Its number, from 1.
    pub n: u32,
    /// When, in milliseconds since the epoch.
    #[ts(type = "number")]
    pub time: u64,
    /// What it said, cut at [`TEXT_CAP`] characters.
    pub text: String,
}

/// What a claim rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct NoteDependency {
    /// `claim`, `fact`, `creature` or `file`.
    pub on: String,
    /// The claim's or fact's id, the creature's name, the file's path.
    pub what: String,
}

/// One note in one tier.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct MemoryNote {
    /// Its fixed id.
    pub id: String,
    /// The creature whose note it is; for a shared note with no owner, the shared row's name.
    pub creature: String,
    /// The tier it was read in.
    pub scope: NoteScope,
    /// Its standing.
    pub standing: NoteStanding,
    /// What it says, cut at [`TEXT_CAP`] characters.
    pub text: String,
    /// Whether the text was cut.
    pub cut: bool,
    /// The route that grounded it — `checked`, `accepted`, `corroborated`, `confirmed` — when its
    /// proof names one.
    pub route: Option<String>,
    /// The proof its standing rests on, as written.
    pub proof: Option<String>,
    /// How sure, in `[0, 1]`.
    pub confidence: f64,
    /// Its version now, from 1.
    pub version: u32,
    /// Its earlier versions, the latest last (at most ten).
    pub versions: Vec<NoteVersion>,
    /// What its current version rests on.
    pub depends: Vec<NoteDependency>,
    /// Why it needs review — something it rests on moved — while it does.
    pub review: Option<String>,
    /// The distinct proofs that grounded its current version.
    pub groundings: Vec<String>,
    /// When it was born, in milliseconds since the epoch.
    #[ts(type = "number")]
    pub born: u64,
    /// The SHA-256 of its id and full text: an act names it, so the server acts on the note the
    /// human read, never on one rewritten since.
    pub seen: String,
    /// Whether the human may confirm it: a proposed claim in its owner's tier, or one grounded and
    /// waiting under review — as `mnemorphd memory confirm` would take it.
    pub confirm: bool,
    /// Whether the human may wake it: the gate retired it.
    pub wake: bool,
}

/// An event whose author or proof does not resolve: its creature's writes stay frozen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct UnprovenEvent {
    /// The tier's file, relative to the world.
    pub file: String,
    /// The event's id.
    pub event: String,
    /// The note it is about.
    pub note: String,
    /// Why it does not resolve.
    pub why: String,
}

/// One creature's memory, as a row of `mnemorphd memory status --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct MemoryCreature {
    /// The creature; the shared row's name for shared notes with no owner.
    pub creature: String,
    /// Whether its memory writes are frozen by an unproven event.
    pub frozen: bool,
    /// Its unproven events (at most 200 listed; `unproven` counts them all).
    pub events: Vec<UnprovenEvent>,
    /// How many unproven events touch it.
    pub unproven: u32,
    /// What its recall traces show: a replay loop, a dissent never read.
    pub pathologies: Vec<String>,
    /// Its claims proposed, waiting for a route.
    pub proposed: u32,
    /// Its notes the gate retired.
    pub retired: u32,
    /// Its claims that need review.
    pub review: u32,
}

/// `GET /api/w/<id>/memory`: the standing, each creature's row, and a page of notes.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BoardMemory {
    /// Counted as `mnemorphd memory status` counts it, as the overview shows it.
    pub standing: Memory,
    /// Each creature's row, by name, the shared row last.
    pub creatures: Vec<MemoryCreature>,
    /// The notes of this page, the newest first.
    pub notes: Vec<MemoryNote>,
    /// How many notes the filter matches in all.
    pub total: u32,
    /// Where this page starts.
    pub offset: u32,
    /// How many a page holds.
    pub limit: u32,
    /// Where it was read: inside the world, so shown only.
    pub source: String,
}

/// What a page of notes is filtered by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// One creature's notes.
    pub creature: Option<String>,
    /// One scope.
    pub scope: Option<NoteScope>,
    /// One standing.
    pub standing: Option<NoteStanding>,
    /// Only the notes waiting for review (`true`), or none of them (`false`).
    pub review: Option<bool>,
    /// The first note of the page.
    pub offset: u32,
    /// How many.
    pub limit: u32,
}

impl Filter {
    /// The filter a query string asks for; `Err` names a parameter that does not read.
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
                ("creature", c) if !c.is_empty() => f.creature = Some(c.to_owned()),
                ("scope", "personal") => f.scope = Some(NoteScope::Personal),
                ("scope", "shared") => f.scope = Some(NoteScope::Shared),
                ("standing", s) => {
                    f.standing = Some(match s {
                        "proposed" => NoteStanding::Proposed,
                        "grounded" => NoteStanding::Grounded,
                        "retired" => NoteStanding::Retired,
                        "dormant" => NoteStanding::Dormant,
                        "tombstoned" => NoteStanding::Tombstoned,
                        _ => return Err(format!("`standing` is no standing: {s}")),
                    });
                }
                ("review", "yes") => f.review = Some(true),
                ("review", "no") => f.review = Some(false),
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
                _ => return Err(format!("`{k}={v}` is no filter of the memory page")),
            }
        }
        Ok(f)
    }

    fn keeps(&self, n: &MemoryNote) -> bool {
        self.creature.as_ref().is_none_or(|c| *c == n.creature)
            && self.scope.is_none_or(|s| s == n.scope)
            && self.standing.is_none_or(|s| s == n.standing)
            && self.review.is_none_or(|r| r == n.review.is_some())
    }
}

/// A count, as the protocol carries it.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The SHA-256 of a note's id and text, in hex.
#[must_use]
pub fn seen(id: &str, text: &str) -> String {
    m::seen(id, text)
}

/// `text`, cut at `cap` characters, and whether it was.
fn cut(text: &str, cap: usize) -> (String, bool) {
    match text.char_indices().nth(cap) {
        Some((at, _)) => (text[..at].to_owned(), true),
        None => (text.to_owned(), false),
    }
}

/// The route a proof names: the word before its `: `, when it is a route's.
fn route_of(proof: Option<&str>) -> Option<String> {
    let (word, _) = proof?.split_once(": ")?;
    ["checked", "accepted", "corroborated", "confirmed"]
        .contains(&word)
        .then(|| word.to_owned())
}

fn dependency(d: &Dependency) -> NoteDependency {
    let (on, what) = match d {
        Dependency::Claim { id } => ("claim", id.clone()),
        Dependency::Fact { id } => ("fact", id.clone()),
        Dependency::Creature { name } => ("creature", name.clone()),
        Dependency::File { path, .. } => ("file", path.clone()),
    };
    NoteDependency {
        on: on.to_owned(),
        what,
    }
}

/// The creatures with a personal tier in the memory at `path`, by name.
fn personal(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(path.join(".mnemorph/memory/creature"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Every tier's events and notes in the copy: shared memory first, then each creature's own.
type Tiers = Vec<(Tier, Vec<m::Event>, BTreeMap<String, Note>)>;

fn tiers(path: &Path) -> Result<Tiers, String> {
    let mut all = vec![Tier::Shared];
    all.extend(personal(path).into_iter().map(Tier::Personal));
    all.into_iter()
        .map(|t| {
            let rows = m::read(path, &t)
                .map_err(|e| format!("{}: {e}", t.file().display()))?
                .rows;
            let notes = m::fold(&rows);
            Ok((t, rows, notes))
        })
        .collect()
}

/// Copy into `snap` every file a claim rests on, so the copy's review reads what the world holds.
fn rest_on(snap: &mut Snapshot, world: &Path, tiers: &Tiers) {
    for (_, _, notes) in tiers {
        for n in notes.values() {
            for d in &n.depends_on {
                if let Dependency::File { path, .. } = d {
                    snap.also(world, path);
                }
            }
        }
    }
}

/// The notes of every tier, as the page lists them, the newest first.
fn notes(path: &Path, tiers: &Tiers) -> Vec<MemoryNote> {
    let mut out = Vec::new();
    for (tier, rows, notes) in tiers {
        let scope = match tier {
            Tier::Shared => NoteScope::Shared,
            _ => NoteScope::Personal,
        };
        let held = match tier {
            Tier::Personal(name) => Some(name.as_str()),
            _ => None,
        };
        // What `memory confirm` takes: the review as written, before the files are read afresh.
        let confirmable: Vec<String> = notes
            .values()
            .filter(|n| {
                held.is_some()
                    && n.owner.as_deref() == held
                    && (n.status == Status::Proposed
                        || (n.status == Status::Grounded && n.review.is_some()))
            })
            .map(|n| n.id.clone())
            .collect();
        let mut live: Vec<Note> = notes.values().cloned().collect();
        mnemorph_loop::world::memory::graph::recheck(path, live.iter_mut());
        for n in live {
            let retired = n.status == Status::Dormant && m::retired_in(rows, &n.id);
            let standing = match n.status {
                Status::Proposed => NoteStanding::Proposed,
                Status::Grounded => NoteStanding::Grounded,
                Status::Dormant if retired => NoteStanding::Retired,
                Status::Dormant => NoteStanding::Dormant,
                Status::Tombstoned => NoteStanding::Tombstoned,
            };
            let (text, was_cut) = cut(&n.text, TEXT_CAP);
            let skip = n.versions.len().saturating_sub(VERSIONS + 1);
            out.push(MemoryNote {
                creature: n
                    .owner
                    .clone()
                    .or_else(|| held.map(str::to_owned))
                    .unwrap_or_else(|| SHARED.to_owned()),
                scope,
                standing,
                route: route_of(n.proof.as_deref()),
                proof: n.proof.clone(),
                confidence: n.confidence,
                version: n.version,
                versions: n
                    .versions
                    .iter()
                    .filter(|v| v.n != n.version)
                    .skip(skip)
                    .map(|v| NoteVersion {
                        n: v.n,
                        time: v.time,
                        text: cut(&v.text, TEXT_CAP).0,
                    })
                    .collect(),
                depends: n.depends_on.iter().map(dependency).collect(),
                review: n.review.clone(),
                groundings: n.groundings.clone(),
                born: n.born,
                seen: seen(&n.id, &n.text),
                confirm: confirmable.contains(&n.id),
                wake: retired,
                cut: was_cut,
                text,
                id: n.id,
            });
        }
    }
    out.sort_by(|a, b| b.born.cmp(&a.born).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Each creature's row, as `mnemorphd memory status --json` builds it: by name, the shared row
/// last.
fn creatures(path: &Path) -> Result<Vec<MemoryCreature>, String> {
    let report = m::status(path).map_err(|e| format!("memory cannot be read: {e}"))?;
    let mut rows: Vec<(String, Vec<m::Unproven>)> = report;
    for n in pathology::traced(path) {
        if !rows.iter().any(|(r, _)| *r == n) {
            rows.push((n, Vec::new()));
        }
    }
    rows.sort_by(|a, b| (a.0 == SHARED, &a.0).cmp(&(b.0 == SHARED, &b.0)));
    Ok(rows
        .into_iter()
        .map(|(name, found)| {
            let mine = name != SHARED;
            let of = |n: usize| if mine { count(n) } else { 0 };
            MemoryCreature {
                frozen: !found.is_empty() && mine,
                unproven: count(found.len()),
                events: found
                    .into_iter()
                    .take(UNPROVEN_CAP)
                    .map(|u| UnprovenEvent {
                        file: u.file,
                        event: u.event,
                        note: u.note,
                        why: u.why,
                    })
                    .collect(),
                pathologies: if mine {
                    pathology::findings(&pathology::traces(path, &name).unwrap_or_default())
                } else {
                    Vec::new()
                },
                proposed: of(m::proposals(path, &name)
                    .map(|p| p.len())
                    .unwrap_or_default()),
                retired: of(m::retired(path, &name).map(|r| r.len()).unwrap_or_default()),
                review: of(m::under_review(path, &name)
                    .map(|r| r.len())
                    .unwrap_or_default()),
                creature: name,
            }
        })
        .collect())
}

/// The memory of the world at `world`, filtered and paged by `filter`.
///
/// # Errors
///
/// Why it cannot be read: the copy refused a file (a link, a pipe, one too large, too many), or a
/// tier does not read.
pub fn memory(world: &Path, filter: &Filter) -> Result<BoardMemory, String> {
    let mut snap = snapshot::take(world)?;
    let path = snap.path().to_path_buf();
    let all = tiers(&path)?;
    rest_on(&mut snap, world, &all);
    let s = m::standing(&path);
    let rows = creatures(&path)?;
    let listed = notes(&path, &all);
    let unproven: u32 = rows.iter().map(|r| r.unproven).sum();
    let retired: u32 = rows.iter().map(|r| r.retired).sum();
    let kept: Vec<MemoryNote> = listed.into_iter().filter(|n| filter.keeps(n)).collect();
    let total = count(kept.len());
    let notes = kept
        .into_iter()
        .skip(filter.offset as usize)
        .take(filter.limit as usize)
        .collect();
    drop(snap);
    Ok(BoardMemory {
        standing: Memory {
            shared: count(s.shared),
            grounded: count(s.grounded),
            proposed: count(s.proposed),
            retired,
            unproven,
        },
        creatures: rows,
        notes,
        total,
        offset: filter.offset,
        limit: filter.limit,
        source: ".mnemorph/memory/ (as `mnemorphd memory status` reads it)".to_owned(),
    })
}

/// The human's two routes on a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// The confirmed route: a proposed claim (or one under review) grounded and shared.
    Confirm,
    /// A note the gate retired, grounded again.
    Wake,
}

/// What the journal says the human's word was, from the board.
pub const BOARD_WORD: &str = "the human's word (board)";

/// What a wake from the board cites.
pub const BOARD_WAKE: &str = "woken by the human (board)";

/// Do `act` on the note `id` of `world`, which the human read as `seen`: checked again on a fresh
/// copy — the note is there, the act applies to it, its text is the one read — then done by the
/// CLI's own function on the world. Returns what was done, in words.
///
/// # Errors
///
/// `(status, why)`: 404 for no such note, 409 when the act does not apply or the note changed
/// since it was read, 422 for a write the world refuses (frozen memory, a file that is a link), 503
/// when the memory cannot be read.
pub fn act(world: &Path, id: &str, seen_as: &str, act: Act) -> Result<String, (u16, String)> {
    let snap = snapshot::take(world).map_err(|e| (503, e))?;
    let all = tiers(snap.path()).map_err(|e| (503, e))?;
    let listed = notes(snap.path(), &all);
    drop(snap);
    let mine: Vec<&MemoryNote> = listed.iter().filter(|n| n.id == id).collect();
    if mine.is_empty() {
        return Err((404, format!("no note `{id}` in this world's memory")));
    }
    let target = mine.iter().find(|n| match act {
        Act::Confirm => n.confirm,
        Act::Wake => n.wake,
    });
    let Some(target) = target else {
        return Err((
            409,
            match act {
                Act::Confirm => format!(
                    "`{id}` is no proposed claim, nor one waiting under review: there is nothing \
                     to confirm"
                ),
                Act::Wake => {
                    format!("`{id}` is no note the gate retired: only a retirement is woken")
                }
            },
        ));
    };
    if target.seen != seen_as {
        return Err((409, m::changed(id)));
    }
    let mut files = vec![".mnemorph/memory/shared/notes.jsonl".to_owned()];
    files.extend(
        personal(world)
            .into_iter()
            .map(|n| format!(".mnemorph/memory/creature/{n}.jsonl")),
    );
    super::knowledge::no_links(world, &files).map_err(|e| (422, e))?;
    match act {
        Act::Confirm => m::vouch(world, id, BOARD_WORD, seen_as)
            .map(|owner| format!("confirmed {id}: {owner}'s claim, grounded and shared"))
            .map_err(|e| (raced(id, &e), e)),
        Act::Wake => m::wake_by(world, id, BOARD_WAKE, seen_as)
            .map(|files| format!("woke {id}: grounded again in {}", files.join(", ")))
            .map_err(|e| {
                let e = e.to_string();
                (raced(id, &e), e)
            }),
    }
}

/// The status of a refused act on `id`: 409 when the note or fact changed between the copy's
/// check and the write, else 422.
#[must_use]
pub fn raced(id: &str, why: &str) -> u16 {
    if why == m::changed(id) { 409 } else { 422 }
}
