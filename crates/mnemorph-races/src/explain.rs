//! Why a rise, a probation or a fall is proposed: one line per bar, typed, so the decision can be
//! read back and recomputed.
//!
//! Every bar the race law reads is a meter held against a threshold by an operator. An
//! [`Explanation`] names the meter, its reading, the operator, the threshold and the record it was
//! read over; for a fall, the bar that would end probation and the rise bar a fallen body must earn
//! again; and, once a ledger fold fills them in, the ledger's head digest and the first and last
//! row of the window it read. [`Explanation::holds`] is the decision itself: the reading held
//! against the threshold, as the law holds it.

use std::fmt::{self, Write as _};

/// How a reading is held against its threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Strictly under.
    Lt,
    /// At or under.
    Le,
    /// At or over.
    Ge,
    /// Strictly over.
    Gt,
}

impl Op {
    /// Whether `reading` stands to `threshold` as the operator says.
    #[must_use]
    pub fn holds(self, reading: f64, threshold: f64) -> bool {
        match self {
            Op::Lt => reading < threshold,
            Op::Le => reading <= threshold,
            Op::Ge => reading >= threshold,
            Op::Gt => reading > threshold,
        }
    }

    /// `<`, `<=`, `>=` or `>`.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Ge => ">=",
            Op::Gt => ">",
        }
    }
}

/// What a line explains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One bar of a rise: every bar of the form has to hold.
    Rise,
    /// A rate under its fall bar on a full recent record: probation.
    Probation,
    /// An event that ends the ascension at once.
    Fall,
}

impl Kind {
    /// `rise`, `probation` or `fall`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Kind::Rise => "rise",
            Kind::Probation => "probation",
            Kind::Fall => "fall",
        }
    }
}

/// How a reading is kept and shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// A percentage, `0.0..=100.0`, shown as a proportion: `71.0` reads `0.71`.
    Rate,
    /// A count, shown whole.
    Count,
}

impl Scale {
    fn show(self, v: f64) -> String {
        match self {
            Scale::Rate => proportion(v),
            Scale::Count => format!("{v:.0}"),
        }
    }
}

/// A percentage as a proportion: two decimals, more when the reading needs them to differ from its
/// bar (`69.95` reads `0.6995`, never `0.70`).
fn proportion(pct: f64) -> String {
    let mut s = format!("{:.4}", pct / 100.0);
    while s.ends_with('0') && s.len() > s.find('.').map_or(0, |d| d + 3) {
        s.pop();
    }
    s
}

/// The bar a body on probation must reach again, on the same meter, and over how much record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recover {
    /// How the meter is held against the bar.
    pub op: Op,
    /// The bar, a percentage.
    pub threshold: f64,
    /// The record it has to recover within.
    pub within: u32,
    /// The record's unit: reports, findings or outward acts.
    pub unit: &'static str,
}

/// The rise bar a fallen body must earn again: a rate, over a record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rerise {
    /// The meter, as the law names it.
    pub meter: &'static str,
    /// How the meter is held against the bar.
    pub op: Op,
    /// The bar, a percentage.
    pub threshold: f64,
    /// The record a rise reads.
    pub over: u32,
    /// The record's unit.
    pub unit: &'static str,
}

/// One bar of a decision, explained.
///
/// ```text
/// probation: landRate 0.71 < 0.75 (last 10 reports); recover: >= 0.75 within 5 reports;
///   re-rise: landRate >= 0.90 over 20 reports; ledger head 3f…9a, rows 12–48
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Explanation {
    /// A rise, a probation or a fall.
    pub kind: Kind,
    /// The meter, as the law names it: `landRate`, `precision`, `rollbacks`.
    pub meter: &'static str,
    /// How its reading is kept and shown.
    pub scale: Scale,
    /// The reading: a percentage for a rate, a count for a count.
    pub reading: f64,
    /// How the reading is held against the threshold.
    pub op: Op,
    /// The bar.
    pub threshold: f64,
    /// The record the reading was taken over: the last ten for a fall, the window for a rise.
    pub window: u32,
    /// The record's unit: reports, findings or outward acts.
    pub unit: &'static str,
    /// What ends probation: a probation's alone.
    pub recover: Option<Recover>,
    /// The rise bar a fallen body must earn again: a probation's and a fall's.
    pub rerise: Option<Rerise>,
    /// The ledger's head digest when the reading was taken; `None` when no ledger was read or it
    /// carries no chain.
    pub head: Option<String>,
    /// The first row of the window the ledger fold read.
    pub from: Option<u64>,
    /// The last row of the window the ledger fold read.
    pub to: Option<u64>,
}

impl Explanation {
    /// The decision: the reading held against the threshold, as the law holds it.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.op.holds(self.reading, self.threshold)
    }

    /// The line, with the ledger it was read from: its head digest and the first and last row
    /// of the window.
    #[must_use]
    pub fn on_ledger(self, head: Option<&str>, window: Option<(u64, u64)>) -> Explanation {
        Explanation {
            head: head.map(str::to_owned),
            from: window.map(|w| w.0),
            to: window.map(|w| w.1),
            ..self
        }
    }
}

impl fmt::Display for Explanation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let span = if self.kind == Kind::Rise {
            "over"
        } else {
            "last"
        };
        let mut line = format!(
            "{}: {} {} {} {} ({span} {} {})",
            self.kind.word(),
            self.meter,
            self.scale.show(self.reading),
            self.op.symbol(),
            self.scale.show(self.threshold),
            self.window,
            self.unit
        );
        if let Some(r) = &self.recover {
            let _ = write!(
                line,
                "; recover: {} {} within {} {}",
                r.op.symbol(),
                proportion(r.threshold),
                r.within,
                r.unit
            );
        } else if self.kind == Kind::Fall {
            line.push_str("; recover: none, it falls at once");
        }
        if let Some(r) = &self.rerise {
            let _ = write!(
                line,
                "; re-rise: {} {} {} over {} {}",
                r.meter,
                r.op.symbol(),
                proportion(r.threshold),
                r.over,
                r.unit
            );
        }
        if self.head.is_some() || self.from.is_some() {
            match &self.head {
                Some(h) => {
                    let _ = write!(line, "; ledger head {h}");
                }
                None => line.push_str("; ledger unchained"),
            }
            if let (Some(a), Some(b)) = (self.from, self.to) {
                let _ = write!(line, ", rows {a}–{b}");
            }
        }
        f.write_str(&line)
    }
}
