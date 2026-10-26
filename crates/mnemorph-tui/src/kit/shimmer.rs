//! A shimmer: a crest of light sweeping across a word while work runs, as Isekai's busy verb
//! wears it — the runes within four of the crest blend from the text's colour toward the
//! accent, the crest at full accent. Advanced by the spinner's frames.

use ratatui::style::Style;
use ratatui::text::Span;

use crate::theme::Rgb;

/// How far either side of the crest the light reaches, in characters.
const REACH: usize = 4;

/// `text` lit by a crest at `tick`, from `base` toward `accent`; the crest runs [`REACH`] past
/// each end before it comes round again.
#[must_use]
pub fn shimmer(
    text: &str,
    tick: usize,
    base: Rgb,
    accent: Rgb,
    style: Style,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let crest = (tick % (n + 2 * REACH)).cast_signed() - REACH.cast_signed();
    chars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let d = (i.cast_signed() - crest).unsigned_abs();
            let colour = if d > REACH {
                base
            } else {
                // Display only: a blend step of a short ramp.
                #[allow(clippy::cast_precision_loss)]
                let k = 1.0 - d as f32 / (REACH + 1) as f32;
                base.mix(accent, k)
            };
            Span::styled(c.to_string(), style.fg(colour.color()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crest_sweeps_across_the_word() {
        let (base, accent) = (Rgb(0, 0, 0), Rgb(200, 200, 200));
        let at = |tick| {
            shimmer("Thinking", tick, base, accent, Style::new())
                .iter()
                .map(|s| s.style.fg)
                .collect::<Vec<_>>()
        };
        // The crest starts before the word: its first letter only catches the edge.
        assert_ne!(at(4)[0], Some(base.color()));
        assert_eq!(at(4)[0], Some(accent.color()));
        assert_eq!(at(4)[7], Some(base.color()));
        assert_eq!(at(11)[7], Some(accent.color()));
        // It comes round again.
        assert_eq!(at(4), at(4 + 8 + 2 * REACH));
    }
}
