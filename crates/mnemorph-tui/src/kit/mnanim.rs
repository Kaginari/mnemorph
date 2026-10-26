//! The races' portraits, in the `.mnanim.json` format of Kaginari/mnemorph-animations (its
//! `FORMAT.md`, v1): a palette and palette-indexed frames of pixels, drawn two pixels to a cell
//! with half blocks — the rule of that repository's `player` (`Anim::cell`), rebuilt here in a
//! few lines rather than depended on.
//!
//! The loops ship inside the binary (`assets/anims/`, [`source`]): each race's idle loop at 32
//! pixels. Each is decoded once per process, on its first use, into its frames' cells, and kept:
//! a frame of the UI only reads the cache.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use mnemorph_races::Race;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::text::Lines;
use crate::theme::Rgb;

/// One cell: its glyph and the colours of its two pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// `▀`, `▄` or a space.
    pub glyph: char,
    /// The glyph's colour.
    pub fg: Option<Rgb>,
    /// The cell's background.
    pub bg: Option<Rgb>,
}

/// A decoded file: its size, its palette, its frames of palette indices (`0` transparent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anim {
    /// Pixels across.
    pub width: usize,
    /// Pixels down.
    pub height: usize,
    /// The colours; index `n` in a frame is `palette[n - 1]`.
    pub palette: Vec<Rgb>,
    /// The frames, row-major from the top-left.
    pub frames: Vec<Vec<u8>>,
    /// Frames a second.
    pub fps: u32,
}

/// A colour written `#rrggbb`.
fn colour(s: &str) -> Option<Rgb> {
    let h = s.strip_prefix('#').filter(|h| h.len() == 6)?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
}

impl Anim {
    /// A file's text decoded; why not, when it is not a v1 `mnanim`.
    ///
    /// # Errors
    ///
    /// Not JSON, not `mnanim` v1, a bad colour, a frame of the wrong size or naming a colour
    /// the palette lacks.
    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if doc["format"] != "mnanim" || doc["version"] != 1 {
            return Err("not an mnanim v1 file".into());
        }
        let size = |k: &str| {
            doc[k]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("no {k}"))
        };
        let (width, height) = (size("width")?, size("height")?);
        let digits = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(2))
            .ok_or("a frame too large")?;
        let palette = doc["palette"]
            .as_array()
            .ok_or("no palette")?
            .iter()
            .map(|c| c.as_str().and_then(colour).ok_or("a bad colour"))
            .collect::<Result<Vec<_>, _>>()?;
        let frames = doc["frames"]
            .as_array()
            .filter(|f| !f.is_empty())
            .ok_or("no frames")?
            .iter()
            .map(|f| {
                let hex = f.as_str().ok_or("a frame is not text")?;
                if !hex.is_ascii() {
                    return Err("a frame that is not hex");
                }
                if hex.len() != digits {
                    return Err("a frame of the wrong size");
                }
                (0..hex.len())
                    .step_by(2)
                    .map(|i| {
                        let v = u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| "bad hex")?;
                        if usize::from(v) > palette.len() {
                            return Err("an index past the palette");
                        }
                        Ok(v)
                    })
                    .collect()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fps = doc["fps"]
            .as_u64()
            .and_then(|f| u32::try_from(f).ok())
            .filter(|f| *f > 0)
            .unwrap_or(16);
        Ok(Self {
            width,
            height,
            palette,
            frames,
            fps,
        })
    }

    /// Pixel (`x`, `y`) of `frame`: `None` when transparent or outside.
    #[must_use]
    pub fn pixel(&self, frame: usize, x: usize, y: usize) -> Option<Rgb> {
        if x >= self.width || y >= self.height {
            return None;
        }
        match self.frames[frame % self.frames.len()][y * self.width + x] {
            0 => None,
            n => self.palette.get(usize::from(n) - 1).copied(),
        }
    }

    /// The cell at (`col`, `row`): the upper half block over two pixels, as the player draws it
    /// — a clear top over an opaque bottom is the lower half block, two clear pixels a space.
    #[must_use]
    pub fn cell(&self, frame: usize, col: usize, row: usize) -> Cell {
        let top = self.pixel(frame, col, row * 2);
        let bottom = self.pixel(frame, col, row * 2 + 1);
        match (top, bottom) {
            (None, None) => Cell {
                glyph: ' ',
                fg: None,
                bg: None,
            },
            (None, Some(b)) => Cell {
                glyph: '▄',
                fg: Some(b),
                bg: None,
            },
            (Some(t), b) => Cell {
                glyph: '▀',
                fg: Some(t),
                bg: b,
            },
        }
    }

    /// Every cell of `frame`, row by row: `height / 2` rows, rounded up.
    #[must_use]
    pub fn cells(&self, frame: usize) -> Vec<Vec<Cell>> {
        (0..self.height.div_ceil(2))
            .map(|row| {
                (0..self.width)
                    .map(|col| self.cell(frame, col, row))
                    .collect()
            })
            .collect()
    }
}

/// Cell rows as lines; a clear cell takes `under` as its background.
#[must_use]
pub fn lines(rows: &[Vec<Cell>], under: Option<Rgb>) -> Lines {
    rows.iter()
        .map(|row| {
            Line::from(
                row.iter()
                    .map(|c| {
                        let mut s = Style::new();
                        if let Some(f) = c.fg {
                            s = s.fg(f.color());
                        }
                        if let Some(b) = c.bg.or(under) {
                            s = s.bg(b.color());
                        }
                        Span::styled(c.glyph.to_string(), s)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

/// The portraits' size, in pixels: as many cells across, half as many rows down.
pub const SIZE: usize = 32;

/// The embedded idle loop of `race`.
#[must_use]
pub fn source(race: Race) -> &'static str {
    macro_rules! anim {
        ($name:literal) => {
            include_str!(concat!("../../assets/anims/", $name, ".mnanim.json"))
        };
    }
    match race {
        Race::Rimuru => anim!("rimuru"),
        Race::Elf => anim!("elf"),
        Race::Orc => anim!("orc"),
        Race::Slime => anim!("slime"),
        Race::Kijin => anim!("kijin"),
        Race::DarkElf => anim!("dark-elf"),
        Race::HighElf => anim!("high-elf"),
        Race::HighOrc => anim!("high-orc"),
        Race::NamedSlime => anim!("named-slime"),
        Race::Oni => anim!("oni"),
        Race::ArchDarkElf => anim!("arch-dark-elf"),
    }
}

/// A portrait decoded: every frame's cells, and its pace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loop {
    /// The frames, each rows of cells.
    pub frames: Vec<Vec<Vec<Cell>>>,
    /// Frames a second.
    pub fps: u32,
}

impl Loop {
    /// The frame on show `elapsed` into the loop: played at its own pace, whatever the UI's.
    #[must_use]
    pub fn at(&self, elapsed: std::time::Duration) -> &[Vec<Cell>] {
        let n = self.frames.len().max(1);
        // A frame count: small, the product of seconds and a frame rate.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let i = (elapsed.as_secs_f64() * f64::from(self.fps)) as usize % n;
        &self.frames[i]
    }
}

/// The portraits decoded so far, by race.
type Cache = Mutex<HashMap<Race, Option<Arc<Loop>>>>;

static DECODES: AtomicUsize = AtomicUsize::new(0);

/// How many times a portrait was decoded in this process: the cache's own instrument.
#[must_use]
pub fn decodes() -> usize {
    DECODES.load(Ordering::Relaxed)
}

/// `race`'s portrait, decoded on its first use, then read from the cache.
#[must_use]
pub fn portrait(race: Race) -> Option<Arc<Loop>> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
    cache
        .entry(race)
        .or_insert_with(|| {
            DECODES.fetch_add(1, Ordering::Relaxed);
            let anim = Anim::parse(source(race)).ok()?;
            Some(Arc::new(Loop {
                frames: (0..anim.frames.len()).map(|f| anim.cells(f)).collect(),
                fps: anim.fps,
            }))
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_two_by_two_file_is_one_row_of_cells() {
        // Top row: red, clear; bottom row: blue, blue.
        let text = r##"{"format":"mnanim","version":1,"name":"t","race":"slime","kind":"still",
            "width":2,"height":2,"fps":1,"loop":false,"background":"#000000",
            "palette":["#ff0000","#0000ff"],"frames":["01000202"]}"##;
        let a = Anim::parse(text).unwrap();
        let cells = a.cells(0);
        assert_eq!(cells.len(), 1);
        assert_eq!(
            cells[0],
            vec![
                Cell {
                    glyph: '▀',
                    fg: Some(Rgb(255, 0, 0)),
                    bg: Some(Rgb(0, 0, 255)),
                },
                Cell {
                    glyph: '▄',
                    fg: Some(Rgb(0, 0, 255)),
                    bg: None,
                },
            ]
        );
    }

    #[test]
    fn a_broken_file_is_refused() {
        let ok = |frame: &str, palette: &str| {
            Anim::parse(&format!(
                r#"{{"format":"mnanim","version":1,"width":1,"height":2,"palette":{palette},"frames":["{frame}"]}}"#
            ))
        };
        assert!(ok("0100", r##"["#ffffff"]"##).is_ok());
        assert!(ok("01", r##"["#ffffff"]"##).is_err(), "too short");
        assert!(ok("0200", r##"["#ffffff"]"##).is_err(), "past the palette");
        assert!(ok("0100", r#"["white"]"#).is_err(), "a bad colour");
        assert!(Anim::parse(r#"{"format":"gif"}"#).is_err());
    }

    #[test]
    fn every_embedded_loop_decodes_at_its_size() {
        for race in Race::ALL {
            let a = Anim::parse(source(race)).unwrap_or_else(|e| panic!("{race:?}: {e}"));
            assert_eq!((a.width, a.height), (SIZE, SIZE), "{race:?}");
            assert!(a.frames.len() > 1, "{race:?} is a loop");
        }
        assert_ne!(portrait(Race::Orc), portrait(Race::Elf), "each its own");
    }

    #[test]
    fn a_malformed_frame_is_an_error_never_a_panic() {
        let doc = |w: &str, frame: &str| {
            format!(
                r##"{{"format":"mnanim","version":1,"width":{w},"height":1,"palette":["#ffffff"],"frames":["{frame}"]}}"##
            )
        };
        // Four bytes of text, a multi-byte character among them: not hex, and no slice panics.
        assert!(Anim::parse(&doc("2", "0é0")).is_err());
        assert!(Anim::parse(&doc("2", "é01")).is_err());
        // A size whose frame length overflows is refused, not wrapped.
        assert!(Anim::parse(&doc(&u64::MAX.to_string(), "00")).is_err());
    }

    #[test]
    fn every_race_has_a_portrait_decoded_once() {
        for race in Race::ALL {
            let p = portrait(race).unwrap_or_else(|| panic!("{race:?}"));
            assert_ne!(p.frames.len(), 0);
            for f in &p.frames {
                assert_eq!(f.len(), SIZE / 2, "{race:?}");
                assert!(f.iter().all(|r| r.len() == SIZE));
            }
        }
        let after = decodes();
        for race in Race::ALL {
            let _ = portrait(race);
        }
        assert_eq!(decodes(), after, "read from the cache, not decoded again");
    }

    #[test]
    fn the_loop_plays_at_its_own_pace() {
        use std::time::Duration;
        let p = portrait(Race::Slime).unwrap();
        let n = p.frames.len();
        assert_eq!(p.at(Duration::ZERO), &p.frames[0][..]);
        // At 16 fps, a sixteenth of a second is the next frame, however often the UI draws.
        let step = Duration::from_secs(1) / p.fps;
        assert_eq!(p.at(step), &p.frames[1 % n][..]);
        assert_eq!(
            p.at(step * u32::try_from(n).unwrap()),
            &p.frames[0][..],
            "seamless"
        );
    }
}
