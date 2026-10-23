//! Token counts of one model call, and the two meters read from them.

use serde::{Deserialize, Serialize};

/// The tokens one model call used, by class. Every class is kept: a reader that drops the cache
/// classes reads a different, wrong number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Input tokens not served from the cache.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Input tokens read from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to the prompt cache.
    pub cache_write: u64,
}

impl std::ops::Add for Usage {
    type Output = Usage;
    fn add(self, o: Usage) -> Usage {
        Usage {
            input: self.input + o.input,
            output: self.output + o.output,
            cache_read: self.cache_read + o.cache_read,
            cache_write: self.cache_write + o.cache_write,
        }
    }
}

/// Weighted tokens: what a call costs, in input-token equivalents.
///
/// `wt = uncached input + 1.25 × cache write + 0.1 × cache read + ratio × output`, where `ratio`
/// is the model's output-to-input price ratio. Rounded up, so a call is never free.
#[must_use]
pub fn wt(u: Usage, output_ratio: f64) -> u64 {
    // Hundredths, in integers, for the fixed weights; the ratio is the one float.
    let fixed =
        100 * u128::from(u.input) + 125 * u128::from(u.cache_write) + 10 * u128::from(u.cache_read);
    #[allow(clippy::cast_precision_loss)]
    let out = (u.output as f64) * output_ratio.max(0.0) * 100.0;
    #[allow(clippy::cast_precision_loss)]
    let hundredths = fixed as f64 + out;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = (hundredths / 100.0).ceil() as u64;
    whole
}

/// Context occupancy: how full the window was at the last call — its whole input, cached or
/// not. A reading of one call, never a sum.
#[must_use]
pub fn occupancy(last: Usage) -> u64 {
    last.input + last.cache_read + last.cache_write
}

/// Spend: every class of every call, summed. The cumulative reading, never one call's.
#[must_use]
pub fn spend(calls: &[Usage]) -> Usage {
    calls.iter().copied().fold(Usage::default(), |a, b| a + b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_tokens() {
        let u = Usage {
            input: 1000,
            output: 100,
            cache_read: 10_000,
            cache_write: 400,
        };
        // 1000 + 1.25·400 + 0.1·10000 + 5·100 = 1000 + 500 + 1000 + 500
        assert_eq!(wt(u, 5.0), 3000);
        assert_eq!(
            wt(
                Usage {
                    cache_read: 1,
                    ..Usage::default()
                },
                5.0
            ),
            1,
            "never free"
        );
        assert_eq!(wt(Usage::default(), 5.0), 0);
        assert_eq!(
            wt(
                Usage {
                    output: 10,
                    ..Usage::default()
                },
                -3.0
            ),
            0,
            "a bad ratio is not a credit"
        );
    }

    #[test]
    fn occupancy_is_not_spend() {
        // A cached conversation: three calls, each re-reading a growing prefix.
        let calls = [
            Usage {
                input: 5000,
                output: 100,
                cache_read: 0,
                cache_write: 5000,
            },
            Usage {
                input: 200,
                output: 120,
                cache_read: 5000,
                cache_write: 200,
            },
            Usage {
                input: 150,
                output: 90,
                cache_read: 5200,
                cache_write: 150,
            },
        ];
        assert_eq!(occupancy(calls[2]), 150 + 5200 + 150);
        let total = spend(&calls);
        assert_eq!(total.cache_read, 10_200, "spend keeps every cache read");
        assert_eq!(total.cache_write, 5350);
        assert_eq!(total.input, 5350);
        assert_eq!(total.output, 310);
    }
}
