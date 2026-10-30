//! Envelope uses: one row per outward act an Oni envelope admitted, appended under a lock the
//! gate holds from the count to the act's intent, so two acts can never both take the last use.
//!
//! The journal lives under the human's state directory, outside every world: a body can neither
//! write a use away nor read one in. A use is counted against the envelope's name and the day it
//! was signed, so an envelope signed again under the same name starts a new count. Its rows are
//! chained by hash ([`crate::chain`]): a use written away or edited is found; a journal from
//! before the chain gains its genesis on the next use.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Day, Journal, Timestamp};

/// One act an envelope admitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UseRow {
    /// The envelope's name.
    pub envelope: String,
    /// The day it was signed, `YYYY-MM-DD`: with the name, which envelope this is.
    pub signed: String,
    /// The act's receipt id.
    pub act: String,
    /// The post that did it (`kijin-forge`).
    pub post: String,
    /// The world it was done in.
    pub world: String,
    /// The act, as the card would have shown it, bounded and escaped by the caller.
    pub command: String,
    /// When it was admitted.
    pub at: Timestamp,
    /// Which use of the envelope it was, from 1.
    pub n: u64,
}

impl UseRow {
    /// Whether the row counts against the envelope `name` signed on `signed`.
    #[must_use]
    pub fn of(&self, name: &str, signed: Day) -> bool {
        self.envelope == name && self.signed == signed.to_string()
    }
}

/// The uses journal, and the lock beside it.
#[derive(Debug, Clone)]
pub struct Uses {
    journal: Journal<UseRow>,
}

/// The uses journal held: no other act counts or records a use until it is dropped.
#[derive(Debug)]
pub struct Held<'a> {
    uses: &'a Uses,
    _lock: File,
}

impl Uses {
    /// The journal at `path`, chained; its lock is `path` with the extension `lock`.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            journal: Journal::chained(path),
        }
    }

    /// Where it lives.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.journal.path()
    }

    /// Every use recorded.
    ///
    /// # Errors
    ///
    /// The journal cannot be read, or holds a line that does not parse: a use that cannot be
    /// read might be one more, so no count is given.
    pub fn rows(&self) -> io::Result<Vec<UseRow>> {
        let back = self.journal.read()?;
        if let Some(line) = back.skipped.first() {
            return Err(io::Error::other(format!(
                "{}: line {line} does not read, so the uses cannot be counted",
                self.path().display()
            )));
        }
        Ok(back.rows)
    }

    /// Hold the journal: wait for the lock, made private to the human if it is new.
    ///
    /// # Errors
    ///
    /// The lock cannot be opened or taken.
    pub fn hold(&self) -> io::Result<Held<'_>> {
        let lock = self.path().with_extension("lock");
        if let Some(dir) = lock.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let mut open = OpenOptions::new();
        open.create(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
        let file = open.open(&lock)?;
        file.lock()?;
        Ok(Held {
            uses: self,
            _lock: file,
        })
    }
}

impl Held<'_> {
    /// How many acts the envelope `name` signed on `signed` has admitted.
    ///
    /// # Errors
    ///
    /// As [`Uses::rows`].
    pub fn count(&self, name: &str, signed: Day) -> io::Result<u64> {
        let rows = self.uses.rows()?;
        Ok(rows.iter().filter(|r| r.of(name, signed)).count() as u64)
    }

    /// Record one use, in one write.
    ///
    /// # Errors
    ///
    /// The row cannot be written.
    pub fn record(&self, row: &UseRow) -> io::Result<()> {
        self.uses.journal.append(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, signed: Day, n: u64) -> UseRow {
        UseRow {
            envelope: name.into(),
            signed: signed.to_string(),
            act: format!("act{n}"),
            post: "kijin-forge".into(),
            world: "/w".into(),
            command: "gh pr comment".into(),
            at: Timestamp::now(),
            n,
        }
    }

    #[test]
    fn uses_count_per_envelope_and_signing() {
        let dir = tempfile::tempdir().unwrap();
        let uses = Uses::at(dir.path().join("oni/uses.jsonl"));
        let (d1, d2) = (
            Day::new(2026, 10, 1).unwrap(),
            Day::new(2026, 10, 2).unwrap(),
        );
        let held = uses.hold().unwrap();
        assert_eq!(held.count("forge", d1).unwrap(), 0);
        held.record(&row("forge", d1, 1)).unwrap();
        held.record(&row("forge", d1, 2)).unwrap();
        held.record(&row("forge", d2, 1)).unwrap();
        held.record(&row("ci", d1, 1)).unwrap();
        assert_eq!(held.count("forge", d1).unwrap(), 2);
        assert_eq!(held.count("forge", d2).unwrap(), 1);
        assert_eq!(held.count("ci", d2).unwrap(), 0);
    }

    /// The uses are chained: a use from before the chain is bound by its genesis, concurrent
    /// holders stay one line of links, an edit is found at its line and a dropped tail against
    /// the head kept outside.
    #[test]
    fn uses_are_chained() {
        use crate::chain::{self, Break};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oni/uses.jsonl");
        let day = Day::new(2026, 10, 1).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let old = serde_json::to_string(&row("forge", day, 0)).unwrap();
        std::fs::write(&path, format!("{old}\n")).unwrap();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let path = path.clone();
                scope.spawn(move || {
                    let uses = Uses::at(path);
                    for _ in 0..5 {
                        let held = uses.hold().unwrap();
                        let n = held.count("forge", day).unwrap() + 1;
                        held.record(&row("forge", day, n)).unwrap();
                    }
                });
            }
        });
        let report = chain::verify(&path).unwrap();
        assert!(report.holds(), "{report}");
        assert_eq!((report.legacy, report.rows), (1, 20));
        assert_eq!(report.genesis.as_ref().map(|g| g.line), Some(2));
        let counted = Uses::at(&path).hold().unwrap().count("forge", day).unwrap();
        assert_eq!(counted, 21, "the use from before the chain still counts");
        let head = report.head.unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let edited: Vec<String> = text
            .lines()
            .enumerate()
            .map(|(i, l)| {
                if i == 4 {
                    l.replacen("act", "acx", 1)
                } else {
                    l.to_owned()
                }
            })
            .collect();
        std::fs::write(&path, edited.join("\n") + "\n").unwrap();
        assert_eq!(
            chain::verify(&path).unwrap().broken,
            Some((5, Break::Edited))
        );

        let kept: Vec<&str> = text.lines().take(10).collect();
        std::fs::write(&path, kept.join("\n") + "\n").unwrap();
        assert!(chain::verify(&path).unwrap().holds());
        assert_eq!(
            chain::verify_against(&path, Some(&head)).unwrap().broken,
            Some((head.line, Break::Anchor))
        );
    }

    #[test]
    fn a_line_that_does_not_read_gives_no_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("uses.jsonl");
        std::fs::write(&path, "{\"torn\n").unwrap();
        let uses = Uses::at(&path);
        let held = uses.hold().unwrap();
        assert!(held.count("forge", Day::new(2026, 1, 1).unwrap()).is_err());
    }

    #[test]
    fn the_lock_is_held_until_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let uses = Uses::at(dir.path().join("uses.jsonl"));
        let held = uses.hold().unwrap();
        let lock = OpenOptions::new()
            .append(true)
            .open(dir.path().join("uses.lock"))
            .unwrap();
        assert!(lock.try_lock().is_err(), "a second holder took the lock");
        drop(held);
        assert!(lock.try_lock().is_ok());
    }
}
