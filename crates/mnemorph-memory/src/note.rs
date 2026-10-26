//! A trace and the events that make it: a note is never edited, only told about. Its current
//! state is a fold over its events, which never trusts line order — two machines that both
//! appended merge without a conflict.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What a note says of what it points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Polarity {
    /// It holds.
    Support,
    /// It does not hold.
    Counter,
    /// It held, and must be read again: a correction.
    Revise,
}

impl Polarity {
    /// A counter or a revise: the dissent, which recall keeps a seat for.
    #[must_use]
    pub fn dissents(self) -> bool {
        matches!(self, Polarity::Counter | Polarity::Revise)
    }
}

/// How far a note is trusted, or where it went. Ordered by strength, the order only events the
/// time cannot separate fall back on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Said, not proved: personal only.
    Proposed,
    /// Proved by a route: it may be shared.
    Grounded,
    /// Kept, not recalled.
    Dormant,
    /// Gone from recall for good; its events stay.
    Tombstoned,
}

/// Who wrote an event: the harness's own record, the end gate, a route's promotion, the human,
/// or an import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum By {
    /// The harness, recording what a body said.
    Record,
    /// The end gate.
    Gate,
    /// A route promoting a note, with its proof.
    Route,
    /// The human.
    Human,
    /// Brought in from elsewhere.
    Import,
}

/// What one event does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Change {
    /// A note is born.
    Create {
        /// What it says.
        text: String,
        /// Its tags, for recall.
        #[serde(default)]
        tags: Vec<String>,
        /// The creature whose note it is; `None` for the world's.
        #[serde(default)]
        owner: Option<String>,
        /// What it says of what it points at.
        polarity: Polarity,
        /// How much it matters, in `[0, 1]`.
        salience: f64,
        /// How sure, in `[0, 1]`.
        confidence: f64,
        /// The evidence it points at: a journal entry, a verdict, a commit, a `file:line`.
        #[serde(default)]
        source: Option<String>,
    },
    /// Its standing changes: a promotion, a demotion, a tombstone.
    Status {
        /// The new standing.
        status: Status,
    },
    /// Its confidence moves: a pass gained, a failure that reconsolidates it.
    Confidence {
        /// The new confidence, in `[0, 1]`.
        confidence: f64,
    },
}

/// One event: one full line of a note file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Unique among events: with `time`, the fold's order.
    pub id: String,
    /// When, in milliseconds since the epoch.
    pub time: u64,
    /// The note it is about.
    pub note: String,
    /// Who wrote it.
    pub by: By,
    /// The proof an event that claims a standing cites: a `log.md` entry, a commit, a verdict.
    #[serde(default)]
    pub proof: Option<String>,
    /// What it does.
    #[serde(flatten)]
    pub change: Change,
}

/// A note as its events leave it.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// Its id.
    pub id: String,
    /// What it says.
    pub text: String,
    /// Its tags.
    pub tags: Vec<String>,
    /// Whose it is; `None` for the world's.
    pub owner: Option<String>,
    /// What it says of what it points at.
    pub polarity: Polarity,
    /// How much it matters.
    pub salience: f64,
    /// How sure.
    pub confidence: f64,
    /// Its standing now.
    pub status: Status,
    /// What it points at.
    pub source: Option<String>,
    /// The proof its standing rests on, when one was cited.
    pub proof: Option<String>,
    /// When it was born, in milliseconds since the epoch: its creation's time.
    pub born: u64,
}

/// Fold events into notes. Events are taken in `(time, id)` order and the latest sets a note's
/// status and confidence, so a demotion lowers what came before it; only events the order cannot
/// separate — the same time and id, a merge's duplicate — take the stronger status. An event about
/// a note never created is set aside, never guessed at; a second creation of one is ignored.
#[must_use]
pub fn fold(events: &[Event]) -> BTreeMap<String, Note> {
    // A total order, whatever the line order: two machines whose union merges listed the same
    // events differently fold them the same.
    let mut ordered: Vec<(&Event, String)> = events.iter().map(|e| (e, format!("{e:?}"))).collect();
    ordered.sort_by(|(a, x), (b, y)| (a.time, &a.id, x).cmp(&(b.time, &b.id, y)));
    let ordered = ordered.into_iter().map(|(e, _)| e);
    let mut notes: BTreeMap<String, Note> = BTreeMap::new();
    let mut last: BTreeMap<String, (u64, String)> = BTreeMap::new();
    for e in ordered {
        match &e.change {
            Change::Create {
                text,
                tags,
                owner,
                polarity,
                salience,
                confidence,
                source,
            } => {
                notes.entry(e.note.clone()).or_insert_with(|| Note {
                    id: e.note.clone(),
                    text: text.clone(),
                    tags: tags.clone(),
                    owner: owner.clone(),
                    polarity: *polarity,
                    salience: salience.clamp(0.0, 1.0),
                    confidence: confidence.clamp(0.0, 1.0),
                    status: Status::Proposed,
                    source: source.clone(),
                    proof: None,
                    born: e.time,
                });
            }
            Change::Status { status } => {
                let Some(n) = notes.get_mut(&e.note) else {
                    continue;
                };
                let key = (e.time, e.id.clone());
                // The same time and id twice is a merge's duplicate: the stronger one stands.
                let tie = last.get(&e.note) == Some(&key);
                if !tie || *status > n.status {
                    n.status = *status;
                    n.proof.clone_from(&e.proof);
                }
                last.insert(e.note.clone(), key);
            }
            Change::Confidence { confidence } => {
                if let Some(n) = notes.get_mut(&e.note) {
                    n.confidence = confidence.clamp(0.0, 1.0);
                }
            }
        }
    }
    notes
}
