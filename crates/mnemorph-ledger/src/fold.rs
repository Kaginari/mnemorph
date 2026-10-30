//! Folds over the usage journal: totals by session, body, race, model or day.

use std::collections::BTreeMap;

use crate::{Usage, UsageRow};

/// What to group by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// The session.
    Session,
    /// The creature.
    Body,
    /// The race.
    Race,
    /// The model.
    Model,
    /// The UTC day.
    Day,
}

/// Totals over a group of calls.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Totals {
    /// Calls.
    pub calls: u64,
    /// Tokens, by class.
    pub usage: Usage,
    /// Weighted tokens.
    pub wt: u64,
    /// US dollars over the priced calls.
    pub usd: f64,
    /// Calls on unpriced models: `usd` does not cover them.
    pub unpriced: u64,
    /// Tokens of the fixed per-session prefix.
    pub prefix: u64,
}

impl Totals {
    /// The share of input served from the cache, `0.0..=1.0`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn cache_hit_rate(&self) -> f64 {
        let all = self.usage.input + self.usage.cache_read + self.usage.cache_write;
        if all == 0 {
            0.0
        } else {
            self.usage.cache_read as f64 / all as f64
        }
    }

    /// The share of input that was the fixed prefix, `0.0..=1.0`: what the law and the cards
    /// cost before any work.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn prefix_share(&self) -> f64 {
        let all = self.usage.input + self.usage.cache_read + self.usage.cache_write;
        if all == 0 {
            0.0
        } else {
            (self.prefix as f64 / all as f64).min(1.0)
        }
    }

    fn add(&mut self, r: &UsageRow) {
        self.calls += 1;
        self.usage = self.usage + r.usage;
        self.wt += r.wt;
        self.prefix += r.prefix;
        match r.usd {
            Some(usd) => self.usd += usd,
            None => self.unpriced += 1,
        }
    }
}

/// Totals of `rows` grouped by `key`, in key order.
#[must_use]
pub fn fold(rows: &[UsageRow], key: Key) -> BTreeMap<String, Totals> {
    let mut out: BTreeMap<String, Totals> = BTreeMap::new();
    for r in rows {
        let k = match key {
            Key::Session => r.session.clone(),
            Key::Body => r.body.clone(),
            Key::Race => r.race.clone(),
            Key::Model => r.model.clone(),
            Key::Day => r.ts.day().to_string(),
        };
        out.entry(k).or_default().add(r);
    }
    out
}
