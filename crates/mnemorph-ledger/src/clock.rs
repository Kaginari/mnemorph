//! UTC timestamps and days, without a date crate: the civil-calendar conversion is small enough
//! to own (Howard Hinnant's `days_from_civil` and its inverse).

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A UTC instant to the second, written `2026-10-23T19:07:00Z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub i64);

/// A UTC calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    /// The year.
    pub year: i64,
    /// The month, 1..=12.
    pub month: u8,
    /// The day of the month, 1..=31.
    pub day: u8,
}

impl Timestamp {
    /// Now, from the system clock.
    #[must_use]
    pub fn now() -> Timestamp {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        Timestamp(secs)
    }

    /// The UTC day this instant falls on.
    #[must_use]
    pub fn day(self) -> Day {
        civil(self.0.div_euclid(86_400))
    }

    /// Reads `YYYY-MM-DDTHH:MM:SSZ`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Timestamp> {
        let bytes = text.as_bytes();
        let shape = [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'Z'),
        ];
        if bytes.len() != 20 || shape.iter().any(|&(at, sep)| bytes[at] != sep) {
            return None;
        }
        // Every other position is a digit: no sign, no space, no `+202`.
        if (0..20).any(|at| !shape.iter().any(|&(s, _)| s == at) && !bytes[at].is_ascii_digit()) {
            return None;
        }
        let num = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
        let (year, month, mday) = (num(0..4)?, num(5..7)?, num(8..10)?);
        let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
        let date = Day {
            year,
            month: u8::try_from(month).ok()?,
            day: u8::try_from(mday).ok()?,
        };
        if !(1..=12).contains(&month)
            || mday < 1
            || mday > i64::from(days_in_month(year, date.month))
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        Some(Timestamp(
            days_from_civil(date) * 86_400 + hour * 3600 + minute * 60 + second,
        ))
    }
}

impl Day {
    /// The day, if the calendar has it: no 31 April, no 29 February in a common year.
    #[must_use]
    pub fn new(year: i64, month: u8, day: u8) -> Option<Day> {
        ((1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month))
            .then_some(Day { year, month, day })
    }

    /// The day `days` later, or earlier when `days` is negative.
    #[must_use]
    pub fn plus(self, days: i64) -> Day {
        civil(days_from_civil(self) + days)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.day();
        let secs = self.0.rem_euclid(86_400);
        write!(
            f,
            "{d}T{:02}:{:02}:{:02}Z",
            secs / 3600,
            secs % 3600 / 60,
            secs % 60
        )
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Timestamp::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("not a UTC timestamp: {text}")))
    }
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: u8) -> u8 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a civil date.
fn days_from_civil(d: Day) -> i64 {
    let (m, dd) = (i64::from(d.month), i64::from(d.day));
    let y = if m <= 2 { d.year - 1 } else { d.year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a count of days since 1970-01-01.
fn civil(days: i64) -> Day {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    // month in 1..=12 and day in 1..=31 by construction.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Day {
        year,
        month: month as u8,
        day: day as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants() {
        assert_eq!(Timestamp(0).to_string(), "1970-01-01T00:00:00Z");
        assert_eq!(Timestamp(951_782_400).to_string(), "2000-02-29T00:00:00Z");
        assert_eq!(Timestamp(1_792_782_420).to_string(), "2026-10-23T19:07:00Z");
        assert_eq!(Timestamp(-1).to_string(), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn round_trips_across_centuries() {
        for secs in (-5_000_000_000i64..10_000_000_000).step_by(86_399 * 37) {
            let t = Timestamp(secs);
            assert_eq!(Timestamp::parse(&t.to_string()), Some(t), "{t}");
        }
    }

    #[test]
    fn refuses_what_is_not_an_instant() {
        for bad in [
            "2026-10-23 19:07:00Z",
            "2026-10-23T19:07:00",
            "2026-13-01T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2026-10-23T24:00:00Z",
            "2026-10-23T23:60:00Z",
            "2026-10-23T23:59:60Z",
            "2026-1x-23T23:59:59Z",
            "+202-10-23T19:07:00Z",
            "2026-10-23T+9:07:00Z",
            "2026-10-23T 9:07:00Z",
            "-001-01-01T00:00:00Z",
            "",
        ] {
            assert_eq!(Timestamp::parse(bad), None, "{bad}");
        }
        assert!(Timestamp::parse("2024-02-29T00:00:00Z").is_some());
        assert!(Timestamp::parse("1900-02-29T00:00:00Z").is_none());
    }

    #[test]
    fn serde_and_now() {
        let t = Timestamp(1_792_782_420);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"2026-10-23T19:07:00Z\"");
        assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), t);
        assert!(serde_json::from_str::<Timestamp>("\"yesterday\"").is_err());
        assert!(Timestamp::now().0 > 1_700_000_000);
    }

    #[test]
    fn days_exist_only_in_the_calendar_and_count_across_months() {
        assert_eq!(Day::new(2026, 4, 31), None);
        assert_eq!(Day::new(2025, 2, 29), None);
        assert_eq!(Day::new(2026, 13, 1), None);
        assert_eq!(Day::new(2026, 1, 0), None);
        let signed = Day::new(2026, 10, 24).unwrap();
        assert_eq!(signed.plus(30).to_string(), "2026-11-23");
        assert_eq!(signed.plus(-24).to_string(), "2026-09-30");
        assert_eq!(
            Day::new(2028, 2, 28).unwrap().plus(1).to_string(),
            "2028-02-29"
        );
        assert_eq!(
            Day::new(2026, 12, 31).unwrap().plus(1).to_string(),
            "2027-01-01"
        );
        assert!(signed < signed.plus(1));
    }
}
