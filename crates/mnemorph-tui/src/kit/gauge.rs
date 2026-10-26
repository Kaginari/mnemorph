//! A gauge, Bubbles' progress bar: a bar exactly some cells wide, filled to a fraction in a heavy
//! `━` over a quiet `─` track, each filled cell coloured by where it sits — so a context meter
//! warms as it nears its line.

use ratatui::style::Style;
use ratatui::text::Span;

/// A gauge's look.
#[derive(Debug, Clone, Copy)]
pub struct Gauge {
    /// The track's style.
    pub track: Style,
}

impl Gauge {
    /// The bar `cells` wide filled to `frac` (clamped to `0..=1`); `fill` styles a filled cell by
    /// its position along the bar, in `0..=1`.
    #[must_use]
    pub fn view(&self, frac: f64, cells: usize, fill: impl Fn(f64) -> Style) -> Vec<Span<'static>> {
        if cells == 0 {
            return Vec::new();
        }
        let frac = if frac.is_nan() {
            0.0
        } else {
            frac.clamp(0.0, 1.0)
        };
        // A cell count: small, and never negative after the clamp.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let filled = ((frac * cells as f64).round() as usize).min(cells);
        let mut spans: Vec<Span<'static>> = Vec::new();
        for i in 0..filled {
            // Display only: a cell's place along a short bar.
            #[allow(clippy::cast_precision_loss)]
            let at = i as f64 / (cells.max(2) - 1) as f64;
            let style = fill(at);
            // Neighbouring cells of one colour join into one span.
            match spans.last_mut() {
                Some(last) if last.style == style => last.content.to_mut().push('━'),
                _ => spans.push(Span::styled("━", style)),
            }
        }
        if filled < cells {
            spans.push(Span::styled("─".repeat(cells - filled), self.track));
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn it_fills_to_its_fraction_and_warms_along_the_bar() {
        let g = Gauge {
            track: Style::new().fg(Color::Gray),
        };
        let hot = |x: f64| Style::new().fg(if x >= 0.75 { Color::Red } else { Color::Cyan });
        assert_eq!(text(&g.view(0.5, 10, hot)), "━━━━━─────");
        assert_eq!(text(&g.view(2.0, 4, hot)), "━━━━");
        assert_eq!(text(&g.view(f64::NAN, 3, hot)), "───");
        assert_eq!(g.view(0.3, 0, hot).len(), 0);
        let full = g.view(1.0, 8, hot);
        assert_eq!(full.len(), 2, "cool cells, then hot ones: {full:?}");
        assert_eq!(full[1].style.fg, Some(Color::Red));
    }
}
