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
/// Visits of one test bench with no green and nothing landed between them, before the stall.
pub const BENCH_STALL: u32 = 12;
/// Muted visits of one bench — its exit status hidden — with no green, before the stall.
pub const MUTED_STALL: u32 = 8;

/// Words that run a command without being it: stripped before a bench is recognised.
const RUNNERS: &[&str] = &[
    "uv run",
    "poetry run",
    "pnpm exec",
    "npx",
    "bunx",
    "pipenv run",
    "nice",
    "time",
    "env",
];

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
    /// Visits since its last green or the last landing.
    pub since_progress: u32,
    /// Muted visits since its last green: a landing does not reset them.
    pub muted_streak: u32,
    /// Piped runs a sure decision read from their output, green or red, instead of muting them.
    pub read: u32,
}

/// What a decision read a piped bench run's output as, when it was sure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Read {
    /// Every test passed.
    Green,
    /// A test failed.
    Red,
}

/// The bench a command runs, if it runs one: the command before any pipe, trimmed — piped or
/// not, it is the same bench.
#[must_use]
pub fn bench_of(command: &str) -> Option<String> {
    let mut c = command
        .split('|')
        .next()
        .unwrap_or(command)
        .split(';')
        .next()
        .unwrap_or(command)
        .trim();
    loop {
        let before = c;
        if let Some((first, rest)) = c.split_once(' ') {
            // `VAR=value cmd`, `timeout 600 cmd`, `uv run cmd`, …
            if first.contains('=') && !first.starts_with('-') {
                c = rest.trim_start();
            } else if first == "timeout" {
                c = rest
                    .trim_start()
                    .split_once(' ')
                    .map_or(rest, |(_, r)| r)
                    .trim_start();
            } else if let Some(r) = RUNNERS
                .iter()
                .find_map(|w| c.strip_prefix(w).and_then(|r| r.strip_prefix(' ')))
            {
                c = r.trim_start();
            }
        }
        if c == before {
            break;
        }
    }
    if let Some((_, after)) = c.rsplit_once("&& ") {
        c = after;
    }
    if BENCHES.iter().any(|b| c.starts_with(b)) {
        return Some(c.trim().to_owned());
    }
    check_script(c, command)
}

/// Interpreters a verification script runs under, and the flag that passes it inline.
const CHECKERS: &[(&str, &str)] = &[
    ("python3", "-c"),
    ("python", "-c"),
    ("node", "-e"),
    ("bash", "-c"),
    ("sh", "-c"),
];

/// A command that checks work without being a test runner: an inline script that asserts
/// (`python3 -c "…assert…"`), or a script file named for testing, checking or verifying
/// (`python3 check_run.py`, `bash verify.sh`). Bodies often verify this way, and a tally that
/// knew only test runners would read such a run as no test at all. Every inline check of one
/// interpreter is one bench, since its text changes from run to run; a named script is its own.
///
/// `c` is the command cut before its first pipe or `;`; `whole` is the command as given, since an
/// inline script's own `;` is no separator, and its `assert` may come after one.
fn check_script(c: &str, whole: &str) -> Option<String> {
    let mut words = c.split_whitespace();
    let interp = words.next()?;
    let (_, flag) = CHECKERS.iter().find(|(i, _)| *i == interp)?;
    let next = words.next()?;
    if next == *flag {
        return whole
            .contains("assert")
            .then(|| format!("{interp} {flag} (an inline check)"));
    }
    let file = next.rsplit('/').next().unwrap_or(next).to_ascii_lowercase();
    let named = ["test", "check", "verify"].iter().any(|w| file.contains(w));
    let script = [".py", ".js", ".mjs", ".sh"]
        .iter()
        .any(|e| file.ends_with(e));
    (named && script).then(|| format!("{interp} {next}"))
}

/// What the guts read from the last act: the instruments a learned value model trains on.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Reading {
    /// The act's score: +2 for a landed write, −1 for a failed act or an unchanged re-read, else 0.
    pub score: i32,
    /// The guts window's sum after the act, before a warning restarts it.
    pub window: i32,
    /// The guts warned on this act.
    pub warned: bool,
    /// The bench the act ran, if it ran one.
    pub bench: Option<String>,
    /// How the bench run counted: `green`, `red`, `muted`, `read green` or `read red`.
    pub bench_outcome: Option<&'static str>,
}

/// Whether a bench run hides its exit status: piped, or followed by `;` or `||`.
#[must_use]
pub fn muted(command: &str) -> bool {
    command.contains('|') || command.contains(';')
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
    last: Reading,
}

impl Stress {
    /// Read one act. Every landing resets the triggers it relieves, so a relieved run is not
    /// stopped by its own past.
    pub fn act(&mut self, call: &str, command: Option<&str>, o: &Outcome) -> Vec<Signal> {
        self.act_read(call, command, o, None)
    }

    /// [`Self::act`], with what a decision read a piped bench run as: a sure read counts the run
    /// green or red, as if its exit status had been seen; no read leaves it muted.
    pub fn act_read(
        &mut self,
        call: &str,
        command: Option<&str>,
        o: &Outcome,
        read: Option<Read>,
    ) -> Vec<Signal> {
        let mut out = Vec::new();
        let mut reading = Reading::default();
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
        // The bench tally: a run that hides its exit status is muted, never green. A landing
        // resets the honest reds, never the muted runs.
        if !o.meta.wrote.is_empty() {
            for b in self.benches.values_mut() {
                b.since_progress = 0;
            }
        }
        out.extend(self.tally(command, o, read, &mut reading));
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
        reading.score = score;
        reading.window = self.window.iter().sum();
        reading.warned = self.window.len() == GUTS_WINDOW && reading.window < GUTS_FLOOR;
        self.last = reading;
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

    /// The bench tally for one act: a run that hides its exit status is muted, never green, and
    /// a landing resets the honest reds, never the muted runs. The stall, when it comes.
    fn tally(
        &mut self,
        command: Option<&str>,
        o: &Outcome,
        read: Option<Read>,
        reading: &mut Reading,
    ) -> Option<Signal> {
        let bench = command.and_then(bench_of)?;
        let hidden = command.is_some_and(muted);
        reading.bench_outcome = Some(match (hidden, read) {
            (true, Some(Read::Green)) => "read green",
            (true, Some(Read::Red)) => "read red",
            (true, None) => "muted",
            (false, _) if o.is_error => "red",
            (false, _) => "green",
        });
        reading.bench = Some(bench.clone());
        let b = self.benches.entry(bench).or_default();
        b.visits += 1;
        b.since_progress += 1;
        if hidden {
            match read {
                Some(Read::Green) => {
                    b.read += 1;
                    b.green += 1;
                    b.since_progress = 0;
                    b.muted_streak = 0;
                }
                Some(Read::Red) => {
                    b.read += 1;
                    b.red += 1;
                }
                None => {
                    b.muted += 1;
                    b.muted_streak += 1;
                }
            }
        } else if o.is_error {
            b.red += 1;
        } else {
            b.green += 1;
            b.since_progress = 0;
            b.muted_streak = 0;
        }
        (b.since_progress >= BENCH_STALL || b.muted_streak >= MUTED_STALL)
            .then_some(Signal::Stall("bench"))
    }

    /// What the guts read from the last act.
    #[must_use]
    pub fn last(&self) -> &Reading {
        &self.last
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
