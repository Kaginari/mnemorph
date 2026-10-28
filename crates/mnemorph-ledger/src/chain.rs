//! Hash chains over an append-only journal: each row names the digest of the row before it, so a
//! silent edit or a removed row breaks a link that [`verify`] finds.
//!
//! A chained row is the row's own JSON with the chain's fields put around it, in a fixed order:
//!
//! ```text
//! {"prev":"<64 hex>"[,"genesis":true][,"torn":<line>],<the row's fields>,"digest":"<64 hex>"}
//! ```
//!
//! `digest` is the SHA-256 of every byte of the line before `,"digest"` — the bytes as written,
//! never a re-serialisation — and `prev` is the digest of the chain's previous row. The chain's
//! first row is marked `genesis`, and its `prev` is the SHA-256 of every byte the file held before
//! it: unchained rows written before the chain began read as before, and are bound by it. `torn`
//! names the first line of a torn tail the row's writer closed (a crash mid-write): truncation,
//! recorded, never taken for tampering. The row's own fields are recovered byte for byte by
//! [`body`], so a row type that refuses unknown fields reads a chained line as it reads a plain one.

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest as _, Sha256};

const PREV: &[u8] = b"{\"prev\":\"";
const GENESIS: &[u8] = b",\"genesis\":true";
const TORN: &[u8] = b",\"torn\":";
const DIGEST: &[u8] = b",\"digest\":\"";
const HEX: usize = 64;
/// `,"digest":"<64 hex>"}`.
const SUFFIX: usize = DIGEST.len() + HEX + 2;
/// The chain's own field names: a row of a chained journal may not use them.
const RESERVED: [&str; 4] = ["prev", "genesis", "torn", "digest"];

/// The newest row of a chain: its digest and its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The row's digest, 64 lowercase hex digits.
    pub digest: String,
    /// The row's line, from 1, as [`crate::ReadBack::skipped`] counts them.
    pub line: usize,
}

impl fmt::Display for Head {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at line {}", self.digest, self.line)
    }
}

/// Why a link is broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Break {
    /// The row's digest is not the digest of its bytes: the row was edited.
    Edited,
    /// The row's `prev` is not the previous row's digest: a row before it was removed, edited or
    /// put in.
    Unlinked,
    /// A line inside the chain is not one of its rows, and no later row recorded it as torn.
    NotARow,
    /// The chain's first row is not marked genesis: the rows before it were removed.
    NoGenesis,
    /// A row past the chain's first claims to be its genesis.
    LateGenesis,
    /// A row records a torn line where there was none.
    FalseTorn,
    /// The head anchored outside the file is not a row of the chain at its line or later: rows
    /// were dropped from the end, or the chain was rooted again after it.
    Anchor,
}

impl fmt::Display for Break {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Break::Edited => "its digest is not the digest of its bytes: the row was edited",
            Break::Unlinked => {
                "its prev is not the previous row's digest: a row before it was removed, edited \
                 or put in"
            }
            Break::NotARow => {
                "the line is not a row of the chain, and no later row recorded it torn"
            }
            Break::NoGenesis => {
                "the chain's first row is not its genesis: rows before it were removed"
            }
            Break::LateGenesis => "a row past the chain's first claims to be its genesis",
            Break::FalseTorn => "the row records a torn line where there was none",
            Break::Anchor => {
                "the anchored head is not in the chain at its line or later: rows were dropped \
                 from the end, or the chain was rooted again"
            }
        })
    }
}

/// What walking a journal found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// Chained rows that verified their own bytes.
    pub rows: usize,
    /// Non-blank unchained lines before the chain's genesis: read as before, bound by it.
    pub legacy: usize,
    /// The newest row whose digest holds; `None` while no row is chained.
    pub head: Option<Head>,
    /// The chain's first row, marked genesis.
    pub genesis: Option<Head>,
    /// Torn lines a later row recorded: truncations the chain resumed after.
    pub healed: Vec<usize>,
    /// The first line of a torn tail no row has closed yet: a truncation, not tampering.
    pub truncated: Option<usize>,
    /// The first broken link: its line and why.
    pub broken: Option<(usize, Break)>,
}

impl Report {
    /// Whether every link holds — a torn tail included, since a truncation is no tampering.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.broken.is_none()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((line, why)) = self.broken {
            return write!(f, "broken at line {line}: {why}");
        }
        let Some(head) = &self.head else {
            return write!(
                f,
                "unchained: no row carries the chain yet ({} lines)",
                self.legacy
            );
        };
        if let Some(line) = self.truncated {
            write!(
                f,
                "truncated at line {line}; the chain holds to head {head}"
            )?;
        } else {
            write!(f, "OK, head {head}")?;
        }
        write!(f, " ({} rows chained", self.rows)?;
        if self.legacy > 0 {
            write!(f, ", {} unchained before the genesis", self.legacy)?;
        }
        if !self.healed.is_empty() {
            let lines: Vec<String> = self.healed.iter().map(ToString::to_string).collect();
            write!(f, ", torn and resumed after line {}", lines.join(", "))?;
        }
        f.write_str(")")
    }
}

/// One chained line, parsed.
struct Sealed<'a> {
    prev: &'a [u8],
    genesis: bool,
    torn: Option<usize>,
    digest: &'a [u8],
    /// The row's fields, without their braces.
    inner: &'a [u8],
    /// Every byte before `,"digest"`: what the digest covers.
    covered: &'a [u8],
}

fn is_hex(b: &[u8]) -> bool {
    b.len() == HEX && b.iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(HEX), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn parse(line: &[u8]) -> Option<Sealed<'_>> {
    if line.len() < PREV.len() + HEX + 1 + SUFFIX || !line.starts_with(PREV) {
        return None;
    }
    let (covered, suffix) = line.split_at(line.len() - SUFFIX);
    let digest = suffix.strip_prefix(DIGEST)?.strip_suffix(b"\"}")?;
    let prev = &covered[PREV.len()..PREV.len() + HEX];
    if !is_hex(digest) || !is_hex(prev) || covered[PREV.len() + HEX] != b'"' {
        return None;
    }
    let mut rest = &covered[PREV.len() + HEX + 1..];
    let genesis = match rest.strip_prefix(GENESIS) {
        Some(r) => {
            rest = r;
            true
        }
        None => false,
    };
    let torn = match rest.strip_prefix(TORN) {
        Some(r) => {
            let digits = r.iter().take_while(|c| c.is_ascii_digit()).count();
            if digits == 0 || digits > 19 || r[0] == b'0' {
                return None;
            }
            rest = &r[digits..];
            Some(std::str::from_utf8(&r[..digits]).ok()?.parse().ok()?)
        }
        None => None,
    };
    let inner = match rest {
        [] => rest,
        [b',', inner @ ..] if !inner.is_empty() => inner,
        _ => return None,
    };
    Some(Sealed {
        prev,
        genesis,
        torn,
        digest,
        inner,
        covered,
    })
}

/// The row's own bytes in a line: a chained line without the chain's fields, byte for byte as
/// the row serialised; any other line as it is.
#[must_use]
pub fn body(line: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    match parse(line) {
        Some(s) => {
            let mut out = Vec::with_capacity(s.inner.len() + 2);
            out.push(b'{');
            out.extend_from_slice(s.inner);
            out.push(b'}');
            out.into()
        }
        None => line.into(),
    }
}

/// Whether `line` carries the chain's fields.
#[must_use]
pub fn is_sealed(line: &[u8]) -> bool {
    parse(line).is_some()
}

/// A walk along a journal, line by line: what [`verify`] reports, and where the next row links.
/// It is fed bytes in order, in any slices, and keeps only the chain's state and an unfinished
/// line.
#[derive(Debug, Clone)]
pub struct Walk {
    /// Lines finished.
    line: usize,
    /// The unfinished last line.
    carry: Vec<u8>,
    /// The digest of every byte before the genesis; `None` once the chain has begun.
    before: Option<Sha256>,
    /// The first line since the head that is not a row of the chain, and what it is.
    bad_from: Option<(usize, Break)>,
    /// The head anchored outside the file, and whether the walk has met it.
    anchor: Option<(Head, bool)>,
    report: Report,
}

impl Default for Walk {
    fn default() -> Self {
        Walk {
            line: 0,
            carry: Vec::new(),
            before: Some(Sha256::new()),
            bad_from: None,
            anchor: None,
            report: Report::default(),
        }
    }
}

impl Walk {
    /// A walk from a journal's first byte.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The walk, held to `anchor`: a head kept outside the file, which must be a row of the chain
    /// at its line or later ([`Break::Anchor`]).
    #[must_use]
    pub fn anchored(mut self, anchor: Head) -> Self {
        self.anchor = Some((anchor, false));
        self
    }

    /// Walks on through `bytes`, the journal's next bytes.
    pub fn feed(&mut self, mut bytes: &[u8]) {
        while let Some(i) = bytes.iter().position(|&b| b == b'\n') {
            self.line += 1;
            if self.carry.is_empty() {
                self.step(&bytes[..i]);
            } else {
                let mut line = std::mem::take(&mut self.carry);
                line.extend_from_slice(&bytes[..i]);
                self.step(&line);
            }
            bytes = &bytes[i + 1..];
        }
        self.carry.extend_from_slice(bytes);
    }

    /// Walks on through everything `r` reads.
    ///
    /// # Errors
    ///
    /// `r` cannot be read.
    pub fn feed_from(&mut self, mut r: impl Read) -> io::Result<()> {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match r.read(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(n) => self.feed(&buf[..n]),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// What the walk found, were the journal to end here: an unfinished last line is a torn tail
    /// unless it is a whole row of the chain short of its newline.
    #[must_use]
    pub fn report(&self) -> Report {
        let torn = !self.carry.is_empty();
        let mut end = self.clone();
        if torn {
            end.feed(b"\n");
        }
        let mut report = end.report;
        if report.head.is_some()
            && let Some((line, kind)) = end.bad_from
        {
            // A tail torn by a crash ends without its newline; a whole line that is no row is an
            // edit, or a row put there by a writer that does not chain.
            if torn {
                report.truncated = Some(line);
            } else {
                report.broken.get_or_insert((line, kind));
            }
        }
        if let Some((anchor, false)) = &end.anchor {
            report.broken.get_or_insert((anchor.line, Break::Anchor));
        }
        report
    }

    fn broke(&mut self, line: usize, why: Break) {
        self.report.broken.get_or_insert((line, why));
    }

    fn step(&mut self, line: &[u8]) {
        let n = self.line;
        let sealed = parse(line);
        let holds = sealed
            .as_ref()
            .is_some_and(|s| hex(&Sha256::digest(s.covered)).as_bytes() == s.digest);
        let looks = sealed.is_some();
        let Some(s) = sealed.filter(|_| holds) else {
            if self.report.head.is_some() {
                let kind = if looks { Break::Edited } else { Break::NotARow };
                // A crash leaves a partial line; a whole JSON value is never torn, so it breaks
                // the chain at once and no later row may record it.
                if serde_json::from_slice::<serde::de::IgnoredAny>(line).is_ok() {
                    self.broke(n, kind);
                } else {
                    self.bad_from.get_or_insert((n, kind));
                }
                return;
            }
            // Before the chain: an unchained row, bound by the genesis to come. A chained row
            // whose digest fails is an edit, whatever comes after.
            if looks {
                self.broke(n, Break::Edited);
            } else if !line.iter().all(u8::is_ascii_whitespace) {
                self.report.legacy += 1;
            }
            if let Some(h) = &mut self.before {
                h.update(line);
                h.update(b"\n");
            }
            return;
        };
        let want = match (&self.report.head, &self.before) {
            (Some(h), _) => h.digest.clone(),
            (None, Some(before)) => hex(&before.clone().finalize()),
            (None, None) => String::new(),
        };
        if self.report.head.is_none() && !s.genesis {
            self.broke(n, Break::NoGenesis);
        } else if self.report.head.is_some() && s.genesis {
            self.broke(n, Break::LateGenesis);
        } else if let Some((bad, kind)) = self.bad_from
            && s.torn != Some(bad)
        {
            // A line in the chain the next row does not record as torn: it was put there.
            self.broke(bad, kind);
        } else if self.bad_from.is_none() && s.torn.is_some() {
            self.broke(n, Break::FalseTorn);
        } else if s.prev != want.as_bytes() {
            self.broke(n, Break::Unlinked);
        }
        if let Some(t) = s.torn {
            self.report.healed.push(t);
        }
        let head = Head {
            digest: String::from_utf8_lossy(s.digest).into_owned(),
            line: n,
        };
        if let Some((anchor, met)) = &mut self.anchor
            && anchor.digest == head.digest
            && n >= anchor.line
        {
            *met = true;
        }
        if self.report.head.is_none() {
            self.report.genesis = Some(head.clone());
        }
        self.report.head = Some(head);
        self.report.rows += 1;
        self.bad_from = None;
        self.before = None;
    }

    /// The bytes that append `row` — a JSON object, serialised — to the journal walked so far,
    /// chained: a newline first when the last line is unfinished, then the row, linked to the
    /// head (or marked genesis), and its newline. Returns the bytes and the walk past them.
    ///
    /// # Errors
    ///
    /// `row` is not a JSON object, or names one of the chain's own fields.
    pub fn seal(&self, row: &[u8]) -> io::Result<(Vec<u8>, Walk)> {
        let inner = row
            .strip_prefix(b"{")
            .and_then(|r| r.strip_suffix(b"}"))
            .ok_or_else(|| io::Error::other("a chained row is a JSON object"))?;
        let keys: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_slice(row).map_err(io::Error::other)?;
        if let Some(k) = RESERVED.iter().find(|k| keys.contains_key(**k)) {
            return Err(io::Error::other(format!(
                "a chained row may not have a field named `{k}`: the chain's"
            )));
        }
        let mut next = self.clone();
        let mut out = Vec::with_capacity(row.len() + HEX * 2 + 48);
        let torn = !next.carry.is_empty();
        if torn {
            out.push(b'\n');
            next.feed(b"\n");
        }
        let start = out.len();
        out.extend_from_slice(PREV);
        match (&next.report.head, &next.before) {
            (Some(h), _) => out.extend_from_slice(h.digest.as_bytes()),
            (None, before) => {
                let before = before.clone().unwrap_or_default();
                out.extend_from_slice(hex(&before.finalize()).as_bytes());
            }
        }
        out.push(b'"');
        if next.report.head.is_none() {
            out.extend_from_slice(GENESIS);
        } else if let Some((bad, _)) = next.bad_from.filter(|_| torn) {
            // Only a tail this append closes is a truncation; any other line that is no row
            // stays a break for the walk to report.
            out.extend_from_slice(TORN);
            out.extend_from_slice(bad.to_string().as_bytes());
        }
        if !inner.is_empty() {
            out.push(b',');
            out.extend_from_slice(inner);
        }
        let digest = hex(&Sha256::digest(&out[start..]));
        out.extend_from_slice(DIGEST);
        out.extend_from_slice(digest.as_bytes());
        out.extend_from_slice(b"\"}\n");
        next.feed(&out[start..]);
        Ok((out, next))
    }
}

/// Walks the journal at `path` once and reports its chain: OK with its head, a truncation, or the
/// first broken link with its line. A missing file is an empty journal. The walk holds a shared
/// lock on the file, so it never reads a row half-written by a writer that holds it.
///
/// # Errors
///
/// The file exists but cannot be read.
pub fn verify(path: &Path) -> io::Result<Report> {
    verify_against(path, None)
}

/// [`verify`], held to `anchor` — a head kept outside the file: a chain that does not hold it at
/// its line or later is broken ([`Break::Anchor`]), even when every link holds.
///
/// # Errors
///
/// The file exists but cannot be read.
pub fn verify_against(path: &Path, anchor: Option<&Head>) -> io::Result<Report> {
    let mut walk = Walk::new();
    if let Some(a) = anchor {
        walk = walk.anchored(a.clone());
    }
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(walk.report()),
        Err(e) => return Err(e),
    };
    file.lock_shared()?;
    walk.feed_from(&file)?;
    Ok(walk.report())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Journal;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Row {
        n: u32,
        text: String,
    }

    fn row(n: u32) -> Row {
        Row {
            n,
            text: format!("row {n}"),
        }
    }

    fn written(n: u32) -> (tempfile::TempDir, Journal<Row>) {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::chained(dir.path().join("deep/usage.jsonl"));
        for i in 0..n {
            j.append(&row(i)).unwrap();
        }
        (dir, j)
    }

    fn lines(j: &Journal<Row>) -> Vec<String> {
        std::fs::read_to_string(j.path())
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn rewrite(j: &Journal<Row>, lines: &[String]) {
        std::fs::write(j.path(), lines.join("\n") + "\n").unwrap();
    }

    #[test]
    fn appended_rows_verify_and_read_back_as_written() {
        let (_dir, j) = written(5);
        let r = j.verify().unwrap();
        assert!(r.holds() && r.truncated.is_none(), "{r}");
        assert_eq!(r.rows, 5);
        let head = r.head.clone().unwrap();
        assert_eq!(head.line, 5);
        assert_eq!(j.head().unwrap(), Some(head.clone()));
        assert!(r.to_string().starts_with(&format!("OK, head {head}")));
        // A row that refuses unknown fields reads a chained line as it reads a plain one.
        let back = j.read().unwrap();
        assert_eq!(back.rows, (0..5).map(row).collect::<Vec<_>>());
        assert_eq!(back.skipped, Vec::<usize>::new());
        let raw = lines(&j);
        assert!(raw[0].starts_with("{\"prev\":\"") && raw[0].contains(",\"genesis\":true,"));
        assert!(!raw[1].contains("genesis"));
        // The body is the row's own serialisation, byte for byte.
        assert_eq!(
            body(raw[2].as_bytes()).as_ref(),
            serde_json::to_vec(&row(2)).unwrap().as_slice()
        );
        // Each row names the previous row's digest.
        let digest = |l: &str| l[l.len() - 66..l.len() - 2].to_owned();
        assert_eq!(&raw[3][9..73], digest(&raw[2]));
        assert_eq!(digest(&raw[4]), head.digest);
    }

    #[test]
    fn a_silent_edit_is_found_at_its_row() {
        let (_dir, j) = written(5);
        let mut raw = lines(&j);
        raw[2] = raw[2].replace("row 2", "row 9");
        rewrite(&j, &raw);
        let r = j.verify().unwrap();
        assert_eq!(r.broken, Some((3, Break::Edited)), "{r}");
        assert!(r.to_string().starts_with("broken at line 3"));
        // Its digest recomputed too: the next row no longer links.
        let (_dir, j) = written(5);
        let mut raw = lines(&j);
        let covered = &raw[2][..raw[2].len() - SUFFIX].replace("row 2", "row 9");
        raw[2] = format!(
            "{covered},\"digest\":\"{}\"}}",
            hex(&Sha256::digest(covered.as_bytes()))
        );
        rewrite(&j, &raw);
        assert_eq!(j.verify().unwrap().broken, Some((4, Break::Unlinked)));
    }

    #[test]
    fn an_edited_digest_is_found_at_its_row() {
        let (_dir, j) = written(3);
        let mut raw = lines(&j);
        let at = raw[1].len() - 3;
        let flipped = if &raw[1][at..=at] == "0" { "1" } else { "0" };
        raw[1].replace_range(at..=at, flipped);
        rewrite(&j, &raw);
        let r = j.verify().unwrap();
        assert!(r.broken.is_some_and(|(line, _)| line == 2), "{r}");
    }

    #[test]
    fn a_deleted_row_is_found() {
        let (_dir, j) = written(5);
        let mut raw = lines(&j);
        raw.remove(2);
        rewrite(&j, &raw);
        assert_eq!(j.verify().unwrap().broken, Some((3, Break::Unlinked)));
        // The chain's first rows removed: what is left has no genesis.
        let (_dir, j) = written(3);
        rewrite(&j, &lines(&j)[1..]);
        assert_eq!(j.verify().unwrap().broken, Some((1, Break::NoGenesis)));
        // The last row edited is found at its row too.
        let (_dir, j) = written(3);
        let mut raw = lines(&j);
        raw[2] = raw[2].replace("row 2", "row 9");
        rewrite(&j, &raw);
        assert_eq!(j.verify().unwrap().broken, Some((3, Break::Edited)));
        // The last row removed holds as a chain: only a head kept elsewhere tells.
        let (_dir, j) = written(3);
        let kept = j.head().unwrap().unwrap();
        rewrite(&j, &lines(&j)[..2]);
        let r = j.verify().unwrap();
        assert!(r.holds());
        assert_ne!(r.head, Some(kept));
    }

    #[test]
    fn a_torn_last_line_is_a_truncation_and_the_chain_resumes() {
        let (_dir, j) = written(2);
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(j.path())
            .unwrap();
        std::io::Write::write_all(&mut f, b"{\"prev\":\"00").unwrap();
        drop(f);
        let r = j.verify().unwrap();
        assert!(r.holds(), "{r}");
        assert_eq!(r.truncated, Some(3));
        assert_eq!(r.head.as_ref().map(|h| h.line), Some(2));
        assert!(r.to_string().starts_with("truncated at line 3"));
        // A writer that walked before the tear catches up, closes it and records it.
        j.append(&row(2)).unwrap();
        let fresh = Journal::<Row>::chained(j.path());
        fresh.append(&row(3)).unwrap();
        let r = j.verify().unwrap();
        assert!(r.holds() && r.truncated.is_none(), "{r}");
        assert_eq!(r.healed, [3]);
        assert_eq!(r.rows, 4);
        assert!(lines(&j)[3].contains(",\"torn\":3,"));
        assert!(r.to_string().contains("torn and resumed after line 3"));
        let back = j.read().unwrap();
        assert_eq!(
            back.rows.iter().map(|r| r.n).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert_eq!(back.skipped, [3]);
    }

    #[test]
    fn a_tail_torn_twice_is_one_truncation() {
        let (_dir, j) = written(1);
        std::fs::write(
            j.path(),
            std::fs::read_to_string(j.path()).unwrap() + "{\"prev\"\n{\"pr",
        )
        .unwrap();
        assert_eq!(j.verify().unwrap().truncated, Some(2));
        j.append(&row(1)).unwrap();
        let r = j.verify().unwrap();
        assert!(r.holds() && r.truncated.is_none(), "{r}");
        assert_eq!(r.healed, [2]);
    }

    #[test]
    fn a_false_torn_claim_or_a_late_genesis_breaks_the_chain() {
        let (_dir, j) = written(2);
        let seal = |line: &str| {
            let covered = &line[..line.len() - SUFFIX];
            format!(
                "{covered},\"digest\":\"{}\"}}",
                hex(&Sha256::digest(covered.as_bytes()))
            )
        };
        let mut raw = lines(&j);
        raw[1] = seal(&raw[1].replacen(",\"n\"", ",\"torn\":1,\"n\"", 1));
        rewrite(&j, &raw);
        assert_eq!(j.verify().unwrap().broken, Some((2, Break::FalseTorn)));
        let (_dir, j) = written(2);
        let mut raw = lines(&j);
        raw[1] = seal(&raw[1].replacen(",\"n\"", ",\"genesis\":true,\"n\"", 1));
        rewrite(&j, &raw);
        assert_eq!(j.verify().unwrap().broken, Some((2, Break::LateGenesis)));
        // A blank line put inside the chain is no row of it.
        let (_dir, j) = written(2);
        let mut raw = lines(&j);
        raw.insert(1, String::new());
        rewrite(&j, &raw);
        assert_eq!(j.verify().unwrap().broken, Some((2, Break::NotARow)));
    }

    #[test]
    fn an_unchained_journal_becomes_chained_from_its_genesis() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.jsonl");
        let old = Journal::<Row>::new(&path);
        old.append(&row(0)).unwrap();
        old.append(&row(1)).unwrap();
        let r = crate::chain::verify(&path).unwrap();
        assert_eq!((r.head.clone(), r.legacy), (None, 2));
        assert!(r.to_string().starts_with("unchained"));
        let j = Journal::<Row>::chained(&path);
        for n in 2..5 {
            j.append(&row(n)).unwrap();
        }
        let r = j.verify().unwrap();
        assert!(r.holds() && r.truncated.is_none(), "{r}");
        assert_eq!((r.legacy, r.rows), (2, 3));
        assert!(lines(&j)[2].contains("\"genesis\":true"));
        assert_eq!(j.read().unwrap().rows, (0..5).map(row).collect::<Vec<_>>());
        // The genesis binds the rows before it.
        let mut raw = lines(&j);
        raw[0] = raw[0].replace("row 0", "row 7");
        rewrite(&j, &raw);
        assert_eq!(j.verify().unwrap().broken, Some((3, Break::Unlinked)));
        // An unchained row after the genesis is no row of the chain.
        let j = Journal::<Row>::chained(dir.path().join("b.jsonl"));
        j.append(&row(0)).unwrap();
        Journal::<Row>::new(j.path()).append(&row(1)).unwrap();
        j.append(&row(2)).unwrap();
        assert_eq!(j.verify().unwrap().broken, Some((2, Break::NotARow)));
    }

    #[test]
    fn concurrent_appenders_keep_the_chain_linear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.jsonl");
        let shared = Journal::<Row>::chained(&path);
        std::thread::scope(|scope| {
            for t in 0..4u32 {
                let path = path.clone();
                let shared = shared.clone();
                scope.spawn(move || {
                    // Two writers of their own, and clones of one: every head is caught up.
                    let own = Journal::<Row>::chained(path);
                    for n in 0..25 {
                        let j = if n % 2 == 0 { &own } else { &shared };
                        j.append(&Row {
                            n: t * 100 + n,
                            text: "x".repeat(200),
                        })
                        .unwrap();
                    }
                });
            }
        });
        let r = crate::chain::verify(&path).unwrap();
        assert!(r.holds() && r.truncated.is_none(), "{r}");
        assert_eq!(r.rows, 100);
        assert_eq!(r.head.map(|h| h.line), Some(100));
    }

    #[test]
    fn a_row_may_not_use_the_chain_s_names_and_must_be_an_object() {
        #[derive(Serialize, Deserialize)]
        struct Prev {
            prev: u8,
        }
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Empty {}
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::<Prev>::chained(dir.path().join("a.jsonl"));
        assert!(j.append(&Prev { prev: 1 }).is_err());
        let j = Journal::<u8>::chained(dir.path().join("b.jsonl"));
        assert!(j.append(&1).is_err());
        assert_eq!(
            Journal::<u8>::new(dir.path().join("c")).head().unwrap(),
            None
        );
        // An empty object chains too.
        let j = Journal::<Empty>::chained(dir.path().join("d.jsonl"));
        j.append(&Empty {}).unwrap();
        j.append(&Empty {}).unwrap();
        assert!(j.verify().unwrap().holds());
        assert_eq!(j.read().unwrap().rows.len(), 2);
    }

    #[test]
    fn a_whole_row_is_never_torn() {
        let (_dir, j) = written(2);
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(j.path())
            .unwrap();
        std::io::Write::write_all(&mut f, b"{\"n\":7,\"text\":\"forged\"}\n{\"pr").unwrap();
        drop(f);
        assert_eq!(j.verify().unwrap().broken, Some((3, Break::NotARow)));
        j.append(&row(2)).unwrap();
        let r = j.verify().unwrap();
        assert_eq!(r.broken, Some((3, Break::NotARow)), "{r}");
        assert!(!lines(&j)[4].contains("\"torn\":3"), "{}", lines(&j)[4]);
        // Read back, the unchained row after the genesis is no row.
        let back = j.read().unwrap();
        assert_eq!(back.rows.iter().map(|r| r.n).collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(back.skipped, [3, 4]);
    }

    #[test]
    fn a_kept_anchor_finds_a_dropped_tail_and_a_new_root() {
        let (_dir, j) = written(4);
        let anchor = j.head().unwrap().unwrap();
        assert!(verify_against(j.path(), Some(&anchor)).unwrap().holds());
        j.append(&row(4)).unwrap();
        assert!(
            verify_against(j.path(), Some(&anchor)).unwrap().holds(),
            "rows after the anchor are growth"
        );
        // The last rows dropped: every link holds, the anchor does not.
        let raw = lines(&j);
        rewrite(&j, &raw[..3]);
        assert!(j.verify().unwrap().holds());
        let r = verify_against(j.path(), Some(&anchor)).unwrap();
        assert_eq!(r.broken, Some((4, Break::Anchor)), "{r}");
        assert!(r.to_string().contains("anchored head"));
        // Rooted again: the chain's fields stripped, a fresh genesis appended.
        let stripped: Vec<String> = raw
            .iter()
            .map(|l| String::from_utf8(body(l.as_bytes()).into_owned()).unwrap())
            .collect();
        rewrite(&j, &stripped);
        let again = Journal::<Row>::chained(j.path());
        for n in 5..9 {
            again.append(&row(n)).unwrap();
        }
        let r = again.verify().unwrap();
        assert!(r.holds(), "{r}");
        assert_eq!(r.genesis.map(|g| g.line), Some(6));
        assert_eq!(
            verify_against(j.path(), Some(&anchor)).unwrap().broken,
            Some((4, Break::Anchor))
        );
        // A missing journal holds no anchor either.
        let gone = verify_against(&j.path().with_extension("gone"), Some(&anchor)).unwrap();
        assert_eq!(gone.broken, Some((4, Break::Anchor)));
    }

    #[test]
    fn a_witness_holds_every_byte_the_journal_gained() {
        let dir = tempfile::tempdir().unwrap();
        let witness = dir.path().join("state/usage.jsonl");
        let j = Journal::<Row>::chained(dir.path().join("usage.jsonl")).witnessed_at(&witness);
        for n in 0..3 {
            j.append(&row(n)).unwrap();
        }
        assert_eq!(
            std::fs::read(&witness).unwrap(),
            std::fs::read(j.path()).unwrap()
        );
        let plain =
            Journal::<Row>::new(dir.path().join("plain.jsonl")).witnessed_at(dir.path().join("w2"));
        plain.append(&row(0)).unwrap();
        assert!(
            !dir.path().join("w2").exists(),
            "an unchained journal keeps no witness"
        );
    }

    #[test]
    fn a_missing_journal_is_empty_and_an_unreadable_one_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let r = verify(&dir.path().join("none.jsonl")).unwrap();
        assert_eq!(r, Report::default());
        assert!(verify(dir.path()).is_err());
        assert_eq!(
            Journal::<Row>::chained(dir.path().join("none.jsonl"))
                .head()
                .unwrap(),
            None
        );
    }

    #[test]
    fn a_line_that_only_looks_chained_is_not() {
        for line in [
            &b"{\"prev\":\"zz\"}"[..],
            b"{\"prev\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"torn\":01,\"digest\":\"0000000000000000000000000000000000000000000000000000000000000000\"}",
            b"{\"prev\":\"0000000000000000000000000000000000000000000000000000000000000000\"x,\"digest\":\"0000000000000000000000000000000000000000000000000000000000000000\"}",
            b"{\"prev\":\"0000000000000000000000000000000000000000000000000000000000000000\",,\"digest\":\"0000000000000000000000000000000000000000000000000000000000000000\"}",
        ] {
            assert!(parse(line).is_none(), "{}", String::from_utf8_lossy(line));
            assert_eq!(body(line).as_ref(), line);
        }
    }
}
