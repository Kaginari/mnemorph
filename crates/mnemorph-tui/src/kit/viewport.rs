//! A viewport over a tall block: an offset counted from the bottom, so new lines at the end keep
//! a following viewport at the tail; scrolling moves on a spring.

use super::spring::{Motion, Spring};

/// What moves a viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewportMsg {
    /// Up by some lines, away from the tail.
    Up(usize),
    /// Down by some lines, toward the tail.
    Down(usize),
    /// To the very top.
    Top,
    /// Back to the tail, following it again.
    Bottom,
    /// Time passed, in seconds: the spring moves.
    Tick(f64),
}

/// The viewport's state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    target: f64,
    motion: Motion,
    spring: Spring,
    /// The most it may scroll: the block's height less the window's, as the last view saw it.
    max: usize,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            target: 0.0,
            motion: Motion::default(),
            spring: Spring::new(22.0),
            max: usize::MAX,
        }
    }
}

impl Viewport {
    /// Apply a message.
    pub fn update(&mut self, msg: ViewportMsg) {
        let max = to_f64(self.max);
        match msg {
            ViewportMsg::Up(n) => self.target = (self.target + to_f64(n)).min(max),
            ViewportMsg::Down(n) => self.target = (self.target - to_f64(n)).max(0.0),
            ViewportMsg::Top => self.target = max,
            ViewportMsg::Bottom => self.target = 0.0,
            ViewportMsg::Tick(dt) => {
                self.motion = self.spring.step(self.motion, self.target, dt);
                if Spring::settled(self.motion, self.target) {
                    self.motion = Motion {
                        pos: self.target,
                        vel: 0.0,
                    };
                }
            }
        }
    }

    /// Lines up from the tail the window shows, now.
    #[must_use]
    pub fn offset(&self) -> usize {
        // Never negative, and capped by `max`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let at = self.motion.pos.round().max(0.0) as usize;
        at.min(self.max)
    }

    /// Whether the window follows the tail.
    #[must_use]
    pub fn following(&self) -> bool {
        self.target <= 0.0
    }

    /// Whether the spring still moves: the loop keeps ticking while it does.
    #[must_use]
    pub fn animating(&self) -> bool {
        (self.motion.pos - self.target).abs() > f64::EPSILON || self.motion.vel != 0.0
    }

    /// Tell the viewport how far it may scroll, once the block has been measured.
    pub fn set_max(&mut self, max: usize) {
        self.max = max;
        let m = to_f64(max);
        if self.target > m {
            self.target = m;
        }
        if self.motion.pos > m {
            self.motion.pos = m;
        }
    }
}

fn to_f64(n: usize) -> f64 {
    // Line counts stay far below 2^52.
    #[allow(clippy::cast_precision_loss)]
    let v = n as f64;
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_scrolls_smoothly_and_comes_back_to_the_tail() {
        let mut v = Viewport::default();
        v.set_max(100);
        assert!(v.following());
        v.update(ViewportMsg::Up(20));
        assert!(!v.following());
        assert_eq!(v.offset(), 0, "the spring has not moved yet");
        v.update(ViewportMsg::Tick(0.016));
        let first = v.offset();
        assert!(first > 0 && first < 20, "{first}");
        for _ in 0..60 {
            v.update(ViewportMsg::Tick(0.016));
        }
        assert_eq!(v.offset(), 20);
        assert!(!v.animating());
        v.update(ViewportMsg::Up(500));
        for _ in 0..60 {
            v.update(ViewportMsg::Tick(0.016));
        }
        assert_eq!(v.offset(), 100, "capped at the top");
        v.update(ViewportMsg::Bottom);
        assert!(v.following());
    }
}
