//! Lip Gloss for Ratatui: a chainable style that boxes a block of lines — padding, a border of
//! some kind with titles set into it, a fixed width and alignment, margins — and joins blocks
//! side by side or one above the other. Colours come from the theme, so a block re-renders in
//! the other variant by rendering again.

use ratatui::style::{Color, Modifier, Style as Paint};
use ratatui::text::{Line, Span};

use super::text::{Align, Lines, block_cells, fit, line_cells};

/// A border's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Border {
    /// None.
    #[default]
    None,
    /// Square corners: `┌─┐`.
    Normal,
    /// Rounded corners: `╭─╮`.
    Rounded,
    /// Heavy lines: `┏━┓`.
    Thick,
    /// Double lines: `╔═╗`.
    Double,
}

/// The glyphs of a border: corners top-left, top-right, bottom-left, bottom-right, then the
/// horizontal and the vertical.
struct Glyphs([&'static str; 6]);

impl Border {
    fn glyphs(self) -> Option<Glyphs> {
        Some(Glyphs(match self {
            Border::None => return None,
            Border::Normal => ["┌", "┐", "└", "┘", "─", "│"],
            Border::Rounded => ["╭", "╮", "╰", "╯", "─", "│"],
            Border::Thick => ["┏", "┓", "┗", "┛", "━", "┃"],
            Border::Double => ["╔", "╗", "╚", "╝", "═", "║"],
        }))
    }
}

/// Space on each side: top, right, bottom, left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sides {
    /// Above.
    pub top: u16,
    /// To the right.
    pub right: u16,
    /// Below.
    pub bottom: u16,
    /// To the left.
    pub left: u16,
}

impl Sides {
    /// `v` above and below, `h` left and right.
    #[must_use]
    pub const fn sym(v: u16, h: u16) -> Self {
        Self {
            top: v,
            right: h,
            bottom: v,
            left: h,
        }
    }
}

/// A box around a block of lines.
#[derive(Debug, Clone, Default)]
pub struct Style {
    text: Paint,
    padding: Sides,
    margin: Sides,
    border: Border,
    border_paint: Paint,
    width: Option<usize>,
    align: Align,
    title: Option<Line<'static>>,
    title_right: Option<Line<'static>>,
    footer: Option<Line<'static>>,
    open_bottom: bool,
}

impl Style {
    /// No box at all: the lines as they are.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The text's colour.
    #[must_use]
    pub fn fg(mut self, c: Color) -> Self {
        self.text = self.text.fg(c);
        self
    }

    /// The background, inside the border.
    #[must_use]
    pub fn bg(mut self, c: Color) -> Self {
        self.text = self.text.bg(c);
        self
    }

    /// Bold text.
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.text = self.text.add_modifier(Modifier::BOLD);
        self
    }

    /// Italic text.
    #[must_use]
    pub fn italic(mut self) -> Self {
        self.text = self.text.add_modifier(Modifier::ITALIC);
        self
    }

    /// Padding inside the border.
    #[must_use]
    pub fn padding(mut self, sides: Sides) -> Self {
        self.padding = sides;
        self
    }

    /// Margin outside the border.
    #[must_use]
    pub fn margin(mut self, sides: Sides) -> Self {
        self.margin = sides;
        self
    }

    /// The border's kind and colour.
    #[must_use]
    pub fn border(mut self, kind: Border, paint: Paint) -> Self {
        self.border = kind;
        self.border_paint = paint;
        self
    }

    /// No bottom edge: a card whose body is empty is one line, its title.
    #[must_use]
    pub fn open_bottom(mut self, open: bool) -> Self {
        self.open_bottom = open;
        self
    }

    /// The whole box's width, margins excluded; content is cut or padded to fit.
    #[must_use]
    pub fn width(mut self, cells: usize) -> Self {
        self.width = Some(cells);
        self
    }

    /// How each content line sits in the width.
    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// A title set into the top edge, on the left.
    #[must_use]
    pub fn title(mut self, title: Line<'static>) -> Self {
        self.title = Some(title);
        self
    }

    /// A title set into the top edge, on the right.
    #[must_use]
    pub fn title_right(mut self, title: Line<'static>) -> Self {
        self.title_right = Some(title);
        self
    }

    /// A line set into the bottom edge, on the left.
    #[must_use]
    pub fn footer(mut self, footer: Line<'static>) -> Self {
        self.footer = Some(footer);
        self
    }

    /// The cells the border and padding take across.
    #[must_use]
    pub fn frame_cells(&self) -> usize {
        let edge = usize::from(self.border != Border::None);
        2 * edge + usize::from(self.padding.left) + usize::from(self.padding.right)
    }

    /// `content` boxed.
    #[must_use]
    pub fn render(&self, content: Lines) -> Lines {
        let glyphs = self.border.glyphs();
        let edge = usize::from(glyphs.is_some());
        let pad_l = usize::from(self.padding.left);
        let pad_r = usize::from(self.padding.right);
        let inner = match self.width {
            Some(w) => w.saturating_sub(self.frame_cells()),
            None => block_cells(&content),
        };
        let outer = inner + pad_l + pad_r + 2 * edge;
        let fill = Paint::new().bg(self.text.bg.unwrap_or(Color::Reset));
        let fill = if self.text.bg.is_some() {
            fill
        } else {
            Paint::new()
        };
        let mut body: Lines = Vec::new();
        let blank = || Line::from(Span::styled(" ".repeat(inner), fill));
        body.extend((0..self.padding.top).map(|_| blank()));
        for line in content {
            let line = paint_under(line, self.text);
            body.push(fit(line, inner, self.align, fill));
        }
        body.extend((0..self.padding.bottom).map(|_| blank()));
        let mut out: Lines = Vec::new();
        if let Some(g) = &glyphs {
            out.push(self.edge_line(g, outer, true));
        }
        for line in body {
            let mut spans = Vec::new();
            if let Some(g) = &glyphs {
                spans.push(Span::styled(g.0[5], self.border_paint));
            }
            if pad_l > 0 {
                spans.push(Span::styled(" ".repeat(pad_l), fill));
            }
            spans.extend(line.spans);
            if pad_r > 0 {
                spans.push(Span::styled(" ".repeat(pad_r), fill));
            }
            if let Some(g) = &glyphs {
                spans.push(Span::styled(g.0[5], self.border_paint));
            }
            out.push(Line::from(spans));
        }
        if let Some(g) = &glyphs
            && !self.open_bottom
        {
            out.push(self.edge_line(g, outer, false));
        }
        self.with_margin(out, outer)
    }

    /// The top or bottom edge, its titles set in.
    fn edge_line(&self, glyphs: &Glyphs, outer: usize, top: bool) -> Line<'static> {
        let (start, end) = if top || self.open_bottom {
            (glyphs.0[0], glyphs.0[1])
        } else {
            (glyphs.0[2], glyphs.0[3])
        };
        let rule_glyph = glyphs.0[4];
        let paint = self.border_paint;
        let (left, right) = if top {
            (self.title.clone(), self.title_right.clone())
        } else {
            (self.footer.clone(), None)
        };
        let mut spans = vec![Span::styled(start, paint)];
        let mut used = 2;
        let room = outer.saturating_sub(2);
        let right_cells = right.as_ref().map_or(0, |r| line_cells(r) + 2);
        if let Some(title) = left {
            let room = room.saturating_sub(right_cells + 3);
            if room > 0 {
                let t = super::text::truncate(title, room);
                spans.push(Span::styled(rule_glyph, paint));
                spans.push(Span::raw(" "));
                used += 2 + line_cells(&t);
                spans.extend(t.spans);
                spans.push(Span::raw(" "));
                used += 1;
            }
        }
        let rule = outer.saturating_sub(used + right_cells + usize::from(right.is_some()));
        spans.push(Span::styled(rule_glyph.repeat(rule), paint));
        used += rule;
        if let Some(title) = right
            && used + right_cells < outer
        {
            spans.push(Span::raw(" "));
            spans.extend(title.spans);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(rule_glyph, paint));
        }
        spans.push(Span::styled(end, paint));
        Line::from(spans)
    }

    fn with_margin(&self, lines: Lines, outer: usize) -> Lines {
        let m = self.margin;
        let full = outer + usize::from(m.left) + usize::from(m.right);
        let mut out: Lines = (0..m.top).map(|_| Line::from(" ".repeat(full))).collect();
        for line in lines {
            if m.left == 0 && m.right == 0 {
                out.push(line);
                continue;
            }
            let mut spans = vec![Span::raw(" ".repeat(usize::from(m.left)))];
            spans.extend(line.spans);
            spans.push(Span::raw(" ".repeat(usize::from(m.right))));
            out.push(Line::from(spans));
        }
        out.extend((0..m.bottom).map(|_| Line::from(" ".repeat(full))));
        out
    }
}

/// A line whose spans take `under` wherever they set nothing themselves.
fn paint_under(line: Line<'static>, under: Paint) -> Line<'static> {
    if under == Paint::default() {
        return line;
    }
    let spans = line
        .spans
        .into_iter()
        .map(|s| {
            let style = under.patch(s.style);
            Span::styled(s.content, style)
        })
        .collect::<Vec<_>>();
    Line::from(spans).style(line.style)
}

/// Blocks one above the other, each line padded to the widest.
#[must_use]
pub fn join_vertical(blocks: Vec<Lines>, align: Align) -> Lines {
    let width = blocks.iter().map(|b| block_cells(b)).max().unwrap_or(0);
    blocks
        .into_iter()
        .flatten()
        .map(|l| fit(l, width, align, Paint::new()))
        .collect()
}

/// Blocks side by side, `gap` cells apart, aligned at the top; a shorter block is padded with
/// blank lines of its own width.
#[must_use]
pub fn join_horizontal(blocks: Vec<Lines>, gap: usize) -> Lines {
    let height = blocks.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = blocks.iter().map(|b| block_cells(b)).collect();
    let mut rows: Vec<Vec<Span<'static>>> = vec![Vec::new(); height];
    for (i, block) in blocks.into_iter().enumerate() {
        let w = widths[i];
        let mut lines = block.into_iter();
        for row in &mut rows {
            if i > 0 && gap > 0 {
                row.push(Span::raw(" ".repeat(gap)));
            }
            let line = lines.next().unwrap_or_default();
            row.extend(fit(line, w, Align::Left, Paint::new()).spans);
        }
    }
    rows.into_iter().map(Line::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::text::plain;

    fn texts(lines: &Lines) -> Vec<String> {
        lines.iter().map(plain).collect()
    }

    #[test]
    fn a_rounded_box_with_padding_and_titles() {
        let out = Style::new()
            .border(Border::Rounded, Paint::new())
            .padding(Sides::sym(0, 1))
            .width(20)
            .title(Line::from("read"))
            .title_right(Line::from("✓"))
            .render(vec![Line::from("hello")]);
        assert!(out.iter().all(|l| line_cells(l) == 20), "{:?}", texts(&out));
        assert!(plain(&out[0]).starts_with("╭─ read "));
        assert!(plain(&out[0]).ends_with(" ✓ ─╮"));
        assert_eq!(plain(&out[1]), "│ hello            │");
    }

    #[test]
    fn an_open_bottom_card_is_its_title_alone() {
        let out = Style::new()
            .border(Border::Normal, Paint::new())
            .open_bottom(true)
            .width(12)
            .title(Line::from("ls"))
            .render(vec![]);
        assert_eq!(texts(&out), ["┌─ ls ─────┐"]);
    }

    #[test]
    fn blocks_join_side_by_side_and_stacked() {
        let a = vec![Line::from("ab"), Line::from("c")];
        let b = vec![Line::from("xyz")];
        assert_eq!(
            texts(&join_horizontal(vec![a.clone(), b.clone()], 1)),
            ["ab xyz", "c     "]
        );
        assert_eq!(
            texts(&join_vertical(vec![a, b], Align::Right)),
            [" ab", "  c", "xyz"]
        );
    }
}
