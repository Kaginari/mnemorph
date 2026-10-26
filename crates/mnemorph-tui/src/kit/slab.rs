//! Depth for Ratatui: a raised panel drawn in the cell grid with colour and block elements
//! alone, as Isekai's frame draws its cards. The page is the plane; a slab stands on it with a
//! lit top edge, a face tinted a few percent toward the hue that says what it is (the human's
//! gold, the answer's cyan, a tool's class), a spine in that hue down its left edge, a shaded
//! right and bottom bevel, and a shadow one cell right and half a row down — tinted from the
//! plane, never black. A float (a toast, the approval) casts the deeper shadow.
//!
//! ```text
//! ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
//! ▌ ● bash  pytest  ▕▄▖
//! ▌   ⎿ ok · 2.0s    ▕ ▌
//! ▝▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀ ▘
//! ```
//!
//! Every tone is a step of one scale taken from the theme's own palette, so a slab re-renders in
//! the other variant by rendering again.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::text::{Align, Lines, fit};
use crate::theme::{Rgb, Theme, Variant};

/// The columns a slab's shadow takes right of its face: the near one dark, the far a fade.
pub const SHADOW: usize = 2;

/// The cells a slab's frame takes from its width: the spine, a space each side, the right
/// bevel, and the shadow.
pub const FRAME: usize = 4 + SHADOW;

/// One surface's tones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surface {
    /// The face the content sits on.
    pub face: Rgb,
    /// The header strip: the face a breath lit, and a breath more of the hue.
    pub band: Rgb,
    /// The lit top edge.
    pub gloss: Rgb,
    /// The shaded right and bottom bevel.
    pub shade: Rgb,
    /// The shadow on the plane, and its far, fading column.
    pub shadow: (Rgb, Rgb),
    /// The spine down the left edge, in the surface's hue.
    pub spine: Option<Rgb>,
}

/// The tonal scale of a theme: the plane under everything, a card on it, a float over it, and
/// the tones a lit edge and a shaded one move toward.
#[derive(Debug, Clone, Copy)]
struct Scale {
    plane: Rgb,
    card: Rgb,
    float: Rgb,
    lift: Rgb,
    sink: Rgb,
    dark: bool,
}

impl Scale {
    fn of(t: &Theme) -> Self {
        let p = t.p;
        match t.variant {
            Variant::Dark => Self {
                plane: p.bg,
                card: p.panel.mix(p.panel_raised, 0.7),
                float: p.panel_raised.mix(p.line, 0.35),
                lift: p.text,
                sink: Rgb(0, 0, 0),
                dark: true,
            },
            Variant::Light => Self {
                plane: p.bg,
                card: p.panel,
                float: p.panel,
                lift: Rgb(255, 255, 255),
                sink: p.text_muted,
                dark: false,
            },
        }
    }
}

impl Surface {
    /// A card raised on the page, tinted `k` toward `hue`, its spine in `hue`.
    #[must_use]
    pub fn card(t: &Theme, hue: Option<Rgb>, k: f32) -> Self {
        let s = Scale::of(t);
        Self::raise(&s, s.card, hue, k, false)
    }

    /// A float over the frame: the deepest shadow.
    #[must_use]
    pub fn float(t: &Theme, hue: Option<Rgb>, k: f32) -> Self {
        let s = Scale::of(t);
        Self::raise(&s, s.float, hue, k, true)
    }

    fn raise(s: &Scale, base: Rgb, hue: Option<Rgb>, k: f32, deep: bool) -> Self {
        let face = hue.map_or(base, |h| base.mix(h, k));
        let (mut band, mut gloss, shade, shadow) = if s.dark {
            (
                face.mix(s.lift, 0.05),
                face.mix(s.lift, 0.3),
                face.mix(s.sink, 0.42),
                s.plane.mix(s.sink, if deep { 0.95 } else { 0.78 }),
            )
        } else {
            (
                face.mix(s.plane, 0.45),
                face.mix(s.sink, 0.12),
                face.mix(s.sink, 0.3),
                s.plane.mix(s.sink, if deep { 0.34 } else { 0.22 }),
            )
        };
        if let Some(h) = hue {
            band = band.mix(h, k * 0.6);
            gloss = gloss.mix(h, k * 1.2);
        }
        Self {
            face,
            band,
            gloss,
            shade,
            shadow: (shadow, shadow.mix(s.plane, 0.5)),
            spine: hue,
        }
    }
}

/// A slab: a surface, which of its edges it draws, and how many rows of its top are the band.
/// A slab whose top or bottom is open continues another: an answer streamed in pieces reads as
/// one panel.
#[derive(Debug, Clone, Copy)]
pub struct Slab {
    /// The tones.
    pub surface: Surface,
    /// Draw the lit top edge.
    pub top: bool,
    /// Draw the bottom bevel.
    pub bottom: bool,
    /// The first rows on the band's tone.
    pub band: usize,
}

impl Slab {
    /// A whole slab of `surface`, no band.
    #[must_use]
    pub fn new(surface: Surface) -> Self {
        Self {
            surface,
            top: true,
            bottom: true,
            band: 0,
        }
    }

    /// The first `rows` on the band.
    #[must_use]
    pub fn band(mut self, rows: usize) -> Self {
        self.band = rows;
        self
    }

    /// Which edges it draws.
    #[must_use]
    pub fn edges(mut self, top: bool, bottom: bool) -> Self {
        self.top = top;
        self.bottom = bottom;
        self
    }

    /// The content's width inside a slab `width` cells wide.
    #[must_use]
    pub fn inner(width: usize) -> usize {
        width.saturating_sub(FRAME)
    }

    /// `rows` raised on the slab, `width` cells wide with its shadow.
    #[must_use]
    pub fn render(&self, rows: Lines, width: usize) -> Lines {
        let s = &self.surface;
        let bw = width.saturating_sub(SHADOW).max(4);
        let inner = bw - 4;
        let fg = |c: Rgb| Style::new().fg(c.color());
        let on = |f: Rgb, b: Rgb| Style::new().fg(f.color()).bg(b.color());
        let (shadow, far) = s.shadow;
        let mut out: Lines = Vec::with_capacity(rows.len() + 2);
        if self.top {
            out.push(Line::from(vec![
                Span::styled("▄".repeat(bw), fg(s.gloss)),
                Span::raw(" ".repeat(SHADOW)),
            ]));
        }
        for (i, row) in rows.into_iter().enumerate() {
            let face = if i < self.band { s.band } else { s.face };
            let left = match s.spine {
                Some(h) => Span::styled("▌", on(h, face)),
                None => Span::styled("▏", on(s.gloss, face)),
            };
            let bg = Style::new().bg(face.color());
            let mut spans = vec![left, Span::styled(" ", bg)];
            let row = Line::from(
                row.spans
                    .into_iter()
                    .map(|sp| {
                        let style = if sp.style.bg.is_none() {
                            sp.style.bg(face.color())
                        } else {
                            sp.style
                        };
                        Span::styled(sp.content, style)
                    })
                    .collect::<Vec<_>>(),
            );
            spans.extend(fit(row, inner, Align::Left, bg).spans);
            spans.push(Span::styled(" ", bg));
            spans.push(Span::styled("▕", on(s.shade, face.mix(s.shade, 0.4))));
            if i == 0 && self.top {
                spans.push(Span::styled("▄", fg(shadow)));
                spans.push(Span::styled("▖", fg(far)));
            } else {
                spans.push(Span::styled(" ", Style::new().bg(shadow.color())));
                spans.push(Span::styled("▌", fg(far)));
            }
            out.push(Line::from(spans));
        }
        if self.bottom {
            out.push(Line::from(vec![
                Span::styled("▝", fg(s.shade)),
                Span::styled("▀", on(s.shade, far)),
                Span::styled("▀".repeat(bw - 2), on(s.shade, shadow)),
                Span::styled(" ", Style::new().bg(shadow.color())),
                Span::styled("▘", fg(far)),
            ]));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::text::{line_cells, plain};
    use crate::theme::Variant;

    #[test]
    fn a_slab_keeps_its_width_and_casts_its_shadow() {
        let t = Theme::new(Variant::Dark);
        let s = Surface::card(&t, Some(t.races.rimuru), 0.1);
        let lines = Slab::new(s)
            .band(1)
            .render(vec![Line::from("head"), Line::from("body")], 20);
        assert_eq!(lines.len(), 4);
        for l in &lines {
            assert_eq!(line_cells(l), 20, "{:?}", plain(l));
        }
        assert!(plain(&lines[0]).starts_with("▄▄▄"));
        assert!(plain(&lines[1]).starts_with("▌ head"));
        assert!(plain(&lines[1]).ends_with("▕▄▖"));
        assert!(plain(&lines[2]).ends_with("▕ ▌"));
        assert!(plain(&lines[3]).starts_with("▝▀"));
        // The band is a breath lighter than the face; the spine wears the hue.
        assert_ne!(s.band, s.face);
        assert_eq!(lines[1].spans[0].style.fg, Some(t.races.rimuru.color()));
    }

    #[test]
    fn an_open_slab_continues_another() {
        let t = Theme::new(Variant::Light);
        let lines = Slab::new(Surface::float(&t, None, 0.0))
            .edges(false, false)
            .render(vec![Line::from("more")], 16);
        assert_eq!(lines.len(), 1);
        assert!(plain(&lines[0]).starts_with("▏ more"));
    }
}
