//! Recall: the few notes that fit a question, ranked by meaning and then by who wrote them, how
//! much they matter, how sure and how fresh — with a seat kept for the dissent, and what was cut
//! said, never hidden.

use crate::note::{Note, Status};
use crate::text::{bm25, content_words};

/// The weights, the floor and the saturation constants of a recall's score; all of them belong
/// in config, set against the recall fixtures.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// The claim strength's weight, `w_σ`: at the default, a grounded claim in good health
    /// (strength at least 0.5) gains more than any confidence can give an unproven one.
    pub strength: f64,
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
            strength: 0.10,
            half_life_days: 120.0,
            floor: 0.12,
            // Set against the fixtures: a lone note matching three content words of four clears
            // the floor; a word every note holds stays far under it.
            saturation: [0.5, 1.0, 1.5],
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

/// One recalled note: the note, its score, its meaning term and its strength.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The note.
    pub note: Note,
    /// Its whole score.
    pub score: f64,
    /// Its meaning term alone, `w_q · bm25/(bm25 + c)`.
    pub meaning: f64,
    /// Its claim strength, with what made it.
    pub strength: crate::graph::Strength,
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
/// Silent when the query has no content words. Dormant and tombstoned notes are never recalled,
/// and a proposed note only when it is the asker's own. A note's meaning term must clear the
/// floor on its own. The seats: the `k − 2` best agreeing notes, the best counter or revise note
/// whatever its rank — so a correction ranked `k + 1` is still recalled — then the best of the
/// rest to fill `k`.
///
/// Every call reads the notes' words afresh: a caller recalling turn after turn keeps them (and
/// their document frequencies) between calls, so the index is never rebuilt inline.
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
    // A proposed note is a guess: only the asker's own are weighed, never another's.
    let live: Vec<&(Note, Relation)> = notes
        .iter()
        .filter(|(n, rel)| {
            n.status == Status::Grounded || (n.status == Status::Proposed && *rel == Relation::Own)
        })
        .collect();
    if terms.is_empty() || live.is_empty() {
        return Recalled::default();
    }
    let docs: Vec<Vec<String>> = live
        .iter()
        .map(|(n, _)| content_words(&format!("{} {}", n.text, n.tags.join(" "))))
        .collect();
    // Over every note, the dormant too: a claim resting on a retired one is no stronger than it.
    let strengths = crate::graph::strengths(notes.iter().map(|(n, _)| n), now);
    let saturation = match terms.len() {
        1 => weights.saturation[0],
        2 | 3 => weights.saturation[1],
        _ => weights.saturation[2],
    };
    let mut hits: Vec<Hit> = live
        .iter()
        .zip(&docs)
        .filter_map(|((n, rel), doc)| {
            // A correction is matched with the words it borrows too; how common a word is stays
            // the notes' own count, so borrowing never dilutes the note it corrects.
            let lent;
            let doc = if n.lent.is_empty() {
                doc
            } else {
                lent = [doc.clone(), content_words(&n.lent.join(" "))].concat();
                &lent
            };
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
            let strength = strengths.get(&n.id).cloned()?;
            let score = meaning
                + weights.relation * rel.weight()
                + weights.salience * n.salience
                + weights.confidence * n.confidence
                + weights.freshness * fresh
                + weights.strength * strength.value
                + dissent;
            Some(Hit {
                note: n.clone(),
                score,
                meaning,
                strength,
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.note.id.cmp(&b.note.id))
    });
    // The seats: the k − 2 best agreeing notes, the best dissent whatever its rank, then the best
    // of the rest to fill k.
    let mut chosen: Vec<usize> = (0..hits.len())
        .filter(|&i| !hits[i].note.polarity.dissents())
        .take(k - 2)
        .collect();
    if let Some(d) = hits.iter().position(|h| h.note.polarity.dissents()) {
        chosen.push(d);
    }
    for i in 0..hits.len() {
        if chosen.len() >= k {
            break;
        }
        if !chosen.contains(&i) {
            chosen.push(i);
        }
    }
    chosen.sort_unstable();
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

/// What a creature recalls from: the notes, each with its relation to it, and which of them are
/// its personal ones.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    /// The notes, each once, with their relations.
    pub notes: Vec<(Note, Relation)>,
    /// The ids of the creature's own personal notes.
    pub personal: std::collections::BTreeSet<String>,
    /// Lines of the note files that could not be read: ignored here, and to be told.
    pub skipped: usize,
    /// Events whose author or proof does not resolve: ignored here, and to be told.
    pub unproven: usize,
}

/// The notes a creature recalls from: its own personal notes, and the shared ones — a shared
/// note of its own, or of its race (`kin`), weighed as such — each note once, its personal copy
/// first.
///
/// # Errors
///
/// A note file cannot be read.
pub fn gather(
    world: &std::path::Path,
    name: &str,
    kin: impl Fn(&str) -> bool,
) -> std::io::Result<Memory> {
    use crate::store::{Tier, read};
    let own = read(world, &Tier::Personal(name.to_owned()))?;
    let short = read(world, &Tier::Short(name.to_owned()))?;
    let shared = read(world, &Tier::Shared)?;
    let skipped = own.skipped.len() + short.skipped.len() + shared.skipped.len();
    // An event whose author or proof does not resolve is ignored, each tier judged on its own.
    let (own, a) = crate::audit::resolved(own.rows);
    let (short, b) = crate::audit::resolved(short.rows);
    let (shared, c) = crate::audit::resolved(shared.rows);
    // The body's own: its personal notes and its short-tier cache, both personal in scope.
    let mut events = own;
    events.extend(short);
    let mine = crate::note::fold(&events);
    let personal: std::collections::BTreeSet<String> = mine.keys().cloned().collect();
    let mut notes: Vec<(Note, Relation)> = mine.into_values().map(|n| (n, Relation::Own)).collect();
    for n in crate::note::fold(&shared).into_values() {
        if personal.contains(&n.id) {
            continue;
        }
        let rel = match n.owner.as_deref() {
            Some(o) if o == name => Relation::Own,
            Some(o) if kin(o) => Relation::Race,
            _ => Relation::Shared,
        };
        notes.push((n, rel));
    }
    lend_to_corrections(&mut notes);
    Ok(Memory {
        notes,
        personal,
        skipped,
        unproven: a + b + c,
    })
}

/// A correction is recalled where the note it corrects would be: a revise note takes the closest
/// relation of the notes it cites — all it links, when it cites none — and their words for
/// matching (`Note::lent`): never stored, only weighed.
/// The head start a correction is given over an ordinary note (`0.154`, worth `0.37` of meaning)
/// holds only at equal relation and equal match; the body that failed and the one that recalls
/// are not always the same, nor its failure line in the habit's words.
fn lend_to_corrections(notes: &mut [(Note, Relation)]) {
    let lent: Vec<Option<(Relation, Vec<String>)>> = notes
        .iter()
        .map(|(n, rel)| {
            if n.polarity != crate::note::Polarity::Revise || n.links.is_empty() {
                return None;
            }
            // What it cites, when it cites any: a note merely recalled into the failed run was in
            // the room, and is never blamed by company.
            let cited: Vec<&str> = n
                .links
                .iter()
                .filter(|l| l.weight >= 0.9)
                .map(|l| l.note.as_str())
                .collect();
            let lenders: Vec<&str> = if cited.is_empty() {
                n.links.iter().map(|l| l.note.as_str()).collect()
            } else {
                cited
            };
            let linked: Vec<&(Note, Relation)> = notes
                .iter()
                .filter(|(m, _)| lenders.contains(&m.id.as_str()))
                .collect();
            let closest = linked
                .iter()
                .map(|(_, r)| *r)
                .chain(std::iter::once(*rel))
                .max_by(|a, b| a.weight().total_cmp(&b.weight()))?;
            let words = linked.iter().map(|(m, _)| m.text.clone()).collect();
            Some((closest, words))
        })
        .collect();
    for ((n, rel), lend) in notes.iter_mut().zip(lent) {
        if let Some((closest, words)) = lend {
            *rel = closest;
            n.lent = words;
        }
    }
}

/// A recall on the wire, as `@M` lines: each anchor with its scope, standing and polarity — and
/// `review` while something it rests on has moved — then its text; the cut above the floor as a count and at most three ids; and, when no dissent was
/// selected, that it was none selected — never that none exist. Nothing for an empty recall.
#[must_use]
pub fn lines(r: &Recalled, personal: &std::collections::BTreeSet<String>) -> Vec<String> {
    if r.anchors.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = r
        .anchors
        .iter()
        .map(|h| {
            let scope = if personal.contains(&h.note.id) {
                "personal"
            } else {
                "shared"
            };
            let standing = match h.note.status {
                Status::Grounded => "grounded",
                _ => "proposed",
            };
            let polarity = match h.note.polarity {
                crate::note::Polarity::Support => "support",
                crate::note::Polarity::Counter => "counter",
                crate::note::Polarity::Revise => "revise",
            };
            let text: String = h.note.text.split_whitespace().collect::<Vec<_>>().join(" ");
            let review = if h.note.review.is_some() {
                " review"
            } else {
                ""
            };
            format!(
                "@M {} {scope} {standing} {polarity}{review}: {text}",
                h.note.id
            )
        })
        .collect();
    if r.cut > 0 {
        out.push(format!("@M -{} {}", r.cut, r.cut_ids.join(" ")));
    }
    if !r.anchors.iter().any(|h| h.note.polarity.dissents()) {
        out.push("@M -0 dissent: none selected".to_owned());
    }
    out
}
