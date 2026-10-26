//! Real pixels: the races' portraits drawn as images through a terminal's graphics protocol —
//! Kitty's, iTerm2's inline images, or Sixel — as sharp as their source, where the terminal can.
//!
//! Everything here is pure: the terminal's replies read ([`Replies::parse`]), the protocol chosen
//! from them ([`decide`]), the box measured ([`square`]), the picture built ([`picture`]) and
//! encoded ([`kitty`], [`iterm`], [`sixel`]). Asking the terminal is [`crate::probe`]'s work;
//! placing, moving and erasing a picture over the cells is [`crate::pictures`]'.
//!
//! The source art is embedded ([`png`]): each race's 256 px portrait. No byte of an escape
//! sequence built here comes from the model or the world — only from those files and from numbers.

use std::fmt::Write as _;
use std::io::Cursor;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use image::imageops::{self, FilterType};
use image::{ImageFormat, RgbImage};
use mnemorph_races::Race;

use crate::theme::Rgb;

/// How the portraits are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// Kitty's graphics protocol: a PNG, placed by id and deleted by id.
    Kitty,
    /// iTerm2's inline images (iTerm2, `WezTerm`, xterm.js): a PNG, full colour.
    Iterm,
    /// Sixel: 255 colours, six pixel rows a band.
    Sixel,
    /// No image: half blocks, two pixels a cell, the idle loop playing.
    Blocks,
}

impl Protocol {
    /// Its name, as `--probe` prints it and `MNEMORPH_TUI_IMAGES` takes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::Iterm => "iterm",
            Self::Sixel => "sixel",
            Self::Blocks => "blocks",
        }
    }

    /// Whether it draws real pixels.
    #[must_use]
    pub fn pixels(self) -> bool {
        self != Self::Blocks
    }
}

/// What `MNEMORPH_TUI_IMAGES` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// The best the terminal answered for.
    Auto,
    /// This protocol, whatever the terminal said (it still needs the cell's size in pixels).
    Force(Protocol),
    /// No images and no probe: half blocks, nothing asked of the terminal.
    Off,
}

impl Choice {
    /// The variable's value read: `auto`, `kitty`, `iterm`, `sixel`, `blocks` or `off`; unset
    /// or empty is `auto`. An unknown value is `auto`, and named in the second part.
    #[must_use]
    pub fn read(value: Option<&str>) -> (Self, Option<String>) {
        let v = value
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let c = match v.as_str() {
            "" | "auto" => Self::Auto,
            "kitty" => Self::Force(Protocol::Kitty),
            "iterm" | "iterm2" => Self::Force(Protocol::Iterm),
            "sixel" => Self::Force(Protocol::Sixel),
            "blocks" | "halfblocks" => Self::Force(Protocol::Blocks),
            "off" | "none" | "0" => Self::Off,
            _ => {
                return (
                    Self::Auto,
                    Some(format!("MNEMORPH_TUI_IMAGES={v:?} is not known; auto")),
                );
            }
        };
        (c, None)
    }
}

/// The terminal's answers to the probe's queries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Replies {
    /// Its name and version (XTVERSION), as it gave them.
    pub name: Option<String>,
    /// A cell's size in pixels, width then height (`CSI 16 t`).
    pub cell: Option<(u16, u16)>,
    /// Kitty's graphics query answered OK.
    pub kitty: bool,
    /// The primary device attributes (DA1), when they came: every terminal answers them, so they
    /// mark the end of the replies.
    pub attributes: Option<Vec<u16>>,
}

/// The Kitty graphics query's image id: the probe's alone.
pub const QUERY_ID: u32 = 31;

impl Replies {
    /// Read what came back, in any order and among anything else.
    #[must_use]
    pub fn parse(reply: &str) -> Self {
        let name = between(reply, "\x1bP>|", "\x1b\\").map(|n| {
            n.chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect::<String>()
        });
        let cell = between(reply, "\x1b[6;", "t").and_then(|s| {
            let (h, w) = s.split_once(';')?;
            let (h, w) = (h.parse::<u16>().ok()?, w.parse::<u16>().ok()?);
            (h > 0 && w > 0).then_some((w, h))
        });
        let kitty = reply.contains(&format!("\x1b_Gi={QUERY_ID};OK"));
        let attributes = attributes(reply);
        Self {
            name,
            cell,
            kitty,
            attributes,
        }
    }

    /// Whether DA1 lists attribute 4: Sixel graphics, on.
    #[must_use]
    pub fn sixel(&self) -> bool {
        self.attributes.as_ref().is_some_and(|a| a.contains(&4))
    }
}

/// The text between the first `open` and the `close` after it.
fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let from = s.find(open)? + open.len();
    let len = s[from..].find(close)?;
    Some(&s[from..from + len])
}

/// The primary device attributes in `reply`: `ESC [ ? a ; b … c`.
#[must_use]
pub fn attributes(reply: &str) -> Option<Vec<u16>> {
    let mut rest = reply;
    while let Some(at) = rest.find("\x1b[?") {
        let body = &rest[at + 3..];
        let end = body
            .find(|c: char| !(c.is_ascii_digit() || c == ';'))
            .unwrap_or(body.len());
        if body[end..].starts_with('c') {
            return Some(
                body[..end]
                    .split(';')
                    .filter_map(|n| n.parse().ok())
                    .collect(),
            );
        }
        rest = body;
    }
    None
}

/// The terminal's window as `TIOCGWINSZ` gives it: cells across and down, pixels across and down.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Window {
    /// Columns.
    pub cols: u16,
    /// Rows.
    pub rows: u16,
    /// Pixels across, `0` when the terminal does not say.
    pub xpx: u16,
    /// Pixels down, `0` when the terminal does not say.
    pub ypx: u16,
}

/// What the probe found, and what follows from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caps {
    /// The replies.
    pub replies: Replies,
    /// The window, when the terminal gave it.
    pub window: Option<Window>,
    /// A cell's size in pixels: the reply's, else the window's pixels over its cells.
    pub cell: Option<(u16, u16)>,
    /// A terminal that draws iTerm2's inline images: xterm.js, iTerm2, `WezTerm`.
    pub iterm_family: bool,
    /// What `MNEMORPH_TUI_IMAGES` asked.
    pub choice: Choice,
    /// The protocol chosen.
    pub protocol: Protocol,
    /// Why, in a few words.
    pub why: String,
    /// The replies as they came, escaped, for support.
    pub raw: String,
    /// Whether the probe gave up waiting.
    pub timed_out: bool,
    /// Notes: an unknown override, a terminal multiplexer.
    pub notes: Vec<String>,
}

impl Caps {
    /// The capabilities of a terminal never asked: half blocks.
    #[must_use]
    pub fn blocks(why: &str) -> Self {
        Self {
            replies: Replies::default(),
            window: None,
            cell: None,
            iterm_family: false,
            choice: Choice::Auto,
            protocol: Protocol::Blocks,
            why: why.into(),
            raw: String::new(),
            timed_out: false,
            notes: Vec::new(),
        }
    }

    /// The report `--probe` prints.
    #[must_use]
    pub fn report(&self) -> String {
        let yes = |b: bool| if b { "yes" } else { "no" };
        let mut s = String::new();
        let _ = writeln!(
            s,
            "terminal     {}",
            self.replies.name.as_deref().unwrap_or("unknown")
        );
        if let Some(w) = self.window {
            let _ = writeln!(
                s,
                "window       {} x {} cells, {} x {} px",
                w.cols, w.rows, w.xpx, w.ypx
            );
        }
        let _ = writeln!(
            s,
            "cell         {}",
            self.cell
                .map_or_else(|| "unknown".into(), |(w, h)| format!("{w} x {h} px"))
        );
        let _ = writeln!(s, "kitty        {}", yes(self.replies.kitty));
        let _ = writeln!(s, "sixel        {}", yes(self.replies.sixel()));
        let _ = writeln!(s, "iterm2       {}", yes(self.iterm_family));
        let choice = match self.choice {
            Choice::Auto => "auto".to_owned(),
            Choice::Force(p) => p.name().to_owned(),
            Choice::Off => "off".to_owned(),
        };
        let _ = writeln!(s, "asked        {choice} (MNEMORPH_TUI_IMAGES)");
        let _ = writeln!(s, "drawing      {} — {}", self.protocol.name(), self.why);
        if self.timed_out {
            let _ = writeln!(s, "timed out    the terminal did not finish answering");
        }
        for n in &self.notes {
            let _ = writeln!(s, "note         {n}");
        }
        let _ = writeln!(
            s,
            "replies      {}",
            if self.raw.is_empty() {
                "none"
            } else {
                &self.raw
            }
        );
        s
    }
}

/// Escape a reply for printing: control bytes as `\x1b`, never sent back raw.
#[must_use]
pub fn escaped(reply: &str) -> String {
    reply.chars().flat_map(char::escape_default).collect()
}

/// Whether the terminal's name or the environment says it draws iTerm2's inline images, and
/// whether the environment alone says images are on (iTerm2 and `WezTerm` always draw them;
/// xterm.js only once its image add-on is enabled, which DA1's attribute 4 tells).
fn iterm_family(name: Option<&str>, env: &impl Fn(&str) -> Option<String>) -> (bool, bool) {
    let name = name.unwrap_or_default();
    let program = env("TERM_PROGRAM").unwrap_or_default();
    let native = name.contains("iTerm2")
        || name.contains("WezTerm")
        || program == "iTerm.app"
        || program == "WezTerm"
        || env("LC_TERMINAL").as_deref() == Some("iTerm2")
        || env("WEZTERM_EXECUTABLE").is_some();
    (native || name.contains("xterm.js"), native)
}

/// The protocol for a terminal that gave `replies` and `window`, as `choice` asks, the
/// environment read through `env`.
///
/// Kitty if it answered; iTerm2's inline images for xterm.js, iTerm2 or `WezTerm` with images on —
/// a PNG in full colour, smaller than Sixel; Sixel if DA1 lists it; else half blocks. Every
/// image protocol needs a cell's size in pixels, to make the picture exactly its box.
#[must_use]
pub fn decide(
    replies: Replies,
    window: Option<Window>,
    choice: Choice,
    env: &impl Fn(&str) -> Option<String>,
) -> Caps {
    let cell = replies.cell.or_else(|| {
        let w = window?;
        (w.xpx > 0 && w.ypx > 0 && w.cols > 0 && w.rows > 0)
            .then(|| (w.xpx / w.cols, w.ypx / w.rows))
            .filter(|(x, y)| *x > 0 && *y > 0)
    });
    let (iterm_family, native) = iterm_family(replies.name.as_deref(), env);
    let mut notes = Vec::new();
    let (protocol, why) = match choice {
        Choice::Off => (Protocol::Blocks, "images off".to_owned()),
        Choice::Force(p) => (p, "as MNEMORPH_TUI_IMAGES asks".to_owned()),
        Choice::Auto => {
            let on = replies.sixel() || replies.kitty || native;
            if replies.kitty {
                (
                    Protocol::Kitty,
                    "the terminal answered Kitty's query".into(),
                )
            } else if iterm_family && on {
                (
                    Protocol::Iterm,
                    "an iTerm2-image terminal with images on".into(),
                )
            } else if replies.sixel() {
                (Protocol::Sixel, "DA1 lists Sixel".into())
            } else if iterm_family {
                (
                    Protocol::Blocks,
                    "images are off in this terminal (VS Code: terminal.integrated.enableImages)"
                        .into(),
                )
            } else {
                (Protocol::Blocks, "no image protocol answered".into())
            }
        }
    };
    let (protocol, why) = if protocol.pixels() && cell.is_none() {
        (
            Protocol::Blocks,
            "the cell's size in pixels is unknown".into(),
        )
    } else if protocol.pixels() && choice == Choice::Auto && env("TMUX").is_some() {
        notes.push("inside tmux: images are not passed through".into());
        (Protocol::Blocks, "inside tmux".into())
    } else {
        (protocol, why)
    };
    Caps {
        replies,
        window,
        cell,
        iterm_family,
        choice,
        protocol,
        why,
        raw: String::new(),
        timed_out: false,
        notes,
    }
}

/// A square portrait's box `cols` cells wide: its rows, as near square in pixels as whole cells
/// allow, and its size in pixels — exactly the cells', so the terminal never rescales it.
#[must_use]
pub fn square(cols: u16, cell: (u16, u16)) -> (u16, u32, u32) {
    let (cw, ch) = (u32::from(cell.0.max(1)), u32::from(cell.1.max(1)));
    let wide = u32::from(cols) * cw;
    let rows = ((wide * 2 + ch) / (ch * 2)).max(1);
    let rows = u16::try_from(rows).unwrap_or(u16::MAX);
    (rows, wide, u32::from(rows) * ch)
}

/// The embedded portrait of `race`: a 256 px RGBA PNG.
#[must_use]
pub fn png(race: Race) -> &'static [u8] {
    macro_rules! art {
        ($name:literal) => {
            include_bytes!(concat!("../../assets/portraits/", $name, ".png"))
        };
    }
    match race {
        Race::Rimuru => art!("rimuru"),
        Race::Elf => art!("elf"),
        Race::Orc => art!("orc"),
        Race::Slime => art!("slime"),
        Race::Kijin => art!("kijin"),
        Race::DarkElf => art!("dark-elf"),
        Race::HighElf => art!("high-elf"),
        Race::HighOrc => art!("high-orc"),
        Race::NamedSlime => art!("named-slime"),
        Race::Oni => art!("oni"),
        Race::ArchDarkElf => art!("arch-dark-elf"),
    }
}

/// `source` (a PNG) as a `w` × `h` picture: laid on `bg` at its own size first, so its clear
/// edges blend with the background and never with black, then Lanczos-resized to a square as
/// large as the box allows, centred on `bg`. `None` when the PNG does not decode.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn picture(source: &[u8], w: u32, h: u32, bg: Rgb) -> Option<RgbImage> {
    let src = image::load_from_memory_with_format(source, ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let flat = RgbImage::from_fn(src.width(), src.height(), |x, y| {
        let p = src.get_pixel(x, y).0;
        let a = u16::from(p[3]);
        let mix = |c: u8, b: u8| {
            let v = (u16::from(c) * a + u16::from(b) * (255 - a) + 127) / 255;
            u8::try_from(v).unwrap_or(u8::MAX)
        };
        image::Rgb([mix(p[0], bg.0), mix(p[1], bg.1), mix(p[2], bg.2)])
    });
    let side = w.min(h).max(1);
    let scaled = imageops::resize(&flat, side, side, FilterType::Lanczos3);
    let mut out = RgbImage::from_pixel(w.max(1), h.max(1), image::Rgb([bg.0, bg.1, bg.2]));
    imageops::replace(
        &mut out,
        &scaled,
        i64::from((w.max(1) - side) / 2),
        i64::from((h.max(1) - side) / 2),
    );
    Some(out)
}

/// `img` as PNG bytes.
#[must_use]
pub fn png_bytes(img: &RgbImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    // Writing a PNG into memory fails only on an image too large for the format.
    let _ = img.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png);
    bytes
}

/// The base64 a Kitty chunk carries at most.
pub const KITTY_CHUNK: usize = 4096;

/// Kitty: transmit and display `png` as image `id`, over `cols` × `rows` cells from the cursor,
/// in chunks of [`KITTY_CHUNK`]; the cursor stays (`C=1`) and the terminal answers nothing
/// (`q=2`).
#[must_use]
pub fn kitty(png: &[u8], cols: u16, rows: u16, id: u32) -> String {
    let data = STANDARD.encode(png);
    let chunks: Vec<&str> = data
        .as_bytes()
        .chunks(KITTY_CHUNK)
        // base64 is ASCII: every chunk is text.
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect();
    let mut out = String::with_capacity(data.len() + chunks.len() * 16 + 64);
    let last = chunks.len().saturating_sub(1);
    for (i, c) in chunks.iter().enumerate() {
        let more = u8::from(i < last);
        if i == 0 {
            let _ = write!(
                out,
                "\x1b_Ga=T,f=100,i={id},c={cols},r={rows},C=1,q=2,m={more};{c}\x1b\\"
            );
        } else {
            let _ = write!(out, "\x1b_Gq=2,m={more};{c}\x1b\\");
        }
    }
    out
}

/// Kitty: delete image `id`, its placements and its data.
#[must_use]
pub fn kitty_delete(id: u32) -> String {
    format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\")
}

/// iTerm2's inline image: `png`, `w` × `h` pixels — its own size, never scaled — the cursor kept.
#[must_use]
pub fn iterm(png: &[u8], w: u32, h: u32) -> String {
    format!(
        "\x1b]1337;File=inline=1;size={};width={w}px;height={h}px;preserveAspectRatio=0;doNotMoveCursor=1:{}\x07",
        png.len(),
        STANDARD.encode(png)
    )
}

/// `img` reduced to at most `n` colours by median cut: the palette, and each pixel's index into
/// it, row-major.
#[must_use]
pub fn quantize(img: &RgbImage, n: usize) -> (Vec<[u8; 3]>, Vec<u8>) {
    use std::collections::HashMap;
    let n = n.clamp(1, 256);
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    for p in img.pixels() {
        *counts.entry(p.0).or_default() += 1;
    }
    let mut boxes: Vec<Vec<([u8; 3], u32)>> = vec![counts.into_iter().collect()];
    let range = |b: &[([u8; 3], u32)], ch: usize| {
        let (lo, hi) = b.iter().fold((255u8, 0u8), |(lo, hi), (c, _)| {
            (lo.min(c[ch]), hi.max(c[ch]))
        });
        hi.saturating_sub(lo)
    };
    while boxes.len() < n {
        // The box to split: the widest along any channel, among those of two colours or more.
        let pick = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .map(|(i, b)| {
                let (ch, r) = (0..3)
                    .map(|ch| (ch, range(b, ch)))
                    .max_by_key(|(_, r)| *r)
                    .unwrap_or((0, 0));
                (i, ch, r)
            })
            .max_by_key(|(_, _, r)| *r);
        let Some((i, ch, _)) = pick else { break };
        let mut b = boxes.swap_remove(i);
        b.sort_unstable_by_key(|(c, _)| c[ch]);
        let total: u64 = b.iter().map(|(_, k)| u64::from(*k)).sum();
        let mut acc = 0u64;
        let mut cut = 1;
        for (j, (_, k)) in b.iter().enumerate() {
            acc += u64::from(*k);
            if acc * 2 >= total {
                cut = (j + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
    }
    let mut palette = Vec::with_capacity(boxes.len());
    let mut index: HashMap<[u8; 3], u8> = HashMap::new();
    for (i, b) in boxes.iter().enumerate() {
        let total: u64 = b.iter().map(|(_, k)| u64::from(*k)).sum::<u64>().max(1);
        let mean = |ch: usize| {
            let s: u64 = b
                .iter()
                .map(|(c, k)| u64::from(c[ch]) * u64::from(*k))
                .sum();
            u8::try_from(s / total).unwrap_or(u8::MAX)
        };
        palette.push([mean(0), mean(1), mean(2)]);
        let i = u8::try_from(i).unwrap_or(u8::MAX);
        for (c, _) in b {
            index.insert(*c, i);
        }
    }
    let pixels = img.pixels().map(|p| index[&p.0]).collect();
    (palette, pixels)
}

/// Sixel: `img` in 255 colours, run-length coded, six pixel rows a band. Unset pixels stay clear
/// (`P2 = 1`), so a last band shorter than six rows paints nothing below the picture.
#[must_use]
pub fn sixel(img: &RgbImage) -> String {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let (palette, px) = quantize(img, 255);
    let mut out = String::with_capacity(w * h / 2);
    let _ = write!(out, "\x1bP0;1;0q\"1;1;{w};{h}");
    let pct = |v: u8| u32::from(v) * 100 / 255;
    for (i, c) in palette.iter().enumerate() {
        let _ = write!(out, "#{i};2;{};{};{}", pct(c[0]), pct(c[1]), pct(c[2]));
    }
    let mut used = vec![false; palette.len()];
    let mut line: Vec<u8> = Vec::with_capacity(w);
    for y0 in (0..h).step_by(6) {
        let band = (h - y0).min(6);
        used.fill(false);
        for dy in 0..band {
            for x in 0..w {
                used[usize::from(px[(y0 + dy) * w + x])] = true;
            }
        }
        let colours: Vec<usize> = (0..palette.len()).filter(|&c| used[c]).collect();
        for (n, &colour) in colours.iter().enumerate() {
            line.clear();
            for x in 0..w {
                let mut bits = 0u8;
                for dy in 0..band {
                    if usize::from(px[(y0 + dy) * w + x]) == colour {
                        bits |= 1 << dy;
                    }
                }
                line.push(63 + bits);
            }
            let _ = write!(out, "#{colour}");
            let mut i = 0;
            while i < line.len() {
                let mut j = i;
                while j < line.len() && line[j] == line[i] {
                    j += 1;
                }
                let ch = char::from(line[i]);
                if j - i > 3 {
                    let _ = write!(out, "!{}{ch}", j - i);
                } else {
                    (i..j).for_each(|_| out.push(ch));
                }
                i = j;
            }
            if n + 1 < colours.len() {
                out.push('$');
            }
        }
        out.push('-');
    }
    out.push_str("\x1b\\");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const XTERM_JS: &str = "\x1bP>|xterm.js(6.1.0-beta.304)\x1b\\\x1b[6;17;8t";

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k: &str| vars.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
    }

    fn chosen(reply: &str, choice: Choice, vars: &[(&str, &str)]) -> Protocol {
        decide(Replies::parse(reply), None, choice, &env(vars)).protocol
    }

    #[test]
    fn the_replies_are_read_in_any_order() {
        let r = Replies::parse(&format!("{XTERM_JS}\x1b[?1;2c"));
        assert_eq!(r.name.as_deref(), Some("xterm.js(6.1.0-beta.304)"));
        assert_eq!(r.cell, Some((8, 17)));
        assert!(!r.kitty);
        assert_eq!(r.attributes, Some(vec![1, 2]));
        assert!(!r.sixel());
        let r = Replies::parse("\x1b[?62;4;22c\x1b_Gi=31;OK\x1b\\");
        assert!(r.kitty && r.sixel());
        assert_eq!(Replies::parse(""), Replies::default());
    }

    #[test]
    fn xterm_js_with_images_on_draws_iterm2_and_off_draws_blocks() {
        let on = format!("{XTERM_JS}\x1b[?1;2;4c");
        assert_eq!(chosen(&on, Choice::Auto, &[]), Protocol::Iterm);
        let off = format!("{XTERM_JS}\x1b[?1;2c");
        let caps = decide(Replies::parse(&off), None, Choice::Auto, &env(&[]));
        assert_eq!(caps.protocol, Protocol::Blocks);
        assert!(caps.why.contains("enableImages"), "{}", caps.why);
    }

    #[test]
    fn kitty_comes_first_then_iterm2_then_sixel() {
        let kitty = "\x1b[6;20;10t\x1b_Gi=31;OK\x1b\\\x1b[?62;4c";
        assert_eq!(chosen(kitty, Choice::Auto, &[]), Protocol::Kitty);
        // A Sixel terminal of no iTerm2 family draws Sixel.
        let sixel = "\x1bP>|foot(1.20)\x1b\\\x1b[6;20;10t\x1b[?62;4c";
        assert_eq!(chosen(sixel, Choice::Auto, &[]), Protocol::Sixel);
        // WezTerm by its environment draws iTerm2's images, even with Sixel listed.
        let wez = "\x1b[6;20;10t\x1b[?62;4c";
        assert_eq!(
            chosen(wez, Choice::Auto, &[("TERM_PROGRAM", "WezTerm")]),
            Protocol::Iterm
        );
        assert_eq!(
            chosen(
                "\x1b[6;20;10t\x1b[?62c",
                Choice::Auto,
                &[("LC_TERMINAL", "iTerm2")]
            ),
            Protocol::Iterm
        );
    }

    #[test]
    fn no_answer_is_half_blocks() {
        assert_eq!(chosen("", Choice::Auto, &[]), Protocol::Blocks);
        assert_eq!(chosen("\x1b[?1;2c", Choice::Auto, &[]), Protocol::Blocks);
        // Sixel with no cell size cannot make an exact box.
        let caps = decide(Replies::parse("\x1b[?62;4c"), None, Choice::Auto, &env(&[]));
        assert_eq!(caps.protocol, Protocol::Blocks);
        assert!(caps.why.contains("cell"));
    }

    #[test]
    fn the_window_gives_the_cell_when_the_reply_does_not() {
        let w = Window {
            cols: 100,
            rows: 50,
            xpx: 800,
            ypx: 850,
        };
        let caps = decide(
            Replies::parse("\x1b[?62;4c"),
            Some(w),
            Choice::Auto,
            &env(&[]),
        );
        assert_eq!(caps.cell, Some((8, 17)));
        assert_eq!(caps.protocol, Protocol::Sixel);
    }

    #[test]
    fn the_environment_overrides_the_choice() {
        let on = format!("{XTERM_JS}\x1b[?1;2;4c");
        for (v, want) in [
            ("kitty", Protocol::Kitty),
            ("iterm", Protocol::Iterm),
            ("sixel", Protocol::Sixel),
            ("blocks", Protocol::Blocks),
            ("off", Protocol::Blocks),
            ("auto", Protocol::Iterm),
            ("", Protocol::Iterm),
        ] {
            let (choice, note) = Choice::read(Some(v));
            assert!(note.is_none(), "{v}");
            assert_eq!(chosen(&on, choice, &[]), want, "{v}");
        }
        assert_eq!(Choice::read(None).0, Choice::Auto);
        assert_eq!(Choice::read(Some("OFF")).0, Choice::Off);
        let (c, note) = Choice::read(Some("png"));
        assert_eq!(c, Choice::Auto);
        assert!(note.unwrap().contains("png"));
        // Forced, a protocol still needs the cell's size.
        assert_eq!(
            chosen("", Choice::Force(Protocol::Kitty), &[]),
            Protocol::Blocks
        );
        // Inside tmux, auto stays on half blocks.
        assert_eq!(
            chosen(&on, Choice::Auto, &[("TMUX", "/tmp/t")]),
            Protocol::Blocks
        );
    }

    #[test]
    fn the_box_is_exact_cells_of_pixels() {
        // 8 × 17 cells, 32 columns: 256 px across, 15 rows (255 px) the nearest square.
        assert_eq!(square(32, (8, 17)), (15, 256, 255));
        assert_eq!(square(32, (10, 20)), (16, 320, 320));
        assert_eq!(square(1, (8, 100)), (1, 8, 100), "never zero rows");
    }

    #[test]
    fn a_picture_is_exactly_its_box_on_the_background() {
        let bg = Rgb(6, 11, 18);
        let img = picture(png(Race::Rimuru), 256, 255, bg).unwrap();
        assert_eq!(img.dimensions(), (256, 255));
        // The square is 255 px, centred: the column left over is the background.
        assert_eq!(img.get_pixel(0, 0).0, [6, 11, 18]);
        for race in Race::ALL {
            assert!(picture(png(race), 16, 16, bg).is_some(), "{race:?}");
        }
        assert!(picture(b"not a png", 8, 8, bg).is_none());
    }

    #[test]
    fn kitty_payloads_are_chunked_and_terminated() {
        let data = vec![7u8; 10_000];
        let s = kitty(&data, 32, 15, 9);
        let parts: Vec<&str> = s.split("\x1b\\").filter(|p| !p.is_empty()).collect();
        let b64 = STANDARD.encode(&data).len();
        assert_eq!(parts.len(), b64.div_ceil(KITTY_CHUNK));
        assert!(parts[0].starts_with("\x1b_Ga=T,f=100,i=9,c=32,r=15,C=1,q=2,m=1;"));
        for p in &parts[1..parts.len() - 1] {
            assert!(p.starts_with("\x1b_Gq=2,m=1;"), "{p:.20}");
        }
        assert!(parts.last().unwrap().starts_with("\x1b_Gq=2,m=0;"));
        for p in &parts {
            assert!(p.starts_with("\x1b_G"));
            let body = p.split_once(';').unwrap().1;
            assert!(body.len() <= KITTY_CHUNK);
            assert!(!body.contains('\x1b'));
        }
        assert!(s.ends_with("\x1b\\"));
        // A small one is one chunk, its last.
        let one = kitty(&[1, 2, 3], 1, 1, 2);
        assert_eq!(one.matches("\x1b_G").count(), 1);
        assert!(one.contains(",m=0;"));
        assert_eq!(kitty_delete(9), "\x1b_Ga=d,d=I,i=9,q=2\x1b\\");
    }

    #[test]
    fn iterm_payloads_say_their_size_and_end_with_bel() {
        let s = iterm(&[1, 2, 3, 4], 256, 255);
        assert!(s.starts_with("\x1b]1337;File=inline=1;size=4;width=256px;height=255px;"));
        assert!(s.ends_with(":AQIDBA==\x07"));
        assert_eq!(s.matches('\x07').count(), 1);
    }

    #[test]
    fn sixel_payloads_are_well_formed() {
        let img = picture(png(Race::Slime), 24, 17, Rgb(6, 11, 18)).unwrap();
        let s = sixel(&img);
        assert!(s.starts_with("\x1bP0;1;0q\"1;1;24;17#0;2;"));
        assert!(s.ends_with("-\x1b\\"));
        // 17 rows: three bands.
        assert_eq!(s.matches('-').count(), 3);
        let body = &s[2..s.len() - 2];
        assert!(!body.contains('\x1b'), "one terminator only");
        assert!(body.chars().all(|c| c.is_ascii() && !c.is_ascii_control()));
    }

    #[test]
    fn quantizing_keeps_few_colours_exact_and_caps_many() {
        let two = RgbImage::from_fn(4, 4, |x, _| {
            if x < 2 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        });
        let (pal, px) = quantize(&two, 255);
        assert_eq!(pal.len(), 2);
        assert_eq!(pal[usize::from(px[0])], [255, 0, 0]);
        assert_eq!(pal[usize::from(px[3])], [0, 0, 255]);
        let many = picture(png(Race::Elf), 64, 64, Rgb(0, 0, 0)).unwrap();
        let (pal, px) = quantize(&many, 255);
        assert!(pal.len() <= 255);
        assert_eq!(px.len(), 64 * 64);
    }

    #[test]
    fn a_name_from_the_terminal_is_kept_printable() {
        let r = Replies::parse("\x1bP>|evil\x07\x08name\x1b\\");
        assert_eq!(r.name.as_deref(), Some("evilname"));
        assert_eq!(escaped("\x1b[c"), "\\u{1b}[c");
    }
}
