//! The welcome: the card the conversation opens on, above its first entry — the MNEMORPH
//! wordmark in block letters, the tagline, what Mnemorph does in four lines, the races as chips,
//! the world, the model, the session, the memory and the verify line in small capitals, and the
//! keys that matter first.
//!
//! The wordmark is drawn in text alone — `█ ▀ ▄` and spaces, each cell one foreground colour on
//! the card's face — so it is as sharp as the font in every terminal, and scrolls with the
//! transcript like any line. Pictures are the sidebar's alone. Across its width the letters run
//! the races' colours: Rimuru's blue, the elves' green, the orcs' amber, the slimes' teal. The
//! wordmark is 62 cells, so it shows on a card 68 wide or more (its frame takes 6): a terminal of
//! 72 columns with the sidebar folded, 108 with it open. On a narrower card a one-line
//! `◈ MNEMORPH` in the same gradient stands in.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::Model;
use crate::kit::slab::{Slab, Surface};
use crate::kit::text::{Lines, prefixed, small_caps, truncate, wrap_text};
use crate::safe::one_line;
use crate::theme::{Rgb, Theme};

/// The tagline under the wordmark.
pub const TAGLINE: &str = "an agent harness · the world remembers in documents";

/// What Mnemorph does, a line each.
pub const ABOUT: [&str; 4] = [
    "Rimuru takes your ask; Elves plan, Orcs route and judge, Slimes write — each in its own soil.",
    "Every change passes the end gate: your verify line, tests intact, the right author.",
    "Memory with provenance: a claim is grounded by a passing check, your word, or a second voice.",
    "The ontology keeps who said what, and shows disagreements instead of averaging them.",
];

/// The block letters, five rows each: full and half blocks only, so every cell is one colour.
const FONT: [(char, [&str; 5]); 7] = [
    ('M', ["██▄ ▄██", "██▀█▀██", "██   ██", "██   ██", "██   ██"]),
    ('N', ["██▄  ██", "██▀▄ ██", "██ ▀▄██", "██  ▀██", "██   ██"]),
    ('E', ["██████", "██    ", "█████ ", "██    ", "██████"]),
    ('O', ["▄█████▄", "██   ██", "██   ██", "██   ██", "▀█████▀"]),
    ('R', ["██████▄", "██   ██", "██████▀", "██  ▀█▄", "██   ██"]),
    ('P', ["██████▄", "██   ██", "██████▀", "██     ", "██     "]),
    ('H', ["██   ██", "██   ██", "███████", "██   ██", "██   ██"]),
];

/// The name the wordmark spells.
const NAME: &str = "MNEMORPH";

/// The wordmark's rows, uncoloured: each letter's row, one space between letters.
#[must_use]
pub fn wordmark_rows() -> [String; 5] {
    std::array::from_fn(|r| {
        NAME.chars()
            .filter_map(|c| FONT.iter().find(|(l, _)| *l == c))
            .map(|(_, rows)| rows[r])
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// The wordmark's width, in cells.
#[must_use]
pub fn wordmark_width() -> usize {
    wordmark_rows()[0].chars().count()
}

/// The races' gradient at `along` (0 at the left, 1 at the right): Rimuru → elf → orc → slime,
/// made readable on the theme's page.
#[must_use]
pub fn hue(t: &Theme, along: f32) -> Rgb {
    let stops = [t.races.rimuru, t.races.elf, t.races.orc, t.races.slime];
    let span = along.clamp(0.0, 1.0) * 3.0;
    // Display only: a stop's index, 0..=2.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let i = (span.floor() as usize).min(2);
    #[allow(clippy::cast_precision_loss)]
    let frac = span - i as f32;
    t.readable(stops[i].mix(stops[i + 1], frac))
}

/// Where column `x` of `width` lies along the gradient.
fn along(x: usize, width: usize) -> f32 {
    // Display only: a column's place across a line of a few dozen cells.
    #[allow(clippy::cast_precision_loss)]
    let f = x as f32 / width.saturating_sub(1).max(1) as f32;
    f
}

/// The wordmark in block letters, each cell coloured by its column; `None` when it is wider than
/// `room`.
#[must_use]
pub fn wordmark(t: &Theme, room: usize) -> Option<Lines> {
    let width = wordmark_width();
    if width > room {
        return None;
    }
    Some(
        wordmark_rows()
            .iter()
            .map(|row| {
                Line::from(
                    row.chars()
                        .enumerate()
                        .map(|(x, c)| {
                            if c == ' ' {
                                Span::raw(" ")
                            } else {
                                Span::styled(
                                    c.to_string(),
                                    Style::new().fg(hue(t, along(x, width)).color()),
                                )
                            }
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
    )
}

/// The narrow stand-in for the wordmark: `◈ MNEMORPH` on one line, in the same gradient.
#[must_use]
pub fn mark_line(t: &Theme) -> Line<'static> {
    let text: Vec<char> = format!("◈ {NAME}").chars().collect();
    let width = text.len();
    Line::from(
        text.into_iter()
            .enumerate()
            .map(|(x, c)| {
                Span::styled(
                    c.to_string(),
                    Style::new()
                        .fg(hue(t, along(x, width)).color())
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn surface(t: &Theme) -> Surface {
    Surface::card(t, Some(t.races.dark_elf), 0.08)
}

/// The welcome card, `w` cells wide.
#[must_use]
pub fn card(m: &Model, w: usize) -> Lines {
    let t = &m.theme;
    let inner = Slab::inner(w);
    let mut rows: Lines = wordmark(t, inner).unwrap_or_else(|| vec![mark_line(t)]);
    rows.push(Line::from(""));
    rows.extend(wrap_text(
        TAGLINE,
        inner,
        t.text().add_modifier(Modifier::ITALIC),
    ));
    rows.push(Line::from(""));
    let races = [t.races.rimuru, t.races.orc, t.races.elf, t.races.slime];
    for (text, hue) in ABOUT.into_iter().zip(races) {
        let bullet = Span::styled("▸ ", Style::new().fg(t.readable(hue).color()));
        let lines = wrap_text(text, inner.saturating_sub(2), t.muted());
        rows.extend(prefixed(lines, &[bullet], &[Span::raw("  ")]));
    }
    rows.push(Line::from(""));
    let chip = |name: &str, hue: Rgb| {
        Span::styled(
            format!(" {name} "),
            Style::new()
                .fg(t.readable(hue).color())
                .bg(t.p.bg.mix(hue, 0.25).color()),
        )
    };
    let mut chips = Vec::new();
    for (name, hue) in [
        ("rimuru", t.races.rimuru),
        ("elf", t.races.elf),
        ("orc", t.races.orc),
        ("slime", t.races.slime),
    ] {
        chips.push(chip(name, hue));
        chips.push(Span::raw(" "));
    }
    rows.push(truncate(Line::from(chips), inner));
    rows.push(Line::from(""));
    let field = |name: &str, value: &str| {
        truncate(
            Line::from(vec![
                label(t, name),
                Span::styled(one_line(value), t.text()),
            ]),
            inner,
        )
    };
    rows.push(field("world", &m.header.world));
    rows.push(field("model", &m.header.model));
    if !m.session_id.is_empty() {
        rows.push(field("session", &m.session_id));
    }
    rows.extend(world(m, inner));
    rows.push(Line::from(""));
    let mut keys = Vec::new();
    for (k, what) in [
        ("⏎", "ask"),
        ("/", "commands"),
        ("ctrl+o", "sidebar"),
        ("ctrl+t", "expand"),
        ("esc", "interrupt"),
    ] {
        keys.push(Span::styled(k, t.text().add_modifier(Modifier::BOLD)));
        keys.push(Span::styled(format!(" {what}   "), t.muted()));
    }
    rows.push(truncate(Line::from(keys), inner));
    Slab::new(surface(t)).render(rows, w)
}

/// A field's name in small capitals, padded to the values' column.
fn label(t: &Theme, name: &str) -> Span<'static> {
    Span::styled(format!("{:<9}", small_caps(name)), t.muted())
}

/// The memory and verify rows, as the world's survey read them; "reading the world…" until it
/// has.
fn world(m: &Model, inner: usize) -> Lines {
    let t = &m.theme;
    let pending = || Span::styled("reading the world…", t.muted());
    let bold = |s: String| Span::styled(s, t.text().add_modifier(Modifier::BOLD));
    let mut rows = Vec::new();
    rows.push(Line::from(match &m.sidebar.memory {
        Some(st) => vec![
            label(t, "memory"),
            Span::styled("grounded ", t.muted()),
            bold(st.grounded.to_string()),
            Span::styled(" · shared ", t.muted()),
            bold(st.shared.to_string()),
        ],
        None => vec![label(t, "memory"), pending()],
    }));
    match &m.sidebar.verify {
        None => rows.push(Line::from(vec![label(t, "verify"), pending()])),
        Some(v) if !v.line.is_empty() => rows.push(Line::from(vec![
            label(t, "verify"),
            Span::styled(one_line(&v.line.join(" · ")), t.text()),
        ])),
        Some(v) => {
            rows.push(Line::from(vec![
                label(t, "verify"),
                Span::styled("none — memory can't be grounded", t.gold()),
            ]));
            let command = v
                .suggested
                .as_deref()
                .map_or_else(|| "<your test command>".to_owned(), one_line);
            rows.push(Line::from(vec![
                Span::raw(" ".repeat(9)),
                Span::styled("add to rimuru's card  ", t.muted()),
                Span::styled(format!("- **Verify:** `{command}`"), t.text()),
            ]));
        }
    }
    rows.into_iter().map(|l| truncate(l, inner)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Cmd, Header, Model, Msg, update, view};
    use crate::sidebar::{Survey, Verify};
    use crate::theme::Variant;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    fn model(variant: Variant) -> Model {
        Model::new(
            Header {
                body: "rimuru".into(),
                model: "deepseek-v4-flash".into(),
                world: "~/sqlglot".into(),
            },
            variant,
            (120, 50),
        )
    }

    fn surveyed(m: &mut Model, verify: Verify) {
        let Cmd::Survey(seq) = m.survey() else {
            unreachable!("a survey asks for a survey")
        };
        update(
            m,
            Msg::Surveyed {
                seq,
                survey: Survey {
                    verify,
                    ..Survey::default()
                },
            },
        );
    }

    fn screen(m: &mut Model, width: u16, height: u16) -> (String, Buffer) {
        update(m, Msg::Resize(width, height));
        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        term.draw(|f| view(m, f)).unwrap();
        let b = term.backend().buffer().clone();
        let text = (0..b.area.height)
            .map(|y| {
                (0..b.area.width)
                    .map(|x| b[(x, y)].symbol().to_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        (text, b)
    }

    #[test]
    fn the_wordmark_is_drawn_with_crisp_glyphs_alone() {
        let rows = wordmark_rows();
        let width = wordmark_width();
        for row in &rows {
            assert_eq!(row.chars().count(), width, "every row as wide: {row:?}");
            for c in row.chars() {
                assert!(" █▀▄▌▐".contains(c), "{c:?} is not a crisp glyph");
            }
        }
        // Drawn: each glyph one foreground on the card's own face, never a background of its own.
        for variant in [Variant::Dark, Variant::Light] {
            let mut m = model(variant);
            let (text, buf) = screen(&mut m, 120, 50);
            let y = text
                .lines()
                .position(|l| l.contains(&rows[0]))
                .expect("the wordmark is drawn");
            let x = text
                .lines()
                .nth(y)
                .unwrap()
                .chars()
                .collect::<Vec<_>>()
                .windows(rows[0].chars().count())
                .position(|win| win.iter().collect::<String>() == rows[0])
                .unwrap();
            let face = surface(&m.theme).face.color();
            for (r, row) in rows.iter().enumerate() {
                for (i, c) in row.chars().enumerate() {
                    let at = (u16::try_from(x + i).unwrap(), u16::try_from(y + r).unwrap());
                    let cell = &buf[at];
                    assert_eq!(cell.symbol(), c.to_string(), "{at:?}");
                    assert_eq!(cell.bg, face, "{variant:?} {at:?}: one colour a cell");
                    if c != ' ' {
                        assert!(matches!(cell.fg, Color::Rgb(..)), "{at:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_wordmark_runs_the_races_gradient() {
        for variant in [Variant::Dark, Variant::Light] {
            let t = Theme::new(variant);
            let rows = wordmark(&t, 200).unwrap();
            let fgs: Vec<Color> = rows[2]
                .spans
                .iter()
                .filter(|s| s.content != " ")
                .filter_map(|s| s.style.fg)
                .collect();
            let first = *fgs.first().unwrap();
            let last = *fgs.last().unwrap();
            assert_eq!(
                first,
                hue(&t, 0.0).color(),
                "{variant:?}: Rimuru's blue first"
            );
            assert_eq!(
                last,
                hue(&t, 1.0).color(),
                "{variant:?}: the slimes' teal last"
            );
            assert_ne!(first, last);
            let distinct: std::collections::BTreeSet<String> =
                fgs.iter().map(|c| format!("{c:?}")).collect();
            assert!(
                distinct.len() > 20,
                "{variant:?}: a hue a column, not a few bands"
            );
        }
        let t = Theme::new(Variant::Dark);
        assert_eq!(hue(&t, 1.0 / 3.0), t.races.elf);
        assert_eq!(hue(&t, 2.0 / 3.0), t.races.orc);
    }

    #[test]
    fn a_narrow_card_falls_back_to_one_line() {
        let t = Theme::new(Variant::Dark);
        assert!(wordmark(&t, wordmark_width()).is_some());
        assert!(wordmark(&t, wordmark_width() - 1).is_none());
        let mut m = model(Variant::Dark);
        let first = |w: usize| crate::kit::text::plain(&card(&m, w)[1]);
        // A card narrower than the wordmark and its frame: one line, in the gradient.
        assert_eq!((wordmark_width(), Slab::inner(68)), (62, 62));
        assert!(first(67).contains("◈ MNEMORPH"), "{}", first(67));
        assert!(first(68).contains(&wordmark_rows()[0]), "{}", first(68));
        assert!(first(58).contains("◈ MNEMORPH"), "{}", first(58));
        assert!(first(40).contains("◈ MNEMORPH"));
        for w in [80, 120, 200] {
            assert!(first(w).contains(&wordmark_rows()[0]), "{w}: {}", first(w));
        }
        let (text, _) = screen(&mut m, 58, 50);
        assert!(!text.contains(&wordmark_rows()[0]), "{text}");
        for w in [80, 120, 200] {
            let (text, _) = screen(&mut m, w, 60);
            assert!(text.contains(&wordmark_rows()[0]), "{w} columns: {text}");
        }
    }

    #[test]
    fn the_wordmark_shows_from_72_columns_folded_and_108_open() {
        for (open, narrow, wide) in [(false, 71u16, 72u16), (true, 107, 108)] {
            let mut m = model(Variant::Dark);
            m.sidebar.open = open;
            let (text, _) = screen(&mut m, narrow, 60);
            assert!(!text.contains(&wordmark_rows()[0]), "{narrow}: {text}");
            assert!(
                text.lines().filter(|l| l.contains("◈ MNEMORPH")).count() >= 2,
                "{narrow}: the header's mark and the card's line: {text}"
            );
            let (text, _) = screen(&mut m, wide, 60);
            assert!(text.contains(&wordmark_rows()[0]), "{wide}: {text}");
        }
    }

    #[test]
    fn the_card_says_what_mnemorph_does() {
        for w in [80u16, 120, 200] {
            let mut m = model(Variant::Light);
            let (text, _) = screen(&mut m, w, 60);
            // The card's rows joined, its frame and bullets cut away: wrapped lines read whole.
            let flat: String = text
                .lines()
                .map(|l| {
                    l.trim_matches(|c: char| " ▌▏▕▖▘▝▀▄│".contains(c))
                        .trim_start_matches("▸ ")
                        .trim()
                })
                .collect::<Vec<_>>()
                .join(" ");
            assert!(flat.contains(TAGLINE), "{w}: {text}");
            for about in ABOUT {
                assert!(flat.contains(about), "{w}: {about:?} in {text}");
            }
            for chip in [" rimuru ", " elf ", " orc ", " slime "] {
                assert!(text.contains(chip), "{w}: {chip}");
            }
            for label in ["world", "model", "memory", "verify"] {
                assert!(text.contains(&small_caps(label)), "{w}: {label}");
            }
            assert!(text.contains("ctrl+o sidebar"));
        }
    }

    #[test]
    fn verify_shows_the_line_or_the_suggestion() {
        let mut m = model(Variant::Dark);
        let (text, _) = screen(&mut m, 120, 60);
        assert!(text.contains("reading the world…"), "before the survey");
        surveyed(
            &mut m,
            Verify {
                line: Vec::new(),
                suggested: Some("cargo test --workspace".into()),
            },
        );
        let (text, _) = screen(&mut m, 120, 60);
        assert!(text.contains("none — memory can't be grounded"), "{text}");
        assert!(
            text.contains("add to rimuru's card  - **Verify:** `cargo test --workspace`"),
            "{text}"
        );
        surveyed(
            &mut m,
            Verify {
                line: vec!["make check".into()],
                suggested: None,
            },
        );
        let (text, _) = screen(&mut m, 120, 60);
        assert!(text.contains(&format!("{:<9}make check", small_caps("verify"))));
        assert!(!text.contains("can't be grounded"));
    }

    #[test]
    fn no_picture_is_wanted_for_the_welcome() {
        let mut m = model(Variant::Dark);
        m.pictures =
            crate::pictures::Pictures::new(crate::kit::pixels::Protocol::Kitty, Some((8, 17)));
        // The sidebar folded: whatever is wanted now could only be the welcome's.
        m.sidebar.open = false;
        let _ = screen(&mut m, 120, 50);
        assert_eq!(m.pictures.wanted(), Vec::new());
        m.sidebar.open = true;
        let _ = screen(&mut m, 120, 50);
        let wanted = m.pictures.wanted();
        assert_eq!(wanted.len(), 1, "the sidebar's alone");
        assert_eq!(wanted[0].slot, crate::pictures::Slot::Sidebar);
    }

    #[test]
    fn nothing_overflows_the_card() {
        for variant in [Variant::Dark, Variant::Light] {
            let mut m = model(variant);
            surveyed(
                &mut m,
                Verify {
                    line: Vec::new(),
                    suggested: Some("cargo test --workspace".into()),
                },
            );
            for w in [40usize, 58, 80, 120, 200] {
                for line in card(&m, w) {
                    assert!(
                        crate::kit::text::line_cells(&line) <= w,
                        "{variant:?} {w}: {:?}",
                        crate::kit::text::plain(&line)
                    );
                }
            }
        }
    }
}
