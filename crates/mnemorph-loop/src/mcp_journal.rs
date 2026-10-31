//! The MCP journal: every level-1 and level-2 load, every call, every offer and every rerank,
//! one row each ([`mnemorph_tools::mcp::Row`]), chained and kept under the human's state
//! directory with its witness beside it — never in the world, every folder `0700` and every file
//! `0600`, since it holds each ask's words. Only the loop appends, from what its own doors
//! reported; the learning believes the journal only when its chain holds, its tail is whole and
//! its head is its witness's.
//!
//! The journal rotates: past [`SEGMENT`] rows the live segment and its witness are sealed aside,
//! and the new segment opens with a seal row naming the sealed head — its digest and line — so the
//! chain continues across the cut and a sealed segment edited, cut or swapped is found. An ask
//! reads the live segment and the sealed one's tail, never more.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use mnemorph_ledger::{Journal, chain, private_dir};
use mnemorph_tools::mcp::{Kind, Row, Touched, learned};

/// The rows of a live segment past which it is sealed and a new one opened.
pub const SEGMENT: usize = 2048;

/// The journal kept in `state` (a world's MCP state directory), witnessed beside it.
#[must_use]
pub fn journal(state: &Path) -> Journal<Row> {
    Journal::chained(learned::journal_file(state)).witnessed_at(learned::witness_file(state))
}

/// Where a live file is kept once sealed: `<stem>.sealed.jsonl`.
fn sealed(live: &Path) -> PathBuf {
    let stem = live
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".jsonl"))
        .unwrap_or_default();
    live.with_file_name(format!("{stem}.sealed.jsonl"))
}

/// The lock every reader and writer of `state`'s journal holds: shared to read, exclusive to
/// append and rotate, so a rotation never runs under a reader or another writer.
fn lock(state: &Path, exclusive: bool) -> std::io::Result<File> {
    private_dir(state)?;
    let mut o = OpenOptions::new();
    o.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let f = o.open(state.join("journal.lock"))?;
    if exclusive {
        f.lock()?;
    } else {
        f.lock_shared()?;
    }
    Ok(f)
}

/// A segment and its witness, walked: their head when both chains hold, neither tail is torn and
/// the heads are one; `None` when neither file exists.
fn believed(file: &Path, witness: &Path, what: &str) -> Result<Option<chain::Head>, String> {
    let exists = |p: &Path| std::fs::symlink_metadata(p).is_ok();
    if !exists(file) && !exists(witness) {
        return Ok(None);
    }
    let walk = |p: &Path, what: &str| {
        chain::verify(p).map_err(|e| format!("the MCP {what} cannot be walked: {e}"))
    };
    let mine = walk(file, what)?;
    let witness_what = format!("{what}'s witness");
    let theirs = walk(witness, &witness_what)?;
    for (report, what) in [(&mine, what), (&theirs, witness_what.as_str())] {
        if !report.holds() {
            return Err(format!("the MCP {what}'s chain does not hold: {report}"));
        }
        if let Some(line) = report.truncated {
            return Err(format!("the MCP {what} is truncated at line {line}"));
        }
    }
    if mine.head != theirs.head {
        return Err(format!(
            "the MCP {what}'s head is not its witness's: rows were dropped or put in"
        ));
    }
    Ok(mine.head)
}

fn rows_of(file: &Path) -> Result<Vec<Row>, String> {
    Journal::<Row>::new(file)
        .read()
        .map(|back| back.rows)
        .map_err(|e| format!("the MCP journal cannot be read: {e}"))
}

/// The journal's rows when they can be believed: the live segment — its chain holding, no tail
/// torn, its head its witness's, so an edit, a row put in, or rows dropped from the end are each
/// found — and, when it opens with a seal, the tail of the sealed segment it names, held to the
/// same tests and to the seal's head, at most [`learned::WINDOW`] rows in all. A journal with no
/// witness, or a witness with no journal, is not believed either.
///
/// # Errors
///
/// Why the rows are not believed.
pub fn read(state: &Path) -> Result<Vec<Row>, String> {
    let file = learned::journal_file(state);
    let witness = learned::witness_file(state);
    if std::fs::symlink_metadata(state).is_err() {
        return Ok(Vec::new());
    }
    let _held = lock(state, false).map_err(|e| format!("the MCP journal cannot be locked: {e}"))?;
    if believed(&file, &witness, "journal")?.is_none() {
        return Ok(Vec::new());
    }
    let live = rows_of(&file)?;
    let Some(seal) = live.first().filter(|r| r.kind == Kind::Seal) else {
        return Ok(live);
    };
    let head = believed(&sealed(&file), &sealed(&witness), "sealed journal")?;
    if head.as_ref().map(|h| (h.digest.as_str(), h.line as u64))
        != Some((seal.fingerprint.as_str(), seal.wt))
    {
        return Err(
            "the sealed MCP journal is not the one the live journal's seal names: it was cut, \
             edited or swapped"
                .into(),
        );
    }
    let mut rows = rows_of(&sealed(&file))?;
    rows.drain(
        ..rows
            .len()
            .saturating_sub(learned::WINDOW.saturating_sub(live.len())),
    );
    rows.extend(live);
    Ok(rows)
}

/// Append `rows` to `journal` (kept in `state`) under the journal's lock, then seal the live
/// segment once it holds `segment` rows or more: it and its witness are moved aside — the sealed
/// pair before replaced — and the new segment opens with a seal row naming the sealed head. A
/// segment that is not believed is never sealed: the evidence stays where it is.
///
/// # Errors
///
/// A row could not be written, or the segment could not be sealed.
pub fn keep(
    state: &Path,
    journal: &Journal<Row>,
    rows: &[Row],
    segment: usize,
) -> std::io::Result<()> {
    let _held = lock(state, true)?;
    for row in rows {
        journal.append(row)?;
    }
    let file = learned::journal_file(state);
    let witness = learned::witness_file(state);
    let Ok(Some(head)) = believed(&file, &witness, "journal") else {
        return Ok(());
    };
    if head.line < segment {
        return Ok(());
    }
    for old in [sealed(&file), sealed(&witness)] {
        match std::fs::remove_file(&old) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    std::fs::rename(&witness, sealed(&witness))?;
    std::fs::rename(&file, sealed(&file))?;
    let last = rows.last();
    journal.append(&Row {
        ts: last.map_or(0, |r| r.ts),
        ask: last.map(|r| r.ask.clone()).unwrap_or_default(),
        kind: Kind::Seal,
        server: String::new(),
        tool: None,
        creature: String::new(),
        race: String::new(),
        fingerprint: head.digest,
        terms: Vec::new(),
        wt: head.line as u64,
        used: false,
        outcome: "sealed".into(),
        helped: false,
        why: None,
    })
}

/// What one ask did with MCP, as rows: each load, call, offer and rerank in order, `used` set when
/// the tool (a server's level 1: any tool of it) was called after, in the same ask.
pub struct AskRows<'a> {
    /// The ask's id, `<run>#<n>`.
    pub ask: &'a str,
    /// The creature.
    pub creature: &'a str,
    /// Its race.
    pub race: &'a str,
    /// The ask's text, as sent.
    pub said: &'a str,
    /// Whether it ended answered with nothing failing at its gate.
    pub helped: bool,
    /// When, in seconds since the epoch.
    pub ts: u64,
}

impl AskRows<'_> {
    /// The rows: the offers (each tool and why) and the rerank first, then what the doors did.
    #[must_use]
    pub fn rows(
        &self,
        offered: &[(String, String)],
        rerank: Option<&(String, u64)>,
        touched: &[Touched],
    ) -> Vec<Row> {
        let (terms, fingerprint) = learned::terms_of(self.said);
        let called = |from: usize, hit: &dyn Fn(&Touched) -> bool| {
            touched[from..]
                .iter()
                .any(|t| t.kind == Kind::Call && t.outcome == "ok" && hit(t))
        };
        let row = |kind, server: &str, tool: Option<String>, wt, used, outcome: &str| Row {
            ts: self.ts,
            ask: self.ask.to_owned(),
            kind,
            server: server.to_owned(),
            tool,
            creature: self.creature.to_owned(),
            race: self.race.to_owned(),
            fingerprint: fingerprint.clone(),
            terms: terms.clone(),
            wt,
            used,
            outcome: outcome.to_owned(),
            helped: self.helped,
            why: None,
        };
        let mut rows = Vec::new();
        for (name, why) in offered {
            let server = name.split("__").next().unwrap_or_default();
            let used = called(0, &|t| t.tool.as_deref() == Some(name.as_str()));
            let mut r = row(Kind::Offer, server, Some(name.clone()), 0, used, "offered");
            r.why = Some(why.clone());
            rows.push(r);
        }
        if let Some((outcome, wt)) = rerank {
            rows.push(row(Kind::Rerank, "", None, *wt, false, outcome));
        }
        for (i, t) in touched.iter().enumerate() {
            let used = match t.kind {
                Kind::Open => called(i + 1, &|c| c.server == t.server),
                Kind::Schema => called(i + 1, &|c| c.tool == t.tool),
                _ => t.outcome == "ok",
            };
            rows.push(row(
                t.kind,
                &t.server,
                t.tool.clone(),
                t.wt,
                used,
                &t.outcome,
            ));
        }
        rows
    }
}
