//! How each entry of the transcript reads: the human's line, the body's Markdown, tool calls
//! as cards with their diffs and folded output, the end gate's verdict. A pure function of the
//! entry, the one before it, the theme and the width.

use std::fmt::Write as _;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

use crate::diff::{self, Change, DiffLine};
use crate::kit::markdown;
use crate::kit::style::{Border, Sides, Style as Boxed};
use crate::kit::text::{Align, Lines, clip, fit, prefixed, wrap_text};
use crate::safe::{clean, one_line};
use crate::theme::Theme;
use crate::transcript::{Call, Entry};

/// What a block is drawn with.
#[derive(Debug, Clone, Copy)]
pub struct Ctx<'a> {
    /// The theme.
    pub theme: Theme,
    /// The transcript column's width.
    pub width: usize,
    /// The spinner's frame number.
    pub frame: usize,
    /// The spinner's glyph.
    pub spinner: &'static str,
    /// The session body's name.
    pub me: &'a str,
}

/// Output lines a done call shows before it folds.
pub const OUTPUT_LINES: usize = 4;
/// Diff lines a card shows before it folds.
pub const DIFF_LINES: usize = 12;
/// The indent of everything the body says or does under its name.
const INDENT: usize = 2;

/// An entry's lines; `prev` is the entry before it, which sets the space above.
#[must_use]
pub fn render(entry: &Entry, prev: Option<&Entry>, ctx: &Ctx<'_>) -> Lines {
    let t = &ctx.theme;
    let w = ctx.width.max(12);
    let gap = || Line::from("");
    let mut out: Lines = Vec::new();
    match entry {
        Entry::User(text) => {
            if prev.is_some() {
                out.push(gap());
            }
            let first = [
                Span::styled("▸ ", t.holo()),
                Span::styled("you", t.text().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
            ];
            let body = wrap_text(&clean(text), w.saturating_sub(7), t.text());
            out.extend(prefixed(body, &first, &[Span::raw(" ".repeat(7))]));
        }
        Entry::Assistant { text, head } => {
            if *head {
                out.push(gap());
                out.push(Line::from(vec![
                    Span::styled("◆ ", t.rimuru()),
                    Span::styled(ctx.me.to_owned(), t.rimuru()),
                ]));
            } else if matches!(prev, Some(Entry::Tool(_) | Entry::Verdict { .. })) {
                out.push(gap());
            }
            // The renderer makes each parsed text safe: entities decode before they are escaped.
            let md = markdown::render(text, w - INDENT, t);
            out.extend(indented(md));
        }
        Entry::Tool(call) => out.extend(indented(card(call, w - INDENT, ctx))),
        Entry::Verdict {
            verdict,
            changed,
            failures,
        } => out.extend(indented(verdict_line(
            verdict,
            *changed,
            failures,
            w - INDENT,
            t,
        ))),
        Entry::Note(text) => out.extend(note(
            "· ",
            text,
            w,
            t.muted().add_modifier(Modifier::ITALIC),
        )),
        Entry::Warning(text) => out.extend(note("! ", text, w, t.gold())),
        Entry::Ending(ending) => {
            let style = if ending == "interrupted" {
                t.muted()
            } else {
                t.alarm()
            };
            out.extend(note("— ", ending, w, style));
        }
        Entry::Info(text) => out.extend(wrap_text(&clean(text), w, t.muted())),
        Entry::Below { body, text, alarm } => {
            let style = if *alarm { t.alarm() } else { t.muted() };
            let first = [
                Span::styled("  ↳ ", t.muted()),
                Span::styled(one_line(body), t.frame().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
            ];
            let pad = 6 + crate::kit::text::cells(&one_line(body));
            let lines = wrap_text(&one_line(text), w.saturating_sub(pad), style);
            out.extend(prefixed(lines, &first, &[Span::raw(" ".repeat(pad))]));
        }
    }
    out
}

fn indented(lines: Lines) -> Lines {
    let pad = [Span::raw(" ".repeat(INDENT))];
    prefixed(lines, &pad, &pad)
}

fn note(mark: &str, text: &str, w: usize, style: Style) -> Lines {
    let lines = wrap_text(&clean(text), w.saturating_sub(INDENT + 2), style);
    let first = [Span::raw("  "), Span::styled(mark.to_owned(), style)];
    prefixed(lines, &first, &[Span::raw("    ")])
}

fn verdict_line(verdict: &str, changed: usize, failures: &[String], w: usize, t: &Theme) -> Lines {
    let (mark, words, style) = match verdict {
        "PASS" => ("✓ ", "end gate passed".to_owned(), t.ok()),
        "NOT_VERIFIED" => ("◌ ", "end gate not verified".to_owned(), t.gold()),
        _ => ("✗ ", "end gate failed".to_owned(), t.alarm()),
    };
    let mut text = words;
    if changed > 0 {
        let _ = write!(text, " · {changed} changed");
    }
    if !failures.is_empty() {
        text.push_str(": ");
        text.push_str(&failures.join("; "));
    }
    let lines = wrap_text(&one_line(&text), w.saturating_sub(2), style);
    prefixed(lines, &[Span::styled(mark, style)], &[Span::raw("  ")])
}

/// The one-line summary of a call's input: its command, path (with the range read), pattern,
/// query or URL; else its JSON, cut.
#[must_use]
pub fn summary(name: &str, input: &Value) -> String {
    if input.is_null() {
        return String::new();
    }
    let get = |k: &str| input.get(k).and_then(Value::as_str);
    if let Some(cmd) = get("command") {
        return one_line(cmd.lines().next().unwrap_or(""));
    }
    if let Some(path) = get("path") {
        let path = one_line(path);
        if name == "read"
            && let Some(off) = input.get("offset").and_then(Value::as_u64)
        {
            let lim = input.get("limit").and_then(Value::as_u64).unwrap_or(0);
            return if lim > 0 {
                format!("{path}:{off}-{}", off + lim - 1)
            } else {
                format!("{path}:{off}")
            };
        }
        return path;
    }
    for k in ["pattern", "query", "url", "body"] {
        if let Some(v) = get(k) {
            return one_line(v);
        }
    }
    clip(&one_line(&input.to_string()), 80)
}

/// Whether a call has something folded that expanding it would show: output past
/// [`OUTPUT_LINES`], a diff past [`DIFF_LINES`], a written file past its head, a read's text.
#[must_use]
pub fn foldable(call: &Call) -> bool {
    let Some(o) = &call.outcome else {
        return false;
    };
    let lines = |s: &str| s.trim_end_matches('\n').lines().count();
    if o.is_error {
        return lines(&o.content) > OUTPUT_LINES;
    }
    match diff::of(&call.name, &call.input) {
        Some(Change::Diff { lines, .. }) => lines.len() > DIFF_LINES,
        Some(Change::NewFile { lines, head, .. }) => lines > head.len(),
        None if call.name == "read" => !o.content.trim().is_empty(),
        None => lines(&o.content) > OUTPUT_LINES,
    }
}

/// A call as a card: its tool and summary set into the top edge, its state on the right — a
/// spinner, ✓ or ✗ — and its diff or folded output inside; expanded, all of it. The card holding
/// the focus wears the accent on its border.
fn card(call: &Call, w: usize, ctx: &Ctx<'_>) -> Lines {
    let t = &ctx.theme;
    let ok = call.outcome.as_ref().is_some_and(|o| !o.is_error);
    let failed = call.outcome.as_ref().is_some_and(|o| o.is_error);
    let change = if ok {
        diff::of(&call.name, &call.input)
    } else {
        None
    };
    let open = call.expanded;
    let name_style = if failed { t.alarm() } else { t.holo() };
    let title = Line::from(vec![
        Span::styled(
            one_line(&call.name),
            name_style.add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(summary(&call.name, &call.input), t.text()),
    ]);
    let mut right: Vec<Span<'static>> = Vec::new();
    let body: Lines = match &call.outcome {
        None if call.stopped => {
            right.push(Span::styled("◌ stopped", t.muted()));
            Vec::new()
        }
        None => {
            right.push(Span::styled(ctx.spinner, t.holo()));
            Vec::new()
        }
        Some(o) if o.is_error => {
            right.push(Span::styled("✗", t.alarm()));
            folded(&o.content, o.cut, w, t.alarm(), t, open)
        }
        Some(o) => {
            if let Some(c) = &change {
                right.push(Span::styled(format!("{} ", c.stat()), t.muted()));
            } else if call.name == "read" {
                let n = o.content.lines().count();
                right.push(Span::styled(format!("{n} lines "), t.muted()));
            }
            right.push(Span::styled("✓", t.ok()));
            match &change {
                Some(c) => change_lines(c, &call.input, w.saturating_sub(4), t, open),
                None if call.name == "read" && !open => Vec::new(),
                None => folded(&o.content, o.cut, w, t.muted(), t, open),
            }
        }
    };
    let border = if call.focused {
        t.holo()
    } else if failed {
        t.alarm()
    } else {
        t.frame()
    };
    Boxed::new()
        .border(Border::Normal, border)
        .padding(Sides::sym(0, 1))
        .width(w)
        .title(title)
        .title_right(Line::from(right))
        .open_bottom(body.is_empty())
        .render(body)
}

/// The line under folded lines: how many more, and how to see them.
fn more_line(more: usize, cut: usize, t: &Theme) -> Option<Line<'static>> {
    if more == 0 && cut == 0 {
        return None;
    }
    let mut s = String::from("…");
    if more > 0 {
        let _ = write!(s, " +{more} lines");
    }
    if cut > 0 {
        let _ = write!(s, " ({cut} bytes cut)");
    }
    let mut spans = vec![Span::styled(s, t.muted())];
    if more > 0 {
        spans.push(Span::styled(" · click to expand", t.frame()));
    }
    Some(Line::from(spans))
}

/// Output cut to [`OUTPUT_LINES`], the rest counted; `open`, all of it.
fn folded(content: &str, cut: usize, w: usize, style: Style, t: &Theme, open: bool) -> Lines {
    let text = clean(content.trim_end_matches('\n'));
    if text.trim().is_empty() {
        return Vec::new();
    }
    let all: Vec<&str> = text.lines().collect();
    let inner = w.saturating_sub(4);
    let shown = if open { all.len() } else { OUTPUT_LINES };
    let mut out: Lines = all
        .iter()
        .take(shown)
        .map(|l| Line::from(Span::styled(clip(l, inner), style)))
        .collect();
    out.extend(more_line(all.len().saturating_sub(shown), cut, t));
    out
}

/// A change's lines, each tinted to the card's inner width; long diffs fold, and `open` shows
/// them whole — a written file's every line, from the call's `input`.
fn change_lines(c: &Change, input: &Value, inner: usize, t: &Theme, open: bool) -> Lines {
    let tint = |fg: crate::theme::Rgb| fg.mix(t.p.bg, 0.82).color();
    let add = (t.readable(t.races.elf), tint(t.readable(t.races.elf)));
    let del = (t.p.alarm, tint(t.p.alarm));
    let line =
        |sign: &str, text: &str, colours: Option<(crate::theme::Rgb, ratatui::style::Color)>| {
            let text = one_line(text);
            match colours {
                Some((fg, bg)) => {
                    let style = Style::new().fg(fg.color()).bg(bg);
                    let l = Line::from(Span::styled(format!("{sign}{text}"), style));
                    fit(l, inner, Align::Left, Style::new().bg(bg))
                }
                None => Line::from(Span::styled(format!("{sign}{text}"), t.muted())),
            }
        };
    match c {
        Change::Diff { lines, .. } => {
            let shown = if open { lines.len() } else { DIFF_LINES };
            let mut out: Lines = lines
                .iter()
                .take(shown)
                .map(|l| match l {
                    DiffLine::Add(s) => line("+ ", s, Some(add)),
                    DiffLine::Del(s) => line("- ", s, Some(del)),
                    DiffLine::Same(s) => line("  ", s, None),
                    DiffLine::Hunk(s) => Line::from(Span::styled(one_line(s), t.frame())),
                })
                .collect();
            out.extend(more_line(lines.len().saturating_sub(shown), 0, t));
            out
        }
        Change::NewFile { path, lines, head } => {
            let mut out = vec![Line::from(Span::styled(
                format!("new file, {lines} lines"),
                t.muted(),
            ))];
            let ext = path.rsplit('.').next().unwrap_or("");
            let whole = input
                .get("content")
                .and_then(Value::as_str)
                .filter(|_| open);
            let shown = whole.map_or(head.len(), |w| w.lines().count());
            let code = clean(&whole.map_or_else(|| head.join("\n"), str::to_owned));
            for spans in crate::kit::highlight::highlight(&code, ext, t, t.text()) {
                out.push(crate::kit::text::truncate(Line::from(spans), inner));
            }
            out.extend(more_line(lines.saturating_sub(shown), 0, t));
            out
        }
    }
}
