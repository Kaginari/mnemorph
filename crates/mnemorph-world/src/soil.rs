//! A body's soil: what it may write, as the tool policy reads it — claims and the exclusions the
//! earlier rules of [`own`](crate::own::own) give away inside them. Owning by domain is not the
//! right to write, so an Orc's soil is its seat and scratch.

use mnemorph_races::Race;

use crate::doc::covers;
use crate::own::{Owner, Rule, WORKSPACE, explain};
use crate::roster::Roster;

/// What a body may write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Soil {
    /// Claims it writes: directories with their `/`, or files.
    pub write: Vec<String>,
    /// Claims inside them it does not: another's seat, the harness's soil, the human's files, a
    /// nested claim that ranks first.
    pub except: Vec<String>,
    /// Whether it writes the world's root files (the workspace).
    pub root_files: bool,
}

impl Soil {
    /// Whether the soil admits `path` (relative to the world, normalised): the longest claim
    /// that covers it decides, and on a tie an exclusion wins. A root file is admitted by
    /// `root_files` unless a claim excludes it.
    #[must_use]
    pub fn admits(&self, path: &str) -> bool {
        let best = |claims: &[String]| {
            claims
                .iter()
                .filter(|c| covers(c, path))
                .map(String::len)
                .max()
        };
        let root = (self.root_files && !path.contains('/')).then_some(0);
        match (best(&self.write).max(root), best(&self.except)) {
            (Some(w), Some(e)) => w > e,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

/// The soil `own` yields for the creature `name`; `None` when it is not a living creature.
#[must_use]
pub fn soil(roster: &Roster, name: &str) -> Option<Soil> {
    let c = roster.creatures.get(name).filter(|c| !c.retired)?;
    let mut write = vec![c.seat.clone(), format!(".mnemorph/tmp/{name}/")];
    match c.race.base() {
        Race::Slime => write.extend(c.zone.iter().cloned()),
        Race::Kijin => write.extend(c.post.iter().cloned()),
        _ => {}
    }
    // The compiled soils this creature holds: asked of `own`, so the two never disagree.
    for compiled in [
        ".mnemorph/plan.md",
        ".mnemorph/dimension.toml",
        ".mnemorph/dark-elf/",
        "tools/",
    ] {
        if held(roster, name, compiled) {
            write.push(compiled.to_owned());
        }
    }
    let root_files = name == WORKSPACE;
    // Every claim in the world that ranks before this creature's, inside its own claims.
    let mut anchors: Vec<String> = [
        ".mnemorph/log.md",
        ".mnemorph/shared/",
        ".mnemorph/instruments/",
        ".mnemorph/memory/",
        ".mnemorph/ontology/",
        ".mnemorph/index/",
        ".mnemorph/tmp/",
        ".mnemorph/law.md",
        ".mnemorph/config.toml",
        ".mnemorph/config.local.toml",
        ".mnemorph/plan.md",
        ".mnemorph/dimension.toml",
        ".mnemorph/dark-elf/",
        "tools/",
    ]
    .map(str::to_owned)
    .to_vec();
    for other in roster.living() {
        anchors.push(other.seat.clone());
        anchors.push(format!(".mnemorph/tmp/{}/", other.name));
        anchors.extend(other.zone.iter().cloned());
        anchors.extend(other.post.iter().cloned());
    }
    let mut except: Vec<String> = anchors
        .into_iter()
        .filter(|a| {
            write
                .iter()
                .any(|w| covers(w, a) || covers(w, a.trim_end_matches('/')))
        })
        .filter(|a| !held(roster, name, a))
        .collect();
    if root_files {
        // The records' own directory is no root file; root files a post claims are the post's.
        except.push(".mnemorph".to_owned());
        except.extend(
            roster
                .living()
                .flat_map(|o| o.post.iter())
                .filter(|p| !p.contains('/'))
                .cloned(),
        );
    }
    except.sort();
    except.dedup();
    write.sort();
    write.dedup();
    Some(Soil {
        write,
        except,
        root_files,
    })
}

/// Whether `name` writes `claim` by `own`'s answer: owned, and not by domain alone.
fn held(roster: &Roster, name: &str, claim: &str) -> bool {
    let o = explain(roster, claim);
    o.owner == Owner::Creature(name.to_owned()) && o.rule != Rule::Domain
}
