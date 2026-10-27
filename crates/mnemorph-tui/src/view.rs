//! The frame, in Isekai's look: a rounded border naming the session on the left of its top edge
//! and its state on the right, the status bar on its bottom edge; inside, the panes labelled in
//! small capitals — ᴄᴏɴᴠᴇʀꜱᴀᴛɪᴏɴ (the welcome, the transcript's visible window, the lines
//! waiting their turn, the activity strip while work runs), ᴀꜱᴋ (the input) and ꜱᴇꜱꜱɪᴏɴ (the
//! sidebar, [`crate::sidebar`]). An approval floats over the view, dimmed, as a modal; a toast
//! floats at the top right while it fades.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Widget};

use crate::app::Model;
use crate::blocks::Ctx;
use crate::kit::keyhelp::{Binding, KeyHelp};
use crate::kit::style::{Border, Sides, Style as Boxed};
use crate::kit::text::{Lines, cells, clip, small_caps, truncate};
use crate::pictures::{Art, Slot, Want};
use crate::theme::Rgb;

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
        now: m.transcript.clock,
    }
}

/// Draw `m` into the frame.
pub fn draw(m: &Model, f: &mut Frame<'_>) {
    let area = f.area();
    let buf = f.buffer_mut();
    let t = &m.theme;
    m.pictures.wants.borrow_mut().clear();
    Block::new().style(t.page()).render(area, buf);
    if area.width < MIN.0 || area.height < MIN.1 {
        let msg = Line::from(Span::styled(
            clip("enlarge the terminal", area.width.into()),
            t.muted(),
        ));
        buf.set_line(area.x, area.y, &msg, area.width);
        return;
    }
    let right = state(m);
    let left_room =
        usize::from(area.width).saturating_sub(crate::kit::text::line_cells(&right) + 4);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(t.frame())
        .title_top(header(m, left_room))
        .title_top(right.right_aligned())
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
    label(buf, main, "conversation", t.holo(), t);
    let below_label = Rect {
        y: main.y + 1,
        height: main.height.saturating_sub(1),
        ..main
    };
    body(m, below_label, buf);
    m.hits.borrow_mut().side_w = 0;
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
    if m.gate.is_some() {
        // The approval never sits under a picture: while it is open, none is wanted.
        m.pictures.wants.borrow_mut().clear();
        dim(buf, inner, t);
        modal(m, below_label, buf);
    }
    // A toast never covers the approval: it waits under the card, unseen, until it is gone.
    if let Some(toast) = m.toast.as_ref().filter(|_| m.gate.is_none()) {
        let lines = toast.view(t);
        let width = lines.first().map_or(0, crate::kit::text::line_cells);
        let left = below_label.right().saturating_sub(to_u16(width));
        for (row, line) in (below_label.y..below_label.bottom()).zip(&lines) {
            buf.set_line(left, row, line, to_u16(width));
        }
        // Nor does a picture sit over the toast.
        let over = Rect::new(left, below_label.y, to_u16(width), to_u16(lines.len()));
        m.pictures
            .wants
            .borrow_mut()
            .retain(|w| w.area.intersection(over).is_empty());
    }
}

/// The approval as a modal over `area`, centred across and a third of the way down. Its first
/// draw arms it, and the rows of the act it shows count as read.
fn modal(m: &Model, area: Rect, buf: &mut Buffer) {
    let Some(a) = &m.gate else {
        return;
    };
    let (cw, ch) = crate::approval::modal(usize::from(area.width), usize::from(area.height));
    let now = std::time::Instant::now();
    a.drawn(now, cw, ch);
    let card = a.view(&m.theme, cw, ch, now, m.waiting.len());
    let left = area.x + to_u16(usize::from(area.width).saturating_sub(cw) / 2);
    let top = area.y + to_u16(usize::from(area.height).saturating_sub(card.len()) / 3);
    for (row, line) in (top..area.bottom()).zip(&card) {
        buf.set_line(left, row, line, to_u16(cw));
    }
}

/// A pane's label on its first row: its name in small capitals, a quiet rule to its edge.
fn label(
    buf: &mut Buffer,
    area: Rect,
    name: &str,
    hue: ratatui::style::Style,
    t: &crate::theme::Theme,
) {
    let name = small_caps(name);
    let rule = usize::from(area.width).saturating_sub(cells(&name) + 1);
    let line = Line::from(vec![
        Span::styled(name, hue.add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled("─".repeat(rule), t.line()),
    ]);
    buf.set_line(area.x, area.y, &line, area.width);
}

/// Everything in `area` dimmed toward the page: the view under a modal.
fn dim(buf: &mut Buffer, area: Rect, t: &crate::theme::Theme) {
    let page = t.p.bg;
    let fade = |c: Color, k: f32| match c {
        Color::Rgb(r, g, b) => Rgb(r, g, b).mix(page, k).color(),
        Color::Reset => c,
        _ => t.p.line.color(),
    };
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            let fg = fade(cell.fg, 0.6);
            let bg = fade(cell.bg, 0.5);
            cell.set_fg(fg);
            cell.set_bg(bg);
        }
    }
}

/// The section title under (`col`, `row`) in the sidebar the last frame drew.
#[must_use]
pub fn sidebar_title_at(m: &Model, col: u16, row: u16) -> Option<crate::sidebar::Section> {
    let h = m.hits.borrow();
    let inside = h.side_w > 0 && col >= h.side_x && col < h.side_x + h.side_w && row >= h.side_y;
    if !inside {
        return None;
    }
    m.sidebar.title_at(usize::from(row - h.side_y))
}

/// The sidebar in `area`: a quiet rule down its left edge, a space, its label, its sections.
fn sidebar(m: &Model, area: Rect, buf: &mut Buffer) {
    let rule = Span::styled("│", m.theme.line());
    let w = usize::from(area.width.saturating_sub(3));
    let top = area.y + 1;
    let lines = crate::sidebar::view(m, w, usize::from(area.height.saturating_sub(1)));
    for y in area.y..area.bottom() {
        buf.set_span(area.x + 1, y, &rule, 1);
    }
    label(
        buf,
        Rect {
            x: area.x + 3,
            width: area.width.saturating_sub(3),
            ..area
        },
        "session",
        m.theme.gold(),
        &m.theme,
    );
    for (y, line) in (top..area.bottom()).zip(&lines) {
        buf.set_line(area.x + 3, y, line, area.width.saturating_sub(3));
    }
    let h = usize::from(area.height.saturating_sub(1));
    if let Some((pad, cols, rows)) = crate::sidebar::picture_box(m, w, h) {
        m.pictures.want(Want {
            slot: Slot::Sidebar,
            // Veldora while no body works; the body working now during a turn.
            art: if m.busy() {
                Art::Still(crate::activity::actor(m).1)
            } else {
                Art::Veldora
            },
            area: Rect::new(area.x + 3 + pad, top, cols, rows),
            bg: m.theme.p.bg,
        });
    }
    let mut hits = m.hits.borrow_mut();
    hits.side_x = area.x + 3;
    hits.side_w = area.width.saturating_sub(3);
    hits.side_y = top;
}

/// The transcript, the queue, the card and the input, stacked in `area`.
fn body(m: &Model, area: Rect, buf: &mut Buffer) {
    let theme = &m.theme;
    let w = usize::from(area.width);
    // While an approval waits it floats over the view; the input takes no keys then.
    let input = input_box(m, w);
    let strip = crate::activity::strip(m, w);
    let queued: Lines = m
        .transcript
        .queued
        .iter()
        .take(if m.gate.is_some() { 0 } else { 3 })
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
    let below = input.len() + strip.len() + queued.len();
    let rows = usize::from(area.height).saturating_sub(below);
    let ctx = ctx(m, w);
    let head = crate::welcome::card(m, w);
    let crate::transcript::Window {
        lines: window,
        owners,
        top,
        head: head_skip,
    } = m.transcript.window(&ctx, rows, m.viewport.offset(), &head);
    m.measured.set(top.unwrap_or(usize::MAX));
    // The transcript sits on the input: a short one is pushed down, as a chat reads.
    let top_row = area.y + to_u16(rows.saturating_sub(window.len()));
    let stack = window.iter().chain(&queued).chain(&strip).chain(&input);
    // A stack taller than the area keeps its bottom: the status and the input stay.
    let stack: Vec<&Line<'static>> = stack.collect();
    let skip = stack.len().saturating_sub(usize::from(area.height));
    let first = top_row.max(area.y);
    for (row, line) in (first..area.bottom()).zip(stack.into_iter().skip(skip)) {
        buf.set_line(area.x, row, line, area.width);
    }
    if let Some(n) = head_skip {
        welcome_art(area, buf, (n, skip, first, window.len()));
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
        let y = area.y + to_u16(rows) - 1;
        buf.set_line(x, y, &note, area.width);
        let over = Rect::new(x, y, area.right().saturating_sub(x), 1);
        m.pictures
            .wants
            .borrow_mut()
            .retain(|w| w.area.intersection(over).is_empty());
    }
}

/// The welcome's portrait, always in half blocks: real pixels belong to the sidebar alone, so the
/// card can scroll freely without a picture withdrawn and placed again under it. `at` is the
/// window's layout: how many head lines lie above it, the stack's skipped lines, the first row
/// drawn and the window's visible lines.
fn welcome_art(area: Rect, buf: &mut Buffer, at: (usize, usize, u16, usize)) {
    let (above, skip, first, shown) = at;
    let cols = crate::welcome::COLS;
    let (line, col) = crate::welcome::ART_AT;
    if usize::from(area.width) < usize::from(col + cols) + crate::kit::slab::FRAME {
        return;
    }
    let x = area.x + col;
    for (r, l) in crate::welcome::still().iter().enumerate() {
        let Some(idx) = (line + r).checked_sub(above) else {
            continue;
        };
        if idx >= skip && idx < shown {
            buf.set_line(x, first + to_u16(idx - skip), l, cols);
        }
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
    spans.push(Span::raw(" "));
    truncate(Line::from(spans), width.saturating_sub(3))
}

/// The top edge's right: the session's id, and its state — `○ idle`, `● running bash 3s`,
/// `◆ waiting for you`.
fn state(m: &Model) -> Line<'static> {
    let t = &m.theme;
    let mut spans = vec![Span::raw(" ")];
    if !m.session_id.is_empty() && m.size.0 >= 70 {
        spans.push(Span::styled("session ", t.muted()));
        spans.push(Span::styled(short_id(&m.session_id), t.text()));
        spans.push(Span::raw("  "));
    }
    let chip = crate::activity::chip(m);
    if m.size.0 < 60 {
        // Narrow, the state keeps its mark alone: the session's name comes first.
        if let Some(first) = chip.into_iter().next() {
            let mark: String = first.content.chars().take(1).collect();
            spans.push(Span::styled(mark, first.style));
        }
    } else {
        spans.extend(chip);
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// A session id as the header shows it: its last part, the rest of it the date.
fn short_id(id: &str) -> String {
    let id = crate::safe::one_line(id);
    let tail = id.rsplit('-').next().unwrap_or(&id);
    clip(tail, 12)
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
    let mut title = vec![Span::styled(
        small_caps("ask"),
        t.alarm().add_modifier(Modifier::BOLD),
    )];
    let hint = if m.gate.is_some() {
        title.push(Span::styled(" · answer the card first", t.gold()));
        String::new()
    } else if m.busy() {
        format!("enter queues for {}", crate::safe::one_line(&m.header.body))
    } else {
        "enter sends · alt+enter newline".to_owned()
    };
    let mut boxed = Boxed::new()
        .border(Border::Rounded, border)
        .padding(Sides::sym(0, 1))
        .width(w)
        .title(Line::from(title));
    if !hint.is_empty() && w >= 50 {
        boxed = boxed.title_right(Line::from(Span::styled(hint, t.muted())));
    }
    boxed.render(lines)
}
