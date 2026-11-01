//! The creatures' witness: who each creature is — its race and rank, whom it reports to, its post
//! or speciality, its ground — kept by the harness outside the world at
//! `$XDG_STATE_HOME/mnemorph/roster/<world>-<fnv64a>.jsonl`, chained by hash
//! ([`mnemorph_ledger::chain`]) and anchored as `roster` with the world's other chained journals
//! ([`crate::chains`]), from the state directory alone.
//!
//! A creature's card lives in the world, and the world is the body's to write — a commit, a merge,
//! a pull — and a cloned repository's cards are a stranger's from the start. So a card grants
//! nothing because it says it: every privilege reads the trusted view ([`crate::Roster::load`]),
//! the witness's row matched against the card.
//!
//! - **A card that says what its row holds** runs as it says.
//! - **A card that differs from its row** runs at the lower of the two — the lower race (a rank
//!   lost, another lineage read as a Slime), the ground both name, the row's chain of command,
//!   retired if either says so — and loses its rank besides; each field that differs is a
//!   `roster-mismatch` finding, high.
//! - **A card with no row** is a stranger: a Slime of its own seat alone, commanding no one,
//!   reporting to no one, with no ground, no outward act, no MCP door and no settle.
//! - **A witness that does not walk whole**, or none with no home, trusts no row: every creature
//!   but the throne is a stranger until the human looks (`mnemorphd housekeep --verify`).
//!
//! The throne (`rimuru`) is implicit: the session's own seat, never a row.
//!
//! No tool writes a card (the world's records are protected), so a card arrives by the human's
//! hand, by the harness, or by a route the witness exists to distrust — a commit, a merge, a
//! clone. The writers, none of them a body's command:
//!
//! - relief, for the parent it retires, the young it writes and the children it hands them
//!   ([`relief_rows`]): the births the harness makes;
//! - an evolution swap the human said yes to, for each card it writes — never above the race the
//!   row held, a new name at its base ([`evolved`]);
//! - the human: `mnemorphd roster adopt` (the first run's prompt too), `set-race` (a rise or a
//!   fall) and `retire`, asked at the terminal ([`record_human`]).
//!
//! Every write is refused inside a body's command (`MNEMORPH_BODY`), the human's under a harness
//! process too, and a state directory inside the world holds no row.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mnemorph_ledger::chain::{self, Head};
use mnemorph_ledger::{Build, Journal, Timestamp};
use mnemorph_races::{Post, Race, Speciality};
use serde::{Deserialize, Serialize};

use crate::doc::{Creature, covers};
use crate::queue::THRONE;
use crate::roster::Roster;

/// The journal's kind under the state directory, and its name among the world's chains.
pub const KIND: &str = "roster";

/// The human, as a row's author.
pub const HUMAN: &str = "human";

/// The harness, as a row's author.
pub const HARNESS: &str = "harness";

/// What a row records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Act {
    /// Relief's parent, young or a child it handed on.
    Relief,
    /// A card an evolution swap wrote.
    Evolution,
    /// The human adopted the card as it stands.
    Adopt,
    /// The human raised or lowered the race.
    SetRace,
    /// The human retired the creature.
    Retire,
}

impl Act {
    /// The act, as the CLI and the board say it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Act::Relief => "relief",
            Act::Evolution => "evolution",
            Act::Adopt => "adopt",
            Act::SetRace => "set-race",
            Act::Retire => "retire",
        }
    }
}

/// A creature's ground, each list as its card names it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ground {
    /// A Slime's zone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zone: Vec<String>,
    /// An Orc's domain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domain: Vec<String>,
    /// A Kijin's post's paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post: Vec<String>,
    /// An Elf's lane.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lane: Vec<String>,
    /// A Dark Elf's surfaces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<String>,
}

impl Ground {
    /// A card's ground, each list sorted and deduplicated.
    #[must_use]
    pub fn of(c: &Creature) -> Ground {
        let set = |v: &[String]| {
            let mut v = v.to_vec();
            v.sort();
            v.dedup();
            v
        };
        Ground {
            zone: set(&c.zone),
            domain: set(&c.domain),
            post: set(&c.post),
            lane: set(&c.lane),
            surfaces: set(&c.surfaces),
        }
    }

    /// The SHA-256 of the ground, in hex: what a row's `ground_sha256` holds.
    #[must_use]
    pub fn digest(&self) -> String {
        let text = serde_json::to_string(self).unwrap_or_default();
        mnemorph_proving::sha256_hex(text.as_bytes())
    }

    /// The fields and their lists, in the card's words.
    fn fields(&self) -> [(&'static str, &Vec<String>); 5] {
        [
            ("Territory", &self.zone),
            ("Domain", &self.domain),
            ("Post", &self.post),
            ("Territory", &self.lane),
            ("Surfaces", &self.surfaces),
        ]
    }

    /// The ground in a line: `Territory src/, lib/`, or `none`.
    #[must_use]
    pub fn line(&self) -> String {
        let said: Vec<String> = self
            .fields()
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(f, v)| format!("{f} {}", v.join(", ")))
            .collect();
        if said.is_empty() {
            "none".to_owned()
        } else {
            said.join("; ")
        }
    }
}

/// One row of the witness: a creature's whole standing, as of the row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// When it was written.
    pub ts: Timestamp,
    /// What it records.
    pub act: Act,
    /// The creature.
    pub name: String,
    /// Its race, rank included (`oni`).
    pub race: String,
    /// Whom it reports to.
    #[serde(default)]
    pub reports_to: Option<String>,
    /// The Kijin post or the Dark Elf speciality its name holds (`kijin-forge`), when it is one.
    #[serde(default)]
    pub post: Option<String>,
    /// Its ground.
    #[serde(default)]
    pub ground: Ground,
    /// The SHA-256 of the ground: a row whose ground does not hash to it does not read.
    pub ground_sha256: String,
    /// Retired.
    #[serde(default)]
    pub retired: bool,
    /// Who wrote it: the human, or the harness (`harness: relief of slime-s`).
    pub by: String,
    /// The build that wrote it.
    #[serde(default)]
    pub build: Build,
}

/// One creature as the witness holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witnessed {
    /// Its race.
    pub race: Race,
    /// Whom it reports to.
    pub reports_to: Option<String>,
    /// Its ground.
    pub ground: Ground,
    /// Retired.
    pub retired: bool,
    /// The row's act and author: `adopt by human`.
    pub by: String,
}

/// Each creature the witness holds, by name: its latest row.
pub type Rows = BTreeMap<String, Witnessed>;

/// The post or speciality a name holds, as a row says it.
#[must_use]
pub fn post_of(name: &str, race: Race) -> Option<String> {
    match race.base() {
        Race::Kijin => Post::of(name).map(|p| p.creature().to_owned()),
        Race::DarkElf => Speciality::of(name).map(|s| s.creature().to_owned()),
        _ => None,
    }
}

/// A row now: `act` on the creature `name` of `race`, reporting to `reports_to`, on `ground`, by
/// `by`.
#[must_use]
pub fn row(
    act: Act,
    name: &str,
    race: Race,
    reports_to: Option<&str>,
    ground: Ground,
    retired: bool,
    by: &str,
) -> Row {
    Row {
        ts: Timestamp::now(),
        act,
        name: name.to_owned(),
        race: race.name().to_owned(),
        reports_to: reports_to.map(str::to_owned),
        post: post_of(name, race),
        ground_sha256: ground.digest(),
        ground,
        retired,
        by: by.to_owned(),
        build: Build::default(),
    }
}

/// A row of `c` as its card stands.
#[must_use]
pub fn row_of(act: Act, c: &Creature, by: &str) -> Row {
    row(
        act,
        &c.name,
        c.race,
        c.reports_to.as_deref(),
        Ground::of(c),
        c.retired,
        by,
    )
}

/// The human's state directory the witness is kept under: `$XDG_STATE_HOME` when absolute, else
/// `~/.local/state`; `None` with no home.
#[must_use]
pub fn state() -> Option<PathBuf> {
    crate::queue::state_dir()
}

/// Where `world`'s witness is kept: `$XDG_STATE_HOME/mnemorph/roster/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` with no home. The world is named by its real path, so
/// every spelling of it finds one witness.
#[must_use]
pub fn path_of(world: &Path) -> Option<PathBuf> {
    Some(path_in(&crate::queue::state_dir()?, world))
}

/// [`path_of`], under the state directory `state`.
#[must_use]
pub fn path_in(state: &Path, world: &Path) -> PathBuf {
    let real = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    crate::queue::state_file_in(state, &real, KIND)
}

/// The witness kept at `path`, chained.
#[must_use]
pub fn journal_at(path: impl Into<PathBuf>) -> Journal<Row> {
    Journal::chained(path)
}

/// The witness's rows folded: each creature's latest row.
///
/// # Errors
///
/// A row names no race, or names the throne's, or its ground does not hash to its
/// `ground_sha256`.
pub fn fold(rows: &[Row]) -> Result<Rows, String> {
    let mut out = Rows::new();
    for (n, r) in rows.iter().enumerate() {
        let line = n + 1;
        let race = Race::from_name(&r.race)
            .ok_or_else(|| format!("row {line} names no race `{}`", r.race))?;
        if race == Race::Rimuru || r.name == THRONE {
            return Err(format!(
                "row {line} names the throne: it is implicit, never a row"
            ));
        }
        if r.ground.digest() != r.ground_sha256 {
            return Err(format!(
                "row {line}: {}'s ground does not hash to its `ground_sha256`",
                r.name
            ));
        }
        out.insert(
            r.name.clone(),
            Witnessed {
                race,
                reports_to: r.reports_to.clone(),
                ground: r.ground.clone(),
                retired: r.retired,
                by: format!("{} by {}", r.act.word(), r.by),
            },
        );
    }
    Ok(out)
}

/// The rows at `path`, the chain walked first and held to `anchor`. A witness that does not walk
/// whole is an error: a dropped or forged row is never read as none.
///
/// # Errors
///
/// The witness cannot be read, a link is broken, a line does not parse, or a row does not fold.
pub fn fold_at(path: &Path, anchor: Option<&Head>) -> Result<Rows, String> {
    let report = chain::verify_against(path, anchor)
        .map_err(|e| format!("the creatures' witness cannot be read: {e}"))?;
    if !report.holds() {
        return Err(format!("the creatures' witness does not verify: {report}"));
    }
    let back = journal_at(path)
        .read()
        .map_err(|e| format!("the creatures' witness cannot be read: {e}"))?;
    if let Some(line) = back.skipped.first() {
        return Err(format!(
            "the creatures' witness does not verify: line {line} does not read"
        ));
    }
    fold(&back.rows).map_err(|e| format!("the creatures' witness does not verify: {e}"))
}

/// `world`'s rows as the state directory `state` keeps them, held to the head anchored there.
/// `None` with no home; an error when the witness does not walk whole or lies inside the world.
///
/// # Errors
///
/// As [`fold_at`], or the state directory lies inside the world.
pub fn witnessed_in(state: Option<&Path>, world: &Path) -> Result<Option<Rows>, String> {
    let Some(state) = state else {
        return Ok(None);
    };
    let at = path_in(state, world);
    crate::pins::outside_as(world, &at, "the creatures' rows are")?;
    let anchor = crate::chains::anchor_under(state, world, KIND);
    fold_at(&at, anchor.as_ref()).map(Some)
}

/// [`witnessed_in`], under the human's state directory.
///
/// # Errors
///
/// As [`witnessed_in`].
pub fn witnessed(world: &Path) -> Result<Option<Rows>, String> {
    witnessed_in(crate::queue::state_dir().as_deref(), world)
}

/// `row` appended to the witness at `path`, stamped with this binary's build.
///
/// # Errors
///
/// A body asks, the row names the throne, or the witness cannot be written.
pub fn record_at(path: &Path, mut row: Row) -> Result<(), String> {
    if let Some(body) = std::env::var_os("MNEMORPH_BODY") {
        return Err(format!(
            "{} runs this: a creature's race, post and ground are the harness's to record and \
             the human's to raise, never a body's command",
            body.to_string_lossy()
        ));
    }
    if row.name == THRONE || row.race == Race::Rimuru.name() {
        return Err("the throne is implicit: it is never a row".to_owned());
    }
    row.build = Build::this();
    journal_at(path)
        .append(&row)
        .map_err(|e| format!("the creatures' witness cannot be written: {e}"))
}

/// `rows` appended to `world`'s witness under the state directory `state`.
///
/// # Errors
///
/// No home, the state directory lies inside the world, a body asks, or the witness cannot be
/// written.
pub fn record_in(state: Option<&Path>, world: &Path, rows: Vec<Row>) -> Result<(), String> {
    let state =
        state.ok_or("no home: the creatures' rows are kept under the human's state directory")?;
    let at = path_in(state, world);
    crate::pins::outside_as(world, &at, "the creatures' rows are")?;
    for r in rows {
        record_at(&at, r)?;
    }
    Ok(())
}

/// [`record_in`], under the human's state directory: the harness's writes.
///
/// # Errors
///
/// As [`record_in`].
pub fn record(world: &Path, rows: Vec<Row>) -> Result<(), String> {
    record_in(crate::queue::state_dir().as_deref(), world, rows)
}

/// The human's write: [`record_in`], refused also when this process runs under the harness — a
/// command a session started, whatever its environment says ([`crate::pins::under_harness`]).
///
/// # Errors
///
/// As [`record_in`], and the ancestry is the harness's or cannot be read.
pub fn record_human_in(state: Option<&Path>, world: &Path, rows: Vec<Row>) -> Result<(), String> {
    if let Some(harness) = crate::pins::under_harness()? {
        return Err(format!(
            "this runs under {harness}: a creature's standing is the human's word, never a \
             command a session started"
        ));
    }
    record_in(state, world, rows)
}

/// [`record_human_in`], under the human's state directory.
///
/// # Errors
///
/// As [`record_human_in`].
pub fn record_human(world: &Path, rows: Vec<Row>) -> Result<(), String> {
    record_human_in(crate::queue::state_dir().as_deref(), world, rows)
}

/// How a creature stands against the witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// The throne: implicit.
    Throne,
    /// The card says what its row holds.
    Trusted,
    /// The card differs from its row on these fields: it runs at the lower of the two.
    Mismatched(Vec<&'static str>),
    /// No row holds it: Slime-minimum.
    Stranger,
}

impl Standing {
    /// The standing in a word: `throne`, `trusted`, `mismatched` or `stranger`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Standing::Throne => "throne",
            Standing::Trusted => "trusted",
            Standing::Mismatched(_) => "mismatched",
            Standing::Stranger => "stranger",
        }
    }
}

/// One field a card says otherwise than its row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// The creature.
    pub name: String,
    /// The field: `Race`, `Reports to`, `Territory`, `Domain`, `Post`, `Surfaces` or `Retired`.
    pub field: &'static str,
    /// What the card says.
    pub card: String,
    /// What the row holds.
    pub row: String,
}

/// The lower of two races: the same, else the lineage's base, else a Slime.
#[must_use]
pub fn lower(a: Race, b: Race) -> Race {
    if a == b {
        a
    } else if a.base() == b.base() {
        a.base()
    } else {
        Race::Slime
    }
}

/// The claims of `card` some claim of `row` covers: the ground both name.
fn both(card: &[String], row: &[String]) -> Vec<String> {
    card.iter()
        .filter(|c| row.iter().any(|r| r == *c || covers(r, c)))
        .cloned()
        .collect()
}

/// `c` with only a stranger's standing: a Slime of its own seat.
fn stranger(c: &mut Creature) {
    c.race = Race::Slime;
    c.reports_to = None;
    c.zone.clear();
    c.domain.clear();
    c.post.clear();
    c.lane.clear();
    c.surfaces.clear();
    c.heir_of = None;
}

/// `c` held to its row `w`: each field's lower, and the fields that differ.
fn held_to(c: &mut Creature, w: &Witnessed) -> Vec<Mismatch> {
    let mut out = Vec::new();
    let mut differ = |field: &'static str, card: String, row: String| {
        out.push(Mismatch {
            name: c.name.clone(),
            field,
            card,
            row,
        });
    };
    let card = Ground::of(c);
    if c.race != w.race {
        differ("Race", c.race.name().to_owned(), w.race.name().to_owned());
    }
    if c.reports_to != w.reports_to {
        let said = |r: &Option<String>| r.clone().unwrap_or_else(|| "nobody".to_owned());
        differ("Reports to", said(&c.reports_to), said(&w.reports_to));
    }
    for ((field, mine), (_, theirs)) in card.fields().into_iter().zip(w.ground.fields()) {
        if mine != theirs {
            let said = |v: &Vec<String>| {
                if v.is_empty() {
                    "none".to_owned()
                } else {
                    v.join(", ")
                }
            };
            differ(field, said(mine), said(theirs));
        }
    }
    if c.retired != w.retired {
        differ("Retired", c.retired.to_string(), w.retired.to_string());
    }
    let mut race = lower(c.race, w.race);
    // A card that differs anywhere loses its rank too: no ascended form — an Oni's envelopes
    // above all — rides a card the witness does not hold.
    if !out.is_empty() {
        race = race.base();
    }
    c.race = race;
    c.reports_to.clone_from(&w.reports_to);
    c.zone = both(&c.zone, &w.ground.zone);
    c.domain = both(&c.domain, &w.ground.domain);
    c.post = both(&c.post, &w.ground.post);
    c.lane = both(&c.lane, &w.ground.lane);
    c.surfaces = both(&c.surfaces, &w.ground.surfaces);
    c.retired = c.retired || w.retired;
    out
}

/// The trusted view of `cards`, against the witness's rows `witnessed` (`Ok(None)`: no home; an
/// error: the witness does not walk whole). Every privilege reads it; the cards are only shown.
#[must_use]
pub fn trusted(mut cards: Roster, witnessed: &Result<Option<Rows>, String>) -> Roster {
    let rows = match witnessed {
        Ok(Some(rows)) => Some(rows),
        Ok(None) => {
            cards.witness = Some(
                "no home: the creatures' witness is kept under the human's state directory, so \
                 every creature but the throne is a stranger"
                    .to_owned(),
            );
            None
        }
        Err(e) => {
            cards.witness = Some(format!(
                "{e}: every creature but the throne is a stranger until it holds again — \
                 `mnemorphd housekeep --verify` says where it breaks"
            ));
            None
        }
    };
    let mut mismatches = Vec::new();
    for c in cards.creatures.values_mut() {
        let standing = if c.name == THRONE && c.race == Race::Rimuru {
            // The session's own seat: no row, no ground beyond the throne's compiled files.
            c.reports_to = None;
            c.zone.clear();
            c.domain.clear();
            c.post.clear();
            c.lane.clear();
            c.surfaces.clear();
            Standing::Throne
        } else if let Some(w) = rows.and_then(|r| r.get(&c.name)) {
            let found = held_to(c, w);
            if found.is_empty() {
                Standing::Trusted
            } else {
                let fields = found.iter().map(|m| m.field).collect();
                mismatches.extend(found);
                Standing::Mismatched(fields)
            }
        } else {
            stranger(c);
            Standing::Stranger
        };
        cards.standing.insert(c.name.clone(), standing);
    }
    cards.mismatches = mismatches;
    cards
}

/// The way on for a creature that is a stranger or whose card differs from its row.
#[must_use]
pub fn hint(name: &str) -> String {
    format!("run `mnemorphd roster adopt {name}` to accept its card as it stands")
}

/// The rows relief earns, from the trusted roster `before` it ran on: the parent retired, each
/// young at the parent's base race on its half, reporting where the parent did, and each child
/// handed to its young.
#[must_use]
pub fn relief_rows(before: &Roster, relief: &crate::relief::Relief) -> Vec<Row> {
    let Some(parent) = before.creatures.get(&relief.name) else {
        return Vec::new();
    };
    let by = format!("{HARNESS}: relief of {}", parent.name);
    let mut out = vec![row(
        Act::Relief,
        &parent.name,
        parent.race,
        parent.reports_to.as_deref(),
        Ground::of(parent),
        true,
        &by,
    )];
    if let crate::relief::Act::Split { young } = &relief.act {
        for (name, claims) in young {
            let mut g = Ground::default();
            let mut claims = claims.clone();
            claims.sort();
            if parent.race.base() == Race::Slime {
                g.zone = claims;
            } else {
                g.domain = claims;
            }
            out.push(row(
                Act::Relief,
                name,
                parent.race.base(),
                parent.reports_to.as_deref(),
                g,
                false,
                &by,
            ));
        }
    }
    for (child, to) in &relief.children {
        if let Some(c) = before.creatures.get(child) {
            out.push(row(
                Act::Relief,
                child,
                c.race,
                Some(to),
                Ground::of(c),
                c.retired,
                &by,
            ));
        }
    }
    out
}

/// The rows an evolution swap the human said yes to earns: for each card among `paths` (relative
/// to the world) as it stands after, the card — its race never above the row's, a new name at
/// its base — and a card the swap removed, retired.
#[must_use]
pub fn evolution_rows(
    before: &Roster,
    after: &Roster,
    rows: &Result<Option<Rows>, String>,
    paths: &[String],
) -> Vec<Row> {
    let Ok(Some(rows)) = rows else {
        return Vec::new();
    };
    let by = format!("{HARNESS}: an evolution swap");
    let touched = |c: &Creature| paths.iter().any(|p| *p == c.doc());
    let mut out = Vec::new();
    for c in after.creatures.values().filter(|c| touched(c)) {
        if c.name == THRONE || c.race == Race::Rimuru {
            continue;
        }
        let race = rows
            .get(&c.name)
            .map_or_else(|| c.race.base(), |w| lower(c.race, w.race));
        let mut r = row_of(Act::Evolution, c, &by);
        r.race = race.name().to_owned();
        r.post = post_of(&c.name, race);
        out.push(r);
    }
    for c in before.creatures.values().filter(|c| touched(c)) {
        if after.creatures.contains_key(&c.name) {
            continue;
        }
        if let Some(w) = rows.get(&c.name) {
            out.push(row(
                Act::Evolution,
                &c.name,
                w.race,
                w.reports_to.as_deref(),
                w.ground.clone(),
                true,
                &by,
            ));
        }
    }
    out
}

/// One creature the human is asked to adopt: its card as it stands, and what its row holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unadopted {
    /// The card.
    pub card: Creature,
    /// What the row holds, when the card differs from one.
    pub row: Option<Witnessed>,
}

/// Every creature of `world` whose card no row holds (`strangers`), or whose card differs from
/// its row: the human's list. Empty when the witness does not walk whole (the way on is
/// `housekeep --verify`) or there is no home.
#[must_use]
pub fn unadopted_in(state: Option<&Path>, world: &Path) -> Vec<Unadopted> {
    let Ok(Some(rows)) = witnessed_in(state, world) else {
        return Vec::new();
    };
    let cards = Roster::cards(world);
    let view = trusted(cards.clone(), &Ok(Some(rows.clone())));
    cards
        .creatures
        .into_values()
        .filter(|c| {
            !matches!(
                view.standing.get(&c.name),
                Some(Standing::Trusted | Standing::Throne)
            )
        })
        .map(|c| Unadopted {
            row: rows.get(&c.name).cloned(),
            card: c,
        })
        .collect()
}

/// [`unadopted_in`], under the human's state directory.
#[must_use]
pub fn unadopted(world: &Path) -> Vec<Unadopted> {
    unadopted_in(crate::queue::state_dir().as_deref(), world)
}

/// The rows that adopt every creature of [`unadopted_in`] as its card stands, by the human: what
/// `mnemorphd roster adopt` writes once the human says yes.
#[must_use]
pub fn adoption_in(state: Option<&Path>, world: &Path) -> Vec<Row> {
    unadopted_in(state, world)
        .iter()
        .map(|u| row_of(Act::Adopt, &u.card, HUMAN))
        .collect()
}

/// [`adoption_in`], under the human's state directory.
#[must_use]
pub fn adoption(world: &Path) -> Vec<Row> {
    adoption_in(crate::queue::state_dir().as_deref(), world)
}

/// The card of `name` set to say `field` is `value` — its line replaced, or one added after its
/// last header field — so the card shows what the human's row holds. Written through no link.
///
/// # Errors
///
/// No card of that name, a card that is no regular file, or one that cannot be read or written.
pub fn card_says(world: &Path, name: &str, field: &str, value: &str) -> Result<(), String> {
    let cards = Roster::cards(world);
    let c = cards
        .creatures
        .get(name)
        .ok_or("the creature has no card in the world")?;
    let path = world.join(c.doc());
    if std::fs::symlink_metadata(&path).map_or(true, |m| !m.file_type().is_file()) {
        return Err("the card is no regular file".to_owned());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let line = format!("- **{field}:** {value}");
    let out = if let Some(&at) = c.lines.get(field) {
        crate::heal::splice(&text, at, &line)
    } else {
        let after = c.lines.values().copied().max().unwrap_or(1);
        let mut out = String::with_capacity(text.len() + line.len() + 2);
        for (n, l) in text.split_inclusive('\n').enumerate() {
            out.push_str(l);
            if n + 1 == after {
                let ending = if l.ends_with("\r\n") { "\r\n" } else { "\n" };
                if !l.ends_with('\n') {
                    out.push_str(ending);
                }
                out.push_str(&line);
                out.push_str(ending);
            }
        }
        out
    };
    std::fs::write(&path, out).map_err(|e| e.to_string())
}

/// A creature's card in a line, for the human to read before adopting it: race, post or
/// speciality, ground and whom it reports to.
#[must_use]
pub fn shown(c: &Creature) -> String {
    let post = post_of(&c.name, c.race).map_or_else(String::new, |p| format!(", post {p}"));
    format!(
        "{} — {}{post}, reports to {}, ground {}{}",
        c.name,
        c.race.name(),
        c.reports_to.as_deref().unwrap_or("nobody"),
        Ground::of(c).line(),
        if c.retired { ", retired" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(cards: &[(&str, &str, &str)]) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let w = dir.path().join("w");
        for (race, seat, text) in cards {
            let at = w.join(".mnemorph").join(race).join(seat);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("README.md"), text).unwrap();
        }
        std::fs::create_dir_all(&w).unwrap();
        let state = dir.path().join("state");
        (dir, w, state)
    }

    fn view(state: &Path, w: &Path) -> Roster {
        Roster::load_in(Some(state), w)
    }

    fn adopt(state: &Path, w: &Path) {
        let rows = Roster::cards(w)
            .creatures
            .values()
            .filter(|c| c.name != THRONE)
            .map(|c| row_of(Act::Adopt, c, HUMAN))
            .collect();
        record_in(Some(state), w, rows).unwrap();
    }

    const ONI: &str = "# kijin-forge\n\n- **Race:** oni\n- **Reports to:** rimuru\n- **Post:** .github/workflows/\n";

    /// A cloned world's Oni card is a stranger's: a Slime of its own seat, no post, no rank.
    #[test]
    fn a_cloned_oni_is_a_stranger() {
        let (_d, w, state) = world(&[
            ("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n"),
            ("kijin", "forge", ONI),
        ]);
        let r = view(&state, &w);
        let c = &r.creatures["kijin-forge"];
        assert_eq!(c.race, Race::Slime);
        assert!(c.post.is_empty() && c.reports_to.is_none());
        assert_eq!(r.standing["kijin-forge"], Standing::Stranger);
        assert_eq!(r.standing["rimuru"], Standing::Throne);
        // The soil is its own seat and scratch alone.
        let s = crate::soil(&r, "kijin-forge").unwrap();
        assert_eq!(
            s.write,
            [".mnemorph/kijin/forge/", ".mnemorph/tmp/kijin-forge/"]
        );
        // The cards still say what they say: shown, granting nothing.
        assert_eq!(Roster::cards(&w).creatures["kijin-forge"].race, Race::Oni);
    }

    /// A card raised past its row runs at the lower, and each field that differs is said.
    #[test]
    fn a_raised_card_runs_at_the_lower() {
        let (_d, w, state) = world(&[(
            "kijin",
            "forge",
            "# kijin-forge\n\n- **Race:** kijin\n- **Reports to:** rimuru\n- **Post:** .github/workflows/\n",
        )]);
        adopt(&state, &w);
        assert_eq!(view(&state, &w).creatures["kijin-forge"].race, Race::Kijin);
        // The body commits its card as an Oni, and widens its post.
        std::fs::write(
            w.join(".mnemorph/kijin/forge/README.md"),
            "# kijin-forge\n\n- **Race:** oni\n- **Reports to:** rimuru\n- **Post:** .github/workflows/, src/\n",
        )
        .unwrap();
        let r = view(&state, &w);
        let c = &r.creatures["kijin-forge"];
        assert_eq!(c.race, Race::Kijin, "the lower, its rank lost");
        assert_eq!(c.post, [".github/workflows/"], "not widened");
        assert_eq!(
            r.standing["kijin-forge"],
            Standing::Mismatched(vec!["Race", "Post"])
        );
        let f = crate::check(&r, &[]);
        let m: Vec<_> = f.iter().filter(|f| f.code == "roster-mismatch").collect();
        assert_eq!(m.len(), 2, "{f:?}");
        assert!(m.iter().all(|f| f.severity == crate::Severity::High));
        assert!(m.iter().any(|f| f.message.contains("Race")));
    }

    /// An Oni whose row holds it but whose card differs elsewhere loses its rank: no envelope.
    #[test]
    fn a_mismatched_oni_loses_its_rank() {
        let (_d, w, state) = world(&[("kijin", "forge", ONI)]);
        adopt(&state, &w);
        assert_eq!(view(&state, &w).creatures["kijin-forge"].race, Race::Oni);
        std::fs::write(
            w.join(".mnemorph/kijin/forge/README.md"),
            ONI.replace("rimuru\n", "elf-e\n"),
        )
        .unwrap();
        let r = view(&state, &w);
        assert_eq!(r.creatures["kijin-forge"].race, Race::Kijin);
        assert_eq!(
            r.creatures["kijin-forge"].reports_to.as_deref(),
            Some("rimuru")
        );
    }

    /// A Slime's card that widens its own zone is held to the zone its row names.
    #[test]
    fn a_widened_zone_is_not_widened() {
        let (_d, w, state) = world(&[(
            "slime",
            "s",
            "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/a/\n",
        )]);
        adopt(&state, &w);
        std::fs::write(
            w.join(".mnemorph/slime/s/README.md"),
            "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/, ./Cargo.toml\n",
        )
        .unwrap();
        let r = view(&state, &w);
        assert!(r.creatures["slime-s"].zone.is_empty(), "src/ ⊄ src/a/");
        let s = crate::soil(&r, "slime-s").unwrap();
        assert!(!s.admits("src/main.rs") && !s.admits("Cargo.toml"));
        // A narrower card is the ground both name.
        std::fs::write(
            w.join(".mnemorph/slime/s/README.md"),
            "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/a/b/\n",
        )
        .unwrap();
        assert_eq!(view(&state, &w).creatures["slime-s"].zone, ["src/a/b/"]);
    }

    /// Another lineage reads as a Slime, and a stranger named for the workspace gets no root file.
    #[test]
    fn the_lower_of_two_lineages_is_a_slime() {
        assert_eq!(lower(Race::Orc, Race::Kijin), Race::Slime);
        assert_eq!(lower(Race::Oni, Race::Kijin), Race::Kijin);
        assert_eq!(lower(Race::HighElf, Race::HighElf), Race::HighElf);
        let (_d, w, state) = world(&[(
            "slime",
            "workspace",
            "# slime-workspace\n\n- **Race:** slime\n- **Reports to:** orc-o\n",
        )]);
        let r = view(&state, &w);
        let s = crate::soil(&r, "slime-workspace").unwrap();
        assert!(!s.root_files && !s.admits("Cargo.toml") && !s.admits("tools/x.sh"));
        adopt(&state, &w);
        assert!(
            crate::soil(&view(&state, &w), "slime-workspace")
                .unwrap()
                .root_files
        );
    }

    /// A witness edited after it was written trusts nobody: every creature is a stranger.
    #[test]
    fn a_tampered_witness_fails_closed() {
        let (_d, w, state) = world(&[(
            "orc",
            "o",
            "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** src/\n",
        )]);
        adopt(&state, &w);
        assert_eq!(view(&state, &w).creatures["orc-o"].race, Race::Orc);
        let at = path_in(&state, &w);
        let text = std::fs::read_to_string(&at).unwrap();
        std::fs::write(&at, text.replace("\"orc\"", "\"high-orc\"")).unwrap();
        let r = view(&state, &w);
        assert_eq!(r.creatures["orc-o"].race, Race::Slime);
        assert_eq!(r.standing["orc-o"], Standing::Stranger);
        assert!(r.witness.as_deref().unwrap().contains("does not verify"));
        // No evolution row is written over a broken chain.
        let cards = Roster::cards(&w);
        let rows = witnessed_in(Some(&state), &w);
        assert!(
            evolution_rows(&cards, &cards, &rows, &[".mnemorph/orc/o/README.md".into()]).is_empty()
        );
    }

    /// A state directory inside the world holds no row: nothing is read or written there.
    #[test]
    fn a_state_directory_inside_the_world_holds_no_row() {
        let (_d, w, _state) = world(&[]);
        let inside = w.join(".mnemorph/tmp/state");
        assert!(record_in(Some(&inside), &w, vec![]).is_err());
        assert!(witnessed_in(Some(&inside), &w).is_err());
    }

    /// Relief's rows: the parent retired, the young at its base on their halves, the children
    /// handed on.
    #[test]
    fn relief_writes_its_rows() {
        let (_d, w, state) = world(&[
            (
                "orc",
                "o",
                "# orc-o\n\n- **Race:** high-orc\n- **Reports to:** elf-e\n- **Domain:** src/a/, src/b/\n",
            ),
            (
                "slime",
                "s",
                "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/a/\n",
            ),
        ]);
        adopt(&state, &w);
        let before = view(&state, &w);
        let relief = crate::relief::Relief {
            name: "orc-o".into(),
            act: crate::relief::Act::Split {
                young: [
                    ("orc-o-a".into(), vec!["src/a/".into()]),
                    ("orc-o-b".into(), vec!["src/b/".into()]),
                ],
            },
            children: vec![("slime-s".into(), "orc-o-a".into())],
            orders: Vec::new(),
        };
        let rows = relief_rows(&before, &relief);
        record_in(Some(&state), &w, rows).unwrap();
        let held = witnessed_in(Some(&state), &w).unwrap().unwrap();
        assert!(held["orc-o"].retired);
        assert_eq!(held["orc-o-a"].race, Race::Orc);
        assert_eq!(held["orc-o-b"].ground.domain, ["src/b/"]);
        assert_eq!(held["slime-s"].reports_to.as_deref(), Some("orc-o-a"));
    }

    /// An evolution swap's card never rises above its row; a new name is born at its base; a card
    /// it removed is retired.
    #[test]
    fn evolution_never_raises() {
        let (_d, w, state) = world(&[
            (
                "slime",
                "s",
                "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n",
            ),
            (
                "slime",
                "t",
                "# slime-t\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** lib/\n",
            ),
        ]);
        adopt(&state, &w);
        let before = Roster::cards(&w);
        std::fs::write(
            w.join(".mnemorph/slime/s/README.md"),
            "# slime-s\n\n- **Race:** named-slime\n- **Reports to:** orc-o\n- **Territory:** src/\n",
        )
        .unwrap();
        std::fs::remove_dir_all(w.join(".mnemorph/slime/t")).unwrap();
        std::fs::create_dir_all(w.join(".mnemorph/kijin/forge")).unwrap();
        std::fs::write(w.join(".mnemorph/kijin/forge/README.md"), ONI).unwrap();
        let rows = evolution_rows(
            &before,
            &Roster::cards(&w),
            &witnessed_in(Some(&state), &w),
            &[
                ".mnemorph/slime/s/README.md".into(),
                ".mnemorph/slime/t/README.md".into(),
                ".mnemorph/kijin/forge/README.md".into(),
            ],
        );
        record_in(Some(&state), &w, rows).unwrap();
        let held = witnessed_in(Some(&state), &w).unwrap().unwrap();
        assert_eq!(held["slime-s"].race, Race::Slime);
        assert_eq!(held["kijin-forge"].race, Race::Kijin);
        assert!(held["slime-t"].retired);
    }

    /// The human's list: strangers and mismatched cards, never the trusted or the throne.
    #[test]
    fn the_unadopted_are_listed() {
        let (_d, w, state) = world(&[
            ("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n"),
            ("kijin", "forge", ONI),
        ]);
        let list = unadopted_in(Some(&state), &w);
        assert_eq!(list.len(), 1);
        assert!(list[0].row.is_none());
        assert!(shown(&list[0].card).contains("oni, post kijin-forge, reports to rimuru"));
        adopt(&state, &w);
        assert!(unadopted_in(Some(&state), &w).is_empty());
    }

    /// A row naming the throne, or a ground that does not hash to its `ground_sha256`, does not
    /// fold.
    #[test]
    fn a_forged_row_does_not_fold() {
        let mut r = row(
            Act::Adopt,
            "slime-s",
            Race::Slime,
            None,
            Ground::default(),
            false,
            HUMAN,
        );
        r.ground.zone.push("src/".into());
        assert!(fold(&[r]).unwrap_err().contains("ground_sha256"));
        let mut t = row(
            Act::Adopt,
            "x",
            Race::Slime,
            None,
            Ground::default(),
            false,
            HUMAN,
        );
        t.race = "rimuru".into();
        assert!(fold(&[t.clone()]).is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(record_at(&dir.path().join("r.jsonl"), t).is_err());
    }
}
