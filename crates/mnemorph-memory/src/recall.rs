//! Recall: the few notes that fit a question, ranked by meaning and then by who wrote them, how
//! much they matter, how sure and how fresh — with a seat kept for the dissent, and what was cut
//! said, never hidden.

use crate::note::{Note, Status};
use crate::text::{bm25, content_words};

/// The weights, the floor and the saturation constants of a recall's score; all of them belong
/// in config, set against the recall fixtures.
#[derive(Debug, Clone, PartialEq)]
pub struct Weights {
    /// The meaning term's weight, `w_q`.
    pub meaning: f64,
    /// The relation term's weight, `w_r`: owner, race, the rest.
    pub relation: f64,
    /// Salience's weight, `w_s`.
    pub salience: f64,
    /// Confidence's weight, `w_κ`.
    pub confidence: f64,
    /// Freshness's weight, `w_t`.
    pub freshness: f64,
    /// The dissent's boost, `β`.
    pub dissent: f64,
    /// Freshness halves every this many days, `H`.
    pub half_life_days: f64,
    /// The floor the meaning term alone must clear, `θ`: the priors rank notes that match, they
    /// never make a note match.
    pub floor: f64,
    /// BM25's saturation `c`, by the query's length in content words: 1, 2–3, 4 and more.
    pub saturation: [f64; 3],
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            meaning: 0.42,
            relation: 0.34,
            salience: 0.10,
            confidence: 0.07,
            freshness: 0.07,
            dissent: 0.10,
            half_life_days: 120.0,
            floor: 0.12,
            saturation: [1.0, 2.0, 3.0],
        }
    }
}

/// Whose a note is, as recall weighs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// The asking body's own creature's: 1.0.
    Own,
    /// A creature of the asker's race: 0.6.
    Race,
    /// Any other shared note: 0.3.
    Shared,
    /// The episodic log: 0.1.
    Log,
}

impl Relation {
    fn weight(self) -> f64 {
        match self {
            Relation::Own => 1.0,
            Relation::Race => 0.6,
            Relation::Shared => 0.3,
            Relation::Log => 0.1,
        }
    }
}

/// One recalled note: the note, its score, and its meaning term.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The note.
    pub note: Note,
    /// Its whole score.
    pub score: f64,
    /// Its meaning term alone, `w_q · bm25/(bm25 + c)`.
    pub meaning: f64,
}

/// What a recall returns: the anchors, and what it cut that cleared the floor — a count and at
/// most three of them, so "none selected" never reads as "none exist".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Recalled {
    /// The anchors, best first; the dissent's seat among them when any dissent matched.
    pub anchors: Vec<Hit>,
    /// How many notes cleared the floor and were cut.
    pub cut: usize,
    /// At most three of the cut, best first, by id.
    pub cut_ids: Vec<String>,
}

/// Recall at most `k` notes (`k ≥ 2`) for `query` from `notes`, each with its relation to the
/// asker, at `now` (milliseconds since the epoch).
///
/// Silent when the query has no content words. Dormant and tombstoned notes are never recalled.
/// A note's meaning term must clear the floor on its own. One of the `k` seats is kept for the
/// best counter or revise note when any clears the floor, so a correction ranked `k + 1` is still
/// recalled; the rest go by score.
#[must_use]
pub fn recall(
    query: &str,
    notes: &[(Note, Relation)],
    now: u64,
    k: usize,
    weights: &Weights,
) -> Recalled {
    let terms = content_words(query);
    let k = k.max(2);
    let live: Vec<&(Note, Relation)> = notes
        .iter()
        .filter(|(n, _)| matches!(n.status, Status::Proposed | Status::Grounded))
        .collect();
    if terms.is_empty() || live.is_empty() {
        return Recalled::default();
    }
    let docs: Vec<Vec<String>> = live
        .iter()
        .map(|(n, _)| content_words(&format!("{} {}", n.text, n.tags.join(" "))))
        .collect();
    let saturation = match terms.len() {
        1 => weights.saturation[0],
        2 | 3 => weights.saturation[1],
        _ => weights.saturation[2],
    };
    let mut hits: Vec<Hit> = live
        .iter()
        .zip(&docs)
        .filter_map(|((n, rel), doc)| {
            let raw = bm25(&terms, doc, &docs);
            let meaning = weights.meaning * raw / (raw + saturation);
            if meaning < weights.floor {
                return None;
            }
            #[allow(clippy::cast_precision_loss, reason = "ages in days are small")]
            let days = now.saturating_sub(n.born) as f64 / 86_400_000.0;
            let fresh = (-days / weights.half_life_days).exp2();
            let dissent = if n.polarity.dissents() {
                weights.dissent
            } else {
                0.0
            };
            let score = meaning
                + weights.relation * rel.weight()
                + weights.salience * n.salience
                + weights.confidence * n.confidence
                + weights.freshness * fresh
                + dissent;
            Some(Hit {
                note: n.clone(),
                score,
                meaning,
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.note.id.cmp(&b.note.id))
    });
    // The dissent's seat: the best counter or revise note, whatever its rank.
    let seat = hits.iter().position(|h| h.note.polarity.dissents());
    let mut chosen: Vec<usize> = (0..hits.len()).take(k).collect();
    if let Some(s) = seat
        && !chosen.contains(&s)
    {
        chosen.pop();
        chosen.push(s);
    }
    let mut anchors = Vec::new();
    let mut cut = Vec::new();
    for (i, h) in hits.into_iter().enumerate() {
        if chosen.contains(&i) {
            anchors.push(h);
        } else {
            cut.push(h.note.id);
        }
    }
    Recalled {
        anchors,
        cut: cut.len(),
        cut_ids: cut.into_iter().take(3).collect(),
    }
}
