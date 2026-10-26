//! How each entry of the transcript reads, in Isekai's visual language: every message and call
//! a raised panel ([`crate::kit::slab`]) with a coloured spine — the human's gold, the answer's
//! cyan with `✦`, a tool's by its class — the reasoning a quiet line that folds to its duration,
//! consecutive looks one compact card, the end gate one line with its counts. A pure function
//! of the entry, the one before it, whether the next continues its panel, the theme, the width
//! and the clock.

use std::fmt::Write as _;
use std::time::Duration;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

use crate::diff::{self, Change, DiffLine};
use crate::kit::markdown;
use crate::kit::slab::{Slab, Surface};
use crate::kit::text::{Align, Lines, clip, clip_left, fit, prefixed, wrap_text};
use crate::safe::{clean, one_line};
use crate::theme::{Rgb, Theme};
use crate::transcript::{Call, EXPLORE, Entry, Group, Thought};

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
    /// The clock, for what is live.
    pub now: Duration,
}

/// Output lines a done call shows before it folds.
pub const OUTPUT_LINES: usize = 3;
/// Diff lines a card shows before it folds.
pub const DIFF_LINES: usize = 12;
/// How far a panel's face leans toward its hue.
const TINT: f32 = 0.07;

/// An entry's lines; `prev` is the entry before it, which sets the space above; `joins` says the
/// next entry continues its panel.
#[must_use]
pub fn render(entry: &Entry, prev: Option<&Entry>, joins: bool, ctx: &Ctx<'_>) -> Lines {
    let t = &ctx.theme;
    let w = ctx.width.max(16);
    let continues = matches!(entry, Entry::Assistant { head: false, .. })
        && matches!(prev, Some(Entry::Assistant { .. }));
    let mut out: Lines = Vec::new();
    if prev.is_some() && !continues {
        out.push(Line::from(""));
    }
    match entry {
        Entry::User(text) => {
            let hue = t.races.veldora;
            let body = wrap_text(&clean(text), Slab::inner(w), t.text());
            out.extend(Slab::new(Surface::card(t, Some(hue), TINT * 1.5)).render(body, w));
        }
        Entry::Assistant { text, .. } => {
            let inner = Slab::inner(w);
            // The renderer makes each parsed text safe: entities decode before they are escaped.
            let md = markdown::render(text, inner.saturating_sub(2), t);
            let mut rows = if continues {
                vec![Line::from("")]
            } else {
                Vec::new()
            };
            let mark = if continues {
                Span::raw("  ")
            } else {
                Span::styled("✦ ", t.holo().add_modifier(Modifier::BOLD))
            };
            rows.extend(prefixed(md, &[mark], &[Span::raw("  ")]));
            let slab = Slab::new(Surface::card(t, Some(t.p.holo), TINT)).edges(!continues, !joins);
            out.extend(slab.render(rows, w));
        }
        Entry::Tool(call) => out.extend(card(call, w, ctx)),
        Entry::Group(group) => out.extend(group_card(group, w, ctx)),
        Entry::Thought(thought) => out.extend(thought_lines(thought, w, ctx)),
        Entry::Verdict {
            verdict,
            changed,
            failures,
            unverified,
        } => out.push(verdict_line(verdict, *changed, failures, unverified, w, t)),
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
            out.extend(note("■ ", ending, w, style));
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

fn note(mark: &str, text: &str, w: usize, style: Style) -> Lines {
    let lines = wrap_text(&clean(text), w.saturating_sub(4), style);
    let first = [Span::raw("  "), Span::styled(mark.to_owned(), style)];
    prefixed(lines, &first, &[Span::raw("    ")])
}

/// A duration as Isekai writes a call's: `12ms` under a second, `1.2s` under a minute, `1m3s`.
#[must_use]
pub fn took(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        // Display only: tenths of a second.
        #[allow(clippy::cast_precision_loss)]
        let s = ms as f64 / 1000.0;
        format!("{s:.1}s")
    } else {
        format!("{}m{}s", ms / 60_000, (ms / 1000) % 60)
    }
}

/// Elapsed time in whole seconds: `0s`, `12s`, `1m3s`.
#[must_use]
pub fn elapsed(d: Duration) -> String {
    let s = d.as_secs();
    if s < 60 {
        format!("{s}s")
    } else {
        format!("{}m{}s", s / 60, s % 60)
    }
}

/// The end gate's verdict as one line: its mark, its word and its counts, the first failure
/// after them, on a chip in the verdict's colour.
fn verdict_line(
    verdict: &str,
    changed: usize,
    failures: &[String],
    unverified: &[String],
    w: usize,
    t: &Theme,
) -> Line<'static> {
    let (mark, word, hue) = match verdict {
        "PASS" => ("✓", "pass", t.readable(t.races.elf)),
        "NOT_VERIFIED" => ("◌", "not verified", t.p.gold),
        _ => ("✗", "fail", t.p.alarm),
    };
    let chip = Style::new()
        .fg(hue.color())
        .bg(t.p.bg.mix(hue, 0.18).color())
        .add_modifier(Modifier::BOLD);
    let mut counts = String::new();
    if changed > 0 {
        let _ = write!(counts, " · {changed} changed");
    }
    if !failures.is_empty() {
        let _ = write!(counts, " · {} failed", failures.len());
    }
    if !unverified.is_empty() {
        let _ = write!(counts, " · {} unverified", unverified.len());
    }
    if let Some(first) = failures.first().or(unverified.first()) {
        let _ = write!(counts, ": {}", one_line(first));
    }
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(format!(" {mark} end gate · {word} "), chip),
        Span::styled(counts, t.muted()),
    ]);
    crate::kit::text::truncate(line, w)
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

/// The summary of a call's input still streaming, from the JSON text so far: the first of the
/// keys [`summary`] reads whose string has begun, as far as it came.
#[must_use]
pub fn partial_summary(json: &str) -> String {
    for key in ["command", "path", "pattern", "query", "url"] {
        let needle = format!("\"{key}\"");
        let Some(at) = json.find(&needle) else {
            continue;
        };
        let rest = json[at + needle.len()..].trim_start();
        let Some(rest) = rest.strip_prefix(':') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(c) => out.push(c),
                    None => break,
                },
                c => out.push(c),
            }
        }
        return one_line(out.lines().next().unwrap_or(""));
    }
    String::new()
}

/// A call's class: the tool's own, or what the classifier read of a command.
#[must_use]
pub fn class_of(call: &Call) -> Option<&str> {
    match call.name.as_str() {
        n if EXPLORE.contains(&n) => Some("read"),
        "write" | "edit" | "multiedit" | "patch" | "move" => Some("write"),
        "delete" => Some("destructive"),
        _ => call.class.as_deref(),
    }
}

/// A class's hue: a look in Rimuru's blue, a write in the dark elf's violet, an outward act in
/// the orc's orange, a destructive one in the alarm.
#[must_use]
pub fn class_hue(t: &Theme, class: Option<&str>) -> Rgb {
    match class {
        Some("read") => t.readable(t.races.rimuru),
        Some("write") => t.readable(t.races.dark_elf),
        Some("outward") => t.readable(t.races.orc),
        Some("destructive") => t.p.alarm,
        _ => t.p.line.mix(t.p.text_muted, 0.45),
    }
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

/// A call's head row: its dot (the spinner while it runs), its name in its class's hue, its
/// summary — as far as its arguments came — and its class tag.
fn head_row(call: &Call, hue: Rgb, ctx: &Ctx<'_>) -> Line<'static> {
    let t = &ctx.theme;
    let failed = call.outcome.as_ref().is_some_and(|o| o.is_error);
    let dot = if call.running() {
        Span::styled(format!("{} ", ctx.spinner), Style::new().fg(hue.color()))
    } else if failed {
        Span::styled("● ", t.alarm())
    } else if call.stopped {
        Span::styled("● ", t.muted())
    } else {
        Span::styled("● ", t.ok())
    };
    let summary = if call.input.is_null() {
        partial_summary(&call.json)
    } else {
        summary(&call.name, &call.input)
    };
    let mut spans = vec![
        dot,
        Span::styled(
            one_line(&call.name),
            Style::new().fg(hue.color()).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(summary, t.text()),
    ];
    if let Some(class) = class_of(call) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(class.to_owned(), Style::new().fg(hue.color())));
    }
    Line::from(spans)
}

/// A call's status row: `⎿ running…`, then `⎿ ok · 1.2s · …`, `⎿ failed · 800ms`.
fn status_row(call: &Call, change: Option<&Change>, ctx: &Ctx<'_>) -> Line<'static> {
    let t = &ctx.theme;
    let elbow = Span::styled("  ⎿ ", t.muted());
    let Some(o) = &call.outcome else {
        let word = if call.stopped {
            "stopped"
        } else {
            "running…"
        };
        return Line::from(vec![elbow, Span::styled(word, t.muted())]);
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(d) = call.took {
        parts.push(took(d));
    }
    if let Some(c) = change {
        parts.push(c.stat());
    } else if !o.is_error {
        let n = o.content.trim_end_matches('\n').lines().count();
        if n > 0 {
            parts.push(format!("{n} line{}", if n == 1 { "" } else { "s" }));
        }
    }
    if o.cut > 0 {
        parts.push(format!("{} bytes cut", o.cut));
    }
    if foldable(call) {
        parts.push(
            if call.expanded {
                "ctrl+t folds"
            } else {
                "ctrl+t"
            }
            .to_owned(),
        );
    }
    let (word, style) = if o.is_error {
        ("failed", t.alarm())
    } else {
        ("ok", t.ok())
    };
    let mut spans = vec![elbow, Span::styled(word, style)];
    for p in parts {
        spans.push(Span::styled(" · ", t.frame()));
        spans.push(Span::styled(p, t.muted()));
    }
    Line::from(spans)
}

/// A call as a raised card: its head on the band, its status under it, then its diff or folded
/// output; expanded, all of it. The card holding the focus wears the accent on its spine.
fn card(call: &Call, w: usize, ctx: &Ctx<'_>) -> Lines {
    let t = &ctx.theme;
    let ok = call.outcome.as_ref().is_some_and(|o| !o.is_error);
    let change = if ok {
        diff::of(&call.name, &call.input)
    } else {
        None
    };
    let hue = class_hue(t, class_of(call));
    let inner = Slab::inner(w);
    let mut rows = vec![
        head_row(call, hue, ctx),
        status_row(call, change.as_ref(), ctx),
    ];
    let open = call.expanded;
    match (&call.outcome, &change) {
        (Some(o), _) if o.is_error => rows.extend(folded(&o.content, inner, t.alarm(), t, open)),
        (Some(_), Some(c)) => rows.extend(change_lines(c, &call.input, inner, t, open)),
        (Some(o), None) if call.name != "read" || open => {
            rows.extend(folded(&o.content, inner, t.muted(), t, open));
        }
        _ => {}
    }
    let mut surface = Surface::card(t, Some(hue), TINT);
    if call.focused {
        surface.spine = Some(t.p.holo);
    }
    Slab::new(surface).band(1).render(rows, w)
}

/// What a group's calls looked at, counted: `read 4 files · grep 3 patterns`.
#[must_use]
pub fn group_counts(g: &Group) -> String {
    let mut parts = Vec::new();
    for (tool, one, many) in [
        ("read", "file", "files"),
        ("grep", "pattern", "patterns"),
        ("glob", "pattern", "patterns"),
    ] {
        let calls: Vec<&Call> = g.calls.iter().filter(|c| c.name == tool).collect();
        if calls.is_empty() {
            continue;
        }
        // The same file read in three ranges is one file; a call whose input has not come is one.
        let mut seen: Vec<String> = calls
            .iter()
            .map(|c| {
                let s = summary(&c.name, &c.input);
                if tool == "read" {
                    s.split(':').next().unwrap_or("").to_owned()
                } else {
                    s
                }
            })
            .collect();
        seen.sort();
        seen.dedup();
        let n = seen.len();
        parts.push(format!("{tool} {n} {}", if n == 1 { one } else { many }));
    }
    parts.join(" · ")
}

/// Consecutive looks as one card: what they looked at, counted; the one running now, or how
/// long they took; expanded, each call on its row.
fn group_card(g: &Group, w: usize, ctx: &Ctx<'_>) -> Lines {
    let t = &ctx.theme;
    let hue = class_hue(t, Some("read"));
    let failed = g
        .calls
        .iter()
        .filter(|c| c.outcome.as_ref().is_some_and(|o| o.is_error))
        .count();
    let dot = if g.running() {
        Span::styled(format!("{} ", ctx.spinner), Style::new().fg(hue.color()))
    } else if failed > 0 {
        Span::styled("● ", t.alarm())
    } else {
        Span::styled("● ", t.ok())
    };
    let mut rows = vec![Line::from(vec![
        dot,
        Span::styled(
            group_counts(g),
            Style::new().fg(hue.color()).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("read", Style::new().fg(hue.color())),
    ])];
    let elbow = Span::styled("  ⎿ ", t.muted());
    if let Some(now) = g.calls.iter().rev().find(|c| c.running()) {
        let what = if now.input.is_null() {
            partial_summary(&now.json)
        } else {
            summary(&now.name, &now.input)
        };
        rows.push(Line::from(vec![
            elbow,
            Span::styled(format!("{} ", now.name), t.muted()),
            Span::styled(what, t.text()),
        ]));
    } else {
        let total: Duration = g.calls.iter().filter_map(|c| c.took).sum();
        rows.push(Line::from(vec![
            elbow,
            if failed > 0 {
                Span::styled(format!("{failed} failed"), t.alarm())
            } else {
                Span::styled("ok", t.ok())
            },
            Span::styled(" · ", t.frame()),
            Span::styled(took(total), t.muted()),
            Span::styled(" · ", t.frame()),
            Span::styled(
                if g.expanded {
                    "ctrl+t folds"
                } else {
                    "ctrl+t lists them"
                },
                t.muted(),
            ),
        ]));
    }
    if g.expanded {
        let inner = Slab::inner(w);
        for c in &g.calls {
            let (mark, style) = match &c.outcome {
                None if c.stopped => ("◌", t.muted()),
                None => (ctx.spinner, Style::new().fg(hue.color())),
                Some(o) if o.is_error => ("✗", t.alarm()),
                Some(_) => ("✓", t.ok()),
            };
            let mut spans = vec![
                Span::raw("    "),
                Span::styled(format!("{mark} "), style),
                Span::styled(format!("{} ", c.name), t.muted()),
                Span::styled(summary(&c.name, &c.input), t.text()),
            ];
            if let Some(o) = c.outcome.as_ref().filter(|o| !o.is_error) {
                let n = o.content.trim_end_matches('\n').lines().count();
                spans.push(Span::styled(format!(" · {n} lines"), t.muted()));
            }
            if let Some(d) = c.took {
                spans.push(Span::styled(format!(" · {}", took(d)), t.muted()));
            }
            rows.push(crate::kit::text::truncate(Line::from(spans), inner));
        }
    }
    let mut surface = Surface::card(t, Some(hue), TINT);
    if g.focused {
        surface.spine = Some(t.p.holo);
    }
    Slab::new(surface).band(1).render(rows, w)
}

/// The reasoning: live, one line that updates in place — its duration and its latest words;
/// done, `✓ thought for 12s`; expanded, its text under a quiet rail.
fn thought_lines(th: &Thought, w: usize, ctx: &Ctx<'_>) -> Lines {
    let t = &ctx.theme;
    let quiet = t.muted().add_modifier(Modifier::ITALIC);
    let Some(d) = th.took else {
        let head = format!("◌ thinking · {}", elapsed(ctx.now.saturating_sub(th.began)));
        let room = w.saturating_sub(crate::kit::text::cells(&head) + 7);
        let mut spans = vec![Span::raw("  "), Span::styled(head, t.holo())];
        let words = one_line(th.text.trim());
        if !words.is_empty() && room > 4 {
            spans.push(Span::styled(" · ", t.frame()));
            spans.push(Span::styled(clip_left(&words, room), quiet));
        }
        return vec![Line::from(spans)];
    };
    let has_text = !th.text.trim().is_empty();
    let mark = if th.focused { t.holo() } else { t.muted() };
    let mut head = vec![
        Span::raw("  "),
        Span::styled("✓ ", mark),
        Span::styled(format!("thought for {}", elapsed(d)), quiet),
    ];
    if has_text {
        head.push(Span::styled(
            if th.expanded {
                " · ctrl+t folds"
            } else {
                " · ctrl+t"
            },
            t.frame(),
        ));
    }
    let mut out = vec![Line::from(head)];
    if th.expanded && has_text {
        let body = wrap_text(&clean(th.text.trim()), w.saturating_sub(6), quiet);
        let rail = [Span::styled("    │ ", t.line())];
        out.extend(prefixed(body, &rail, &rail));
    }
    out
}

/// The line under folded lines: how many more, and how to see them.
fn more_line(more: usize, t: &Theme) -> Option<Line<'static>> {
    (more > 0).then(|| {
        Line::from(vec![
            Span::styled(format!("    … +{more} lines"), t.muted()),
            Span::styled(" · click or ctrl+t to expand", t.frame()),
        ])
    })
}

/// Output cut to [`OUTPUT_LINES`], the rest counted; `open`, all of it.
fn folded(content: &str, inner: usize, style: Style, t: &Theme, open: bool) -> Lines {
    let text = clean(content.trim_end_matches('\n'));
    if text.trim().is_empty() {
        return Vec::new();
    }
    let all: Vec<&str> = text.lines().collect();
    let shown = if open { all.len() } else { OUTPUT_LINES };
    let mut out: Lines = all
        .iter()
        .take(shown)
        .map(|l| {
            Line::from(Span::styled(
                format!("    {}", clip(l, inner.saturating_sub(4))),
                style,
            ))
        })
        .collect();
    out.extend(more_line(all.len().saturating_sub(shown), t));
    out
}

/// The first new-side line number of a unified hunk header, `@@ -a,b +c,d @@`.
fn hunk_start(h: &str) -> Option<usize> {
    let plus = h.split_whitespace().find(|p| p.starts_with('+'))?;
    plus[1..].split(',').next()?.parse().ok()
}

/// A change's lines with their numbers, each tinted to the card's inner width; long diffs fold,
/// and `open` shows them whole — a written file's every line, from the call's `input`.
fn change_lines(c: &Change, input: &Value, inner: usize, t: &Theme, open: bool) -> Lines {
    type Colours = Option<(Rgb, ratatui::style::Color)>;
    let tint = |fg: Rgb| fg.mix(t.p.bg, 0.82).color();
    let add = (t.readable(t.races.elf), tint(t.readable(t.races.elf)));
    let del = (t.p.alarm, tint(t.p.alarm));
    let number = |n: Option<usize>| {
        Span::styled(
            format!("{:>4} ", n.map(|n| n.to_string()).unwrap_or_default()),
            t.line(),
        )
    };
    let row = |n: Option<usize>, sign: &str, text: &str, colours: Colours| {
        let text = one_line(text);
        let body = match colours {
            Some((fg, bg)) => {
                let style = Style::new().fg(fg.color()).bg(bg);
                fit(
                    Line::from(Span::styled(format!("{sign}{text}"), style)),
                    inner.saturating_sub(5),
                    Align::Left,
                    Style::new().bg(bg),
                )
            }
            None => Line::from(Span::styled(format!("{sign}{text}"), t.muted())),
        };
        let mut spans = vec![number(n)];
        spans.extend(body.spans);
        Line::from(spans)
    };
    match c {
        Change::Diff { lines, .. } => {
            let shown = if open { lines.len() } else { DIFF_LINES };
            let mut at = 1usize;
            let mut out: Lines = Vec::new();
            for l in lines.iter().take(shown) {
                out.push(match l {
                    DiffLine::Add(s) => {
                        at += 1;
                        row(Some(at - 1), "+ ", s, Some(add))
                    }
                    DiffLine::Del(s) => row(None, "- ", s, Some(del)),
                    DiffLine::Same(s) => {
                        at += 1;
                        row(Some(at - 1), "  ", s, None)
                    }
                    DiffLine::Hunk(s) => {
                        if let Some(n) = hunk_start(s) {
                            at = n;
                        }
                        Line::from(vec![
                            Span::raw("     "),
                            Span::styled(one_line(s), t.frame()),
                        ])
                    }
                });
            }
            out.extend(more_line(lines.len().saturating_sub(shown), t));
            out
        }
        Change::NewFile { path, lines, head } => {
            let mut out = vec![Line::from(Span::styled(
                format!("    new file, {lines} lines"),
                t.muted(),
            ))];
            let ext = path.rsplit('.').next().unwrap_or("");
            let whole = input
                .get("content")
                .and_then(Value::as_str)
                .filter(|_| open);
            let shown = whole.map_or(head.len(), |w| w.lines().count());
            let code = clean(&whole.map_or_else(|| head.join("\n"), str::to_owned));
            for (i, spans) in crate::kit::highlight::highlight(&code, ext, t, t.text())
                .into_iter()
                .enumerate()
            {
                let mut row = vec![number(Some(i + 1))];
                row.extend(spans);
                out.push(crate::kit::text::truncate(Line::from(row), inner));
            }
            out.extend(more_line(lines.saturating_sub(shown), t));
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_streaming_input_shows_its_summary_as_far_as_it_came() {
        assert_eq!(partial_summary(r#"{"comm"#), "");
        assert_eq!(partial_summary(r#"{"command": "pyte"#), "pyte");
        assert_eq!(
            partial_summary(r#"{"command":"pytest -q \"a b\"","#),
            r#"pytest -q "a b""#
        );
        assert_eq!(partial_summary(r#"{"path":"src/a.rs"}"#), "src/a.rs");
    }

    #[test]
    fn durations_read_as_isekai_writes_them() {
        assert_eq!(took(Duration::from_millis(12)), "12ms");
        assert_eq!(took(Duration::from_millis(1234)), "1.2s");
        assert_eq!(took(Duration::from_secs(63)), "1m3s");
        assert_eq!(elapsed(Duration::from_millis(12_400)), "12s");
        assert_eq!(elapsed(Duration::from_secs(63)), "1m3s");
    }

    #[test]
    fn a_hunk_header_gives_its_first_line() {
        assert_eq!(hunk_start("@@ -10,3 +12,4 @@"), Some(12));
        assert_eq!(hunk_start("@@ -1 +1 @@"), Some(1));
        assert_eq!(hunk_start("⋯"), None);
    }
}
