//! A toast: a word that floats in at the top right, holds, and fades away on the spring — the
//! end gate's `◆ gate pass`, as Isekai floats it. Its fade is a [`Spring`] on its opacity, so a
//! slow frame never makes it flicker; the model ticks while one is on screen.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::slab::{Slab, Surface};
use super::spring::{Motion, Spring};
use super::text::{Lines, cells};
use crate::theme::{Rgb, Theme};

/// How long a toast holds before it fades, in seconds.
pub const HOLD: f64 = 3.0;
/// The fade's spring: about half a second in, and out.
const FADE: Spring = Spring::new(9.0);

/// A toast on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    /// Its words.
    pub text: String,
    /// Its hue: the verdict's.
    pub hue: Rgb,
    /// Its opacity, on the spring.
    fade: Motion,
    /// Seconds since it came.
    age: f64,
}

impl Toast {
    /// A toast of `text` in `hue`, about to fade in.
    #[must_use]
    pub fn new(text: impl Into<String>, hue: Rgb) -> Self {
        Self {
            text: text.into(),
            hue,
            fade: Motion::default(),
            age: 0.0,
        }
    }

    /// `dt` seconds passed.
    pub fn tick(&mut self, dt: f64) {
        self.age += dt;
        let target = if self.age < HOLD { 1.0 } else { 0.0 };
        self.fade = FADE.step(self.fade, target, dt);
    }

    /// How much of it shows, `0..=1`.
    #[must_use]
    pub fn opacity(&self) -> f64 {
        self.fade.pos.clamp(0.0, 1.0)
    }

    /// Whether it is still on screen: it has not yet faded out after its hold.
    #[must_use]
    pub fn alive(&self) -> bool {
        self.age < HOLD || self.fade.pos > 0.02
    }

    /// Its lines, a float faded to its opacity over the page.
    #[must_use]
    pub fn view(&self, t: &Theme) -> Lines {
        // Display only: an opacity of a colour blend.
        #[allow(clippy::cast_possible_truncation)]
        let gone = (1.0 - self.opacity()) as f32;
        let plane = t.p.bg;
        let fade = |c: Rgb| c.mix(plane, gone);
        let mut s = Surface::float(t, Some(self.hue), 0.18);
        s.face = fade(s.face);
        s.band = fade(s.band);
        s.gloss = fade(s.gloss);
        s.shade = fade(s.shade);
        s.shadow = (fade(s.shadow.0), fade(s.shadow.1));
        s.spine = s.spine.map(fade);
        let word = Line::from(Span::styled(
            format!(" {} ", self.text),
            Style::new()
                .fg(fade(t.readable(self.hue)).color())
                .add_modifier(Modifier::BOLD),
        ));
        let w = cells(&self.text) + 2 + super::slab::FRAME;
        Slab::new(s).band(1).render(vec![word], w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_fades_in_holds_and_fades_out() {
        let mut toast = Toast::new("◆ gate pass", Rgb(0x2f, 0xa3, 0x6b));
        assert!(toast.opacity() < 0.01);
        for _ in 0..30 {
            toast.tick(1.0 / 30.0);
        }
        assert!(toast.opacity() > 0.9, "{}", toast.opacity());
        assert!(toast.alive());
        for _ in 0..(30 * 3) {
            toast.tick(1.0 / 30.0);
        }
        assert!(toast.opacity() < 0.5, "fading: {}", toast.opacity());
        for _ in 0..60 {
            toast.tick(1.0 / 30.0);
        }
        assert!(!toast.alive());
    }
}
