//! What the world learns from its MCP journal: every level-1 and level-2 load, every call and
//! every offer, one row each, kept chained under the human's state directory and witnessed beside
//! it — never in the world, so no body and no repository's script writes it. Only the harness
//! appends a row, from what its own doors did; the loop walks the chain and the witness before a
//! row is believed ([`Learned::from_rows`] is fed only rows that held).
//!
//! The learning is model-free, bounded and deterministic: the same journal and the same ask give
//! the same order, every time.
//!
//! - **Relation.** A tool this creature (weight 1) or its race (weight ½) called and that answered
//!   on a similar ask is lifted. Similarity is BM25 of the ask's words over each past call's ask
//!   words, divided by the best such score, so the most similar past ask counts 1; an ask that
//!   ended with something failing at its gate counts half. The lift is `ln(1 + Σ)`, capped at
//!   [`RELATION_CAP`].
//! - **Idle penalty.** A schema loaded (level 2) and never called in its ask costs the tool
//!   [`IDLE_PENALTY`], halved every [`HALF_LIFE`] asks the journal records after it, the sum
//!   capped at [`PENALTY_CAP`].
//!
//! The lift orders, never admits: a tool is offered only for an ask it matches by BM25.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::rank::{bm25, words};

/// The journal's file, under a world's MCP state directory ([`super::state_dir`]).
pub const JOURNAL: &str = "journal.jsonl";
/// The most of an ask's words a row keeps: enough to match on, never the whole commission.
pub const TERMS: usize = 32;
/// The most rows the learning reads, newest kept.
pub const WINDOW: usize = 4096;
/// How much relation a tool may gain.
pub const RELATION_CAP: f64 = 2.0;
/// What one schema loaded and never called costs, before it decays.
pub const IDLE_PENALTY: f64 = 0.5;
/// After how many later asks an idle load's penalty is halved.
pub const HALF_LIFE: f64 = 8.0;
/// The most penalty a tool may carry.
pub const PENALTY_CAP: f64 = 1.5;
/// How much a use by another creature of the same race weighs against the creature's own.
pub const RACE_WEIGHT: f64 = 0.5;

/// Where `state`'s journal lives.
#[must_use]
pub fn journal_file(state: &Path) -> PathBuf {
    state.join(JOURNAL)
}

/// Where the journal's witness lives: beside the state directory, `<state>.witness.jsonl`. The
/// harness writes every byte to it first; a journal whose head is not the witness's lost rows.
#[must_use]
pub fn witness_file(state: &Path) -> PathBuf {
    let mut name = state.file_name().unwrap_or_default().to_os_string();
    name.push(".witness.jsonl");
    state.with_file_name(name)
}

/// What a row records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A server's level 1 loaded.
    Open,
    /// A tool's level 2 loaded.
    Schema,
    /// A tool called.
    Call,
    /// A tool offered in an ask's tail.
    Offer,
    /// The fast tier reranked an ask's shortlist.
    Rerank,
}

/// What one door did, as the tool reports it to the loop: the loop turns it into a [`Row`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touched {
    /// What it was.
    pub kind: Kind,
    /// The server.
    pub server: String,
    /// The tool, by the name a body calls it; `None` for a level-1 load.
    pub tool: Option<String>,
    /// The weighted tokens it put in the conversation.
    pub wt: u64,
    /// How it ended: `loaded`, `ok`, `error`, `denied` or `unheld`.
    pub outcome: String,
}

/// One journal row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// When, in seconds since the epoch.
    pub ts: u64,
    /// The ask it belongs to: `<run>#<n>`.
    pub ask: String,
    /// What it records.
    pub kind: Kind,
    /// The server.
    pub server: String,
    /// The tool; `None` for a level-1 load or a rerank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// The creature that asked.
    pub creature: String,
    /// Its race.
    pub race: String,
    /// The commission's fingerprint: the SHA-256 of its sorted words, 16 hex digits.
    pub fingerprint: String,
    /// The commission's words, sorted, at most [`TERMS`]: what a later ask is matched against.
    pub terms: Vec<String>,
    /// The weighted tokens spent: what the load or the answer put in the conversation, or what
    /// the rerank cost.
    pub wt: u64,
    /// Whether the tool (a level-1 load: any tool of the server) was called after this, in the
    /// same ask.
    pub used: bool,
    /// How it ended: `loaded`, `ok`, `error`, `denied`, `unheld`, `offered`, `reranked` or
    /// `fallback`.
    pub outcome: String,
    /// Whether the ask ended answered with nothing failing at its gate.
    pub helped: bool,
    /// Why it was offered, for an offer row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// An ask's words as a row keeps them, and their fingerprint.
#[must_use]
pub fn terms_of(ask: &str) -> (Vec<String>, String) {
    let all: BTreeSet<String> = words(ask).into_iter().collect();
    let mut h = Sha256::new();
    for w in &all {
        h.update(w.as_bytes());
        h.update([0]);
    }
    let fingerprint = h.finalize().iter().take(8).fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    });
    (all.into_iter().take(TERMS).collect(), fingerprint)
}

/// Who an ask is for: the creature and its race, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Who {
    /// The creature.
    pub creature: String,
    /// Its race.
    pub race: String,
}

/// What the journal taught of one tool for one ask.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lift {
    /// The relation: `ln(1 + Σ similarity × weight)`, at most [`RELATION_CAP`].
    pub relation: f64,
    /// Past calls on similar asks by the creature itself.
    pub by_creature: usize,
    /// Past calls on similar asks by others of its race.
    pub by_race: usize,
    /// The creature, for the line.
    pub creature: String,
    /// The race, for the line.
    pub race: String,
    /// The idle penalty, at most [`PENALTY_CAP`].
    pub penalty: f64,
    /// Schemas loaded and never called.
    pub idle: usize,
}

impl Lift {
    /// The relation less the penalty.
    #[must_use]
    pub fn net(&self) -> f64 {
        self.relation - self.penalty
    }

    /// What it adds to a line: ` + used 4× by kijin-forge (+1.61)`, ` − loaded idle 2× (−0.71)`.
    #[must_use]
    pub fn said(&self) -> String {
        let mut out = String::new();
        if self.relation > 0.0 {
            let mut by = Vec::new();
            if self.by_creature > 0 {
                by.push(format!("{}× by {}", self.by_creature, self.creature));
            }
            if self.by_race > 0 {
                by.push(format!("{}× by its race {}", self.by_race, self.race));
            }
            let _ = write!(out, " + used {} (+{:.2})", by.join(", "), self.relation);
        }
        if self.penalty > 0.0 {
            let _ = write!(out, " − loaded idle {}× (−{:.2})", self.idle, self.penalty);
        }
        out
    }
}

/// The rows the learning reads, as they held.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Learned {
    rows: Vec<Row>,
}

impl Learned {
    /// The learning over `rows`, oldest first: the newest [`WINDOW`] kept. Feed it only rows whose
    /// chain and witness held.
    #[must_use]
    pub fn from_rows(mut rows: Vec<Row>) -> Self {
        if rows.len() > WINDOW {
            rows.drain(..rows.len() - WINDOW);
        }
        Self { rows }
    }

    /// The rows.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Each tool's lift for an ask of `terms` by `who`; a tool the journal taught nothing of has
    /// none.
    #[must_use]
    pub fn lifts(&self, terms: &BTreeSet<String>, who: &Who) -> BTreeMap<String, Lift> {
        let mut out: BTreeMap<String, Lift> = BTreeMap::new();
        let blank = || Lift {
            creature: who.creature.clone(),
            race: who.race.clone(),
            ..Lift::default()
        };
        // Relation: past calls that answered, by this creature or its race, on similar asks.
        let calls: Vec<&Row> = self
            .rows
            .iter()
            .filter(|r| r.kind == Kind::Call && r.outcome == "ok" && r.tool.is_some())
            .filter(|r| r.creature == who.creature || r.race == who.race)
            .collect();
        if !terms.is_empty() && !calls.is_empty() {
            let docs: Vec<Vec<String>> = calls.iter().map(|r| r.terms.clone()).collect();
            let sims = bm25(&docs, terms);
            let best = sims.iter().copied().fold(0.0_f64, f64::max);
            let mut sum: BTreeMap<String, f64> = BTreeMap::new();
            if best > 0.0 {
                for (row, sim) in calls.iter().zip(sims) {
                    if sim <= 0.0 {
                        continue;
                    }
                    let Some(tool) = &row.tool else { continue };
                    let own = row.creature == who.creature;
                    let weight = if own { 1.0 } else { RACE_WEIGHT };
                    let helped = if row.helped { 1.0 } else { 0.5 };
                    *sum.entry(tool.clone()).or_default() += sim / best * weight * helped;
                    let lift = out.entry(tool.clone()).or_insert_with(blank);
                    if own {
                        lift.by_creature += 1;
                    } else {
                        lift.by_race += 1;
                    }
                }
            }
            for (tool, s) in sum {
                if let Some(l) = out.get_mut(&tool) {
                    l.relation = s.ln_1p().min(RELATION_CAP);
                }
            }
        }
        // Idle penalty: schemas loaded and never called, each halved per later ask.
        let mut later = BTreeSet::new();
        let mut penalty: BTreeMap<String, f64> = BTreeMap::new();
        for row in self.rows.iter().rev() {
            if row.kind == Kind::Schema
                && !row.used
                && let Some(tool) = &row.tool
            {
                #[allow(clippy::cast_precision_loss, reason = "the window is capped")]
                let age = later.len() as f64;
                *penalty.entry(tool.clone()).or_default() +=
                    IDLE_PENALTY * 0.5_f64.powf(age / HALF_LIFE);
                out.entry(tool.clone()).or_insert_with(blank).idle += 1;
            }
            later.insert(row.ask.as_str());
        }
        for (tool, p) in penalty {
            if let Some(l) = out.get_mut(&tool) {
                l.penalty = p.min(PENALTY_CAP);
            }
        }
        out
    }

    /// Per server, the weighted tokens its levels 1 and 2 loaded against those whose tools were
    /// then called, and what its calls' answers and the reranks cost: a server whose catalog
    /// costs more than it earns shows up as a number.
    #[must_use]
    pub fn spend(&self) -> Vec<String> {
        let mut by: BTreeMap<&str, (u64, u64, u64)> = BTreeMap::new();
        let mut rerank = (0usize, 0u64);
        for r in &self.rows {
            match r.kind {
                Kind::Open | Kind::Schema => {
                    let e = by.entry(&r.server).or_default();
                    e.0 += r.wt;
                    if r.used {
                        e.1 += r.wt;
                    }
                }
                Kind::Call => by.entry(&r.server).or_default().2 += r.wt,
                Kind::Rerank => {
                    rerank.0 += 1;
                    rerank.1 += r.wt;
                }
                Kind::Offer => {}
            }
        }
        let mut out: Vec<String> = by
            .into_iter()
            .map(|(server, (loaded, used, calls))| {
                format!(
                    "{server}: {loaded} wt of levels loaded, {used} wt of them used; {calls} wt of answers"
                )
            })
            .collect();
        if rerank.0 > 0 {
            out.push(format!("rerank: {} calls, {} wt", rerank.0, rerank.1));
        }
        out
    }

    /// The last ask's offers, as `status` shows them: `<tool> — <why>`, with the creature that
    /// asked; and its rerank, when one ran.
    #[must_use]
    pub fn last_offers(&self) -> Vec<String> {
        let Some(ask) = self
            .rows
            .iter()
            .rev()
            .find(|r| matches!(r.kind, Kind::Offer | Kind::Rerank))
            .map(|r| r.ask.clone())
        else {
            return Vec::new();
        };
        self.rows
            .iter()
            .filter(|r| r.ask == ask)
            .filter_map(|r| match r.kind {
                Kind::Offer => Some(format!(
                    "{} offered {} — {}{}",
                    r.creature,
                    r.tool.as_deref().unwrap_or_default(),
                    r.why.as_deref().unwrap_or_default(),
                    if r.used { " · called" } else { "" }
                )),
                Kind::Rerank => Some(format!("{} rerank {} — {} wt", r.creature, r.outcome, r.wt)),
                _ => None,
            })
            .collect()
    }
}
