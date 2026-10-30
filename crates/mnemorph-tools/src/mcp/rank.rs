//! Ranking the MCP catalog for an ask: lexical relevance over names and descriptions, nudged by
//! what the world learned from its own landed asks. No model ranks, and none writes the record.

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

/// The words of a text, lower-cased, split on anything not a letter or a digit.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(str::to_lowercase)
        .collect()
}

/// `specs` ordered best first for `query`: each query word a tool's name or description holds
/// scores its rarity across the catalog (`ln(1 + n / df)`), plus the learned boost; ties keep
/// the catalog's own order, so the result is deterministic.
#[must_use]
pub fn rank<'a>(specs: &'a [Spec], query: &str, usage: &Usage) -> Vec<(&'a Spec, f64)> {
    let docs: Vec<BTreeSet<String>> = specs
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
    #[allow(clippy::cast_precision_loss)]
    let n = specs.len() as f64;
    let mut scored: Vec<(usize, f64)> = docs
        .iter()
        .enumerate()
        .map(|(i, doc)| {
            let lexical: f64 = query
                .iter()
                .filter(|w| doc.contains(*w))
                .map(|w| {
                    #[allow(clippy::cast_precision_loss)]
                    let df = df[w] as f64;
                    (n / df).ln_1p()
                })
                .sum();
            (i, lexical + usage.boost(&specs[i].name))
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(i, s)| (&specs[i], s)).collect()
}
