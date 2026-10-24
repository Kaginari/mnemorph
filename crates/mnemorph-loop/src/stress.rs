//! The stress instruments: code that reads what the acts did, never the body's mood.

use std::collections::{BTreeMap, VecDeque};

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

/// The same call (tool and input) in a row before a warning, and before the stall.
pub const LOOP_WARN: u32 = 3;
/// See [`LOOP_WARN`].
pub const LOOP_STALL: u32 = 5;
/// Visits of one test bench without a single green before the stall.
pub const BENCH_STALL: u32 = 12;

/// Commands that run a test bench, by their start.
const BENCHES: &[&str] = &[
    "cargo test",
    "cargo nextest",
    "npm test",
    "npm run test",
    "pnpm test",
    "pnpm run test",
    "yarn test",
    "pytest",
    "python -m pytest",
    "python3 -m pytest",
    "go test",
    "just check",
    "just test",
    "make test",
    "vitest",
    "jest",
    "mvn test",
    "gradle test",
    "./gradlew test",
    "bun test",
    "deno test",
    "mix test",
];

/// One bench's record.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Bench {
    /// Times it ran.
    pub visits: u32,
    /// Times it failed.
    pub red: u32,
    /// Times it passed, its exit status seen.
    pub green: u32,
    /// Times it ran piped, its exit status hidden: never counted green.
    pub muted: u32,
}

/// The bench a command runs, if it runs one: the command before any pipe, trimmed — piped or
/// not, it is the same bench.
#[must_use]
pub fn bench_of(command: &str) -> Option<String> {
    let c = command.split('|').next().unwrap_or(command).trim();
    BENCHES
        .iter()
        .any(|b| {
            c.starts_with(b) || c.contains(&format!("&& {b}")) || c.contains(&format!("; {b}"))
        })
        .then(|| c.to_owned())
}

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
    last_call: Option<String>,
    repeats: u32,
    benches: BTreeMap<String, Bench>,
}

impl Stress {
    /// Read one act. Every landing resets the triggers it relieves, so a relieved run is not
    /// stopped by its own past.
    pub fn act(&mut self, call: &str, command: Option<&str>, o: &Outcome) -> Vec<Signal> {
        let mut out = Vec::new();
        // The same call again: its result will not change.
        if self.last_call.as_deref() == Some(call) {
            self.repeats += 1;
        } else {
            self.last_call = Some(call.to_owned());
            self.repeats = 1;
        }
        if self.repeats == LOOP_WARN {
            out.push(Signal::Warn(
                "[loop] The same call, with the same input, three times in a row: its result will not change. Change the approach, or end the turn and say what blocks you.".into(),
            ));
        } else if self.repeats >= LOOP_STALL {
            out.push(Signal::Stall("loop"));
        }
        // The bench tally: a piped run hides its exit status, so it is never green.
        if let Some(bench) = command.and_then(bench_of) {
            let piped = command.is_some_and(|c| c.contains('|'));
            let b = self.benches.entry(bench).or_default();
            b.visits += 1;
            if piped {
                b.muted += 1;
            } else if o.is_error {
                b.red += 1;
            } else {
                b.green += 1;
            }
            if b.green == 0 && b.visits >= BENCH_STALL {
                out.push(Signal::Stall("bench"));
            }
        }
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

    /// Every bench this ask ran, and its record.
    #[must_use]
    pub fn benches(&self) -> &BTreeMap<String, Bench> {
        &self.benches
    }

    /// Count one model turn.
    pub fn turn(&mut self) -> Option<Signal> {
        self.turns += 1;
        (self.turns > ACT_BUDGET).then_some(Signal::Stall("actBudget"))
    }
}
