//! The portraits as real pixels, placed over the cells: what a frame wants shown
//! ([`Pictures`], on the model) and what the terminal shows ([`Placer`], in the runtime).
//!
//! A terminal image is drawn over cells, and text drawn later does not cover it. So the view
//! draws plain cells of the picture's background where a picture goes, and says so ([`Want`]);
//! after the frame is written, the placer compares what is wanted with what is shown:
//!
//! - **unchanged** — nothing is sent: the same race, box, background and terminal size is never
//!   sent twice, however many frames are drawn;
//! - **gone or moved** — the old one is erased first: Kitty deletes it by id; under Sixel and
//!   iTerm2 the cells under its box are written again from the frame just drawn (their text
//!   covers the image);
//! - **new** — the picture, built once per race, box and background and kept encoded, is sent at
//!   the box's top-left with the cursor saved and restored.
//!
//! The view wants nothing while an approval is open — the card never sits under a picture —
//! nor where a toast or a note would cover one. A resize redraws the whole screen, so each
//! picture is sent again.
//!
//! In image mode a portrait is still: the working body's race, swapped when it changes. A Kitty
//! animation would upload each frame once as its own image id and switch placements by id
//! (`a=p`); [`Art`] is where such a picture would be named.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write;
use std::sync::Arc;

use mnemorph_races::Race;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use unicode_width::UnicodeWidthStr;

use crate::kit::pixels::{self, Protocol};
use crate::theme::Rgb;

/// Where a picture goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The sidebar's head: the working body.
    Sidebar,
    /// The welcome card: Rimuru.
    Welcome,
}

impl Slot {
    /// Its Kitty image id: one per slot, so a slot's picture is deleted by id.
    #[must_use]
    pub fn id(self) -> u32 {
        match self {
            Self::Sidebar => 0x4d_4e01,
            Self::Welcome => 0x4d_4e02,
        }
    }
}

/// What a picture shows: a race's still portrait. (A Kitty animation would add its frame here.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Art {
    /// A race's portrait, still.
    Still(Race),
}

/// A picture a frame wants: its slot, its art, its box in cells, and the background it sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Want {
    /// Where.
    pub slot: Slot,
    /// What.
    pub art: Art,
    /// Its box, in cells.
    pub area: Rect,
    /// The background the picture is laid on: the cells' own.
    pub bg: Rgb,
}

/// The model's side: the protocol, the cell's size, and what the last frame wanted.
#[derive(Debug, Default)]
pub struct Pictures {
    /// How portraits are drawn.
    pub protocol: Option<Protocol>,
    /// A cell's size in pixels.
    pub cell: Option<(u16, u16)>,
    /// What the last frame wanted shown.
    pub(crate) wants: RefCell<Vec<Want>>,
}

impl Pictures {
    /// Pictures drawn with `protocol`, cells of `cell` pixels; half blocks without a cell size.
    #[must_use]
    pub fn new(protocol: Protocol, cell: Option<(u16, u16)>) -> Self {
        Self {
            protocol: cell.and(protocol.pixels().then_some(protocol)),
            cell,
            wants: RefCell::new(Vec::new()),
        }
    }

    /// Whether portraits are real pixels.
    #[must_use]
    pub fn active(&self) -> bool {
        self.protocol.is_some()
    }

    /// The rows a portrait `cols` wide takes, when it is a picture.
    #[must_use]
    pub fn rows(&self, cols: u16) -> Option<u16> {
        self.protocol?;
        Some(pixels::square(cols, self.cell?).0)
    }

    /// What the last frame wanted shown.
    #[must_use]
    pub fn wanted(&self) -> Vec<Want> {
        self.wants.borrow().clone()
    }

    /// A frame wants `want` shown.
    pub(crate) fn want(&self, want: Want) {
        if self.active() {
            self.wants.borrow_mut().push(want);
        }
    }
}

/// A picture on screen: what it shows, and the terminal's size when it was sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
    want: Want,
    size: (u16, u16),
}

/// The cache's key: a picture is built once per slot (its Kitty id), art, box size and
/// background.
type Key = (Slot, Art, u16, u16, Rgb);

/// The most encoded pictures kept: a few boxes, a few races.
const KEPT: usize = 24;

/// The runtime's side: what the terminal shows, and the pictures encoded so far.
#[derive(Debug)]
pub struct Placer {
    protocol: Option<Protocol>,
    cell: (u16, u16),
    shown: Vec<Shown>,
    cache: HashMap<Key, Arc<str>>,
    /// Pictures sent to the terminal.
    pub sends: usize,
    /// Pictures built and encoded.
    pub builds: usize,
}

impl Placer {
    /// A placer for `pictures`' protocol.
    #[must_use]
    pub fn new(pictures: &Pictures) -> Self {
        Self {
            protocol: pictures.protocol,
            cell: pictures.cell.unwrap_or((8, 16)),
            shown: Vec::new(),
            cache: HashMap::new(),
            sends: 0,
            builds: 0,
        }
    }

    /// Whether a picture is on screen.
    #[must_use]
    pub fn showing(&self) -> bool {
        !self.shown.is_empty()
    }

    /// Before a frame: under Kitty, whose pictures float over the text, every picture is deleted
    /// before the frame is written when it wants none — an approval opening never shows under
    /// one, not even for a frame.
    ///
    /// # Errors
    ///
    /// `out` cannot be written.
    pub fn before(&mut self, wants_none: bool, out: &mut impl Write) -> std::io::Result<()> {
        if wants_none && self.protocol == Some(Protocol::Kitty) && self.showing() {
            self.clear(out)?;
        }
        Ok(())
    }

    /// Delete every Kitty picture shown: for leaving, and before a frame that wants none. Under
    /// Sixel and iTerm2 a picture lives in the cells, and the next frame's text covers it.
    ///
    /// # Errors
    ///
    /// `out` cannot be written.
    pub fn clear(&mut self, out: &mut impl Write) -> std::io::Result<()> {
        if self.protocol == Some(Protocol::Kitty) {
            for s in &self.shown {
                out.write_all(pixels::kitty_delete(s.want.slot.id()).as_bytes())?;
            }
            out.flush()?;
        }
        self.shown.clear();
        Ok(())
    }

    /// After a frame drawn as `buf` at terminal `size`: erase what is no longer wanted, send what
    /// is newly wanted; nothing when nothing changed.
    ///
    /// # Errors
    ///
    /// `out` cannot be written.
    pub fn sync(
        &mut self,
        wants: &[Want],
        size: (u16, u16),
        buf: &Buffer,
        out: &mut impl Write,
    ) -> std::io::Result<()> {
        let Some(protocol) = self.protocol else {
            return Ok(());
        };
        let keep = |s: &Shown| s.size == size && wants.contains(&s.want);
        let (kept, gone): (Vec<Shown>, Vec<Shown>) = self.shown.drain(..).partition(keep);
        let mut wrote = false;
        for g in &gone {
            match protocol {
                Protocol::Kitty => {
                    out.write_all(pixels::kitty_delete(g.want.slot.id()).as_bytes())?;
                }
                _ => repaint(buf, g.want.area, out)?,
            }
            wrote = true;
        }
        self.shown = kept;
        for w in wants {
            if self.shown.iter().any(|s| s.want == *w) {
                continue;
            }
            // A box off the frame is never sent: a picture past the edge would scroll it.
            if w.area.right() > buf.area.right() || w.area.bottom() > buf.area.bottom() {
                continue;
            }
            let Some(payload) = self.payload(protocol, w) else {
                continue;
            };
            write!(out, "\x1b7\x1b[{};{}H", w.area.y + 1, w.area.x + 1)?;
            out.write_all(payload.as_bytes())?;
            out.write_all(b"\x1b8")?;
            self.sends += 1;
            wrote = true;
            self.shown.push(Shown { want: *w, size });
        }
        if wrote {
            out.flush()?;
        }
        Ok(())
    }

    /// The encoded picture for `w`: from the cache, else built once.
    fn payload(&mut self, protocol: Protocol, w: &Want) -> Option<Arc<str>> {
        let key = (w.slot, w.art, w.area.width, w.area.height, w.bg);
        if let Some(p) = self.cache.get(&key) {
            return Some(Arc::clone(p));
        }
        let (cw, ch) = (u32::from(self.cell.0), u32::from(self.cell.1));
        let (px_w, px_h) = (u32::from(w.area.width) * cw, u32::from(w.area.height) * ch);
        let Art::Still(race) = w.art;
        let img = pixels::picture(pixels::png(race), px_w, px_h, w.bg)?;
        let encoded: String = match protocol {
            Protocol::Kitty => pixels::kitty(
                &pixels::png_bytes(&img),
                w.area.width,
                w.area.height,
                w.slot.id(),
            ),
            Protocol::Iterm => pixels::iterm(&pixels::png_bytes(&img), px_w, px_h),
            Protocol::Sixel => pixels::sixel(&img),
            Protocol::Blocks => return None,
        };
        let encoded: Arc<str> = Arc::from(encoded);
        if self.cache.len() >= KEPT {
            self.cache.clear();
        }
        self.cache.insert(key, Arc::clone(&encoded));
        self.builds += 1;
        Some(encoded)
    }
}

/// Write `area`'s cells again from `buf`, over whatever picture lies there: each row erased
/// (`ECH`), then its cells as the frame has them. The cursor is saved and restored, and the
/// colours reset after, as the backend leaves them.
fn repaint(buf: &Buffer, area: Rect, out: &mut impl Write) -> std::io::Result<()> {
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return Ok(());
    }
    let mut s = String::from("\x1b7");
    for y in area.top()..area.bottom() {
        let _ = write!(
            s,
            "\x1b[{};{}H\x1b[0m\x1b[{}X",
            y + 1,
            area.x + 1,
            area.width
        );
        let mut skip = 0usize;
        for x in area.left()..area.right() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let cell = &buf[(x, y)];
            let sym = cell.symbol();
            skip = sym.width().saturating_sub(1);
            s.push_str("\x1b[0");
            sgr(&mut s, cell.fg, 38);
            sgr(&mut s, cell.bg, 48);
            for (m, code) in [
                (Modifier::BOLD, "1"),
                (Modifier::DIM, "2"),
                (Modifier::ITALIC, "3"),
                (Modifier::UNDERLINED, "4"),
                (Modifier::REVERSED, "7"),
            ] {
                if cell.modifier.contains(m) {
                    s.push(';');
                    s.push_str(code);
                }
            }
            s.push('m');
            // A cell holds text the frame drew: control characters never reach it.
            if sym.is_empty() || sym.chars().any(char::is_control) {
                s.push(' ');
            } else {
                s.push_str(sym);
            }
        }
    }
    s.push_str("\x1b[0m\x1b8");
    out.write_all(s.as_bytes())
}

/// A colour's SGR parameters, `base` 38 for the text and 48 for the background.
fn sgr(out: &mut String, colour: Color, base: u8) {
    let _ = match colour {
        Color::Rgb(r, g, b) => write!(out, ";{base};2;{r};{g};{b}"),
        Color::Indexed(i) => write!(out, ";{base};5;{i}"),
        _ => Ok(()),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placer(p: Protocol) -> Placer {
        Placer::new(&Pictures::new(p, Some((8, 17))))
    }

    fn want(slot: Slot, race: Race, x: u16, y: u16) -> Want {
        Want {
            slot,
            art: Art::Still(race),
            area: Rect::new(x, y, 4, 2),
            bg: Rgb(6, 11, 18),
        }
    }

    #[test]
    fn an_unchanged_frame_sends_nothing() {
        for p in [Protocol::Kitty, Protocol::Iterm, Protocol::Sixel] {
            let mut pl = placer(p);
            let buf = Buffer::empty(Rect::new(0, 0, 40, 20));
            let wants = [want(Slot::Sidebar, Race::Rimuru, 30, 1)];
            let mut out = Vec::new();
            pl.sync(&wants, (40, 20), &buf, &mut out).unwrap();
            assert!(!out.is_empty(), "{p:?}: sent once");
            assert_eq!((pl.sends, pl.builds), (1, 1));
            for _ in 0..5 {
                let mut again = Vec::new();
                pl.sync(&wants, (40, 20), &buf, &mut again).unwrap();
                assert!(
                    again.is_empty(),
                    "{p:?}: an unchanged frame wrote something"
                );
            }
            assert_eq!(pl.sends, 1);
        }
    }

    #[test]
    fn a_placement_saves_and_restores_the_cursor() {
        let mut pl = placer(Protocol::Iterm);
        let buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        let mut out = Vec::new();
        pl.sync(
            &[want(Slot::Welcome, Race::Elf, 3, 5)],
            (40, 20),
            &buf,
            &mut out,
        )
        .unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("\x1b7\x1b[6;4H\x1b]1337;File=inline=1;"));
        assert!(s.ends_with("\x07\x1b8"));
    }

    #[test]
    fn a_moved_or_changed_picture_is_erased_then_sent_again() {
        let buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        let mut k = placer(Protocol::Kitty);
        k.sync(
            &[want(Slot::Sidebar, Race::Rimuru, 30, 1)],
            (40, 20),
            &buf,
            &mut Vec::new(),
        )
        .unwrap();
        // The working body changes: the old picture deleted by id, the new one sent.
        let mut out = Vec::new();
        k.sync(
            &[want(Slot::Sidebar, Race::Orc, 30, 1)],
            (40, 20),
            &buf,
            &mut out,
        )
        .unwrap();
        let s = String::from_utf8(out).unwrap();
        let del = pixels::kitty_delete(Slot::Sidebar.id());
        assert!(s.starts_with(&del), "deleted first");
        assert!(s.contains("a=T,f=100"));
        // The sidebar folds: deleted, nothing sent.
        let mut out = Vec::new();
        k.sync(&[], (40, 20), &buf, &mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), del);
        assert!(!k.showing());

        // Under Sixel, the cells under the old box are written again from the frame.
        let mut sx = placer(Protocol::Sixel);
        let mut frame = Buffer::empty(Rect::new(0, 0, 40, 20));
        frame.set_string(30, 1, "abcd", ratatui::style::Style::new());
        sx.sync(
            &[want(Slot::Welcome, Race::Slime, 30, 1)],
            (40, 20),
            &frame,
            &mut Vec::new(),
        )
        .unwrap();
        let mut out = Vec::new();
        sx.sync(
            &[want(Slot::Welcome, Race::Slime, 30, 3)],
            (40, 20),
            &frame,
            &mut out,
        )
        .unwrap();
        let s = String::from_utf8(out).unwrap();
        let erase = s
            .find("\x1b[2;31H\x1b[0m\x1b[4X")
            .expect("the old box's rows erased");
        let place = s.find("\x1bP0;1;0q").expect("sent again");
        assert!(erase < place, "erased before the new one is sent");
        assert!(s.contains("ma\x1b[0mb"), "its text written again: {s:.200}");
        assert_eq!(sx.builds, 1, "the same box size is built once");
    }

    #[test]
    fn a_resize_sends_every_picture_again() {
        let mut pl = placer(Protocol::Iterm);
        let buf = Buffer::empty(Rect::new(0, 0, 60, 30));
        let wants = [want(Slot::Sidebar, Race::Rimuru, 30, 1)];
        pl.sync(&wants, (60, 30), &buf, &mut Vec::new()).unwrap();
        pl.sync(&wants, (60, 31), &buf, &mut Vec::new()).unwrap();
        assert_eq!(pl.sends, 2);
        assert_eq!(pl.builds, 1);
    }

    #[test]
    fn half_blocks_send_nothing() {
        let pics = Pictures::new(Protocol::Blocks, Some((8, 17)));
        assert!(!pics.active());
        assert_eq!(pics.rows(32), None);
        pics.want(want(Slot::Sidebar, Race::Rimuru, 0, 0));
        assert_eq!(pics.wanted(), Vec::new());
        // An image protocol without a cell size is half blocks too.
        assert!(!Pictures::new(Protocol::Iterm, None).active());
        let mut pl = Placer::new(&pics);
        let mut out = Vec::new();
        pl.sync(
            &[want(Slot::Sidebar, Race::Rimuru, 0, 0)],
            (40, 20),
            &Buffer::empty(Rect::new(0, 0, 40, 20)),
            &mut out,
        )
        .unwrap();
        assert_eq!(out, b"");
    }

    #[test]
    fn a_box_past_the_frame_is_never_sent() {
        let mut pl = placer(Protocol::Iterm);
        let buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        let mut out = Vec::new();
        pl.sync(
            &[want(Slot::Welcome, Race::Rimuru, 18, 9)],
            (20, 10),
            &buf,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, b"");
    }

    #[test]
    fn kitty_pictures_go_before_a_frame_that_wants_none() {
        let mut pl = placer(Protocol::Kitty);
        let buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        pl.sync(
            &[want(Slot::Sidebar, Race::Rimuru, 30, 1)],
            (40, 20),
            &buf,
            &mut Vec::new(),
        )
        .unwrap();
        let mut out = Vec::new();
        pl.before(false, &mut out).unwrap();
        assert_eq!(out, b"");
        pl.before(true, &mut out).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            pixels::kitty_delete(Slot::Sidebar.id())
        );
        assert!(!pl.showing());
    }
}
