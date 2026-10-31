//! Caps across sessions: the month, the day and the session, read from the journal.

use crate::{Timestamp, UsageRow};

/// The share of a cap at which the human is warned.
const WARN_PCT: u64 = 80;

/// The limits, in weighted tokens. `None` is no limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caps {
    /// This calendar month (UTC).
    pub month: Option<u64>,
    /// Today (UTC).
    pub day: Option<u64>,
    /// This session.
    pub session: Option<u64>,
}

/// The worst reading across the caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapReading {
    /// Under every warning line.
    Ok,
    /// At or past 80% of a cap: tell the human.
    Warn {
        /// Which cap.
        cap: &'static str,
        /// Weighted tokens used against it.
        used: u64,
        /// The cap.
        of: u64,
    },
    /// At or past a cap: no new commissions; the current turn finishes its report.
    Stop {
        /// Which cap.
        cap: &'static str,
        /// Weighted tokens used against it.
        used: u64,
        /// The cap.
        of: u64,
    },
}

/// The weight a call the ledger cannot read is taken to have spent, when no call it can read is
/// larger.
pub const UNSEEN_FLOOR_WT: u64 = 20_000;

/// The weight assumed for `lines` ledger lines that cannot be read — a torn write after a crash,
/// a hand edit. Each is taken to be as large as the largest call the ledger can read, and never
/// less than [`UNSEEN_FLOOR_WT`]: spend the caps cannot see is counted high, never skipped.
#[must_use]
pub fn unseen_wt(rows: &[UsageRow], lines: usize) -> u64 {
    let each = rows
        .iter()
        .map(|r| r.wt)
        .max()
        .unwrap_or(0)
        .max(UNSEEN_FLOOR_WT);
    each.saturating_mul(u64::try_from(lines).unwrap_or(u64::MAX))
}

/// The share of every cap held back, out of sight, for spend the ledger cannot read.
pub const UNSEEN_RESERVE_PCT: u64 = 15;

/// What `unseen` weighs against a cap of `of`: never more than [`UNSEEN_RESERVE_PCT`] of it, so
/// a line the ledger cannot read is counted high but never stops a session by itself.
#[must_use]
pub fn unseen_against(unseen: u64, of: u64) -> u64 {
    let reserve =
        u64::try_from(u128::from(of) * u128::from(UNSEEN_RESERVE_PCT) / 100).unwrap_or(of);
    unseen.min(reserve)
}

/// Reads the caps against the journal's rows at `now`, for `session`. `unseen` — the weight
/// assumed for lines that cannot be read ([`unseen_wt`]) — counts against every cap, since a
/// line that cannot be read cannot say its day or its session, and against each only as far as
/// its hidden reserve ([`unseen_against`]).
#[must_use]
pub fn check_caps(
    rows: &[UsageRow],
    session: &str,
    now: Timestamp,
    caps: Caps,
    unseen: u64,
) -> CapReading {
    let today = now.day();
    let used = |pick: &dyn Fn(&UsageRow) -> bool| {
        rows.iter()
            .filter(|r| pick(r))
            .fold(0_u64, |sum, r| sum.saturating_add(r.wt))
    };
    let readings = [
        (
            "month",
            caps.month,
            used(&|r| {
                let d = r.ts.day();
                (d.year, d.month) == (today.year, today.month)
            }),
        ),
        ("day", caps.day, used(&|r| r.ts.day() == today)),
        ("session", caps.session, used(&|r| r.session == session)),
    ];
    let mut worst = CapReading::Ok;
    for (cap, limit, used) in readings {
        let Some(of) = limit else { continue };
        let used = used.saturating_add(unseen_against(unseen, of));
        if used >= of {
            return CapReading::Stop { cap, used, of };
        }
        if worst == CapReading::Ok
            && u128::from(used) * 100 >= u128::from(of) * u128::from(WARN_PCT)
        {
            worst = CapReading::Warn { cap, used, of };
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line the ledger cannot read is charged as its largest call, against every cap.
    #[test]
    fn unseen_lines_count_high_against_every_cap() {
        assert_eq!(unseen_wt(&[], 2), 2 * UNSEEN_FLOOR_WT);
        // A readable call larger than the floor sets the weight of each unseen line.
        assert_eq!(unseen_wt(&[row(NOW, "s", 50_000)], 2), 100_000);
        assert!(matches!(
            check_caps(
                &[row(NOW, "s", 17_000)],
                "s",
                Timestamp(NOW),
                Caps {
                    day: Some(UNSEEN_FLOOR_WT),
                    ..Caps::default()
                },
                unseen_wt(&[], 1),
            ),
            CapReading::Stop {
                cap: "day",
                used: 20_000,
                ..
            }
        ));
        // Against a cap, unseen spend weighs at most the hidden reserve: alone, it never stops.
        let caps = Caps {
            session: Some(100_000),
            ..Caps::default()
        };
        assert_eq!(unseen_against(unseen_wt(&[], 1), 100_000), 15_000);
        assert_eq!(unseen_against(1_000, 100_000), 1_000);
        assert_eq!(
            check_caps(&[], "s", Timestamp(NOW), caps, unseen_wt(&[], 9)),
            CapReading::Ok
        );
        // With real spend, the reserve is part of what is used: 70 000 + 15 000 passes 80%.
        assert_eq!(
            check_caps(
                &[row(NOW, "s", 70_000)],
                "s",
                Timestamp(NOW),
                caps,
                unseen_wt(&[], 1)
            ),
            CapReading::Warn {
                cap: "session",
                used: 85_000,
                of: 100_000
            }
        );
    }
    use crate::Usage;

    fn row(ts: i64, session: &str, wt: u64) -> UsageRow {
        UsageRow {
            ts: Timestamp(ts),
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
            envelope: None,
            purpose: None,
        }
    }

    const NOW: i64 = 1_792_782_420; // 2026-10-23T19:07:00Z
    const DAY: i64 = 86_400;

    #[test]
    fn each_cap_reads_its_own_window() {
        let rows = [
            row(NOW - 40 * DAY, "old", 1_000), // last month
            row(NOW - 2 * DAY, "s0", 300),     // this month, another day
            row(NOW - 60, "s1", 500),          // today, another session
            row(NOW - 10, "s2", 100),          // today, this session
        ];
        let now = Timestamp(NOW);
        assert_eq!(
            check_caps(&rows, "s2", now, Caps::default(), 0),
            CapReading::Ok
        );
        assert_eq!(
            check_caps(
                &rows,
                "s2",
                now,
                Caps {
                    month: Some(900),
                    ..Caps::default()
                },
                0
            ),
            CapReading::Stop {
                cap: "month",
                used: 900,
                of: 900
            }
        );
        assert_eq!(
            check_caps(
                &rows,
                "s2",
                now,
                Caps {
                    day: Some(700),
                    ..Caps::default()
                },
                0
            ),
            CapReading::Warn {
                cap: "day",
                used: 600,
                of: 700
            }
        );
        assert_eq!(
            check_caps(
                &rows,
                "s2",
                now,
                Caps {
                    session: Some(1_000),
                    ..Caps::default()
                },
                0
            ),
            CapReading::Ok
        );
        assert_eq!(
            check_caps(
                &rows,
                "s2",
                now,
                Caps {
                    session: Some(120),
                    ..Caps::default()
                },
                0
            ),
            CapReading::Warn {
                cap: "session",
                used: 100,
                of: 120
            }
        );
    }

    #[test]
    fn a_stop_beats_a_warning() {
        let rows = [row(NOW, "s", 100)];
        let caps = Caps {
            month: Some(110),
            day: Some(100),
            session: None,
        };
        assert_eq!(
            check_caps(&rows, "s", Timestamp(NOW), caps, 0),
            CapReading::Stop {
                cap: "day",
                used: 100,
                of: 100
            }
        );
    }
}
