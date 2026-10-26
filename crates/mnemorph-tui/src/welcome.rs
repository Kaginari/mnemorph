//! The welcome: the card the conversation opens on, above its first entry — Rimuru's portrait, still,
//! the name and the tagline, the races as chips, the world, the model and the session
//! in small capitals, and the keys that matter first.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::Model;
use crate::kit::mnanim;
use crate::kit::slab::{Slab, Surface};
use crate::kit::text::{Lines, clip, small_caps, truncate};
use crate::safe::one_line;
use crate::theme::Rgb;

/// The tagline under the name.
pub const TAGLINE: &str = "an agent harness · the world remembers in documents";

/// The welcome card, `w` cells wide.
#[must_use]
pub fn card(m: &Model, w: usize) -> Lines {
    let t = &m.theme;
    let inner = Slab::inner(w);
    // Rimuru's portrait, still on its first frame.
    let art = mnanim::portrait(mnemorph_races::Race::Rimuru)
        .and_then(|p| p.frames.first().map(|f| mnanim::lines(f, None)))
        .unwrap_or_default();
    let room = inner.saturating_sub(mnanim::SIZE + 2);
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
    let beside = [
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            Span::styled("✦ ", t.holo().add_modifier(Modifier::BOLD)),
            Span::styled("mnemorph", t.rimuru()),
        ],
        vec![Span::styled(clip(TAGLINE, room), t.muted())],
        Vec::new(),
        chips,
    ];
    let mut rows: Lines = art
        .into_iter()
        .zip(beside.into_iter().chain(std::iter::repeat_with(Vec::new)))
        .map(|(a, text)| {
            let mut spans = a.spans;
            spans.push(Span::raw("  "));
            spans.extend(text);
            truncate(Line::from(spans), inner)
        })
        .collect();
    rows.push(Line::from(""));
    let field = |name: &str, value: &str| {
        truncate(
            Line::from(vec![
                Span::styled(format!("{:<9}", small_caps(name)), t.muted()),
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
    Slab::new(Surface::card(t, Some(t.races.dark_elf), 0.08)).render(rows, w)
}
