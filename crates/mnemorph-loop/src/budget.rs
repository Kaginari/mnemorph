//! The human's caps, read against what this world already spent: the budget a session opens
//! with. Every cap is in weighted tokens, which count every call — priced, unpriced or free — so
//! no model slips under one.

use mnemorph_ledger::{Day, UsageRow};

/// The caps the human set, in weighted tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Caps {
    /// The most one session may spend.
    pub session: Option<u64>,
    /// The most a UTC day may spend, in this world, across sessions.
    pub day: Option<u64>,
    /// The most a UTC month may spend, in this world, across sessions.
    pub month: Option<u64>,
}

/// What a session may spend, and the cap that sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Left {
    /// Weighted tokens the session may spend.
    pub wt: u64,
    /// `session`, `day` or `month`: the tightest.
    pub cap: &'static str,
}

/// The budget left under `caps` on `today`, given what `rows` already spent; `None` when the
/// human set no cap.
#[must_use]
pub fn left(caps: Caps, rows: &[UsageRow], today: Day) -> Option<Left> {
    let spent = |same: &dyn Fn(Day) -> bool| -> u64 {
        rows.iter().filter(|r| same(r.ts.day())).map(|r| r.wt).sum()
    };
    let day = caps
        .day
        .map(|cap| cap.saturating_sub(spent(&|d| d == today)));
    let month = caps
        .month
        .map(|cap| cap.saturating_sub(spent(&|d| d.year == today.year && d.month == today.month)));
    [("session", caps.session), ("day", day), ("month", month)]
        .into_iter()
        .filter_map(|(cap, wt)| wt.map(|wt| Left { wt, cap }))
        .min_by_key(|l| l.wt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemorph_ledger::{Timestamp, Usage};

    fn row(ts: &str, wt: u64) -> UsageRow {
        UsageRow {
            ts: Timestamp::parse(ts).unwrap(),
            session: "s".into(),
            run: "r".into(),
            body: "rimuru".into(),
            race: "rimuru".into(),
            commission: None,
            model: "m".into(),
            provider: "p".into(),
            usage: Usage::default(),
            prefix: 0,
            wt,
            usd: None,
            envelope: None,
        }
    }

    #[test]
    fn the_tightest_cap_sets_the_budget_and_counts_every_call() {
        let today = Day::new(2026, 10, 24).unwrap();
        let rows = [
            row("2026-10-24T08:00:00Z", 30_000),
            row("2026-10-23T23:59:59Z", 50_000),
            row("2026-09-30T12:00:00Z", 900_000),
        ];
        assert_eq!(
            left(Caps::default(), &rows, today),
            None,
            "no cap, no budget"
        );
        let caps = Caps {
            session: Some(100_000),
            day: Some(80_000),
            month: Some(1_000_000),
        };
        // Today spent 30k of 80k; October 80k of 1M; the session may spend 100k: the day binds.
        assert_eq!(
            left(caps, &rows, today),
            Some(Left {
                wt: 50_000,
                cap: "day"
            })
        );
        let caps = Caps {
            month: Some(70_000),
            ..caps
        };
        assert_eq!(
            left(caps, &rows, today),
            Some(Left {
                wt: 0,
                cap: "month"
            }),
            "spent is spent"
        );
    }
}
