//! The races themselves: who they are, whom they answer to, whom they command.

use std::fmt;

/// Which class of model rides a race when configuration names none for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelTier {
    /// The most capable model configured.
    Frontier,
    /// A strong, cheaper model.
    Strong,
    /// The fastest model.
    Fast,
}

/// A race: six base races and five ascended forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Race {
    /// The throne: understands the human, plans, dispatches, answers. Never writes code.
    Rimuru,
    /// The lane's planner and voice: plans one slice across domains, routes to its Orcs.
    Elf,
    /// The domain and its gate: writes the test first, judges every landing.
    Orc,
    /// The ground truth of a zone: the only race that changes code.
    Slime,
    /// The hands that reach outside: the only race whose shelf carries `outward`.
    Kijin,
    /// The adversary: attacks the code, and never claims what it cannot prove.
    DarkElf,
    /// An Elf that ascended: orders work in other lanes through their Elf.
    HighElf,
    /// An Orc that ascended: may amend, never weaken, its Slimes' verify lines mid-turn.
    HighOrc,
    /// A Slime that ascended: its own verify additions run the turn they are written.
    NamedSlime,
    /// A Kijin that ascended: may act under envelopes the human signed in advance.
    Oni,
    /// A Dark Elf that ascended: its seal proposals go straight to the human.
    ArchDarkElf,
}

impl Race {
    /// Every race, base races first.
    pub const ALL: [Race; 11] = [
        Race::Rimuru,
        Race::Elf,
        Race::Orc,
        Race::Slime,
        Race::Kijin,
        Race::DarkElf,
        Race::HighElf,
        Race::HighOrc,
        Race::NamedSlime,
        Race::Oni,
        Race::ArchDarkElf,
    ];

    /// The race's name as creatures carry it: `slime`, `high-orc`, `dark-elf`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Race::Rimuru => "rimuru",
            Race::Elf => "elf",
            Race::Orc => "orc",
            Race::Slime => "slime",
            Race::Kijin => "kijin",
            Race::DarkElf => "dark-elf",
            Race::HighElf => "high-elf",
            Race::HighOrc => "high-orc",
            Race::NamedSlime => "named-slime",
            Race::Oni => "oni",
            Race::ArchDarkElf => "arch-dark-elf",
        }
    }

    /// Reads a race name as [`Race::name`] writes it.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Race> {
        Race::ALL.into_iter().find(|r| r.name() == name)
    }

    /// The base race of an ascended form; a base race is its own base.
    #[must_use]
    pub fn base(self) -> Race {
        match self {
            Race::HighElf => Race::Elf,
            Race::HighOrc => Race::Orc,
            Race::NamedSlime => Race::Slime,
            Race::Oni => Race::Kijin,
            Race::ArchDarkElf => Race::DarkElf,
            base => base,
        }
    }

    /// The form this race ascends to, if any. The throne does not ascend.
    #[must_use]
    pub fn ascends_to(self) -> Option<Race> {
        match self {
            Race::Elf => Some(Race::HighElf),
            Race::Orc => Some(Race::HighOrc),
            Race::Slime => Some(Race::NamedSlime),
            Race::Kijin => Some(Race::Oni),
            Race::DarkElf => Some(Race::ArchDarkElf),
            _ => None,
        }
    }

    /// Whether this is an ascended form.
    #[must_use]
    pub fn is_ascended(self) -> bool {
        self.base() != self
    }

    /// The race this one reports to, by base race. The throne reports to the human.
    #[must_use]
    pub fn reports_to(self) -> Option<Race> {
        match self.base() {
            Race::Rimuru => None,
            Race::Orc => Some(Race::Elf),
            Race::Slime => Some(Race::Orc),
            _ => Some(Race::Rimuru),
        }
    }

    /// The base races this race may commission directly. An ascended form commands what its
    /// base commands; a commanded race includes its ascended forms.
    #[must_use]
    pub fn commands(self) -> &'static [Race] {
        match self.base() {
            Race::Rimuru => &[Race::Elf, Race::Kijin, Race::DarkElf],
            Race::Elf => &[Race::Orc],
            Race::Orc => &[Race::Slime],
            _ => &[],
        }
    }

    /// Whether this race may commission another of its own race (Orc ⇄ Orc).
    #[must_use]
    pub fn sideways(self) -> bool {
        self.base() == Race::Orc
    }

    /// The model tier that rides this race when configuration names no model for it.
    #[must_use]
    pub fn default_tier(self) -> ModelTier {
        match self.base() {
            Race::Rimuru | Race::Elf | Race::DarkElf => ModelTier::Frontier,
            _ => ModelTier::Strong,
        }
    }

    /// Steps a body of this race may take in one commission. Passed down whole, never split.
    #[must_use]
    pub fn steps(self) -> u32 {
        match self {
            Race::NamedSlime => 60,
            r => match r.base() {
                Race::Elf | Race::Orc => 30,
                Race::Kijin => 20,
                _ => 40,
            },
        }
    }

    /// The largest share, in percent, of what its parent has left when it carves, that a body of
    /// this race may be given. A ceiling, never a default; the throne holds the whole session.
    #[must_use]
    pub fn max_share(self) -> u8 {
        match self.base() {
            Race::Elf | Race::Orc => 60,
            Race::Slime => 70,
            Race::Kijin => 30,
            Race::DarkElf => 40,
            _ => 100,
        }
    }
}

impl fmt::Display for Race {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for r in Race::ALL {
            assert_eq!(Race::from_name(r.name()), Some(r));
            assert_eq!(r.to_string(), r.name());
        }
        assert_eq!(Race::from_name("veldora"), None);
    }

    #[test]
    fn ascension_pairs() {
        let pairs: Vec<_> = Race::ALL
            .into_iter()
            .filter_map(|r| r.ascends_to().map(|a| (r, a)))
            .collect();
        assert_eq!(pairs.len(), 5);
        for (base, risen) in pairs {
            assert_eq!(risen.base(), base);
            assert!(risen.is_ascended() && !base.is_ascended());
            assert_eq!(risen.ascends_to(), None, "no second ascension");
            assert_eq!(risen.reports_to(), base.reports_to());
            assert_eq!(risen.commands(), base.commands());
            assert_eq!(risen.max_share(), base.max_share());
            assert_eq!(risen.default_tier(), base.default_tier());
        }
        assert_eq!(Race::Rimuru.ascends_to(), None);
    }

    #[test]
    fn the_tree_reaches_the_throne() {
        for r in Race::ALL {
            let mut at = r;
            let mut hops = 0;
            while let Some(up) = at.reports_to() {
                at = up;
                hops += 1;
                assert!(hops <= 3, "{r} does not reach the throne");
            }
            assert_eq!(at, Race::Rimuru);
            if let Some(up) = r.reports_to() {
                assert!(up.commands().contains(&r.base()), "{up} must command {r}");
            }
        }
    }

    #[test]
    fn steps_and_shares_follow_the_book() {
        let steps: Vec<_> = Race::ALL.iter().map(|r| r.steps()).collect();
        assert_eq!(steps, [40, 30, 30, 40, 20, 40, 30, 30, 60, 20, 40]);
        let shares: Vec<_> = Race::ALL.iter().map(|r| r.max_share()).collect();
        assert_eq!(shares, [100, 60, 60, 70, 30, 40, 60, 60, 70, 30, 40]);
        assert!(
            Race::ALL
                .iter()
                .all(|r| !r.sideways() || r.base() == Race::Orc)
        );
    }
}
