//! `@FORGE` drafts: a repeating need a run proved, riding the report up to be curated.

use std::fmt;

use crate::{fnv, tag};

/// What a draft would become.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForgeKind {
    /// A tool.
    Tool,
    /// A Mind (skill).
    Skill,
}

impl ForgeKind {
    fn word(self) -> &'static str {
        match self {
            ForgeKind::Tool => "tool",
            ForgeKind::Skill => "skill",
        }
    }
}

/// One `@FORGE <kind>:<name> | <text>` draft. The wire never mints a draft; it carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forge {
    /// Tool or skill.
    pub kind: ForgeKind,
    /// The draft's name.
    pub name: String,
    /// What it does and how; may be empty.
    pub text: String,
}

impl Forge {
    /// Reads one `<kind>:<name> | <text>` payload. The pipe part may be absent; only the first
    /// pipe splits, so the text may hold pipes of its own.
    #[must_use]
    pub fn parse(payload: &str) -> Option<Self> {
        let (kind, rest) = payload.split_once(':')?;
        let kind = match kind.trim().to_ascii_lowercase().as_str() {
            "tool" => ForgeKind::Tool,
            "skill" => ForgeKind::Skill,
            _ => return None,
        };
        let (name, text) = rest.split_once('|').unwrap_or((rest, ""));
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        Some(Forge {
            kind,
            name: name.to_owned(),
            text: text.trim().to_owned(),
        })
    }

    /// The draft's content address: the first ten hex digits of an FNV-1a hash over kind, name
    /// and text with runs of whitespace folded, so a relayed line hashes the same at every hop
    /// and the first filer wins.
    #[must_use]
    pub fn hash(&self) -> String {
        let folded = self.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let key = format!("{}:{}|{folded}", self.kind.word(), self.name);
        format!("{:016x}", fnv::fnv64a(key.as_bytes()))[..10].to_owned()
    }
}

impl fmt::Display for Forge {
    /// The payload of the wire line: `<kind>:<name>` or `<kind>:<name> | <text>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.word(), self.name)?;
        if !self.text.is_empty() {
            write!(f, " | {}", self.text)?;
        }
        Ok(())
    }
}

/// Reads the `@FORGE` lines out of any text — a wire report or a prose answer (the throne
/// speaks prose; its drafts still count).
#[must_use]
pub fn forges_of(text: &str) -> Vec<Forge> {
    text.lines()
        .filter_map(|l| tag::split(l, false))
        .filter(|(t, _)| *t == "FORGE")
        .filter_map(|(_, rest)| Forge::parse(&rest))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints() {
        let f = Forge::parse("tool:ui-wrangler | rewrite bare /v1 strings | python3 w.py").unwrap();
        assert_eq!(f.kind, ForgeKind::Tool);
        assert_eq!(f.name, "ui-wrangler");
        assert_eq!(f.text, "rewrite bare /v1 strings | python3 w.py");
        assert_eq!(Forge::parse(&f.to_string()), Some(f));
        let s = Forge::parse("Skill:ttl-craft").unwrap();
        assert_eq!((s.kind, s.text.as_str()), (ForgeKind::Skill, ""));
        assert_eq!(s.to_string(), "skill:ttl-craft");
        assert_eq!(Forge::parse("nonsense"), None);
        assert_eq!(Forge::parse("widget:x"), None);
        assert_eq!(Forge::parse("tool: | text"), None);
    }

    #[test]
    fn hash_folds_whitespace() {
        let a = Forge::parse("tool:x | one  two\tthree").unwrap();
        let b = Forge::parse("tool:x | one two three").unwrap();
        assert_eq!(a.hash(), b.hash());
        assert_eq!(a.hash().len(), 10);
        assert_ne!(
            a.hash(),
            Forge::parse("skill:x | one two three").unwrap().hash()
        );
    }

    #[test]
    fn drafts_in_prose() {
        let found = forges_of("I learned this.\n@FORGE skill:gate-reading | read the gate\nbye");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "gate-reading");
    }
}
