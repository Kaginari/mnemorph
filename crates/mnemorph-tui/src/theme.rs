//! The palette: Mnemorph's colours, the same values as `packages/theme` (a test keeps them
//! equal), and the theme the terminal wears — dark or light — with the roles every block reads.

use ratatui::style::{Color, Modifier, Style};

/// One `#rrggbb` colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub rimuru: Rgb,
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
    pub veldora: Rgb,
}

/// The races' colours, the same in both variants.
pub const RACES: Races = Races {
    rimuru: Rgb::hex(0x3f_a9f5),
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
    veldora: Rgb::hex(0xd4_af37),
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
    /// Every race with its name in the TypeScript source, destructured exhaustively as
    /// [`Palette::named`] is.
    #[must_use]
    pub fn named(&self) -> Vec<(&'static str, Rgb)> {
        let Self {
            rimuru,
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
            veldora,
        } = *self;
        vec![
            ("rimuru", rimuru),
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
            ("veldora", veldora),
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
        let word = |w: &str| match w.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        };
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

/// The theme: a palette and the roles the blocks read from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Which variant.
    pub variant: Variant,
    /// Its palette.
    pub p: Palette,
    /// The races' colours.
    pub races: Races,
}

impl Theme {
    /// The theme of `variant`.
    #[must_use]
    pub fn new(variant: Variant) -> Self {
        let p = match variant {
            Variant::Dark => DARK,
            Variant::Light => LIGHT,
        };
        Self {
            variant,
            p,
            races: RACES,
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
    pub fn rimuru(&self) -> Style {
        Style::new()
            .fg(self.readable(self.races.rimuru).color())
            .add_modifier(Modifier::BOLD)
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
