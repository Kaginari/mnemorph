//! Model prices, read from TOML. A model with no price is *unpriced*, never free.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use crate::Usage;

/// The output-to-input price ratio used when a model has no price.
pub const DEFAULT_OUTPUT_RATIO: f64 = 5.0;

/// One model's prices, in US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Price {
    /// Uncached input.
    pub input: f64,
    /// Output.
    pub output: f64,
    /// Cache reads. Required: the multiple differs by model (a tenth of input on some, a
    /// twentieth or a fortieth on the newest), so it is never assumed.
    pub cache_read: f64,
    /// Cache writes with the 5-minute lifetime Mnemorph uses. Required, like reads.
    pub cache_write: f64,
}

impl Price {
    /// The output-to-input ratio weighted tokens use.
    #[must_use]
    pub fn output_ratio(&self) -> f64 {
        if self.input > 0.0 && self.output > 0.0 {
            self.output / self.input
        } else {
            // A free or input-free price must not make output free in weighted tokens.
            DEFAULT_OUTPUT_RATIO
        }
    }

    /// What `u` cost, in US dollars.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn usd(&self, u: Usage) -> f64 {
        (u.input as f64 * self.input
            + u.output as f64 * self.output
            + u.cache_read as f64 * self.cache_read
            + u.cache_write as f64 * self.cache_write)
            / 1_000_000.0
    }
}

/// The price table: `[models."<model id>"]` entries.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prices {
    /// Prices by model id.
    #[serde(default)]
    pub models: BTreeMap<String, Price>,
}

/// A price table that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricesError(pub String);

impl fmt::Display for PricesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "price table: {}", self.0)
    }
}

impl std::error::Error for PricesError {}

impl Prices {
    /// Reads a price table. Unknown keys are errors: a misspelt field must not price at zero.
    ///
    /// # Errors
    ///
    /// The TOML does not parse, a field is missing or misspelt, or a price is negative.
    pub fn parse(text: &str) -> Result<Prices, PricesError> {
        let p: Prices = toml::from_str(text).map_err(|e| PricesError(e.to_string()))?;
        for (model, price) in &p.models {
            let all = [
                price.input,
                price.output,
                price.cache_read,
                price.cache_write,
            ];
            if all.into_iter().any(|v| !v.is_finite() || v < 0.0) {
                return Err(PricesError(format!(
                    "{model}: prices are finite and not negative"
                )));
            }
        }
        Ok(p)
    }

    /// Layers `over` on top of this table: its entries win, model by model — an entry replaces
    /// the whole row, so a config row states all four prices.
    #[must_use]
    pub fn overlay(mut self, over: Prices) -> Prices {
        self.models.extend(over.models);
        self
    }

    /// A model's price, if known.
    #[must_use]
    pub fn get(&self, model: &str) -> Option<&Price> {
        self.models.get(model)
    }

    /// The output ratio for a model: its own, or [`DEFAULT_OUTPUT_RATIO`] when unpriced.
    #[must_use]
    pub fn output_ratio(&self, model: &str) -> f64 {
        self.get(model)
            .map_or(DEFAULT_OUTPUT_RATIO, Price::output_ratio)
    }

    /// What a call cost in US dollars, or `None` when the model is unpriced.
    #[must_use]
    pub fn usd(&self, model: &str, u: Usage) -> Option<f64> {
        self.get(model).map(|p| p.usd(u))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = r#"
        [models."model-a"]
        input = 3.0
        output = 15.0
        cache-read = 0.3
        cache-write = 3.75

        [models."model-b"]
        input = 1.0
        output = 4.0
        cache-read = 0.05
        cache-write = 1.5
    "#;

    #[test]
    fn reads_and_prices() {
        let p = Prices::parse(TABLE).unwrap();
        let u = Usage {
            input: 1_000_000,
            output: 1_000_000,
            cache_read: 1_000_000,
            cache_write: 1_000_000,
        };
        let a = p.usd("model-a", u).unwrap();
        assert!((a - (3.0 + 15.0 + 0.3 + 3.75)).abs() < 1e-9, "{a}");
        let b = p.usd("model-b", u).unwrap();
        assert!((b - (1.0 + 4.0 + 0.05 + 1.5)).abs() < 1e-9, "{b}");
        assert!((p.output_ratio("model-a") - 5.0).abs() < 1e-9);
        assert!((p.output_ratio("model-b") - 4.0).abs() < 1e-9);
    }

    #[test]
    fn unpriced_is_none_not_zero() {
        let p = Prices::default();
        assert_eq!(
            p.usd(
                "anything",
                Usage {
                    input: 10,
                    ..Usage::default()
                }
            ),
            None
        );
        assert!((p.output_ratio("anything") - DEFAULT_OUTPUT_RATIO).abs() < 1e-9);
        let free = Prices::parse(
            "[models.free]\ninput = 0.0\noutput = 0.0\ncache-read = 0.0\ncache-write = 0.0\n[models.cheap-out]\ninput = 1.0\noutput = 0.0\ncache-read = 0.1\ncache-write = 1.25",
        )
        .unwrap();
        assert!(
            (free.output_ratio("cheap-out") - DEFAULT_OUTPUT_RATIO).abs() < 1e-9,
            "free output is still weighed"
        );
        assert!(
            (free.output_ratio("free") - DEFAULT_OUTPUT_RATIO).abs() < 1e-9,
            "a free model still weighs output"
        );
    }

    #[test]
    fn bad_tables_are_refused() {
        for bad in [
            "[models.x]\ninput = 1.0",
            "[models.x]\ninput = 1.0\noutput = 2.0",
            "[models.x]\ninput = 1.0\noutput = 2.0\ncache-read = 0.1\ncache-write = 1.0\ncache_red = 0.1",
            "[models.x]\ninput = -1.0\noutput = 2.0\ncache-read = 0.1\ncache-write = 1.0",
            "[models.x]\ninput = 1.0\noutput = nan\ncache-read = 0.1\ncache-write = 1.0",
            "price = 3",
        ] {
            let e = Prices::parse(bad).unwrap_err();
            assert!(e.to_string().starts_with("price table: "), "{bad}");
        }
    }

    #[test]
    fn overlay_wins_model_by_model() {
        let base = Prices::parse(TABLE).unwrap();
        let over = Prices::parse(
            "[models.\"model-a\"]\ninput = 2.0\noutput = 8.0\ncache-read = 0.2\ncache-write = 2.5",
        )
        .unwrap();
        let p = base.overlay(over);
        assert!((p.get("model-a").unwrap().input - 2.0).abs() < 1e-9);
        assert!(p.get("model-b").is_some());
    }
}
