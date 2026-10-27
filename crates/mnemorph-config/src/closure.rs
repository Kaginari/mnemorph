//! Receipt-based closure: a rolled-back or corrected outward act stays an open incident until four
//! receipts are recorded, and while it is open the envelope that covered it is suspended.
//!
//! An incident opens in the witness, outside the world, keyed to the act's receipt: when the
//! human approves an undo (a **rollback**), when the human marks an observed act as wrong (a
//! **correction**), or when the human marks a `reconcile` as one. It closes only when each of four
//! receipts exists, each with its evidence:
//!
//! 1. **undone** — the receipt of the act's undo, `observed` in the witness: its probe saw the
//!    effect gone;
//! 2. **journal corrected** — the `corrected` row the harness appends to the act once that undo is
//!    observed, superseding its earlier rows, never editing them;
//! 3. **downstream told** — the receipt of a typed comment that names the incident (`tells`),
//!    `observed` in the witness; it went through the act runner like any comment;
//! 4. **lesson** — a test, a verify line or a Thought, checked to exist when it is recorded, and
//!    kept with its SHA-256.
//!
//! Every receipt is read again from the witness and the post's intents each time the incident is
//! read: an entry naming a receipt the witness does not hold as described counts for nothing, and
//! nothing a body writes in the world's journal is read at all.
//!
//! While an incident is open, the envelopes whose patterns read its act are suspended
//! ([`suspended`], for the gate that honours envelopes), and the act runner refuses every new act
//! of the post on the same target — the same branch on the same push URLs, or the same pull
//! request — but the comment that tells downstream.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use mnemorph_ledger::{Journal, Timestamp};
use mnemorph_races::Post;
use serde::{Deserialize, Serialize};

use crate::outward::envelope_reads;
use crate::receipt::{excerpt, fold, lock, private_file, sha256};
use crate::runner::{Intent, Typed, intents_path};
use crate::{Act, Envelope, Pin, PostJournal, Row, State};

/// The largest file a lesson may name, in bytes.
const LESSON_BYTES: u64 = 4 * 1024 * 1024;

/// Why an incident opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cause {
    /// The human approved the act's undo.
    Rollback,
    /// The human marked an observed act as wrong.
    Correction,
    /// The human marked a `reconcile` as an incident.
    Reconcile,
}

impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Cause::Rollback => "rollback",
            Cause::Correction => "correction",
            Cause::Reconcile => "reconcile",
        })
    }
}

/// The lesson an incident leaves: where it is written, and its SHA-256 when it was recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Lesson {
    /// A test, by its path in the world.
    Test {
        /// The file, relative to the world.
        path: String,
        /// Its content's SHA-256.
        sha256: String,
    },
    /// A verify line: a `Verify:` line of a creature's card.
    Verify {
        /// The card, relative to the world.
        path: String,
        /// The line, from 1.
        line: usize,
        /// The line's SHA-256.
        sha256: String,
    },
    /// A Thought: a note of the world's memory, by its id.
    Thought {
        /// The note's id.
        id: String,
        /// Its text's SHA-256.
        sha256: String,
    },
}

impl fmt::Display for Lesson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lesson::Test { path, sha256 } => write!(f, "test {path} (sha256 {sha256:.12})"),
            Lesson::Verify { path, line, sha256 } => {
                write!(f, "verify line {path}:{line} (sha256 {sha256:.12})")
            }
            Lesson::Thought { id, sha256 } => write!(f, "Thought {id} (sha256 {sha256:.12})"),
        }
    }
}

/// A file of the world a lesson names: a plain relative path, inside the world once links are
/// resolved, a regular file of at most [`LESSON_BYTES`].
fn world_file(world: &Path, path: &str) -> Result<(PathBuf, Vec<u8>), String> {
    let rel = Path::new(path);
    if path.is_empty()
        || path.chars().any(char::is_control)
        || !rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!(
            "`{}` is not a plain path inside the world",
            excerpt(path.as_bytes(), 200)
        ));
    }
    let root = world
        .canonicalize()
        .map_err(|e| format!("the world cannot be read: {e}"))?;
    let file = root
        .join(rel)
        .canonicalize()
        .map_err(|e| format!("{path}: {e}"))?;
    if !file.starts_with(&root) {
        return Err(format!("{path} leads outside the world"));
    }
    let meta = fs::metadata(&file).map_err(|e| format!("{path}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{path} is not a file"));
    }
    if meta.len() > LESSON_BYTES {
        return Err(format!("{path} is larger than {LESSON_BYTES} bytes"));
    }
    let bytes = fs::read(&file).map_err(|e| format!("{path}: {e}"))?;
    Ok((file, bytes))
}

/// The SHA-256 of `bytes`, in lower-case hex.
fn bytes_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

impl Lesson {
    /// A test at `path` in `world`, checked to exist, kept with its SHA-256.
    ///
    /// # Errors
    ///
    /// The path is not a plain file inside the world, or cannot be read.
    pub fn test(world: &Path, path: &str) -> Result<Self, String> {
        let (_, bytes) = world_file(world, path)?;
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Err(format!("{path} is empty: no lesson is written there"));
        }
        Ok(Lesson::Test {
            path: path.to_owned(),
            sha256: bytes_sha256(&bytes),
        })
    }

    /// The verify line at `path:line` in `world`: the line must hold `Verify:`.
    ///
    /// # Errors
    ///
    /// The path is not a plain file inside the world, or that line holds no `Verify:`.
    pub fn verify(world: &Path, path: &str, line: usize) -> Result<Self, String> {
        let (_, bytes) = world_file(world, path)?;
        let text = String::from_utf8_lossy(&bytes);
        let Some(found) = line.checked_sub(1).and_then(|n| text.lines().nth(n)) else {
            return Err(format!("{path} has no line {line}"));
        };
        if !found.contains("Verify:") || found.trim_start().len() <= "Verify:".len() {
            return Err(format!(
                "{path}:{line} is not a verify line (`Verify: <command>`)"
            ));
        }
        Ok(Lesson::Verify {
            path: path.to_owned(),
            line,
            sha256: sha256(found),
        })
    }

    /// The Thought `id`, whose text the caller read from the world's memory.
    ///
    /// # Errors
    ///
    /// The id is not plain, or the text is blank.
    pub fn thought(id: &str, text: &str) -> Result<Self, String> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
        {
            return Err("a Thought's id is plain letters, digits, `-`, `_`, `.` or `:`".into());
        }
        if text.trim().is_empty() {
            return Err(format!("Thought {id} says nothing"));
        }
        Ok(Lesson::Thought {
            id: id.to_owned(),
            sha256: sha256(text),
        })
    }
}

/// One line of a post's incidents file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub(crate) enum Entry {
    /// An incident opens on the act.
    Open {
        act: String,
        cause: Cause,
        at: Timestamp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        why: Option<String>,
        /// The envelope that covered the act, once the gate honours envelopes (M11).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        envelope: Option<String>,
    },
    /// Receipt (1): the act's undo.
    Undone {
        act: String,
        receipt: String,
        at: Timestamp,
    },
    /// Receipt (3): the comment that told downstream.
    Told {
        act: String,
        receipt: String,
        at: Timestamp,
    },
    /// Receipt (4): the lesson.
    Lesson {
        act: String,
        lesson: Lesson,
        at: Timestamp,
    },
}

impl Entry {
    fn act(&self) -> &str {
        match self {
            Entry::Open { act, .. }
            | Entry::Undone { act, .. }
            | Entry::Told { act, .. }
            | Entry::Lesson { act, .. } => act,
        }
    }
}

/// The four receipts an incident needs, by name.
pub const RECEIPTS: [&str; 4] = ["undone", "journal corrected", "downstream told", "lesson"];

/// An incident as its file and the witness leave it: each receipt present only when its evidence
/// holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incident {
    /// The post whose act it is.
    pub post: Post,
    /// The act's receipt id: the incident's key.
    pub act: String,
    /// Why it opened.
    pub cause: Cause,
    /// When.
    pub opened: Timestamp,
    /// What the human said of it.
    pub why: Option<String>,
    /// The envelope that covered the act, when one did.
    pub envelope: Option<String>,
    /// (1) the undo's receipt, `observed` in the witness.
    pub undone: Option<String>,
    /// (2) when the `corrected` row naming that undo was journaled.
    pub corrected: Option<Timestamp>,
    /// (3) the telling comment's receipt, `observed` in the witness.
    pub told: Option<String>,
    /// (4) the lesson.
    pub lesson: Option<Lesson>,
    /// The act, typed, and where it landed, from its intent.
    pub(crate) typed: Option<(Act, Pin)>,
}

impl Incident {
    /// The receipts still missing, by name ([`RECEIPTS`]).
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let have = [
            self.undone.is_some(),
            self.corrected.is_some(),
            self.told.is_some(),
            self.lesson.is_some(),
        ];
        RECEIPTS
            .iter()
            .zip(have)
            .filter(|(_, h)| !h)
            .map(|(n, _)| *n)
            .collect()
    }

    /// Whether it is still open: a receipt is missing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.missing().is_empty()
    }

    /// The act it is about, when its intent was kept.
    #[must_use]
    pub fn act_typed(&self) -> Option<&Act> {
        self.typed.as_ref().map(|(a, _)| a)
    }

    /// Whether `envelope` is suspended by it: it is open, and it names the envelope, or one of the
    /// envelope's patterns reads its act, or its act is not known (then every envelope is).
    #[must_use]
    pub fn suspends(&self, envelope: &Envelope) -> bool {
        self.is_open()
            && (self.envelope.as_deref() == Some(envelope.name.as_str())
                || self
                    .act_typed()
                    .is_none_or(|act| envelope_reads(envelope, act)))
    }

    /// Whether an act on `act` at `pin` lands where this incident's act did: the same branch on a
    /// push URL they share, or the same pull request on the same host.
    #[must_use]
    pub fn same_target(&self, act: &Act, pin: &Pin) -> bool {
        self.typed
            .as_ref()
            .is_some_and(|(mine, at)| same_target(mine, at, act, pin))
    }

    /// One escaped line for the human (`mnemorph status`).
    #[must_use]
    pub fn line(&self) -> String {
        let missing = self.missing();
        format!(
            "{} incident on act {} ({}) open since {}{}: missing {}; the envelopes that read it are suspended and new acts on its target are refused until it closes",
            self.post.creature(),
            excerpt(self.act.as_bytes(), 64),
            self.cause,
            self.opened,
            self.why
                .as_deref()
                .map(|w| format!(" — {}", excerpt(w.as_bytes(), 200)))
                .unwrap_or_default(),
            missing.join(", ")
        )
    }
}

/// The pull request a forge act is on: its host, its repository (as GitHub compares it) and its
/// number.
fn forge_target<'a>(act: &'a Act, pin: &'a Pin) -> Option<(&'a str, String, u64)> {
    match (act, pin) {
        (Act::AddLabel { repo, pr, .. } | Act::Comment { repo, pr, .. }, Pin::Forge { host }) => {
            Some((host.as_str(), repo.to_ascii_lowercase(), *pr))
        }
        _ => None,
    }
}

fn same_target(a: &Act, pa: &Pin, b: &Act, pb: &Pin) -> bool {
    match (a, pa, b, pb) {
        (
            Act::PushNewBranch { name: na, .. },
            Pin::Push { urls: ua, .. },
            Act::PushNewBranch { name: nb, .. },
            Pin::Push { urls: ub, .. },
        ) => na == nb && ua.iter().any(|u| ub.contains(u)),
        _ => match (forge_target(a, pa), forge_target(b, pb)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
    }
}

/// Whether a comment on `comment` at `pin` speaks to the pull request of the incident's act: for a
/// label or a comment, the same pull request; a branch has none the harness knows, so any.
pub(crate) fn tells_downstream(incident: &Incident, comment: &Act, pin: &Pin) -> bool {
    match &incident.typed {
        Some((mine @ (Act::AddLabel { .. } | Act::Comment { .. }), at)) => {
            matches!(comment, Act::Comment { .. }) && same_target(mine, at, comment, pin)
        }
        Some((Act::PushNewBranch { .. }, _)) => matches!(comment, Act::Comment { .. }),
        _ => false,
    }
}

impl PostJournal {
    fn incidents_file(&self) -> io::Result<PathBuf> {
        self.witness()
            .map(|w| w.with_extension("incidents.jsonl"))
            .ok_or_else(|| {
                io::Error::other("there is no state directory to keep incidents outside the world")
            })
    }

    /// Append one entry, under the witness's lock.
    pub(crate) fn record(&self, entry: &Entry) -> io::Result<()> {
        let path = self.incidents_file()?;
        let witness = self.witness().map(Path::to_path_buf).unwrap_or_default();
        let held = lock(&witness)?;
        drop(private_file(&path)?);
        Journal::new(path).append(entry)?;
        drop(held);
        Ok(())
    }

    /// The post's incidents, open and closed, each receipt counted only when the witness and the
    /// post's intents hold its evidence. A post with no witness has none.
    ///
    /// # Errors
    ///
    /// The incidents, the witness or the intents exist but cannot be read.
    pub fn incidents(&self) -> io::Result<Vec<Incident>> {
        let Some(witness) = self.witness().map(Path::to_path_buf) else {
            return Ok(Vec::new());
        };
        let path = self.incidents_file()?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let entries = Journal::<Entry>::new(&path).read()?.rows;
        let witnessed = Journal::<Row>::new(&witness);
        let rows = if witness.exists() {
            witnessed.read()?.rows
        } else {
            Vec::new()
        };
        let (latest, _) = if witness.exists() {
            fold(&witnessed)?
        } else {
            (Vec::new(), Vec::new())
        };
        let intents_file = intents_path(&witness);
        let intents: Vec<Intent> = if intents_file.exists() {
            Journal::<Intent>::new(&intents_file).read()?.rows
        } else {
            Vec::new()
        };
        // The last intent row of an act wins, as the runner reads them.
        let intent = |id: &str| intents.iter().rev().find(|i| i.act == id);
        let state = |id: &str| latest.iter().find(|r| r.act == id);
        let mut out: Vec<Incident> = Vec::new();
        for entry in &entries {
            if let Entry::Open {
                act,
                cause,
                at,
                why,
                envelope,
            } = entry
                && !out.iter().any(|i| &i.act == act)
            {
                out.push(Incident {
                    post: self.post,
                    act: act.clone(),
                    cause: *cause,
                    opened: *at,
                    why: why.clone(),
                    envelope: envelope.clone(),
                    undone: None,
                    corrected: None,
                    told: None,
                    lesson: None,
                    typed: intent(act).map(|i| (i.typed.act(), i.pin.pin())),
                });
            }
        }
        for entry in &entries {
            let Some(inc) = out.iter_mut().find(|i| i.act == entry.act()) else {
                continue;
            };
            match entry {
                Entry::Undone { receipt, .. } if inc.undone.is_none() => {
                    // (1) the undo's own latest row: observed, undoing this act.
                    let holds = state(receipt).is_some_and(|r| {
                        r.state == State::Observed && r.undoes.as_deref() == Some(&inc.act)
                    });
                    if holds {
                        inc.undone = Some(receipt.clone());
                    }
                }
                Entry::Told { receipt, .. } if inc.told.is_none() => {
                    // (3) an observed comment whose intent names this incident.
                    let observed = state(receipt).is_some_and(|r| r.state == State::Observed);
                    let named = intent(receipt).is_some_and(|i| {
                        matches!(i.typed, Typed::Comment { .. })
                            && i.tells.as_deref() == Some(&inc.act)
                            && i.at.0 >= inc.opened.0
                    });
                    if observed && named {
                        inc.told = Some(receipt.clone());
                    }
                }
                Entry::Lesson { lesson, .. } if inc.lesson.is_none() => {
                    inc.lesson = Some(lesson.clone());
                }
                _ => {}
            }
        }
        // (2) the act's `corrected` row, naming the undo counted as (1).
        for inc in &mut out {
            if let Some(undo) = &inc.undone {
                inc.corrected = rows
                    .iter()
                    .find(|r| {
                        r.act == inc.act
                            && r.state == State::Corrected
                            && r.by.as_deref() == Some(undo)
                    })
                    .map(|r| r.at);
            }
        }
        Ok(out)
    }

    /// The incident on `act`, if one was opened.
    ///
    /// # Errors
    ///
    /// The incidents cannot be read.
    pub fn incident(&self, act: &str) -> io::Result<Option<Incident>> {
        Ok(self.incidents()?.into_iter().find(|i| i.act == act))
    }

    /// The open incident whose act landed where `act` at `pin` would: what refuses a new act.
    /// Incidents that cannot be read refuse every act.
    pub(crate) fn blocking(&self, act: &Act, pin: &Pin) -> Result<Option<Incident>, String> {
        let all = self
            .incidents()
            .map_err(|e| format!("the post's incidents cannot be read, so no act runs: {e}"))?;
        Ok(all
            .into_iter()
            .find(|i| i.is_open() && i.same_target(act, pin)))
    }

    /// Open a rollback incident on `act`, unless one is already open or closed on it.
    pub(crate) fn open_rollback(&self, act: &str) -> io::Result<()> {
        if self.incident(act)?.is_some() {
            return Ok(());
        }
        self.record(&Entry::Open {
            act: act.to_owned(),
            cause: Cause::Rollback,
            at: Timestamp::now(),
            why: None,
            envelope: None,
        })
    }

    /// The human marks act `act` as an incident: a `reconcile` (a reconcile incident), or an
    /// observed act that was wrong (a correction). It then closes only with its four receipts.
    ///
    /// # Errors
    ///
    /// The act is in neither state, holds no kept intent (nothing could undo it), or already has
    /// an incident; or the record cannot be written.
    pub fn open_incident(&self, act: &str, why: Option<&str>) -> Result<Incident, String> {
        let row = self.latest(act)?;
        let cause = match row.state {
            State::Reconcile => Cause::Reconcile,
            State::Observed | State::Settled => Cause::Correction,
            other => {
                return Err(format!(
                    "act {act} is {other}: only an act in reconcile, or one observed or settled, is marked an incident"
                ));
            }
        };
        if row.undoes.is_some() {
            return Err(format!(
                "act {act} is an undo: the incident is the act it undoes"
            ));
        }
        if self
            .incident(act)
            .map_err(|e| format!("the post's incidents cannot be read: {e}"))?
            .is_some()
        {
            return Err(format!("act {act} has an incident already"));
        }
        let kept = self
            .witness()
            .map(intents_path)
            .filter(|p| p.exists())
            .map(|p| Journal::<Intent>::new(p).read().map(|b| b.rows))
            .transpose()
            .map_err(|e| format!("the post's intents cannot be read: {e}"))?
            .unwrap_or_default();
        if !kept.iter().any(|i| i.act == act) {
            return Err(format!(
                "act {act} has no kept intent: nothing could undo it, so it is no incident this post can close; settle it, or put it right by hand"
            ));
        }
        self.record(&Entry::Open {
            act: act.to_owned(),
            cause,
            at: Timestamp::now(),
            why: why.map(str::to_owned),
            envelope: None,
        })
        .map_err(|e| format!("the incident could not be recorded: {e}"))?;
        self.incident(act)
            .map_err(|e| format!("the post's incidents cannot be read: {e}"))?
            .ok_or_else(|| "the incident was not recorded".into())
    }

    /// The human settles act `act`, in `reconcile`, as no incident: a `settled` row, nothing
    /// undone, and it leaves their list.
    ///
    /// # Errors
    ///
    /// The act is not in `reconcile`, has an incident, or the row cannot be written.
    pub fn settle(&self, act: &str, why: Option<&str>) -> Result<(), String> {
        let row = self.latest(act)?;
        if row.state != State::Reconcile {
            return Err(format!(
                "act {act} is {}: only an act in reconcile is settled",
                row.state
            ));
        }
        if self
            .incident(act)
            .map_err(|e| format!("the post's incidents cannot be read: {e}"))?
            .is_some()
        {
            return Err(format!(
                "act {act} is an incident: it closes with its four receipts, not by settling"
            ));
        }
        let mut settled = Row::new(act, State::Settled);
        settled.why = Some(format!(
            "the human settled it as no incident{}",
            why.map(|w| format!(": {w}")).unwrap_or_default()
        ));
        settled.undoes.clone_from(&row.undoes);
        self.append(&settled)
            .map_err(|e| format!("the row could not be written: {e}"))
    }

    /// Record the lesson of the incident on `act`: receipt (4).
    ///
    /// # Errors
    ///
    /// There is no open incident on the act, it has its lesson already, or the record cannot be
    /// written.
    pub fn lesson(&self, act: &str, lesson: Lesson) -> Result<Incident, String> {
        let read = |j: &Self| {
            j.incident(act)
                .map_err(|e| format!("the post's incidents cannot be read: {e}"))
        };
        let Some(inc) = read(self)? else {
            return Err(format!("act {act} has no incident"));
        };
        if inc.lesson.is_some() {
            return Err(format!("the incident on act {act} has its lesson already"));
        }
        self.record(&Entry::Lesson {
            act: act.to_owned(),
            lesson,
            at: Timestamp::now(),
        })
        .map_err(|e| format!("the lesson could not be recorded: {e}"))?;
        read(self)?.ok_or_else(|| "the incident is gone".into())
    }

    /// Whether `envelope` is suspended by an open incident of this post. Incidents that cannot be
    /// read suspend it.
    #[must_use]
    pub fn suspends(&self, envelope: &Envelope) -> bool {
        self.incidents()
            .map_or(true, |all| all.iter().any(|i| i.suspends(envelope)))
    }

    /// The act's latest row, read from the witness alone.
    fn latest(&self, act: &str) -> Result<Row, String> {
        let Some(witness) = self.witness() else {
            return Err("there is no state directory: the post's witness is not kept".into());
        };
        let (rows, _) = fold(&Journal::<Row>::new(witness))
            .map_err(|e| format!("the post's witness cannot be read: {e}"))?;
        rows.into_iter().find(|r| r.act == act).ok_or_else(|| {
            format!(
                "act {act} is not in the {} post's witness",
                self.post.creature()
            )
        })
    }
}

/// Whether `envelope` is suspended: an open incident of any post of `world` reads its act. What M11's
/// gate asks before it honours an envelope; anything unreadable suspends it.
#[must_use]
pub fn suspended(world: &Path, envelope: &Envelope) -> bool {
    Post::ALL
        .into_iter()
        .any(|post| PostJournal::of(world, post).suspends(envelope))
}

/// One line per open incident of any post of `world`, with its missing receipts, for the human
/// (`mnemorph status`); one per post whose incidents cannot be read.
#[must_use]
pub fn incidents(world: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    for post in Post::ALL {
        match PostJournal::of(world, post).incidents() {
            Ok(all) => lines.extend(all.iter().filter(|i| i.is_open()).map(Incident::line)),
            Err(e) => lines.push(format!(
                "{}: the incidents cannot be read ({e}): every envelope is suspended",
                post.creature()
            )),
        }
    }
    lines
}
