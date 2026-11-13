//! The palette: Mnemorph's colours, the same values as `packages/theme` (a test keeps them
//! equal), and the theme the terminal wears — dark or light — with the roles every block reads.

use mnemorph_races::Race;
use ratatui::style::{Color, Modifier, Style};

/// One `#rrggbb` colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// From `0xrrggbb`.
    #[must_use]
    pub const fn hex(v: u32) -> Self {
        let [_, r, g, b] = v.to_be_bytes();
        Self(r, g, b)
    }

    /// As `#rrggbb`.
    #[must_use]
    pub fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// As a terminal colour.
    #[must_use]
    pub const fn color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }

    /// `self` moved toward `other` by `t` (0 keeps it, 1 is `other`).
    #[must_use]
    pub fn mix(self, other: Self, t: f32) -> Self {
        let m = |a: u8, b: u8| {
            let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t.clamp(0.0, 1.0);
            // In 0..=255 by the clamp above.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let v = v.round() as u8;
            v
        };
        Self(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }
}

/// The surface and signal colours of one variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// The page.
    pub bg: Rgb,
    /// A panel on the page.
    pub panel: Rgb,
    /// A panel raised above another: code, the input.
    pub panel_raised: Rgb,
    /// Borders and rules.
    pub line: Rgb,
    /// Body text.
    pub text: Rgb,
    /// Secondary text.
    pub text_muted: Rgb,
    /// The accent: Mnemorph's hologram cyan.
    pub holo: Rgb,
    /// Failures, refusals, danger.
    pub alarm: Rgb,
    /// Attention: the human's gate, warnings.
    pub gold: Rgb,
}

/// The dark variant.
pub const DARK: Palette = Palette {
    bg: Rgb::hex(0x06_0b12),
    panel: Rgb::hex(0x0d_141f),
    panel_raised: Rgb::hex(0x14_2033),
    line: Rgb::hex(0x1f_3346),
    text: Rgb::hex(0xe0_e6ed),
    text_muted: Rgb::hex(0xa1_aab5),
    holo: Rgb::hex(0x00_f2ff),
    alarm: Rgb::hex(0xff_2a7d),
    gold: Rgb::hex(0xf7_d002),
};

/// The light variant.
pub const LIGHT: Palette = Palette {
    bg: Rgb::hex(0xf3_f6fa),
    panel: Rgb::hex(0xff_ffff),
    panel_raised: Rgb::hex(0xe9_eff6),
    line: Rgb::hex(0xc9_d5e2),
    text: Rgb::hex(0x0d_141f),
    text_muted: Rgb::hex(0x46_505c),
    holo: Rgb::hex(0x00_707a),
    alarm: Rgb::hex(0xc0_004c),
    gold: Rgb::hex(0x7a_5c00),
};

/// One colour per race and form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct Races {
    pub summoner: Rgb,
    pub elf: Rgb,
    pub orc: Rgb,
    pub slime: Rgb,
    pub kijin: Rgb,
    pub dark_elf: Rgb,
    pub high_elf: Rgb,
    pub high_orc: Rgb,
    pub named_slime: Rgb,
    pub oni: Rgb,
    pub arch_dark_elf: Rgb,
    pub player: Rgb,
}

/// The races' colours, the same in both variants.
pub const RACES: Races = Races {
    summoner: Rgb::hex(0x3f_a9f5),
    elf: Rgb::hex(0x2f_a36b),
    orc: Rgb::hex(0xc9_822b),
    slime: Rgb::hex(0x19_c3b0),
    kijin: Rgb::hex(0xd7_263d),
    dark_elf: Rgb::hex(0x7b_4fd6),
    high_elf: Rgb::hex(0xf5_e6b8),
    high_orc: Rgb::hex(0xe8_a83c),
    named_slime: Rgb::hex(0xa0_e8f0),
    oni: Rgb::hex(0xe8_b04a),
    arch_dark_elf: Rgb::hex(0xc4_cce4),
    player: Rgb::hex(0xd4_af37),
};

impl Palette {
    /// Every field with its name in the TypeScript source. The destructuring is exhaustive: a
    /// field added here and not named fails to compile.
    #[must_use]
    pub fn named(&self) -> Vec<(&'static str, Rgb)> {
        let Self {
            bg,
            panel,
            panel_raised,
            line,
            text,
            text_muted,
            holo,
            alarm,
            gold,
        } = *self;
        vec![
            ("bg", bg),
            ("panel", panel),
            ("panelRaised", panel_raised),
            ("line", line),
            ("text", text),
            ("textMuted", text_muted),
            ("holo", holo),
            ("alarm", alarm),
            ("gold", gold),
        ]
    }
}

impl Races {
    /// The colour of `race`.
    #[must_use]
    pub fn of(&self, race: Race) -> Rgb {
        match race {
            Race::Summoner => self.summoner,
            Race::Elf => self.elf,
            Race::Orc => self.orc,
            Race::Slime => self.slime,
            Race::Kijin => self.kijin,
            Race::DarkElf => self.dark_elf,
            Race::HighElf => self.high_elf,
            Race::HighOrc => self.high_orc,
            Race::NamedSlime => self.named_slime,
            Race::Oni => self.oni,
            Race::ArchDarkElf => self.arch_dark_elf,
        }
    }

    /// Every race with its name in the TypeScript source, destructured exhaustively as
    /// [`Palette::named`] is.
    #[must_use]
    pub fn named(&self) -> Vec<(&'static str, Rgb)> {
        let Self {
            summoner,
            elf,
            orc,
            slime,
            kijin,
            dark_elf,
            high_elf,
            high_orc,
            named_slime,
            oni,
            arch_dark_elf,
            player,
        } = *self;
        vec![
            ("summoner", summoner),
            ("elf", elf),
            ("orc", orc),
            ("slime", slime),
            ("kijin", kijin),
            ("darkElf", dark_elf),
            ("highElf", high_elf),
            ("highOrc", high_orc),
            ("namedSlime", named_slime),
            ("oni", oni),
            ("archDarkElf", arch_dark_elf),
            ("player", player),
        ]
    }
}

/// Which variant the terminal wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Light text on a dark page.
    Dark,
    /// Dark text on a light page.
    Light,
}

impl Variant {
    /// The variant the human asked for: the flag when it names one, else `MNEMORPH_THEME`, else
    /// the terminal's background as `COLORFGBG` reports it, else dark.
    #[must_use]
    pub fn choose(flag: Option<&str>, env: impl Fn(&str) -> Option<String>) -> Self {
        let one = |w: &str| match w.to_ascii_lowercase().as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        };
        let word = |v: &str| words(v).find_map(one);
        if let Some(v) = flag.and_then(word) {
            return v;
        }
        if let Some(v) = env("MNEMORPH_THEME").as_deref().and_then(word) {
            return v;
        }
        // `COLORFGBG` is "fg;bg": a background of 7 or 15 is a light terminal.
        match env("COLORFGBG")
            .as_deref()
            .and_then(|v| v.rsplit(';').next())
        {
            Some("7" | "15") => Self::Light,
            _ => Self::Dark,
        }
    }
}

/// A colour scheme the terminal app wears: Mnemorph's own HUD, which the board shares, or one
/// built on a four-colour palette — its accent, the human's colour, a soft second and alarm,
/// the page and its panels tinted toward it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    /// Violet, magenta, pink and yellow: the default.
    #[default]
    Neon,
    /// Peach, sand and two oranges.
    Ember,
    /// Deep teal, teal, mint and cream.
    Lagoon,
    /// Navy, blue, sky and ice.
    Harbor,
    /// Orange, gold, cream and a dusk blue.
    Dune,
    /// Teal, amber, orange and red.
    Spice,
    /// Mnemorph's hologram cyan: the board's palette, `packages/theme`.
    Hud,
}

/// The roles a scheme's palette fills.
#[derive(Debug, Clone, Copy)]
struct Roles {
    /// Titles, the spinner, links: the accent.
    accent: Rgb,
    /// The human: their lines, the ask, the gate.
    human: Rgb,
    /// Failures and danger.
    alarm: Rgb,
    /// What the page and its panels lean toward.
    tint: Rgb,
}

impl Scheme {
    /// Every scheme, the default first.
    pub const ALL: [Self; 7] = [
        Self::Neon,
        Self::Ember,
        Self::Lagoon,
        Self::Harbor,
        Self::Dune,
        Self::Spice,
        Self::Hud,
    ];

    /// Its name, as `--theme` and `/theme` take it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Ember => "ember",
            Self::Lagoon => "lagoon",
            Self::Harbor => "harbor",
            Self::Dune => "dune",
            Self::Neon => "neon",
            Self::Spice => "spice",
            Self::Hud => "hud",
        }
    }

    /// The scheme `word` names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        let w = word.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|s| s.word() == w)
    }

    /// The scheme the human asked for: the first scheme the flag names, else `MNEMORPH_THEME`'s,
    /// else the default. Either may also name a variant: `lagoon-light`, `neon dark`.
    #[must_use]
    pub fn choose(flag: Option<&str>, env: impl Fn(&str) -> Option<String>) -> Self {
        let named = |v: &str| words(v).find_map(Self::parse);
        flag.and_then(named)
            .or_else(|| env("MNEMORPH_THEME").as_deref().and_then(named))
            .unwrap_or_default()
    }

    fn roles(self) -> Option<Roles> {
        let r = |accent, human, alarm, tint| Roles {
            accent: Rgb::hex(accent),
            human: Rgb::hex(human),
            alarm: Rgb::hex(alarm),
            tint: Rgb::hex(tint),
        };
        Some(match self {
            // colorhunt fba17e f7d794 ff9a00 ee6200
            Self::Ember => r(0xff_9a00, 0xf7_d794, 0xf2_452e, 0xee_6200),
            // colorhunt 287a74 55a9a0 aeeed3 fff8b0
            Self::Lagoon => r(0xae_eed3, 0xff_f8b0, 0xff_6f61, 0x28_7a74),
            // colorhunt 1b365d 005a9c 60c1e5 a0e0ef
            Self::Harbor => r(0x60_c1e5, 0xa0_e0ef, 0xff_5c7a, 0x1b_365d),
            // colorhunt ff933e ffcb56 ffe9a9 73a5ca
            Self::Dune => r(0xff_933e, 0xff_cb56, 0xe5_484d, 0x73_a5ca),
            // colorhunt 5003c0 ab03a9 ff467a ffd51e
            Self::Neon => r(0xff_467a, 0xff_d51e, 0xff_3b30, 0x50_03c0),
            // colorhunt 309898 ff9f00 f4631e cb0404
            Self::Spice => r(0x3c_b8b8, 0xff_9f00, 0xe8_2a22, 0x30_9898),
            Self::Hud => return None,
        })
    }

    /// Its palette in `variant`.
    #[must_use]
    pub fn palette(self, variant: Variant) -> Palette {
        let base = match variant {
            Variant::Dark => DARK,
            Variant::Light => LIGHT,
        };
        let Some(r) = self.roles() else {
            return base;
        };
        match variant {
            Variant::Dark => Palette {
                bg: base.bg.mix(r.tint, 0.10),
                panel: base.panel.mix(r.tint, 0.13),
                panel_raised: base.panel_raised.mix(r.tint, 0.16),
                line: base.line.mix(r.tint, 0.30),
                text: base.text.mix(r.human, 0.06),
                text_muted: base.text_muted.mix(r.tint, 0.12),
                holo: r.accent,
                alarm: r.alarm,
                gold: r.human,
            },
            Variant::Light => Palette {
                bg: base.bg.mix(r.human, 0.10),
                panel: base.panel.mix(r.human, 0.05),
                panel_raised: base.panel_raised.mix(r.tint, 0.08),
                line: base.line.mix(r.tint, 0.25),
                text: base.text.mix(r.tint, 0.10),
                text_muted: base.text_muted.mix(r.tint, 0.10),
                holo: r.accent.mix(base.text, 0.45),
                alarm: r.alarm.mix(base.text, 0.30),
                gold: r.human.mix(base.text, 0.62),
            },
        }
    }
}

/// Whether every word of `v` is a scheme, a variant or `auto`, and at least one is there.
#[must_use]
pub fn names_a_theme(v: &str) -> bool {
    let mut any = false;
    for w in words(v) {
        any = true;
        let w = w.to_ascii_lowercase();
        if Scheme::parse(&w).is_none() && !matches!(w.as_str(), "dark" | "light" | "auto") {
            return false;
        }
    }
    any
}

/// The words of a theme's name: `lagoon-light`, `neon dark`, `spice:light`.
fn words(v: &str) -> impl Iterator<Item = &str> {
    v.split(|c: char| c == '-' || c == ':' || c == ',' || c.is_whitespace())
        .filter(|w| !w.is_empty())
}

/// The theme: a palette and the roles the blocks read from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Which scheme.
    pub scheme: Scheme,
    /// Which variant.
    pub variant: Variant,
    /// Its palette.
    pub p: Palette,
    /// The races' colours.
    pub races: Races,
}

impl Theme {
    /// The HUD theme of `variant`: Mnemorph's own colours, the board's.
    #[must_use]
    pub fn new(variant: Variant) -> Self {
        Self::of(Scheme::Hud, variant)
    }

    /// The theme of `scheme` in `variant`: its palette, and the human's colour from it.
    #[must_use]
    pub fn of(scheme: Scheme, variant: Variant) -> Self {
        let p = scheme.palette(variant);
        let mut races = RACES;
        if let Some(r) = scheme.roles() {
            // On a light page the human's colour is the palette's, already darkened to read.
            races.player = match variant {
                Variant::Dark => r.human,
                Variant::Light => p.gold,
            };
        }
        Self {
            scheme,
            variant,
            p,
            races,
        }
    }

    /// A race's colour made readable on this variant's page: a light page darkens the bright
    /// race hues toward its text.
    #[must_use]
    pub fn readable(&self, c: Rgb) -> Rgb {
        match self.variant {
            Variant::Dark => c,
            Variant::Light => c.mix(self.p.text, 0.35),
        }
    }

    /// Plain text on the page.
    #[must_use]
    pub fn text(&self) -> Style {
        Style::new().fg(self.p.text.color())
    }

    /// Secondary text.
    #[must_use]
    pub fn muted(&self) -> Style {
        Style::new().fg(self.p.text_muted.color())
    }

    /// The accent.
    #[must_use]
    pub fn holo(&self) -> Style {
        Style::new().fg(self.p.holo.color())
    }

    /// Failure and danger.
    #[must_use]
    pub fn alarm(&self) -> Style {
        Style::new().fg(self.p.alarm.color())
    }

    /// Attention.
    #[must_use]
    pub fn gold(&self) -> Style {
        Style::new().fg(self.p.gold.color())
    }

    /// Success: the elves' green, readable on the page.
    #[must_use]
    pub fn ok(&self) -> Style {
        Style::new().fg(self.readable(self.races.elf).color())
    }

    /// Borders and rules.
    #[must_use]
    pub fn line(&self) -> Style {
        Style::new().fg(self.p.line.color())
    }

    /// The page itself.
    #[must_use]
    pub fn page(&self) -> Style {
        Style::new().bg(self.p.bg.color()).fg(self.p.text.color())
    }

    /// The session body's voice.
    #[must_use]
    pub fn summoner(&self) -> Style {
        Style::new()
            .fg(self.readable(self.races.summoner).color())
            .add_modifier(Modifier::BOLD)
    }

    /// A race's colour, readable on the page.
    #[must_use]
    pub fn race(&self, race: Race) -> Style {
        Style::new().fg(self.readable(self.races.of(race)).color())
    }

    /// A border a little brighter than the rules, for what holds attention without alarm.
    #[must_use]
    pub fn frame(&self) -> Style {
        Style::new().fg(self.p.line.mix(self.p.text_muted, 0.45).color())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_wins_then_the_environment_then_the_terminal() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == k)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        let none = env(&[]);
        assert_eq!(Variant::choose(Some("light"), none), Variant::Light);
        assert_eq!(Variant::choose(Some("auto"), none), Variant::Dark);
        let light_env = env(&[("MNEMORPH_THEME", "light")]);
        assert_eq!(Variant::choose(Some("auto"), light_env), Variant::Light);
        assert_eq!(Variant::choose(Some("dark"), light_env), Variant::Dark);
        let fgbg = env(&[("COLORFGBG", "0;15")]);
        assert_eq!(Variant::choose(None, fgbg), Variant::Light);
    }

    /// WCAG's contrast ratio of two colours.
    fn contrast(a: Rgb, b: Rgb) -> f64 {
        let lum = |c: Rgb| {
            let ch = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.039_28 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * ch(c.0) + 0.7152 * ch(c.1) + 0.0722 * ch(c.2)
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_scheme_reads_on_its_page_in_both_variants() {
        let mut short = Vec::new();
        for s in Scheme::ALL {
            for v in [Variant::Dark, Variant::Light] {
                let t = Theme::of(s, v);
                let p = t.p;
                let mut at = |what: &str, c: Rgb, on: Rgb, min: f64| {
                    let r = contrast(c, on);
                    if r < min {
                        short.push(format!("{} {v:?}: {what} {r:.2} < {min}", s.word()));
                    }
                };
                at("text", p.text, p.bg, 4.5);
                at("text on a panel", p.text, p.panel_raised, 4.5);
                at("muted", p.text_muted, p.bg, 3.0);
                for (what, c) in [("accent", p.holo), ("alarm", p.alarm), ("human", p.gold)] {
                    at(what, c, p.bg, 3.0);
                }
                at("the human's spine", t.readable(t.races.player), p.bg, 3.0);
            }
        }
        assert!(short.is_empty(), "{}", short.join("\n"));
    }

    #[test]
    fn a_scheme_is_named_with_or_without_its_variant() {
        let none = |_: &str| None;
        assert_eq!(Scheme::choose(None, none), Scheme::Neon, "the default");
        assert_eq!(Scheme::choose(Some("lagoon"), none), Scheme::Lagoon);
        assert_eq!(Scheme::choose(Some("neon-light"), none), Scheme::Neon);
        assert_eq!(Variant::choose(Some("neon-light"), none), Variant::Light);
        assert_eq!(Variant::choose(Some("spice dark"), none), Variant::Dark);
        assert_eq!(Scheme::choose(Some("light"), none), Scheme::Neon);
        let env = |k: &str| (k == "MNEMORPH_THEME").then(|| "harbor".to_owned());
        assert_eq!(Scheme::choose(None, env), Scheme::Harbor);
        assert_eq!(Scheme::choose(Some("dune"), env), Scheme::Dune);
        // The HUD is the board's palette, unchanged.
        assert_eq!(Theme::new(Variant::Dark).p, DARK);
        assert_eq!(Theme::of(Scheme::Hud, Variant::Light).p, LIGHT);
        for s in Scheme::ALL {
            assert_eq!(Scheme::parse(s.word()), Some(s));
        }
    }

    #[test]
    fn a_mix_moves_between_two_colours() {
        let a = Rgb(0, 0, 0);
        let b = Rgb(200, 100, 50);
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        assert_eq!(a.mix(b, 0.5), Rgb(100, 50, 25));
        assert_eq!(Rgb::hex(0x00_f2ff).css(), "#00f2ff");
    }
}
