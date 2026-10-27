//! `@?` holes: what a body could not settle. Four grammars ride holes; anything else is prose.

use std::fmt;

/// One `@?` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hole {
    /// `@CAP <cap>: <n> lines cut — full report at <dump>`: the emitter cut the report.
    Cut {
        /// The ceiling in force.
        cap: usize,
        /// Lines dropped, across every cut this report has been through.
        dropped: usize,
        /// Where the full report lies, if anywhere.
        dump: Option<String>,
    },
    /// `contest #<id>: <reason>`: an Orc contests a Dark Elf's finding.
    Contest {
        /// The finding's id, without `#`.
        finding: String,
        /// Why.
        reason: String,
    },
    /// `budget: need ~<n> wt, have <m> wt`: the next call does not fit the envelope.
    Budget {
        /// Weighted tokens the next call needs, estimated.
        need: u64,
        /// Weighted tokens left.
        have: u64,
    },
    /// `nv #<fingerprint> blocker:<fact> plan:<check>`: a Dark Elf's finding that needs
    /// validation — no severity, never a finding, never a seal.
    Nv {
        /// The root-cause fingerprint, without `#`: ten lowercase hex digits.
        fingerprint: String,
        /// The exact fact that blocks the proof; it ends at its first word opening `plan:`.
        blocker: String,
        /// The check that would settle it; the last field, it runs to the line's end.
        plan: String,
    },
    /// Anything else, in the body's own words.
    Other(String),
}

impl Hole {
    /// Reads a hole's payload; text that matches no grammar is [`Hole::Other`].
    #[must_use]
    pub fn parse(text: &str) -> Hole {
        Self::cut(text)
            .or_else(|| Self::contest(text))
            .or_else(|| Self::budget(text))
            .or_else(|| Self::nv(text))
            .unwrap_or_else(|| Hole::Other(text.to_owned()))
    }

    fn cut(text: &str) -> Option<Hole> {
        let rest = text.strip_prefix("@CAP ")?;
        let (cap, rest) = rest.split_once(": ")?;
        let (dropped, rest) = rest.split_once(' ')?;
        let dump = match rest {
            "line cut" | "lines cut" => None,
            _ => Some(
                rest.strip_prefix("line cut — full report at ")
                    .or_else(|| rest.strip_prefix("lines cut — full report at "))?
                    .to_owned(),
            ),
        };
        Some(Hole::Cut {
            cap: cap.parse().ok()?,
            dropped: dropped.parse().ok()?,
            dump,
        })
    }

    fn contest(text: &str) -> Option<Hole> {
        let rest = text.strip_prefix("contest #")?;
        let (finding, reason) = rest.split_once(": ")?;
        if finding.is_empty() || finding.contains(char::is_whitespace) {
            return None;
        }
        Some(Hole::Contest {
            finding: finding.to_owned(),
            reason: reason.to_owned(),
        })
    }

    /// The exact grammar, nothing between its fields: a severity anywhere before `plan:` makes
    /// the line a hole of its own.
    fn nv(text: &str) -> Option<Hole> {
        let rest = text.strip_prefix("nv #")?;
        let (fingerprint, rest) = rest.split_once(' ')?;
        let rest = rest.strip_prefix("blocker:")?;
        let at = plan_at(rest)?;
        let (blocker, plan) = (rest[..at].trim(), rest[at + "plan:".len()..].trim());
        let fingerprint_ok = fingerprint.len() == 10
            && fingerprint
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        let severity = blocker
            .split_whitespace()
            .any(|w| w.to_ascii_lowercase().starts_with("severity:"));
        if !fingerprint_ok || blocker.is_empty() || plan.is_empty() || severity {
            return None;
        }
        Some(Hole::Nv {
            fingerprint: fingerprint.to_owned(),
            blocker: blocker.to_owned(),
            plan: plan.to_owned(),
        })
    }

    fn budget(text: &str) -> Option<Hole> {
        let rest = text.strip_prefix("budget: need ~")?;
        let (need, rest) = rest.split_once(" wt, have ")?;
        let have = rest.strip_suffix(" wt")?;
        Some(Hole::Budget {
            need: need.parse().ok()?,
            have: have.parse().ok()?,
        })
    }
}

impl fmt::Display for Hole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hole::Cut { cap, dropped, dump } => {
                let plural = if *dropped == 1 { "" } else { "s" };
                write!(f, "@CAP {cap}: {dropped} line{plural} cut")?;
                if let Some(dump) = dump {
                    write!(f, " — full report at {dump}")?;
                }
                Ok(())
            }
            Hole::Contest { finding, reason } => write!(f, "contest #{finding}: {reason}"),
            Hole::Budget { need, have } => write!(f, "budget: need ~{need} wt, have {have} wt"),
            Hole::Nv {
                fingerprint,
                blocker,
                plan,
            } => write!(f, "nv #{fingerprint} blocker:{blocker} plan:{plan}"),
            Hole::Other(text) => f.write_str(text),
        }
    }
}

/// Where the first whitespace-led word opening `plan:` starts in `text`, at its `p`.
fn plan_at(text: &str) -> Option<usize> {
    text.char_indices()
        .filter(|&(i, c)| c.is_whitespace() && text[i + c.len_utf8()..].starts_with("plan:"))
        .map(|(i, c)| i + c.len_utf8())
        .next()
}

#[cfg(test)]
mod tests {
    use super::Hole;

    #[test]
    fn grammars_round_trip() {
        for hole in [
            Hole::Cut {
                cap: 2048,
                dropped: 1,
                dump: None,
            },
            Hole::Cut {
                cap: 512,
                dropped: 12,
                dump: Some(".mnemorph/tmp/r.md".into()),
            },
            Hole::Contest {
                finding: "a1".into(),
                reason: "the input is trusted here".into(),
            },
            Hole::Budget {
                need: 12_000,
                have: 3_500,
            },
            Hole::Other("what about x".into()),
        ] {
            assert_eq!(Hole::parse(&hole.to_string()), hole);
        }
    }

    #[test]
    fn near_misses_are_prose() {
        for text in [
            "@CAP lots: 1 line cut",
            "@CAP 10: some lines cut",
            "@CAP 10: 2 lines vanished",
            "@CAP 10 no colon",
            "contest #: empty id",
            "contest #a b: spaced id",
            "contest #a no colon",
            "budget: need ~x wt, have 1 wt",
            "budget: need ~1 wt, have y wt",
            "budget: need ~1 wt, have 2",
            "budget: need ~1, have 2 wt",
        ] {
            assert_eq!(Hole::parse(text), Hole::Other(text.into()), "{text}");
        }
    }
}
