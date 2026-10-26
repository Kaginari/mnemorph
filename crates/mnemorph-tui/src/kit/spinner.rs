//! A spinner: frames that turn while work runs, advanced by the model's tick.

/// The braille dots, the frames Charm's spinner turns by default.
pub const DOTS: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How long one frame shows, in milliseconds.
pub const FRAME_MS: u64 = 80;

/// A spinner's state: which frame shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spinner {
    frame: usize,
}

impl Spinner {
    /// The next frame.
    pub fn update(&mut self) {
        self.frame = (self.frame + 1) % DOTS.len();
    }

    /// The frame on show.
    #[must_use]
    pub fn view(&self) -> &'static str {
        DOTS[self.frame]
    }

    /// The frame's number, for a cache that must redraw when it turns.
    #[must_use]
    pub fn frame(&self) -> usize {
        self.frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_turns_and_wraps() {
        let mut s = Spinner::default();
        assert_eq!(s.view(), "⠋");
        for _ in 0..DOTS.len() {
            s.update();
        }
        assert_eq!(s.view(), "⠋");
        s.update();
        assert_eq!(s.view(), "⠙");
    }
}
