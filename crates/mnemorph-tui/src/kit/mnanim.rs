//! The races' animations, in the `.mnanim.json` format of Kaginari/mnemorph-animations (its
//! `FORMAT.md`, v2): a JSON header with a palette, and the frames in an 8-bit indexed PNG sprite
//! sheet beside it. The sheet's decoding and the glyph renderer ([`super::glyphs`]) are vendored
//! from that repository's `player` crate (`mnanim`) at commit `ff8af6d`, rebuilt on this crate's
//! `image` buffers: the repository is private, so a git dependency could not be fetched by CI.
//!
//! The clips ship inside the binary (`assets/anims/256px/`, [`source`]): each race's idle loop
//! (48 frames at 16 fps, seamless) and the five ascensions (80 frames, played once), all drawn
//! natively at 256 pixels. A terminal that draws real pixels is sent the frames themselves
//! ([`crate::pictures`]); every other terminal gets them as glyph cells — octants, sextants,
//! quadrants or half blocks, as fine as its font draws ([`set_glyphs`]) — decoded once per clip
//! into [`SIZE`] columns, and kept: a frame of the UI only reads the cache.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use mnemorph_races::Race;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::glyphs::{self, Glyphs};
use super::text::Lines;
use crate::theme::Rgb;

/// One cell: its glyph and its two colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// A block glyph, or a space.
    pub glyph: char,
    /// The glyph's colour.
    pub fg: Option<Rgb>,
    /// The cell's background; `None` leaves the cells' own.
    pub bg: Option<Rgb>,
}

/// An animation: a race's idle loop, or a base race's ascension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Clip {
    /// The race's idle loop, seamless.
    Idle(Race),
    /// The base race ascending to its risen form, played once: `ascend-<base>`.
    Ascend(Race),
}

impl Clip {
    /// The ascension of `base`, when it ascends.
    #[must_use]
    pub fn ascension(base: Race) -> Option<Self> {
        base.ascends_to().map(|_| Self::Ascend(base))
    }

    /// Its file's stem.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Idle(r) => r.name().to_owned(),
            Self::Ascend(r) => format!("ascend-{}", r.name()),
        }
    }

    /// Its place among the clips, 0 to 21: what its Kitty image ids are numbered from.
    #[must_use]
    pub fn index(self) -> u32 {
        let at = |r: Race| {
            Race::ALL
                .iter()
                .position(|x| *x == r)
                .and_then(|i| u32::try_from(i).ok())
                .unwrap_or(0)
        };
        match self {
            Self::Idle(r) => at(r),
            Self::Ascend(r) => 11 + at(r),
        }
    }

    /// Its frames: 48 for an idle loop, 80 for an ascension.
    #[must_use]
    pub fn frames(self) -> usize {
        if self.looping() { 48 } else { 80 }
    }

    /// Whether it loops.
    #[must_use]
    pub fn looping(self) -> bool {
        matches!(self, Self::Idle(_))
    }

    /// What plays once it is over: the risen form's idle loop after an ascension.
    #[must_use]
    pub fn then(self) -> Self {
        match self {
            Self::Ascend(b) => Self::Idle(b.ascends_to().unwrap_or(b)),
            idle @ Self::Idle(_) => idle,
        }
    }
}

/// A clip's embedded files: its JSON and its sprite sheet.
#[must_use]
pub fn source(clip: Clip) -> (&'static str, &'static [u8]) {
    macro_rules! anim {
        ($name:literal) => {
            (
                include_str!(concat!("../../assets/anims/256px/", $name, ".mnanim.json")),
                include_bytes!(concat!("../../assets/anims/256px/", $name, ".mnanim.png")),
            )
        };
    }
    match clip {
        Clip::Idle(Race::Rimuru) | Clip::Ascend(Race::Rimuru) => anim!("rimuru"),
        Clip::Idle(Race::Elf) => anim!("elf"),
        Clip::Idle(Race::Orc) => anim!("orc"),
        Clip::Idle(Race::Slime) => anim!("slime"),
        Clip::Idle(Race::Kijin) => anim!("kijin"),
        Clip::Idle(Race::DarkElf) => anim!("dark-elf"),
        Clip::Idle(Race::HighElf) => anim!("high-elf"),
        Clip::Idle(Race::HighOrc) => anim!("high-orc"),
        Clip::Idle(Race::NamedSlime) => anim!("named-slime"),
        Clip::Idle(Race::Oni) => anim!("oni"),
        Clip::Idle(Race::ArchDarkElf) => anim!("arch-dark-elf"),
        Clip::Ascend(Race::Elf | Race::HighElf) => anim!("ascend-elf"),
        Clip::Ascend(Race::Orc | Race::HighOrc) => anim!("ascend-orc"),
        Clip::Ascend(Race::Slime | Race::NamedSlime) => anim!("ascend-slime"),
        Clip::Ascend(Race::Kijin | Race::Oni) => anim!("ascend-kijin"),
        Clip::Ascend(Race::DarkElf | Race::ArchDarkElf) => anim!("ascend-dark-elf"),
    }
}

/// A decoded clip: its size, pace, palette and frames of palette indices (`0` clear).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anim {
    /// Pixels across.
    pub width: usize,
    /// Pixels down.
    pub height: usize,
    /// Frames a second.
    pub fps: u32,
    /// Whether frame 0 follows the last.
    pub looping: bool,
    /// The colours; index `n` in a frame is `palette[n - 1]`.
    pub palette: Vec<Rgb>,
    /// The frames, row-major from the top-left.
    pub frames: Vec<Vec<u8>>,
}

/// A colour written `#rrggbb`.
fn colour(s: &str) -> Option<Rgb> {
    let h = s.strip_prefix('#').filter(|h| h.len() == 6)?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
}

impl Anim {
    /// A v2 file decoded from its JSON `text` and its sheet `png`.
    ///
    /// # Errors
    ///
    /// Not JSON, not `mnanim` v2, a bad colour, a sheet that is not 8-bit indexed, of the wrong
    /// size, or naming a colour the palette lacks.
    pub fn parse(text: &str, png: &[u8]) -> Result<Self, String> {
        let doc: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if doc["format"] != "mnanim" || doc["version"] != 2 {
            return Err("not an mnanim v2 file".into());
        }
        let size = |v: &Value, k: &str| {
            v[k].as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n > 0 && *n <= 4096)
                .ok_or_else(|| format!("no {k}"))
        };
        let (width, height) = (size(&doc, "width")?, size(&doc, "height")?);
        let frames = &doc["frames"];
        let (columns, count) = (size(frames, "columns")?, size(frames, "count")?);
        let palette = doc["palette"]
            .as_array()
            .filter(|p| p.len() <= 255)
            .ok_or("no palette")?
            .iter()
            .map(|c| c.as_str().and_then(colour).ok_or("a bad colour"))
            .collect::<Result<Vec<_>, _>>()?;
        let fps = doc["fps"]
            .as_u64()
            .and_then(|f| u32::try_from(f).ok())
            .filter(|f| *f > 0)
            .unwrap_or(16);
        let looping = doc["loop"].as_bool().unwrap_or(true);
        let frames = sheet(png, (width, height), (columns, count), palette.len())?;
        Ok(Self {
            width,
            height,
            fps,
            looping,
            palette,
            frames,
        })
    }

    /// Frame `n` as RGBA: clear pixels transparent.
    #[must_use]
    #[allow(clippy::many_single_char_names)]
    pub fn rgba(&self, n: usize) -> RgbaImage {
        let frame = &self.frames[n % self.frames.len()];
        let (w, h) = (to_u32(self.width), to_u32(self.height));
        RgbaImage::from_fn(w, h, |x, y| {
            let i = frame[y as usize * self.width + x as usize];
            match i
                .checked_sub(1)
                .and_then(|i| self.palette.get(usize::from(i)))
            {
                Some(c) => Rgba([c.0, c.1, c.2, 255]),
                None => Rgba([0, 0, 0, 0]),
            }
        })
    }

    /// Frame `n` laid on `bg` in a box of `w` × `h` pixels, as the player's `--native` draws it:
    /// one frame pixel a screen pixel, centred, when the frame fits the box; else shrunk
    /// (Lanczos) to the largest square that does. Exactly the box's pixels, so the terminal never
    /// rescales it.
    #[must_use]
    pub fn boxed(&self, n: usize, w: u32, h: u32, bg: Rgb) -> image::RgbImage {
        let (w, h) = (w.max(1), h.max(1));
        let frame = self.rgba(n);
        let fill = image::Rgb([bg.0, bg.1, bg.2]);
        let flat = image::RgbImage::from_fn(frame.width(), frame.height(), |x, y| {
            let p = frame.get_pixel(x, y).0;
            if p[3] >= 128 {
                image::Rgb([p[0], p[1], p[2]])
            } else {
                fill
            }
        });
        let flat = if flat.width() <= w && flat.height() <= h {
            flat
        } else {
            let side = w.min(h);
            imageops::resize(&flat, side, side, FilterType::Lanczos3)
        };
        let mut out = image::RgbImage::from_pixel(w, h, fill);
        imageops::replace(
            &mut out,
            &flat,
            i64::from((w - flat.width()) / 2),
            i64::from((h - flat.height()) / 2),
        );
        out
    }

    /// Frame `n` as `glyphs` cells, [`SIZE`] columns by `SIZE / 2` rows: shrunk to the box's
    /// sub-pixels, square on a 1 : 2 cell.
    #[must_use]
    pub fn cells(&self, n: usize, glyphs: Glyphs) -> Vec<Vec<Cell>> {
        let (gw, gh) = glyphs.grid();
        glyphs::cells(&self.shrink(n, SIZE * gw, SIZE / 2 * gh), glyphs)
    }

    /// Frame `n` shrunk to `w` × `h` by area: each pixel the mean colour of the opaque frame
    /// pixels it covers, as opaque as their share of it — so a clear pixel's colour never
    /// bleeds in. One pass over the frame: cheap enough to decode every clip at start.
    #[must_use]
    pub fn shrink(&self, n: usize, w: usize, h: usize) -> RgbaImage {
        let frame = &self.frames[n % self.frames.len()];
        let (w, h) = (w.clamp(1, self.width), h.clamp(1, self.height));
        let span = |o: usize, out: usize, of: usize| {
            (o * of / out, ((o + 1) * of / out).max(o * of / out + 1))
        };
        let mut img = RgbaImage::new(to_u32(w), to_u32(h));
        for oy in 0..h {
            let (y0, y1) = span(oy, h, self.height);
            for ox in 0..w {
                let (x0, x1) = span(ox, w, self.width);
                let (mut sum, mut lit) = ([0u32; 3], 0u32);
                for y in y0..y1 {
                    for &i in &frame[y * self.width + x0..y * self.width + x1] {
                        if let Some(c) = i
                            .checked_sub(1)
                            .and_then(|i| self.palette.get(usize::from(i)))
                        {
                            sum[0] += u32::from(c.0);
                            sum[1] += u32::from(c.1);
                            sum[2] += u32::from(c.2);
                            lit += 1;
                        }
                    }
                }
                let area = to_u32((y1 - y0) * (x1 - x0)).max(1);
                let px = if lit == 0 {
                    Rgba([0, 0, 0, 0])
                } else {
                    let mean = |v: u32| u8::try_from((v + lit / 2) / lit).unwrap_or(u8::MAX);
                    let alpha = u8::try_from(lit * 255 / area).unwrap_or(u8::MAX);
                    Rgba([mean(sum[0]), mean(sum[1]), mean(sum[2]), alpha])
                };
                img.put_pixel(to_u32(ox), to_u32(oy), px);
            }
        }
        img
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Cut a v2 sheet — 8-bit indexed, frames left to right then top to bottom — into frames of
/// palette indices, as the player's `sheet_frames` does.
fn sheet(
    png_bytes: &[u8],
    (width, height): (usize, usize),
    (columns, count): (usize, usize),
    colours: usize,
) -> Result<Vec<Vec<u8>>, String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
    dec.set_transformations(png::Transformations::IDENTITY);
    let mut reader = dec.read_info().map_err(|e| format!("sheet: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("sheet: {e}"))?;
    if info.color_type != png::ColorType::Indexed || info.bit_depth != png::BitDepth::Eight {
        return Err("sheet: not 8-bit indexed".into());
    }
    let (sw, sh) = (info.width as usize, info.height as usize);
    if sw != columns.min(count) * width || sh != count.div_ceil(columns) * height {
        return Err(format!("sheet: {sw}x{sh} px for {count} frames"));
    }
    let stride = info.line_size;
    (0..count)
        .map(|n| {
            let (fx, fy) = ((n % columns) * width, (n / columns) * height);
            let mut f = Vec::with_capacity(width * height);
            for y in 0..height {
                let row = buf
                    .get((fy + y) * stride + fx..)
                    .and_then(|r| r.get(..width))
                    .ok_or("sheet: short")?;
                if row.iter().any(|&v| usize::from(v) > colours) {
                    return Err(format!("sheet: frame {n} past the palette"));
                }
                f.extend_from_slice(row);
            }
            Ok(f)
        })
        .collect()
}

/// `clip` decoded from its embedded files.
///
/// # Errors
///
/// An embedded file that does not decode (the tests decode every one).
pub fn decode(clip: Clip) -> Result<Anim, String> {
    let (text, png) = source(clip);
    Anim::parse(text, png)
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

/// The portrait's width in cells; it is half as many rows tall.
pub const SIZE: usize = 32;

/// A clip as glyph cells: every frame's, its pace, and whether it loops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loop {
    /// The frames, each rows of cells.
    pub frames: Vec<Vec<Vec<Cell>>>,
    /// Frames a second.
    pub fps: u32,
    /// Whether it loops; a clip played once holds its last frame.
    pub looping: bool,
}

/// The frame of a clip of `count` frames at `fps` on show `elapsed` in: played at its own pace,
/// whatever the UI's; a clip played once holds its last frame.
#[must_use]
pub fn frame_at(elapsed: Duration, fps: u32, count: usize, looping: bool) -> usize {
    let count = count.max(1);
    // A frame count: small, the product of seconds and a frame rate.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let i = (elapsed.as_secs_f64() * f64::from(fps)) as usize;
    if looping { i % count } else { i.min(count - 1) }
}

/// How long a clip of `count` frames at `fps` takes to play once.
#[must_use]
pub fn length(fps: u32, count: usize) -> Duration {
    Duration::from_secs(u64::try_from(count).unwrap_or(0)) / fps.max(1)
}

impl Loop {
    /// The frame on show `elapsed` in.
    #[must_use]
    pub fn at(&self, elapsed: Duration) -> &[Vec<Cell>] {
        &self.frames[frame_at(elapsed, self.fps, self.frames.len(), self.looping)]
    }
}

/// The glyphs portraits are drawn with: what the terminal's font draws, set once at start.
static GLYPHS: AtomicU8 = AtomicU8::new(0);

/// Draw portraits with `g` from now on.
pub fn set_glyphs(g: Glyphs) {
    let i = Glyphs::ALL.iter().position(|x| *x == g).unwrap_or(0);
    GLYPHS.store(u8::try_from(i).unwrap_or(0), Ordering::Relaxed);
}

/// The glyphs portraits are drawn with.
#[must_use]
pub fn glyphs_now() -> Glyphs {
    Glyphs::ALL[usize::from(GLYPHS.load(Ordering::Relaxed)) % Glyphs::ALL.len()]
}

/// The clips decoded so far, by clip and glyphs.
type Cache = Mutex<HashMap<(Clip, Glyphs), Option<Arc<Loop>>>>;

static DECODES: AtomicUsize = AtomicUsize::new(0);

/// How many times a clip was decoded into cells in this process: the cache's own instrument.
#[must_use]
pub fn decodes() -> usize {
    DECODES.load(Ordering::Relaxed)
}

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `c` as cells in the glyphs of now: decoded on its first use — a fraction of a second, so never
/// on the UI's thread: [`warm`] and [`fetch`] run it off it — then read from the cache. The lock
/// is never held while it decodes.
#[must_use]
pub fn clip(c: Clip) -> Option<Arc<Loop>> {
    let g = glyphs_now();
    if let Some(done) = cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&(c, g))
    {
        return done.clone();
    }
    DECODES.fetch_add(1, Ordering::Relaxed);
    let decoded = decode(c).ok().map(|anim| {
        Arc::new(Loop {
            frames: (0..anim.frames.len()).map(|f| anim.cells(f, g)).collect(),
            fps: anim.fps,
            looping: anim.looping,
        })
    });
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry((c, g))
        .or_insert(decoded)
        .clone()
}

/// `c` if it is decoded already, else `None`: what a frame draws. It never decodes, so the UI
/// never waits on one.
#[must_use]
pub fn clip_ready(c: Clip) -> Option<Arc<Loop>> {
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&(c, glyphs_now()))
        .cloned()
        .flatten()
}

/// Decode `c` on a thread of its own, once, unless it is decoded or decoding: for a clip the
/// warm-up does not cover (an ascension).
pub fn fetch(c: Clip) {
    static ASKED: OnceLock<Mutex<HashSet<(Clip, Glyphs)>>> = OnceLock::new();
    let fresh = ASKED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert((c, glyphs_now()));
    if fresh && clip_ready(c).is_none() {
        std::thread::spawn(move || {
            let _ = clip(c);
        });
    }
}

/// `race`'s idle loop as cells: [`clip`] of [`Clip::Idle`].
#[must_use]
pub fn portrait(race: Race) -> Option<Arc<Loop>> {
    clip(Clip::Idle(race))
}

/// `race`'s idle loop if it is decoded already: [`clip_ready`] of [`Clip::Idle`].
#[must_use]
pub fn ready(race: Race) -> Option<Arc<Loop>> {
    clip_ready(Clip::Idle(race))
}

/// Decode every race's idle loop into the cache, Rimuru first: for a blocking thread at start.
pub fn warm() {
    for race in Race::ALL {
        let _ = portrait(race);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_clip() -> Vec<Clip> {
        let mut all: Vec<Clip> = Race::ALL.iter().map(|r| Clip::Idle(*r)).collect();
        all.extend(Race::ALL.iter().filter_map(|r| Clip::ascension(*r)));
        all
    }

    #[test]
    fn every_embedded_clip_decodes_at_256_px() {
        let all = every_clip();
        assert_eq!(all.len(), 16, "eleven idles, five ascensions");
        for c in all {
            let a = decode(c).unwrap_or_else(|e| panic!("{c:?}: {e}"));
            assert_eq!((a.width, a.height, a.fps), (256, 256, 16), "{c:?}");
            assert_eq!(a.looping, c.looping(), "{c:?}");
            assert_eq!(a.frames.len(), c.frames(), "{c:?}");
            let (text, _) = source(c);
            assert!(
                text.contains(&format!("\"name\":\"{}\"", c.name())),
                "{c:?}"
            );
        }
        let ids: HashSet<u32> = every_clip().into_iter().map(Clip::index).collect();
        assert_eq!(ids.len(), 16, "each clip its own ids");
    }

    #[test]
    fn an_ascension_ends_in_the_risen_form() {
        assert_eq!(Clip::ascension(Race::Orc), Some(Clip::Ascend(Race::Orc)));
        assert_eq!(Clip::Ascend(Race::Orc).then(), Clip::Idle(Race::HighOrc));
        assert_eq!(Clip::ascension(Race::Rimuru), None);
        assert_eq!(Clip::ascension(Race::HighElf), None, "no second ascension");
        let (text, _) = source(Clip::Ascend(Race::Slime));
        assert!(text.contains("\"race\":\"named-slime\""));
    }

    #[test]
    fn a_broken_file_is_refused_never_a_panic() {
        let (text, png) = source(Clip::Idle(Race::Elf));
        assert!(
            Anim::parse(text, &png[..png.len() / 2]).is_err(),
            "a cut sheet"
        );
        assert!(Anim::parse(text, b"not a png").is_err());
        assert!(Anim::parse(r#"{"format":"gif"}"#, png).is_err());
        let wrong = text.replacen("\"columns\":", "\"columns\":1", 1);
        assert!(
            Anim::parse(&wrong, png).is_err(),
            "the sheet's size disagrees"
        );
    }

    #[test]
    fn a_native_frame_is_its_own_pixels_centred_in_the_box() {
        let a = decode(Clip::Idle(Race::Slime)).unwrap();
        let bg = Rgb(1, 2, 3);
        // A box larger than the frame: unscaled, centred, the margin the background.
        let big = a.boxed(0, 300, 280, bg);
        assert_eq!(big.dimensions(), (300, 280));
        assert_eq!(big.get_pixel(0, 0).0, [1, 2, 3]);
        let rgba = a.rgba(0);
        let opaque = (0..256u32)
            .flat_map(|y| (0..256u32).map(move |x| (x, y)))
            .find(|&(x, y)| rgba.get_pixel(x, y).0[3] == 255)
            .expect("an opaque pixel");
        let p = rgba.get_pixel(opaque.0, opaque.1).0;
        assert_eq!(
            big.get_pixel(opaque.0 + 22, opaque.1 + 12).0,
            [p[0], p[1], p[2]],
            "one frame pixel a screen pixel"
        );
        // A smaller box: shrunk to it, exactly its pixels.
        assert_eq!(a.boxed(0, 224, 238, bg).dimensions(), (224, 238));
    }

    #[test]
    fn every_glyph_set_fills_the_portrait_box() {
        let a = decode(Clip::Idle(Race::Kijin)).unwrap();
        for g in Glyphs::ALL {
            let cells = a.cells(3, g);
            assert_eq!(cells.len(), SIZE / 2, "{g:?}");
            assert!(cells.iter().all(|r| r.len() == SIZE), "{g:?}");
        }
    }

    #[test]
    fn every_race_has_a_portrait_decoded_once() {
        for race in Race::ALL {
            let p = portrait(race).unwrap_or_else(|| panic!("{race:?}"));
            assert_eq!(p.frames.len(), 48);
            for f in &p.frames {
                assert_eq!(f.len(), SIZE / 2, "{race:?}");
            }
        }
        let after = decodes();
        for race in Race::ALL {
            let _ = portrait(race);
        }
        assert_eq!(decodes(), after, "read from the cache, not decoded again");
    }

    #[test]
    fn a_clip_plays_at_its_own_pace_and_an_ascension_holds_its_end() {
        let step = Duration::from_secs(1) / 16;
        assert_eq!(frame_at(Duration::ZERO, 16, 48, true), 0);
        assert_eq!(frame_at(step, 16, 48, true), 1);
        assert_eq!(frame_at(step * 48, 16, 48, true), 0, "seamless");
        assert_eq!(frame_at(step * 200, 16, 80, false), 79, "held");
        assert_eq!(length(16, 80), Duration::from_secs(5));
    }
}
