//! The file tools: read, write, edit, glob, grep. Every path is resolved and checked against the
//! world (and, for writes, the soil) before anything is touched.

use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use globset::GlobBuilder;
use ignore::WalkBuilder;
use mnemorph_races::ToolClass;
use regex::RegexBuilder;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::tool::{Context, Outcome, Seen, Tool, args};

/// Lines a read returns when the call names no limit.
const DEFAULT_LINES: usize = 2000;
/// Characters of one line a read shows before cutting it.
const LINE_CHARS: usize = 2000;
/// Paths a glob lists, matches a grep shows.
const MAX_RESULTS: usize = 500;

fn hash(bytes: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

/// Keep the world as it is in the attic before a write replaces part of it.
pub(crate) fn keep(cx: &Context) {
    if let Some(attic) = cx.attic() {
        // The attic is recovery, not containment: a failed snapshot does not stop the write.
        let _ = attic.snapshot("before a file write");
    }
}

// ── read ────────────────────────────────────────────────────────────────────

/// Read a file's lines, numbered.
pub struct Read;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

impl Tool for Read {
    fn name(&self) -> &'static str {
        "read"
    }

    fn description(&self) -> &'static str {
        "Read a text file inside the world, as numbered lines. `offset` is the first line (1-based), `limit` how many lines. Reading the same unchanged window again returns a short pointer instead of the text."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "The file, relative to the world's root."},
                "offset": {"type": "integer", "minimum": 1, "description": "The first line to read (1-based)."},
                "limit": {"type": "integer", "minimum": 1, "description": "How many lines to read."}
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({"path": "src/main.rs", "offset": 1, "limit": 40})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: ReadArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        let path = match cx.inside(&a.path) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let bytes = match crate::beneath::read(cx.world(), &path) {
            Ok(b) => b,
            Err(e) => return Outcome::error(format!("cannot read `{}`: {e}", cx.shown(&path))),
        };
        if bytes.iter().take(8192).any(|b| *b == 0) {
            return Outcome::error(format!(
                "`{}` is a binary file; read reads text",
                cx.shown(&path)
            ));
        }
        let text = String::from_utf8_lossy(&bytes);
        let total = text.lines().count();
        let first = a.offset.unwrap_or(1).max(1);
        let count = a.limit.unwrap_or(DEFAULT_LINES).max(1);
        let last = (first + count - 1).min(total);
        let seen = Seen {
            window: (first, last),
            hash: hash(&bytes),
            pointed: false,
        };
        let shown = cx.shown(&path);
        let earlier = cx.seen.get(&path);
        let same = earlier.is_some_and(|e| e.window == seen.window && e.hash == seen.hash);
        // Once, the earlier result is pointed at — with where the file goes on. Asked again, the
        // body did not find it: the window is shown, never pointed at a second time.
        if same && !earlier.is_some_and(|e| e.pointed) {
            let on = if last < total {
                format!(
                    " The file has {total} lines: read on with offset {}, or grep for what you need.",
                    last + 1
                )
            } else {
                String::new()
            };
            let mut o = Outcome::ok(
                format!(
                    "`{shown}` lines {first}–{last} are unchanged since you last read them; that \
                     result still holds above.{on}"
                ),
                ToolClass::Read,
            );
            o.meta.unchanged = true;
            cx.seen.insert(
                path,
                Seen {
                    pointed: true,
                    ..seen
                },
            );
            return o;
        }
        let mut out = String::new();
        if total == 0 {
            let _ = write!(out, "`{shown}` is empty.");
        } else if first > total {
            return Outcome::error(format!(
                "`{shown}` has {total} lines; offset {first} is past the end"
            ));
        } else {
            for (i, line) in text
                .lines()
                .enumerate()
                .skip(first - 1)
                .take(last + 1 - first)
            {
                let cut: String = line.chars().take(LINE_CHARS).collect();
                let more = if cut.len() < line.len() { " …" } else { "" };
                let _ = writeln!(out, "{:>6}\t{cut}{more}", i + 1);
            }
            if last < total {
                let _ = write!(
                    out,
                    "… {} more lines (offset {} continues)",
                    total - last,
                    last + 1
                );
            }
        }
        cx.seen.insert(path, seen);
        Outcome::ok(out, ToolClass::Read)
    }
}

// ── write ───────────────────────────────────────────────────────────────────

/// Write a whole file inside the body's soil.
pub struct Write;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteArgs {
    path: String,
    content: String,
}

impl Tool for Write {
    fn name(&self) -> &'static str {
        "write"
    }

    fn description(&self) -> &'static str {
        "Write a whole file inside your soil, creating it and its directories if needed. For a change to part of a file, use edit."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "The file, relative to the world's root."},
                "content": {"type": "string", "description": "The file's entire new content."}
            },
            "required": ["path", "content"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Write
    }

    fn example(&self) -> Value {
        json!({"path": "notes.md", "content": "# Notes\n"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: WriteArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        let path = match cx.writable(&a.path) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let before = crate::beneath::read(cx.world(), &path).ok();
        if before.as_deref() == Some(a.content.as_bytes()) {
            return Outcome::error(format!(
                "`{}` already holds exactly that; nothing was written",
                cx.shown(&path)
            ));
        }
        keep(cx);
        if let Err(e) = crate::beneath::write(cx.world(), &path, a.content.as_bytes()) {
            return Outcome::error(format!("cannot write `{}`: {e}", cx.shown(&path)));
        }
        cx.seen.remove(&path);
        let lines = a.content.lines().count();
        let mut o = Outcome::ok(
            format!("wrote `{}` ({lines} lines)", cx.shown(&path)),
            ToolClass::Write,
        );
        // A file that was not text had no lines to remove: counting its bytes as lines is noise.
        let old = before
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default();
        (o.meta.lines_added, o.meta.lines_removed) = line_delta(&old, &a.content);
        o.meta.wrote.push(path);
        o
    }
}

/// Lines added and removed from `before` to `after`, as a multiset of lines: cheap, and the same
/// on every run. A line moved within the file counts as neither.
#[must_use]
pub fn line_delta(before: &str, after: &str) -> (u32, u32) {
    let mut count: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for line in before.lines() {
        *count.entry(line).or_default() -= 1;
    }
    for line in after.lines() {
        *count.entry(line).or_default() += 1;
    }
    let (mut added, mut removed) = (0u32, 0u32);
    for n in count.values() {
        let k = u32::try_from(n.unsigned_abs()).unwrap_or(u32::MAX);
        if *n > 0 {
            added = added.saturating_add(k);
        } else {
            removed = removed.saturating_add(k);
        }
    }
    (added, removed)
}

/// Why an edit was refused: its words, and whether `old_string` was missing from the file.
struct Refused {
    why: String,
    miss: bool,
}

impl From<&str> for Refused {
    fn from(why: &str) -> Refused {
        Refused {
            why: why.to_owned(),
            miss: false,
        }
    }
}

/// An edit's refusal as a call's outcome; a missing `old_string` is marked an edit miss.
fn refused(r: Refused) -> Outcome {
    let mut o = Outcome::error(r.why);
    o.meta.edit_miss = r.miss;
    o
}

/// Replace `old` with `new` in `text` under edit's rules: exactly one match, or every match with
/// `all`. Returns the new text, the count replaced and the first line touched; an error names the
/// closest line for no match, or the lines of several.
fn replace(
    text: &str,
    old: &str,
    new: &str,
    all: bool,
    shown: &str,
) -> Result<(String, usize, usize), Refused> {
    if old.is_empty() {
        return Err("`old_string` is empty; to create or replace a whole file, use write".into());
    }
    if old == new {
        return Err("`old_string` and `new_string` are the same; nothing would change".into());
    }
    let at: Vec<usize> = text.match_indices(old).map(|(i, _)| i).collect();
    let line_of = |byte: usize| text[..byte].matches('\n').count() + 1;
    match at.len() {
        0 => {
            let (n, line) = closest_line(text, old);
            return Err(Refused {
                why: format!(
                    "`old_string` does not occur in `{shown}`; the closest line is {n}: `{}`. Read the file and copy the text exactly.",
                    line.trim()
                ),
                miss: true,
            });
        }
        k if k > 1 && !all => {
            let lines: Vec<String> = at.iter().map(|&i| line_of(i).to_string()).collect();
            return Err(format!(
                "`old_string` occurs {k} times in `{shown}` (lines {}); add context to make it unique, or set replace_all",
                lines.join(", ")
            )
            .as_str()
            .into());
        }
        _ => {}
    }
    let updated = if all {
        text.replace(old, new)
    } else {
        text.replacen(old, new, 1)
    };
    Ok((updated, if all { at.len() } else { 1 }, line_of(at[0])))
}

// ── edit ────────────────────────────────────────────────────────────────────

/// Replace one exact string in a file inside the body's soil.
pub struct Edit;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditArgs {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

impl Tool for Edit {
    fn name(&self) -> &'static str {
        "edit"
    }

    fn description(&self) -> &'static str {
        "Replace `old_string` with `new_string` in a file inside your soil. `old_string` must match exactly once (or set `replace_all`); no match names the closest line, several matches name their lines."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "The file, relative to the world's root."},
                "old_string": {"type": "string", "description": "The exact text to replace."},
                "new_string": {"type": "string", "description": "The text to put in its place."},
                "replace_all": {"type": "boolean", "description": "Replace every occurrence instead of exactly one."}
            },
            "required": ["path", "old_string", "new_string"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Write
    }

    fn example(&self) -> Value {
        json!({"path": "src/lib.rs", "old_string": "fn old()", "new_string": "fn new()"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: EditArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        let path = match cx.writable(&a.path) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let shown = cx.shown(&path);
        let text = match crate::beneath::read(cx.world(), &path) {
            Ok(b) => match String::from_utf8(b) {
                Ok(t) => t,
                // Rewriting it would replace the bytes that are not text: refused whole.
                Err(_) => {
                    return Outcome::error(format!(
                        "`{shown}` is not UTF-8 text; edit would replace its bytes — rewrite it whole with write"
                    ));
                }
            },
            Err(e) => return Outcome::error(format!("cannot read `{shown}`: {e}")),
        };
        let (updated, n, first_line) =
            match replace(&text, &a.old_string, &a.new_string, a.replace_all, &shown) {
                Ok(r) => r,
                Err(e) => return refused(e),
            };
        keep(cx);
        if let Err(e) = crate::beneath::write(cx.world(), &path, updated.as_bytes()) {
            return Outcome::error(format!("cannot write `{shown}`: {e}"));
        }
        cx.seen.remove(&path);
        let mut o = Outcome::ok(
            format!("edited `{shown}`: {n} replacement(s), from line {first_line}"),
            ToolClass::Write,
        );
        (o.meta.lines_added, o.meta.lines_removed) = line_delta(&text, &updated);
        o.meta.wrote.push(path);
        o
    }
}

/// The line of `text` most like the first non-blank line of `wanted`, and its number.
fn closest_line<'a>(text: &'a str, wanted: &str) -> (usize, &'a str) {
    let target = wanted
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    text.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l, similarity(l.trim(), target)))
        .max_by(|a, b| a.2.total_cmp(&b.2).then(b.0.cmp(&a.0)))
        .map_or((1, ""), |(n, l, _)| (n, l))
}

/// How alike two lines are, from 0 to 1: one minus the edit distance over the longer length.
fn similarity(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) =
        (a.chars().take(400).collect(), b.chars().take(400).collect());
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "line lengths are capped at 400 characters"
    )]
    let score = 1.0 - row[b.len()] as f64 / longest as f64;
    score
}

// ── multiedit ───────────────────────────────────────────────────────────────

/// Several exact replacements in one file, in order, all or none.
pub struct MultiEdit;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OneEdit {
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MultiEditArgs {
    path: String,
    edits: Vec<OneEdit>,
}

impl Tool for MultiEdit {
    fn name(&self) -> &'static str {
        "multiedit"
    }

    fn description(&self) -> &'static str {
        "Make several edits to one file inside your soil, in order, each under edit's rules (each `old_string` matches exactly once, or set its `replace_all`; a later edit sees the earlier ones' result). If any edit fails, none is written."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "The file, relative to the world's root."},
                "edits": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": {"type": "string"},
                            "new_string": {"type": "string"},
                            "replace_all": {"type": "boolean"}
                        },
                        "required": ["old_string", "new_string"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["path", "edits"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Write
    }

    fn example(&self) -> Value {
        json!({"path": "src/lib.rs", "edits": [{"old_string": "fn a()", "new_string": "fn b()"}]})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: MultiEditArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        if a.edits.is_empty() {
            return Outcome::error("`edits` is empty; nothing would change");
        }
        let path = match cx.writable(&a.path) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let shown = cx.shown(&path);
        let original = match crate::beneath::read(cx.world(), &path) {
            Ok(b) => b,
            Err(e) => return Outcome::error(format!("cannot read `{shown}`: {e}")),
        };
        // A file that is not UTF-8 text would come back with its bytes replaced: refused whole.
        let Ok(mut text) = String::from_utf8(original.clone()) else {
            return Outcome::error(format!(
                "`{shown}` is not UTF-8 text; multiedit would replace its bytes — rewrite it whole with write"
            ));
        };
        let mut total = 0;
        for (i, e) in a.edits.iter().enumerate() {
            match replace(&text, &e.old_string, &e.new_string, e.replace_all, &shown) {
                Ok((t, n, _)) => {
                    text = t;
                    total += n;
                }
                Err(why) => {
                    return refused(Refused {
                        why: format!(
                            "edit {} of {}: {} Nothing was written.",
                            i + 1,
                            a.edits.len(),
                            why.why
                        ),
                        miss: why.miss,
                    });
                }
            }
        }
        if text.as_bytes() == original.as_slice() {
            return Outcome::error(format!(
                "the edits leave `{shown}` as it was; nothing was written"
            ));
        }
        keep(cx);
        if let Err(e) = crate::beneath::write(cx.world(), &path, text.as_bytes()) {
            return Outcome::error(format!("cannot write `{shown}`: {e}"));
        }
        cx.seen.remove(&path);
        let mut o = Outcome::ok(
            format!(
                "edited `{shown}`: {} edit(s), {total} replacement(s)",
                a.edits.len()
            ),
            ToolClass::Write,
        );
        (o.meta.lines_added, o.meta.lines_removed) =
            line_delta(&String::from_utf8_lossy(&original), &text);
        o.meta.wrote.push(path);
        o
    }
}

// ── move ────────────────────────────────────────────────────────────────────

/// Rename a file or a directory inside the body's soil.
pub struct Move;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveArgs {
    from: String,
    to: String,
}

impl Tool for Move {
    fn name(&self) -> &'static str {
        "move"
    }

    fn description(&self) -> &'static str {
        "Move or rename a file or a directory inside your soil. Both ends must be yours — for a directory, every file in it — and the destination must not exist yet; missing directories on its way are made."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "from": {"type": "string", "description": "What to move, relative to the world's root."},
                "to": {"type": "string", "description": "Where it goes, relative to the world's root."}
            },
            "required": ["from", "to"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Write
    }

    fn example(&self) -> Value {
        json!({"from": "src/old.rs", "to": "src/new.rs"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: MoveArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        // The entries themselves: a link is moved as a link, never its target.
        let (from, to) = match (cx.writable_entry(&a.from), cx.writable_entry(&a.to)) {
            (Ok(f), Ok(t)) => (f, t),
            (Err(e), _) | (_, Err(e)) => return Outcome::error(e),
        };
        let (shown_from, shown_to) = (cx.shown(&from), cx.shown(&to));
        let Ok(meta) = from.symlink_metadata() else {
            return Outcome::error(format!("`{shown_from}` does not exist"));
        };
        if to.starts_with(&from) {
            return Outcome::error(format!(
                "cannot move `{shown_from}` into itself (`{shown_to}`)"
            ));
        }
        let mut moved = vec![from.clone()];
        if meta.is_dir() {
            // A folder that holds another's spot, at either end, is never moved whole: its files
            // may be the body's, the spot is not.
            for (end, shown) in [(&from, &shown_from), (&to, &shown_to)] {
                if cx.holds_spot(end) {
                    return Outcome::error(format!(
                        "`{shown}` holds another's spot; move the files that are yours one by one"
                    ));
                }
            }
            // Every file the directory holds must be the body's to move, and land where it may.
            let entries = match walkdir(&from) {
                Ok(e) => e,
                Err(dir) => {
                    return Outcome::error(format!(
                        "cannot move `{shown_from}`: `{}` cannot be read, so it cannot be checked",
                        cx.shown(&dir)
                    ));
                }
            };
            for entry in entries {
                let rel = entry.strip_prefix(&from).unwrap_or(&entry);
                let landing = to.join(rel);
                for p in [&entry, &landing] {
                    if let Err(e) = cx.writable_at(p) {
                        return Outcome::error(format!("cannot move `{shown_from}`: {e}"));
                    }
                }
                moved.push(entry);
            }
        }
        if to.symlink_metadata().is_ok() {
            return Outcome::error(format!(
                "`{shown_to}` already exists; move does not overwrite — pick another name, or delete it first"
            ));
        }
        keep(cx);
        if let Err(e) = crate::beneath::rename(cx.world(), &from, &to) {
            return Outcome::error(format!("cannot move `{shown_from}` to `{shown_to}`: {e}"));
        }
        for p in &moved {
            cx.seen.remove(p);
        }
        let mut o = Outcome::ok(
            format!("moved `{shown_from}` to `{shown_to}`"),
            ToolClass::Write,
        );
        // What the call wrote: every file it moved, at both ends.
        for p in &moved {
            let rel = p.strip_prefix(&from).unwrap_or(Path::new(""));
            o.meta.wrote.push(p.clone());
            o.meta.wrote.push(if rel.as_os_str().is_empty() {
                to.clone()
            } else {
                to.join(rel)
            });
        }
        o
    }
}

// ── delete ──────────────────────────────────────────────────────────────────

/// Delete a file inside the body's soil, a copy kept in the attic first.
pub struct Delete;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteArgs {
    path: String,
}

impl Tool for Delete {
    fn name(&self) -> &'static str {
        "delete"
    }

    fn description(&self) -> &'static str {
        "Delete a file inside your soil. A copy is kept in the attic first, so it can be restored; a directory is refused — delete its files."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "The file, relative to the world's root."}
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Write
    }

    fn example(&self) -> Value {
        json!({"path": "src/unused.rs"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: DeleteArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        // The entry itself: a link is deleted as a link, never its target.
        let path = match cx.writable_entry(&a.path) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let shown = cx.shown(&path);
        match path.symlink_metadata() {
            Err(_) => return Outcome::error(format!("`{shown}` does not exist")),
            Ok(m) if m.is_dir() => {
                // An empty directory holds nothing to lose: it goes without a copy.
                let empty = std::fs::read_dir(&path).is_ok_and(|mut d| d.next().is_none());
                if !empty {
                    return Outcome::error(format!(
                        "`{shown}` is a directory with files in it; delete its files first"
                    ));
                }
                if let Err(e) = crate::beneath::remove(cx.world(), &path, true) {
                    return Outcome::error(format!("cannot delete `{shown}`: {e}"));
                }
                let mut o = Outcome::ok(
                    format!("deleted the empty directory `{shown}`"),
                    ToolClass::Write,
                );
                o.meta.wrote.push(path);
                return o;
            }
            Ok(_) => {}
        }
        // Nothing is destroyed: without a copy in the attic, nothing is deleted.
        let Some(attic) = cx.attic() else {
            return Outcome::error(
                "delete keeps a copy in the attic first, and this body has no attic; nothing was deleted",
            );
        };
        let snap = match attic.snapshot("before a delete") {
            Ok(snap) => snap,
            Err(e) => {
                return Outcome::error(format!(
                    "the attic could not keep a copy ({e}); nothing was deleted"
                ));
            }
        };
        // The attic leaves out what the world ignores, build output and large files: a delete
        // goes ahead only when the copy is really there.
        if !attic.holds(&snap, &shown) {
            return Outcome::error(format!(
                "the attic keeps no copy of `{shown}` (ignored, build output, or too large); nothing was deleted — remove it with a command if it is yours to lose"
            ));
        }
        if let Err(e) = crate::beneath::remove(cx.world(), &path, false) {
            return Outcome::error(format!("cannot delete `{shown}`: {e}"));
        }
        cx.seen.remove(&path);
        let mut o = Outcome::ok(
            format!("deleted `{shown}`; the attic keeps a copy"),
            ToolClass::Write,
        );
        o.meta.wrote.push(path);
        o
    }
}

/// Every entry under `dir` that is not a directory — files and links, links never followed —
/// or the first directory that cannot be read.
fn walkdir(dir: &Path) -> Result<Vec<PathBuf>, PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = std::fs::read_dir(&d).map_err(|_| d.clone())?;
        for e in entries {
            let e = e.map_err(|_| d.clone())?;
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(_) => out.push(p),
                Err(_) => return Err(p),
            }
        }
    }
    Ok(out)
}

// ── glob and grep ───────────────────────────────────────────────────────────

/// Walk the world under `root`, honouring .gitignore, skipping .git.
fn walk(root: &Path) -> impl Iterator<Item = PathBuf> {
    WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .map(ignore::DirEntry::into_path)
}

/// List files matching a glob pattern.
pub struct Glob;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
}

impl Tool for Glob {
    fn name(&self) -> &'static str {
        "glob"
    }

    fn description(&self) -> &'static str {
        "List the files inside the world that match a glob pattern (`**/*.rs`), relative to `path` (default: the root). Honours .gitignore."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "A glob, such as `src/**/*.rs`."},
                "path": {"type": "string", "description": "The directory to search, relative to the world's root."}
            },
            "required": ["pattern"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({"pattern": "**/*.rs"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: GlobArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        let root = match cx.inside(a.path.as_deref().unwrap_or(".")) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let matcher = match GlobBuilder::new(&a.pattern).literal_separator(true).build() {
            Ok(g) => g.compile_matcher(),
            Err(e) => return Outcome::error(format!("`{}` is not a glob: {e}", a.pattern)),
        };
        let mut found: Vec<String> = walk(&root)
            .filter(|p| p.strip_prefix(&root).is_ok_and(|rel| matcher.is_match(rel)))
            .map(|p| cx.shown(&p))
            .collect();
        found.sort();
        let total = found.len();
        found.truncate(MAX_RESULTS);
        let mut out = if total == 0 {
            format!("no file matches `{}`", a.pattern)
        } else {
            found.join("\n")
        };
        if total > MAX_RESULTS {
            let _ = write!(out, "\n… {} more; narrow the pattern", total - MAX_RESULTS);
        }
        Outcome::ok(out, ToolClass::Read)
    }
}

/// Search file contents with a regular expression.
pub struct Grep;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrepArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    files_only: bool,
}

impl Tool for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search the text files inside the world for a regular expression; shows `path:line: text`, or only the paths with `files_only`. `glob` narrows the files. Honours .gitignore."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "A regular expression (Rust syntax)."},
                "path": {"type": "string", "description": "A file or directory to search, relative to the world's root."},
                "glob": {"type": "string", "description": "Only files whose path matches this glob, such as `*.rs`."},
                "ignore_case": {"type": "boolean"},
                "files_only": {"type": "boolean", "description": "List matching files instead of matching lines."}
            },
            "required": ["pattern"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({"pattern": "fn main", "glob": "*.rs"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: GrepArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        let root = match cx.inside(a.path.as_deref().unwrap_or(".")) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let re = match RegexBuilder::new(&a.pattern)
            .case_insensitive(a.ignore_case)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return Outcome::error(format!("`{}` is not a regular expression: {e}", a.pattern));
            }
        };
        let filter = match a.glob.as_deref().map(|g| GlobBuilder::new(g).build()) {
            Some(Ok(g)) => Some(g.compile_matcher()),
            Some(Err(e)) => {
                return Outcome::error(format!(
                    "`{}` is not a glob: {e}",
                    a.glob.unwrap_or_default()
                ));
            }
            None => None,
        };
        let mut files: Vec<PathBuf> = if root.is_file() {
            vec![root.clone()]
        } else {
            walk(&root).collect()
        };
        files.sort();
        let mut out: Vec<String> = Vec::new();
        let mut total = 0usize;
        for file in files {
            let rel = file.strip_prefix(&root).unwrap_or(&file);
            if let Some(m) = &filter
                && !(m.is_match(rel) || file.file_name().is_some_and(|n| m.is_match(n)))
            {
                continue;
            }
            let Ok(bytes) = crate::beneath::read(cx.world(), &file) else {
                continue;
            };
            if bytes.iter().take(8192).any(|b| *b == 0) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            let shown = cx.shown(&file);
            for (i, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    total += 1;
                    if a.files_only {
                        out.push(shown.clone());
                        break;
                    }
                    if out.len() < MAX_RESULTS {
                        let cut: String = line.chars().take(LINE_CHARS).collect();
                        out.push(format!("{shown}:{}: {cut}", i + 1));
                    }
                }
            }
        }
        if out.is_empty() {
            return Outcome::ok(format!("no match for `{}`", a.pattern), ToolClass::Read);
        }
        let mut text = out.join("\n");
        if !a.files_only && total > MAX_RESULTS {
            let _ = write!(
                text,
                "\n… {} more matches; narrow the pattern, the path or the glob",
                total - MAX_RESULTS
            );
        }
        Outcome::ok(text, ToolClass::Read)
    }
}
