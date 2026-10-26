//! The frame: a rounded border naming the session and carrying the status bar, the transcript's
//! visible window, the lines waiting their turn, the approval card and the input box. The
//! sidebar's column is reserved here and drawn by a later view.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Widget};

use crate::app::Model;
use crate::blocks::Ctx;
use crate::kit::keyhelp::{Binding, KeyHelp};
use crate::kit::style::{Border, Sides, Style as Boxed};
use crate::kit::text::{Lines, cells, clip, truncate, wrap_text};

/// The smallest frame worth drawing.
const MIN: (u16, u16) = (24, 8);
/// The most rows the input grows to.
const INPUT_ROWS: usize = 6;
/// The most rows of an act the approval card shows.
const ACT_ROWS: usize = 6;

/// The sidebar's width at a frame's width: none yet — the sidebar is a later view's — but the
/// layout already splits for it.
#[must_use]
pub fn sidebar_width(_width: u16) -> u16 {
    0
}

/// Draw `m` into the frame.
pub fn draw(m: &Model, f: &mut Frame<'_>) {
    let area = f.area();
    let buf = f.buffer_mut();
    let t = &m.theme;
    Block::new().style(t.page()).render(area, buf);
    if area.width < MIN.0 || area.height < MIN.1 {
        let msg = Line::from(Span::styled(
            clip("enlarge the terminal", area.width.into()),
            t.muted(),
        ));
        buf.set_line(area.x, area.y, &msg, area.width);
        return;
    }
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(t.frame())
        .title_top(header(m, area.width.into()))
        .title_bottom(status(m, area.width.into()))
        .render(area, buf);
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };
    let side = sidebar_width(inner.width);
    let main = Rect {
        width: inner.width - side,
        ..inner
    };
    body(m, main, buf);
}

/// The transcript, the queue, the card and the input, stacked in `area`.
fn body(m: &Model, area: Rect, buf: &mut Buffer) {
    let theme = &m.theme;
    let w = usize::from(area.width);
    let input = input_box(m, w);
    let card = m
        .gate
        .as_ref()
        .map(|(c, choice)| gate_card(m, c, *choice, w));
    let queued: Lines = m
        .transcript
        .queued
        .iter()
        .take(3)
        .map(|q| {
            truncate(
                Line::from(vec![
                    Span::styled("⧗ waiting  ", theme.muted()),
                    Span::styled(crate::safe::one_line(q), theme.text()),
                ]),
                w,
            )
        })
        .collect();
    let below = input.len() + card.as_ref().map_or(0, Vec::len) + queued.len();
    let rows = usize::from(area.height).saturating_sub(below);
    let ctx = Ctx {
        theme: m.theme,
        width: w,
        frame: m.spinner.frame(),
        spinner: m.spinner.view(),
        me: &m.header.body,
    };
    let (window, top) = m.transcript.window(&ctx, rows, m.viewport.offset());
    m.measured.set(top.unwrap_or(usize::MAX));
    // The transcript sits on the input: a short one is pushed down, as a chat reads.
    let top_row = area.y + to_u16(rows.saturating_sub(window.len()));
    let stack = window
        .iter()
        .chain(&queued)
        .chain(card.iter().flatten())
        .chain(&input);
    for (row, line) in (top_row..).zip(stack) {
        buf.set_line(area.x, row, line, area.width);
    }
    if !m.viewport.following() && rows > 0 {
        let note = Line::from(Span::styled(
            " ↓ pgdn · esc to the end ",
            theme.muted().add_modifier(Modifier::REVERSED),
        ));
        let x = area.x
            + area
                .width
                .saturating_sub(to_u16(cells(" ↓ pgdn · esc to the end ")));
        buf.set_line(x, area.y + to_u16(rows) - 1, &note, area.width);
    }
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// The top edge: Mnemorph, the body and its model; what holds the session, on the right.
fn header(m: &Model, width: usize) -> Line<'static> {
    let t = &m.theme;
    let mut spans = vec![
        Span::styled("─ ", t.frame()),
        Span::styled("◈ MNEMORPH", t.holo().add_modifier(Modifier::BOLD)),
        Span::styled(" · ", t.frame()),
        Span::styled(crate::safe::one_line(&m.header.body), t.rimuru()),
    ];
    if width >= 50 {
        spans.push(Span::styled(" · ", t.frame()));
        spans.push(Span::styled(
            crate::safe::one_line(&m.header.model),
            t.muted(),
        ));
    }
    if m.gate.is_some() {
        spans.push(Span::styled(" · ", t.frame()));
        spans.push(Span::styled(
            "approval waiting",
            t.gold().add_modifier(Modifier::BOLD),
        ));
    } else if m.busy() {
        spans.push(Span::styled(" · ", t.frame()));
        spans.push(Span::styled(
            format!("{} working", m.spinner.view()),
            t.holo(),
        ));
    }
    spans.push(Span::raw(" "));
    truncate(Line::from(spans), width.saturating_sub(3))
}

/// Tokens, short: 950, 12.3k, 4.1M.
fn short(n: u64) -> String {
    // Display only: a rounding of a token count.
    #[allow(clippy::cast_precision_loss)]
    let f = n as f64;
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", f / 1e3)
    } else {
        format!("{:.1}M", f / 1e6)
    }
}

/// The bottom edge: the spend, the context against its stress line, the keys that matter now,
/// the world.
fn status(m: &Model, width: usize) -> Line<'static> {
    let t = &m.theme;
    let u = &m.transcript.usage;
    let sep = || Span::styled(" · ", t.frame());
    let mut spans = vec![Span::styled("─ ", t.frame())];
    let mut parts: Vec<Span<'static>> = Vec::new();
    if u.tokens > 0 || u.usd > 0.0 {
        parts.push(Span::styled(
            if u.unpriced && u.usd == 0.0 {
                "unpriced".to_owned()
            } else {
                format!("${:.2}{}", u.usd, if u.unpriced { "+" } else { "" })
            },
            t.text(),
        ));
        parts.push(Span::styled(format!("{} tok", short(u.tokens)), t.muted()));
    }
    if let Some(pct) = u.occupancy.saturating_mul(100).checked_div(u.stress_line) {
        let style = match pct {
            0..75 => t.muted(),
            75..90 => t.gold(),
            _ => t.alarm(),
        };
        parts.push(Span::styled(format!("ctx {pct}%"), style));
    }
    let q = m.transcript.queued.len();
    if q > 0 {
        parts.push(Span::styled(format!("{q} queued"), t.muted()));
    }
    if m.transcript.held > 0 {
        parts.push(Span::styled(
            format!("{} held", m.transcript.held),
            t.muted(),
        ));
    }
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            spans.push(sep());
        }
        spans.push(p);
    }
    let used: usize = spans.iter().map(|s| cells(&s.content)).sum();
    let world = format!("world {}", crate::safe::one_line(&m.header.world));
    let world_cells = if width >= 80 { cells(&world) + 3 } else { 0 };
    let room = width.saturating_sub(used + world_cells + 6);
    let help = KeyHelp {
        key: t.text(),
        action: t.muted(),
        sep: t.frame(),
    }
    .view(bindings(m), room);
    if !help.spans.is_empty() {
        if used > 2 {
            spans.push(sep());
        }
        spans.extend(help.spans);
    }
    if world_cells > 0 {
        spans.push(sep());
        spans.push(Span::styled(world, t.muted()));
    }
    spans.push(Span::raw(" "));
    truncate(Line::from(spans), width.saturating_sub(3))
}

fn bindings(m: &Model) -> &'static [Binding] {
    const GATE: &[Binding] = &[
        Binding {
            key: "y/n",
            action: "choose",
        },
        Binding {
            key: "enter",
            action: "confirm",
        },
        Binding {
            key: "esc",
            action: "deny",
        },
    ];
    const BUSY: &[Binding] = &[
        Binding {
            key: "esc",
            action: "stop",
        },
        Binding {
            key: "enter",
            action: "queue",
        },
        Binding {
            key: "pgup",
            action: "scroll",
        },
    ];
    const IDLE: &[Binding] = &[
        Binding {
            key: "enter",
            action: "send",
        },
        Binding {
            key: "alt+enter",
            action: "newline",
        },
        Binding {
            key: "↑↓",
            action: "history",
        },
        Binding {
            key: "pgup",
            action: "scroll",
        },
        Binding {
            key: "ctrl+c",
            action: "quit",
        },
    ];
    if m.gate.is_some() {
        GATE
    } else if m.busy() {
        BUSY
    } else {
        IDLE
    }
}

/// The input: a rounded box, bright while it takes a line to run now, quiet while a turn runs
/// (a line sent then waits its turn), and closed while an approval waits.
fn input_box(m: &Model, w: usize) -> Lines {
    let t = &m.theme;
    let inner = w.saturating_sub(6);
    let focused = m.gate.is_none();
    let mut lines = m.input.view(inner, INPUT_ROWS, t.text(), focused);
    if m.input.text().is_empty() && focused {
        let hint = if m.busy() {
            "a line now waits its turn"
        } else {
            "ask, or /help"
        };
        if let Some(first) = lines.first_mut() {
            first
                .spans
                .push(Span::styled(clip(hint, inner.saturating_sub(1)), t.muted()));
        }
    }
    let prompt = Span::styled("› ", if focused { t.holo() } else { t.muted() });
    let lines = crate::kit::text::prefixed(lines, &[prompt], &[Span::raw("  ")]);
    let border = if focused && !m.busy() {
        t.holo()
    } else {
        t.frame()
    };
    Boxed::new()
        .border(Border::Rounded, border)
        .padding(Sides::sym(0, 1))
        .width(w)
        .render(lines)
}

/// The approval card: the gate's own frame and words, the act and its reasons as the core
/// escaped them, and the choice — made, then confirmed.
fn gate_card(m: &Model, card: &crate::gate::Card, choice: Option<bool>, w: usize) -> Lines {
    let t = &m.theme;
    let inner = w.saturating_sub(4);
    let mut lines: Lines = wrap_text(
        &format!("{} asks to run this {} act:", card.body, card.class),
        inner,
        t.text(),
    );
    let act = wrap_text(
        &card.act,
        inner.saturating_sub(2),
        t.text().add_modifier(Modifier::BOLD),
    );
    let more = act.len().saturating_sub(ACT_ROWS);
    for l in act.into_iter().take(ACT_ROWS) {
        let mut spans = vec![Span::styled("  ", t.text())];
        spans.extend(l.spans);
        lines.push(Line::from(spans));
    }
    if more > 0 {
        lines.push(Line::from(Span::styled(
            format!("  … +{more} lines"),
            t.muted(),
        )));
    }
    for why in &card.why {
        lines.extend(wrap_text(&format!("· {why}"), inner, t.muted()));
    }
    if !card.outside.is_empty() {
        let text = format!("opens outside the world: {}", card.outside.join(", "));
        lines.extend(wrap_text(&text, inner, t.alarm()));
    }
    lines.push(Line::from(""));
    let pick = |on: bool, key: &str| {
        let bold = t.gold().add_modifier(Modifier::BOLD);
        let style = if on {
            bold.add_modifier(Modifier::REVERSED)
        } else {
            bold
        };
        Span::styled(format!(" {key} "), style)
    };
    let next = if choice.is_some() {
        "  enter confirms · esc denies"
    } else {
        "  choose, then enter · esc denies"
    };
    lines.push(truncate(
        Line::from(vec![
            pick(choice == Some(true), "y"),
            Span::styled(" run it once  ", t.text()),
            pick(choice == Some(false), "n"),
            Span::styled(" deny", t.text()),
            Span::styled(next, t.muted()),
        ]),
        inner,
    ));
    Boxed::new()
        .border(Border::Thick, t.gold())
        .padding(Sides::sym(0, 1))
        .width(w)
        .title(Line::from(Span::styled(
            "⚠ the human gate",
            t.gold().add_modifier(Modifier::BOLD),
        )))
        .title_right(Line::from(Span::styled("silence denies", t.muted())))
        .render(lines)
}
