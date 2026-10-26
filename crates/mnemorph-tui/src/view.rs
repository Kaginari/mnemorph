//! The frame: a rounded border naming the session and carrying the status bar, the transcript's
//! visible window, the lines waiting their turn, the approval card and the input box, and the
//! sidebar's column on the right ([`crate::sidebar`]).

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
use crate::kit::text::{Lines, cells, clip, truncate};

/// The smallest frame worth drawing.
const MIN: (u16, u16) = (24, 8);
/// The most rows the input grows to.
const INPUT_ROWS: usize = 6;

/// What a block of the transcript is drawn with, at `width`.
#[must_use]
pub fn ctx(m: &Model, width: usize) -> Ctx<'_> {
    Ctx {
        theme: m.theme,
        width,
        frame: m.spinner.frame(),
        spinner: m.spinner.view(),
        me: &m.header.body,
    }
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
    let side = m.sidebar.width(inner.width);
    let main = Rect {
        width: inner.width - side,
        ..inner
    };
    body(m, main, buf);
    if side > 0 {
        sidebar(
            m,
            Rect {
                x: main.right(),
                width: side,
                ..inner
            },
            buf,
        );
    }
}

/// The sidebar in `area`: a quiet rule down its left edge, a space, its sections.
fn sidebar(m: &Model, area: Rect, buf: &mut Buffer) {
    let rule = Span::styled("│", m.theme.line());
    let w = usize::from(area.width.saturating_sub(3));
    let lines = crate::sidebar::view(m, w, usize::from(area.height));
    for y in area.y..area.bottom() {
        buf.set_span(area.x + 1, y, &rule, 1);
    }
    for (y, line) in (area.y..area.bottom()).zip(&lines) {
        buf.set_line(area.x + 3, y, line, area.width.saturating_sub(3));
    }
}

/// The transcript, the queue, the card and the input, stacked in `area`.
fn body(m: &Model, area: Rect, buf: &mut Buffer) {
    let theme = &m.theme;
    let w = usize::from(area.width);
    let now = std::time::Instant::now();
    // While an approval waits it takes the input's place: the input takes no keys then.
    let card = m.gate.as_ref().map(|a| {
        a.drawn(now, w, usize::from(area.height));
        a.view(theme, w, usize::from(area.height), now, m.waiting.len())
    });
    let input = if card.is_some() {
        Vec::new()
    } else {
        input_box(m, w)
    };
    let queued: Lines = m
        .transcript
        .queued
        .iter()
        .take(if card.is_some() { 0 } else { 3 })
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
    let ctx = ctx(m, w);
    let crate::transcript::Window {
        lines: window,
        owners,
        top,
    } = m.transcript.window(&ctx, rows, m.viewport.offset());
    m.measured.set(top.unwrap_or(usize::MAX));
    // The transcript sits on the input: a short one is pushed down, as a chat reads.
    let top_row = area.y + to_u16(rows.saturating_sub(window.len()));
    let stack = window
        .iter()
        .chain(&queued)
        .chain(card.iter().flatten())
        .chain(&input);
    // A stack taller than the area keeps its bottom: the card's choice and the input stay.
    let stack: Vec<&Line<'static>> = stack.collect();
    let skip = stack.len().saturating_sub(usize::from(area.height));
    let first = top_row.max(area.y);
    for (row, line) in (first..area.bottom()).zip(stack.into_iter().skip(skip)) {
        buf.set_line(area.x, row, line, area.width);
    }
    // Each row's entry, for a click: the transcript's rows only, as drawn.
    let mut hits = m.hits.borrow_mut();
    hits.x = area.x;
    hits.width = area.width;
    hits.y = first;
    hits.rows = rows;
    hits.owners.clear();
    hits.owners.extend(owners.into_iter().skip(skip));
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
    let mut spans = vec![Span::styled("─ ", t.frame())];
    spans.extend(mark(t));
    spans.push(Span::styled(" · ", t.frame()));
    spans.push(Span::styled(
        crate::safe::one_line(&m.header.body),
        t.rimuru(),
    ));
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

/// Mnemorph's mark: the hologram's diamond in the slimes' teal, the name drawn across the
/// water races — Rimuru's blue to the named slime's pale cyan — as a gradient, one hue a letter.
fn mark(t: &crate::theme::Theme) -> Vec<Span<'static>> {
    const NAME: &str = "MNEMORPH";
    let stops = [t.races.rimuru, t.races.slime, t.races.named_slime];
    let mut spans = vec![Span::styled(
        "◈ ",
        t.race(mnemorph_races::Race::Slime)
            .add_modifier(Modifier::BOLD),
    )];
    let last = NAME.len() - 1;
    for (i, c) in NAME.chars().enumerate() {
        // Display only: a letter's place along an eight-letter gradient.
        #[allow(clippy::cast_precision_loss)]
        let along = i as f32 / last as f32 * 2.0;
        let (from, to, frac) = if along < 1.0 {
            (stops[0], stops[1], along)
        } else {
            (stops[1], stops[2], along - 1.0)
        };
        let hue = t.readable(from.mix(to, frac));
        spans.push(Span::styled(
            c.to_string(),
            ratatui::style::Style::new()
                .fg(hue.color())
                .add_modifier(Modifier::BOLD),
        ));
    }
    spans
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
        parts.push(Span::styled(
            format!("{} tok", crate::sidebar::short(u.tokens)),
            t.muted(),
        ));
    }
    if let Some(pct) = u.occupancy.saturating_mul(100).checked_div(u.stress_line) {
        parts.push(Span::styled(
            format!("ctx {pct}%"),
            crate::sidebar::band(t, pct),
        ));
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
