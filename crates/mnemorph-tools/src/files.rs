//! The file tools: read, write, edit, glob, grep. Every path is resolved and checked against the
//! world (and, for writes, the soil) before anything is touched.

use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::fs;
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

/// Write `content` to `path` whole or not at all: a temporary sibling, then a rename.
fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_file_name(format!(
        ".{}.mnemorph-tmp",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    ));
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)
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
        let bytes = match fs::read(&path) {
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
        };
        let shown = cx.shown(&path);
        if cx.seen.get(&path) == Some(&seen) {
            let mut o = Outcome::ok(
                format!(
                    "`{shown}` lines {first}–{last} are unchanged since you last read them; that result still holds."
                ),
                ToolClass::Read,
            );
            o.meta.unchanged = true;
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
        if let Err(e) = write_atomic(&path, a.content.as_bytes()) {
            return Outcome::error(format!("cannot write `{}`: {e}", cx.shown(&path)));
        }
        cx.seen.remove(&path);
        let lines = a.content.lines().count();
        let mut o = Outcome::ok(
            format!("wrote `{}` ({lines} lines)", cx.shown(&path)),
            ToolClass::Write,
        );
        o.meta.wrote.push(path);
        o
    }
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
        if a.old_string.is_empty() {
            return Outcome::error(
                "`old_string` is empty; to create or replace a whole file, use write",
            );
        }
        if a.old_string == a.new_string {
            return Outcome::error(
                "`old_string` and `new_string` are the same; nothing would change",
            );
        }
        let path = match cx.writable(&a.path) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let shown = cx.shown(&path);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return Outcome::error(format!("cannot read `{shown}`: {e}")),
        };
        let at: Vec<usize> = text.match_indices(&a.old_string).map(|(i, _)| i).collect();
        let line_of = |byte: usize| text[..byte].matches('\n').count() + 1;
        match at.len() {
            0 => {
                let (n, line) = closest_line(&text, &a.old_string);
                return Outcome::error(format!(
                    "`old_string` does not occur in `{shown}`; the closest line is {n}: `{}`. Read the file and copy the text exactly.",
                    line.trim()
                ));
            }
            k if k > 1 && !a.replace_all => {
                let lines: Vec<String> = at.iter().map(|&i| line_of(i).to_string()).collect();
                return Outcome::error(format!(
                    "`old_string` occurs {k} times in `{shown}` (lines {}); add context to make it unique, or set replace_all",
                    lines.join(", ")
                ));
            }
            _ => {}
        }
        let first_line = line_of(at[0]);
        let updated = if a.replace_all {
            text.replace(&a.old_string, &a.new_string)
        } else {
            text.replacen(&a.old_string, &a.new_string, 1)
        };
        if let Err(e) = write_atomic(&path, updated.as_bytes()) {
            return Outcome::error(format!("cannot write `{shown}`: {e}"));
        }
        cx.seen.remove(&path);
        let n = if a.replace_all { at.len() } else { 1 };
        let mut o = Outcome::ok(
            format!("edited `{shown}`: {n} replacement(s), from line {first_line}"),
            ToolClass::Write,
        );
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
            let Ok(bytes) = fs::read(&file) else { continue };
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
