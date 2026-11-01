//! A landing's Dark Elf challenge, as the harness keeps it: the landing as it was routed, its tree
//! and its author, how each routed speciality is staffed, each routed speciality's coverage
//! ledger, the findings that block it, and the human's waiver. The challenge is pending until every
//! routed speciality has filed a ledger that holds, and every finding of medium or higher has its
//! verifier's verdict ([`verification`]); a critical finding, whatever its verdict, or a high one
//! confirmed at high, blocks it whatever was filed; a human waiver, with its
//! reason, settles it either way.
//!
//! The records are the harness's, outside the world and out of every command's reach:
//! `$XDG_STATE_HOME/mnemorph/challenges/<world>-<fnv64a>.jsonl`, one row per line, the folder
//! `0700` and the file `0600`. Every read and every write holds the file's lock, so a filing is
//! checked against the records and written whole before another is read. A ledger is kept only
//! once it validates, and every filing is validated again when the records are read, so a line
//! written by anything else counts for nothing. Every row is chained by hash
//! ([`mnemorph_ledger::chain`]), so an edit of the value ledger's history is found
//! ([`crate::chains`]).

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use mnemorph_ledger::chain::Walk;
use mnemorph_proving::World;
use mnemorph_proving::coverage::{
    CoverageLedger, Landing, Ran, Refused, Routed, Sources, Surface, stamp, validate, validate_in,
};
use mnemorph_races::Speciality;
use serde::{Deserialize, Serialize};

use crate::roster::Roster;

pub mod verification;
pub(crate) use verification::finding_rows_of;

/// The records' hash chain, as a walk reports it.
pub use mnemorph_ledger::Report as ChainReport;
pub use mnemorph_ledger::{
    Ascension, Build, CANNOT_VERIFY_BAR, FindingRow, Proposed, SpecialityFold, Standing, WINDOW,
    fold_findings_by_build,
};
pub use verification::{
    Decision, FLOOR, Filing, Finding, Found, Guard, Outcome, Pinned, Proposal, Rerun, Ruling,
    Verification,
};

/// The longest one row may be, in bytes: a filing past it is refused, never split.
pub const MAX_ROW: usize = 1024 * 1024;

/// Where a world's challenges are kept: `$XDG_STATE_HOME/mnemorph/challenges/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` when there is no home.
#[must_use]
pub fn records_of(world: &Path) -> Option<PathBuf> {
    crate::queue::state_file(world, "challenges")
}

/// The landing `id` wrote `written`, routed as the roster routes it: each speciality, and the
/// surfaces of its — compiled, then those its living creature's doc declares — the paths touched.
#[must_use]
pub fn route(roster: &Roster, id: &str, written: &[String]) -> Landing {
    let written: Vec<String> = written
        .iter()
        .map(|p| crate::own::normal(p).unwrap_or_else(|| p.clone()))
        .collect();
    let routed = roster
        .route(&written)
        .into_iter()
        .map(|s| {
            let compiled = s.surfaces().iter().map(|g| Surface {
                at: (*g).to_owned(),
                declared: false,
            });
            let declared = roster
                .living()
                .filter(|c| Speciality::of(&c.name) == Some(s))
                .flat_map(|c| c.surfaces.iter())
                .map(|claim| Surface {
                    at: claim.clone(),
                    declared: true,
                });
            let mut surfaces: Vec<Surface> = Vec::new();
            for surface in compiled.chain(declared) {
                if written.iter().any(|p| surface.holds(p)) && !surfaces.contains(&surface) {
                    surfaces.push(surface);
                }
            }
            Routed {
                speciality: s,
                surfaces,
            }
        })
        .collect();
    Landing {
        id: id.to_owned(),
        written,
        routed,
    }
}

/// The Dark Elf's value ledger for `world`: its findings folded per speciality, from the
/// challenge records alone ([`Challenges::finding_rows`]). The records are read only from the
/// human's state directory, outside the world: never a journal, a finding record or any file a
/// body could write inside it.
///
/// # Errors
///
/// There is no home to keep records under, or the records' path lies inside the world.
pub fn scores(world: &Path) -> Result<BTreeMap<String, SpecialityFold>, Refusal> {
    scored(world).map(|(folded, _)| folded)
}

/// [`scores`], with the records' hash chain walked over the same bytes the fold read — one read
/// under the lock, so the head a proposal names is the head of the rows it was folded from;
/// `None` when the records cannot be read.
///
/// # Errors
///
/// As [`scores`].
pub fn scored(world: &Path) -> Result<Scored, Refusal> {
    let records = records_of(world).ok_or(Refusal::NoState)?;
    scored_at(world, &records, true)
}

/// Each speciality's fold, and the records' chain as the same read walked it.
pub type Scored = (BTreeMap<String, SpecialityFold>, Option<ChainReport>);

/// The fold of `records` for `world`, and the chain the same read walked; `outside` off skips the
/// source check: how a test shows it is needed.
fn scored_at(world: &Path, records: &Path, outside: bool) -> Result<Scored, Refusal> {
    let real = |p: &Path| {
        p.canonicalize().unwrap_or_else(|_| {
            // Not there yet: its nearest existing ancestor, resolved, with the rest.
            p.ancestors()
                .find_map(|a| {
                    let rest = p.strip_prefix(a).ok()?;
                    a.canonicalize().ok().map(|r| r.join(rest))
                })
                .unwrap_or_else(|| p.to_path_buf())
        })
    };
    if outside && real(records).starts_with(real(world)) {
        return Err(Refusal::InWorld(records.to_path_buf()));
    }
    let (rows, chain) = Challenges::at(records).finding_rows_chained();
    Ok((mnemorph_ledger::fold_findings(&rows), chain))
}

/// Who stands over every body: the only author a waiver may have.
pub const HUMAN: &str = "human";

/// What the harness knows of a landing as it opens its challenge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    /// The SHA-256 of the landing's tree: every written path with its bytes as the landing left
    /// them, a removed one as absent.
    pub tree: String,
    /// The body whose landing it is: never one of its reviewers, never the one that sends them.
    pub by: String,
    /// The bodies above the author, nearest first: none of them files a ledger for it either.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain: Vec<String>,
    /// How many paths the landing wrote, when the routed landing keeps only a sample of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<u64>,
    /// The SHA-256 of every written path, sorted, one per line, when only a sample is kept.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub paths_sha256: String,
    /// When only a sample is kept, the SHA-256 of the sample's tree, as [`Opening::tree`] hashes
    /// the whole: what a later filing can hash again from the records.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sample_tree: String,
}

impl Opening {
    /// The tree a filing is held to: the sample's when only a sample is kept, else the whole's.
    #[must_use]
    pub fn current_tree(&self) -> &str {
        if self.paths.is_some() {
            &self.sample_tree
        } else {
            &self.tree
        }
    }

    /// Whether `body` is the landing's author or above it: such a body never clears it.
    #[must_use]
    pub fn holds_author(&self, body: &str) -> bool {
        self.by == body || self.chain.iter().any(|c| c == body)
    }
}

/// How one routed speciality stands for its review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "staff", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Staff {
    /// Waiting: past the landing's cap of reviewers, or for the challenge reserve. It is
    /// commissioned at a later sweep, never dropped.
    Queued {
        /// Why it waits.
        why: String,
    },
    /// No living body of the world bears the speciality's name: the human is told, and the
    /// challenge stays pending.
    Unstaffed,
    /// Its body was commissioned on the landing.
    Commissioned {
        /// The body, by name.
        body: String,
        /// Who sent it: the throne, never the landing's author.
        by: String,
        /// The weighted tokens its review spent, booked against the challenge.
        wt: u64,
        /// How many sweeps since found it commissioned with no ledger filed: past
        /// a few, it is sent again.
        #[serde(default, skip_serializing_if = "is_zero")]
        waited: u32,
    },
}

// Serde's `skip_serializing_if` passes the field by reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A finding that blocks the landing until it is fixed or the human waives the challenge: a
/// critical one, whatever one verifier says, or a high one confirmed at high.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocker {
    /// The speciality that found it, by its creature name.
    pub speciality: String,
    /// Its root-cause fingerprint.
    pub fingerprint: String,
    /// `critical` or `high`.
    pub severity: String,
    /// What it claims.
    pub claim: String,
    /// Where its verification stands: who confirmed it, or why the human decides.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub standing: String,
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{} {} ({}): {}",
            self.fingerprint, self.severity, self.speciality, self.claim
        )?;
        if !self.standing.is_empty() {
            write!(f, " [{}]", self.standing)?;
        }
        Ok(())
    }
}

/// The human's word that a landing stands without the rest of its challenge. Only
/// [`Waiver::human`] makes one, and the records keep it only with a reason that says something,
/// bound to the tree the challenge was opened on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Waiver {
    by: String,
    reason: String,
    #[serde(default)]
    tree: String,
}

impl Waiver {
    /// The human's waiver, with `reason`. Refused inside a body's command (`MNEMORPH_BODY` is
    /// set there): a body never waives its own challenge.
    ///
    /// # Errors
    ///
    /// A body asks, or the reason says nothing (under 20 characters, a stock phrase, an
    /// unreadable character, over 1 KiB).
    pub fn human(reason: &str) -> Result<Waiver, Refusal> {
        if let Some(body) = std::env::var_os("MNEMORPH_BODY") {
            return Err(Refusal::NotHuman(body.to_string_lossy().into_owned()));
        }
        let reason = reason.trim();
        if !reasoned(reason) {
            return Err(Refusal::Unreasoned);
        }
        Ok(Waiver {
            by: HUMAN.to_owned(),
            reason: reason.to_owned(),
            tree: String::new(),
        })
    }

    /// Why the human waived it.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Whether the records keep it for a challenge opened on `tree`.
    fn holds(&self, tree: &str) -> bool {
        self.by == HUMAN && reasoned(&self.reason) && self.tree == tree
    }
}

/// A reason that says something: substantive, readable, at most 1 KiB.
fn reasoned(reason: &str) -> bool {
    reason.len() <= mnemorph_proving::coverage::MAX_TEXT
        && mnemorph_proving::coverage::substantive(reason)
        && !reason.chars().any(mnemorph_wire::hidden)
}

/// Where a landing's challenge stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Some routed specialities have not filed a valid ledger yet.
    Pending {
        /// They, in routing order.
        awaiting: Vec<Speciality>,
        /// The records' lines, from 1, that did not parse and could be this landing's: until
        /// they are, nothing they held is known, so the challenge cannot clear.
        unreadable: Vec<u64>,
    },
    /// A critical finding, whatever one verifier says, or a high one confirmed at high, blocks
    /// the landing, whatever was filed.
    Blocked {
        /// The findings, in the order they were found.
        findings: Vec<Blocker>,
    },
    /// Every routed speciality has filed and nothing blocks, but findings of medium or higher
    /// still wait on their verifier's verdict.
    Verifying {
        /// Their fingerprints, in the order they were found.
        findings: Vec<String>,
    },
    /// Every routed speciality has filed, every finding has its verdict, and nothing blocks.
    Cleared,
    /// The human waived it.
    Waived {
        /// Why.
        reason: String,
    },
}

impl Status {
    /// Whether the landing still waits on its challenge: pending, blocked or verifying.
    #[must_use]
    pub fn open(&self) -> bool {
        matches!(
            self,
            Status::Pending { .. } | Status::Blocked { .. } | Status::Verifying { .. }
        )
    }
}

/// Why a filing or an opening was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No challenge is open under that id.
    NoSuchLanding(String),
    /// A challenge is already open under that id.
    AlreadyOpen(String),
    /// The id is not 1–128 of `A-Z a-z 0-9 . _ : -`.
    BadId,
    /// This speciality has filed its ledger for that landing already.
    AlreadyFiled(Speciality),
    /// The ledger breaks the state table.
    Invalid(Refused),
    /// A body asked for what only the human gives.
    NotHuman(String),
    /// A waiver with no reason worth the name.
    Unreasoned,
    /// The challenge was waived already.
    AlreadyWaived(String),
    /// The landing's author, or a body above it, filed for it.
    Author(String),
    /// A finding, a verifier, a verdict or a seal proposal breaks these guards.
    Verification(Vec<Guard>),
    /// The row would be over [`MAX_ROW`] bytes.
    TooLarge(usize),
    /// The records could not be read or written.
    Unwritten(String),
    /// There is no home to keep the records under.
    NoState,
    /// The records' path lies inside the world, where a body could write them.
    InWorld(PathBuf),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NoSuchLanding(id) => write!(f, "no challenge is open for landing `{id}`"),
            Refusal::AlreadyOpen(id) => write!(f, "a challenge is already open for `{id}`"),
            Refusal::BadId => f.write_str(
                "a landing id is 1 to 128 characters of letters, digits, `.`, `_`, `:` and `-`",
            ),
            Refusal::AlreadyFiled(s) => write!(f, "{s} has filed its ledger for this landing"),
            Refusal::Invalid(r) => write!(
                f,
                "the ledger breaks the state table; fix every line and file it again:\n{r}"
            ),
            Refusal::NotHuman(body) => write!(
                f,
                "{body} asks: a challenge is the human's to waive, never a body's"
            ),
            Refusal::Unreasoned => f.write_str(
                "a waiver says why: at least 20 characters of reason, not a stock phrase",
            ),
            Refusal::AlreadyWaived(id) => write!(f, "the challenge of `{id}` is waived already"),
            Refusal::Author(body) => write!(
                f,
                "{body} wrote this landing, or commands who did: it never clears its own challenge"
            ),
            Refusal::Verification(guards) => {
                let said: Vec<String> = guards
                    .iter()
                    .map(|g| format!("{g}: {}", g.asks()))
                    .collect();
                write!(f, "refused — {}", said.join("; "))
            }
            Refusal::TooLarge(n) => write!(f, "the filing is {n} bytes, over {MAX_ROW}"),
            Refusal::Unwritten(why) => write!(f, "the challenge records cannot be kept: {why}"),
            Refusal::NoState => f.write_str("no state directory: the challenges cannot be read"),
            Refusal::InWorld(p) => write!(
                f,
                "the challenge records at {} lie inside the world, where a body could write \
                 them: they are never folded",
                p.display()
            ),
        }
    }
}

/// One line of the records. Unknown fields are read past, so an older and a newer binary read
/// each other's rows; a line that still does not parse holds its challenge pending ([`fold`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "row", rename_all = "kebab-case")]
enum Row {
    Opened {
        landing: Landing,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opening: Option<Opening>,
    },
    Filed {
        ledger: CoverageLedger,
    },
    Staffed {
        landing: String,
        speciality: String,
        staff: Staff,
    },
    /// A blocker recorded before findings were verified: it still blocks — fail closed.
    Blocked {
        landing: String,
        blocker: Blocker,
    },
    /// Found, ruled and decided rows settle a finding's standing — the value ledger's rows — and
    /// carry the build that wrote them; a row from before stamps reads as `unknown`.
    Found {
        landing: String,
        found: Box<Found>,
        #[serde(default)]
        build: Build,
    },
    Assigned {
        landing: String,
        fingerprint: String,
        staff: Staff,
    },
    Ruled {
        landing: String,
        verification: Verification,
        #[serde(default)]
        build: Build,
    },
    Proposed {
        landing: String,
        proposal: Proposal,
    },
    Decided {
        landing: String,
        fingerprint: String,
        decision: Decision,
        #[serde(default)]
        build: Build,
    },
    Waived {
        landing: String,
        waiver: Waiver,
    },
}

/// One landing's challenge as the records fold it.
#[derive(Debug, Clone)]
struct Open {
    landing: Landing,
    opening: Opening,
    filed: BTreeMap<Speciality, CoverageLedger>,
    staff: BTreeMap<Speciality, Staff>,
    blockers: Vec<Blocker>,
    findings: Vec<Finding>,
    waived: Option<Waiver>,
    /// Per fingerprint, the line, from 1, of the record that last settled it — found, ruled or
    /// decided — and the build that wrote it.
    at: BTreeMap<String, (u64, Build)>,
    /// Lines, from 1, that did not parse and could be this landing's.
    unreadable: Vec<u64>,
}

impl Open {
    fn new(landing: Landing, opening: Opening) -> Open {
        Open {
            landing,
            opening,
            filed: BTreeMap::new(),
            staff: BTreeMap::new(),
            blockers: Vec::new(),
            findings: Vec::new(),
            waived: None,
            at: BTreeMap::new(),
            unreadable: Vec::new(),
        }
    }

    fn status(&self) -> Status {
        if let Some(w) = &self.waived {
            return Status::Waived {
                reason: w.reason.clone(),
            };
        }
        let mut blockers = self.blockers.clone();
        blockers.extend(self.blocking());
        if !blockers.is_empty() {
            return Status::Blocked { findings: blockers };
        }
        let awaiting: Vec<Speciality> = self
            .landing
            .routed
            .iter()
            .map(|r| r.speciality)
            .filter(|s| !self.filed.contains_key(s))
            .collect();
        if !awaiting.is_empty() || !self.unreadable.is_empty() {
            return Status::Pending {
                awaiting,
                unreadable: self.unreadable.clone(),
            };
        }
        let verifying = self.verifying();
        if verifying.is_empty() {
            Status::Cleared
        } else {
            Status::Verifying {
                findings: verifying,
            }
        }
    }

    fn routed(&self, s: Speciality) -> bool {
        self.landing.routed.iter().any(|r| r.speciality == s)
    }
}

/// A challenge as the board, the CLI and the harness read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The landing as it was routed.
    pub landing: Landing,
    /// Its tree and its author.
    pub opening: Opening,
    /// How each routed speciality is staffed, in routing order; one never staffed yet is absent.
    pub staff: Vec<(Speciality, Staff)>,
    /// The findings of medium or higher its reviewers reported, in the order they were found,
    /// each with its verifier, its verdict and its seal proposal.
    pub findings: Vec<Finding>,
    /// Where it stands.
    pub status: Status,
}

/// The records, opened and locked whole until dropped, and the chain walked through them.
struct Held {
    file: File,
    all: BTreeMap<String, Open>,
    walk: Walk,
}

impl Held {
    /// Appends `row` as one line, in one write, chained to the records' head, and syncs it: a
    /// torn line left by a crash is closed first, so the row stands on a line of its own, and
    /// the row records it.
    fn append(&mut self, row: &Row) -> Result<(), Refusal> {
        let line = serde_json::to_vec(row).map_err(|e| Refusal::Unwritten(e.to_string()))?;
        if line.len() >= MAX_ROW {
            return Err(Refusal::TooLarge(line.len()));
        }
        let io = |e: std::io::Error| Refusal::Unwritten(e.to_string());
        let (bytes, walk) = self.walk.seal(&line).map_err(io)?;
        self.file.seek(SeekFrom::End(0)).map_err(io)?;
        self.file.write_all(&bytes).map_err(io)?;
        self.file.sync_data().map_err(io)?;
        self.walk = walk;
        Ok(())
    }
}

/// The challenges of one world.
#[derive(Debug, Clone)]
pub struct Challenges {
    path: PathBuf,
}

impl Challenges {
    /// The challenges of `world`, kept in the human's state directory; `None` with no home.
    #[must_use]
    pub fn of(world: &Path) -> Option<Challenges> {
        records_of(world).map(|f| Challenges::at(&f))
    }

    /// The challenges kept in `file`.
    #[must_use]
    pub fn at(file: &Path) -> Challenges {
        Challenges {
            path: file.to_path_buf(),
        }
    }

    /// The records' hash chain, walked ([`mnemorph_ledger::chain::verify`]): its head is the
    /// digest a proposal's explanation names, so the decision can be recomputed from these rows.
    ///
    /// # Errors
    ///
    /// The records cannot be read.
    pub fn chain(&self) -> std::io::Result<mnemorph_ledger::Report> {
        mnemorph_ledger::chain::verify(&self.path)
    }

    /// Opens the challenge of `landing`, with no tree and no author on record. See
    /// [`Challenges::open_at`].
    ///
    /// # Errors
    ///
    /// As [`Challenges::open_at`].
    pub fn open(&self, landing: &Landing) -> Result<Status, Refusal> {
        self.open_at(landing, &Opening::default())
    }

    /// Opens the challenge of `landing`, opened on `opening`'s tree, by its author. A landing
    /// routed to no speciality has nothing to clear, and is not recorded.
    ///
    /// # Errors
    ///
    /// The id is malformed or already open, or the records cannot be kept.
    pub fn open_at(&self, landing: &Landing, opening: &Opening) -> Result<Status, Refusal> {
        if !is_id(&landing.id) {
            return Err(Refusal::BadId);
        }
        if landing.routed.is_empty() {
            return Ok(Status::Cleared);
        }
        let mut held = self.hold()?;
        if held.all.contains_key(&landing.id) {
            return Err(Refusal::AlreadyOpen(landing.id.clone()));
        }
        held.append(&Row::Opened {
            landing: landing.clone(),
            opening: Some(opening.clone()),
        })?;
        Ok(Open::new(landing.clone(), opening.clone()).status())
    }

    /// Records how `speciality` stands for the review of `landing`; the latest row stands.
    ///
    /// # Errors
    ///
    /// No such open landing, a speciality it was not routed, or records that cannot be kept.
    pub fn staff(
        &self,
        landing: &str,
        speciality: Speciality,
        staff: Staff,
    ) -> Result<(), Refusal> {
        let mut held = self.hold()?;
        let open = held
            .all
            .get(landing)
            .ok_or_else(|| Refusal::NoSuchLanding(landing.to_owned()))?;
        if !open.routed(speciality) {
            return Err(Refusal::NoSuchLanding(format!("{landing} ({speciality})")));
        }
        held.append(&Row::Staffed {
            landing: landing.to_owned(),
            speciality: speciality.creature().to_owned(),
            staff,
        })
    }

    /// Waives the challenge of `landing` on the human's word: it is bound to the tree the
    /// challenge was opened on, and settles it whatever is pending or blocks.
    ///
    /// # Errors
    ///
    /// No such open landing, one waived already, or records that cannot be kept.
    pub fn waive(&self, landing: &str, waiver: &Waiver) -> Result<Status, Refusal> {
        let mut held = self.hold()?;
        let open = held
            .all
            .get_mut(landing)
            .ok_or_else(|| Refusal::NoSuchLanding(landing.to_owned()))?;
        if open.waived.is_some() {
            return Err(Refusal::AlreadyWaived(landing.to_owned()));
        }
        let mut waiver = waiver.clone();
        waiver.tree.clone_from(&open.opening.tree);
        if !waiver.holds(&open.opening.tree) {
            return Err(Refusal::Unreasoned);
        }
        open.waived = Some(waiver.clone());
        let status = open.status();
        held.append(&Row::Waived {
            landing: landing.to_owned(),
            waiver,
        })?;
        Ok(status)
    }

    /// The challenge of `landing` as the records hold it; `None` when none is open under that id.
    #[must_use]
    pub fn record(&self, landing: &str) -> Option<Record> {
        self.read().remove(landing).map(record_of)
    }

    /// Every challenge kept, by landing id.
    #[must_use]
    pub fn records(&self) -> BTreeMap<String, Record> {
        self.read()
            .into_iter()
            .map(|(id, open)| (id, record_of(open)))
            .collect()
    }

    /// Files `ledger` for its landing and gives the challenge's status after it. Its commands
    /// with no hash are stamped from `ran`, the filing body's recorded commands; its evidence is
    /// held to `tree`, the landing's tree, and to `ran`.
    ///
    /// # Errors
    ///
    /// No such open landing, a ledger already filed by this speciality, a ledger that breaks the
    /// state table or is too large, or records that cannot be kept.
    pub fn file(
        &self,
        mut ledger: CoverageLedger,
        tree: &dyn World,
        ran: &Ran,
    ) -> Result<Status, Refusal> {
        stamp(&mut ledger, ran);
        if !self.path.exists() {
            return Err(Refusal::NoSuchLanding(ledger.landing));
        }
        let mut held = self.hold()?;
        let Some(open) = held.all.get(&ledger.landing) else {
            return Err(Refusal::NoSuchLanding(ledger.landing.clone()));
        };
        if open.filed.contains_key(&ledger.speciality) {
            return Err(Refusal::AlreadyFiled(ledger.speciality));
        }
        if open.opening.holds_author(ledger.speciality.creature()) {
            return Err(Refusal::Author(ledger.speciality.creature().to_owned()));
        }
        let sources = Sources {
            tree: Some(tree),
            ran: Some(ran),
        };
        validate_in(&ledger, &open.landing, sources).map_err(Refusal::Invalid)?;
        held.append(&Row::Filed {
            ledger: ledger.clone(),
        })?;
        let open = held
            .all
            .get_mut(&ledger.landing)
            .ok_or_else(|| Refusal::NoSuchLanding(ledger.landing.clone()))?;
        open.filed.insert(ledger.speciality, ledger);
        Ok(open.status())
    }

    /// Where the challenge of `landing` stands; `None` when none is open under that id.
    #[must_use]
    pub fn status(&self, landing: &str) -> Option<Status> {
        self.read().get(landing).map(Open::status)
    }

    /// The landing as it was routed, and the ledgers filed for it, in routing order.
    #[must_use]
    pub fn ledgers(&self, landing: &str) -> Option<(Landing, Vec<CoverageLedger>)> {
        let open = self.read().remove(landing)?;
        let ledgers = open
            .landing
            .routed
            .iter()
            .filter_map(|r| open.filed.get(&r.speciality).cloned())
            .collect();
        Some((open.landing, ledgers))
    }

    /// Every landing whose challenge is pending, by id, with who it awaits.
    #[must_use]
    pub fn pending(&self) -> BTreeMap<String, Vec<Speciality>> {
        self.read()
            .into_iter()
            .filter_map(|(id, open)| match open.status() {
                Status::Pending { awaiting, .. } => Some((id, awaiting)),
                _ => None,
            })
            .collect()
    }

    /// The records' lines, from 1, that did not parse and hold a challenge pending.
    #[must_use]
    pub fn unreadable(&self) -> Vec<u64> {
        let mut lines: Vec<u64> = self
            .read()
            .into_values()
            .flat_map(|open| open.unreadable)
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// The records folded, read under the lock; empty when they cannot be read.
    fn read(&self) -> BTreeMap<String, Open> {
        if !self.path.exists() {
            return BTreeMap::new();
        }
        self.hold().map(|h| h.all).unwrap_or_default()
    }

    /// The records opened — the folder `0700`, the file `0600`, made if need be — locked, and
    /// folded.
    fn hold(&self) -> Result<Held, Refusal> {
        let io = |e: std::io::Error| Refusal::Unwritten(e.to_string());
        if let Some(dir) = self.path.parent() {
            private_dir(dir).map_err(io)?;
        }
        let mut o = std::fs::OpenOptions::new();
        o.create(true).read(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
        let mut file = o.open(&self.path).map_err(io)?;
        file.lock().map_err(io)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(io)?;
        let mut walk = Walk::new();
        walk.feed(&bytes);
        Ok(Held {
            file,
            all: fold(&bytes),
            walk,
        })
    }
}

/// A folded challenge, as it is read out.
fn record_of(open: Open) -> Record {
    let status = open.status();
    let staff = open
        .landing
        .routed
        .iter()
        .filter_map(|r| {
            open.staff
                .get(&r.speciality)
                .map(|s| (r.speciality, s.clone()))
        })
        .collect();
    Record {
        landing: open.landing,
        opening: open.opening,
        staff,
        findings: open.findings,
        status,
    }
}

/// The records folded: the first opening of an id stands; a filing counts only when its landing
/// is open, its speciality has not filed yet, and its ledger validates again; a staffing only for
/// a routed speciality, the latest standing; a waiver only the human's, reasoned and bound to the
/// opened tree; a decision on a finding only the human's, reasoned and bound to the opened tree,
/// the latest standing. A line that does not parse — a torn write, a hand edit, a row of a kind
/// this binary does not know — fails closed ([`unreadable`]).
#[allow(clippy::too_many_lines)] // one arm per kind of row
fn fold(bytes: &[u8]) -> BTreeMap<String, Open> {
    let mut all: BTreeMap<String, Open> = BTreeMap::new();
    for (n, line) in (0u64..).zip(bytes.split(|&b| b == b'\n')) {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(row) = serde_json::from_slice::<Row>(&mnemorph_ledger::chain::body(line)) else {
            unreadable(&mut all, n + 1, line);
            continue;
        };
        match row {
            Row::Opened { landing, opening } => {
                if is_id(&landing.id) && !all.contains_key(&landing.id) {
                    all.insert(
                        landing.id.clone(),
                        Open::new(landing, opening.unwrap_or_default()),
                    );
                }
            }
            Row::Staffed {
                landing,
                speciality,
                staff,
            } => {
                if let Some(open) = all.get_mut(&landing)
                    && let Some(s) = Speciality::of(&speciality)
                    && open.routed(s)
                {
                    open.staff.insert(s, staff);
                }
            }
            Row::Blocked { landing, blocker } => {
                if let Some(open) = all.get_mut(&landing)
                    && !open
                        .blockers
                        .iter()
                        .any(|b| b.fingerprint == blocker.fingerprint)
                {
                    open.blockers.push(blocker);
                }
            }
            Row::Found {
                landing,
                found,
                build,
            } => {
                if let Some(open) = all.get_mut(&landing) {
                    let fp = found.fingerprint().to_owned();
                    if open.fold_found(*found) {
                        open.at.insert(fp, (n + 1, build));
                    }
                }
            }
            Row::Assigned {
                landing,
                fingerprint,
                staff,
            } => {
                if let Some(open) = all.get_mut(&landing) {
                    open.fold_assigned(&fingerprint, staff);
                }
            }
            Row::Ruled {
                landing,
                verification,
                build,
            } => {
                if let Some(open) = all.get_mut(&landing) {
                    let fp = verification.fingerprint.clone();
                    if open.fold_ruled(verification) {
                        open.at.insert(fp, (n + 1, build));
                    }
                }
            }
            Row::Proposed { landing, proposal } => {
                if let Some(open) = all.get_mut(&landing) {
                    open.fold_proposed(proposal);
                }
            }
            Row::Decided {
                landing,
                fingerprint,
                decision,
                build,
            } => {
                if let Some(open) = all.get_mut(&landing)
                    && open.fold_decided(&fingerprint, decision)
                {
                    open.at.insert(fingerprint, (n + 1, build));
                }
            }
            Row::Waived { landing, waiver } => {
                if let Some(open) = all.get_mut(&landing)
                    && open.waived.is_none()
                    && waiver.holds(&open.opening.tree)
                {
                    open.waived = Some(waiver);
                }
            }
            Row::Filed { ledger } => {
                if let Some(open) = all.get_mut(&ledger.landing)
                    && !open.filed.contains_key(&ledger.speciality)
                    && !open.opening.holds_author(ledger.speciality.creature())
                    && validate(&ledger, &open.landing).is_ok()
                {
                    open.filed.insert(ledger.speciality, ledger);
                }
            }
        }
    }
    all
}

/// A line that did not parse, line `n` from 1: never skipped. The landing it names, when it names
/// one already open, stays pending on it; one that names none could be any landing opened before
/// it, and holds each of them pending. A landing opened after it is not its.
fn unreadable(all: &mut BTreeMap<String, Open>, n: u64, line: &[u8]) {
    let named = serde_json::from_slice::<serde_json::Value>(line)
        .ok()
        .and_then(|v| {
            let id = v
                .get("landing")
                .and_then(|l| l.as_str().or_else(|| l.get("id")?.as_str()))
                .or_else(|| v.get("ledger")?.get("landing")?.as_str());
            id.map(str::to_owned)
        });
    match named {
        Some(id) => {
            if let Some(open) = all.get_mut(&id) {
                open.unreadable.push(n);
            }
        }
        None => {
            for open in all.values_mut() {
                open.unreadable.push(n);
            }
        }
    }
}

/// Creates `dir`, its owner's alone where the platform says so.
fn private_dir(dir: &Path) -> std::io::Result<()> {
    if dir.as_os_str().is_empty() || dir.is_dir() {
        return Ok(());
    }
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    b.create(dir)
}

/// 1–128 of `A-Z a-z 0-9 . _ : -`.
fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemorph_proving::coverage::{CommandRun, Entry, Evidence, FileRead, Rule, State};

    const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "mnemorph-challenge-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Every written file reads `abc`.
    fn tree(landing: &Landing) -> BTreeMap<String, Vec<u8>> {
        landing
            .written
            .iter()
            .map(|p| (p.clone(), b"abc".to_vec()))
            .collect()
    }

    fn ran() -> Ran {
        let mut r = Ran::default();
        r.record("cargo test", "abc");
        r
    }

    /// A ledger that holds: each high-risk class checked on the written file, the rest ruled out
    /// each for its own reason.
    fn full(landing: &Landing, s: Speciality) -> CoverageLedger {
        // The first written path on one of its surfaces.
        let surfaces = landing
            .routed(s)
            .map(|r| r.surfaces.clone())
            .unwrap_or_default();
        let file = landing
            .written
            .iter()
            .find(|p| surfaces.iter().any(|x| x.holds(p)))
            .cloned()
            .unwrap_or_default();
        CoverageLedger {
            landing: landing.id.clone(),
            speciality: s,
            entries: landing
                .required(s)
                .into_iter()
                .enumerate()
                .map(|(n, (surface, class))| Entry {
                    surface,
                    class: class.name().to_owned(),
                    state: if mnemorph_proving::coverage::HIGH_RISK.contains(&class) {
                        State::Checked {
                            method: format!("read {class} paths through the file and ran tests"),
                            evidence: Evidence {
                                files: vec![FileRead {
                                    path: file.clone(),
                                    symbols: vec!["main".to_owned()],
                                    sha256: HASH.to_owned(),
                                }],
                                commands: vec![CommandRun {
                                    run: "cargo test".to_owned(),
                                    sha256: None,
                                }],
                            },
                        }
                    } else {
                        State::NotApplicable {
                            reason: format!("fact {n}: the landing adds no reader of this kind"),
                            evidence: Evidence::default(),
                        }
                    },
                })
                .collect(),
        }
    }

    fn landing(written: &[&str]) -> Landing {
        let written: Vec<String> = written.iter().map(|s| (*s).to_owned()).collect();
        route(&Roster::default(), "run-7:1", &written)
    }

    #[test]
    fn routing_keeps_each_speciality_s_touched_surfaces() {
        let l = landing(&["./crates/mnemorph-wire/src/report.rs", "Cargo.lock"]);
        assert_eq!(
            l.written,
            ["crates/mnemorph-wire/src/report.rs", "Cargo.lock"]
        );
        let routed: Vec<(Speciality, Vec<&str>)> = l
            .routed
            .iter()
            .map(|r| {
                (
                    r.speciality,
                    r.surfaces.iter().map(|s| s.at.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            routed,
            [
                (Speciality::Agent, vec!["crates/mnemorph-wire/**"]),
                (Speciality::Supply, vec!["**/Cargo.lock"]),
                (Speciality::General, vec![]),
            ]
        );
        assert_eq!(l.required(Speciality::General), []);
        assert_eq!(l.required(Speciality::Supply).len(), 5);
    }

    #[test]
    fn a_world_s_declared_surface_is_routed_and_held() {
        let w = temp("declared");
        let doc = w.join(".mnemorph/dark-elf/dark-elf-web");
        std::fs::create_dir_all(&doc).unwrap();
        std::fs::write(
            doc.join("README.md"),
            "# dark-elf-web\n\n- **Race:** dark-elf\n- **Reports to:** rimuru\n- **Surfaces:** src/auth/\n",
        )
        .unwrap();
        let roster = Roster::load(&w);
        assert!(roster.problems.is_empty(), "{:?}", roster.problems);
        let l = route(&roster, "l1", &["src/auth/login.rs".to_owned()]);
        let web = l
            .routed(Speciality::Web)
            .expect("routed by the declared surface");
        assert_eq!(
            web.surfaces,
            [Surface {
                at: "src/auth/".to_owned(),
                declared: true
            }]
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    /// Two routed hunters (and the generalist): one filed leaves the challenge pending, all
    /// filed clears it.
    #[test]
    fn the_challenge_clears_only_when_every_routed_speciality_has_filed() {
        let w = temp("clear");
        let c = Challenges::at(&w.join("records/challenges.jsonl"));
        let l = landing(&["crates/mnemorph-wire/src/report.rs", "Cargo.lock"]);
        let files = tree(&l);
        assert_eq!(
            c.open(&l),
            Ok(Status::Pending {
                awaiting: vec![Speciality::Agent, Speciality::Supply, Speciality::General],
                unreadable: vec![],
            })
        );
        assert_eq!(
            c.file(full(&l, Speciality::Agent), &files, &ran()),
            Ok(Status::Pending {
                awaiting: vec![Speciality::Supply, Speciality::General],
                unreadable: vec![],
            })
        );
        // An invalid ledger is refused, and changes nothing.
        let mut half = full(&l, Speciality::Supply);
        half.entries.pop();
        let Err(Refusal::Invalid(r)) = c.file(half, &files, &ran()) else {
            panic!("a missing pair is refused");
        };
        assert_eq!(r.rules(), [Rule::MissingPair]);
        assert!(matches!(
            c.status(&l.id),
            Some(Status::Pending { ref awaiting, .. }) if awaiting.len() == 2
        ));
        assert_eq!(c.pending().len(), 1);
        assert_eq!(
            c.file(full(&l, Speciality::Supply), &files, &ran()),
            Ok(Status::Pending {
                awaiting: vec![Speciality::General],
                unreadable: vec![],
            })
        );
        // The generalist owns no class: its ledger is empty, and filing it says its review ran.
        assert_eq!(
            c.file(full(&l, Speciality::General), &files, &ran()),
            Ok(Status::Cleared)
        );
        assert_eq!(c.status(&l.id), Some(Status::Cleared));
        assert!(c.pending().is_empty());
        let (back, ledgers) = c.ledgers(&l.id).unwrap();
        assert_eq!(back, l);
        assert_eq!(ledgers.len(), 3);
        // The stored ledger carries the hash the harness stamped from the journal.
        assert!(
            std::fs::read_to_string(c.path.clone())
                .unwrap()
                .contains(HASH)
        );
        assert_eq!(
            c.file(full(&l, Speciality::Agent), &files, &ran()),
            Err(Refusal::AlreadyFiled(Speciality::Agent))
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    /// A line the fold cannot read never clears a challenge: one that names no landing holds
    /// every landing opened before it pending, one that names a landing holds that one, and a
    /// landing opened after it is not its. A row with a field this binary does not know reads.
    #[test]
    fn an_unreadable_row_holds_its_challenge_pending() {
        use std::io::Write as _;
        let w = temp("unreadable");
        let c = Challenges::at(&w.join("records/challenges.jsonl"));
        let l = landing(&["crates/mnemorph-wire/src/report.rs", "Cargo.lock"]);
        let files = tree(&l);
        c.open(&l).unwrap();
        for s in [Speciality::Agent, Speciality::Supply, Speciality::General] {
            c.file(full(&l, s), &files, &ran()).unwrap();
        }
        assert_eq!(c.status(&l.id), Some(Status::Cleared));
        let append = |line: &str| {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&c.path)
                .unwrap();
            writeln!(f, "{line}").unwrap();
        };
        let lines = |c: &Challenges| {
            u64::try_from(std::fs::read_to_string(&c.path).unwrap().lines().count()).unwrap()
        };
        // A newer binary's row, with a field this one does not know: read, not dropped.
        append(&format!(
            r#"{{"row":"staffed","landing":"{}","speciality":"{}","staff":{{"staff":"unstaffed"}},"future":1}}"#,
            l.id,
            Speciality::Agent.creature()
        ));
        let r = c.record(&l.id).unwrap();
        assert_eq!(r.staff[0], (Speciality::Agent, Staff::Unstaffed));
        assert_eq!(r.status, Status::Cleared);
        // A row naming another landing, unreadable: this one stays cleared.
        append(r#"{"row":"martian","landing":"run-9:9"}"#);
        assert_eq!(c.status(&l.id), Some(Status::Cleared));
        // A row naming this landing, of a kind this binary does not know: pending on it.
        append(&format!(r#"{{"row":"martian","landing":"{}"}}"#, l.id));
        let named = lines(&c);
        // A torn line names nothing: it could be any landing opened before it.
        append(r#"{"row":"found","landing":"#);
        let torn = lines(&c);
        assert_eq!(
            c.status(&l.id),
            Some(Status::Pending {
                awaiting: vec![],
                unreadable: vec![named, torn],
            })
        );
        assert_eq!(c.unreadable(), [named, torn]);
        // A landing opened after the torn line is not held by it.
        let later = Landing {
            id: "l2".into(),
            ..l.clone()
        };
        c.open(&later).unwrap();
        assert!(matches!(
            c.status("l2"),
            Some(Status::Pending { ref unreadable, .. }) if unreadable.is_empty()
        ));
        let _ = std::fs::remove_dir_all(&w);
    }

    /// The hollow pass: every pair ruled out by one copied reason leaves the challenge pending.
    #[test]
    fn a_copied_ruling_does_not_clear() {
        let w = temp("hollow");
        let c = Challenges::at(&w.join("challenges.jsonl"));
        let l = landing(&["crates/mnemorph-wire/src/report.rs"]);
        c.open(&l).unwrap();
        let mut hollow = full(&l, Speciality::Agent);
        for e in &mut hollow.entries {
            e.state = State::NotApplicable {
                reason: "the landing only renames a test helper; no input reaches it".to_owned(),
                evidence: Evidence::default(),
            };
        }
        let Err(Refusal::Invalid(r)) = c.file(hollow, &tree(&l), &ran()) else {
            panic!("a hollow ledger is refused");
        };
        assert_eq!(
            r.rules(),
            [
                Rule::UniformReason,
                Rule::UnprovenRuling,
                Rule::UncheckedSurface
            ]
        );
        assert!(matches!(c.status(&l.id), Some(Status::Pending { .. })));
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn a_command_the_body_never_ran_is_refused() {
        let w = temp("unrun");
        let c = Challenges::at(&w.join("challenges.jsonl"));
        let l = landing(&["crates/mnemorph-wire/src/report.rs"]);
        c.open(&l).unwrap();
        let Err(Refusal::Invalid(r)) =
            c.file(full(&l, Speciality::Agent), &tree(&l), &Ran::default())
        else {
            panic!("an unrun command is refused");
        };
        assert_eq!(r.rules(), [Rule::UnrunCommand]);
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn openings_and_filings_are_refused_where_they_do_not_fit() {
        let w = temp("refusals");
        let c = Challenges::at(&w.join("challenges.jsonl"));
        let l = landing(&["Cargo.lock"]);
        let files = tree(&l);
        let mut bad = l.clone();
        bad.id = "a b".to_owned();
        assert_eq!(c.open(&bad), Err(Refusal::BadId));
        bad.id = "x".repeat(129);
        assert_eq!(c.open(&bad), Err(Refusal::BadId));
        let nothing = landing(&["README.md"]);
        assert_eq!(c.open(&nothing), Ok(Status::Cleared));
        assert_eq!(c.status(&nothing.id), None, "nothing routed, nothing kept");
        assert!(c.open(&l).is_ok());
        assert_eq!(c.open(&l), Err(Refusal::AlreadyOpen(l.id.clone())));
        let mut elsewhere = full(&l, Speciality::Supply);
        elsewhere.landing = "other".to_owned();
        assert_eq!(
            c.file(elsewhere, &files, &ran()),
            Err(Refusal::NoSuchLanding("other".to_owned()))
        );
        let Err(Refusal::Invalid(r)) = c.file(full(&l, Speciality::Web), &files, &ran()) else {
            panic!("an unrouted speciality is refused");
        };
        assert_eq!(r.rules(), [Rule::Unrouted]);
        // A landing too large for one row is refused, never split.
        let mut huge = l.clone();
        huge.id = "huge".to_owned();
        huge.written = vec!["x".repeat(1000); MAX_ROW / 1000 + 1];
        assert!(matches!(c.open(&huge), Err(Refusal::TooLarge(_))));
        for refusal in [
            Refusal::NoSuchLanding("x".into()),
            Refusal::AlreadyOpen("x".into()),
            Refusal::BadId,
            Refusal::AlreadyFiled(Speciality::Web),
            Refusal::Invalid(r),
            Refusal::TooLarge(1),
            Refusal::Unwritten("disk".into()),
        ] {
            assert_ne!(refusal.to_string(), "");
        }
        let _ = std::fs::remove_dir_all(&w);
    }

    /// A filing line written by anything but the harness counts only if it validates on read.
    #[test]
    fn the_fold_validates_every_filing_again() {
        let w = temp("fold");
        let file = w.join("challenges.jsonl");
        let c = Challenges::at(&file);
        let l = landing(&["Cargo.lock"]);
        c.open(&l).unwrap();
        let mut forged = full(&l, Speciality::Supply);
        forged.entries.truncate(1);
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap();
        let line = serde_json::to_string(&Row::Filed { ledger: forged }).unwrap();
        // A torn last line, as a crash leaves it.
        write!(f, "{line}\nnot json").unwrap();
        drop(f);
        assert!(matches!(c.status(&l.id), Some(Status::Pending { .. })));
        assert_eq!(c.ledgers(&l.id).unwrap().1, []);
        // The next row closes the torn line and stands on its own.
        let other = route(&Roster::default(), "run-7:2", &["Cargo.lock".to_owned()]);
        c.open(&other).unwrap();
        assert!(c.status(&other.id).is_some());
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn records_live_outside_the_world_and_are_the_owner_s_alone() {
        let w = temp("where");
        let r = records_of(&w).unwrap();
        assert!(!r.starts_with(&w), "{}", r.display());
        assert!(r.to_string_lossy().contains("mnemorph/challenges/"));
        assert!(Challenges::of(&w).is_some());
        let file = w.join("private/challenges.jsonl");
        Challenges::at(&file)
            .open(&landing(&["Cargo.lock"]))
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&file), 0o600);
            assert_eq!(mode(&w.join("private")), 0o700);
        }
        let _ = std::fs::remove_dir_all(&w);
    }

    fn opening() -> Opening {
        Opening {
            tree: HASH.to_owned(),
            by: "orc-o".to_owned(),
            ..Default::default()
        }
    }

    /// The opening's tree and author stand; staffing folds to the latest row, only for a routed
    /// speciality; the records read back whole.
    #[test]
    fn a_challenge_keeps_its_tree_author_and_staffing() {
        let w = temp("staff");
        let c = Challenges::at(&w.join("challenges.jsonl"));
        let l = landing(&["crates/mnemorph-tools/src/files.rs", "Cargo.lock"]);
        c.open_at(&l, &opening()).unwrap();
        c.staff(&l.id, Speciality::Harness, Staff::Unstaffed)
            .unwrap();
        c.staff(
            &l.id,
            Speciality::Supply,
            Staff::Queued {
                why: "past the cap".into(),
            },
        )
        .unwrap();
        c.staff(
            &l.id,
            Speciality::Supply,
            Staff::Commissioned {
                body: "dark-elf-supply".into(),
                by: "rimuru".into(),
                wt: 7,
                waited: 0,
            },
        )
        .unwrap();
        assert!(matches!(
            c.staff(&l.id, Speciality::Web, Staff::Unstaffed),
            Err(Refusal::NoSuchLanding(_))
        ));
        assert!(matches!(
            c.staff("nope", Speciality::Web, Staff::Unstaffed),
            Err(Refusal::NoSuchLanding(_))
        ));
        let r = c.record(&l.id).unwrap();
        assert_eq!(r.opening, opening());
        assert_eq!(
            r.staff,
            [
                (Speciality::Harness, Staff::Unstaffed),
                (
                    Speciality::Supply,
                    Staff::Commissioned {
                        body: "dark-elf-supply".into(),
                        by: "rimuru".into(),
                        wt: 7,
                        waited: 0,
                    }
                ),
            ]
        );
        assert!(r.status.open());
        assert_eq!(c.records().len(), 1);
        assert!(c.record("nope").is_none());
        let _ = std::fs::remove_dir_all(&w);
    }

    /// A blocker recorded before findings were verified still blocks, even once every ledger is
    /// filed — fail closed; one fingerprint blocks once.
    #[test]
    fn a_legacy_blocker_still_holds_the_landing() {
        let w = temp("block");
        let file = w.join("challenges.jsonl");
        let c = Challenges::at(&file);
        let l = landing(&["Cargo.lock"]);
        c.open_at(&l, &opening()).unwrap();
        let blocker = Blocker {
            speciality: "dark-elf-supply".into(),
            fingerprint: "0123456789abcdef".into(),
            severity: "high".into(),
            claim: "a yanked crate".into(),
            standing: String::new(),
        };
        assert!(blocker.to_string().contains("#0123456789abcdef high"));
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap();
        let row = Row::Blocked {
            landing: l.id.clone(),
            blocker: blocker.clone(),
        };
        for _ in 0..2 {
            writeln!(f, "{}", serde_json::to_string(&row).unwrap()).unwrap();
        }
        drop(f);
        let held = Status::Blocked {
            findings: vec![blocker.clone()],
        };
        assert_eq!(
            c.file(full(&l, Speciality::Supply), &tree(&l), &ran()),
            Ok(held.clone())
        );
        assert_eq!(c.status(&l.id), Some(held));
        assert!(c.pending().is_empty(), "blocked is not awaiting a ledger");
        let _ = std::fs::remove_dir_all(&w);
    }

    /// The human's waiver settles a pending or blocked challenge, with its reason kept; a waiver
    /// with no reason, or a row no human wrote, is refused or counts for nothing.
    #[test]
    fn a_waiver_is_the_human_s_reasoned_and_bound_to_the_tree() {
        let w = temp("waive");
        let file = w.join("challenges.jsonl");
        let c = Challenges::at(&file);
        let l = landing(&["Cargo.lock"]);
        c.open_at(&l, &opening()).unwrap();
        assert_eq!(Waiver::human("ok").unwrap_err(), Refusal::Unreasoned);
        assert_eq!(Waiver::human("n/a").unwrap_err(), Refusal::Unreasoned);
        assert_eq!(
            Waiver::human("the lock moved only by a\u{202e} patch").unwrap_err(),
            Refusal::Unreasoned
        );
        // Forged rows, as anything but the waiver door would write them: a body as the author,
        // a stock reason, a tree the challenge was not opened on.
        let reason = "the lockfile change is a revert of yesterday's bump";
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap();
        for (by, why, tree) in [
            ("orc-o", reason, HASH),
            (HUMAN, "not relevant", HASH),
            (HUMAN, reason, "f00d"),
        ] {
            let row = Row::Waived {
                landing: l.id.clone(),
                waiver: Waiver {
                    by: by.to_owned(),
                    reason: why.to_owned(),
                    tree: tree.to_owned(),
                },
            };
            writeln!(f, "{}", serde_json::to_string(&row).unwrap()).unwrap();
        }
        drop(f);
        assert!(
            matches!(c.status(&l.id), Some(Status::Pending { .. })),
            "a forged waiver counts for nothing"
        );
        let waiver = Waiver::human(reason).unwrap();
        assert_eq!(waiver.reason(), reason);
        assert_eq!(
            c.waive(&l.id, &waiver),
            Ok(Status::Waived {
                reason: reason.to_owned()
            })
        );
        assert_eq!(
            c.status(&l.id),
            Some(Status::Waived {
                reason: reason.to_owned()
            })
        );
        assert!(!c.status(&l.id).unwrap().open());
        assert_eq!(
            c.waive(&l.id, &waiver),
            Err(Refusal::AlreadyWaived(l.id.clone()))
        );
        assert!(matches!(
            c.waive("nope", &waiver),
            Err(Refusal::NoSuchLanding(_))
        ));
        for r in [
            Refusal::NotHuman("orc-o".into()),
            Refusal::Unreasoned,
            Refusal::AlreadyWaived("x".into()),
        ] {
            assert_ne!(r.to_string(), "");
        }
        let _ = std::fs::remove_dir_all(&w);
    }

    /// The landing's author, or a body above it, never clears its own challenge: its filing is
    /// refused, and one written into the records counts for nothing.
    #[test]
    fn the_author_never_files_for_its_own_landing() {
        let w = temp("author");
        let file = w.join("challenges.jsonl");
        let c = Challenges::at(&file);
        let l = landing(&["Cargo.lock"]);
        c.open_at(
            &l,
            &Opening {
                tree: HASH.to_owned(),
                by: "dark-elf-supply".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            c.file(full(&l, Speciality::Supply), &tree(&l), &ran()),
            Err(Refusal::Author("dark-elf-supply".to_owned()))
        );
        let mut above = l.clone();
        above.id = "run-7:9".to_owned();
        c.open_at(
            &above,
            &Opening {
                tree: HASH.to_owned(),
                by: "slime-s".to_owned(),
                chain: vec!["orc-o".to_owned(), "dark-elf-supply".to_owned()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            c.file(full(&above, Speciality::Supply), &tree(&above), &ran()),
            Err(Refusal::Author(_))
        ));
        // Written by hand, the author's filing is dropped on read.
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap();
        let mut stamped = full(&l, Speciality::Supply);
        stamp(&mut stamped, &ran());
        let row = Row::Filed { ledger: stamped };
        writeln!(f, "{}", serde_json::to_string(&row).unwrap()).unwrap();
        drop(f);
        assert!(matches!(c.status(&l.id), Some(Status::Pending { .. })));
        assert_ne!(Refusal::Author("x".into()).to_string(), "");
        let _ = std::fs::remove_dir_all(&w);
    }

    /// Filings racing from many threads all land, each on a line of its own.
    #[test]
    fn concurrent_filings_are_serialised_by_the_lock() {
        let w = temp("race");
        let file = w.join("challenges.jsonl");
        let ids: Vec<String> = (0..16).map(|n| format!("l-{n}")).collect();
        std::thread::scope(|s| {
            for id in &ids {
                let file = file.clone();
                s.spawn(move || {
                    let c = Challenges::at(&file);
                    let l = route(&Roster::default(), id, &["Cargo.lock".to_owned()]);
                    c.open(&l).unwrap();
                    c.file(full(&l, Speciality::Supply), &tree(&l), &ran())
                        .unwrap();
                });
            }
        });
        let c = Challenges::at(&file);
        for id in &ids {
            assert_eq!(c.status(id), Some(Status::Cleared), "{id}");
        }
        // Every filing chained, one after another, under the lock.
        let chain = mnemorph_ledger::chain::verify(&file).unwrap();
        assert!(chain.holds() && chain.truncated.is_none(), "{chain}");
        assert_eq!(
            chain.rows,
            std::fs::read_to_string(&file).unwrap().lines().count()
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    /// Records written before the chain read as before; the first chained row is the genesis,
    /// and an edit of an earlier row is found.
    #[test]
    fn old_records_read_as_before_and_the_chain_starts_after_them() {
        let w = temp("genesis");
        let file = w.join("challenges.jsonl");
        let l = landing(&["Cargo.lock"]);
        let opened = Row::Opened {
            landing: l.clone(),
            opening: Some(opening()),
        };
        std::fs::write(&file, serde_json::to_string(&opened).unwrap() + "\n").unwrap();
        let c = Challenges::at(&file);
        assert!(c.status(&l.id).is_some_and(|s| s.open()));
        assert_eq!(
            c.file(full(&l, Speciality::Supply), &tree(&l), &ran()),
            Ok(Status::Cleared)
        );
        let chain = mnemorph_ledger::chain::verify(&file).unwrap();
        assert!(chain.holds(), "{chain}");
        assert_eq!((chain.legacy, chain.rows), (1, 1));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.lines().nth(1).unwrap().contains("\"genesis\":true"));
        std::fs::write(&file, text.replacen("Cargo.lock", "Cargo.lockx", 1)).unwrap();
        let chain = mnemorph_ledger::chain::verify(&file).unwrap();
        assert_eq!(
            chain.broken,
            Some((2, mnemorph_ledger::chain::Break::Unlinked))
        );
        let _ = std::fs::remove_dir_all(&w);
    }
}
