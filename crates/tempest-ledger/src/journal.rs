//! The append-only JSONL journal: one row per line, never rewritten.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// An append-only file of JSON rows, one per line.
#[derive(Debug, Clone)]
pub struct Journal<T> {
    path: PathBuf,
    rows: PhantomData<fn() -> T>,
}

/// What reading a journal back found.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadBack<T> {
    /// Every row that parsed, in order.
    pub rows: Vec<T>,
    /// Lines that did not parse — a torn last write, or a hand edit. Reported, never fatal.
    pub skipped: Vec<usize>,
}

impl<T: Serialize + DeserializeOwned> Journal<T> {
    /// A journal at `path`; the file is created on the first append.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Journal {
            path: path.into(),
            rows: PhantomData,
        }
    }

    /// Where it lives.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one row as one line, in a single write, and syncs it to disk before returning:
    /// a row the caller was told is written survives a crash.
    ///
    /// # Errors
    ///
    /// The row does not serialise, or the file cannot be opened, written or synced.
    pub fn append(&self, row: &T) -> io::Result<()> {
        let mut line = serde_json::to_vec(row).map_err(io::Error::other)?;
        line.push(b'\n');
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        // A torn previous write leaves a line without its newline: close it so this row stands
        // on a line of its own.
        if file.metadata()?.len() > 0 && !ends_with_newline(&self.path)? {
            file.write_all(b"\n")?;
        }
        file.write_all(&line)?;
        file.sync_data()
    }

    /// Reads every row back. A missing file is an empty journal; a line that does not parse is
    /// skipped and its number reported.
    ///
    /// # Errors
    ///
    /// The file exists but cannot be read.
    pub fn read(&self) -> io::Result<ReadBack<T>> {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(ReadBack {
                    rows: Vec::new(),
                    skipped: Vec::new(),
                });
            }
            Err(e) => return Err(e),
        };
        let mut back = ReadBack {
            rows: Vec::new(),
            skipped: Vec::new(),
        };
        for (n, line) in BufReader::new(file).split(b'\n').enumerate() {
            let line = line?;
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            match serde_json::from_slice(&line) {
                Ok(row) => back.rows.push(row),
                Err(_) => back.skipped.push(n + 1),
            }
        }
        Ok(back)
    }
}

fn ends_with_newline(path: &Path) -> io::Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = File::open(path)?;
    f.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    f.read_exact(&mut last)?;
    Ok(last[0] == b'\n')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Row {
        n: u32,
        text: String,
    }

    #[test]
    fn appends_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let j: Journal<Row> = Journal::new(dir.path().join("deep/usage.jsonl"));
        assert_eq!(j.read().unwrap().rows, []);
        for n in 0..3 {
            j.append(&Row {
                n,
                text: format!("line\n{n}"),
            })
            .unwrap();
        }
        let back = j.read().unwrap();
        assert_eq!(back.rows.len(), 3);
        assert_eq!(
            back.rows[2],
            Row {
                n: 2,
                text: "line\n2".into()
            }
        );
        assert_eq!(back.skipped, Vec::<usize>::new());
        let raw = std::fs::read_to_string(j.path()).unwrap();
        assert_eq!(
            raw.lines().count(),
            3,
            "a newline inside a field stays escaped"
        );
    }

    #[test]
    fn a_torn_write_is_skipped_and_the_next_row_stands_alone() {
        let dir = tempfile::tempdir().unwrap();
        let j: Journal<Row> = Journal::new(dir.path().join("usage.jsonl"));
        j.append(&Row {
            n: 1,
            text: "a".into(),
        })
        .unwrap();
        // A crash mid-write: half a row, no newline.
        let mut f = OpenOptions::new().append(true).open(j.path()).unwrap();
        f.write_all(b"\n   \n{\"n\":2,\"te").unwrap();
        drop(f);
        j.append(&Row {
            n: 3,
            text: "c".into(),
        })
        .unwrap();
        let back = j.read().unwrap();
        assert_eq!(back.rows.iter().map(|r| r.n).collect::<Vec<_>>(), [1, 3]);
        assert_eq!(
            back.skipped,
            [4],
            "blank lines are skipped silently, the torn one is named"
        );
    }

    #[test]
    fn an_unreadable_journal_is_an_error_not_empty() {
        let dir = tempfile::tempdir().unwrap();
        let j: Journal<Row> = Journal::new(dir.path());
        assert!(j.read().is_err(), "a directory is not an empty journal");
    }
}
