//! The human's caps, read against what this world spent: what a session opens with, for the
//! client to show. The run itself rereads the ledger before every call (`check_caps`), so the
//! caps hold across the sessions that share a world. Every cap is in weighted tokens, which count
//! every call — priced, unpriced or free — so no model slips under one.

pub use mnemorph_ledger::Caps;
use mnemorph_ledger::{Timestamp, UsageRow};

/// What a session may spend, and the cap that sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Left {
    /// Weighted tokens the session may spend.
    pub wt: u64,
    /// `session`, `day` or `month`: the tightest.
    pub cap: &'static str,
}

/// The budget left under `caps` at `now` for `session`, given what `rows` already spent and the
/// weight assumed for lines the ledger cannot read (`unseen`, against every cap, as far as its
/// hidden reserve); `None` when the human set no cap.
#[must_use]
pub fn left(
    caps: Caps,
    rows: &[UsageRow],
    session: &str,
    now: Timestamp,
    unseen: u64,
) -> Option<Left> {
    let today = now.day();
    let spent = |pick: &dyn Fn(&UsageRow) -> bool| -> u64 {
        rows.iter()
            .filter(|r| pick(r))
            .fold(0, |sum, r| sum.saturating_add(r.wt))
    };
    let month = spent(&|r| {
        let d = r.ts.day();
        (d.year, d.month) == (today.year, today.month)
    });
    let day = spent(&|r| r.ts.day() == today);
    let this = spent(&|r| r.session == session);
    [
        ("session", caps.session, this),
        ("day", caps.day, day),
        ("month", caps.month, month),
    ]
    .into_iter()
    .filter_map(|(cap, limit, used)| {
        limit.map(|of| Left {
            wt: of.saturating_sub(used.saturating_add(mnemorph_ledger::unseen_against(unseen, of))),
            cap,
        })
    })
    .min_by_key(|l| l.wt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemorph_ledger::Usage;

    fn row(ts: &str, session: &str, wt: u64) -> UsageRow {
        UsageRow {
            ts: Timestamp::parse(ts).unwrap(),
            session: session.into(),
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
            reported_cost: None,
            envelope: None,
            purpose: None,
        }
    }

    #[test]
    fn the_tightest_cap_sets_the_budget_and_counts_every_call() {
        let now = Timestamp::parse("2026-10-24T12:00:00Z").unwrap();
        // Unseen spend comes out of the cap's hidden reserve, never more: nine torn lines leave
        // 85% of a session cap.
        let session = Caps {
            session: Some(100_000),
            ..Caps::default()
        };
        let nine = mnemorph_ledger::unseen_wt(&[], 9);
        assert_eq!(left(session, &[], "this", now, nine).unwrap().wt, 85_000);
        let rows = [
            row("2026-10-24T08:00:00Z", "other", 30_000),
            row("2026-10-23T23:59:59Z", "other", 50_000),
            row("2026-09-30T12:00:00Z", "other", 900_000),
            row("2026-10-24T09:00:00Z", "this", 10_000),
        ];
        assert_eq!(
            left(Caps::default(), &rows, "this", now, 0),
            None,
            "no cap, no budget"
        );
        let caps = Caps {
            session: Some(100_000),
            day: Some(80_000),
            month: Some(1_000_000),
        };
        // Today spent 40k of 80k; the session 10k of 100k; October 90k of 1M: the day binds.
        assert_eq!(
            left(caps, &rows, "this", now, 0),
            Some(Left {
                wt: 40_000,
                cap: "day"
            })
        );
        let caps = Caps {
            month: Some(70_000),
            ..caps
        };
        assert_eq!(
            left(caps, &rows, "this", now, 0),
            Some(Left {
                wt: 0,
                cap: "month"
            }),
            "spent is spent"
        );
        let huge = [
            row("2026-10-24T08:00:00Z", "x", u64::MAX),
            row("2026-10-24T08:00:01Z", "x", 5),
        ];
        assert_eq!(
            left(
                Caps {
                    day: Some(10),
                    ..Caps::default()
                },
                &huge,
                "this",
                now,
                0
            )
            .unwrap()
            .wt,
            0,
            "never wraps"
        );
    }
}
