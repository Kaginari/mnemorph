//! Ranking the MCP catalog for an ask: BM25 over names and descriptions, plus what the world
//! learned from its own journal of loads and calls ([`super::learned`]). No model ranks here, and
//! none writes the journal.

use std::collections::{BTreeMap, BTreeSet};

use super::Spec;
use super::learned::{Learned, Lift, Who};

/// Words that say nothing of a tool: a commission's glue, never matched.
const STOP: &[&str] = &[
    "a", "an", "and", "any", "are", "as", "at", "be", "by", "can", "do", "for", "from", "has",
    "have", "how", "if", "in", "into", "is", "it", "its", "of", "on", "or", "that", "the", "their",
    "then", "this", "to", "was", "we", "what", "when", "which", "will", "with", "you", "your",
];

/// The words of a text, lower-cased, split on anything not a letter or a digit, the stop words
/// and single letters aside.
#[must_use]
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter(|w| w.chars().count() > 1 || w.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_lowercase)
        .filter(|w| !STOP.contains(&w.as_str()))
        .collect()
}

/// BM25's term saturation.
const K1: f64 = 1.2;
/// BM25's length normalisation.
const B: f64 = 0.75;

/// BM25 of `query`'s words over each of `docs` (`k1 = 1.2`, `b = 0.75`, the formula recall
/// uses): one score per document, in order.
#[must_use]
pub fn bm25(docs: &[Vec<String>], query: &BTreeSet<String>) -> Vec<f64> {
    // Each query word's document frequency, counted once: scoring stays linear in the corpus.
    let df: BTreeMap<&String, usize> = query
        .iter()
        .map(|w| (w, docs.iter().filter(|d| d.contains(w)).count()))
        .collect();
    #[allow(clippy::cast_precision_loss, reason = "catalogs are small")]
    let n = docs.len() as f64;
    #[allow(clippy::cast_precision_loss, reason = "catalogs are small")]
    let avg = (docs.iter().map(Vec::len).sum::<usize>() as f64 / n.max(1.0)).max(1.0);
    docs.iter()
        .map(|doc| {
            #[allow(clippy::cast_precision_loss, reason = "documents are capped")]
            let len = doc.len() as f64;
            query
                .iter()
                .map(|w| {
                    let tf = doc.iter().filter(|d| *d == w).count();
                    if tf == 0 {
                        return 0.0;
                    }
                    #[allow(clippy::cast_precision_loss, reason = "documents are capped")]
                    let tf = tf as f64;
                    #[allow(clippy::cast_precision_loss, reason = "catalogs are small")]
                    let df = df[w] as f64;
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len / avg))
                })
                .sum()
        })
        .collect()
}

/// `specs` ordered best first for `query`: BM25 of the query's words over each tool's name,
/// server and description, plus the learned lift for `who` — the relation to what this creature
/// or race used on similar asks, less the decaying penalty of loads never used. No model ranks;
/// ties keep the catalog's own order, so the result is deterministic.
#[must_use]
pub fn rank<'a>(specs: &'a [Spec], query: &str, learned: &Learned, who: &Who) -> Vec<Ranked<'a>> {
    let docs: Vec<Vec<String>> = specs
        .iter()
        .map(|s| {
            words(&s.tool)
                .into_iter()
                .chain(words(&s.server))
                .chain(words(&s.description))
                .collect()
        })
        .collect();
    let terms: BTreeSet<String> = words(query).into_iter().collect();
    let lexical = bm25(&docs, &terms);
    let lifts = learned.lifts(&terms, who);
    let mut scored: Vec<(usize, f64, Lift)> = lexical
        .into_iter()
        .enumerate()
        .map(|(i, l)| (i, l, lifts.get(&specs[i].name).cloned().unwrap_or_default()))
        .collect();
    scored.sort_by(|a, b| {
        (b.1 + b.2.net())
            .total_cmp(&(a.1 + a.2.net()))
            .then(a.0.cmp(&b.0))
    });
    scored
        .into_iter()
        .map(|(i, lexical, lift)| Ranked {
            spec: &specs[i],
            lexical,
            score: lexical + lift.net(),
            lift,
        })
        .collect()
}

/// One ranked tool.
#[derive(Debug, Clone)]
pub struct Ranked<'a> {
    /// The tool.
    pub spec: &'a Spec,
    /// Its BM25 score for the query alone: above zero when it matches a word of it.
    pub lexical: f64,
    /// What the journal taught: the relation and the idle penalty, each bounded.
    pub lift: Lift,
    /// What it is ordered by: the BM25 score and the learned lift.
    pub score: f64,
}

impl Ranked<'_> {
    /// Why it was offered, for the human: `bm25 3.1 + used 4× by kijin-forge`.
    #[must_use]
    pub fn why(&self) -> String {
        format!("bm25 {:.1}{}", self.lexical, self.lift.said())
    }
}
