//! `patch`: a unified diff across files, applied whole or not at all — the form many models are
//! trained to write.

use std::collections::BTreeMap;
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

/// What a `diff --git` block carries that a text patch cannot apply.
fn git_only(line: &str) -> Option<&'static str> {
    if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
        Some("a binary change — write the file whole")
    } else if line.starts_with("old mode ") || line.starts_with("new mode ") {
        Some("a mode change — patch changes text, not modes")
    } else {
        None
    }
}

/// Read a unified diff into its files. A `--- ` line is a header only when a `+++ ` line follows
/// it, so a removed `-- comment` stays a removed line; blank lines that trail a hunk are the
/// writer's separators, not context; a `diff --git` block with no text to apply is refused by
/// name, never skipped.
fn parse(text: &str) -> Result<Vec<FilePatch>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let header = |i: usize| {
        lines[i].starts_with("--- ") && lines.get(i + 1).is_some_and(|n| n.starts_with("+++ "))
    };
    let mut files: Vec<FilePatch> = Vec::new();
    // The open `diff --git` block: its first line, and whether it brought `---`/`+++`.
    let mut block: Option<(&str, bool)> = None;
    let close = |block: Option<(&str, bool)>| match block {
        Some((first, false)) => Err(format!(
            "`{first}` carries no text hunks (an empty file, a pure rename or a copy): use write or move"
        )),
        _ => Ok(()),
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("diff --git ") {
            close(block)?;
            block = Some((line, false));
        } else if let Some(why) = git_only(line) {
            return Err(format!("the diff holds {why}"));
        } else if header(i) {
            files.push(FilePatch {
                old: header_path(&line[4..]),
                new: header_path(&lines[i + 1][4..]),
                hunks: Vec::new(),
            });
            if let Some(b) = block.as_mut() {
                b.1 = true;
            }
            i += 2;
            continue;
        } else if let Some(rest) = line.strip_prefix("@@ ") {
            let file = files
                .last_mut()
                .ok_or("a hunk before any `---`/`+++` header")?;
            let start = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches('-')
                .split(',')
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .ok_or_else(|| format!("`@@ {rest}` names no start line"))?;
            let mut hunk = Hunk {
                start,
                ..Hunk::default()
            };
            i += 1;
            while i < lines.len()
                && !lines[i].starts_with("@@ ")
                && !lines[i].starts_with("diff --git ")
                && !header(i)
            {
                hunk_line(lines[i], &mut hunk)?;
                i += 1;
            }
            while hunk
                .lines
                .last()
                .is_some_and(|(k, l)| *k == ' ' && l.is_empty())
            {
                hunk.lines.pop();
            }
            file.hunks.push(hunk);
            continue;
        }
        i += 1;
    }
    close(block)?;
    if files.is_empty() {
        return Err("no `---`/`+++` file header: the patch must be a unified diff".into());
    }
    Ok(files)
}

/// One line of a hunk's body.
fn hunk_line(line: &str, hunk: &mut Hunk) -> Result<(), String> {
    if line.starts_with('\\') {
        match hunk.lines.last() {
            Some(('-', _)) => hunk.no_newline_old = true,
            Some(('+', _)) => hunk.no_newline_new = true,
            _ => {
                hunk.no_newline_old = true;
                hunk.no_newline_new = true;
            }
        }
        return Ok(());
    }
    let (kind, body) = match line.chars().next() {
        Some(c @ (' ' | '-' | '+')) => (c, line[1..].to_owned()),
        // An empty line inside a hunk is a blank context line, as many writers emit it.
        None => (' ', String::new()),
        Some(_) => return Err(format!("`{line}` is no diff line (` `, `-`, `+`)")),
    };
    hunk.lines.push((kind, body));
    Ok(())
}

/// Apply `hunks` to `text`: each where it says, or at the one other place its old lines match.
/// A file written with `\r\n` keeps them.
fn apply(text: &str, hunks: &[Hunk], shown: &str) -> Result<String, String> {
    let crlf = text.contains("\r\n");
    let text = if crlf {
        text.replace("\r\n", "\n")
    } else {
        text.to_owned()
    };
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
        let shift =
            |base: usize| usize::try_from(isize::try_from(base).unwrap_or(0) + drift).unwrap_or(0);
        // A pure insertion (`@@ -5,0 …`) goes after its stated line, shifted like the rest.
        let stated = if old.is_empty() {
            shift(h.start).min(lines.len())
        } else {
            shift(h.start.max(1) - 1)
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
    Ok(if crlf { out.replace('\n', "\r\n") } else { out })
}

/// What one file becomes, and what it was, so a failure can put it back.
enum Change {
    /// Write `text`; `before` is the file's content, `None` when the patch creates it.
    Write {
        path: PathBuf,
        text: String,
        before: Option<String>,
    },
    /// Remove the file, whose content was `before`.
    Remove { path: PathBuf, before: String },
}

impl Change {
    fn path(&self) -> &PathBuf {
        match self {
            Change::Write { path, .. } | Change::Remove { path, .. } => path,
        }
    }
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
        let plan = match parse(&a.patch).and_then(|files| plan_of(&files, cx)) {
            Ok(p) => p,
            Err(e) => return Outcome::error(e),
        };
        let wrote = match land(&plan, cx) {
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

/// Every file's result, before anything is written. A path named twice gets its later parts
/// applied to the earlier ones' result; a file left as it was is no change.
fn plan_of(files: &[FilePatch], cx: &Context) -> Result<Vec<Change>, String> {
    let mut plan: Vec<Change> = Vec::new();
    let mut at: BTreeMap<PathBuf, usize> = BTreeMap::new();
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
        let path = if f.new.is_none() {
            cx.writable_entry(&name)
        } else {
            cx.writable(&name)
        }?;
        let shown = cx.shown(&path);
        if let Some(&i) = at.get(&path) {
            let Change::Write { text, .. } = &mut plan[i] else {
                return Err(format!(
                    "`{shown}` is deleted, then named again; nothing was written"
                ));
            };
            if f.old.is_none() || f.new.is_none() {
                return Err(format!(
                    "`{shown}` is named twice, once to create or delete it; nothing was written"
                ));
            }
            *text =
                apply(text, &f.hunks, &shown).map_err(|e| format!("{e} Nothing was written."))?;
            continue;
        }
        let exists = path.symlink_metadata().is_ok();
        let before = if f.old.is_none() {
            if exists {
                return Err(format!("`{shown}` already exists; the diff creates it"));
            }
            None
        } else {
            match crate::beneath::read(cx.world(), &path).map(String::from_utf8) {
                Ok(Ok(t)) => Some(t),
                Ok(Err(_)) => {
                    return Err(format!(
                        "`{shown}` is not UTF-8 text; patch would replace its bytes — rewrite it whole with write"
                    ));
                }
                Err(e) => {
                    return Err(format!(
                        "cannot read `{shown}`: {e} — to create it, the diff's old side is `/dev/null`"
                    ));
                }
            }
        };
        let result = apply(before.as_deref().unwrap_or(""), &f.hunks, &shown)
            .map_err(|e| format!("{e} Nothing was written."))?;
        at.insert(path.clone(), plan.len());
        if f.new.is_none() {
            if !result.is_empty() {
                return Err(format!(
                    "the diff deletes `{shown}` but leaves lines in it; nothing was written"
                ));
            }
            plan.push(Change::Remove {
                path,
                before: before.unwrap_or_default(),
            });
        } else {
            plan.push(Change::Write {
                path,
                text: result,
                before,
            });
        }
    }
    plan.retain(|c| !matches!(c, Change::Write { text, before: Some(b), .. } if text == b));
    if plan.is_empty() {
        return Err("the patch leaves every file as it was; nothing was written".into());
    }
    Ok(plan)
}

/// Write the plan: deletes only once the attic holds their copy; on a failed write every file
/// already changed is put back from the plan's own originals, so a failure leaves the tree as it
/// was.
fn land(plan: &[Change], cx: &mut Context) -> Result<Vec<PathBuf>, String> {
    if plan.iter().any(|c| matches!(c, Change::Remove { .. })) {
        let Some(attic) = cx.attic() else {
            return Err(
                "the diff deletes a file, and this body has no attic to keep a copy; nothing was written — delete it with a command if it is yours to lose".into(),
            );
        };
        let Ok(snap) = attic.snapshot("before a patch") else {
            return Err("the attic could not keep a copy; nothing was written".into());
        };
        for c in plan {
            if let Change::Remove { path, .. } = c
                && !attic.holds(&snap, &cx.shown(path))
            {
                return Err(format!(
                    "the attic keeps no copy of `{}`; nothing was written — delete it with a command if it is yours to lose",
                    cx.shown(path)
                ));
            }
        }
    } else {
        keep(cx);
    }
    let mut done: Vec<&Change> = Vec::new();
    for c in plan {
        let result = match c {
            Change::Write { path, text, .. } => {
                crate::beneath::write(cx.world(), path, text.as_bytes())
            }
            Change::Remove { path, .. } => crate::beneath::remove(cx.world(), path, false),
        };
        if let Err(e) = result {
            let unwound = done.iter().rev().all(|d| undo(d, cx));
            return Err(format!(
                "cannot write `{}`: {e}; {}",
                cx.shown(c.path()),
                if unwound {
                    "every file was put back as it was".to_owned()
                } else {
                    "some files could not be put back — read them before going on".to_owned()
                }
            ));
        }
        done.push(c);
    }
    let wrote: Vec<PathBuf> = plan.iter().map(|c| c.path().clone()).collect();
    for p in &wrote {
        cx.seen.remove(p);
    }
    Ok(wrote)
}

/// Put one landed change back: the original content, or no file where the patch made one.
fn undo(c: &Change, cx: &Context) -> bool {
    match c {
        Change::Write {
            path,
            before: Some(b),
            ..
        }
        | Change::Remove { path, before: b } => {
            crate::beneath::write(cx.world(), path, b.as_bytes()).is_ok()
        }
        Change::Write {
            path, before: None, ..
        } => crate::beneath::remove(cx.world(), path, false).is_ok(),
    }
}
