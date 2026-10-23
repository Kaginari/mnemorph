//! The usage row: one model call, as the journal keeps it.

use serde::{Deserialize, Serialize};

use crate::{Prices, Timestamp, Usage, wt};

/// The envelope of the body that made the call, after the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeReading {
    /// Weighted tokens left.
    pub left: u64,
    /// The envelope's size.
    pub of: u64,
}

/// One model call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    /// When the call ended.
    pub ts: Timestamp,
    /// The session.
    pub session: String,
    /// The run (one body's commission).
    pub run: String,
    /// The creature whose body made the call.
    pub body: String,
    /// Its race, by name.
    pub race: String,
    /// The commission id, when the body was commissioned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commission: Option<String>,
    /// The model id.
    pub model: String,
    /// The provider.
    pub provider: String,
    /// The tokens, by class.
    #[serde(flatten)]
    pub usage: Usage,
    /// Of the input, the tokens of the fixed per-session prefix (law, race card, creature card,
    /// manifests) — the cost every turn carries before it does any work.
    #[serde(default)]
    pub prefix: u64,
    /// Weighted tokens.
    pub wt: u64,
    /// US dollars, or `null` when the model is unpriced — never a made-up zero.
    pub usd: Option<f64>,
    /// The envelope after the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope: Option<EnvelopeReading>,
}

impl UsageRow {
    /// Prices a call: fills `wt` and `usd` from `prices`.
    #[must_use]
    pub fn priced(mut self, prices: &Prices) -> Self {
        self.wt = wt(self.usage, prices.output_ratio(&self.model));
        self.usd = prices.usd(&self.model, self.usage);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> UsageRow {
        UsageRow {
            ts: Timestamp(1_792_782_420),
            session: "s1".into(),
            run: "r1".into(),
            body: "slime-wire".into(),
            race: "slime".into(),
            commission: Some("c-17".into()),
            model: "model-a".into(),
            provider: "anthropic".into(),
            usage: Usage {
                input: 5580,
                output: 112,
                cache_read: 4096,
                cache_write: 0,
            },
            prefix: 3900,
            wt: 0,
            usd: None,
            envelope: Some(EnvelopeReading {
                left: 54_888,
                of: 60_000,
            }),
        }
    }

    #[test]
    fn the_book_shape() {
        let json = serde_json::to_value(row()).unwrap();
        for key in [
            "ts",
            "session",
            "run",
            "body",
            "race",
            "commission",
            "model",
            "provider",
            "input",
            "output",
            "cacheRead",
            "cacheWrite",
            "prefix",
            "wt",
            "usd",
            "envelope",
        ] {
            assert!(json.get(key).is_some(), "{key} missing in {json}");
        }
        assert_eq!(json["usd"], serde_json::Value::Null, "unpriced is null");
        let back: UsageRow = serde_json::from_value(json).unwrap();
        assert_eq!(back, row());
    }

    #[test]
    fn priced_and_unpriced() {
        let prices = Prices::parse("[models.\"model-a\"]\ninput = 3.0\noutput = 15.0").unwrap();
        let r = row().priced(&prices);
        assert_eq!(r.wt, wt(r.usage, 5.0));
        assert!(r.usd.is_some_and(|u| u > 0.0));
        let unpriced = UsageRow {
            model: "model-z".into(),
            ..row()
        }
        .priced(&prices);
        assert_eq!(unpriced.usd, None);
        assert_eq!(unpriced.wt, wt(unpriced.usage, 5.0));
    }
}
