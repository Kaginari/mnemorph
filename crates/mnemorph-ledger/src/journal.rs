//! The append-only JSONL journal: one row per line, never rewritten; chained by choice
//! ([`crate::chain`]).

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use crate::chain::{Head, Report, Walk};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// An append-only file of JSON rows, one per line.
#[derive(Debug, Clone)]
pub struct Journal<T> {
    path: PathBuf,
    /// A chained journal's walk so far, shared by its clones; `None` for an unchained one.
    chain: Option<Arc<Mutex<Option<Tip>>>>,
    /// Where a chained journal's every appended byte is written again, out of every command's
    /// reach, for the tripwire to put the journal back from.
    witness: Option<PathBuf>,
    /// Where a chained journal's every appended byte is written again after it, under its own
    /// lock: a copy for readers, the journal itself being the trusted one.
    mirror: Option<PathBuf>,
    rows: PhantomData<fn() -> T>,
}

/// Where a chained journal's walk stands: the walk, and the file's length it walked to.
#[derive(Debug)]
struct Tip {
    walk: Walk,
    len: u64,
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
            chain: None,
            witness: None,
            mirror: None,
            rows: PhantomData,
        }
    }

    /// A chained journal at `path`: each row appended carries `prev` and `digest`
    /// ([`crate::chain`]). Unchained rows already there read as before; the first row this
    /// appends is the chain's genesis. Every writer of a chained journal chains: the head is kept
    /// in memory, walked once on the first append, and caught up under the file's lock when
    /// another writer appended since.
    #[must_use]
    pub fn chained(path: impl Into<PathBuf>) -> Self {
        Journal {
            path: path.into(),
            chain: Some(Arc::new(Mutex::new(None))),
            witness: None,
            mirror: None,
            rows: PhantomData,
        }
    }

    /// The same chained journal, every byte it appends written first to `file` too, under that
    /// file's lock: the witness the tripwire rebuilds the journal from after a command, so a row
    /// the harness wrote while the command ran is never dropped. An unchained journal keeps no
    /// witness.
    #[must_use]
    pub fn witnessed_at(mut self, file: impl Into<PathBuf>) -> Self {
        if self.chain.is_some() {
            self.witness = Some(file.into());
        }
        self
    }

    /// The same chained journal, every byte it appends written after it to `file` too, under the
    /// journal's lock: the journal is the trusted record its chain is walked over, and `file` a
    /// copy of it for readers who cannot reach it. A Kijin post's witness keeps the chain and
    /// mirrors it into the post's journal in the world. An unchained journal keeps no mirror.
    #[must_use]
    pub fn mirrored_to(mut self, file: impl Into<PathBuf>) -> Self {
        if self.chain.is_some() {
            self.mirror = Some(file.into());
        }
        self
    }

    /// Whether the journal chains its rows.
    #[must_use]
    pub fn is_chained(&self) -> bool {
        self.chain.is_some()
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
        use std::io::{Read, Seek, SeekFrom};
        if let Some(chain) = &self.chain {
            return self.append_chained(chain, row);
        }
        let mut line = serde_json::to_vec(row).map_err(io::Error::other)?;
        line.push(b'\n');
        let dir = self.path.parent().filter(|d| !d.as_os_str().is_empty());
        if let Some(dir) = dir {
            std::fs::create_dir_all(dir)?;
        }
        let created = !self.path.exists();
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&self.path)?;
        // A torn previous write leaves a line without its newline: close it first, on the same
        // handle, so this row stands on a line of its own.
        if file.metadata()?.len() > 0 {
            let mut last = [0u8; 1];
            file.seek(SeekFrom::End(-1))?;
            file.read_exact(&mut last)?;
            if last[0] != b'\n' {
                line.insert(0, b'\n');
            }
        }
        file.write_all(&line)?;
        file.sync_data()?;
        // A new file's directory entry is durable only once the directory is synced too.
        if created && let Some(dir) = dir {
            File::open(dir)?.sync_all()?;
        }
        Ok(())
    }

    /// [`Journal::append`] for a chained journal: under the file's exclusive lock, the walk is
    /// caught up with what other writers appended, and the row is sealed to its head.
    fn append_chained(&self, chain: &Mutex<Option<Tip>>, row: &T) -> io::Result<()> {
        let body = serde_json::to_vec(row).map_err(io::Error::other)?;
        let dir = self.path.parent().filter(|d| !d.as_os_str().is_empty());
        if let Some(dir) = dir {
            std::fs::create_dir_all(dir)?;
        }
        // The witness first, then the journal: the tripwire reads both under the witness's lock.
        let witness = match &self.witness {
            Some(w) => {
                if let Some(d) = w.parent() {
                    std::fs::create_dir_all(d)?;
                }
                let f = OpenOptions::new().create(true).append(true).open(w)?;
                f.lock()?;
                Some(f)
            }
            None => None,
        };
        let created = !self.path.exists();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&self.path)?;
        file.lock()?;
        let mut tip = chain.lock().unwrap_or_else(PoisonError::into_inner);
        // Taken: a write that fails leaves no tip, and the next append walks again.
        let walked = caught_up(tip.take(), &file)?;
        let (bytes, walk) = walked.walk.seal(&body)?;
        if let Some(mut w) = witness.as_ref() {
            w.write_all(&bytes)?;
            w.sync_data()?;
        }
        (&file).write_all(&bytes)?;
        file.sync_data()?;
        if created && let Some(dir) = dir {
            File::open(dir)?.sync_all()?;
        }
        *tip = Some(Tip {
            walk,
            len: walked.len + bytes.len() as u64,
        });
        if let Some(mirror) = &self.mirror {
            mirror_bytes(mirror, &bytes)?;
        }
        Ok(())
    }

    /// The chain's head: the newest row whose digest holds, read from the walk kept in memory and
    /// caught up with the file; `None` for an unchained journal or one no row is chained in yet.
    ///
    /// # Errors
    ///
    /// The file exists but cannot be read.
    pub fn head(&self) -> io::Result<Option<Head>> {
        let Some(chain) = &self.chain else {
            return Ok(None);
        };
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        file.lock_shared()?;
        let mut tip = chain.lock().unwrap_or_else(PoisonError::into_inner);
        let walked = caught_up(tip.take(), &file)?;
        let head = walked.walk.report().head;
        *tip = Some(walked);
        Ok(head)
    }

    /// Walks the whole file once and reports its chain ([`crate::chain::verify`]).
    ///
    /// # Errors
    ///
    /// The file exists but cannot be read.
    pub fn verify(&self) -> io::Result<Report> {
        crate::chain::verify(&self.path)
    }

    /// Reads every row back. A missing file is an empty journal; a line that does not parse is
    /// skipped and its number reported, and so is an unchained line once the chain has begun: no
    /// writer of a chained journal writes one.
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
        let mut chained = false;
        for (n, line) in BufReader::new(file).split(b'\n').enumerate() {
            let mut line = line?;
            if n == 0 && line.starts_with(b"\xEF\xBB\xBF") {
                line.drain(..3);
            }
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let sealed = crate::chain::is_sealed(&line);
            chained |= sealed;
            if chained && !sealed {
                back.skipped.push(n + 1);
                continue;
            }
            match serde_json::from_slice(&crate::chain::body(&line)) {
                Ok(row) => back.rows.push(row),
                Err(_) => back.skipped.push(n + 1),
            }
        }
        Ok(back)
    }
}

/// `bytes` appended to `mirror` in one write and synced; a torn last line there is closed first.
fn mirror_bytes(mirror: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::{Read as _, Seek, SeekFrom};
    if let Some(d) = mirror.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d)?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(mirror)?;
    let mut line = Vec::with_capacity(bytes.len() + 1);
    if f.metadata()?.len() > 0 {
        let mut last = [0u8; 1];
        f.seek(SeekFrom::End(-1))?;
        f.read_exact(&mut last)?;
        if last[0] != b'\n' {
            line.push(b'\n');
        }
    }
    line.extend_from_slice(bytes);
    f.write_all(&line)?;
    f.sync_data()
}

/// `tip` caught up with `file`, read under its lock: what was appended since is walked on; a file
/// shorter than the walk, or no walk yet, is walked from its start.
fn caught_up(tip: Option<Tip>, file: &File) -> io::Result<Tip> {
    use std::io::{Read as _, Seek, SeekFrom};
    let len = file.metadata()?.len();
    let mut tip = match tip {
        Some(t) if t.len <= len => t,
        _ => Tip {
            walk: Walk::new(),
            len: 0,
        },
    };
    if tip.len < len {
        let mut f = file;
        f.seek(SeekFrom::Start(tip.len))?;
        tip.walk.feed_from(f.take(len - tip.len))?;
        tip.len = len;
    }
    Ok(tip)
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
    fn a_byte_order_mark_is_not_a_torn_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.jsonl");
        std::fs::write(&path, b"\xEF\xBB\xBF{\"n\":1,\"text\":\"a\"}\r\n").unwrap();
        let back = Journal::<Row>::new(&path).read().unwrap();
        assert_eq!(back.rows.len(), 1);
        assert_eq!(back.skipped, Vec::<usize>::new());
    }

    #[test]
    fn concurrent_appenders_lose_no_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.jsonl");
        std::thread::scope(|scope| {
            for t in 0..4u32 {
                let path = path.clone();
                scope.spawn(move || {
                    let j: Journal<Row> = Journal::new(path);
                    for n in 0..25 {
                        j.append(&Row {
                            n: t * 100 + n,
                            text: "x".repeat(200),
                        })
                        .unwrap();
                    }
                });
            }
        });
        let back = Journal::<Row>::new(&path).read().unwrap();
        assert_eq!(back.rows.len(), 100);
        assert_eq!(back.skipped, Vec::<usize>::new());
    }

    #[test]
    fn an_unwritable_place_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("plain");
        std::fs::write(&file, b"x").unwrap();
        let j: Journal<Row> = Journal::new(file.join("usage.jsonl"));
        assert!(
            j.append(&Row {
                n: 1,
                text: String::new()
            })
            .is_err()
        );
    }

    #[test]
    fn an_unreadable_journal_is_an_error_not_empty() {
        let dir = tempfile::tempdir().unwrap();
        let j: Journal<Row> = Journal::new(dir.path());
        assert!(j.read().is_err(), "a directory is not an empty journal");
    }
}
