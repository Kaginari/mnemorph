//! A file's skeleton: its symbols with their line ranges, its imports and its length — what a body
//! reads first, so that it then reads only the lines it needs.
//!
//! Rust, TypeScript, JavaScript, Python and Go are parsed with tree-sitter; Markdown headings,
//! TOML tables and JSON keys by small scanners. Any other file has a line count and no skeleton.
//! A skeleton is computed once per content in a session and kept by the context.

mod plain;
mod syntax;

use std::fmt::Write as _;
use std::path::Path;

use mnemorph_races::ToolClass;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::tool::{Context, Outcome, Tool, args};

/// Symbols one skeleton shows; the rest are counted.
pub(crate) const MAX_SYMBOLS: usize = 200;
/// Nesting shown: a symbol this deep or deeper is counted on its nearest shown ancestor.
pub(crate) const MAX_DEPTH: usize = 3;
/// Imports one skeleton names; the rest are counted.
const MAX_IMPORTS: usize = 12;
/// Characters of one import shown.
const IMPORT_CHARS: usize = 60;
/// Files one index call takes.
const MAX_FILES: usize = 10;
/// A file larger than this is not parsed: its skeleton is its line count.
const MAX_BYTES: usize = 4 << 20;
/// Skeletons a session keeps before it starts again.
const CACHE: usize = 512;

/// One symbol: what it is, its name, its lines (1-based, inclusive) and how deep it nests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Symbol {
    pub(crate) kind: &'static str,
    pub(crate) name: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) depth: usize,
}

/// What a parser returns: the imports (each its line and text) and the symbols.
pub(crate) type Made = (Vec<(usize, String)>, Vec<Symbol>);

/// A file's skeleton.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Skeleton {
    /// The language it was read as; `None` when no skeleton could be made.
    pub(crate) lang: Option<&'static str>,
    pub(crate) lines: usize,
    /// Each import: its first line and its text, whitespace collapsed.
    pub(crate) imports: Vec<(usize, String)>,
    /// Every symbol, in the file's order.
    pub(crate) symbols: Vec<Symbol>,
}

/// The languages a skeleton is made for, by file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Lang {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    Markdown,
    Toml,
    Json,
}

impl Lang {
    fn of(path: &Path) -> Option<Lang> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "rs" => Lang::Rust,
            "ts" | "mts" | "cts" => Lang::TypeScript,
            "tsx" => Lang::Tsx,
            "js" | "jsx" | "mjs" | "cjs" => Lang::JavaScript,
            "py" | "pyi" => Lang::Python,
            "go" => Lang::Go,
            "md" | "markdown" => Lang::Markdown,
            "toml" => Lang::Toml,
            "json" => Lang::Json,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Lang::Rust => "rust",
            Lang::TypeScript => "typescript",
            Lang::Tsx => "tsx",
            Lang::JavaScript => "javascript",
            Lang::Python => "python",
            Lang::Go => "go",
            Lang::Markdown => "markdown",
            Lang::Toml => "toml",
            Lang::Json => "json",
        }
    }
}

/// The skeleton of `text`, read as the language `path`'s extension names.
pub(crate) fn skeleton(path: &Path, text: &str) -> Skeleton {
    let lines = text.lines().count();
    let mut s = Skeleton {
        lines,
        ..Skeleton::default()
    };
    let Some(lang) = Lang::of(path) else {
        return s;
    };
    if text.len() > MAX_BYTES {
        return s;
    }
    let made = match lang {
        Lang::Markdown => Some(plain::markdown(text)),
        Lang::Toml => Some(plain::toml(text)),
        Lang::Json => Some(plain::json(text)),
        _ => syntax::parse(lang, text),
    };
    if let Some((imports, symbols)) = made {
        s.lang = Some(lang.name());
        s.imports = imports;
        s.symbols = symbols;
    }
    s
}

/// The skeleton of a file's content, from the context's cache when the same content was indexed
/// before in this session.
pub(crate) fn cached(cx: &mut Context, path: &Path, bytes: &[u8], text: &str) -> Skeleton {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    Lang::of(path).hash(&mut h);
    bytes.hash(&mut h);
    let key = h.finish();
    if let Some(s) = cx.skeletons.get(&key) {
        return s.clone();
    }
    let s = skeleton(path, text);
    if cx.skeletons.len() >= CACHE {
        cx.skeletons.clear();
    }
    cx.skeletons.insert(key, s.clone());
    s
}

/// Collapse whitespace runs to one space.
pub(crate) fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A line of the skeleton: `start–end kind name`, indented by depth.
fn entry(out: &mut String, s: &Symbol, hidden: usize) {
    let indent = "  ".repeat(s.depth);
    let _ = write!(out, "{indent}{}–{} {} {}", s.start, s.end, s.kind, s.name);
    if hidden > 0 {
        let _ = write!(out, " (+{hidden} nested)");
    }
    out.push('\n');
}

/// The skeleton as the model reads it, at most `cap` symbols; `shown` is the file's path.
pub(crate) fn render(shown: &str, s: &Skeleton, cap: usize) -> String {
    let mut out = String::new();
    let Some(lang) = s.lang else {
        let _ = writeln!(
            out,
            "{shown} — {} lines; no skeleton for this file type: read it by range, or grep.",
            s.lines
        );
        return out;
    };
    let _ = writeln!(out, "{shown} — {lang}, {} lines", s.lines);
    if !s.imports.is_empty() {
        let first = s.imports[0].0;
        let last = s.imports[s.imports.len() - 1].0;
        let names: Vec<String> = s
            .imports
            .iter()
            .take(MAX_IMPORTS)
            .map(|(_, t)| {
                let cut: String = t.chars().take(IMPORT_CHARS).collect();
                if cut.len() < t.len() {
                    format!("{cut}…")
                } else {
                    cut
                }
            })
            .collect();
        let more = s.imports.len().saturating_sub(MAX_IMPORTS);
        let _ = write!(
            out,
            "imports {first}–{last} ({}): {}",
            s.imports.len(),
            names.join(" · ")
        );
        if more > 0 {
            let _ = write!(out, " · … {more} more");
        }
        out.push('\n');
    }
    if s.symbols.is_empty() {
        out.push_str("no symbols\n");
        return out;
    }
    // Shown symbols, each with how many deeper ones it hides.
    let mut shown_syms: Vec<(&Symbol, usize)> = Vec::new();
    let mut omitted = 0usize;
    let mut omitted_from = None;
    for sym in &s.symbols {
        if sym.depth >= MAX_DEPTH {
            // Counted on the nearest shown ancestor: the last shown symbol shallower than it.
            if let Some((_, hidden)) = shown_syms
                .iter_mut()
                .rev()
                .find(|(a, _)| a.depth < MAX_DEPTH && a.start <= sym.start && a.end >= sym.end)
            {
                *hidden += 1;
            }
            continue;
        }
        if shown_syms.len() >= cap {
            omitted += 1;
            omitted_from.get_or_insert(sym.start);
            continue;
        }
        shown_syms.push((sym, 0));
    }
    for (sym, hidden) in &shown_syms {
        entry(&mut out, sym, *hidden);
    }
    if let Some(from) = omitted_from {
        let _ = writeln!(
            out,
            "… {omitted} more symbols from line {from}: read by range from there."
        );
    }
    out
}

/// The symbols `name` picks in a skeleton: a bare name matches every symbol of that name; a
/// qualified one (`Parent::name` or `Parent.name`) only those inside a parent of that name.
pub(crate) fn find<'a>(s: &'a Skeleton, name: &str) -> Vec<&'a Symbol> {
    let (parent, leaf) = match name.rsplit_once("::").or_else(|| name.rsplit_once('.')) {
        Some((p, l)) if !p.is_empty() && !l.is_empty() => (Some(p), l),
        _ => (None, name),
    };
    let mut out: Vec<&Symbol> = Vec::new();
    for (i, sym) in s.symbols.iter().enumerate() {
        // A whole-name match (a Go method is named `Type.Name`, a heading may hold dots).
        if sym.name == name {
            out.push(sym);
            continue;
        }
        // An impl is named by its type, never picked as the type itself.
        if sym.name != leaf && (sym.kind == "impl" || short(&sym.name) != leaf) {
            continue;
        }
        let Some(p) = parent else {
            out.push(sym);
            continue;
        };
        // Its enclosing symbols, nearest first.
        let inside = s.symbols[..i]
            .iter()
            .rev()
            .filter(|a| a.depth < sym.depth && a.start <= sym.start && a.end >= sym.end);
        if inside
            .into_iter()
            .any(|a| a.name == p || short(&a.name) == p)
        {
            out.push(sym);
        }
    }
    out
}

/// The last word of a symbol's name, generics dropped: `Tool for Read` reads as `Read`,
/// `Stack<T>` as `Stack`, `Stack.push` as `push`.
fn short(name: &str) -> &str {
    let bare = name.split('<').next().unwrap_or(name).trim_end();
    let tail = bare.rsplit(' ').next().unwrap_or(bare);
    tail.rsplit('.').next().unwrap_or(tail)
}

// ── index ───────────────────────────────────────────────────────────────────

/// A file's skeleton, for the model.
pub struct Index;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexArgs {
    path: String,
    #[serde(default)]
    paths: Vec<String>,
}

impl Tool for Index {
    fn name(&self) -> &'static str {
        "index"
    }

    fn description(&self) -> &'static str {
        "A file's skeleton before its text: its symbols (fn, struct, enum, impl, trait, mod, class, method, interface, const…; Markdown headings; TOML tables; JSON keys) each with its line range `start–end`, its imports and its line count. Index a file first, then read only the lines you need: `read` with `offset` and `end`, or `path#symbol`. Rust, TypeScript, JavaScript, Python, Go, Markdown, TOML and JSON; other files show their line count."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "The file, relative to the world's root."},
                "paths": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_FILES - 1, "description": "More files, indexed after `path`."}
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({"path": "src/main.rs"})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: IndexArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        let mut paths = a.paths;
        paths.insert(0, a.path);
        if paths.len() > MAX_FILES {
            return Outcome::error(format!(
                "index takes at most {MAX_FILES} files a call; you named {}",
                paths.len()
            ));
        }
        let mut out = String::new();
        let mut failed = 0;
        for text in &paths {
            match one(cx, text) {
                Ok(s) => out.push_str(&s),
                Err(e) => {
                    failed += 1;
                    let _ = writeln!(out, "{e}");
                }
            }
        }
        if failed == paths.len() {
            return Outcome::error(out.trim_end().to_owned());
        }
        Outcome::ok(out.trim_end().to_owned(), ToolClass::Read)
    }
}

/// One file's rendered skeleton, or why there is none.
fn one(cx: &mut Context, text: &str) -> Result<String, String> {
    let path = cx.inside(text)?;
    let shown = cx.shown(&path);
    let bytes = crate::beneath::read(cx.world(), &path)
        .map_err(|e| format!("cannot index `{shown}`: {e}"))?;
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Err(format!("`{shown}` is a binary file; index reads text"));
    }
    let text = String::from_utf8_lossy(&bytes);
    let s = cached(cx, &path, &bytes, &text);
    Ok(render(&shown, &s, MAX_SYMBOLS))
}

#[cfg(test)]
mod tests;
