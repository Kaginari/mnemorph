//! The commission: what a dispatcher sends down.

use std::fmt;

use crate::{report::one_line, tag};

/// The answer ceiling, in bytes, when a commission names none.
pub const DEFAULT_CAP: usize = 2048;

/// The office an `@ASK` word selects. Each has a name in the world — the three first Muses,
/// daughters of Mnemosyne, memory: **Mneme** reads (memory), **Melete** judges (deliberation),
/// **Aoede** writes (voice) — and a function a configuration may name instead: read, judge, write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Office {
    /// `findings` — gather facts.
    Findings,
    /// `verdict` — weigh a claim.
    Verdict,
    /// `draft` — write.
    Draft,
}

impl Office {
    /// The `@ASK` word for this office.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Office::Findings => "findings",
            Office::Verdict => "verdict",
            Office::Draft => "draft",
        }
    }

    /// The office's name in the world: `mneme`, `melete` or `aoede`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Office::Findings => "mneme",
            Office::Verdict => "melete",
            Office::Draft => "aoede",
        }
    }

    /// What the office does: `read`, `judge` or `write`.
    #[must_use]
    pub fn function(self) -> &'static str {
        match self {
            Office::Findings => "read",
            Office::Verdict => "judge",
            Office::Draft => "write",
        }
    }

    /// Every office, in the order a commission's work flows: read, judge, write.
    pub const ALL: [Office; 3] = [Office::Findings, Office::Verdict, Office::Draft];

    fn from_word(word: &str) -> Option<Self> {
        match word {
            "findings" => Some(Office::Findings),
            "verdict" => Some(Office::Verdict),
            "draft" => Some(Office::Draft),
            _ => None,
        }
    }
}

/// What a commission asks for: one of the three offices, or free words.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Ask {
    /// No `@ASK` line.
    #[default]
    None,
    /// An office word.
    Office(Office),
    /// Words that name no office, kept as written.
    Free(String),
}

/// What a dispatcher sends down.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Commission {
    /// `@ROOT` — the territory pin.
    pub root: Option<String>,
    /// `@SCOPE` — what is in scope.
    pub scope: Option<String>,
    /// `@ASK` — the office.
    pub ask: Ask,
    /// `+unsaid` on the `@ASK` line: surface the unsaid.
    pub unsaid: bool,
    /// `wire:raw` on the `@ASK` line: the pipe-dense form is welcome.
    pub raw: bool,
    /// `@CAP` — the answer ceiling in bytes.
    pub cap: Option<usize>,
    /// `@BUDGET` — the carved slice of the parent's envelope, in weighted tokens.
    pub budget: Option<u64>,
    /// `@DUMP` — where overflow goes.
    pub dump: Option<String>,
    /// `@SIZE` — a draft's payload budget.
    pub size: Option<usize>,
    /// Lines that are not tags: the ask in prose.
    pub body: Vec<String>,
}

impl Commission {
    /// Reads a commission. Unrecognised lines, and tag lines whose value does not parse, are
    /// kept in [`Commission::body`] — nothing a dispatcher wrote is lost.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut c = Commission::default();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let Some((t, rest)) = tag::split(line, false) else {
                c.body.push(line.to_owned());
                continue;
            };
            let known = match t {
                "ROOT" => {
                    c.root = Some(rest);
                    true
                }
                "SCOPE" => {
                    c.scope = Some(rest);
                    true
                }
                "ASK" => {
                    c.read_ask(&rest);
                    true
                }
                // `@CAP 0` would read as "no ceiling": it is refused like any bad value.
                "CAP" => rest
                    .parse()
                    .ok()
                    .filter(|&n| n > 0)
                    .map(|n| c.cap = Some(n))
                    .is_some(),
                "BUDGET" => parse_budget(&rest).map(|n| c.budget = Some(n)).is_some(),
                "DUMP" => {
                    c.dump = Some(rest);
                    true
                }
                "SIZE" => rest.parse().map(|n| c.size = Some(n)).is_ok(),
                _ => false,
            };
            if !known {
                c.body.push(line.to_owned());
            }
        }
        c
    }

    fn read_ask(&mut self, rest: &str) {
        let mut words = Vec::new();
        for w in rest.split_whitespace() {
            match w {
                "+unsaid" => self.unsaid = true,
                "wire:raw" => self.raw = true,
                _ => words.push(w),
            }
        }
        self.ask = match words.as_slice() {
            [] => Ask::None,
            [one] => {
                Office::from_word(one).map_or_else(|| Ask::Free((*one).to_owned()), Ask::Office)
            }
            many => Ask::Free(many.join(" ")),
        };
    }

    /// The ceiling in force.
    #[must_use]
    pub fn cap_or_default(&self) -> usize {
        self.cap.unwrap_or(DEFAULT_CAP)
    }
}

impl fmt::Display for Commission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(root) = &self.root {
            writeln!(f, "@ROOT {}", one_line(root))?;
        }
        if let Some(scope) = &self.scope {
            writeln!(f, "@SCOPE {}", one_line(scope))?;
        }
        let mut ask = match &self.ask {
            Ask::None => String::new(),
            Ask::Office(o) => o.word().to_owned(),
            Ask::Free(words) => words.clone(),
        };
        if self.unsaid {
            ask.push_str(" +unsaid");
        }
        if self.raw {
            ask.push_str(" wire:raw");
        }
        if !ask.trim().is_empty() {
            writeln!(f, "@ASK {}", one_line(ask.trim()))?;
        }
        if let Some(cap) = self.cap {
            writeln!(f, "@CAP {cap}")?;
        }
        if let Some(budget) = self.budget {
            writeln!(f, "@BUDGET {budget} wt")?;
        }
        if let Some(dump) = &self.dump {
            writeln!(f, "@DUMP {}", one_line(dump))?;
        }
        if let Some(size) = self.size {
            writeln!(f, "@SIZE {size}")?;
        }
        for line in &self.body {
            writeln!(f, "{}", one_line(line))?;
        }
        Ok(())
    }
}

/// Reads a budget in weighted tokens: `60000`, `60000 wt`, `60k`, `60k wt`, `1.5M wt`.
#[must_use]
pub fn parse_budget(text: &str) -> Option<u64> {
    let text = text.trim();
    let number = text.strip_suffix("wt").map_or(text, str::trim_end);
    let scale: u32 = match number.chars().last()? {
        'k' | 'K' => 1_000,
        'm' | 'M' => 1_000_000,
        _ => return number.parse().ok(),
    };
    let value: f64 = number[..number.len() - 1].parse().ok()?;
    let scaled = (value * f64::from(scale)).round();
    // f64 holds every integer up to 2^53 exactly; past that, or negative, it is not a budget.
    if !(0.0..=9_007_199_254_740_992.0).contains(&scaled) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(scaled as u64)
}

#[cfg(test)]
mod tests {
    #[test]
    fn each_office_has_a_word_a_name_and_a_function() {
        let rows: Vec<_> = super::Office::ALL
            .iter()
            .map(|o| (o.word(), o.name(), o.function()))
            .collect();
        assert_eq!(
            rows,
            [
                ("findings", "mneme", "read"),
                ("verdict", "melete", "judge"),
                ("draft", "aoede", "write"),
            ]
        );
    }

    use super::*;

    #[test]
    fn reads_every_tag() {
        let c = Commission::parse(
            "@ROOT /w/x\n@SCOPE pkgs a, b\n@ASK draft +unsaid wire:raw\n@CAP 512\n\
             @BUDGET 60k wt\n@DUMP .mnemorph/tmp/out.md\n@SIZE 4000\nBuild the thing.\n\nSecond line.",
        );
        assert_eq!(c.root.as_deref(), Some("/w/x"));
        assert_eq!(c.scope.as_deref(), Some("pkgs a, b"));
        assert_eq!(c.ask, Ask::Office(Office::Draft));
        assert!(c.unsaid && c.raw);
        assert_eq!(c.cap, Some(512));
        assert_eq!(c.budget, Some(60_000));
        assert_eq!(c.dump.as_deref(), Some(".mnemorph/tmp/out.md"));
        assert_eq!(c.size, Some(4000));
        assert_eq!(c.body, ["Build the thing.", "Second line."]);
        assert_eq!(Commission::parse(&c.to_string()), c);
    }

    #[test]
    fn defaults_and_free_asks() {
        assert_eq!(
            Commission::parse("just prose").cap_or_default(),
            DEFAULT_CAP
        );
        let c = Commission::parse("@ASK look around the gate");
        assert_eq!(c.ask, Ask::Free("look around the gate".into()));
        assert_eq!(Commission::parse(&c.to_string()), c);
    }

    #[test]
    fn a_bad_value_is_kept_not_lost() {
        let c = Commission::parse("@CAP lots\n@BUDGET plenty\n@NOPE x\n@CAP 0");
        assert_eq!(c.cap, None);
        assert_eq!(c.budget, None);
        assert_eq!(c.body, ["@CAP lots", "@BUDGET plenty", "@NOPE x", "@CAP 0"]);
        assert_eq!(
            c.cap_or_default(),
            DEFAULT_CAP,
            "@CAP 0 never means no ceiling"
        );
    }

    #[test]
    fn budgets() {
        assert_eq!(parse_budget("60000"), Some(60_000));
        assert_eq!(parse_budget("60000 wt"), Some(60_000));
        assert_eq!(parse_budget("200k wt"), Some(200_000));
        assert_eq!(parse_budget("1.5M wt"), Some(1_500_000));
        assert_eq!(parse_budget("2K"), Some(2_000));
        assert_eq!(parse_budget(""), None);
        assert_eq!(parse_budget("wt"), None);
        assert_eq!(parse_budget("-1k"), None);
        assert_eq!(parse_budget("NaNk"), None);
        assert_eq!(parse_budget("1e300M"), None);
    }
}
