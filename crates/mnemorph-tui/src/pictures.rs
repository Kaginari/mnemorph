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
//! - **new** — the picture, built once per art, box and background and kept encoded, is sent at
//!   the box's top-left with the cursor saved and restored.
//!
//! The view wants nothing while an approval is open — the card never sits under a picture —
//! nor where a toast or a note would cover one. A resize redraws the whole screen, so each
//! picture is sent again.
//!
//! While a body works its idle loop plays as real pixels ([`Art::Frame`]): each frame is the
//! 256 px drawing laid on the box one frame pixel a screen pixel, as the animations' player shows
//! it with `--native` (shrunk only when the box is smaller). Frames are encoded off the UI's
//! thread, once per clip, box and background ([`Placer`]'s reels), and a frame not encoded yet is
//! simply waited for: the old one stays up. A new frame over the same box replaces the old one
//! without erasing it first, so nothing flickers:
//!
//! - **Kitty** — each frame is transmitted once under its own image id and placed by id after: a
//!   frame change is a few bytes, at the clip's own 16 frames a second;
//! - **iTerm2, Sixel** — the frame's pixels are sent again over the old ones, so the clip plays at
//!   8 frames a second ([`fps`]), a frame sent only when it changes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use unicode_width::UnicodeWidthStr;

use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;

use crate::kit::mnanim::{self, Clip};
use crate::kit::pixels::{self, Protocol};
use crate::theme::Rgb;

/// Where a picture goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The sidebar's head: the working body. The only slot: the welcome card draws its
    /// wordmark in text, and scrolls with nothing to withdraw.
    Sidebar,
}

impl Slot {
    /// Its Kitty image id: one per slot, so a slot's picture is deleted by id.
    #[must_use]
    pub fn id(self) -> u32 {
        match self {
            Self::Sidebar => 0x4d_4e01,
        }
    }
}

/// What a picture shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Art {
    /// Veldora's portrait: the human, shown while no body works.
    Veldora,
    /// Frame `n` of a clip: a working body's idle loop, or an ascension.
    Frame(Clip, u16),
}

/// The frames a second a clip is sent at as real pixels: all of its 16 under Kitty, where a frame
/// change is a placement; 8 under iTerm2 and Sixel, where it is the frame's pixels again.
#[must_use]
pub fn fps(protocol: Protocol) -> u32 {
    if protocol == Protocol::Kitty { 16 } else { 8 }
}

/// The first Kitty image id of the clips' frames: frame `n` of clip `c` is
/// `FRAME_IDS + c.index() * 128 + n`.
pub const FRAME_IDS: u32 = 0x4d_5000;

/// A picture's Kitty image id: its slot's for a still, its own for a clip's frame.
#[must_use]
pub fn kitty_id(slot: Slot, art: Art) -> u32 {
    match art {
        Art::Frame(clip, n) => FRAME_IDS + clip.index() * 128 + u32::from(n),
        Art::Veldora => slot.id(),
    }
}

/// The animations' native size, in pixels: a frame shows one pixel a screen pixel in a box this
/// large.
pub const NATIVE: u32 = 256;

/// The last Kitty image id a clip's frame can take.
const LAST_FRAME_ID: u32 = FRAME_IDS + 22 * 128;

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
    /// A protocol set aside until the cell's size can be trusted again.
    lost: Option<Protocol>,
}

impl Pictures {
    /// Pictures drawn with `protocol`, cells of `cell` pixels; half blocks without a cell size.
    #[must_use]
    pub fn new(protocol: Protocol, cell: Option<(u16, u16)>) -> Self {
        Self {
            protocol: cell.and(protocol.pixels().then_some(protocol)),
            cell,
            wants: RefCell::new(Vec::new()),
            lost: None,
        }
    }

    /// Whether portraits are real pixels.
    #[must_use]
    pub fn active(&self) -> bool {
        self.protocol.is_some()
    }

    /// Whether the terminal draws pictures at all, now or once the cell's size returns.
    #[must_use]
    pub fn images(&self) -> bool {
        self.protocol.is_some() || self.lost.is_some()
    }

    /// The frames a second a clip is drawn at: as [`fps`] sends it in real pixels, else 16.
    #[must_use]
    pub fn fps(&self) -> u32 {
        self.protocol.map_or(16, fps)
    }

    /// The rows a portrait `cols` wide takes, when it is a picture: a square of cells, and
    /// enough rows for a 256 px frame to show at its own size when the columns hold it.
    #[must_use]
    pub fn rows(&self, cols: u16) -> Option<u16> {
        self.protocol?;
        let cell = self.cell?;
        let square = pixels::square(cols, cell).0;
        let native = u32::from(cols) * u32::from(cell.0) >= NATIVE;
        let tall = u16::try_from(NATIVE.div_ceil(u32::from(cell.1.max(1)))).unwrap_or(u16::MAX);
        Some(if native { square.max(tall) } else { square })
    }

    /// The frame on show of `clip`, `elapsed` into it, as the protocol sends it: every frame at
    /// 16 a second, or every other one at 8.
    #[must_use]
    pub fn frame(&self, clip: Clip, elapsed: Duration) -> u16 {
        let step = (16 / self.fps().max(1)).max(1) as usize;
        let count = clip.frames();
        let raw = mnanim::frame_at(elapsed, 16, count, clip.looping());
        let n = if raw + 1 == count {
            raw
        } else {
            raw - raw % step
        };
        u16::try_from(n).unwrap_or(0)
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

/// The slots whose Kitty picture is on screen, one bit each: what a panic, which never reaches
/// the placer, deletes on its way out ([`release`]).
static KITTY_SHOWN: AtomicU8 = AtomicU8::new(0);

/// The bit [`KITTY_SHOWN`] keeps for frames stored in the terminal.
const FRAMES_STORED: u8 = 0x80;

impl Slot {
    fn bit(self) -> u8 {
        match self {
            Self::Sidebar => 1,
        }
    }
}

/// Delete every Kitty picture still on screen, into `out`: for the panic hook and the screen's
/// drop, before the terminal is given back. Safe to run twice, and after the placer's own
/// [`Placer::clear`]: what is deleted once is never deleted again.
///
/// # Errors
///
/// `out` cannot be written.
pub fn release(out: &mut impl Write) -> std::io::Result<()> {
    let shown = KITTY_SHOWN.swap(0, Ordering::SeqCst);
    let slot = Slot::Sidebar;
    if shown & slot.bit() != 0 {
        out.write_all(pixels::kitty_delete(slot.id()).as_bytes())?;
    }
    if shown & FRAMES_STORED != 0 {
        out.write_all(pixels::kitty_free_range(FRAME_IDS, LAST_FRAME_ID).as_bytes())?;
    }
    out.flush()
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

/// A reel's key: a clip encoded for one protocol, box (cells and pixels) and background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ReelKey {
    clip: Clip,
    protocol: Protocol,
    cells: (u16, u16),
    px: (u32, u32),
    bg: Rgb,
}

/// A clip's frames, encoded on a thread of their own and filled in as each is done.
#[derive(Debug, Default)]
struct Reel {
    frames: Mutex<Vec<Option<Arc<str>>>>,
}

impl Reel {
    fn get(&self, n: usize) -> Option<Arc<str>> {
        self.frames
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(n)
            .cloned()
            .flatten()
    }
}

/// The most reels kept: the working body's clip and a few before it.
const REELS: usize = 4;

/// Encode `key`'s clip into `reel`, frame `first` first, counting each frame in `encoded`.
fn encode(key: ReelKey, first: usize, reel: &Reel, encoded: &AtomicUsize) {
    let Ok(anim) = mnanim::decode(key.clip) else {
        return;
    };
    let count = anim.frames.len();
    reel.frames
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .resize(count, None);
    for n in (first..count).chain(0..first.min(count)) {
        let img = anim.boxed(n, key.px.0, key.px.1, key.bg);
        let bytes: String = match key.protocol {
            Protocol::Kitty => pixels::kitty_transmit(
                &pixels::png_bytes(&img),
                kitty_id(
                    Slot::Sidebar,
                    Art::Frame(key.clip, u16::try_from(n).unwrap_or(0)),
                ),
            ),
            Protocol::Iterm => pixels::iterm(&pixels::png_bytes(&img), key.cells.0, key.cells.1),
            Protocol::Sixel => pixels::sixel(&img),
            Protocol::Blocks => return,
        };
        encoded.fetch_add(1, Ordering::Relaxed);
        if let Some(slot) = reel
            .frames
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(n)
        {
            *slot = Some(Arc::from(bytes));
        }
    }
}

/// Kitty: placements of a stored frame before it is sent again, in case the terminal evicted it.
const REFRESH: u32 = 32;

/// How soon a frame still encoding is looked for again.
const ENCODING: Duration = Duration::from_millis(16);

/// How long a moved box stays put before its picture is sent.
pub const SETTLE: Duration = Duration::from_millis(150);

/// After a resize, `cell` the cell's size read again (`None` when the window gives no pixels).
/// A new size rebuilds the pictures. Sixel draws exactly the pixels it was built for, so with
/// no size to trust it falls back to half blocks — the old picture erased — until one returns;
/// Kitty and iTerm2 are given their box in cells and fit it themselves.
pub fn resized(pictures: &mut Pictures, placer: &mut Placer, cell: Option<(u16, u16)>) {
    match cell {
        Some(c) => {
            if let Some(p) = pictures.lost.take() {
                pictures.protocol = Some(p);
                placer.set_protocol(Some(p));
            }
            pictures.cell = Some(c);
            placer.set_cell(c);
        }
        None if pictures.protocol == Some(Protocol::Sixel) => {
            pictures.lost = pictures.protocol.take();
            placer.set_protocol(None);
        }
        None => {}
    }
}

/// The most encoded pictures kept: a few boxes, a few races.
const KEPT: usize = 24;

/// The runtime's side: what the terminal shows, and the pictures encoded so far.
#[derive(Debug)]
pub struct Placer {
    protocol: Option<Protocol>,
    /// The protocol the pictures on screen were sent with: what erases them, even once the
    /// pictures fell back to half blocks.
    erase_with: Option<Protocol>,
    cell: (u16, u16),
    shown: Vec<Shown>,
    /// Each slot's box as last wanted, and since when it has been there.
    seen: Vec<(Slot, Rect, Instant)>,
    /// When a picture held back while its box moved has settled.
    settle_at: Option<Instant>,
    cache: HashMap<Key, Arc<str>>,
    /// The clips' frames, encoded off the UI's thread.
    reels: HashMap<ReelKey, Arc<Reel>>,
    /// Kitty: the frames stored in the terminal, by image id: the reel each came from, and its
    /// placements since it was sent.
    stored: HashMap<u32, (ReelKey, u32)>,
    /// Kitty: frames of reels let go, to free from the terminal once none of them is on screen.
    freeing: Vec<u32>,
    /// Frames encoded, by every reel: the frame cache's own instrument.
    encoded: Arc<AtomicUsize>,
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
            erase_with: pictures.protocol,
            cell: pictures.cell.unwrap_or((8, 16)),
            shown: Vec::new(),
            seen: Vec::new(),
            settle_at: None,
            cache: HashMap::new(),
            reels: HashMap::new(),
            stored: HashMap::new(),
            freeing: Vec::new(),
            encoded: Arc::new(AtomicUsize::new(0)),
            sends: 0,
            builds: 0,
        }
    }

    /// A cell is now `cell` pixels (a font zoom): every picture is built again at the new size.
    pub fn set_cell(&mut self, cell: (u16, u16)) {
        if cell != self.cell {
            self.cell = cell;
            self.cache.clear();
            self.let_reels_go();
        }
    }

    /// Drop every reel: under Kitty, the frames they stored are no longer trusted — sent again
    /// when next wanted — and freed from the terminal at the next sync, unless one is on screen
    /// then or was sent again since. Over a long session the terminal never holds more than the
    /// last few reels' frames.
    fn let_reels_go(&mut self) {
        self.reels.clear();
        self.freeing.extend(self.stored.drain().map(|(id, _)| id));
    }

    /// Free the frames let go that nothing shows and nothing stored again.
    fn free_let_go(&mut self, out: &mut impl Write) -> std::io::Result<bool> {
        let shown: Vec<u32> = self
            .shown
            .iter()
            .map(|s| kitty_id(s.want.slot, s.want.art))
            .collect();
        let stored = &self.stored;
        self.freeing.retain(|id| !stored.contains_key(id));
        self.freeing.sort_unstable();
        self.freeing.dedup();
        let (wait, free): (Vec<u32>, Vec<u32>) =
            self.freeing.drain(..).partition(|id| shown.contains(id));
        self.freeing = wait;
        for id in &free {
            out.write_all(pixels::kitty_delete(*id).as_bytes())?;
        }
        Ok(!free.is_empty())
    }

    /// Draw with `protocol` from now on; `None` falls back to half blocks, and the next sync
    /// erases what is on screen.
    pub fn set_protocol(&mut self, protocol: Option<Protocol>) {
        self.protocol = protocol;
        if protocol.is_some() {
            self.erase_with = protocol;
        }
    }

    /// How long until a picture held back while its box moved may be placed: zero when a frame
    /// is owed now, `None` when none waits.
    #[must_use]
    pub fn due(&self, now: Instant) -> Option<Duration> {
        self.settle_at.map(|t| t.saturating_duration_since(now))
    }

    /// How many clip frames were encoded, by every reel so far.
    #[must_use]
    pub fn encoded(&self) -> usize {
        self.encoded.load(Ordering::Relaxed)
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
        if wants_none && self.erase_with == Some(Protocol::Kitty) && self.showing() {
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
        if self.erase_with == Some(Protocol::Kitty) {
            for s in &self.shown {
                KITTY_SHOWN.fetch_and(!s.want.slot.bit(), Ordering::SeqCst);
                out.write_all(take_down(s.want).as_bytes())?;
            }
            // The frames stored are freed by id, so nothing of them outlives the session.
            let mut ids: Vec<u32> = self.stored.drain().map(|(id, _)| id).collect();
            ids.append(&mut self.freeing);
            ids.sort_unstable();
            ids.dedup();
            for id in ids {
                out.write_all(pixels::kitty_delete(id).as_bytes())?;
            }
            KITTY_SHOWN.fetch_and(!FRAMES_STORED, Ordering::SeqCst);
            out.flush()?;
        }
        self.shown.clear();
        Ok(())
    }

    /// After a frame drawn as `buf` at terminal `size`, now: see [`Placer::sync_at`].
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
        self.sync_at(wants, size, buf, out, Instant::now())
    }

    /// After a frame drawn as `buf` at terminal `size`, at `now`: erase what is no longer wanted,
    /// send what is newly wanted; nothing when nothing changed. A picture whose box moved is not
    /// sent until its box has stayed put for [`SETTLE`]: a moving box shows its plain cells, and
    /// is never sent once a frame.
    ///
    /// # Errors
    ///
    /// `out` cannot be written.
    pub fn sync_at(
        &mut self,
        wants: &[Want],
        size: (u16, u16),
        buf: &Buffer,
        out: &mut impl Write,
        now: Instant,
    ) -> std::io::Result<()> {
        let live = self.protocol.is_some();
        let keep = |s: &Shown| live && s.size == size && wants.contains(&s.want);
        let (kept, gone): (Vec<Shown>, Vec<Shown>) = self.shown.drain(..).partition(keep);
        self.shown = kept;
        self.settle_at = None;
        // What each newly wanted picture sends, or whether it waits for its frame to be encoded.
        let mut sends: Vec<(Want, Vec<Arc<str>>)> = Vec::new();
        let mut waiting: Vec<Want> = Vec::new();
        if let Some(protocol) = self.protocol {
            for w in wants {
                if self.shown.iter().any(|s| s.want == *w) {
                    continue;
                }
                // A box off the frame is never sent: a picture past the edge would scroll it.
                if w.area.right() > buf.area.right() || w.area.bottom() > buf.area.bottom() {
                    continue;
                }
                let since = self.since(w, now);
                if now < since + SETTLE {
                    let at = since + SETTLE;
                    self.settle_at = Some(self.settle_at.map_or(at, |t| t.min(at)));
                    continue;
                }
                match self.payload(protocol, w) {
                    Some(parts) => sends.push((*w, parts)),
                    None if matches!(w.art, Art::Frame(..)) => {
                        waiting.push(*w);
                        let at = now + ENCODING;
                        self.settle_at = Some(self.settle_at.map_or(at, |t| t.min(at)));
                    }
                    None => {}
                }
            }
        }
        let over = |w: &Want, g: &Shown| {
            live && g.size == size && w.slot == g.want.slot && w.area == g.want.area
        };
        let mut wrote = false;
        let mut covered: Vec<Shown> = Vec::new();
        for g in gone {
            if waiting.iter().any(|w| over(w, &g)) {
                // Its successor is still encoding: the old picture stays up until it is ready.
                self.shown.push(g);
            } else if sends.iter().any(|(w, _)| over(w, &g)) {
                // The new picture covers it whole: taken down after it is placed, never before.
                covered.push(g);
            } else {
                match self.erase_with {
                    Some(Protocol::Kitty) => {
                        KITTY_SHOWN.fetch_and(!g.want.slot.bit(), Ordering::SeqCst);
                        out.write_all(take_down(g.want).as_bytes())?;
                    }
                    _ => repaint(buf, g.want.area, out)?,
                }
                wrote = true;
            }
        }
        let protocol = self.protocol;
        for (w, parts) in sends {
            write!(out, "\x1b7\x1b[{};{}H", w.area.y + 1, w.area.x + 1)?;
            for p in &parts {
                out.write_all(p.as_bytes())?;
            }
            out.write_all(b"\x1b8")?;
            if protocol == Some(Protocol::Kitty) {
                for g in covered.iter().filter(|g| over(&w, g)) {
                    if kitty_id(g.want.slot, g.want.art) != kitty_id(w.slot, w.art) {
                        out.write_all(take_down(g.want).as_bytes())?;
                    }
                }
                KITTY_SHOWN.fetch_or(w.slot.bit(), Ordering::SeqCst);
            }
            self.sends += 1;
            wrote = true;
            self.shown.push(Shown { want: w, size });
        }
        if self.erase_with == Some(Protocol::Kitty) && !self.freeing.is_empty() {
            wrote |= self.free_let_go(out)?;
        }
        if wrote {
            out.flush()?;
        }
        Ok(())
    }

    /// Since when `w`'s box has been where it is: a slot first seen counts as settled.
    fn since(&mut self, w: &Want, now: Instant) -> Instant {
        if let Some((_, area, since)) = self.seen.iter_mut().find(|(s, ..)| *s == w.slot) {
            if *area != w.area {
                *area = w.area;
                *since = now;
            }
            return *since;
        }
        let settled = now.checked_sub(SETTLE).unwrap_or(now);
        self.seen.push((w.slot, w.area, settled));
        settled
    }

    /// What shows `w`: a clip's frame from its reel, or a still from the cache, else built once.
    /// `None` while the frame is encoding.
    fn payload(&mut self, protocol: Protocol, w: &Want) -> Option<Vec<Arc<str>>> {
        if let Art::Frame(clip, n) = w.art {
            return self.frame(protocol, w, clip, n);
        }
        self.still(protocol, w).map(|p| vec![p])
    }

    /// Frame `n` of `clip` for `w`: its pixels from the reel, which starts encoding on the first
    /// ask; under Kitty, its transmission only the first time, then its placement alone.
    fn frame(&mut self, protocol: Protocol, w: &Want, clip: Clip, n: u16) -> Option<Vec<Arc<str>>> {
        let (cw, ch) = (u32::from(self.cell.0), u32::from(self.cell.1));
        let key = ReelKey {
            clip,
            protocol,
            cells: (w.area.width, w.area.height),
            px: (u32::from(w.area.width) * cw, u32::from(w.area.height) * ch),
            bg: w.bg,
        };
        let reel = if let Some(r) = self.reels.get(&key) {
            Arc::clone(r)
        } else {
            if self.reels.len() >= REELS {
                self.let_reels_go();
            }
            let reel = Arc::new(Reel::default());
            let (r, encoded) = (Arc::clone(&reel), Arc::clone(&self.encoded));
            let first = usize::from(n);
            std::thread::spawn(move || encode(key, first, &r, &encoded));
            self.reels.insert(key, Arc::clone(&reel));
            reel
        };
        let bytes = reel.get(usize::from(n))?;
        if protocol != Protocol::Kitty {
            return Some(vec![bytes]);
        }
        let id = kitty_id(w.slot, w.art);
        let mut parts = Vec::with_capacity(2);
        // Sent when not stored from this reel, and again every REFRESH placements: Kitty may
        // evict a frame under its quota, and with `q=2` nothing says so — a frame lost blanks for
        // a few turns of the loop at most.
        match self.stored.get_mut(&id) {
            Some((k, placed)) if *k == key && *placed < REFRESH => *placed += 1,
            _ => {
                parts.push(bytes);
                self.stored.insert(id, (key, 1));
                KITTY_SHOWN.fetch_or(FRAMES_STORED, Ordering::SeqCst);
            }
        }
        parts.push(Arc::from(pixels::kitty_place(
            id,
            w.area.width,
            w.area.height,
        )));
        Some(parts)
    }

    /// The encoded still for `w`: from the cache, else built once.
    fn still(&mut self, protocol: Protocol, w: &Want) -> Option<Arc<str>> {
        let key = (w.slot, w.art, w.area.width, w.area.height, w.bg);
        if let Some(p) = self.cache.get(&key) {
            return Some(Arc::clone(p));
        }
        let (cw, ch) = (u32::from(self.cell.0), u32::from(self.cell.1));
        let (px_w, px_h) = (u32::from(w.area.width) * cw, u32::from(w.area.height) * ch);
        let source = match w.art {
            Art::Veldora => pixels::VELDORA,
            Art::Frame(..) => return None,
        };
        let img = pixels::picture(source, px_w, px_h, w.bg)?;
        let encoded: String = match protocol {
            Protocol::Kitty => pixels::kitty(
                &pixels::png_bytes(&img),
                w.area.width,
                w.area.height,
                w.slot.id(),
            ),
            Protocol::Iterm => pixels::iterm(&pixels::png_bytes(&img), w.area.width, w.area.height),
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

/// Kitty: what takes `want`'s picture off the screen — a still deleted by its slot's id, a clip's
/// frame unplaced by its own, its data kept for the loop's next turn.
fn take_down(want: Want) -> String {
    match want.art {
        Art::Frame(..) => pixels::kitty_unplace(kitty_id(want.slot, want.art)),
        Art::Veldora => pixels::kitty_delete(want.slot.id()),
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
    use mnemorph_races::Race;

    fn placer(p: Protocol) -> Placer {
        Placer::new(&Pictures::new(p, Some((8, 17))))
    }

    /// A still picture: Veldora, on a background of its own for each race named, so two
    /// races are two pictures.
    fn want(slot: Slot, race: Race, x: u16, y: u16) -> Want {
        let shade = u8::try_from(Race::ALL.iter().position(|r| *r == race).unwrap()).unwrap();
        Want {
            slot,
            art: Art::Veldora,
            area: Rect::new(x, y, 4, 2),
            bg: Rgb(6, 11, 18 + shade),
        }
    }

    fn frame(n: u16) -> Want {
        Want {
            slot: Slot::Sidebar,
            art: Art::Frame(Clip::Idle(Race::Slime), n),
            area: Rect::new(30, 1, 4, 2),
            bg: Rgb(6, 11, 18),
        }
    }

    /// Sync `w` until it is on screen — its frame is encoded on a thread of its own — and return
    /// what was written then.
    fn show(pl: &mut Placer, w: Want, size: (u16, u16), buf: &Buffer) -> String {
        let start = Instant::now();
        loop {
            let mut out = Vec::new();
            pl.sync_at(&[w], size, buf, &mut out, Instant::now())
                .unwrap();
            if pl.shown.iter().any(|s| s.want == w && s.size == size) {
                return String::from_utf8(out).unwrap();
            }
            assert!(
                out.is_empty() || !pl.shown.is_empty(),
                "nothing erased while it waits"
            );
            assert!(
                pl.due(Instant::now()).is_some(),
                "a frame encoding asks to be looked for"
            );
            assert!(start.elapsed() < Duration::from_secs(120), "never encoded");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Wait until every frame of the slime's loop is encoded.
    fn all_encoded(pl: &Placer) {
        let start = Instant::now();
        while pl.encoded() < 48 {
            assert!(start.elapsed() < Duration::from_secs(300), "never encoded");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_kitty_clip_is_stored_once_by_id_then_placed() {
        let mut pl = placer(Protocol::Kitty);
        let buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        let first = show(&mut pl, frame(0), (40, 20), &buf);
        let id0 = kitty_id(Slot::Sidebar, Art::Frame(Clip::Idle(Race::Slime), 0));
        let id1 = id0 + 1;
        assert!(
            first.contains(&format!("\x1b_Ga=t,f=100,i={id0},q=2,")),
            "stored by id"
        );
        assert!(
            first.contains(&pixels::kitty_place(id0, 4, 2)),
            "then placed"
        );
        // The next frame: stored, placed, and only then the old one taken down — never a blank.
        let next = show(&mut pl, frame(1), (40, 20), &buf);
        let (place, down) = (
            next.find(&pixels::kitty_place(id1, 4, 2)).unwrap(),
            next.find(&pixels::kitty_unplace(id0)).unwrap(),
        );
        assert!(place < down, "placed before the old one goes");
        // Frame 0 again, the loop's next turn: its placement alone, a few bytes.
        let again = show(&mut pl, frame(0), (40, 20), &buf);
        assert!(!again.contains("a=t"), "never stored twice: {again:.120}");
        assert!(again.contains(&pixels::kitty_place(id0, 4, 2)));
        assert!(again.len() < 200, "{} bytes", again.len());
        all_encoded(&pl);
        // The sidebar folds: the frame taken off by id, its data kept.
        let mut out = Vec::new();
        pl.sync(&[], (40, 20), &buf, &mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), pixels::kitty_unplace(id0));
        // Shown again, then the terminal resized: taken off by id and placed anew.
        show(&mut pl, frame(0), (40, 20), &buf);
        let resized = show(&mut pl, frame(0), (41, 20), &buf);
        assert!(
            resized.starts_with(&pixels::kitty_unplace(id0)),
            "{resized:.80}"
        );
        assert!(resized.contains(&pixels::kitty_place(id0, 4, 2)));
        // Leaving: every frame stored is freed by its id.
        let mut out = Vec::new();
        pl.clear(&mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        for id in [id0, id1] {
            assert!(out.contains(&pixels::kitty_delete(id)), "{id} freed");
        }
        assert!(!pl.showing());
        assert_eq!(pl.encoded(), 48, "each frame encoded once");
    }

    #[test]
    fn a_reel_let_go_frees_its_kitty_frames_and_a_stored_frame_is_refreshed() {
        let mut pl = placer(Protocol::Kitty);
        let buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        let id0 = kitty_id(Slot::Sidebar, Art::Frame(Clip::Idle(Race::Slime), 0));
        // Two frames by turns: frame 0 placed from the store, sent again every REFRESH placements.
        let mut sent0 = 0;
        for turn in 0..2 * REFRESH + 2 {
            let out = show(
                &mut pl,
                frame(u16::try_from(turn % 2).unwrap()),
                (40, 20),
                &buf,
            );
            if out.contains(&format!("a=t,f=100,i={id0},")) {
                sent0 += 1;
            }
        }
        assert_eq!(
            sent0, 2,
            "stored once, refreshed once after {REFRESH} placements"
        );
        // A font zoom lets the reels go: their frames are freed by id, but the one on screen
        // waits until it is off.
        pl.set_cell((9, 18));
        let mut out = Vec::new();
        pl.sync(&[], (40, 20), &buf, &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        let unplace = out
            .find(&pixels::kitty_unplace(id0 + 1))
            .expect("taken off");
        for id in [id0, id0 + 1] {
            let free = out.find(&pixels::kitty_delete(id)).expect("freed");
            assert!(free > unplace, "freed once off the screen");
        }
        // Nothing is freed twice, and shown again it is stored anew.
        let mut again = Vec::new();
        pl.sync(&[], (40, 20), &buf, &mut again).unwrap();
        assert_eq!(again, b"");
        let back = show(&mut pl, frame(0), (40, 20), &buf);
        assert!(back.contains(&format!("a=t,f=100,i={id0},")), "sent again");
    }

    #[test]
    fn the_frame_cache_never_encodes_an_unchanged_frame_again() {
        for p in [Protocol::Iterm, Protocol::Sixel] {
            let mut pl = placer(p);
            let mut frame_buf = Buffer::empty(Rect::new(0, 0, 40, 20));
            frame_buf.set_string(30, 1, "abcd", ratatui::style::Style::new());
            show(&mut pl, frame(0), (40, 20), &frame_buf);
            all_encoded(&pl);
            let sends = pl.sends;
            // Two turns of the loop at 8 frames a second: each frame from the cache.
            for n in (0..48u16).step_by(2).chain((0..48u16).step_by(2)).skip(1) {
                let out = show(&mut pl, frame(n), (40, 20), &frame_buf);
                assert!(
                    !out.contains("\x1b[0m\x1b[4X"),
                    "{p:?}: a new frame never erases the old first"
                );
                assert!(out.starts_with("\x1b7\x1b[2;31H"), "{p:?}");
            }
            assert_eq!(pl.encoded(), 48, "{p:?}: encoded once, however often shown");
            assert_eq!(pl.sends, sends + 47, "{p:?}");
            // The same frame again sends nothing at all.
            let mut out = Vec::new();
            pl.sync(&[frame(46)], (40, 20), &frame_buf, &mut out)
                .unwrap();
            assert_eq!(out, b"", "{p:?}");
        }
    }

    #[test]
    fn a_clip_plays_at_the_protocols_pace() {
        let clip = Clip::Idle(Race::Orc);
        let step = Duration::from_secs(1) / 16;
        let kitty = Pictures::new(Protocol::Kitty, Some((8, 17)));
        let sixel = Pictures::new(Protocol::Sixel, Some((8, 17)));
        assert_eq!((kitty.fps(), sixel.fps()), (16, 8));
        assert_eq!(kitty.frame(clip, step * 3), 3);
        assert_eq!(
            sixel.frame(clip, step * 3),
            2,
            "every other frame at 8 a second"
        );
        assert_eq!(sixel.frame(clip, step * 49), 0, "seamless");
        let rise = Clip::Ascend(Race::Orc);
        assert_eq!(
            sixel.frame(rise, step * 500),
            79,
            "an ascension holds its last frame"
        );
        // Room for a 256 px frame at its own size: 16 rows of 17 px.
        assert_eq!(kitty.rows(32), Some(16));
        assert_eq!(
            Pictures::new(Protocol::Iterm, Some((7, 15))).rows(32),
            Some(15)
        );
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
            &[want(Slot::Sidebar, Race::Elf, 3, 5)],
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
        // The working body changes over the same box: the new picture sent under the slot's id,
        // which replaces the old one whole — nothing deleted first, so nothing flickers.
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
        assert!(!s.contains(&del), "never deleted first: {s:.80}");
        assert!(s.contains("a=T,f=100,i=5066241"));
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
            &[want(Slot::Sidebar, Race::Slime, 30, 1)],
            (40, 20),
            &frame,
            &mut Vec::new(),
        )
        .unwrap();
        // It moves: the old box erased, nothing sent while it moves.
        let t0 = Instant::now();
        let mut out = Vec::new();
        sx.sync_at(
            &[want(Slot::Sidebar, Race::Slime, 30, 3)],
            (40, 20),
            &frame,
            &mut out,
            t0,
        )
        .unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(
            s.contains("\x1b[2;31H\x1b[0m\x1b[4X"),
            "the old box's rows erased"
        );
        assert!(s.contains("ma\x1b[0mb"), "its text written again: {s:.200}");
        assert!(!s.contains("\x1bP"), "no picture sent for a moving box");
        assert_eq!(sx.due(t0), Some(SETTLE));
        // A frame a step later, still moving: no payload, no repaint either.
        for (i, y) in [4u16, 5, 6].into_iter().enumerate() {
            let at = t0 + Duration::from_millis(33 * (i as u64 + 1));
            let mut out = Vec::new();
            sx.sync_at(
                &[want(Slot::Sidebar, Race::Slime, 30, y)],
                (40, 20),
                &frame,
                &mut out,
                at,
            )
            .unwrap();
            assert_eq!(out, b"", "frame {i}: something sent while moving");
        }
        assert_eq!(sx.sends, 1);
        // Settled for 150 ms: placed once.
        let still = want(Slot::Sidebar, Race::Slime, 30, 6);
        let early = t0 + Duration::from_millis(99 + 140);
        let mut out = Vec::new();
        sx.sync_at(&[still], (40, 20), &frame, &mut out, early)
            .unwrap();
        assert_eq!(out, b"");
        let settled = t0 + Duration::from_millis(99) + SETTLE;
        assert_eq!(sx.due(early), Some(settled - early));
        let mut out = Vec::new();
        sx.sync_at(&[still], (40, 20), &frame, &mut out, settled)
            .unwrap();
        assert!(
            String::from_utf8(out)
                .unwrap()
                .starts_with("\x1b7\x1b[7;31H\x1bP0;1;0q")
        );
        assert_eq!(
            (sx.sends, sx.builds),
            (2, 1),
            "the same box size is built once"
        );
        assert_eq!(sx.due(settled), None);
    }

    #[test]
    fn sixel_falls_back_to_half_blocks_when_the_cell_cannot_be_read_again() {
        let mut pics = Pictures::new(Protocol::Sixel, Some((8, 17)));
        let mut pl = Placer::new(&pics);
        let buf = Buffer::empty(Rect::new(0, 0, 60, 30));
        let w = [want(Slot::Sidebar, Race::Rimuru, 30, 1)];
        pl.sync(&w, (60, 30), &buf, &mut Vec::new()).unwrap();
        // Zoomed out; the window gives no pixels: half blocks, the old picture's box repainted.
        resized(&mut pics, &mut pl, None);
        assert!(!pics.active() && pics.images());
        assert_eq!(pics.rows(32), None, "the view draws half blocks");
        let mut out = Vec::new();
        pl.sync(&w, (70, 30), &buf, &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(
            s.contains("\x1b[2;31H\x1b[0m\x1b[4X"),
            "the old box repainted"
        );
        assert!(!s.contains("\x1bP"), "no picture sent");
        assert!(!pl.showing());
        // A size to trust returns: pictures again, built at it.
        resized(&mut pics, &mut pl, Some((10, 21)));
        assert!(pics.active());
        assert_eq!(pics.cell, Some((10, 21)));
        pl.sync(&w, (70, 30), &buf, &mut Vec::new()).unwrap();
        assert_eq!(pl.sends, 2);
        // iTerm2 and Kitty fit their cells themselves: they keep their pictures.
        for p in [Protocol::Iterm, Protocol::Kitty] {
            let mut pics = Pictures::new(p, Some((8, 17)));
            let mut pl = Placer::new(&pics);
            resized(&mut pics, &mut pl, None);
            assert!(pics.active(), "{p:?}");
        }
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
    fn a_new_cell_size_builds_every_picture_again() {
        let mut pl = placer(Protocol::Iterm);
        let buf = Buffer::empty(Rect::new(0, 0, 60, 30));
        let wants = [want(Slot::Sidebar, Race::Rimuru, 30, 1)];
        pl.sync(&wants, (60, 30), &buf, &mut Vec::new()).unwrap();
        pl.set_cell((8, 17));
        pl.sync(&wants, (60, 31), &buf, &mut Vec::new()).unwrap();
        assert_eq!(pl.builds, 1, "the same cell: from the cache");
        // Zoomed: cells of 10 x 21 px, and the window's size changed with them.
        pl.set_cell((10, 21));
        pl.sync(&wants, (50, 25), &buf, &mut Vec::new()).unwrap();
        assert_eq!((pl.builds, pl.sends), (2, 3));
    }

    #[test]
    fn a_panic_deletes_the_kitty_pictures_once() {
        let mut pl = placer(Protocol::Kitty);
        let buf = Buffer::empty(Rect::new(0, 0, 60, 30));
        let wants = [want(Slot::Sidebar, Race::Rimuru, 30, 1)];
        pl.sync(&wants, (60, 30), &buf, &mut Vec::new()).unwrap();
        // The panic hook, then the screen's drop: deleted once, then nothing.
        let mut first = Vec::new();
        release(&mut first).unwrap();
        let first = String::from_utf8(first).unwrap();
        assert!(first.contains(&pixels::kitty_delete(Slot::Sidebar.id())));
        let mut second = Vec::new();
        release(&mut second).unwrap();
        assert_eq!(second, b"");
        // The normal way out leaves nothing for the hook either.
        pl.sync(&wants, (60, 31), &buf, &mut Vec::new()).unwrap();
        pl.clear(&mut Vec::new()).unwrap();
        let mut after = Vec::new();
        release(&mut after).unwrap();
        assert_eq!(after, b"");
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
            &[want(Slot::Sidebar, Race::Rimuru, 18, 9)],
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
