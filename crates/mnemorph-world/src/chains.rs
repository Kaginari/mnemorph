//! The world's chained journals, walked and anchored. Each chains its rows by hash
//! ([`mnemorph_ledger::chain`]), and their heads are anchored outside the files — an entry in
//! `log.md`, and a copy kept under the human's state directory, out of every command's reach — so
//! each walk is held to the newest head kept, and a dropped tail or a chain rooted again is found.
//! `mnemorphd housekeep --verify` walks and anchors them; the Dark Elf's law audit calls
//! [`verify`]. The journals:
//!
//! - `usage`: the usage ledger, in the world;
//! - `value`: the Dark Elf's challenge records — findings, verdicts, seal proposals, waivers;
//! - `evolution`: the evolution ledger;
//! - `gate`: the end gate's verdicts ([`crate::verdicts`]);
//! - `uses` and `revocations`: the Oni envelopes' uses and revocations, one pair for every world;
//! - `kijin-<post>`: each Kijin post's witness, the trusted copy of its journal.
//!
//! Every one but `usage` lives under the human's state directory, so a world with no home keeps
//! only `usage`.

use std::path::{Path, PathBuf};

use mnemorph_ledger::chain::{self, Head, Report};
use mnemorph_ledger::{Journal, Timestamp};
use mnemorph_races::Post;
use serde::{Deserialize, Serialize};

use crate::log::{Entry, LOG};
use crate::queue::state_file_in;
use crate::worth::USAGE;

/// One chained journal of a world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chained {
    /// Its name: `usage`, `value`, `evolution`, `gate`, `uses`, `revocations` or `kijin-<post>`.
    pub name: &'static str,
    /// Where it lives.
    pub path: PathBuf,
}

/// The chained journals of `world`: the usage ledger, and with a home to keep them under, the
/// value, evolution and gate journals, the envelopes' uses and revocations, and each Kijin post's
/// witness that is kept or was anchored.
#[must_use]
pub fn journals(world: &Path) -> Vec<Chained> {
    journals_under(world, crate::queue::state_dir().as_deref())
}

/// [`journals`], the human's state directory at `state`.
fn journals_under(world: &Path, state: Option<&Path>) -> Vec<Chained> {
    let mut out = vec![Chained {
        name: "usage",
        path: world.join(USAGE),
    }];
    let Some(state) = state else {
        return out;
    };
    for (name, kind) in [
        ("value", "challenges"),
        ("evolution", "evolution"),
        ("gate", "gate"),
    ] {
        out.push(Chained {
            name,
            path: state_file_in(state, world, kind),
        });
    }
    let oni = state.join("mnemorph/oni");
    out.push(Chained {
        name: "uses",
        path: oni.join("uses.jsonl"),
    });
    out.push(Chained {
        name: "revocations",
        path: oni.join("revoked.jsonl"),
    });
    // A post's witness is walked once kept, and for good once anchored: a witness removed whole
    // is then found against its anchor.
    let anchors = state_file_in(state, world, "anchors");
    for post in Post::ALL {
        let path = post_witness_in(state, world, post);
        if path.exists() || anchored(world, &anchors, post.creature()).is_some() {
            out.push(Chained {
                name: post.creature(),
                path,
            });
        }
    }
    out
}

/// Where the witness of `world`'s Kijin `post` is kept:
/// `$XDG_STATE_HOME/mnemorph/kijin/<world>-<fnv64a>/<post>.jsonl`, else under `~/.local/state`;
/// `None` with no home. The post journal writes it at the same place
/// (`mnemorph_tools::kijin_witness`).
#[must_use]
pub fn post_witness(world: &Path, post: Post) -> Option<PathBuf> {
    Some(post_witness_in(&crate::queue::state_dir()?, world, post))
}

fn post_witness_in(state: &Path, world: &Path, post: Post) -> PathBuf {
    state_file_in(state, world, "kijin")
        .with_extension("")
        .join(format!("{}.jsonl", post.creature()))
}

/// Where `world`'s anchors are kept apart from `log.md`:
/// `$XDG_STATE_HOME/mnemorph/anchors/<world>-<fnv64a>.jsonl`; `None` with no home.
#[must_use]
pub fn kept_at(world: &Path) -> Option<PathBuf> {
    crate::queue::state_file(world, "anchors")
}

/// One anchor, as the state directory keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    ts: Timestamp,
    journal: String,
    digest: String,
    line: usize,
}

/// One journal, walked and held to its newest anchor.
#[derive(Debug)]
pub struct Reading {
    /// The journal.
    pub journal: Chained,
    /// What the walk found; an error when the journal cannot be read.
    pub report: std::io::Result<Report>,
    /// The head the walk was held to: the state directory's newest, else `log.md`'s.
    pub anchor: Option<Head>,
    /// How `log.md`'s newest anchor and the state directory's disagree, when they do: one of the
    /// two was edited.
    pub mismatch: Option<String>,
}

impl Reading {
    /// Whether the chain holds, its anchor included, and the two anchors agree.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.mismatch.is_none() && self.report.as_ref().is_ok_and(Report::holds)
    }

    /// The reading in a line.
    #[must_use]
    pub fn text(&self) -> String {
        let mut text = match &self.report {
            Ok(r) => r.to_string(),
            Err(e) => format!("unreadable: {e}"),
        };
        if let Some(m) = &self.mismatch {
            text.push_str("; ");
            text.push_str(m);
        }
        text
    }
}

/// Each chained journal of `world`, walked once and held to its newest anchor: OK with its head,
/// a truncation, or the first broken link with its line — a head kept outside the file that the
/// chain no longer holds included — and any disagreement between `log.md`'s anchor and the
/// state directory's.
#[must_use]
pub fn verify(world: &Path) -> Vec<Reading> {
    verify_kept(world, kept_at(world).as_deref())
}

/// The newest head anchored for the journal `name` of `world`: the state directory's, else
/// `log.md`'s; `None` when none was anchored yet.
#[must_use]
pub fn anchor_of(world: &Path, name: &str) -> Option<Head> {
    match kept_at(world) {
        Some(kept) => anchored(world, &kept, name),
        None => logged(
            &std::fs::read_to_string(world.join(LOG)).unwrap_or_default(),
            name,
        ),
    }
}

/// The newest head of the journal `name` anchored in `kept` or, failing that, in `world`'s
/// `log.md`.
fn anchored(world: &Path, kept: &Path, name: &str) -> Option<Head> {
    Journal::<Kept>::new(kept)
        .read()
        .ok()
        .and_then(|r| {
            r.rows
                .into_iter()
                .rev()
                .find(|k| k.journal == name)
                .map(|k| Head {
                    digest: k.digest,
                    line: k.line,
                })
        })
        .or_else(|| {
            logged(
                &std::fs::read_to_string(world.join(LOG)).unwrap_or_default(),
                name,
            )
        })
}

/// [`verify`], the anchors kept at `kept`.
fn verify_kept(world: &Path, kept: Option<&Path>) -> Vec<Reading> {
    verify_these(world, kept, journals(world))
}

/// `journals` of `world` walked, the anchors kept at `kept`.
fn verify_these(world: &Path, kept: Option<&Path>, journals: Vec<Chained>) -> Vec<Reading> {
    let kept = kept
        .map(|p| {
            Journal::<Kept>::new(p)
                .read()
                .map(|r| r.rows)
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let log = std::fs::read_to_string(world.join(LOG)).unwrap_or_default();
    journals
        .into_iter()
        .map(|journal| {
            let k = kept
                .iter()
                .rev()
                .find(|k| k.journal == journal.name)
                .map(|k| Head {
                    digest: k.digest.clone(),
                    line: k.line,
                });
            let l = logged(&log, journal.name);
            let mismatch = match (&k, &l) {
                (Some(k), Some(l)) if k != l => Some(format!(
                    "log.md's newest anchor ({l}) is not the one kept under the state directory \
                     ({k})"
                )),
                (Some(k), None) => Some(format!(
                    "log.md holds no anchor of it; the state directory keeps {k}"
                )),
                _ => None,
            };
            let anchor = k.or(l);
            let report = chain::verify_against(&journal.path, anchor.as_ref());
            Reading {
                journal,
                report,
                anchor,
                mismatch,
            }
        })
        .collect()
}

/// The newest head `log.md` anchors for the journal `name`.
fn logged(log: &str, name: &str) -> Option<Head> {
    let label = format!("- **{name} chain:** ");
    log.lines()
        .rev()
        .filter_map(|l| l.strip_prefix(&label))
        .find_map(|text| {
            let rest = &text[text.find("head ")? + 5..];
            let digest = rest.get(..64)?;
            let line = rest.get(64..)?.strip_prefix(" at line ")?;
            let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
            digest
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
                .then(|| Head {
                    digest: digest.to_owned(),
                    line: digits.parse().unwrap_or(0),
                })
                .filter(|h| h.line > 0)
        })
}

/// The lines that anchor `readings` in a `log.md` entry: one per journal, its name and its
/// reading — the head, whole. An ascension, probation or fall entry carries them too.
#[must_use]
pub fn anchor_lines(readings: &[Reading]) -> Vec<(String, String)> {
    readings
        .iter()
        .map(|r| (format!("{} chain", r.journal.name), r.text()))
        .collect()
}

/// Writes `readings` into `world`'s `log.md` as one entry by `by`, and keeps each head that holds
/// under the state directory too: the heads anchored outside the files. A reading that does not
/// hold anchors no new head, so a later walk is still held to the last good one. Returns the
/// entry's heading, `None` when the world keeps no journal (and nothing is kept).
///
/// # Errors
///
/// The journal or the kept anchors cannot be written.
pub fn anchor(world: &Path, by: &str, readings: &[Reading]) -> std::io::Result<Option<String>> {
    anchor_kept(world, by, readings, kept_at(world).as_deref())
}

/// [`anchor`], the anchors kept at `kept`.
fn anchor_kept(
    world: &Path,
    by: &str,
    readings: &[Reading],
    kept: Option<&Path>,
) -> std::io::Result<Option<String>> {
    let heading = crate::log::append_entry(
        world,
        &Entry {
            body: by.to_owned(),
            title: "chains: heads anchored".to_owned(),
            lines: anchor_lines(readings),
        },
    )?;
    if heading.is_some()
        && let Some(path) = kept
    {
        let kept = Journal::<Kept>::new(path);
        for r in readings.iter().filter(|r| r.holds()) {
            if let Ok(Report {
                head: Some(head), ..
            }) = &r.report
            {
                kept.append(&Kept {
                    ts: Timestamp::now(),
                    journal: r.journal.name.to_owned(),
                    digest: head.digest.clone(),
                    line: head.line,
                })?;
            }
        }
    }
    Ok(heading)
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

    /// A world with a chained usage ledger of `n` rows, and where its anchors are kept.
    fn world(n: u64) -> (tempfile::TempDir, PathBuf, Journal<UsageRow>) {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        let usage = Journal::<UsageRow>::chained(world.join(USAGE));
        for i in 0..n {
            usage.append(&usage_row(i)).unwrap();
        }
        let kept = dir.path().join("state/anchors.jsonl");
        (dir, kept, usage)
    }

    fn walk(usage: &Journal<UsageRow>, kept: &Path) -> Reading {
        let world = usage.path().ancestors().nth(3).unwrap();
        verify_kept(world, Some(kept)).remove(0)
    }

    fn anchored(usage: &Journal<UsageRow>, kept: &Path) -> String {
        let world = usage.path().ancestors().nth(3).unwrap();
        let readings = verify_kept(world, Some(kept));
        anchor_kept(world, "housekeep", &readings, Some(kept))
            .unwrap()
            .unwrap()
    }

    #[test]
    fn the_head_anchored_in_log_md_matches_the_journal() {
        let (_dir, kept, usage) = world(3);
        let world = usage.path().ancestors().nth(3).unwrap().to_path_buf();
        let first = walk(&usage, &kept);
        assert_eq!(first.journal.name, "usage");
        assert!(first.holds() && first.anchor.is_none(), "{}", first.text());
        let heading = anchored(&usage, &kept);
        assert!(heading.contains("housekeep — chains: heads anchored"));
        let log = std::fs::read_to_string(world.join(LOG)).unwrap();
        let head = usage.head().unwrap().unwrap();
        let line = log
            .lines()
            .find(|l| l.starts_with("- **usage chain:**"))
            .unwrap();
        assert!(
            line.contains(&format!("OK, head {} at line 3", head.digest)),
            "{line}"
        );
        assert_eq!(logged(&log, "usage"), Some(head.clone()));
        // The next walk is held to it, and growth after it holds.
        usage.append(&usage_row(3)).unwrap();
        let again = walk(&usage, &kept);
        assert!(again.holds(), "{}", again.text());
        assert_eq!(again.anchor, Some(head));
    }

    #[test]
    fn a_dropped_tail_is_found_against_the_kept_anchor() {
        let (_dir, kept, usage) = world(4);
        anchored(&usage, &kept);
        let text = std::fs::read_to_string(usage.path()).unwrap();
        let left: Vec<&str> = text.lines().take(2).collect();
        std::fs::write(usage.path(), left.join("\n") + "\n").unwrap();
        assert!(chain::verify(usage.path()).unwrap().holds());
        let r = walk(&usage, &kept);
        assert!(!r.holds());
        assert_eq!(
            r.report.as_ref().unwrap().broken,
            Some((4, chain::Break::Anchor))
        );
        // A failed walk anchors no new head: the next walk still finds it.
        anchored(&usage, &kept);
        assert!(!walk(&usage, &kept).holds());
    }

    #[test]
    fn a_chain_rooted_again_is_found_against_the_kept_anchor() {
        let (_dir, kept, usage) = world(3);
        anchored(&usage, &kept);
        let text = std::fs::read_to_string(usage.path()).unwrap();
        let stripped: String = text
            .lines()
            .map(|l| String::from_utf8(chain::body(l.as_bytes()).into_owned()).unwrap() + "\n")
            .collect();
        std::fs::write(usage.path(), stripped).unwrap();
        let fresh = Journal::<UsageRow>::chained(usage.path());
        for n in 0..4 {
            fresh.append(&usage_row(n)).unwrap();
        }
        assert!(chain::verify(usage.path()).unwrap().holds());
        let r = walk(&usage, &kept);
        assert_eq!(
            r.report.as_ref().unwrap().broken,
            Some((3, chain::Break::Anchor)),
            "{}",
            r.text()
        );
    }

    #[test]
    fn an_edit_of_log_md_alone_is_found() {
        let (_dir, kept, usage) = world(3);
        anchored(&usage, &kept);
        let world = usage.path().ancestors().nth(3).unwrap().to_path_buf();
        let log = std::fs::read_to_string(world.join(LOG)).unwrap();
        let head = usage.head().unwrap().unwrap();
        std::fs::write(
            world.join(LOG),
            log.replace(&format!("at line {}", head.line), "at line 2"),
        )
        .unwrap();
        let r = walk(&usage, &kept);
        assert!(r.report.as_ref().unwrap().holds());
        assert!(!r.holds());
        assert!(r.text().contains("log.md's newest anchor"), "{}", r.text());
        // The entry removed from log.md: said too.
        std::fs::write(world.join(LOG), "").unwrap();
        assert!(
            walk(&usage, &kept)
                .mismatch
                .is_some_and(|m| m.contains("log.md holds no anchor"))
        );
    }

    /// With a home, every trust journal is walked: the usage, value, evolution and gate
    /// journals, the envelopes' uses and revocations, and each Kijin post's witness once kept or
    /// anchored; a dropped tail of any is found against the head kept for it.
    #[test]
    fn every_trust_journal_is_walked_and_held_to_its_anchor() {
        let (dir, kept, _usage) = world(1);
        let world = dir.path().join("w");
        let state = dir.path().join("state");
        let names = |j: &[Chained]| j.iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names(&journals_under(&world, None)), ["usage"]);
        let all = journals_under(&world, Some(&state));
        assert_eq!(
            names(&all),
            ["usage", "value", "evolution", "gate", "uses", "revocations"]
        );
        assert!(all.iter().skip(1).all(|c| c.path.starts_with(&state)));
        // A post's witness, kept.
        let forge = post_witness_in(&state, &world, Post::Forge);
        std::fs::create_dir_all(forge.parent().unwrap()).unwrap();
        std::fs::write(&forge, "").unwrap();
        let mut all = journals_under(&world, Some(&state));
        assert_eq!(
            all.last().map(|c| (c.name, &c.path)),
            Some(("kijin-forge", &forge))
        );
        // Two rows in each, anchored, then the last of each dropped: each is found.
        for c in &all {
            let j = Journal::<UsageRow>::chained(&c.path);
            if c.name != "usage" {
                j.append(&usage_row(0)).unwrap();
            }
            j.append(&usage_row(1)).unwrap();
        }
        let readings = verify_these(&world, Some(&kept), all.clone());
        assert!(readings.iter().all(Reading::holds));
        anchor_kept(&world, "housekeep", &readings, Some(&kept)).unwrap();
        for c in &all {
            let text = std::fs::read_to_string(&c.path).unwrap();
            std::fs::write(&c.path, text.lines().next().unwrap().to_owned() + "\n").unwrap();
        }
        for r in verify_these(&world, Some(&kept), all.clone()) {
            assert_eq!(
                r.report.as_ref().unwrap().broken,
                Some((2, chain::Break::Anchor)),
                "{}: {}",
                r.journal.name,
                r.text()
            );
        }
        // A witness removed whole is still walked, since it was anchored, and found.
        std::fs::remove_file(&forge).unwrap();
        all = journals_under(&world, Some(&state));
        let gone = all.iter().find(|c| c.name == "kijin-forge").cloned();
        assert!(gone.is_some(), "an anchored witness stays walked");
    }

    #[test]
    fn an_unreadable_journal_is_said() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path();
        std::fs::create_dir_all(world.join(USAGE)).unwrap();
        let readings = verify_kept(world, None);
        let lines = anchor_lines(&readings);
        assert!(lines[0].1.starts_with("unreadable: "), "{lines:?}");
        // A world without `.mnemorph` keeps no journal: nothing is anchored, nothing kept.
        let bare = tempfile::tempdir().unwrap();
        let kept = bare.path().join("kept.jsonl");
        assert_eq!(
            anchor_kept(bare.path(), "housekeep", &readings, Some(&kept)).unwrap(),
            None
        );
        assert!(!kept.exists());
    }
}
