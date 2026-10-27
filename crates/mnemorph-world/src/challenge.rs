//! A landing's Dark Elf challenge, as the harness keeps it: the landing as it was routed, and each
//! routed speciality's coverage ledger. The challenge is pending until every routed speciality has
//! filed a ledger that holds; then it clears.
//!
//! The records are the harness's, outside the world and out of every command's reach:
//! `$XDG_STATE_HOME/mnemorph/challenges/<world>-<fnv64a>.jsonl`, append-only. A ledger is kept only
//! once it validates, and every filing is validated again when the records are read, so a line
//! written by anything else counts for nothing.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use mnemorph_ledger::Journal;
use mnemorph_races::Speciality;
use serde::{Deserialize, Serialize};

use crate::coverage::{
    CoverageLedger, Landing, Refused, Routed, Surface, Tree, validate, validate_in,
};
use crate::roster::Roster;

/// The largest file whose hash the harness reads back for a ledger's evidence.
const MAX_HASHED: u64 = 32 * 1024 * 1024;

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
    /// The records could not be written.
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
            Refusal::Unwritten(why) => write!(f, "the challenge records cannot be written: {why}"),
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

/// The challenges of one world.
#[derive(Debug, Clone)]
pub struct Challenges {
    journal: Journal<Row>,
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
            journal: Journal::new(file.to_path_buf()),
        }
    }

    /// Opens the challenge of `landing`. A landing routed to no speciality has nothing to clear,
    /// and is not recorded.
    ///
    /// # Errors
    ///
    /// The id is malformed or already open, or the records cannot be written.
    pub fn open(&self, landing: &Landing) -> Result<Status, Refusal> {
        if !is_id(&landing.id) {
            return Err(Refusal::BadId);
        }
        if landing.routed.is_empty() {
            return Ok(Status::Cleared);
        }
        if self.fold().contains_key(&landing.id) {
            return Err(Refusal::AlreadyOpen(landing.id.clone()));
        }
        self.append(&Row::Opened {
            landing: landing.clone(),
        })?;
        Ok(Open {
            landing: landing.clone(),
            filed: BTreeMap::new(),
        }
        .status())
    }

    /// Files `ledger` for its landing, its evidence held to the world's files through `tree`
    /// (see [`crate::coverage::check`]), and gives the challenge's status after it.
    ///
    /// # Errors
    ///
    /// No such open landing, a ledger already filed by this speciality, a ledger that breaks the
    /// state table, or records that cannot be written.
    pub fn file(&self, ledger: &CoverageLedger, tree: Tree<'_>) -> Result<Status, Refusal> {
        let mut all = self.fold();
        let Some(open) = all.get_mut(&ledger.landing) else {
            return Err(Refusal::NoSuchLanding(ledger.landing.clone()));
        };
        if open.filed.contains_key(&ledger.speciality) {
            return Err(Refusal::AlreadyFiled(ledger.speciality));
        }
        validate_in(ledger, &open.landing, tree).map_err(Refusal::Invalid)?;
        self.append(&Row::Filed {
            ledger: ledger.clone(),
        })?;
        open.filed.insert(ledger.speciality, ledger.clone());
        Ok(open.status())
    }

    /// Where the challenge of `landing` stands; `None` when none is open under that id.
    #[must_use]
    pub fn status(&self, landing: &str) -> Option<Status> {
        self.fold().get(landing).map(Open::status)
    }

    /// The landing as it was routed, and the ledgers filed for it, in routing order.
    #[must_use]
    pub fn ledgers(&self, landing: &str) -> Option<(Landing, Vec<CoverageLedger>)> {
        let open = self.fold().remove(landing)?;
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
        self.fold()
            .into_iter()
            .filter_map(|(id, open)| match open.status() {
                Status::Pending { awaiting } => Some((id, awaiting)),
                Status::Cleared => None,
            })
            .collect()
    }

    /// The records folded: the first opening of an id stands; a filing counts only when its
    /// landing is open, its speciality has not filed yet, and its ledger validates again.
    fn fold(&self) -> BTreeMap<String, Open> {
        let rows = self.journal.read().map(|r| r.rows).unwrap_or_default();
        let mut all: BTreeMap<String, Open> = BTreeMap::new();
        for row in rows {
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

    fn append(&self, row: &Row) -> Result<(), Refusal> {
        if let Some(dir) = self.journal.path().parent() {
            private_dir(dir).map_err(|e| Refusal::Unwritten(e.to_string()))?;
        }
        self.journal
            .append(row)
            .map_err(|e| Refusal::Unwritten(e.to_string()))
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

/// The SHA-256 of the file at `path` in `world`, as 64 lowercase hex digits: `None` for a path
/// that does not normalise, a file that resolves outside the world, anything but a regular file,
/// or one over 32 MiB.
#[must_use]
pub fn tree_hash(world: &Path, path: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    use std::io::Read as _;
    let rel = crate::own::normal(path).filter(|p| !p.ends_with('/'))?;
    let root = std::fs::canonicalize(world).ok()?;
    let file = std::fs::canonicalize(world.join(rel)).ok()?;
    if !file.starts_with(&root) {
        return None;
    }
    let f = std::fs::File::open(&file).ok()?;
    let meta = f.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_HASHED {
        return None;
    }
    let mut bytes = Vec::new();
    f.take(MAX_HASHED + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_HASHED {
        return None;
    }
    Some(
        Sha256::digest(&bytes)
            .iter()
            .fold(String::new(), |mut hex, b| {
                let _ = write!(hex, "{b:02x}");
                hex
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::{Entry, State};

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

    fn na(surface: &str, class: &str) -> Entry {
        Entry {
            surface: surface.to_owned(),
            class: class.to_owned(),
            state: State::NotApplicable {
                reason: "the landing only renames a test helper; no input reaches it".to_owned(),
            },
        }
    }

    /// Every pair the landing requires of `s`, not applicable.
    fn full(landing: &Landing, s: Speciality) -> CoverageLedger {
        CoverageLedger {
            landing: landing.id.clone(),
            speciality: s,
            entries: landing
                .required(s)
                .into_iter()
                .map(|(surface, class)| na(&surface, class.name()))
                .collect(),
        }
    }

    fn no_tree(_: &str) -> Option<String> {
        None
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
        assert!(web.surfaces[0].holds("src/auth/token.rs"));
        assert!(!web.surfaces[0].holds("src/authz.rs"));
        let _ = std::fs::remove_dir_all(&w);
    }

    /// Two routed hunters (and the generalist): one filed leaves the challenge pending, all
    /// filed clears it.
    #[test]
    fn the_challenge_clears_only_when_every_routed_speciality_has_filed() {
        let w = temp("clear");
        let c = Challenges::at(&w.join("records/challenges.jsonl"));
        let l = landing(&["crates/mnemorph-wire/src/report.rs", "Cargo.lock"]);
        assert_eq!(
            c.open(&l),
            Ok(Status::Pending {
                awaiting: vec![Speciality::Agent, Speciality::Supply, Speciality::General]
            })
        );
        assert_eq!(
            c.file(&full(&l, Speciality::Agent), &no_tree),
            Ok(Status::Pending {
                awaiting: vec![Speciality::Supply, Speciality::General]
            })
        );
        // An invalid ledger is refused, and changes nothing.
        let mut half = full(&l, Speciality::Supply);
        half.entries.pop();
        let Err(Refusal::Invalid(r)) = c.file(&half, &no_tree) else {
            panic!("a missing pair is refused");
        };
        assert_eq!(r.rules(), [crate::coverage::Rule::MissingPair]);
        assert!(matches!(
            c.status(&l.id),
            Some(Status::Pending { ref awaiting }) if awaiting.len() == 2
        ));
        assert_eq!(c.pending().len(), 1);
        assert_eq!(
            c.file(&full(&l, Speciality::Supply), &no_tree),
            Ok(Status::Pending {
                awaiting: vec![Speciality::General]
            })
        );
        // The generalist owns no class: its ledger is empty, and filing it says its review ran.
        assert_eq!(
            c.file(&full(&l, Speciality::General), &no_tree),
            Ok(Status::Cleared)
        );
        assert_eq!(c.status(&l.id), Some(Status::Cleared));
        assert!(c.pending().is_empty());
        let (back, ledgers) = c.ledgers(&l.id).unwrap();
        assert_eq!(back, l);
        assert_eq!(ledgers.len(), 3);
        assert_eq!(
            c.file(&full(&l, Speciality::Agent), &no_tree),
            Err(Refusal::AlreadyFiled(Speciality::Agent))
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn openings_and_filings_are_refused_where_they_do_not_fit() {
        let w = temp("refusals");
        let c = Challenges::at(&w.join("challenges.jsonl"));
        let l = landing(&["Cargo.lock"]);
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
            c.file(&elsewhere, &no_tree),
            Err(Refusal::NoSuchLanding("other".to_owned()))
        );
        // Not routed here: refused by the state table.
        let Err(Refusal::Invalid(r)) = c.file(&full(&l, Speciality::Web), &no_tree) else {
            panic!("an unrouted speciality is refused");
        };
        assert_eq!(r.rules(), [crate::coverage::Rule::Unrouted]);
        for refusal in [
            Refusal::NoSuchLanding("x".into()),
            Refusal::AlreadyOpen("x".into()),
            Refusal::BadId,
            Refusal::AlreadyFiled(Speciality::Web),
            Refusal::Invalid(r),
            Refusal::Unwritten("disk".into()),
        ] {
            assert_ne!(refusal.to_string(), "");
        }
        let _ = std::fs::remove_dir_all(&w);
    }

    /// A filing line written by anything but the harness counts only if it validates on read.
    #[test]
    fn the_fold_validates_every_filing_again() {
        use std::io::Write as _;
        let w = temp("fold");
        let file = w.join("challenges.jsonl");
        let c = Challenges::at(&file);
        let l = landing(&["Cargo.lock"]);
        c.open(&l).unwrap();
        let mut forged = full(&l, Speciality::Supply);
        forged.entries.truncate(1);
        let line = serde_json::to_string(&Row::Filed { ledger: forged }).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap();
        writeln!(f, "{line}\nnot json").unwrap();
        // A second opening of the same id, written past the harness, is ignored too.
        let mut again = l.clone();
        again.routed.clear();
        let line = serde_json::to_string(&Row::Opened { landing: again }).unwrap();
        writeln!(f, "{line}").unwrap();
        drop(f);
        assert!(matches!(c.status(&l.id), Some(Status::Pending { .. })));
        assert_eq!(c.ledgers(&l.id).unwrap().1, []);
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn records_live_outside_the_world() {
        let w = temp("where");
        let r = records_of(&w).unwrap();
        assert!(!r.starts_with(&w), "{}", r.display());
        assert!(r.to_string_lossy().contains("mnemorph/challenges/"));
        assert!(Challenges::of(&w).is_some());
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn tree_hash_reads_only_regular_files_inside_the_world() {
        let w = temp("hash");
        std::fs::write(w.join("a.txt"), "abc").unwrap();
        assert_eq!(
            tree_hash(&w, "./a.txt").as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(tree_hash(&w, "missing"), None);
        assert_eq!(tree_hash(&w, "../a.txt"), None);
        assert_eq!(tree_hash(&w, "/etc/hostname"), None);
        std::fs::create_dir_all(w.join("d")).unwrap();
        assert_eq!(tree_hash(&w, "d"), None);
        #[cfg(unix)]
        {
            let outside = temp("hash-outside");
            std::fs::write(outside.join("secret"), "s").unwrap();
            std::os::unix::fs::symlink(outside.join("secret"), w.join("link")).unwrap();
            assert_eq!(tree_hash(&w, "link"), None, "a link out of the world");
            let _ = std::fs::remove_dir_all(&outside);
        }
        let _ = std::fs::remove_dir_all(&w);
    }
}
