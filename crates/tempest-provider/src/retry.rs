//! When to try again, and how long to wait: the retry ladder and the three timeouts.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Retries and timeouts for one provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// Attempts after the first.
    pub max_retries: u32,
    /// The first backoff; it doubles per attempt.
    pub base: Duration,
    /// The longest single wait, `retry-after` included.
    pub cap: Duration,
    /// Connecting.
    pub connect: Duration,
    /// From the request to the first byte of the body — generous: a model may queue or prefill
    /// for a long time before it says anything.
    pub first_byte: Duration,
    /// Between two chunks once the body has started.
    pub stall: Duration,
    /// The whole call, every attempt and wait included: past it, the call ends.
    pub turn: Duration,
}

impl Default for Policy {
    fn default() -> Policy {
        Policy {
            max_retries: 3,
            base: Duration::from_secs(1),
            cap: Duration::from_secs(60),
            connect: Duration::from_secs(15),
            first_byte: Duration::from_secs(600),
            stall: Duration::from_secs(120),
            turn: Duration::from_mins(30),
        }
    }
}

/// Statuses worth another attempt: timeouts, conflicts, rate limits, server errors, overload.
#[must_use]
pub fn retryable(status: u16) -> bool {
    matches!(status, 408 | 409 | 429 | 500 | 502 | 503 | 504 | 529)
}

impl Policy {
    /// The wait before attempt `attempt` (1 for the first retry). The server's `retry-after`
    /// wins when it gives one, whole (the call's deadline decides whether it is worth waiting);
    /// otherwise an exponential backoff with full jitter, never past `cap`.
    #[must_use]
    pub fn wait(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        if let Some(after) = retry_after {
            return after;
        }
        let ceiling = self
            .base
            .saturating_mul(1u32 << attempt.saturating_sub(1).min(16))
            .min(self.cap);
        // Full jitter from the clock's nanoseconds: enough to spread a herd, no dependency.
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        ceiling.mul_f64(f64::from(nanos % 1_000_000) / 1_000_000.0)
    }
}

/// Reads `retry-after-ms` or `retry-after` (seconds) from response headers.
#[must_use]
pub fn retry_after(ms: Option<&str>, secs: Option<&str>) -> Option<Duration> {
    if let Some(ms) = ms
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
    {
        return Some(Duration::from_secs_f64(ms / 1000.0));
    }
    secs.and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(Duration::from_secs_f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses() {
        for s in [408, 409, 429, 500, 502, 503, 504, 529] {
            assert!(retryable(s), "{s}");
        }
        for s in [200, 400, 401, 402, 403, 404, 413, 422] {
            assert!(!retryable(s), "{s}");
        }
    }

    #[test]
    fn waits() {
        let p = Policy::default();
        assert_eq!(
            p.wait(3, Some(Duration::from_secs(7))),
            Duration::from_secs(7)
        );
        assert_eq!(
            p.wait(1, Some(Duration::from_secs(600))),
            Duration::from_secs(600),
            "a server's hint is honoured whole; the deadline decides"
        );
        for attempt in 1..10 {
            let w = p.wait(attempt, None);
            let ceiling = p.base * (1 << (attempt - 1));
            assert!(w <= ceiling.min(p.cap), "attempt {attempt}: {w:?}");
        }
        assert!(p.wait(40, None) <= p.cap);
    }

    #[test]
    fn retry_after_headers() {
        assert_eq!(
            retry_after(Some("1500"), Some("9")),
            Some(Duration::from_millis(1500))
        );
        assert_eq!(retry_after(None, Some("2")), Some(Duration::from_secs(2)));
        assert_eq!(
            retry_after(Some("x"), Some("0.5")),
            Some(Duration::from_millis(500))
        );
        assert_eq!(retry_after(None, Some("-1")), None);
        assert_eq!(
            retry_after(None, Some("Wed, 21 Oct 2026 07:28:00 GMT")),
            None
        );
        assert_eq!(retry_after(None, None), None);
    }
}
