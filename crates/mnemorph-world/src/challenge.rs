//! A landing's Dark Elf challenge, as the harness keeps it: the landing as it was routed, its tree
//! and its author, how each routed speciality is staffed, each routed speciality's coverage
//! ledger, the findings that block it, and the human's waiver. The challenge is pending until every
//! routed speciality has filed a ledger that holds, and every finding of medium or higher has its
//! verifier's verdict ([`verification`]); a finding confirmed at high or critical, or a critical
//! one not refuted nor confirmed lower, blocks it whatever was filed; a human waiver, with its
//! reason, settles it either way.
//!
//! The records are the harness's, outside the world and out of every command's reach:
//! `$XDG_STATE_HOME/mnemorph/challenges/<world>-<fnv64a>.jsonl`, one row per line, the folder
//! `0700` and the file `0600`. Every read and every write holds the file's lock, so a filing is
//! checked against the records and written whole before another is read. A ledger is kept only
//! once it validates, and every filing is validated again when the records are read, so a line
//! written by anything else counts for nothing.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use mnemorph_proving::World;
use mnemorph_proving::coverage::{
    CoverageLedger, Landing, Ran, Refused, Routed, Sources, Surface, stamp, validate, validate_in,
};
use mnemorph_races::Speciality;
use serde::{Deserialize, Serialize};

use crate::roster::Roster;

pub mod verification;

pub use verification::{
    FLOOR, Filing, Finding, Found, Guard, Outcome, Pinned, Proposal, Rerun, Ruling, Verification,
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
}

impl Opening {
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

/// A finding that blocks the landing until it is fixed or the human waives the challenge: one
/// confirmed at high or critical, or a critical one no verdict refuted or lowered.
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
        && !reason
            .chars()
            .any(|c| c.is_control() || mnemorph_wire::invisible(c))
}

/// Where a landing's challenge stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Some routed specialities have not filed a valid ledger yet.
    Pending {
        /// They, in routing order.
        awaiting: Vec<Speciality>,
    },
    /// A finding confirmed at high or critical, or a critical one unverified, blocks the
    /// landing, whatever was filed.
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
        }
    }
}

/// One line of the records.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "row", rename_all = "kebab-case", deny_unknown_fields)]
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
    Found {
        landing: String,
        found: Box<Found>,
    },
    Assigned {
        landing: String,
        fingerprint: String,
        staff: Staff,
    },
    Ruled {
        landing: String,
        verification: Verification,
    },
    Proposed {
        landing: String,
        proposal: Proposal,
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
        if !awaiting.is_empty() {
            return Status::Pending { awaiting };
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

/// The records, opened and locked whole until dropped.
struct Held {
    file: File,
    all: BTreeMap<String, Open>,
}

impl Held {
    /// Appends `row` as one line, in one write, and syncs it: a torn line left by a crash is
    /// closed first, so the row stands on a line of its own.
    fn append(&mut self, row: &Row) -> Result<(), Refusal> {
        let mut line = serde_json::to_vec(row).map_err(|e| Refusal::Unwritten(e.to_string()))?;
        if line.len() >= MAX_ROW {
            return Err(Refusal::TooLarge(line.len()));
        }
        line.push(b'\n');
        let io = |e: std::io::Error| Refusal::Unwritten(e.to_string());
        let len = self.file.metadata().map_err(io)?.len();
        if len > 0 {
            let mut last = [0u8; 1];
            self.file.seek(SeekFrom::End(-1)).map_err(io)?;
            self.file.read_exact(&mut last).map_err(io)?;
            if last[0] != b'\n' {
                line.insert(0, b'\n');
            }
        }
        self.file.seek(SeekFrom::End(0)).map_err(io)?;
        self.file.write_all(&line).map_err(io)?;
        self.file.sync_data().map_err(io)
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
                Status::Pending { awaiting } => Some((id, awaiting)),
                _ => None,
            })
            .collect()
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
        Ok(Held {
            file,
            all: fold(&bytes),
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
/// opened tree. A line that does not parse — a torn write, a hand edit — counts for nothing.
fn fold(bytes: &[u8]) -> BTreeMap<String, Open> {
    let mut all: BTreeMap<String, Open> = BTreeMap::new();
    for line in bytes.split(|&b| b == b'\n') {
        let Ok(row) = serde_json::from_slice::<Row>(line) else {
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
            Row::Found { landing, found } => {
                if let Some(open) = all.get_mut(&landing) {
                    open.fold_found(*found);
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
            } => {
                if let Some(open) = all.get_mut(&landing) {
                    open.fold_ruled(verification);
                }
            }
            Row::Proposed { landing, proposal } => {
                if let Some(open) = all.get_mut(&landing) {
                    open.fold_proposed(proposal);
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
                awaiting: vec![Speciality::Agent, Speciality::Supply, Speciality::General]
            })
        );
        assert_eq!(
            c.file(full(&l, Speciality::Agent), &files, &ran()),
            Ok(Status::Pending {
                awaiting: vec![Speciality::Supply, Speciality::General]
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
            Some(Status::Pending { ref awaiting }) if awaiting.len() == 2
        ));
        assert_eq!(c.pending().len(), 1);
        assert_eq!(
            c.file(full(&l, Speciality::Supply), &files, &ran()),
            Ok(Status::Pending {
                awaiting: vec![Speciality::General]
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
        let _ = std::fs::remove_dir_all(&w);
    }
}
