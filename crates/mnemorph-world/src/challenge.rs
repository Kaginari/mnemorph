//! A landing's Dark Elf challenge, as the harness keeps it: the landing as it was routed, and each
//! routed speciality's coverage ledger. The challenge is pending until every routed speciality has
//! filed a ledger that holds; then it clears.
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

/// Where a landing's challenge stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Some routed specialities have not filed a valid ledger yet.
    Pending {
        /// They, in routing order.
        awaiting: Vec<Speciality>,
    },
    /// Every routed speciality has.
    Cleared,
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
            Refusal::TooLarge(n) => write!(f, "the filing is {n} bytes, over {MAX_ROW}"),
            Refusal::Unwritten(why) => write!(f, "the challenge records cannot be kept: {why}"),
        }
    }
}

/// One line of the records.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "row", rename_all = "kebab-case", deny_unknown_fields)]
enum Row {
    Opened { landing: Landing },
    Filed { ledger: CoverageLedger },
}

/// One landing's challenge as the records fold it.
#[derive(Debug, Clone)]
struct Open {
    landing: Landing,
    filed: BTreeMap<Speciality, CoverageLedger>,
}

impl Open {
    fn status(&self) -> Status {
        let awaiting: Vec<Speciality> = self
            .landing
            .routed
            .iter()
            .map(|r| r.speciality)
            .filter(|s| !self.filed.contains_key(s))
            .collect();
        if awaiting.is_empty() {
            Status::Cleared
        } else {
            Status::Pending { awaiting }
        }
    }
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

    /// Opens the challenge of `landing`. A landing routed to no speciality has nothing to clear,
    /// and is not recorded.
    ///
    /// # Errors
    ///
    /// The id is malformed or already open, or the records cannot be kept.
    pub fn open(&self, landing: &Landing) -> Result<Status, Refusal> {
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
        })?;
        Ok(Open {
            landing: landing.clone(),
            filed: BTreeMap::new(),
        }
        .status())
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
                Status::Cleared => None,
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

/// The records folded: the first opening of an id stands; a filing counts only when its landing
/// is open, its speciality has not filed yet, and its ledger validates again. A line that does
/// not parse — a torn write, a hand edit — counts for nothing.
fn fold(bytes: &[u8]) -> BTreeMap<String, Open> {
    let mut all: BTreeMap<String, Open> = BTreeMap::new();
    for line in bytes.split(|&b| b == b'\n') {
        let Ok(row) = serde_json::from_slice::<Row>(line) else {
            continue;
        };
        match row {
            Row::Opened { landing } => {
                if is_id(&landing.id) && !all.contains_key(&landing.id) {
                    all.insert(
                        landing.id.clone(),
                        Open {
                            landing,
                            filed: BTreeMap::new(),
                        },
                    );
                }
            }
            Row::Filed { ledger } => {
                if let Some(open) = all.get_mut(&ledger.landing)
                    && !open.filed.contains_key(&ledger.speciality)
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
