//! The stress instruments: code that reads what the acts did, never the body's mood.

use std::collections::VecDeque;

use mnemorph_tools::Outcome;

/// Acts in the guts window.
pub const GUTS_WINDOW: usize = 5;
/// The window's score below which guts warns.
pub const GUTS_FLOOR: i32 = -2;
/// Guts warnings without a landing before the doom checkpoint.
pub const DOOM_AFTER: u32 = 3;
/// Consecutive failed acts that end the turn in escalation (more than this many).
pub const ESCALATE_AFTER: u32 = 2;
/// Re-reads of unchanged windows, with nothing landed, before the stall checkpoint.
pub const REPIN: u32 = 3;
/// Turns with nothing landed before the stall checkpoint.
pub const ACT_BUDGET: u32 = 40;

/// What an instrument says after an act or a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// Guts: a warning the body reads inside its tool result.
    Warn(String),
    /// Guts warned too often: the run stops.
    Doom,
    /// Too many failed acts in a row: the turn ends one hop up.
    Escalate,
    /// Busy, not working: the run stops (`repin` or `actBudget`).
    Stall(&'static str),
}

/// The instruments' readings for one run.
#[derive(Debug, Default, Clone)]
pub struct Stress {
    window: VecDeque<i32>,
    warnings: u32,
    failed_streak: u32,
    rereads: u32,
    turns: u32,
}

impl Stress {
    /// Read one act. Every landing resets the triggers it relieves, so a relieved run is not
    /// stopped by its own past.
    pub fn act(&mut self, o: &Outcome) -> Vec<Signal> {
        let mut out = Vec::new();
        let landed = !o.meta.wrote.is_empty();
        let score = if o.is_error || o.meta.unchanged {
            -1
        } else if landed {
            2
        } else {
            0
        };
        if landed {
            self.warnings = 0;
            self.rereads = 0;
            self.turns = 0;
        }
        self.failed_streak = if o.is_error {
            self.failed_streak + 1
        } else {
            0
        };
        if self.failed_streak > ESCALATE_AFTER {
            out.push(Signal::Escalate);
        }
        if o.meta.unchanged {
            self.rereads += 1;
            if self.rereads >= REPIN {
                out.push(Signal::Stall("repin"));
            }
        }
        self.window.push_back(score);
        if self.window.len() > GUTS_WINDOW {
            self.window.pop_front();
        }
        if self.window.len() == GUTS_WINDOW && self.window.iter().sum::<i32>() < GUTS_FLOOR {
            // A new window starts after each warning: three warnings are three windows.
            self.window.clear();
            self.warnings += 1;
            if self.warnings >= DOOM_AFTER {
                out.push(Signal::Doom);
            } else {
                out.push(Signal::Warn(
                    "[guts] the last acts are not landing: failures and re-reads. Stop, re-read the ask, and change approach — or end the turn and say what blocks you."
                        .into(),
                ));
            }
        }
        out
    }

    /// Count one model turn.
    pub fn turn(&mut self) -> Option<Signal> {
        self.turns += 1;
        (self.turns > ACT_BUDGET).then_some(Signal::Stall("actBudget"))
    }
}
