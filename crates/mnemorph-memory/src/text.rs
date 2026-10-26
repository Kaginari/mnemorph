//! Text for recall: words, content words, and BM25 over a small set of documents.

use std::collections::BTreeSet;

/// Words with no meaning of their own: a query of only these recalls nothing.
const STOP: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "do", "for", "from", "has", "have",
    "how", "i", "if", "in", "into", "is", "it", "its", "of", "on", "or", "so", "that", "the",
    "then", "there", "this", "to", "was", "we", "what", "when", "where", "which", "who", "why",
    "will", "with", "you",
];

/// A text's words for BM25: lower-cased, alphanumeric runs.
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// BM25 of `query` against `doc` among `docs`.
#[must_use]
pub fn bm25(query: &[String], doc: &[String], docs: &[Vec<String>]) -> f64 {
    const K1: f64 = 1.2;
    const B: f64 = 0.75;
    #[allow(clippy::cast_precision_loss, reason = "word counts are small")]
    let len = |d: &[String]| d.len() as f64;
    #[allow(clippy::cast_precision_loss, reason = "document counts are small")]
    let n = docs.len() as f64;
    let avg = docs.iter().map(|d| len(d)).sum::<f64>() / n.max(1.0);
    query
        .iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|term| {
            #[allow(clippy::cast_precision_loss, reason = "counts are small")]
            let df = docs.iter().filter(|d| d.contains(term)).count() as f64;
            #[allow(clippy::cast_precision_loss, reason = "counts are small")]
            let tf = doc.iter().filter(|w| *w == term).count() as f64;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len(doc) / avg.max(1.0)))
        })
        .sum()
}

/// A text's content words: its words, the stop words aside.
#[must_use]
pub fn content_words(text: &str) -> Vec<String> {
    words(text)
        .into_iter()
        .filter(|w| !STOP.contains(&w.as_str()))
        .collect()
}
