//! The forgiving rungs of `edit`: when `old_string` does not occur exactly, the same text is looked
//! for with line endings, then trailing whitespace, then inner whitespace, then indentation
//! forgiven, in that order. A rung is taken only when exactly one region of the file matches it;
//! several stop the ladder and name their lines, since picking one would be a guess. A taken rung
//! says what it forgave and where, so the body learns it and the journal keeps it. There is no
//! first-and-last-line anchoring: it can match a region that only looks right at its ends.
//! Whitespace that carries meaning is never forgiven: a run inside a quoted string, and trailing
//! spaces in Markdown (a hard line break).

/// The most line comparisons a rung may make (file lines × `old_string` lines); past it the edit
/// stays strict, so a huge file never costs a quadratic search.
const MAX_WORK: usize = 20_000_000;

/// How a rung compares a file line with an `old_string` line.
type Same<'a> = &'a dyn Fn(&str, &str) -> bool;

/// What a forgiving rung made of the file.
pub(crate) struct Forgiven {
    /// The file with the one region replaced.
    pub text: String,
    /// The region's first line, 1-based.
    pub first_line: usize,
    /// What was forgiven, and where: `indentation (+4 spaces) at lines 120–131`.
    pub note: String,
}

/// Where the ladder ended.
pub(crate) enum Ladder {
    /// Exactly one region matched at a rung.
    One(Forgiven),
    /// Several regions matched at `rung`, starting at `lines`; nothing is forgiven.
    Many {
        rung: &'static str,
        lines: Vec<usize>,
    },
    /// No rung matched.
    Nothing,
}

/// Climb the ladder for `old` → `new` in `text`, whose exact search found nothing; `markdown` keeps
/// trailing whitespace strict.
pub(crate) fn ladder(text: &str, old: &str, new: &str, markdown: bool) -> Ladder {
    if let Some(l) = line_endings(text, old, new) {
        return l;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let old_n = old.replace("\r\n", "\n");
    let ends = old_n.ends_with('\n');
    let olds: Vec<&str> = old_n
        .strip_suffix('\n')
        .unwrap_or(&old_n)
        .split('\n')
        .collect();
    if olds.len() > lines.len() || lines.len().saturating_mul(olds.len()) > MAX_WORK {
        return Ladder::Nothing;
    }
    let new_n = new.replace("\r\n", "\n");
    // The replacement's lines; when `old_string` ends with its newline, so does each new line.
    let news: Vec<&str> = if new_n.is_empty() {
        Vec::new()
    } else if ends {
        new_n
            .strip_suffix('\n')
            .unwrap_or(&new_n)
            .split('\n')
            .collect()
    } else {
        new_n.split('\n').collect()
    };
    let trailing = |a: &str, b: &str| !markdown && a.trim_end() == b.trim_end();
    let inner = |a: &str, b: &str| matches!((squeeze(a, markdown), squeeze(b, markdown)), (Some(x), Some(y)) if x == y);
    let rungs: [(&'static str, Same<'_>); 2] = [
        ("trailing whitespace", &trailing),
        ("inner whitespace", &inner),
    ];
    for (rung, same) in rungs {
        let found = windows(&lines, &olds, same);
        match found.as_slice() {
            [] => {}
            [at] => {
                let replaced: Vec<String> = news.iter().map(|&l| l.to_owned()).collect();
                return Ladder::One(splice(&lines, *at, olds.len(), &replaced, rung));
            }
            many => return many_of(rung, many),
        }
    }
    indentation(&lines, &olds, &news, markdown)
}

/// The first rung: the text matches once its `\r\n` are read as `\n`; the replacement takes the
/// file's line endings.
fn line_endings(text: &str, old: &str, new: &str) -> Option<Ladder> {
    if !text.contains('\r') && !old.contains('\r') {
        return None;
    }
    // The file with every `\r` of a `\r\n` dropped, and where each was, in the dropped text.
    let mut flat = String::with_capacity(text.len());
    let mut dropped: Vec<usize> = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c == '\r' && chars.peek().is_some_and(|&(_, n)| n == '\n') {
            dropped.push(flat.len());
        } else {
            flat.push(c);
        }
    }
    let old_n = old.replace("\r\n", "\n");
    let at: Vec<usize> = flat.match_indices(&old_n).map(|(i, _)| i).collect();
    let line_of = |byte: usize| flat[..byte].matches('\n').count() + 1;
    match at.as_slice() {
        [] => None,
        [one] => {
            let real = |n: usize| n + dropped.partition_point(|&p| p < n);
            let (start, end) = (real(*one), real(one + old_n.len()));
            let crlf = text.contains("\r\n");
            let mut put = new.replace("\r\n", "\n");
            if crlf {
                put = put.replace('\n', "\r\n");
            }
            let first = line_of(*one);
            let last = first + old_n.trim_end_matches('\n').matches('\n').count();
            Some(Ladder::One(Forgiven {
                text: format!("{}{put}{}", &text[..start], &text[end..]),
                first_line: first,
                note: format!(
                    "line endings ({}) at {}",
                    if crlf { "CRLF" } else { "LF" },
                    span(first, last)
                ),
            }))
        }
        many => Some(Ladder::Many {
            rung: "line endings",
            lines: many.iter().map(|&i| line_of(i)).collect(),
        }),
    }
}

/// A line with every run of spaces and tabs after its indentation read as one space, and none at
/// its end (kept as it is in `markdown`); the indentation is kept as it is. `None` when a run that
/// would change sits inside a quote (an odd count of `"`, `'` or `` ` `` before it on the line):
/// a string's spaces are its content.
fn squeeze(l: &str, markdown: bool) -> Option<String> {
    let t = l.trim_end();
    let lead = t.len() - t.trim_start_matches([' ', '\t']).len();
    let mut s = t[..lead].to_owned();
    let mut run = String::new();
    let mut quotes = [0usize; 3];
    for c in t[lead..].chars() {
        if c == ' ' || c == '\t' {
            run.push(c);
            continue;
        }
        if !run.is_empty() {
            if run != " " && quotes.iter().any(|q| q % 2 == 1) {
                return None;
            }
            s.push(' ');
            run.clear();
        }
        if let Some(q) = ['"', '\'', '`'].iter().position(|&q| q == c) {
            quotes[q] += 1;
        }
        s.push(c);
    }
    if markdown {
        s.push_str(&l[t.len()..]);
    }
    Some(s)
}

/// A file line without the `\r` of its `\r\n`.
fn body(l: &str) -> &str {
    l.strip_suffix('\r').unwrap_or(l)
}

/// Every start in `lines` where `olds` match line by line under `same`. The first line is compared
/// first, so a search stays near linear unless the file repeats it.
fn windows(lines: &[&str], olds: &[&str], same: impl Fn(&str, &str) -> bool) -> Vec<usize> {
    (0..=lines.len() - olds.len())
        .filter(|&i| {
            olds.iter()
                .enumerate()
                .all(|(j, o)| same(body(lines[i + j]), o))
        })
        .collect()
}

fn many_of(rung: &'static str, at: &[usize]) -> Ladder {
    Ladder::Many {
        rung,
        lines: at.iter().map(|i| i + 1).collect(),
    }
}

/// `lines 3–5`, or `line 3`.
fn span(first: usize, last: usize) -> String {
    if last > first {
        format!("lines {first}–{last}")
    } else {
        format!("line {first}")
    }
}

/// Replace `k` lines at `at` with `news`, each ending as the file's lines there end.
fn splice(lines: &[&str], at: usize, k: usize, news: &[String], rung: &str) -> Forgiven {
    let crlf = lines[at].ends_with('\r');
    let last_cr = lines[at + k - 1].ends_with('\r');
    let mut out: Vec<String> = lines[..at].iter().map(|&l| l.to_owned()).collect();
    for (j, l) in news.iter().enumerate() {
        let cr = if j + 1 == news.len() { last_cr } else { crlf };
        out.push(if cr { format!("{l}\r") } else { l.clone() });
    }
    out.extend(lines[at + k..].iter().map(|&l| l.to_owned()));
    Forgiven {
        text: out.join("\n"),
        first_line: at + 1,
        note: format!("{rung} at {}", span(at + 1, at + k)),
    }
}

/// How a set of lines is indented: the common indentation (in characters), whether it is tabs,
/// and the step between levels (0 when every line is at one level).
struct Indent {
    common: usize,
    tabs: bool,
    unit: usize,
}

/// The leading spaces or tabs of a line.
fn lead(l: &str) -> &str {
    &l[..l.len() - l.trim_start_matches([' ', '\t']).len()]
}

/// The indentation of the non-blank `lines`; `None` when tabs and spaces are mixed.
fn indent_of(lines: &[&str]) -> Option<Indent> {
    let leads: Vec<&str> = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| lead(l))
        .collect();
    let tabs = leads.iter().any(|l| l.contains('\t'));
    let spaces = leads.iter().any(|l| l.contains(' '));
    if tabs && spaces {
        return None;
    }
    let common = leads.iter().map(|l| l.len()).min().unwrap_or(0);
    let unit = leads.iter().map(|l| l.len().abs_diff(common)).fold(0, gcd);
    Some(Indent { common, tabs, unit })
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// A line's level above `ind`'s common indentation.
fn level(l: &str, ind: &Indent) -> usize {
    match ind.unit {
        0 => 0,
        u => (lead(l).len() - ind.common) / u,
    }
}

/// The last rung: the same lines at another indentation, matched level by level; the replacement
/// is moved by the same amount, in the file's own indentation (tabs or spaces). Levels are read
/// from `old_string` alone; a replacement line between levels keeps its extra spaces in a
/// space-indented file and stays strict in a tab-indented one.
fn indentation(lines: &[&str], olds: &[&str], news: &[&str], markdown: bool) -> Ladder {
    let Some(model) = indent_of(olds) else {
        return Ladder::Nothing;
    };
    let mut found: Vec<(usize, Indent)> = Vec::new();
    let same = |a: &str, b: &str| {
        if markdown {
            a.trim_start() == b.trim_start()
        } else {
            a.trim() == b.trim()
        }
    };
    for i in windows(lines, olds, same) {
        let window: Vec<&str> = lines[i..i + olds.len()].iter().map(|l| body(l)).collect();
        let Some(file) = indent_of(&window) else {
            continue;
        };
        let same = window
            .iter()
            .zip(olds)
            .filter(|(f, _)| !f.trim().is_empty())
            .all(|(f, o)| level(f, &file) == level(o, &model));
        if same {
            found.push((i, file));
        }
    }
    let (at, file) = match found.len() {
        0 => return Ladder::Nothing,
        1 => found.remove(0),
        _ => {
            return many_of(
                "indentation",
                &found.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            );
        }
    };
    // The replacement's step: `old_string`'s, else the one its lines and the replacement's share.
    let unit = match model.unit {
        0 => news
            .iter()
            .chain(olds)
            .filter(|l| !l.trim().is_empty())
            .map(|l| lead(l).len().abs_diff(model.common))
            .fold(0, gcd),
        u => u,
    };
    // The file's step: the window's, else the file's first indented line's style, else the model's.
    let tabs = file.tabs
        || (file.common == 0
            && file.unit == 0
            && lines
                .iter()
                .find(|l| !lead(body(l)).is_empty())
                .is_some_and(|l| l.starts_with('\t')));
    let step = match (tabs, file.unit, unit) {
        (true, _, _) => 1,
        (false, 0, 0) => 4,
        (false, 0, m) if !model.tabs => m,
        (false, 0, _) => 4,
        (false, f, _) => f,
    };
    let pad = if tabs { "\t" } else { " " };
    let put: Option<Vec<String>> = news
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                return Some(String::new());
            }
            let w = lead(l).len();
            let rest = l.trim_start_matches([' ', '\t']);
            let (levels, extra) = match unit {
                0 => (0, 0),
                u => (w.abs_diff(model.common) / u, w.abs_diff(model.common) % u),
            };
            // A line between levels keeps its extra spaces; tabs cannot hold them.
            if extra > 0 && tabs {
                return None;
            }
            let width = if w >= model.common {
                file.common + levels * step + extra
            } else {
                file.common.checked_sub(levels * step + extra)?
            };
            Some(format!("{}{rest}", pad.repeat(width)))
        })
        .collect();
    let Some(put) = put else {
        return Ladder::Nothing;
    };
    let what = shift(&model, &file, tabs);
    let mut f = splice(lines, at, olds.len(), &put, "indentation");
    f.note = format!("indentation ({what}) at {}", span(at + 1, at + olds.len()));
    Ladder::One(f)
}

/// The move from the model's indentation to the file's, in words: `+4 spaces`, `-1 tab`, or a
/// change of style.
fn shift(model: &Indent, file: &Indent, tabs: bool) -> String {
    let unit_of = |n: usize, tabs: bool| match (n, tabs) {
        (1, true) => "tab".to_owned(),
        (_, true) => "tabs".to_owned(),
        (1, false) => "space".to_owned(),
        (_, false) => "spaces".to_owned(),
    };
    if model.tabs == tabs && (model.unit == file.unit || model.unit == 0 || file.unit == 0) {
        let d = file.common.abs_diff(model.common);
        let sign = if file.common >= model.common {
            '+'
        } else {
            '-'
        };
        format!("{sign}{d} {}", unit_of(d, tabs))
    } else {
        let style = |i: &Indent, tabs: bool| {
            if tabs {
                "tabs".to_owned()
            } else {
                format!("{}-space levels", i.unit.max(1))
            }
        };
        format!(
            "{} as {}, at {} {}",
            style(model, model.tabs),
            style(file, tabs),
            file.common,
            unit_of(file.common, tabs)
        )
    }
}
