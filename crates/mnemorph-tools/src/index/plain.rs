//! Skeletons by small scanners: Markdown headings, TOML tables, JSON keys.

use super::{Made, Symbol};

/// The last line from `start` up to `before` (exclusive) that is not blank, at least `start`.
fn trim_end(lines: &[&str], start: usize, before: usize) -> usize {
    let mut end = before.saturating_sub(1).max(start);
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    end
}

/// Markdown's ATX headings outside code fences, each spanning to the next heading of its level or
/// higher; nested by level.
pub(super) fn markdown(text: &str) -> Made {
    let lines: Vec<&str> = text.lines().collect();
    let mut heads: Vec<(usize, usize, String)> = Vec::new(); // (line, level, title)
    let mut fence: Option<char> = None;
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim_start();
        if raw.len() - line.len() > 3 {
            continue;
        }
        if line.starts_with("```") || line.starts_with("~~~") {
            let c = line.chars().next().unwrap_or('`');
            match fence {
                Some(f) if f == c => fence = None,
                None => fence = Some(c),
                Some(_) => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let level = line.chars().take_while(|c| *c == '#').count();
        if !(1..=6).contains(&level) {
            continue;
        }
        let rest = &line[level..];
        if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
            continue;
        }
        let title = rest.trim().trim_end_matches('#').trim().to_owned();
        heads.push((i + 1, level, title));
    }
    let mut symbols = Vec::new();
    let mut open: Vec<usize> = Vec::new(); // levels of the enclosing headings
    for (k, (line, level, title)) in heads.iter().enumerate() {
        let next = heads[k + 1..]
            .iter()
            .find(|(_, l, _)| l <= level)
            .map_or(lines.len() + 1, |(n, _, _)| *n);
        while open.last().is_some_and(|l| l >= level) {
            open.pop();
        }
        symbols.push(Symbol {
            kind: KINDS[level - 1],
            name: title.clone(),
            start: *line,
            end: trim_end(&lines, *line, next),
            depth: open.len(),
        });
        open.push(*level);
    }
    (Vec::new(), symbols)
}

/// A heading's kind, by level.
const KINDS: [&str; 6] = ["#", "##", "###", "####", "#####", "######"];

/// TOML's table headers (`[a.b]`, `[[a]]`), each spanning to the next header; outside multi-line
/// strings.
pub(super) fn toml(text: &str) -> Made {
    let lines: Vec<&str> = text.lines().collect();
    let mut heads: Vec<(usize, &'static str, String)> = Vec::new();
    let mut in_string = false;
    for (i, raw) in lines.iter().enumerate() {
        let quotes = raw.matches("\"\"\"").count() + raw.matches("'''").count();
        if in_string {
            if quotes % 2 == 1 {
                in_string = false;
            }
            continue;
        }
        if quotes % 2 == 1 {
            in_string = true;
        }
        let line = raw.trim();
        let line = line.split_once(" #").map_or(line, |(l, _)| l).trim_end();
        let (kind, inner) =
            if let Some(inner) = line.strip_prefix("[[").and_then(|l| l.strip_suffix("]]")) {
                ("array", inner)
            } else if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                ("table", inner)
            } else {
                continue;
            };
        heads.push((i + 1, kind, inner.trim().to_owned()));
    }
    let symbols = heads
        .iter()
        .enumerate()
        .map(|(k, (line, kind, name))| {
            let next = heads.get(k + 1).map_or(lines.len() + 1, |h| h.0);
            Symbol {
                kind,
                name: name.clone(),
                start: *line,
                end: trim_end(&lines, *line, next),
                depth: 0,
            }
        })
        .collect();
    (Vec::new(), symbols)
}

/// A container open in the JSON scan.
struct Frame {
    object: bool,
    /// The next string in this object is a key.
    expect_key: bool,
    /// The symbol of the key whose value is being read.
    open: Option<usize>,
    /// Every container from the root to this one is an object: its keys are symbols.
    keyed: bool,
}

/// JSON's keys, every object from the root down, each spanning its value.
pub(super) fn json(text: &str) -> Made {
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut line = 1usize;
    // The line of the last character that was not white space.
    let mut seen = 1usize;
    let mut chars = text.chars();
    let close = |symbols: &mut Vec<Symbol>, frame: &mut Frame, end: usize| {
        if let Some(i) = frame.open.take() {
            symbols[i].end = end;
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            '"' => {
                let start = line;
                let mut s = String::new();
                let mut escaped = false;
                for c in chars.by_ref() {
                    if c == '\n' {
                        line += 1;
                    }
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                        continue;
                    } else if c == '"' {
                        break;
                    }
                    if s.len() < 200 {
                        s.push(c);
                    }
                }
                seen = line;
                if let Some(top) = stack.last_mut()
                    && top.object
                    && top.expect_key
                {
                    top.expect_key = false;
                    if top.keyed {
                        top.open = Some(symbols.len());
                        symbols.push(Symbol {
                            kind: "key",
                            name: s,
                            start,
                            end: start,
                            depth: stack.len() - 1,
                        });
                    }
                }
            }
            '{' | '[' => {
                let object = c == '{';
                let keyed = object && stack.last().is_none_or(|p| p.keyed);
                stack.push(Frame {
                    object,
                    expect_key: object,
                    open: None,
                    keyed,
                });
                seen = line;
            }
            '}' | ']' => {
                if let Some(mut f) = stack.pop() {
                    close(&mut symbols, &mut f, seen);
                }
                seen = line;
            }
            ',' => {
                if let Some(top) = stack.last_mut()
                    && top.object
                {
                    close(&mut symbols, top, seen);
                    top.expect_key = true;
                }
            }
            c if c.is_whitespace() || c == ':' => {}
            _ => seen = line,
        }
    }
    (Vec::new(), symbols)
}
