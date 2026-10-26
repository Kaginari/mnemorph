//! Harmonica for Ratatui: a critically damped spring, the motion of smooth scrolling. It reaches
//! its target as fast as it can without overshooting, from any position and velocity, and its
//! step is exact (a closed form), so a slow frame never makes it wobble.

/// A critically damped spring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// How fast it closes in, in radians per second: higher is snappier.
    pub omega: f64,
}

/// A moving value: where it is and how fast it moves.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Motion {
    /// The position.
    pub pos: f64,
    /// The velocity, per second.
    pub vel: f64,
}

impl Spring {
    /// A spring closing in at `omega` radians per second.
    #[must_use]
    pub const fn new(omega: f64) -> Self {
        Self { omega }
    }

    /// `m` after `dt` seconds pulled toward `target`.
    #[must_use]
    pub fn step(&self, m: Motion, target: f64, dt: f64) -> Motion {
        // x(t) = target + (c1 + c2·t)·e^(−ωt), with c1 = x0 − target and c2 = v0 + ω·c1.
        let w = self.omega;
        let c1 = m.pos - target;
        let c2 = m.vel + w * c1;
        let decay = (-w * dt).exp();
        Motion {
            pos: target + (c1 + c2 * dt) * decay,
            vel: (c2 - w * (c1 + c2 * dt)) * decay,
        }
    }

    /// Whether `m` has come to rest at `target`, within half a cell.
    #[must_use]
    pub fn settled(m: Motion, target: f64) -> bool {
        (m.pos - target).abs() < 0.5 && m.vel.abs() < 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reaches_the_target_without_overshooting() {
        let s = Spring::new(18.0);
        let mut m = Motion { pos: 0.0, vel: 0.0 };
        let mut frames = 0;
        while !Spring::settled(m, 100.0) {
            m = s.step(m, 100.0, 1.0 / 60.0);
            assert!(m.pos <= 100.0 + 1e-9, "overshot: {}", m.pos);
            frames += 1;
            assert!(frames < 120, "never settled");
        }
        assert!(frames > 3, "too abrupt to read as motion: {frames}");
    }

    #[test]
    fn one_long_step_equals_many_short_ones() {
        let s = Spring::new(12.0);
        let start = Motion {
            pos: 3.0,
            vel: -40.0,
        };
        let once = s.step(start, 50.0, 0.2);
        let mut many = start;
        for _ in 0..20 {
            many = s.step(many, 50.0, 0.01);
        }
        assert!((once.pos - many.pos).abs() < 1e-9);
        assert!((once.vel - many.vel).abs() < 1e-9);
    }
}
