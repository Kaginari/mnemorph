//! Shelves, soil and gates: what each race may do, where it may write, and who judges it.

use std::fmt;

use crate::Race;

/// The class of a tool call. The shell classifier yields the first four; `Dispatch` and `Seal`
/// are harness tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolClass {
    /// Look inside the world.
    Read,
    /// Change files inside the world, deletes and renames in the writer's own soil included.
    Write,
    /// Anything that leaves the machine.
    Outward,
    /// Irreversible: history rewrites, force and tag pushes, publishes, foreign deletes.
    Destructive,
    /// Commission another body.
    Dispatch,
    /// Propose freezing landings on a territory.
    Seal,
}

impl fmt::Display for ToolClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ToolClass::Read => "read",
            ToolClass::Write => "write",
            ToolClass::Outward => "outward",
            ToolClass::Destructive => "destructive",
            ToolClass::Dispatch => "dispatch",
            ToolClass::Seal => "seal",
        })
    }
}

/// A kind of ground a race may write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Soil {
    /// The creature's own doc.
    OwnDoc,
    /// A Slime's zone: directory prefixes.
    Zone,
    /// The verify block of its Slimes' docs (an Orc's).
    VerifyBlock,
    /// A Kijin's post: CI, release, deploy or PR files.
    Post,
    /// The Dark Elf's proving ground, `.tempest/dark-elf/`.
    ProvingGround,
    /// The throne's plan and the dimension manifest.
    Throne,
}

/// Who judges a race's writes at the end gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gate {
    /// Its writes are judged by the verify lines its Orc wrote.
    ByItsOrc,
    /// It holds the gate over its Slimes.
    OverItsSlimes,
    /// It holds its own gate over its own soil.
    OwnSoil,
    /// It writes nothing but its own doc; the gate checks only that doc.
    DocOnly,
}

impl Race {
    /// The tool classes this race may use. Anything else is refused before it runs.
    #[must_use]
    pub fn shelf(self) -> &'static [ToolClass] {
        use ToolClass::{Destructive, Dispatch, Outward, Read, Seal, Write};
        match self {
            Race::Rimuru | Race::Elf | Race::HighElf | Race::Orc | Race::HighOrc => {
                &[Read, Write, Dispatch]
            }
            Race::Slime | Race::NamedSlime => &[Read, Write],
            Race::Kijin | Race::Oni => &[Read, Write, Outward, Destructive],
            Race::DarkElf | Race::ArchDarkElf => &[Read, Write, Seal],
        }
    }

    /// The soil this race may write.
    #[must_use]
    pub fn soil(self) -> &'static [Soil] {
        match self {
            Race::Rimuru => &[Soil::Throne],
            Race::Elf | Race::HighElf => &[Soil::OwnDoc],
            Race::Orc | Race::HighOrc => &[Soil::OwnDoc, Soil::VerifyBlock],
            Race::Slime | Race::NamedSlime => &[Soil::OwnDoc, Soil::Zone],
            Race::Kijin | Race::Oni => &[Soil::OwnDoc, Soil::Post],
            Race::DarkElf | Race::ArchDarkElf => &[Soil::OwnDoc, Soil::ProvingGround],
        }
    }

    /// Who judges this race's writes.
    #[must_use]
    pub fn gate(self) -> Gate {
        match self.base() {
            Race::Slime => Gate::ByItsOrc,
            Race::Orc => Gate::OverItsSlimes,
            Race::Rimuru | Race::Kijin | Race::DarkElf => Gate::OwnSoil,
            _ => Gate::DocOnly,
        }
    }

    /// Whether this race may write code: only a Slime, in its zone.
    #[must_use]
    pub fn writes_code(self) -> bool {
        self.soil().contains(&Soil::Zone)
    }
}

/// Checks a tool class against a race's shelf. A refusal names the lawful route.
///
/// # Errors
///
/// The route to take when `class` is not on `race`'s shelf.
pub fn check_class(race: Race, class: ToolClass) -> Result<(), ShelfRefusal> {
    if race.shelf().contains(&class) {
        Ok(())
    } else {
        Err(ShelfRefusal { race, class })
    }
}

/// A tool call refused because its class is not on the race's shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShelfRefusal {
    /// The race that asked.
    pub race: Race,
    /// The class it lacks.
    pub class: ToolClass,
}

impl fmt::Display for ShelfRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is not on the {} shelf: {}",
            self.class,
            self.race,
            self.class.route()
        )
    }
}

/// A Kijin's post: the five ways the world reaches outside, compiled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Post {
    /// CI: reads runs and logs, re-runs jobs, publishes CI as an instrument.
    Ci,
    /// Release: builds artifacts; tag pushes and registry publishes (both destructive).
    Release,
    /// Deploy: deploys, verifies health, rolls back.
    Deploy,
    /// Forge: pushes branches, opens and updates pull requests, reads issues.
    Forge,
    /// Scout: keeps the advisory mirror and watches upstream releases.
    Scout,
}

impl Post {
    /// Every post.
    pub const ALL: [Post; 5] = [
        Post::Ci,
        Post::Release,
        Post::Deploy,
        Post::Forge,
        Post::Scout,
    ];

    /// The creature name of the post: `kijin-ci`, …
    #[must_use]
    pub fn creature(self) -> &'static str {
        match self {
            Post::Ci => "kijin-ci",
            Post::Release => "kijin-release",
            Post::Deploy => "kijin-deploy",
            Post::Forge => "kijin-forge",
            Post::Scout => "kijin-scout",
        }
    }
}

impl ToolClass {
    /// The lawful route for a body whose shelf lacks this class.
    #[must_use]
    pub fn route(self) -> &'static str {
        match self {
            ToolClass::Read | ToolClass::Write => "stay inside your own soil",
            ToolClass::Outward => {
                "route to a kijin post: kijin-ci, kijin-release, kijin-deploy, kijin-forge or kijin-scout"
            }
            ToolClass::Destructive => "route to a kijin; it always passes the human gate",
            ToolClass::Dispatch => "report up with @? — your parent dispatches",
            ToolClass::Seal => "report the finding to rimuru; a dark elf proposes seals",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_kijin_reach_outside() {
        for r in Race::ALL {
            let outward = r.shelf().contains(&ToolClass::Outward);
            assert_eq!(outward, r.base() == Race::Kijin, "{r}");
            assert_eq!(r.shelf().contains(&ToolClass::Destructive), outward, "{r}");
        }
    }

    #[test]
    fn only_slimes_write_code() {
        for r in Race::ALL {
            assert_eq!(r.writes_code(), r.base() == Race::Slime, "{r}");
        }
    }

    #[test]
    fn dispatch_follows_command() {
        for r in Race::ALL {
            assert_eq!(
                r.shelf().contains(&ToolClass::Dispatch),
                !r.commands().is_empty(),
                "{r}"
            );
        }
    }

    #[test]
    fn gates_and_soil() {
        assert_eq!(Race::Slime.gate(), Gate::ByItsOrc);
        assert_eq!(Race::HighOrc.gate(), Gate::OverItsSlimes);
        assert_eq!(Race::Oni.gate(), Gate::OwnSoil);
        assert_eq!(Race::ArchDarkElf.gate(), Gate::OwnSoil);
        assert_eq!(Race::Rimuru.gate(), Gate::OwnSoil);
        assert_eq!(Race::HighElf.gate(), Gate::DocOnly);
        assert_eq!(Race::Rimuru.soil(), [Soil::Throne]);
        assert!(Race::Orc.soil().contains(&Soil::VerifyBlock));
        assert!(Race::DarkElf.soil().contains(&Soil::ProvingGround));
        assert!(Race::Kijin.soil().contains(&Soil::Post));
    }

    #[test]
    fn refusals_name_the_route() {
        assert!(check_class(Race::Slime, ToolClass::Write).is_ok());
        let e = check_class(Race::Slime, ToolClass::Outward).unwrap_err();
        assert_eq!((e.race, e.class), (Race::Slime, ToolClass::Outward));
        let text = e.to_string();
        for name in Post::ALL.map(Post::creature) {
            assert!(text.contains(name), "{text} names {name}");
        }
        assert!(
            check_class(Race::Orc, ToolClass::Destructive)
                .unwrap_err()
                .to_string()
                .contains("human gate")
        );
        assert!(
            check_class(Race::Slime, ToolClass::Dispatch)
                .unwrap_err()
                .to_string()
                .contains("@?")
        );
        assert!(
            check_class(Race::Elf, ToolClass::Seal)
                .unwrap_err()
                .to_string()
                .contains("dark elf")
        );
        for class in [
            ToolClass::Read,
            ToolClass::Write,
            ToolClass::Outward,
            ToolClass::Destructive,
            ToolClass::Dispatch,
            ToolClass::Seal,
        ] {
            assert_ne!(class.to_string(), "");
            assert_ne!(class.route(), "");
        }
    }
}
