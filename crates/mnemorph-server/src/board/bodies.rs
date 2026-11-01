//! A world's colony, as `GET /api/w/<id>/bodies` shows it: every creature its docs describe — its
//! race, its post or speciality, the creature it reports to, its ground, whether it works now, the
//! Minds it wears and where each stands with the witness, and its latest acts — beside the race
//! law, compiled in, for every race, born in this world or not.
//!
//! Each part says where it was read. The roster is the world's own docs, copied first under the
//! snapshot's caps: shown, granting nothing. A Mind's standing is the witness's, kept under the
//! human's state directory: trusted. Whether a body works now is read from the run journals and
//! locks inside the world: shown. The race law is compiled into the binary: trusted.

use std::path::Path;
use std::time::{Duration, SystemTime};

use mnemorph_loop::world::Roster;
use mnemorph_loop::world::chains;
use mnemorph_loop::world::creatures::{self, Standing as Trust};
use mnemorph_loop::world::doc::Creature;
use mnemorph_loop::world::log::LOG;
use mnemorph_loop::world::minds::{self, Standing, witness};
use mnemorph_loop::world::races::{
    Gate, Kind, ModelTier, Op, Post, Race, Scale, Soil, Speciality, law_bars, meter_bars,
};
use serde::Serialize;
use ts_rs::TS;

use super::journal;
use super::snapshot;
use super::timeline::{self, Commission, CommissionState, TimelineEntry};

/// The most acts a body's card lists.
pub const ACTS: usize = 6;

/// The most paths of one ground listed.
pub const GROUND: usize = 40;

/// The most roster problems listed.
pub const PROBLEMS: usize = 50;

/// A commission with no end, its journal written this recently while a session is live in the
/// world, reads as working.
pub const WORKING: Duration = Duration::from_mins(15);

/// Where a part of the answer was read, and whether that source is trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Provenance {
    /// Where it was read.
    pub source: String,
    /// Whether it is the harness's own (under the human's state directory, or compiled in)
    /// rather than read inside the world, which a body can write.
    pub trusted: bool,
    /// What qualifies it, or why it is unknown.
    pub why: Option<String>,
}

fn prov(source: &str, trusted: bool, why: Option<String>) -> Provenance {
    Provenance {
        source: source.to_owned(),
        trusted,
        why,
    }
}

/// Whether a body works now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum BodyState {
    /// A commission runs: its journal has no end, and was written lately while a session is live.
    Working,
    /// Its last commission ended.
    Idle,
    /// Its last commission has no end, and nothing runs it: cut mid-turn.
    Cut,
    /// No commission journal names it.
    Never,
    /// Its doc retired it.
    Retired,
}

/// A body's live state, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct LiveState {
    /// The state.
    pub state: BodyState,
    /// Why, in words.
    pub why: String,
    /// The commission it rests on.
    pub run: Option<String>,
    /// When that commission started (RFC 3339, UTC).
    pub at: Option<String>,
}

/// A path a creature's doc claims, and as what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Ground {
    /// `zone`, `domain`, `post`, `lane` or `surfaces`.
    pub kind: String,
    /// The path.
    pub path: String,
}

/// Where a Mind stands with the witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum MindStanding {
    /// Adopted, its doc unchanged: it loads.
    Adopted,
    /// Never adopted: it does not load.
    Unadopted,
    /// Changed since adoption: it does not load until re-adopted.
    Changed,
    /// Retired by the human.
    Retired,
    /// The witness does not verify: no Mind loads.
    Unverified,
    /// The doc names it, but no such Mind is in the world.
    Absent,
}

/// A Mind a body wears, or may wear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BodyMind {
    /// Its name.
    pub name: String,
    /// Where it stands with the witness.
    pub standing: MindStanding,
    /// Shared: any body may wear it.
    pub shared: bool,
    /// What it is for, in one line, as its doc says.
    pub about: String,
    /// Its owner, as the witness recorded it.
    pub owner: Option<String>,
}

/// One creature of the colony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardBody {
    /// Its name.
    pub name: String,
    /// Its race (`high-orc`).
    pub race: String,
    /// Its base race: the lane it is drawn in.
    pub lane: String,
    /// Who it reports to, as its doc says; `null` when the doc names no one.
    pub parent: Option<String>,
    /// A Kijin's post (`forge`), read from its name.
    pub post: Option<String>,
    /// A Dark Elf's speciality (`supply`, `general`), read from its name.
    pub speciality: Option<String>,
    /// The paths its doc claims.
    pub ground: Vec<Ground>,
    /// Its verify lines, counted.
    pub verify: u32,
    /// Retired by its doc.
    pub retired: bool,
    /// The creature whose ground it took.
    pub heir_of: Option<String>,
    /// Its doc, relative to the world.
    pub doc: String,
    /// The Minds its doc names, and the shared ones, with their standing.
    pub minds: Vec<BodyMind>,
    /// Whether it works now.
    pub live: LiveState,
    /// Its latest acts: commissions and verdicts, newest first.
    pub acts: Vec<TimelineEntry>,
    /// How its card stands against the creatures' witness under the state directory: `throne`,
    /// `trusted`, `mismatched` (it runs at the lower of its card and its row) or `stranger` (no
    /// row: its own seat alone).
    pub standing: String,
    /// The race it runs as: its row's, held to its card ([`Roster::load`]); a stranger's is
    /// `slime`.
    pub runs_as: String,
    /// The fields its card says otherwise than its row (`Race`, `Post`).
    pub differs: Vec<String>,
}

/// Whether the throne — the session's own body — is live in the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ThroneLive {
    /// A session holds a run in the world now.
    pub live: bool,
    /// Why, in words.
    pub why: String,
}

/// A headline meter and its bars, percentages.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct LawMeter {
    /// What it measures.
    pub label: String,
    /// At or over: ok.
    pub ok: f64,
    /// At or over: warn; under: low.
    pub warn: f64,
}

/// One bar of a form's rise or fall.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct LawBarView {
    /// `rise`, `probation` or `fall`.
    pub kind: String,
    /// The meter.
    pub meter: String,
    /// `rate` (a percentage) or `count`.
    pub scale: String,
    /// `<`, `<=`, `>=` or `>`.
    pub op: String,
    /// The threshold.
    pub threshold: f64,
}

/// A Kijin post, compiled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct LawPost {
    /// The creature that holds it (`kijin-forge`).
    pub creature: String,
    /// The outward acts it may do.
    pub acts: Vec<String>,
}

/// A Dark Elf speciality, compiled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct LawSpeciality {
    /// The creature that bears it (`dark-elf-supply`).
    pub creature: String,
    /// The attack classes it owns.
    pub classes: Vec<String>,
    /// The surfaces it is routed, compiled.
    pub surfaces: Vec<String>,
}

/// A race, as the compiled law says it: a race card.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct RaceLaw {
    /// Its name (`high-orc`).
    pub race: String,
    /// Its title (`High Orc`).
    pub title: String,
    /// Its base race.
    pub base: String,
    /// An ascended form.
    pub ascended: bool,
    /// The form it ascends to.
    pub ascends_to: Option<String>,
    /// The race it reports to; `null` for the throne, which answers to the human.
    pub reports_to: Option<String>,
    /// The base races it commissions.
    pub commands: Vec<String>,
    /// Whether it commissions its own race sideways.
    pub sideways: bool,
    /// Its shelf: the tool classes it may use.
    pub shelf: Vec<String>,
    /// The soil it may write, in words.
    pub soil: Vec<String>,
    /// Who judges its writes, in words.
    pub gate: String,
    /// Whether it writes code.
    pub writes_code: bool,
    /// Steps one commission may take.
    pub steps: u32,
    /// Its largest share of its parent's budget, in percent.
    pub max_share: u8,
    /// The model tier that rides it by default.
    pub tier: String,
    /// Its headline meter.
    pub meter: LawMeter,
    /// Its rise and fall bars (an ascended form's).
    pub bars: Vec<LawBarView>,
    /// Its posts (a Kijin's, an Oni's).
    pub posts: Vec<LawPost>,
    /// Its specialities (a Dark Elf's, an Arch Dark Elf's).
    pub specialities: Vec<LawSpeciality>,
    /// Living bodies of this race in the world.
    pub living: u32,
    /// Its card's art, by exact name.
    pub art: String,
}

/// `GET /api/w/<id>/bodies`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct BoardBodies {
    /// The creatures, in the roster's order (race, then seat).
    pub bodies: Vec<BoardBody>,
    /// Docs that could not be read or do not hold together, and why.
    pub problems: Vec<String>,
    /// Creatures whose chain of command comes back to themselves.
    pub loops: Vec<String>,
    /// Whether the throne is live.
    pub throne: ThroneLive,
    /// Every race of the law, in its order.
    pub races: Vec<RaceLaw>,
    /// Where the roster was read.
    pub roster: Provenance,
    /// Where each creature's standing was read: the creatures' witness.
    pub witness: Provenance,
    /// Where the Minds' standing was read.
    pub minds: Provenance,
    /// Where the live states were read.
    pub live: Provenance,
    /// Where the acts were read.
    pub acts: Provenance,
    /// Where the race law comes from.
    pub law: Provenance,
}

/// A race's title, as the cards print it.
#[must_use]
pub fn title(race: Race) -> &'static str {
    match race {
        Race::Rimuru => "Rimuru",
        Race::Elf => "Elf",
        Race::Orc => "Orc",
        Race::Slime => "Slime",
        Race::Kijin => "Kijin",
        Race::DarkElf => "Dark Elf",
        Race::HighElf => "High Elf",
        Race::HighOrc => "High Orc",
        Race::NamedSlime => "Named Slime",
        Race::Oni => "Oni",
        Race::ArchDarkElf => "Arch Dark Elf",
    }
}

/// The art of `race`'s card: `/art/cards/<race>.webp`.
#[must_use]
pub fn art(race: Race) -> String {
    format!("/art/cards/{}.webp", race.name())
}

fn soil_words(s: Soil) -> &'static str {
    match s {
        Soil::OwnDoc => "its own doc",
        Soil::Zone => "its zone",
        Soil::VerifyBlock => "its Slimes' verify lines",
        Soil::Post => "its post",
        Soil::ProvingGround => "its proving ground (.mnemorph/dark-elf/)",
        Soil::Throne => "the plan and the dimension manifest",
    }
}

fn gate_words(g: Gate) -> &'static str {
    match g {
        Gate::ByItsOrc => "judged by the verify lines its Orc wrote",
        Gate::OverItsSlimes => "holds the gate over its Slimes",
        Gate::OwnSoil => "holds its own gate over its own soil",
        Gate::DocOnly => "writes only its doc; the gate checks that doc",
    }
}

fn tier_word(t: ModelTier) -> &'static str {
    match t {
        ModelTier::Frontier => "frontier",
        ModelTier::Strong => "strong",
        ModelTier::Fast => "fast",
    }
}

fn kind_word(k: Kind) -> &'static str {
    match k {
        Kind::Rise => "rise",
        Kind::Probation => "probation",
        _ => "fall",
    }
}

fn op_word(o: Op) -> &'static str {
    o.symbol()
}

/// The post a creature holds, as a word (`forge`).
fn post_of(name: &str) -> Option<String> {
    Post::of(name).map(|p| p.creature().trim_start_matches("kijin-").to_owned())
}

/// The speciality a creature bears, as a word (`supply`, `general`).
fn speciality_of(name: &str) -> Option<String> {
    Speciality::of(name).map(|s| match s.creature().strip_prefix("dark-elf-") {
        Some(w) => w.to_owned(),
        None => "general".to_owned(),
    })
}

/// Every race of the law, `living` counting each race's living bodies.
#[must_use]
pub fn races(living: impl Fn(Race) -> u32) -> Vec<RaceLaw> {
    Race::ALL
        .into_iter()
        .map(|r| {
            let (label, ok, warn) = meter_bars(r);
            let base = r.base();
            RaceLaw {
                race: r.name().to_owned(),
                title: title(r).to_owned(),
                base: base.name().to_owned(),
                ascended: r.is_ascended(),
                ascends_to: r.ascends_to().map(|a| a.name().to_owned()),
                reports_to: r.reports_to().map(|a| a.name().to_owned()),
                commands: r.commands().iter().map(|c| c.name().to_owned()).collect(),
                sideways: r.sideways(),
                shelf: r.shelf().iter().map(ToString::to_string).collect(),
                soil: r.soil().iter().map(|s| soil_words(*s).to_owned()).collect(),
                gate: gate_words(r.gate()).to_owned(),
                writes_code: r.writes_code(),
                steps: r.steps(),
                max_share: r.max_share(),
                tier: tier_word(r.default_tier()).to_owned(),
                meter: LawMeter {
                    label: label.to_owned(),
                    ok,
                    warn,
                },
                bars: law_bars(r)
                    .into_iter()
                    .map(|b| LawBarView {
                        kind: kind_word(b.kind).to_owned(),
                        meter: b.meter.to_owned(),
                        scale: match b.scale {
                            Scale::Rate => "rate",
                            Scale::Count => "count",
                        }
                        .to_owned(),
                        op: op_word(b.op).to_owned(),
                        threshold: b.threshold,
                    })
                    .collect(),
                posts: if base == Race::Kijin {
                    Post::ALL
                        .into_iter()
                        .map(|p| LawPost {
                            creature: p.creature().to_owned(),
                            acts: p.acts().iter().map(ToString::to_string).collect(),
                        })
                        .collect()
                } else {
                    Vec::new()
                },
                specialities: if base == Race::DarkElf {
                    Speciality::ALL
                        .into_iter()
                        .map(|s| LawSpeciality {
                            creature: s.creature().to_owned(),
                            classes: s.classes().iter().map(ToString::to_string).collect(),
                            surfaces: s.surfaces().iter().map(|p| (*p).to_owned()).collect(),
                        })
                        .collect()
                } else {
                    Vec::new()
                },
                living: living(r),
                art: art(r),
            }
        })
        .collect()
}

/// The paths a creature's doc claims, each with its kind, at most [`GROUND`].
fn ground(c: &Creature) -> Vec<Ground> {
    [
        ("zone", &c.zone),
        ("domain", &c.domain),
        ("post", &c.post),
        ("lane", &c.lane),
        ("surfaces", &c.surfaces),
    ]
    .into_iter()
    .flat_map(|(kind, paths)| {
        paths.iter().map(move |p| Ground {
            kind: kind.to_owned(),
            path: p.clone(),
        })
    })
    .take(GROUND)
    .collect()
}

fn standing(s: &Standing) -> MindStanding {
    match s {
        Standing::Adopted => MindStanding::Adopted,
        Standing::Unadopted => MindStanding::Unadopted,
        Standing::Changed => MindStanding::Changed,
        Standing::Retired => MindStanding::Retired,
        Standing::Unverified => MindStanding::Unverified,
    }
}

/// The Minds `c` names, then the shared ones, each with its standing in `survey`.
fn minds_of(c: &Creature, survey: &minds::Survey) -> Vec<BodyMind> {
    let about = |s: &minds::Surveyed| super::runs::one_line(&s.mind.about, 160);
    let mut out: Vec<BodyMind> = c
        .minds
        .iter()
        .map(|m| m.trim_matches('`'))
        .filter(|m| !m.is_empty())
        .map(
            |name| match survey.minds.iter().find(|s| s.mind.name == name) {
                Some(s) => BodyMind {
                    name: name.to_owned(),
                    standing: standing(&s.standing),
                    shared: s.mind.shared,
                    about: about(s),
                    owner: s.owner.clone(),
                },
                None => BodyMind {
                    name: name.to_owned(),
                    standing: MindStanding::Absent,
                    shared: false,
                    about: String::new(),
                    owner: None,
                },
            },
        )
        .collect();
    for s in survey.minds.iter().filter(|s| s.mind.shared) {
        if out.iter().all(|m| m.name != s.mind.name) {
            out.push(BodyMind {
                name: s.mind.name.clone(),
                standing: standing(&s.standing),
                shared: true,
                about: about(s),
                owner: s.owner.clone(),
            });
        }
    }
    out
}

/// Whether `c` works now, from its latest commission; `session` whether a session is live in the
/// world, `now` the clock.
#[must_use]
pub fn live_of(
    retired: bool,
    latest: Option<&Commission>,
    session: bool,
    now: SystemTime,
) -> LiveState {
    if retired {
        return LiveState {
            state: BodyState::Retired,
            why: "its doc retired it: it owns nothing and commands no one".into(),
            run: None,
            at: None,
        };
    }
    let Some(c) = latest else {
        return LiveState {
            state: BodyState::Never,
            why: "no commission journal names it yet".into(),
            run: None,
            at: None,
        };
    };
    let recent = c
        .written
        .and_then(|w| now.duration_since(w).ok())
        .is_some_and(|d| d <= WORKING);
    let (state, why) = match c.state {
        CommissionState::Ended => (
            BodyState::Idle,
            format!(
                "its last commission ended: {}",
                c.ending.as_deref().unwrap_or("ended")
            ),
        ),
        CommissionState::Open if session && recent => (
            BodyState::Working,
            "a commission runs: its journal has no end, and a session is live".into(),
        ),
        CommissionState::Open => (
            BodyState::Cut,
            "its last commission has no end, and nothing runs it now".into(),
        ),
        CommissionState::Unknown => (
            BodyState::Cut,
            format!(
                "its last commission's journal does not read: {}",
                c.why.as_deref().unwrap_or("unknown")
            ),
        ),
    };
    LiveState {
        state,
        why,
        run: Some(c.run.clone()),
        at: Some(c.at.to_string()),
    }
}

/// One creature of the colony: its doc's fields, its Minds in `survey`, its live state from its
/// latest of `commissions` (the throne's from the session), its acts among `all`.
fn body_of(
    c: &Creature,
    trusted: &Roster,
    survey: &minds::Survey,
    commissions: &[Commission],
    all: &[TimelineEntry],
    (session, throne, held): (bool, &ThroneLive, &[String]),
    now: SystemTime,
) -> BoardBody {
    let latest = commissions.iter().find(|x| x.body == c.name);
    let mut live = live_of(c.retired, latest, session, now);
    if c.race == Race::Rimuru && !c.retired {
        live = LiveState {
            state: if throne.live {
                BodyState::Working
            } else {
                BodyState::Idle
            },
            why: throne.why.clone(),
            run: held.first().cloned(),
            at: None,
        };
    }
    BoardBody {
        name: c.name.clone(),
        race: c.race.name().to_owned(),
        lane: c.race.base().name().to_owned(),
        parent: c.reports_to.clone(),
        post: post_of(&c.name),
        speciality: speciality_of(&c.name),
        ground: ground(c),
        verify: u32::try_from(c.verify.len()).unwrap_or(u32::MAX),
        retired: c.retired,
        heir_of: c.heir_of.clone(),
        doc: c.doc(),
        minds: minds_of(c, survey),
        live,
        acts: all
            .iter()
            .filter(|e| e.body == c.name)
            .take(ACTS)
            .cloned()
            .collect(),
        standing: trusted
            .standing
            .get(&c.name)
            .map_or("stranger", Trust::word)
            .to_owned(),
        runs_as: trusted
            .creatures
            .get(&c.name)
            .map_or(Race::Slime, |t| t.race)
            .name()
            .to_owned(),
        differs: match trusted.standing.get(&c.name) {
            Some(Trust::Mismatched(f)) => f.iter().map(|f| (*f).to_owned()).collect(),
            _ => Vec::new(),
        },
    }
}

/// `world`'s colony; `chats` how many board chats are open in it.
///
/// # Errors
///
/// The creature docs or the Minds cannot be copied: a file that is no regular file, too large,
/// or too many entries.
pub fn bodies(world: &Path, chats: u32) -> Result<BoardBodies, String> {
    let snap = snapshot::bodies(world)?;
    // The cards as copied are shown; what each runs as is the witness's, read for the world.
    let roster = Roster::cards(snap.path());
    let trusted = creatures::trusted(roster.clone(), &creatures::witnessed(world));
    let log = || {
        if snapshot::parents_plain(world, Path::new(LOG)).is_err() {
            return String::new();
        }
        journal::tail(&world.join(LOG), journal::JOURNAL_BYTES)
            .ok()
            .flatten()
            .map(|(t, ..)| t)
            .unwrap_or_default()
    };
    let at = witness::path_of(world);
    let anchor = at
        .as_ref()
        .and_then(|_| chains::anchor_with(world, "minds", log));
    let survey = minds::survey_at(snap.path(), at.as_deref(), anchor.as_ref());
    let held = mnemorph_loop::resume::live(world);
    let session = chats > 0 || !held.is_empty();
    let throne = ThroneLive {
        live: session,
        why: if chats > 0 {
            format!("{chats} chat(s) open on the board")
        } else if let Some(run) = held.first() {
            format!("run {run} is held by a live session")
        } else {
            "no session holds a run".into()
        },
    };
    let (commissions, older) = timeline::commissions(world);
    let (all, _) = timeline::entries_with(world, &commissions);
    let now = SystemTime::now();
    let bodies: Vec<BoardBody> = roster
        .creatures
        .values()
        .map(|c| {
            body_of(
                c,
                &trusted,
                &survey,
                &commissions,
                &all,
                (session, &throne, &held),
                now,
            )
        })
        .collect();
    let count = |r: Race| {
        u32::try_from(roster.living().filter(|c| c.race == r).count()).unwrap_or(u32::MAX)
    };
    Ok(BoardBodies {
        problems: roster.problems.iter().take(PROBLEMS).cloned().collect(),
        loops: roster.loops(),
        throne,
        races: races(count),
        roster: prov(
            ".mnemorph/<race>/<seat>/README.md (in the world)",
            false,
            Some(
                "a body can write its doc: shown, granting nothing — each runs as the creatures' \
                 witness under the state directory holds it"
                    .into(),
            ),
        ),
        witness: prov(
            "the creatures' witness under the state directory (each creature's race, chain, \
             post and ground)",
            trusted.witness.is_none(),
            trusted.witness.clone(),
        ),
        minds: prov(
            "the Minds' witness under the state directory (a Mind's about and shared mark are \
             its doc's, in the world)",
            at.is_some() && survey.error.is_none(),
            survey.error.clone().or_else(|| {
                at.is_none()
                    .then(|| "no home: the witness cannot be read, no Mind loads".to_owned())
            }),
        ),
        live: prov(
            ".mnemorph/instruments/loop/ (run journals and locks, in the world)",
            false,
            (older > 0).then(|| format!("{older} older journal(s) not read")),
        ),
        acts: prov(
            "the run journals (in the world) and the gate's verdict journal (state directory)",
            false,
            Some("each act carries its own flag: only a verdict is trusted".into()),
        ),
        law: prov("the race law, compiled into mnemorphd", true, None),
        bodies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemorph_loop::world::ledger::Timestamp;

    fn commission(state: CommissionState, written: SystemTime) -> Commission {
        Commission {
            body: "slime-a".into(),
            run: "slime-a-1-0".into(),
            at: Timestamp(0),
            ask: None,
            state,
            ending: Some("answered".into()),
            verdict: None,
            changed: 0,
            why: None,
            written: Some(written),
        }
    }

    #[test]
    fn the_live_state_reads_the_latest_commission() {
        let now = SystemTime::now();
        let old = now - Duration::from_hours(2);
        assert_eq!(live_of(true, None, true, now).state, BodyState::Retired);
        assert_eq!(live_of(false, None, true, now).state, BodyState::Never);
        let ended = commission(CommissionState::Ended, now);
        assert_eq!(
            live_of(false, Some(&ended), true, now).state,
            BodyState::Idle
        );
        let open = commission(CommissionState::Open, now);
        assert_eq!(
            live_of(false, Some(&open), true, now).state,
            BodyState::Working
        );
        assert_eq!(
            live_of(false, Some(&open), false, now).state,
            BodyState::Cut
        );
        let stale = commission(CommissionState::Open, old);
        assert_eq!(
            live_of(false, Some(&stale), true, now).state,
            BodyState::Cut
        );
        let unread = commission(CommissionState::Unknown, now);
        assert_eq!(
            live_of(false, Some(&unread), true, now).state,
            BodyState::Cut
        );
    }

    #[test]
    fn every_race_has_a_card_even_unborn() {
        let all = races(|r| u32::from(r == Race::Orc));
        assert_eq!(all.len(), Race::ALL.len());
        let orc = all.iter().find(|r| r.race == "orc").unwrap();
        assert_eq!((orc.living, orc.reports_to.as_deref()), (1, Some("elf")));
        assert!(orc.sideways);
        let oni = all.iter().find(|r| r.race == "oni").unwrap();
        assert_eq!(oni.posts.len(), Post::ALL.len());
        assert!(oni.bars.iter().any(|b| b.kind == "rise"));
        assert!(oni.bars.iter().any(|b| b.kind == "fall"));
        let arch = all.iter().find(|r| r.race == "arch-dark-elf").unwrap();
        assert_eq!(arch.specialities.len(), Speciality::ALL.len());
        assert!(arch.bars.iter().any(|b| b.kind == "probation"));
        let rimuru = all.iter().find(|r| r.race == "rimuru").unwrap();
        assert_eq!(rimuru.reports_to, None);
        assert_eq!(rimuru.art, "/art/cards/rimuru.webp");
        assert_eq!(post_of("kijin-forge").as_deref(), Some("forge"));
        assert_eq!(speciality_of("dark-elf").as_deref(), Some("general"));
        assert_eq!(speciality_of("dark-elf-web").as_deref(), Some("web"));
        assert_eq!(speciality_of("elf-a"), None);
    }
}
