//! `patch`: a unified diff across files, applied whole or not at all — the form many models are
//! trained to write.

use std::path::PathBuf;

use mnemorph_races::ToolClass;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::files::keep;
use crate::tool::{Context, Outcome, Tool, args};

/// Apply a unified diff inside the body's soil.
pub struct Patch;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchArgs {
    patch: String,
}

/// One file's part of the diff.
#[derive(Debug, Default)]
struct FilePatch {
    old: Option<String>,
    new: Option<String>,
    hunks: Vec<Hunk>,
}

/// One hunk: where the old side starts (1-based), and its lines.
#[derive(Debug, Default)]
struct Hunk {
    start: usize,
    lines: Vec<(char, String)>,
    /// The last old or new line has no newline at the end.
    no_newline_old: bool,
    no_newline_new: bool,
}

/// A path as a diff header names it: `a/` and `b/` prefixes dropped, `/dev/null` as none.
fn header_path(rest: &str) -> Option<String> {
    let p = rest.split('\t').next().unwrap_or("").trim();
    if p == "/dev/null" {
        return None;
    }
    let p = p
        .strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p);
    Some(p.to_owned())
}

/// Read a unified diff into its files.
fn parse(text: &str) -> Result<Vec<FilePatch>, String> {
    let mut files: Vec<FilePatch> = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("--- ") {
            let Some(plus) = lines.next().and_then(|l| l.strip_prefix("+++ ")) else {
                return Err(format!("`--- {rest}` is not followed by a `+++` line"));
            };
            files.push(FilePatch {
                old: header_path(rest),
                new: header_path(plus),
                hunks: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix("@@ ") {
            let file = files
                .last_mut()
                .ok_or("a hunk before any `---`/`+++` header")?;
            let old = rest.split_whitespace().next().unwrap_or("");
            let start = old
                .trim_start_matches('-')
                .split(',')
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .ok_or_else(|| format!("`@@ {rest}` names no start line"))?;
            let mut hunk = Hunk {
                start,
                ..Hunk::default()
            };
            while let Some(next) = lines.peek() {
                let next = *next;
                if next.starts_with("@@ ") || next.starts_with("--- ") || next.starts_with("diff ")
                {
                    break;
                }
                lines.next();
                if next.starts_with('\\') {
                    match hunk.lines.last() {
                        Some(('-', _)) => hunk.no_newline_old = true,
                        Some(('+', _)) => hunk.no_newline_new = true,
                        _ => {
                            hunk.no_newline_old = true;
                            hunk.no_newline_new = true;
                        }
                    }
                    continue;
                }
                let (kind, body) = match next.chars().next() {
                    Some(c @ (' ' | '-' | '+')) => (c, next[1..].to_owned()),
                    // An empty line inside a hunk is a blank context line, as many writers emit it.
                    None => (' ', String::new()),
                    Some(_) => return Err(format!("`{next}` is no diff line (` `, `-`, `+`)")),
                };
                hunk.lines.push((kind, body));
            }
            file.hunks.push(hunk);
        }
    }
    if files.is_empty() {
        return Err("no `---`/`+++` file header: the patch must be a unified diff".into());
    }
    Ok(files)
}

/// Apply `hunks` to `text`: each where it says, or at the one other place its old lines match.
fn apply(text: &str, hunks: &[Hunk], shown: &str) -> Result<String, String> {
    let had_newline = text.is_empty() || text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut drift: isize = 0;
    let mut newline = had_newline;
    for (n, h) in hunks.iter().enumerate() {
        let old: Vec<&str> = h
            .lines
            .iter()
            .filter(|(k, _)| *k != '+')
            .map(|(_, l)| l.as_str())
            .collect();
        let new: Vec<String> = h
            .lines
            .iter()
            .filter(|(k, _)| *k != '-')
            .map(|(_, l)| l.clone())
            .collect();
        let fits = |at: usize| {
            at + old.len() <= lines.len()
                && lines[at..at + old.len()]
                    .iter()
                    .zip(&old)
                    .all(|(a, b)| a == b)
        };
        let stated =
            usize::try_from(isize::try_from(h.start.max(1) - 1).unwrap_or(0) + drift).unwrap_or(0);
        // A pure insertion (`@@ -5,0 …`) goes after its stated line, shifted like the rest.
        let stated = if old.is_empty() {
            usize::try_from(isize::try_from(h.start).unwrap_or(0) + drift)
                .unwrap_or(0)
                .min(lines.len())
        } else {
            stated
        };
        let at = if fits(stated) {
            stated
        } else {
            let all: Vec<usize> = (0..=lines.len().saturating_sub(old.len()))
                .filter(|&i| fits(i))
                .collect();
            match all.as_slice() {
                [one] => *one,
                [] => {
                    return Err(format!(
                        "hunk {} of `{shown}` does not apply: its old lines are not in the file. Read the file and make the diff again.",
                        n + 1
                    ));
                }
                many => {
                    return Err(format!(
                        "hunk {} of `{shown}` matches {} places; give it more context",
                        n + 1,
                        many.len()
                    ));
                }
            }
        };
        lines.splice(at..at + old.len(), new.iter().cloned());
        drift += isize::try_from(new.len()).unwrap_or(0) - isize::try_from(old.len()).unwrap_or(0);
        if h.no_newline_new {
            newline = false;
        } else if h.no_newline_old {
            newline = true;
        }
    }
    let mut out = lines.join("\n");
    if newline && !lines.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

/// What one file becomes.
enum Outcome1 {
    Write(PathBuf, String),
    Remove(PathBuf),
}

impl Tool for Patch {
    fn name(&self) -> &'static str {
        "patch"
    }

    fn description(&self) -> &'static str {
        "Apply a unified diff (`--- a/path`, `+++ b/path`, `@@` hunks) to files inside your soil: every file's result is computed first, and if any hunk does not apply nothing is written. `/dev/null` as the old side creates a file, as the new side deletes one (the attic must hold a copy)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "patch": {"type": "string", "description": "The unified diff."}
            },
            "required": ["patch"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Write
    }

    fn example(&self) -> Value {
        json!({"patch": "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-fn old() {}\n+fn new() {}\n"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: PatchArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        let files = match parse(&a.patch) {
            Ok(f) => f,
            Err(e) => return Outcome::error(e),
        };
        let plan = match plan_of(&files, cx) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let wrote = match land(plan, cx) {
            Ok(w) => w,
            Err(e) => return Outcome::error(e),
        };
        let shown: Vec<String> = wrote.iter().map(|p| cx.shown(p)).collect();
        let mut o = Outcome::ok(
            format!("patched {} file(s): {}", shown.len(), shown.join(", ")),
            ToolClass::Write,
        );
        o.meta.wrote = wrote;
        o
    }
}

/// Every file's result, before anything is written.
fn plan_of(files: &[FilePatch], cx: &Context) -> Result<Vec<Outcome1>, String> {
    let mut plan: Vec<Outcome1> = Vec::new();
    for f in files {
        let name = f
            .new
            .as_ref()
            .or(f.old.as_ref())
            .cloned()
            .unwrap_or_default();
        if f.old.is_some() && f.new.is_some() && f.old != f.new {
            return Err(format!(
                "`{}` → `{}` renames a file; patch does not rename — use move, then patch",
                f.old.as_deref().unwrap_or(""),
                f.new.as_deref().unwrap_or("")
            ));
        }
        let check = if f.new.is_none() {
            cx.writable_entry(&name)
        } else {
            cx.writable(&name)
        };
        let path = check?;
        let shown = cx.shown(&path);
        let exists = path.symlink_metadata().is_ok();
        let text = if f.old.is_none() {
            if exists {
                return Err(format!("`{shown}` already exists; the diff creates it"));
            }
            String::new()
        } else {
            match crate::beneath::read(cx.world(), &path).map(String::from_utf8) {
                Ok(Ok(t)) => t,
                Ok(Err(_)) => {
                    return Err(format!(
                        "`{shown}` is not UTF-8 text; patch would replace its bytes"
                    ));
                }
                Err(e) => return Err(format!("cannot read `{shown}`: {e}")),
            }
        };
        let result = match apply(&text, &f.hunks, &shown) {
            Ok(r) => r,
            Err(e) => return Err(format!("{e} Nothing was written.")),
        };
        if f.new.is_none() {
            if !result.is_empty() {
                return Err(format!(
                    "the diff deletes `{shown}` but leaves lines in it; nothing was written"
                ));
            }
            plan.push(Outcome1::Remove(path));
        } else {
            plan.push(Outcome1::Write(path, result));
        }
    }
    Ok(plan)
}

/// Write the plan: deletes only once the attic holds their copy; a failed write names what
/// already landed.
fn land(plan: Vec<Outcome1>, cx: &mut Context) -> Result<Vec<PathBuf>, String> {
    // Deletes need the attic's copy, asked of before anything changes.
    if plan.iter().any(|p| matches!(p, Outcome1::Remove(_))) {
        let Some(attic) = cx.attic() else {
            return Err(
                "the diff deletes a file, and this body has no attic to keep a copy; nothing was written".into(),
            );
        };
        let Ok(snap) = attic.snapshot("before a patch") else {
            return Err("the attic could not keep a copy; nothing was written".into());
        };
        for p in &plan {
            if let Outcome1::Remove(path) = p
                && !attic.holds(&snap, &cx.shown(path))
            {
                return Err(format!(
                    "the attic keeps no copy of `{}`; nothing was written",
                    cx.shown(path)
                ));
            }
        }
    } else {
        keep(cx);
    }
    let mut wrote: Vec<PathBuf> = Vec::new();
    for p in plan {
        let (path, done) = match p {
            Outcome1::Write(path, text) => {
                let r = crate::beneath::write(cx.world(), &path, text.as_bytes());
                (path, r)
            }
            Outcome1::Remove(path) => {
                let r = crate::beneath::remove(cx.world(), &path, false);
                (path, r)
            }
        };
        if let Err(e) = done {
            let landed: Vec<String> = wrote.iter().map(|p| cx.shown(p)).collect();
            return Err(format!(
                "cannot write `{}`: {e}; already written: {} — the attic holds the state before the patch",
                cx.shown(&path),
                if landed.is_empty() {
                    "nothing".into()
                } else {
                    landed.join(", ")
                }
            ));
        }
        cx.seen.remove(&path);
        wrote.push(path);
    }
    Ok(wrote)
}
