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
/// is the model's output-to-input price ratio. Computed in integers (the ratio to a thousandth)
/// and rounded up, so a call is never free and the same call always weighs the same.
#[must_use]
pub fn wt(u: Usage, output_ratio: f64) -> u64 {
    // Thousandths throughout: 1 000 per input token, 1 250 per cache write, 100 per cache read.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ratio = if output_ratio.is_finite() {
        (output_ratio.max(0.0) * 1000.0).round() as u128
    } else {
        0
    };
    let thousandths = 1000 * u128::from(u.input)
        + 1250 * u128::from(u.cache_write)
        + 100 * u128::from(u.cache_read)
        + ratio * u128::from(u.output);
    u64::try_from(thousandths.div_ceil(1000)).unwrap_or(u64::MAX)
}

/// A call's weight before it is made, for pre-flight: the cached prefix at the cache-read weight,
/// the rest of the input uncached, the whole `max_output` at the model's ratio. One
/// implementation, so the loop never re-derives it.
#[must_use]
pub fn estimate(cached_prefix: u64, rest: u64, max_output: u64, output_ratio: f64) -> u64 {
    wt(
        Usage {
            input: rest,
            output: max_output,
            cache_read: cached_prefix,
            cache_write: 0,
        },
        output_ratio,
    )
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
                1.1
            ),
            11,
            "no float round-up"
        );
        assert_eq!(
            wt(
                Usage {
                    output: 100,
                    ..Usage::default()
                },
                0.07
            ),
            7
        );
        assert_eq!(
            wt(
                Usage {
                    output: 1,
                    ..Usage::default()
                },
                f64::NAN
            ),
            0
        );
        assert_eq!(estimate(10_000, 500, 1_000, 5.0), 1_000 + 500 + 5_000);
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
