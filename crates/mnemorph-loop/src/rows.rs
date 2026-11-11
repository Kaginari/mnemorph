//! Statement rows: the ask's own examples and rules as runnable rows, kept in the world at
//! [`ROWS`], written from the ask alone (by the human, or a body that never sees the code). The
//! loop runs them itself: once when an ask begins, after every turn that wrote a file, and before
//! the answer. A row that passed when the ask began and fails after a write is said at once,
//! naming the write as the cause, so the body hears that its design broke a stated rule when it
//! makes the edit, not a hundred steps later. Rows are advice, never a gate: a row can be wrong,
//! and the body may answer with a row failing if it cites the ask against it.
//!
//! A rows script prints one line per row, `ROW <id> PASS <label>` or
//! `ROW <id> FAIL <label>: <why>`; any other line is ignored.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mnemorph_tools::{Context, Tool, call};

/// The rows, from the world's root. Under `.mnemorph`, outside `tmp`: a body's tools cannot
/// write them.
pub const ROWS: &str = ".mnemorph/rows.py";

/// How long one run of the rows may take, in seconds.
pub const ROWS_SECS: u64 = 120;

/// The most failing rows one note names; the rest are counted.
const NAMED: usize = 8;

/// One run's rows: by id, whether it passed and what it says.
pub type Sheet = BTreeMap<String, (bool, String)>;

/// The rows a script's output reports. A row reported twice keeps its last line.
#[must_use]
pub fn parse(out: &str) -> Sheet {
    let mut sheet = Sheet::new();
    for line in out.lines() {
        let Some(rest) = line.trim().strip_prefix("ROW ") else {
            continue;
        };
        let mut parts = rest.splitn(3, ' ');
        let (Some(id), Some(verdict)) = (parts.next(), parts.next()) else {
            continue;
        };
        let passed = match verdict {
            "PASS" => true,
            "FAIL" => false,
            _ => continue,
        };
        sheet.insert(
            id.to_owned(),
            (passed, parts.next().unwrap_or_default().trim().to_owned()),
        );
    }
    sheet
}

/// What the loop keeps of the rows through one ask.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rows {
    /// The rows that passed when the ask began.
    began: BTreeSet<String>,
    /// The rows failing at the last run.
    failing: BTreeSet<String>,
    /// Whether the body was told, before an answer, what still fails.
    told: bool,
}

impl Rows {
    /// The rows as the ask begins: `sheet` from their first run.
    #[must_use]
    pub fn begin(sheet: &Sheet) -> Self {
        Self {
            began: passing(sheet),
            failing: failing(sheet),
            told: false,
        }
    }

    /// After a turn that wrote: the rows that passed when the ask began and fail now for the
    /// first time since the last run, said as the write's doing; `None` when none newly broke.
    pub fn after_write(&mut self, sheet: &Sheet) -> Option<String> {
        let now = failing(sheet);
        let broke: Vec<String> = now
            .iter()
            .filter(|id| self.began.contains(*id) && !self.failing.contains(*id))
            .cloned()
            .collect();
        self.failing = now;
        if broke.is_empty() {
            return None;
        }
        let broke: Vec<&String> = broke.iter().collect();
        Some(format!(
            "[rows] Your last change broke what the ask states: these rows passed when you began \
             and fail now:\n{}\nUndo or rethink that change rather than patch around it. (The rows \
             are {ROWS}, written from the ask alone; a row can be wrong: if one contradicts the \
             ask, say so, citing the ask.)",
            named(&broke, sheet)
        ))
    }

    /// Before the body's answer, once an ask: the rows that still fail; `None` when all pass or
    /// the body was told already.
    pub fn before_answer(&mut self, sheet: &Sheet) -> Option<String> {
        self.failing = failing(sheet);
        if self.told || self.failing.is_empty() {
            return None;
        }
        self.told = true;
        let ids: Vec<&String> = self.failing.iter().collect();
        Some(format!(
            "[rows] Before you answer: these rows, written from the ask alone, still fail:\n{}\n\
             Fix them, or, for a row that contradicts the ask, cite the ask's words in your answer.",
            named(&ids, sheet)
        ))
    }
}

fn passing(sheet: &Sheet) -> BTreeSet<String> {
    sheet
        .iter()
        .filter(|(_, (ok, _))| *ok)
        .map(|(id, _)| id.clone())
        .collect()
}

fn failing(sheet: &Sheet) -> BTreeSet<String> {
    sheet
        .iter()
        .filter(|(_, (ok, _))| !*ok)
        .map(|(id, _)| id.clone())
        .collect()
}

/// The rows `ids` as lines, at most [`NAMED`] of them, the rest counted.
fn named(ids: &[&String], sheet: &Sheet) -> String {
    let mut out = String::new();
    for id in ids.iter().take(NAMED) {
        let said = sheet.get(*id).map_or("", |(_, s)| s.as_str());
        let _ = writeln!(out, "- ROW {id} {said}");
    }
    if ids.len() > NAMED {
        let _ = writeln!(out, "- and {} more", ids.len() - NAMED);
    }
    out.trim_end().to_owned()
}

/// Run the world's rows with the body's own `bash`, under its guard: `None` when the world has
/// no rows or the body has no shell.
pub fn run(tools: &[Box<dyn Tool>], cx: &mut Context) -> Option<Sheet> {
    if !cx.world().join(ROWS).is_file() || !tools.iter().any(|t| t.name() == "bash") {
        return None;
    }
    let command = format!("timeout {ROWS_SECS} python3 {ROWS}");
    let input = serde_json::json!({ "command": command }).to_string();
    Some(parse(&call(tools, "bash", &input, cx).content))
}

/// Whether a tool call by `name` writes files.
#[must_use]
pub fn writes(name: &str) -> bool {
    matches!(
        name,
        "edit" | "multiedit" | "write" | "patch" | "move" | "delete"
    )
}
