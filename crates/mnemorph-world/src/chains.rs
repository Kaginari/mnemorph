//! The world's chained journals, walked and anchored: the usage ledger and the Dark Elf's value
//! ledger (the challenge records) chain their rows by hash ([`mnemorph_ledger::chain`]), and
//! their heads are written into `log.md`, outside the files, so a chain is checked against a head
//! someone keeps. `mnemorphd housekeep --verify` walks and anchors them; the Dark Elf's law audit
//! calls [`verify`].
//!
//! Gate verdicts, seals, envelopes and Kijin posts are not chained yet: each is chained by opening
//! its journal with [`mnemorph_ledger::Journal::chained`] and naming it in [`journals`].

use std::path::{Path, PathBuf};

use mnemorph_ledger::chain::{self, Report};

use crate::log::Entry;
use crate::worth::USAGE;

/// One chained journal of a world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chained {
    /// Its name: `usage` or `value`.
    pub name: &'static str,
    /// Where it lives.
    pub path: PathBuf,
}

/// The chained journals of `world`: the usage ledger, and the value ledger when there is a home
/// to keep it under.
#[must_use]
pub fn journals(world: &Path) -> Vec<Chained> {
    let mut out = vec![Chained {
        name: "usage",
        path: world.join(USAGE),
    }];
    if let Some(path) = crate::challenge::records_of(world) {
        out.push(Chained {
            name: "value",
            path,
        });
    }
    out
}

/// Each chained journal of `world`, walked once: OK with its head, a truncation, or the first
/// broken link with its line; an error when it cannot be read.
#[must_use]
pub fn verify(world: &Path) -> Vec<(Chained, std::io::Result<Report>)> {
    journals(world)
        .into_iter()
        .map(|j| {
            let report = chain::verify(&j.path);
            (j, report)
        })
        .collect()
}

/// The lines that anchor `readings` in a `log.md` entry: one per journal, its name and its
/// reading — the head, whole. An ascension, probation or fall entry carries them too.
#[must_use]
pub fn anchor_lines(readings: &[(Chained, std::io::Result<Report>)]) -> Vec<(String, String)> {
    readings
        .iter()
        .map(|(j, r)| {
            let text = match r {
                Ok(r) => r.to_string(),
                Err(e) => format!("unreadable: {e}"),
            };
            (format!("{} chain", j.name), text)
        })
        .collect()
}

/// Writes `readings` into `world`'s `log.md` as one entry by `by`: the heads anchored outside the
/// files. Returns the entry's heading, `None` when the world keeps no journal.
///
/// # Errors
///
/// The journal cannot be written.
pub fn anchor(
    world: &Path,
    by: &str,
    readings: &[(Chained, std::io::Result<Report>)],
) -> std::io::Result<Option<String>> {
    crate::log::append_entry(
        world,
        &Entry {
            body: by.to_owned(),
            title: "chains: heads anchored".to_owned(),
            lines: anchor_lines(readings),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemorph_ledger::{Journal, Timestamp, Usage, UsageRow};

    fn usage_row(n: u64) -> UsageRow {
        UsageRow {
            ts: Timestamp(1_792_782_420 + i64::try_from(n).unwrap()),
            session: "s".into(),
            run: "r".into(),
            body: "slime".into(),
            race: "slime".into(),
            commission: None,
            model: "m".into(),
            provider: "p".into(),
            usage: Usage {
                input: n,
                ..Usage::default()
            },
            prefix: 0,
            wt: n,
            usd: None,
            envelope: None,
        }
    }

    #[test]
    fn the_head_anchored_in_log_md_matches_the_journal() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path();
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        let usage = Journal::<UsageRow>::chained(world.join(USAGE));
        for n in 0..3 {
            usage.append(&usage_row(n)).unwrap();
        }
        let readings = verify(world);
        assert_eq!(readings[0].0.name, "usage");
        let heading = anchor(world, "housekeep", &readings).unwrap().unwrap();
        assert!(heading.contains("housekeep — chains: heads anchored"));
        let log = std::fs::read_to_string(world.join(crate::log::LOG)).unwrap();
        let head = usage.head().unwrap().unwrap();
        let line = log
            .lines()
            .find(|l| l.starts_with("- **usage chain:**"))
            .unwrap();
        assert!(
            line.contains(&format!("OK, head {} at line 3", head.digest)),
            "{line}"
        );
        // A row removed from the end still holds as a chain; the anchored head does not match.
        let text = std::fs::read_to_string(usage.path()).unwrap();
        let kept: Vec<&str> = text.lines().take(2).collect();
        std::fs::write(usage.path(), kept.join("\n") + "\n").unwrap();
        let now = chain::verify(usage.path()).unwrap();
        assert!(now.holds());
        assert!(!line.contains(&now.head.unwrap().digest));
    }

    #[test]
    fn an_unreadable_journal_is_said() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path();
        std::fs::create_dir_all(world.join(USAGE)).unwrap();
        let readings = verify(world);
        let lines = anchor_lines(&readings);
        assert!(lines[0].1.starts_with("unreadable: "), "{lines:?}");
        // A world without `.mnemorph` keeps no journal: nothing is anchored.
        let bare = tempfile::tempdir().unwrap();
        assert_eq!(anchor(bare.path(), "housekeep", &readings).unwrap(), None);
    }
}
