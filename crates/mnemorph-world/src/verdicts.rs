//! The end gate's verdicts, kept by the harness outside the world:
//! `$XDG_STATE_HOME/mnemorph/gate/<world>-<fnv64a>.jsonl`, one row per verdict the gate journals in
//! `log.md`, written by the harness alone, chained by hash ([`mnemorph_ledger::chain`]) and anchored
//! with the world's other chained journals ([`crate::chains`]). Each row carries the build that
//! wrote it, so a replay tells a pass earned under old gate rules from one earned under new ones
//! ([`by_build`]).

use std::io;
use std::path::{Path, PathBuf};

use mnemorph_ledger::{Build, Journal, Timestamp};
use serde::{Deserialize, Serialize};

/// One verdict of the end gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictRow {
    /// When it was given.
    pub ts: Timestamp,
    /// The body whose landing it judged.
    pub body: String,
    /// The verdict as `log.md` heads it: `PASS`, `FAIL, handed back`, …
    pub verdict: String,
    /// The paths the landing changed.
    #[serde(default)]
    pub changed: Vec<String>,
    /// The checks that failed.
    #[serde(default)]
    pub failures: Vec<String>,
    /// The checks that gave no result.
    #[serde(default)]
    pub unverified: Vec<String>,
    /// The heading of its `log.md` entry; `None` when the entry could not be written.
    #[serde(default)]
    pub entry: Option<String>,
    /// The build that gave it; `unknown` for a row from before stamps.
    #[serde(default)]
    pub build: Build,
    /// The run whose landing it judged; `None` for a row from before runs were named, which the
    /// report fold gives to the body's latest run begun before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

/// Where `world`'s gate verdicts are kept: `$XDG_STATE_HOME/mnemorph/gate/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` when there is no home.
#[must_use]
pub fn verdicts_of(world: &Path) -> Option<PathBuf> {
    crate::queue::state_file(world, "gate")
}

/// `world`'s gate verdicts, chained, for the harness to append to; `None` when there is no home.
/// Every process that gives a verdict appends under the file's lock, the head read and the row
/// written in one hold.
#[must_use]
pub fn journal(world: &Path) -> Option<Journal<VerdictRow>> {
    verdicts_of(world).map(journal_at)
}

/// The gate verdicts kept at `path`, chained.
#[must_use]
pub fn journal_at(path: impl Into<PathBuf>) -> Journal<VerdictRow> {
    Journal::chained(path)
}

/// Every verdict of `world`, in order; an empty list with no home.
///
/// # Errors
///
/// The verdicts exist but cannot be read.
pub fn read(world: &Path) -> io::Result<Vec<VerdictRow>> {
    journal(world).map_or_else(|| Ok(Vec::new()), |j| Ok(j.read()?.rows))
}

/// The verdicts of `rows` that `build` gave, in order: a replay held to one build's rules.
pub fn by_build<'a>(
    rows: &'a [VerdictRow],
    build: &'a Build,
) -> impl Iterator<Item = &'a VerdictRow> {
    rows.iter().filter(move |r| &r.build == build)
}

/// Appends `row` to `world`'s gate verdicts, stamped with this binary's build. Nothing is written
/// with no home.
///
/// # Errors
///
/// The verdicts cannot be written.
pub fn record(world: &Path, row: VerdictRow) -> io::Result<()> {
    journal(world).map_or(Ok(()), |j| append_stamped(&j, row))
}

/// `row` appended to `journal`, stamped with this binary's build.
fn append_stamped(journal: &Journal<VerdictRow>, mut row: VerdictRow) -> io::Result<()> {
    row.build = Build::this();
    journal.append(&row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verdicts_live_outside_the_world_and_are_anchored_with_the_others() {
        let world = Path::new("/srv/worlds/jura");
        let Some(at) = verdicts_of(world) else {
            return;
        };
        assert!(!at.starts_with(world), "{}", at.display());
        assert!(at.to_string_lossy().contains("mnemorph/gate/jura-"));
        assert!(journal(world).is_some_and(|j| j.is_chained()));
        assert!(
            crate::chains::journals(world)
                .iter()
                .any(|j| j.name == "gate" && j.path == at)
        );
    }

    #[test]
    fn an_old_row_reads_unknown() {
        let row: VerdictRow = serde_json::from_str(
            r#"{"ts":"2026-10-30T12:00:00Z","body":"slime","verdict":"PASS"}"#,
        )
        .unwrap();
        assert_eq!(row.build, Build::default());
        assert_eq!(row.build.to_string(), "unknown (unknown)");
    }

    fn row(n: u32) -> VerdictRow {
        VerdictRow {
            ts: Timestamp::now(),
            body: format!("slime-{n}"),
            verdict: "PASS".into(),
            changed: vec!["src/a.rs".into()],
            failures: Vec::new(),
            unverified: Vec::new(),
            entry: None,
            build: Build::default(),
            run: None,
        }
    }

    /// Verdicts are chained: concurrent writers stay one line of links, an edit is found at its
    /// line, a dropped tail against the head kept outside; each row carries its build, and a
    /// replay can keep one build's rows.
    #[test]
    fn verdicts_are_chained_and_stamped() {
        use mnemorph_ledger::chain::{self, Break};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gate/w.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A row from before the chain and before stamps.
        std::fs::write(
            &path,
            "{\"ts\":\"2026-10-30T12:00:00Z\",\"body\":\"orc\",\"verdict\":\"FAIL\"}\n",
        )
        .unwrap();
        std::thread::scope(|scope| {
            for t in 0..4 {
                let path = path.clone();
                scope.spawn(move || {
                    let j = journal_at(path);
                    for n in 0..5 {
                        append_stamped(&j, row(t * 10 + n)).unwrap();
                    }
                });
            }
        });
        let report = chain::verify(&path).unwrap();
        assert!(report.holds(), "{report}");
        assert_eq!((report.legacy, report.rows), (1, 20));
        assert_eq!(report.genesis.as_ref().map(|g| g.line), Some(2));
        let rows = journal_at(&path).read().unwrap().rows;
        assert_eq!(rows.len(), 21);
        assert_eq!(by_build(&rows, &Build::this()).count(), 20);
        assert_eq!(by_build(&rows, &Build::default()).count(), 1);
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            raw.lines()
                .skip(1)
                .all(|l| l.contains("\"build\":{\"version\":")),
            "every new row carries its build"
        );
        let head = report.head.unwrap();

        let edited: Vec<String> = raw
            .lines()
            .enumerate()
            .map(|(i, l)| {
                if i == 5 {
                    l.replacen("PASS", "FAIL", 1)
                } else {
                    l.to_owned()
                }
            })
            .collect();
        std::fs::write(&path, edited.join("\n") + "\n").unwrap();
        assert_eq!(
            chain::verify(&path).unwrap().broken,
            Some((6, Break::Edited))
        );

        let kept: Vec<&str> = raw.lines().take(9).collect();
        std::fs::write(&path, kept.join("\n") + "\n").unwrap();
        assert!(chain::verify(&path).unwrap().holds());
        assert_eq!(
            chain::verify_against(&path, Some(&head)).unwrap().broken,
            Some((21, Break::Anchor))
        );
    }
}
