//! Text-only terminals: a picture as glyph cells, from half blocks (two pixels a cell) down to
//! octants (eight). Vendored from Kaginari/mnemorph-animations `player/src/glyphs.rs` at commit
//! `ff8af6d`, on this crate's `image` buffers and cells, its rule unchanged.
//!
//! A cell has one foreground and one background colour, so a glyph finer than a half block shows
//! its sub-pixels in two colours: [`cells`] tries every split of a cell's sub-pixels into two
//! groups and keeps the one with the least colour error, each group drawn in its mean colour.
//! Half blocks lose nothing; the finer glyphs trade a little colour at sharp edges for two to four
//! times the pixels. Pixels below half alpha are clear: a cell with any shows its opaque
//! sub-pixels on the cells' own background.

use image::RgbaImage;

use super::glyph_tables::{OCTANTS, SEXTANTS};
use super::mnanim::Cell;
use crate::theme::Rgb;

/// Which glyphs draw the sub-pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Glyphs {
    /// `▀` `▄`: 1 × 2 a cell; every font has them, and no colour is lost.
    #[default]
    Half,
    /// `▘` `▚` `▟`…: 2 × 2 a cell; every font with block elements has them.
    Quadrants,
    /// Unicode 13 sextants: 2 × 3 a cell.
    Sextants,
    /// Unicode 16 octants: 2 × 4 a cell, square sub-pixels on a 1 : 2 cell.
    Octants,
}

const HALVES: [char; 4] = [' ', '▀', '▄', '█'];
const QUADRANTS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

impl Glyphs {
    /// Every set, coarsest first.
    pub const ALL: [Self; 4] = [Self::Half, Self::Quadrants, Self::Sextants, Self::Octants];

    /// Sub-pixels a cell holds: across, down.
    #[must_use]
    pub fn grid(self) -> (usize, usize) {
        match self {
            Self::Half => (1, 2),
            Self::Quadrants => (2, 2),
            Self::Sextants => (2, 3),
            Self::Octants => (2, 4),
        }
    }

    /// The glyph whose set sub-pixels are `mask`'s bits, row-major from the top-left.
    #[must_use]
    pub fn glyph(self, mask: usize) -> char {
        match self {
            Self::Half => HALVES[mask & 3],
            Self::Quadrants => QUADRANTS[mask & 15],
            Self::Sextants => SEXTANTS[mask & 63],
            Self::Octants => OCTANTS[mask & 255],
        }
    }

    /// Its name, as `MNEMORPH_TUI_GLYPHS` takes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Half => "half",
            Self::Quadrants => "quadrants",
            Self::Sextants => "sextants",
            Self::Octants => "octants",
        }
    }

    /// A name read back: `half` (or `blocks`), `quadrants`, `sextants`, `octants`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "half" | "blocks" | "halfblocks" => Some(Self::Half),
            "quadrants" => Some(Self::Quadrants),
            "sextants" => Some(Self::Sextants),
            "octants" => Some(Self::Octants),
            _ => None,
        }
    }
}

/// The picture as rows of cells, `ceil(w / across)` by `ceil(h / down)`; sub-pixels past its
/// edge are clear.
#[must_use]
pub fn cells(im: &RgbaImage, glyphs: Glyphs) -> Vec<Vec<Cell>> {
    let (gw, gh) = glyphs.grid();
    let (w, h) = (im.width() as usize, im.height() as usize);
    let at = |x: usize, y: usize| -> Option<[u8; 3]> {
        if x >= w || y >= h {
            return None;
        }
        // x and y are inside the picture, whose sides are u32.
        let p = im
            .get_pixel(u32::try_from(x).ok()?, u32::try_from(y).ok()?)
            .0;
        (p[3] >= 128).then_some([p[0], p[1], p[2]])
    };
    (0..h.div_ceil(gh))
        .map(|row| {
            (0..w.div_ceil(gw))
                .map(|col| {
                    let subs: Vec<Option<[u8; 3]>> = (0..gw * gh)
                        .map(|i| at(col * gw + i % gw, row * gh + i / gw))
                        .collect();
                    if glyphs == Glyphs::Half {
                        half(subs[0], subs[1])
                    } else {
                        split(&subs, glyphs)
                    }
                })
                .collect()
        })
        .collect()
}

fn rgb(p: [u8; 3]) -> Rgb {
    Rgb(p[0], p[1], p[2])
}

const CLEAR: Cell = Cell {
    glyph: ' ',
    fg: None,
    bg: None,
};

/// The half-block rule: `▀` with the top pixel as its colour, `▄` under a clear top.
fn half(top: Option<[u8; 3]>, bottom: Option<[u8; 3]>) -> Cell {
    match (top, bottom) {
        (None, None) => CLEAR,
        (None, Some(b)) => Cell {
            glyph: '▄',
            fg: Some(rgb(b)),
            bg: None,
        },
        (Some(t), b) => Cell {
            glyph: '▀',
            fg: Some(rgb(t)),
            bg: b.map(rgb),
        },
    }
}

fn mean(px: impl Iterator<Item = [u8; 3]>) -> [u8; 3] {
    let (mut s, mut n) = ([0u32; 3], 0u32);
    for p in px {
        for k in 0..3 {
            s[k] += u32::from(p[k]);
        }
        n += 1;
    }
    let n = n.max(1);
    s.map(|v| u8::try_from((v + n / 2) / n).unwrap_or(u8::MAX))
}

/// The best two-colour cell for the sub-pixels `subs`.
#[allow(clippy::many_single_char_names)]
fn split(subs: &[Option<[u8; 3]>], glyphs: Glyphs) -> Cell {
    let n = subs.len();
    let opaque: usize = (0..n).filter(|&i| subs[i].is_some()).map(|i| 1 << i).sum();
    if opaque == 0 {
        return CLEAR;
    }
    if opaque != (1usize << n) - 1 {
        // The clear sub-pixels are the background: the opaque ones are the glyph.
        return Cell {
            glyph: glyphs.glyph(opaque),
            fg: Some(rgb(mean(subs.iter().flatten().copied()))),
            bg: None,
        };
    }
    let px: Vec<[u8; 3]> = subs.iter().flatten().copied().collect();
    if px.iter().all(|p| *p == px[0]) {
        return Cell {
            glyph: ' ',
            fg: None,
            bg: Some(rgb(px[0])),
        };
    }
    // Every split; a mask and its complement are the same split, so bit n-1 stays clear.
    let (mut best, mut best_err) = (0usize, f64::INFINITY);
    for mask in 1..(1usize << (n - 1)) {
        let mut err = 0.0;
        for side in [true, false] {
            let (mut s, mut q, mut c) = ([0i64; 3], 0i64, 0i64);
            for (i, p) in px.iter().enumerate() {
                if (mask >> i & 1 == 1) == side {
                    for k in 0..3 {
                        let v = i64::from(p[k]);
                        s[k] += v;
                        q += v * v;
                    }
                    c += 1;
                }
            }
            // The group's squared error about its mean: its sum of squares less (sum)² / count.
            // Sums of at most eight sub-pixels: far inside f64's exact integers.
            #[allow(clippy::cast_precision_loss)]
            let e = q as f64 - s.iter().map(|v| (v * v) as f64).sum::<f64>() / c as f64;
            err += e;
        }
        if err < best_err {
            (best, best_err) = (mask, err);
        }
    }
    let group = |side: bool| {
        mean(
            px.iter()
                .enumerate()
                .filter(move |(i, _)| (best >> i & 1 == 1) == side)
                .map(|(_, p)| *p),
        )
    };
    Cell {
        glyph: glyphs.glyph(best),
        fg: Some(rgb(group(true))),
        bg: Some(rgb(group(false))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| image::Rgba(f(x, y)))
    }

    #[test]
    fn half_blocks_keep_both_pixels() {
        let im = picture(1, 2, |_, y| {
            if y == 0 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        });
        let c = cells(&im, Glyphs::Half);
        assert_eq!(
            c,
            vec![vec![Cell {
                glyph: '▀',
                fg: Some(Rgb(255, 0, 0)),
                bg: Some(Rgb(0, 0, 255)),
            }]]
        );
    }

    #[test]
    fn a_two_colour_cell_splits_exactly() {
        // Left column white, right column black: the left half block, white on black.
        for g in [Glyphs::Quadrants, Glyphs::Sextants, Glyphs::Octants] {
            let (gw, gh) = g.grid();
            let im = picture(
                u32::try_from(gw).unwrap(),
                u32::try_from(gh).unwrap(),
                |x, _| if x == 0 { [255; 4] } else { [0, 0, 0, 255] },
            );
            let c = cells(&im, g)[0][0];
            let (ink, paper) = (Some(Rgb(255, 255, 255)), Some(Rgb(0, 0, 0)));
            assert!(
                (c.fg, c.bg) == (ink, paper) || (c.fg, c.bg) == (paper, ink),
                "{g:?}: {c:?}"
            );
            assert!(c.glyph == '▌' || c.glyph == '▐', "{g:?}: {c:?}");
        }
    }

    #[test]
    fn clear_sub_pixels_are_the_background() {
        let im = picture(2, 4, |_, y| if y < 2 { [9, 9, 9, 255] } else { [0; 4] });
        let c = cells(&im, Glyphs::Octants)[0][0];
        assert_eq!((c.fg, c.bg), (Some(Rgb(9, 9, 9)), None));
        assert_eq!(
            cells(&picture(2, 4, |_, _| [0; 4]), Glyphs::Octants)[0][0],
            CLEAR
        );
        assert_eq!(Glyphs::parse(" Octants"), Some(Glyphs::Octants));
        for g in Glyphs::ALL {
            assert_eq!(Glyphs::parse(g.name()), Some(g));
        }
    }
}
