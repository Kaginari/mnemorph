//! The MCP journal: every level-1 and level-2 load, every call, every offer and every rerank,
//! one row each ([`mnemorph_tools::mcp::Row`]), chained and kept under the human's state
//! directory with its witness beside it — never in the world. Only the loop appends, from what
//! its own doors reported; the learning believes the journal only when its chain holds, its tail
//! is whole and its head is its witness's.

use std::path::Path;

use mnemorph_ledger::{Journal, chain};
use mnemorph_tools::mcp::{Kind, Row, Touched, learned};

/// The journal kept in `state` (a world's MCP state directory), witnessed beside it.
#[must_use]
pub fn journal(state: &Path) -> Journal<Row> {
    Journal::chained(learned::journal_file(state)).witnessed_at(learned::witness_file(state))
}

/// The journal's rows when it can be believed: its chain holds, no tail is torn, and its head is
/// its witness's — so an edit, a row put in, or rows dropped from the end are each found. A
/// journal with no witness, or a witness with no journal, is not believed either.
///
/// # Errors
///
/// Why the rows are not believed.
pub fn read(state: &Path) -> Result<Vec<Row>, String> {
    let file = learned::journal_file(state);
    let witness = learned::witness_file(state);
    let exists = |p: &Path| std::fs::symlink_metadata(p).is_ok();
    if !exists(&file) && !exists(&witness) {
        return Ok(Vec::new());
    }
    let walk = |p: &Path, what: &str| {
        chain::verify(p).map_err(|e| format!("the MCP {what} cannot be walked: {e}"))
    };
    let mine = walk(&file, "journal")?;
    let theirs = walk(&witness, "journal's witness")?;
    for (report, what) in [(&mine, "journal"), (&theirs, "journal's witness")] {
        if !report.holds() {
            return Err(format!("the MCP {what}'s chain does not hold: {report}"));
        }
        if let Some(line) = report.truncated {
            return Err(format!("the MCP {what} is truncated at line {line}"));
        }
    }
    if mine.head != theirs.head {
        return Err(
            "the MCP journal's head is not its witness's: rows were dropped or put in".into(),
        );
    }
    Journal::<Row>::new(&file)
        .read()
        .map(|back| back.rows)
        .map_err(|e| format!("the MCP journal cannot be read: {e}"))
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
