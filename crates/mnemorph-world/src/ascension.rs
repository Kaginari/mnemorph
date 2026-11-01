//! Rises and falls, from the compiled race law alone: what it proposes from the report ledger, and
//! the human's act that applies one to the creatures' witness.
//!
//! - **Inputs.** The report ledger's rows ([`crate::reports`]), folded from the trusted journals
//!   under the human's state directory, and the creatures' witness ([`crate::creatures`]), which
//!   gives each creature the race it is held to. Nothing learned enters: no model, no estimate,
//!   no interval — the bars read the point estimate ([`mnemorph_races::rises`], `falls`,
//!   `recovers`), and the Wilson interval is only shown.
//! - **The window.** A rise reads the creature's reports of the last [`WINDOW_DAYS`] days, by the
//!   trusted time each began, up to the ledger's clock: the newest report's time. A report counts
//!   once the creature's next report has begun, so a report still in flight never moves a
//!   proposal; and only reports since the creature's last rise or fall count, so a form is earned
//!   again from nothing.
//! - **Probation.** An ascended creature's reports since its rise are walked in order: an event
//!   ends the form at once; a rate under its fall bar over its last ten starts probation; after
//!   [`RECOVER_REPORTS`] reports on probation it recovers at its fall bar, or falls. Probation
//!   changes no privilege, so it is read, never written.
//! - **Proposals never apply themselves.** A proposal carries its inputs — the meters, the record
//!   read, the window — the law's lines, and the SHA-256 of the reports it read and the move. Only
//!   the human's act applies one ([`apply_in`]): asked at the terminal, refused inside a body's
//!   command, under a harness process, or with a state directory inside the world, it writes a
//!   `rise` or `fall` row to the witness, chained, naming the proposal and its digest.
//! - **Replay.** The proposals are a function of the ledger's rows and the witness's rows alone,
//!   read at a trusted time the row keeps (`as_of`): [`replay`] recomputes every `rise` and `fall`
//!   row from them and names the first that does not reproduce.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use mnemorph_ledger::chain::Walk;
use mnemorph_ledger::reports::{Inputs, ReportRow, fold as fold_rows};
use mnemorph_ledger::{Build, Timestamp};
use mnemorph_races::{
    Explanation, FallReading, Kind, LawBar, RECOVER_REPORTS, Race, Rate, Recent, Recover, Signals,
    explain_fall, explain_rise, falls, law_bars, recovers, rerise, rises,
};
use serde::Serialize;

use crate::creatures::{self, Act, HUMAN, Row};

/// The days a rise's window reads.
pub const WINDOW_DAYS: i64 = 28;

/// The window, in seconds.
const WINDOW: i64 = WINDOW_DAYS * 86_400;

/// The recent record a rate-driven fall reads: the last ten reports.
const FALL_RECORD: usize = 10;

/// A move of the race law.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Move {
    /// From a base race to its ascended form.
    Rise,
    /// From an ascended form back to its base race.
    Fall,
}

impl Move {
    /// `rise` or `fall`: the word, and the `mnemorphd roster` act that applies it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Move::Rise => "rise",
            Move::Fall => "fall",
        }
    }

    /// The witness row's act.
    #[must_use]
    pub fn act(self) -> Act {
        match self {
            Move::Rise => Act::Rise,
            Move::Fall => Act::Fall,
        }
    }
}

/// A rise or a fall the compiled law proposes, for the human to apply or leave.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    /// Its id: the first twelve digits of [`Proposal::inputs_sha256`].
    pub id: String,
    /// The creature.
    pub creature: String,
    /// The move.
    pub act: Move,
    /// The race the witness holds it to now.
    pub from: Race,
    /// The race it would be.
    pub to: Race,
    /// A fall at the end of a probation it did not recover from.
    pub ended_probation: bool,
    /// The law's lines: every bar of a rise, the bar or event of a fall.
    pub why: Vec<Explanation>,
    /// The creature's meters over the reports read, each with its count (and, shown, its
    /// interval).
    pub meters: Vec<(&'static str, Rate)>,
    /// The reports read.
    pub n: u32,
    /// The window the reports were read in: after its first instant, up to its last.
    pub window: (Timestamp, Timestamp),
    /// The trusted time it was read at: the ledger's clock for a rise, the report that settled
    /// the deciding one for a fall.
    pub as_of: Timestamp,
    /// The SHA-256 of the reports read and the move, in hex.
    pub inputs_sha256: String,
    /// The build that read it: shown, never part of the digest, so a replay under another build
    /// reads the same proposal.
    pub build: Build,
}

impl Proposal {
    /// What it proposes, in words: `rise to named-slime`, `fall to slime (probation ended
    /// unrecovered)`.
    #[must_use]
    pub fn what(&self) -> String {
        let ended = if self.ended_probation {
            " (probation ended unrecovered)"
        } else {
            ""
        };
        format!("{} to {}{ended}", self.act.word(), self.to.name())
    }

    /// The command that applies it, for `world`, its words quoted for a shell.
    #[must_use]
    pub fn command(&self, world: &Path) -> String {
        format!(
            "mnemorphd roster {} {} --world {}",
            self.act.word(),
            quoted(&self.creature),
            quoted(&world.display().to_string())
        )
    }

    /// The proposal in full, for the human to read before the yes: the move, the meters, the
    /// record and window, the law's lines, the digest and the build. Unescaped: the caller
    /// escapes for its surface.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let meters: Vec<String> = self
            .meters
            .iter()
            .map(|(m, r)| format!("{m} {r}"))
            .collect();
        let mut out = vec![
            format!(
                "{} ({}): {} proposed — proposal {}",
                self.creature,
                self.from.name(),
                self.what(),
                self.id
            ),
            format!(
                "  read over {} reports, {} to {} (the {WINDOW_DAYS}-day window), as of {}",
                self.n, self.window.0, self.window.1, self.as_of
            ),
            format!("  meters: {}", meters.join(" · ")),
        ];
        out.extend(self.why.iter().map(|w| format!("  {w}")));
        out.push(format!(
            "  inputs SHA-256 {}, read by {}",
            self.inputs_sha256, self.build
        ));
        out
    }
}

/// A creature on probation: its reports since probation began, of the [`RECOVER_REPORTS`] it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probation {
    /// The creature.
    pub creature: String,
    /// Its ascended form.
    pub race: Race,
    /// Reports since probation began.
    pub reports: u32,
    /// When the report that began it began.
    pub began: Timestamp,
}

impl fmt::Display for Probation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): on probation since {} — {} of {RECOVER_REPORTS} reports; it recovers at its \
             fall bar or a fall is proposed",
            self.creature,
            self.race.name(),
            self.began,
            self.reports
        )
    }
}

/// What the law reads of every creature: the proposals, the probations, and the creatures whose
/// law reads an input no trusted journal carries yet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Survey {
    /// The proposals, by creature.
    pub proposals: Vec<Proposal>,
    /// The creatures on probation.
    pub probation: Vec<Probation>,
    /// Creatures whose bars read an unknown input, and its law name: no proposal is computed.
    pub unknown: Vec<(String, &'static str)>,
}

impl Survey {
    /// The proposal for `creature`, when there is one.
    #[must_use]
    pub fn of(&self, creature: &str) -> Option<&Proposal> {
        self.proposals.iter().find(|p| p.creature == creature)
    }

    /// The probation of `creature`, when it is on one.
    #[must_use]
    pub fn probation_of(&self, creature: &str) -> Option<&Probation> {
        self.probation.iter().find(|p| p.creature == creature)
    }
}

/// A word for a shell: as it is when it holds only plain characters, else single-quoted.
#[must_use]
pub fn quoted(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/-:@+=,".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// How the witness holds one creature at a point of its rows.
#[derive(Debug, Clone)]
struct Held {
    race: Race,
    retired: bool,
    /// Its reports count from after this: its last rise or fall, or the human's `set-race`.
    since: Option<Timestamp>,
}

/// Each creature as `rows` hold it, by name.
fn held(rows: &[Row]) -> BTreeMap<String, Held> {
    let mut out: BTreeMap<String, Held> = BTreeMap::new();
    for r in rows {
        let Some(race) = Race::from_name(&r.race) else {
            continue;
        };
        let was = out.get(&r.name);
        let since = match r.act {
            Act::Rise | Act::Fall => r.as_of.or(Some(r.ts)),
            Act::SetRace => Some(r.ts),
            _ => match was {
                Some(w) if w.race != race => Some(r.ts),
                Some(w) => w.since,
                None => None,
            },
        };
        out.insert(
            r.name.clone(),
            Held {
                race,
                retired: r.retired,
                since,
            },
        );
    }
    out
}

/// Whether `row` is one of `creature`'s reports in `race`'s own unit: an outward act for a Kijin,
/// a run for every other race; its race the one the usage witness says it ran as, by base.
fn counts(row: &ReportRow, creature: &str, race: Race) -> bool {
    let (c, r) = row.key();
    let unit = match race.base() {
        Race::Kijin => matches!(row, ReportRow::Act(_)),
        _ => matches!(row, ReportRow::Run(_)),
    };
    unit && c == creature && Race::from_name(r).is_some_and(|x| x.base() == race.base())
}

/// The inputs over `rows`, one creature's.
fn inputs_of(rows: &[&ReportRow]) -> Inputs {
    let owned: Vec<ReportRow> = rows.iter().map(|r| (*r).clone()).collect();
    fold_rows(&owned)
        .values()
        .fold(Inputs::default(), |mut acc, i| {
            crate::reports::merge(&mut acc, i);
            acc
        })
}

/// The signals `race`'s law reads over `rows`, with the inputs they came from.
fn signals_of(race: Race, rows: &[&ReportRow]) -> Result<(Signals, Inputs), &'static str> {
    let i = inputs_of(rows);
    let s = i.signals(race)?;
    Ok((s, i))
}

/// The meters a race is read on, as counts over `i`.
fn meters_of(race: Race, i: &Inputs) -> Vec<(&'static str, Rate)> {
    crate::meters::of_race(race)
        .iter()
        .map(|&m| (m, i.rate(m).unwrap_or_default()))
        .collect()
}

/// The SHA-256 of a move over the reports it read.
fn digest(
    creature: &str,
    act: Move,
    from: Race,
    to: Race,
    ended: bool,
    rows: &[&ReportRow],
) -> String {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Digested<'a> {
        creature: &'a str,
        act: Move,
        from: &'a str,
        to: &'a str,
        ended_probation: bool,
        reports: &'a [&'a ReportRow],
    }
    let text = serde_json::to_vec(&Digested {
        creature,
        act,
        from: from.name(),
        to: to.name(),
        ended_probation: ended,
        reports: rows,
    })
    .unwrap_or_default();
    mnemorph_proving::sha256_hex(&text)
}

/// What a walk of an ascended creature's reports came to.
enum Walked {
    Holds,
    OnProbation {
        reports: u32,
        began: Timestamp,
    },
    Falls {
        at: usize,
        why: Vec<Explanation>,
        ended: bool,
    },
    Unknown(&'static str),
}

/// The fall bar that starts `risen`'s probation, as the law sets it.
fn fall_bar(risen: Race) -> Option<LawBar> {
    law_bars(risen)
        .into_iter()
        .find(|b| b.kind == Kind::Probation)
}

/// The line of a probation that ended unrecovered: the fall bar over the reports since it began.
fn unrecovered(risen: Race, i: &Inputs, n: u32) -> Vec<Explanation> {
    let Some(bar) = fall_bar(risen) else {
        return Vec::new();
    };
    let reading = i
        .rate(bar.meter)
        .and_then(|r| r.value())
        .map_or(0.0, |p| p * 100.0);
    vec![Explanation {
        kind: Kind::Probation,
        meter: bar.meter,
        scale: bar.scale,
        reading,
        op: bar.op,
        threshold: bar.threshold,
        window: n,
        unit: "reports",
        recover: Some(Recover {
            op: mnemorph_races::Op::Ge,
            threshold: bar.threshold,
            within: RECOVER_REPORTS,
            unit: "reports",
        }),
        rerise: rerise(risen, bar.meter),
        head: None,
        from: None,
        to: None,
    }]
}

/// `risen`'s reports since its rise, settled and in order, walked: an event falls it at once; a
/// rate under the fall bar over its last ten (in the window) starts probation; after
/// [`RECOVER_REPORTS`] reports on probation it recovers at the fall bar or falls.
fn walk(risen: Race, rows: &[&ReportRow]) -> Walked {
    let mut probation: Option<usize> = None;
    for (i, row) in rows.iter().enumerate() {
        let lo = row.ts().0 - WINDOW;
        let from = probation.unwrap_or(0);
        let mut picked: Vec<&ReportRow> = rows[from..=i]
            .iter()
            .copied()
            .filter(|r| r.ts().0 > lo)
            .collect();
        if probation.is_none() {
            picked.drain(..picked.len().saturating_sub(FALL_RECORD));
        }
        let (s, inputs) = match signals_of(risen, &picked) {
            Ok(x) => x,
            Err(e) => return Walked::Unknown(e),
        };
        let reading = falls(risen, &Recent(s));
        if reading == FallReading::Immediate {
            return Walked::Falls {
                at: i,
                why: explain_fall(risen, &Recent(s)),
                ended: false,
            };
        }
        let on = u32::try_from(picked.len()).unwrap_or(u32::MAX);
        match probation {
            None if reading == FallReading::Probation => probation = Some(i + 1),
            None => {}
            Some(_) if on < RECOVER_REPORTS => {}
            Some(_) if recovers(risen, &Recent(s)) => probation = None,
            Some(_) => {
                return Walked::Falls {
                    at: i,
                    why: unrecovered(risen, &inputs, on),
                    ended: true,
                };
            }
        }
    }
    match probation {
        Some(p) => Walked::OnProbation {
            reports: u32::try_from(rows.len() - p).unwrap_or(u32::MAX),
            began: rows[p - 1].ts(),
        },
        None => Walked::Holds,
    }
}

/// One creature's reports since its last move, up to `clock`, in order of their trusted time:
/// all of them, and the settled ones — those after which its next report has begun.
fn reports_of<'a>(
    rows: &'a [ReportRow],
    name: &str,
    h: &Held,
    clock: Timestamp,
) -> (Vec<&'a ReportRow>, Vec<&'a ReportRow>) {
    let mut mine: Vec<&ReportRow> = rows
        .iter()
        .filter(|r| counts(r, name, h.race))
        .filter(|r| r.ts() <= clock && h.since.is_none_or(|t| r.ts() > t))
        .collect();
    mine.sort_by_key(|r| r.ts());
    let newest = mine.last().map(|r| r.ts());
    let settled = mine
        .iter()
        .copied()
        .filter(|r| Some(r.ts()) != newest)
        .collect();
    (mine, settled)
}

/// The rise of a base-race creature over the window to `clock`, when every bar holds.
fn rise_of(
    name: &str,
    race: Race,
    settled: &[&ReportRow],
    clock: Timestamp,
) -> Result<Option<Proposal>, &'static str> {
    let Some(risen) = race.ascends_to() else {
        return Ok(None);
    };
    let lo = Timestamp(clock.0 - WINDOW);
    let window: Vec<&ReportRow> = settled.iter().copied().filter(|r| r.ts() > lo).collect();
    let (s, i) = signals_of(race, &window)?;
    if !rises(risen, &s) {
        return Ok(None);
    }
    let sha = digest(name, Move::Rise, race, risen, false, &window);
    Ok(Some(Proposal {
        id: sha[..12].to_owned(),
        creature: name.to_owned(),
        act: Move::Rise,
        from: race,
        to: risen,
        ended_probation: false,
        why: explain_rise(risen, &s),
        meters: meters_of(race, &i),
        n: u32::try_from(window.len()).unwrap_or(u32::MAX),
        window: (lo, clock),
        as_of: clock,
        inputs_sha256: sha,
        build: Build::this(),
    }))
}

/// The fall of an ascended creature whose walk fell at its settled report `at`: read at the start
/// of the report that settled it, over every report since its rise up to it.
fn fall_of(
    name: &str,
    race: Race,
    (mine, settled): (&[&ReportRow], &[&ReportRow]),
    (at, why, ended): (usize, Vec<Explanation>, bool),
    clock: Timestamp,
) -> Proposal {
    let picked = &settled[..=at];
    let i = inputs_of(picked);
    let as_of = mine
        .iter()
        .map(|r| r.ts())
        .find(|t| *t > settled[at].ts())
        .unwrap_or(clock);
    let to = race.base();
    let sha = digest(name, Move::Fall, race, to, ended, picked);
    Proposal {
        id: sha[..12].to_owned(),
        creature: name.to_owned(),
        act: Move::Fall,
        from: race,
        to,
        ended_probation: ended,
        why,
        meters: meters_of(race, &i),
        n: u32::try_from(picked.len()).unwrap_or(u32::MAX),
        window: (Timestamp(picked[0].ts().0 - 1), settled[at].ts()),
        as_of,
        inputs_sha256: sha,
        build: Build::this(),
    }
}

/// What the law proposes at the trusted time `clock`, from the report ledger's `rows` and the
/// witness's `roster` rows: a pure function of the two, so a replay at the same time reads the
/// same.
#[must_use]
pub fn propose_at(rows: &[ReportRow], roster: &[Row], clock: Timestamp) -> Survey {
    let mut out = Survey::default();
    for (name, h) in held(roster) {
        if h.retired {
            continue;
        }
        let (mine, settled) = reports_of(rows, &name, &h, clock);
        if h.race.ascends_to().is_some() {
            match rise_of(&name, h.race, &settled, clock) {
                Ok(p) => out.proposals.extend(p),
                Err(e) => out.unknown.push((name, e)),
            }
            continue;
        }
        if h.race == h.race.base() {
            continue;
        }
        match walk(h.race, &settled) {
            Walked::Holds => {}
            Walked::Unknown(e) => out.unknown.push((name, e)),
            Walked::OnProbation { reports, began } => out.probation.push(Probation {
                creature: name,
                race: h.race,
                reports,
                began,
            }),
            Walked::Falls { at, why, ended } => out.proposals.push(fall_of(
                &name,
                h.race,
                (&mine, &settled),
                (at, why, ended),
                clock,
            )),
        }
    }
    out
}

/// What the law proposes now, from `rows` and `roster`: read at the ledger's clock, the newest
/// report's trusted time. Nothing with no report.
#[must_use]
pub fn propose(rows: &[ReportRow], roster: &[Row]) -> Survey {
    match rows.iter().map(ReportRow::ts).max() {
        Some(clock) => propose_at(rows, roster, clock),
        None => Survey::default(),
    }
}

/// `world`'s survey, from the trusted journals and the witness under the state directory `state`.
///
/// # Errors
///
/// The report fold is unavailable, the report ledger kept there does not walk whole, or the
/// witness does not.
pub fn survey_in(world: &Path, state: &Path) -> Result<Survey, String> {
    let (rows, roster) = inputs_in(world, state)?;
    Ok(propose(&rows, &roster))
}

/// [`survey_in`], under the human's state directory.
///
/// # Errors
///
/// As [`survey_in`], or there is no home.
pub fn survey(world: &Path) -> Result<Survey, String> {
    let state = crate::queue::state_dir()
        .ok_or_else(|| crate::reports::Unavailable::NoState.to_string())?;
    survey_in(world, &state)
}

/// The report ledger's rows and the witness's rows for `world` under `state`, each trusted.
fn inputs_in(world: &Path, state: &Path) -> Result<(Vec<ReportRow>, Vec<Row>), String> {
    let folded = crate::reports::fold_in(world, state).map_err(|e| e.to_string())?;
    // The report ledger kept beside the journals: one that does not walk whole was written by
    // something other than the fold, and nothing is proposed from beside it.
    let kept = crate::queue::state_file_in(state, world, "reports");
    if let Ok(bytes) = std::fs::read(&kept) {
        let mut w = Walk::new();
        w.feed(&bytes);
        let r = w.report();
        if let Some((line, why)) = r.broken {
            return Err(format!(
                "the report ledger's chain is broken at line {line}: {why} — `mnemorphd meters` \
                 rebuilds it from the journals"
            ));
        }
    }
    let roster = creatures::rows_in(Some(state), world)?
        .ok_or("no home: the creatures' witness is kept under the human's state directory")?;
    Ok((folded.rows, roster))
}

/// The witness row that applies `p`, from the creature's latest row in `roster`: its race the
/// proposal's, by the human, naming the proposal, its digest and the time it was read at.
#[must_use]
pub fn row_for(p: &Proposal, roster: &[Row]) -> Option<Row> {
    let last = roster.iter().rev().find(|r| r.name == p.creature)?;
    let mut row = creatures::row(
        p.act.act(),
        &p.creature,
        p.to,
        last.reports_to.as_deref(),
        last.ground.clone(),
        last.retired,
        HUMAN,
    );
    row.minds.clone_from(&last.minds);
    row.proposal = Some(p.id.clone());
    row.inputs_sha256 = Some(p.inputs_sha256.clone());
    row.as_of = Some(p.as_of);
    Some(row)
}

/// The proposal of `act` for `creature` in `world`, read now from the trusted journals and the
/// witness under `state`.
///
/// # Errors
///
/// The inputs cannot be trusted ([`survey_in`]), or the law proposes no such move.
pub fn proposed_in(
    world: &Path,
    state: &Path,
    creature: &str,
    act: Move,
) -> Result<Proposal, String> {
    let survey = survey_in(world, state)?;
    match survey.of(creature) {
        Some(p) if p.act == act => Ok(p.clone()),
        Some(p) => Err(format!(
            "the law proposes a {} for {creature}, not a {}",
            p.act.word(),
            act.word()
        )),
        None => Err(format!(
            "the law proposes no {} for {creature}: nothing to apply",
            act.word()
        )),
    }
}

/// The human's apply: the proposal of `act` for `creature`, read again now and held to the `id`
/// the human was shown, written to the witness as a `rise` or `fall` row. Refused inside a body's
/// command, under a harness process, with no home or a state directory inside the world
/// ([`creatures::record_human_in`]), and when the proposal moved since it was shown.
///
/// # Errors
///
/// As [`proposed_in`] and [`creatures::record_human_in`], or the proposal is no longer `id`.
pub fn apply_in(
    state: Option<&Path>,
    world: &Path,
    creature: &str,
    act: Move,
    id: &str,
) -> Result<Proposal, String> {
    let state =
        state.ok_or("no home: the creatures' rows are kept under the human's state directory")?;
    crate::pins::outside_as(world, state, "the creatures' rows are")?;
    let p = proposed_in(world, state, creature, act)?;
    if p.id != id {
        return Err(format!(
            "the proposal moved since it was shown ({id} is now {}): nothing was written",
            p.id
        ));
    }
    let roster = creatures::rows_in(Some(state), world)?.unwrap_or_default();
    let row = row_for(&p, &roster).ok_or("the creature has no row in the witness")?;
    creatures::record_human_in(Some(state), world, vec![row])?;
    Ok(p)
}

/// One `rise` or `fall` row a replay reproduced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replayed {
    /// The row's line in the witness, from 1.
    pub line: usize,
    /// The creature.
    pub creature: String,
    /// The move.
    pub act: Act,
    /// The race it moved to.
    pub to: String,
    /// The proposal's id.
    pub proposal: String,
}

/// Every `rise` and `fall` row of the witness `roster`, recomputed from the report ledger's
/// `rows` and the witness's rows before it, at the time the row was read at: each must name the
/// very proposal the law makes there — its id, its digest and its race.
///
/// # Errors
///
/// The first row that does not reproduce, and why.
pub fn replay(rows: &[ReportRow], roster: &[Row]) -> Result<Vec<Replayed>, String> {
    let mut out = Vec::new();
    for (n, r) in roster.iter().enumerate() {
        let act = match r.act {
            Act::Rise => Move::Rise,
            Act::Fall => Move::Fall,
            _ => continue,
        };
        let line = n + 1;
        let as_of = r
            .as_of
            .ok_or_else(|| format!("row {line}: a {} names no time it was read at", act.word()))?;
        let survey = propose_at(rows, &roster[..n], as_of);
        let p = survey.of(&r.name).filter(|p| p.act == act).ok_or_else(|| {
            format!(
                "row {line}: the law proposes no {} for {} at {as_of}",
                act.word(),
                r.name
            )
        })?;
        if r.proposal.as_deref() != Some(p.id.as_str())
            || r.inputs_sha256.as_deref() != Some(p.inputs_sha256.as_str())
            || r.race != p.to.name()
        {
            return Err(format!(
                "row {line}: {}'s {} names proposal {} ({}), the law reads {} ({}) to {}",
                r.name,
                act.word(),
                r.proposal.as_deref().unwrap_or("none"),
                r.race,
                p.id,
                p.inputs_sha256,
                p.to.name()
            ));
        }
        out.push(Replayed {
            line,
            creature: r.name.clone(),
            act: r.act,
            to: r.race.clone(),
            proposal: p.id.clone(),
        });
    }
    Ok(out)
}

/// [`replay`] for `world`, from the trusted journals and the witness under `state`.
///
/// # Errors
///
/// As [`survey_in`] and [`replay`].
pub fn replay_in(world: &Path, state: &Path) -> Result<Vec<Replayed>, String> {
    let (rows, roster) = inputs_in(world, state)?;
    replay(&rows, &roster)
}

/// Each pending proposal and probation of `world`, in lines for `status` and `mnemorphd meters`,
/// each with the command that applies it; one line saying why when nothing can be read.
/// Unescaped: the caller escapes for its surface.
#[must_use]
pub fn pending_lines(world: &Path) -> Vec<String> {
    match survey(world) {
        Ok(s) => lines_of(&s, world),
        Err(e) => vec![format!("rises and falls unavailable — {e}")],
    }
}

/// The lines of `s`, for `world`.
#[must_use]
pub fn lines_of(s: &Survey, world: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for p in &s.proposals {
        let meters: Vec<String> = p.meters.iter().map(|(m, r)| format!("{m} {r}")).collect();
        out.push(format!(
            "{}: {} proposed ({}; {} reports) — the human applies it with: {}",
            p.creature,
            p.what(),
            meters.join(" · "),
            p.n,
            p.command(world)
        ));
    }
    out.extend(s.probation.iter().map(ToString::to_string));
    out
}

#[cfg(test)]
mod tests;
