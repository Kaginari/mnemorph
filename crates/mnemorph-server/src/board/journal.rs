//! A world's journal, as `GET /api/w/<id>/journal` shows it: `.mnemorph/log.md` cut into its
//! entries — each `### [<time>] <body> — <title>` heading and the lines under it — newest first,
//! with a table of contents and a search. The page renders each entry's text through the board's
//! inert Markdown subset.
//!
//! `log.md` lives inside the world: it opens only as a regular file (a link, a pipe or a device is
//! refused), and only its last [`JOURNAL_BYTES`] are read — the older part is said, never guessed.
//! Each entry's text is cut at [`ENTRY_CHARS`]. What it holds is a body's to write: shown, granting
//! nothing.

use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::Path;

use mnemorph_loop::world::log::LOG;
use mnemorph_loop::world::regular;
use serde::Serialize;
use ts_rs::TS;

/// The most bytes of `log.md` read: its end.
pub const JOURNAL_BYTES: u64 = 4 << 20;

/// The most characters of one entry's text shown.
pub const ENTRY_CHARS: usize = 16_000;

/// The most entries one page holds, and the default.
pub const PAGE_MAX: usize = 100;
const PAGE: usize = 20;

/// The most entries the table of contents lists.
pub const TOC_MAX: usize = 2000;

/// The longest search.
pub const QUERY_MAX: usize = 200;

/// One entry of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct JournalEntry {
    /// Its place in the file, from 1 at the first entry read.
    pub n: u32,
    /// Its heading, as written (without the `###`); the journal's head for the text before the
    /// first entry.
    pub heading: String,
    /// The time its heading names (`[…]`), as written.
    pub at: Option<String>,
    /// The body its heading names.
    pub body: Option<String>,
    /// What its heading says happened.
    pub title: String,
    /// Its lines, Markdown, cut at [`ENTRY_CHARS`].
    pub text: String,
    /// Whether its text was cut.
    pub cut: bool,
}

/// One line of the table of contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct JournalToc {
    /// The entry's place.
    pub n: u32,
    /// Its time, as written.
    pub at: Option<String>,
    /// Its body.
    pub body: Option<String>,
    /// Its title.
    pub title: String,
}

/// `GET /api/w/<id>/journal[?q=&offset=&limit=]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardJournal {
    /// The page of entries, newest first.
    pub entries: Vec<JournalEntry>,
    /// Every entry matching the search, newest first, at most [`TOC_MAX`].
    pub toc: Vec<JournalToc>,
    /// Entries matching the search.
    pub total: u32,
    /// Entries read in all.
    pub all: u32,
    /// Where the page starts.
    pub offset: u32,
    /// The page's size.
    pub limit: u32,
    /// The search, as applied.
    pub q: Option<String>,
    /// Whether an older part of the file was left unread.
    pub older: bool,
    /// The file's size, in bytes; `null` when there is no journal.
    #[ts(type = "number | null")]
    pub bytes: Option<u64>,
    /// Where it was read: inside the world, shown only.
    pub source: String,
}

/// The journal's filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Entries whose heading or text holds this, any case.
    pub q: Option<String>,
    /// Where the page starts.
    pub offset: usize,
    /// The page's size.
    pub limit: usize,
}

impl Filter {
    /// The filter a query string asks for.
    ///
    /// # Errors
    ///
    /// A key it does not know, or a value that does not read.
    pub fn parse(query: Option<&str>) -> Result<Self, String> {
        let mut f = Filter {
            limit: PAGE,
            ..Filter::default()
        };
        for (k, v) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
            match &*k {
                "q" if v.chars().count() > QUERY_MAX => {
                    return Err(format!("the search holds at most {QUERY_MAX} characters"));
                }
                "q" => f.q = Some(v.trim().to_owned()).filter(|q| !q.is_empty()),
                "offset" => f.offset = v.parse().map_err(|_| "`offset` is a count")?,
                "limit" => {
                    f.limit = v
                        .parse::<usize>()
                        .map_err(|_| "`limit` is a count")?
                        .clamp(1, PAGE_MAX);
                }
                other => return Err(format!("the journal has no filter `{other}`")),
            }
        }
        Ok(f)
    }
}

/// The last `cap` bytes of the regular file at `path` as text, from the first whole line, whether
/// an older part was left, and the file's size. `None` when it is absent.
///
/// # Errors
///
/// Not a regular file, or it cannot be read.
pub fn tail(path: &Path, cap: u64) -> Result<Option<(String, bool, u64)>, String> {
    let Some(mut file) = regular::open(path).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    let from = len.saturating_sub(cap);
    file.seek(SeekFrom::Start(from))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(cap)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let start = if from > 0 {
        bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |i| i + 1)
    } else {
        0
    };
    Ok(Some((
        String::from_utf8_lossy(&bytes[start..]).into_owned(),
        from > 0,
        len,
    )))
}

/// `text` cut to `max` characters, and whether it was.
fn cut(text: &str, max: usize) -> (String, bool) {
    match text.char_indices().nth(max) {
        Some((at, _)) => (text[..at].to_owned(), true),
        None => (text.to_owned(), false),
    }
}

/// The parts of an entry's heading: `[<at>] <body> — <title>`.
fn heading_parts(heading: &str) -> (Option<String>, Option<String>, String) {
    let Some(rest) = heading.strip_prefix('[') else {
        return (None, None, heading.to_owned());
    };
    let Some((at, rest)) = rest.split_once(']') else {
        return (None, None, heading.to_owned());
    };
    let rest = rest.trim_start();
    match rest.split_once(" — ") {
        Some((body, title)) => (
            Some(at.to_owned()),
            Some(body.trim().to_owned()),
            title.trim().to_owned(),
        ),
        None => (Some(at.to_owned()), None, rest.trim().to_owned()),
    }
}

/// Whether `line` opens an entry: `### [`.
fn opens(line: &str) -> Option<&str> {
    line.strip_prefix("### ").filter(|h| h.starts_with('['))
}

/// The entries of `text`, in file order. The text before the first entry is the journal's head,
/// kept as an entry when it holds anything but blanks and rules; past a cut, it is a part of an
/// older entry, kept as one too.
#[must_use]
pub fn split(text: &str, older: bool) -> Vec<JournalEntry> {
    let mut out = Vec::new();
    let mut heading: Option<&str> = None;
    let mut body: Vec<&str> = Vec::new();
    let flush = |heading: Option<&str>, body: &mut Vec<&str>, out: &mut Vec<JournalEntry>| {
        // An entry's last line is the rule before the next one: not its text.
        while body
            .last()
            .is_some_and(|l| l.trim().is_empty() || l.trim() == "---")
        {
            body.pop();
        }
        while body.first().is_some_and(|l| l.trim().is_empty()) {
            body.remove(0);
        }
        let joined = body.join("\n");
        body.clear();
        let (head, at, who, title) = match heading {
            Some(h) => {
                let (at, who, title) = heading_parts(h);
                (h.to_owned(), at, who, title)
            }
            None if joined.trim().is_empty() => return,
            None if older => {
                let t = "the end of an older entry".to_owned();
                (t.clone(), None, None, t)
            }
            None => {
                let t = "the journal's head".to_owned();
                (t.clone(), None, None, t)
            }
        };
        let (text, was_cut) = cut(&joined, ENTRY_CHARS);
        out.push(JournalEntry {
            n: u32::try_from(out.len() + 1).unwrap_or(u32::MAX),
            heading: head,
            at,
            body: who,
            title,
            text,
            cut: was_cut,
        });
    };
    for line in text.lines() {
        if let Some(h) = opens(line) {
            flush(heading, &mut body, &mut out);
            heading = Some(h);
        } else {
            body.push(line);
        }
    }
    flush(heading, &mut body, &mut out);
    out
}

/// Whether `e` matches the search `q`, already lower-cased.
fn matches(e: &JournalEntry, q: &str) -> bool {
    e.heading.to_lowercase().contains(q) || e.text.to_lowercase().contains(q)
}

/// `world`'s journal, searched and paged.
///
/// # Errors
///
/// `log.md` is not a regular file, or cannot be read.
pub fn journal(world: &Path, filter: &Filter) -> Result<BoardJournal, String> {
    super::snapshot::parents_plain(world, Path::new(LOG))?;
    let read = tail(&world.join(LOG), JOURNAL_BYTES).map_err(|e| format!("{LOG}: {e}"))?;
    let (text, older, bytes) = match read {
        Some((t, o, b)) => (t, o, Some(b)),
        None => (String::new(), false, None),
    };
    let mut all = split(&text, older);
    all.reverse();
    let count = u32::try_from(all.len()).unwrap_or(u32::MAX);
    let q = filter.q.as_ref().map(|q| q.to_lowercase());
    let found: Vec<JournalEntry> = match &q {
        Some(q) => all.into_iter().filter(|e| matches(e, q)).collect(),
        None => all,
    };
    let limit = filter.limit.clamp(1, PAGE_MAX);
    Ok(BoardJournal {
        toc: found
            .iter()
            .take(TOC_MAX)
            .map(|e| JournalToc {
                n: e.n,
                at: e.at.clone(),
                body: e.body.clone(),
                title: e.title.clone(),
            })
            .collect(),
        total: u32::try_from(found.len()).unwrap_or(u32::MAX),
        entries: found.into_iter().skip(filter.offset).take(limit).collect(),
        all: count,
        offset: u32::try_from(filter.offset).unwrap_or(u32::MAX),
        limit: u32::try_from(limit).unwrap_or(u32::MAX),
        q: filter.q.clone(),
        older,
        bytes,
        source: LOG.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG_TEXT: &str = "# Journal\n\nThe world's log.\n\n---\n\n### [2026-10-01T10:00:00+02:00] rimuru — the first\n- **Done:** one\n\n---\n\n### [2026-10-02T10:00:00+02:00] orc-o — the second\n- **Done:** two\n";

    #[test]
    fn it_splits_entries_and_their_headings() {
        let e = split(LOG_TEXT, false);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].title, "the journal's head");
        assert_eq!(e[0].text, "# Journal\n\nThe world's log.");
        assert_eq!(e[1].at.as_deref(), Some("2026-10-01T10:00:00+02:00"));
        assert_eq!(e[1].body.as_deref(), Some("rimuru"));
        assert_eq!(e[1].title, "the first");
        assert_eq!(e[1].text, "- **Done:** one");
        assert_eq!(e[2].n, 3);
        let past = split("tail of an old one\n### [x] y — z\nb\n", true);
        assert_eq!(past[0].title, "the end of an older entry");
        assert_eq!(
            heading_parts("no bracket"),
            (None, None, "no bracket".into())
        );
        assert_eq!(
            heading_parts("[t] only title"),
            (Some("t".into()), None, "only title".into())
        );
        assert_eq!(heading_parts("[open"), (None, None, "[open".into()));
    }

    #[test]
    fn an_entry_is_cut_and_says_so() {
        let long = format!("### [t] a — b\n{}\n", "x".repeat(ENTRY_CHARS + 10));
        let e = split(&long, false);
        assert!(e[0].cut);
        assert_eq!(e[0].text.chars().count(), ENTRY_CHARS);
    }

    #[test]
    fn the_filter_reads_and_refuses() {
        let f = Filter::parse(Some("q=%20Gate%20&offset=2&limit=999")).unwrap();
        assert_eq!(f.q.as_deref(), Some("Gate"));
        assert_eq!((f.offset, f.limit), (2, PAGE_MAX));
        assert_eq!(Filter::parse(Some("q=")).unwrap().q, None);
        assert!(Filter::parse(Some(&format!("q={}", "a".repeat(QUERY_MAX + 1)))).is_err());
        assert!(Filter::parse(Some("x=1")).is_err());
        assert!(Filter::parse(Some("limit=a")).is_err());
    }

    #[test]
    fn the_journal_is_read_newest_first_searched_and_capped() {
        let w = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(w.path().join(".mnemorph")).unwrap();
        let none = journal(w.path(), &Filter::parse(None).unwrap()).unwrap();
        assert_eq!((none.all, none.bytes), (0, None));
        std::fs::write(w.path().join(LOG), LOG_TEXT).unwrap();
        let j = journal(w.path(), &Filter::parse(None).unwrap()).unwrap();
        assert_eq!(j.all, 3);
        assert_eq!(j.entries[0].title, "the second");
        assert_eq!(j.toc.len(), 3);
        assert!(!j.older);
        let s = journal(w.path(), &Filter::parse(Some("q=FIRST")).unwrap()).unwrap();
        assert_eq!(s.total, 1);
        assert_eq!(s.entries[0].title, "the first");
        // Past the cap, only the end is read, and the page says so.
        let big = format!("{}{LOG_TEXT}", "filler line\n".repeat(400_000));
        std::fs::write(w.path().join(LOG), &big).unwrap();
        let j = journal(w.path(), &Filter::parse(None).unwrap()).unwrap();
        assert!(j.older);
        assert_eq!(j.bytes, Some(big.len() as u64));
        assert_eq!(j.entries[0].title, "the second");
        #[cfg(unix)]
        {
            std::fs::remove_file(w.path().join(LOG)).unwrap();
            std::os::unix::fs::symlink("/dev/zero", w.path().join(LOG)).unwrap();
            let e = journal(w.path(), &Filter::parse(None).unwrap()).unwrap_err();
            assert!(e.contains("not a regular file"), "{e}");
        }
    }
}
