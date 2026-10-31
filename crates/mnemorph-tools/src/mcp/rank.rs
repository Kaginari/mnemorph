//! Ranking the MCP catalog for an ask: BM25 over names and descriptions, nudged by what the
//! world learned from its own landed asks. No model ranks, and none writes the record.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::Spec;

/// The usage record's file, under a world's MCP state directory (`super::state_dir`): outside
/// the world, never a body's to write.
pub const USAGE: &str = "usage.json";

/// How much one helped ask weighs against lexical relevance: `ln(1 + helped)` times this.
pub const HELPED_WEIGHT: f64 = 1.0;
/// How much a call that helped nothing costs a tool: `ln(1 + idle)` times this.
pub const IDLE_WEIGHT: f64 = 0.25;

/// What one tool came to in this world.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    /// Asks it was called in.
    pub called: u64,
    /// Of those, asks that ended answered with nothing failing at their gate.
    pub helped: u64,
}

/// The world's record, by exposed tool name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Each tool's tally.
    pub tools: BTreeMap<String, Tally>,
}

impl Usage {
    /// The record kept under `state`; empty when there is none or it does not read.
    #[must_use]
    pub fn load(state: &Path) -> Self {
        std::fs::read(path(state))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// The learned part of a tool's score.
    #[must_use]
    pub fn boost(&self, tool: &str) -> f64 {
        let t = self.tools.get(tool).copied().unwrap_or_default();
        let idle = t.called.saturating_sub(t.helped);
        #[allow(clippy::cast_precision_loss)]
        let (helped, idle) = (t.helped as f64, idle as f64);
        HELPED_WEIGHT * helped.ln_1p() - IDLE_WEIGHT * idle.ln_1p()
    }
}

fn path(state: &Path) -> PathBuf {
    state.join(USAGE)
}

/// Record one ask under `state`: each tool in `used` was called in it, and helped when the ask did.
///
/// # Errors
///
/// The record could not be written.
pub fn learn(state: &Path, used: &BTreeSet<String>, helped: bool) -> std::io::Result<()> {
    if used.is_empty() {
        return Ok(());
    }
    let mut usage = Usage::load(state);
    for tool in used {
        let t = usage.tools.entry(tool.clone()).or_default();
        t.called += 1;
        if helped {
            t.helped += 1;
        }
    }
    let file = path(state);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&usage).unwrap_or_default())?;
    std::fs::rename(tmp, file)
}

/// Words that say nothing of a tool: a commission's glue, never matched.
const STOP: &[&str] = &[
    "a", "an", "and", "any", "are", "as", "at", "be", "by", "can", "do", "for", "from", "has",
    "have", "how", "if", "in", "into", "is", "it", "its", "of", "on", "or", "that", "the", "their",
    "then", "this", "to", "was", "we", "what", "when", "which", "will", "with", "you", "your",
];

/// The words of a text, lower-cased, split on anything not a letter or a digit, the stop words
/// and single letters aside.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 1 || w.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_lowercase)
        .filter(|w| !STOP.contains(&w.as_str()))
        .collect()
}

/// BM25's term saturation.
const K1: f64 = 1.2;
/// BM25's length normalisation.
const B: f64 = 0.75;

/// `specs` ordered best first for `query`: BM25 of the query's words over each tool's name,
/// server and description (`k1 = 1.2`, `b = 0.75`, the formula recall uses), plus the learned
/// boost. No model ranks; ties keep the catalog's own order, so the result is deterministic.
#[must_use]
pub fn rank<'a>(specs: &'a [Spec], query: &str, usage: &Usage) -> Vec<(&'a Spec, f64)> {
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
    let query: BTreeSet<String> = words(query).into_iter().collect();
    // Each query word's document frequency, counted once: ranking stays linear in the catalog.
    let df: BTreeMap<&String, usize> = query
        .iter()
        .map(|w| (w, docs.iter().filter(|d| d.contains(w)).count()))
        .collect();
    #[allow(clippy::cast_precision_loss, reason = "catalogs are small")]
    let n = specs.len() as f64;
    #[allow(clippy::cast_precision_loss, reason = "catalogs are small")]
    let avg = (docs.iter().map(Vec::len).sum::<usize>() as f64 / n.max(1.0)).max(1.0);
    let mut scored: Vec<(usize, f64)> = docs
        .iter()
        .enumerate()
        .map(|(i, doc)| {
            #[allow(clippy::cast_precision_loss, reason = "descriptions are capped")]
            let len = doc.len() as f64;
            let lexical: f64 = query
                .iter()
                .map(|w| {
                    let tf = doc.iter().filter(|d| *d == w).count();
                    if tf == 0 {
                        return 0.0;
                    }
                    #[allow(clippy::cast_precision_loss, reason = "descriptions are capped")]
                    let tf = tf as f64;
                    #[allow(clippy::cast_precision_loss, reason = "catalogs are small")]
                    let df = df[w] as f64;
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len / avg))
                })
                .sum();
            (i, lexical + usage.boost(&specs[i].name))
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(i, s)| (&specs[i], s)).collect()
}
