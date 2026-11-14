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
/// Re-reads of unchanged windows back to back, with nothing landed, before the stall checkpoint:
/// any other act between them is work, and exploring a large codebase re-reads as it goes.
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
/// not, it is the same bench — and the folder it first moves to, when it does: a check run in
/// another folder checks another thing.
#[must_use]
pub fn bench_of(command: &str) -> Option<String> {
    let bench = bench_here(command)?;
    Some(match folder_of(command) {
        Some(dir) => format!("{bench} in {dir}"),
        None => bench,
    })
}

/// The folder a command moves to first, `cd <dir> && …`, written one way: no leading `./`, no
/// trailing `/`. None when it does not move, or moves to `.`.
fn folder_of(command: &str) -> Option<String> {
    let rest = command.trim_start().strip_prefix("cd ")?;
    let (dir, _) = rest.split_once("&&")?;
    let dir = dir.trim().trim_matches(|c| c == '"' || c == '\'');
    let dir = dir.strip_prefix("./").unwrap_or(dir).trim_end_matches('/');
    (!dir.is_empty() && dir != "." && !dir.contains(char::is_whitespace)).then(|| dir.to_owned())
}

/// [`bench_of`] without the folder.
fn bench_here(command: &str) -> Option<String> {
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
/// (`python3 -c "…assert…"`, or a heredoc `python3 - <<'EOF'`), or a script file named for
/// testing, checking or verifying (`python3 check_run.py`, `bash verify.sh`). Bodies often verify
/// this way, and a tally that knew only test runners would read such a run as no test at all.
/// Every inline check of one interpreter is one bench, since its text changes from run to run; a
/// named script is its own.
///
/// `c` is the command cut before its first pipe or `;`; `whole` is the command as given, since an
/// inline script's own `;` is no separator, and its `assert` may come after one.
fn check_script(c: &str, whole: &str) -> Option<String> {
    let mut words = c.split_whitespace();
    let interp = words.next()?;
    let (_, flag) = CHECKERS.iter().find(|(i, _)| *i == interp)?;
    let next = words.next()?;
    if next == *flag || next == "-" || next.starts_with("<<") {
        let (script, _) = inline_script(whole)?;
        return asserts(script).then(|| format!("{interp} (an inline check)"));
    }
    let file = next.rsplit('/').next().unwrap_or(next).to_ascii_lowercase();
    let named = ["test", "check", "verify"].iter().any(|w| file.contains(w));
    let script = [".py", ".js", ".mjs", ".sh"]
        .iter()
        .any(|e| file.ends_with(e));
    (named && script).then(|| format!("{interp} {next}"))
}

/// The script of an inline check in `command`, and the command with the script taken out: the
/// quoted argument of `-c` or `-e`, or a heredoc's body (`python3 - <<'EOF'`, `node <<EOF`).
fn inline_script(command: &str) -> Option<(&str, String)> {
    for (interp, flag) in CHECKERS {
        for (at, _) in command.match_indices(interp) {
            // The interpreter as a word of its own: not the `sh` of `bash`, nor `python` of `python3`.
            let starts = command[..at]
                .chars()
                .last()
                .is_none_or(|c| c.is_whitespace() || "&;|(".contains(c));
            let after = &command[at + interp.len()..];
            if !starts || !after.starts_with(' ') {
                continue;
            }
            let rest = after.trim_start();
            if let Some(arg) = rest.strip_prefix(flag).and_then(|r| r.strip_prefix(' ')) {
                let arg = arg.trim_start();
                let Some(q) = arg.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                    continue;
                };
                let open = command.len() - arg.len();
                let body = &command[open + 1..];
                let end = closing(body, q)?;
                let outside = format!("{}{}", &command[..open], &command[open + 2 + end..]);
                return Some((&body[..end], outside));
            }
            if rest.starts_with("- <<") || rest.starts_with("<<") {
                let line_end = at + command[at..].find('\n')?;
                return Some((&command[line_end + 1..], command[..line_end].to_owned()));
            }
        }
    }
    None
}

/// Where the quote `q` that opened `body` closes; a `"` escaped with a backslash does not close.
fn closing(body: &str, q: char) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in body.char_indices() {
        match c {
            '\\' if q == '"' && !escaped => escaped = true,
            c if c == q && !escaped => return Some(i),
            _ => escaped = false,
        }
    }
    None
}

/// The script asserts: `assert` as a word, followed by a space, `(` or `.` (`assert x`,
/// `assert(x)`, `assert.equal`), never `assert` inside a longer name or a quoted word.
fn asserts(script: &str) -> bool {
    script.match_indices("assert").any(|(i, w)| {
        let before = script[..i]
            .chars()
            .last()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '\'' || c == '"'));
        let after = script[i + w.len()..].chars().next();
        before && matches!(after, Some(' ' | '(' | '.' | '\n'))
    })
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
    // An inline check's own `|` and `;` are its script's, not the shell's: its exit status is the
    // script's, hidden only by what comes outside it.
    let outside = inline_script(command).map(|(_, rest)| rest);
    let c = outside.as_deref().unwrap_or(command);
    c.contains('|') || c.contains(';')
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
    /// Whether this turn has counted a failure already: a turn is one choice of the model's, so
    /// three refusals in one turn are one failed act in the streak, not three.
    failed_in_turn: bool,
    /// Whether the guts has warned in this turn already: the calls of a turn were all sent before
    /// the model read any warning, so a turn is one warning at most toward the doom checkpoint.
    warned_in_turn: bool,
    failed: u32,
    rereads: u32,
    turns: u32,
    last_call: Option<String>,
    repeats: u32,
    benches: BTreeMap<String, Bench>,
    last: Reading,
    /// Turns with nothing landed before `actBudget` stalls the run; 0 reads as [`ACT_BUDGET`].
    act_budget: u32,
}

impl Stress {
    /// The run's failed acts, all told.
    #[must_use]
    pub fn failed(&self) -> u32 {
        self.failed
    }

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
        // A command that ran to its end and answered no (an empty `grep`, a probe that raises, a
        // red test) is the body at work, not a failure: the guts reads it as the streak does.
        let score = if failed_act(o) || o.meta.unchanged {
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
        // A denial is the human's choice: it never strains the body.
        self.failed += u32::from(o.is_error && !o.meta.denied);
        // The streak counts turns that failed, not results: three orders refused in one turn
        // ended an Elf before it had read the first refusal. And it counts acts that failed, not
        // commands that answered no: a probe that exits 1 is a result ([`failed_act`]).
        if failed_act(o) {
            if !self.failed_in_turn {
                self.failed_streak += 1;
                self.failed_in_turn = true;
            }
        } else {
            self.failed_streak = 0;
            self.failed_in_turn = false;
        }
        if self.failed_streak > ESCALATE_AFTER {
            out.push(Signal::Escalate);
            // A new streak starts: a body with no one to hand the turn to is warned every third
            // failure in a row, not at each one.
            self.failed_streak = 0;
        }
        if o.meta.unchanged {
            self.rereads += 1;
            if self.rereads >= REPIN {
                out.push(Signal::Stall("repin"));
            }
        } else {
            self.rereads = 0;
        }
        self.window.push_back(score);
        if self.window.len() > GUTS_WINDOW {
            self.window.pop_front();
        }
        reading.score = score;
        reading.window = self.window.iter().sum();
        let low = self.window.len() == GUTS_WINDOW && reading.window < GUTS_FLOOR;
        // A window that closes low after this turn has warned is not a second warning: the model
        // has not read the first. It starts a new window all the same.
        reading.warned = low && !self.warned_in_turn;
        self.last = reading;
        if low {
            // A new window starts after each warning: three warnings are three windows.
            self.window.clear();
        }
        if low && !self.warned_in_turn {
            self.warned_in_turn = true;
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

    /// The instruments for a run of `steps` steps: `actBudget` never stalls it before its steps
    /// could be spent — a run given more steps than [`ACT_BUDGET`] may use them before landing.
    #[must_use]
    pub fn for_steps(steps: u32) -> Self {
        Self {
            act_budget: steps.max(ACT_BUDGET),
            ..Self::default()
        }
    }

    /// Count one model turn.
    pub fn turn(&mut self) -> Option<Signal> {
        self.turns += 1;
        self.failed_in_turn = false;
        self.warned_in_turn = false;
        let budget = if self.act_budget == 0 {
            ACT_BUDGET
        } else {
            self.act_budget
        };
        (self.turns > budget).then_some(Signal::Stall("actBudget"))
    }
}

/// Whether an act failed, for the failed-turns streak: a refusal, a tool error, or a command
/// killed at its limit or by a signal. A command that ran to its end and exited non-zero is a
/// result — an empty `grep`, a red test, a probe that says no — and a body that reads three of
/// them in a row is working, not stuck: that rule ended a fifth of members' orders, most often on
/// a plain `exit 1`. The guts' score reads it the same way: such a command scores 0, not -1. It
/// was 51% of what scored -1 in the guts' warnings over 614 kept journals.
#[must_use]
pub fn failed_act(o: &Outcome) -> bool {
    let answered_no = o.meta.exit.is_some_and(|code| code != 0) && !o.meta.timed_out;
    o.is_error && !answered_no
}

#[cfg(test)]
mod tests {
    use super::{REPIN, Signal, Stress};
    use mnemorph_races::ToolClass;
    use mnemorph_tools::Outcome;

    fn reread() -> Outcome {
        let mut o = Outcome::ok("[unchanged since your last read]", ToolClass::Read);
        o.meta.unchanged = true;
        o
    }

    fn stalled(signals: &[Signal]) -> bool {
        signals.iter().any(|s| matches!(s, Signal::Stall("repin")))
    }

    /// A turn is one choice: three failures in one turn count once in the streak, and only three
    /// failed turns in a row escalate.
    #[test]
    fn failures_in_one_turn_count_once() {
        let mut s = Stress::default();
        let refused = || Outcome::error("refused: open a plan item that names orc-front");
        let escalated = |v: &[Signal]| v.iter().any(|x| matches!(x, Signal::Escalate));
        let mut signals = Vec::new();
        for _ in 0..3 {
            signals.extend(s.act("commission", None, &refused()));
        }
        assert!(
            !escalated(&signals),
            "one turn's three refusals: {signals:?}"
        );
        for _ in 0..2 {
            s.turn();
            signals.extend(s.act("commission", None, &refused()));
        }
        assert!(
            escalated(&signals),
            "the third failed turn in a row escalates"
        );
    }

    /// A command that ran and exited non-zero is a result, not a failed act: a probe that finds
    /// nothing, a test that is red. Three turns of them in a row end no order, and the guts scores
    /// them 0. A refusal, a tool error and a command killed at its limit still count, and score -1.
    #[test]
    fn a_command_that_exited_non_zero_is_a_result_not_a_failed_act() {
        let escalated = |v: &[Signal]| v.iter().any(|x| matches!(x, Signal::Escalate));
        let exited = |code: i32| {
            let mut o = Outcome::error("no match");
            o.meta.exit = Some(code);
            o
        };
        let killed = || {
            let mut o = Outcome::error("killed after 120s");
            o.meta.timed_out = true;
            o
        };
        // Three turns of a probe that exits 1, each a different command.
        let mut s = Stress::default();
        let mut signals = Vec::new();
        for i in 0..4 {
            s.turn();
            signals.extend(s.act(&format!("bash grep {i}"), None, &exited(1)));
            assert_eq!(
                s.last().score,
                0,
                "a command that answered no is no failure"
            );
        }
        assert!(!escalated(&signals), "{signals:?}");
        // Three turns of a command killed at its limit: failed acts.
        let mut s = Stress::default();
        let mut signals = Vec::new();
        for i in 0..3 {
            s.turn();
            signals.extend(s.act(&format!("bash sleep {i}"), None, &killed()));
            assert_eq!(s.last().score, -1, "a kill scores as a failure");
        }
        assert!(escalated(&signals), "a kill counts: {signals:?}");
        // A refusal has no exit code: three turns of them still escalate, and a red test between
        // two refusals is no failure, so the three are not in a row.
        let refused = || Outcome::error("refused: outside your soil");
        let mut s = Stress::default();
        let mut signals = Vec::new();
        for (i, o) in [refused(), refused(), exited(1), refused()]
            .iter()
            .enumerate()
        {
            s.turn();
            signals.extend(s.act(&format!("call {i}"), None, o));
        }
        assert!(!escalated(&signals), "{signals:?}");
        s.turn();
        signals.extend(s.act("call 4", None, &refused()));
        s.turn();
        signals.extend(s.act("call 5", None, &refused()));
        assert!(escalated(&signals), "three refusals in a row: {signals:?}");
    }

    /// The body at work probes: `python -c` that raises, a `grep` that finds nothing, a red test.
    /// Five such commands in a row are no guts warning. Over the 614 kept journals that read
    /// 51% of what sat in a warning's window, and scored 0 the same replay gives 93 warnings for
    /// 240. Five refusals in a row still warn.
    #[test]
    fn probes_that_answer_no_never_warn_the_guts() {
        let warned = |v: &[Signal]| v.iter().any(|x| matches!(x, Signal::Warn(_)));
        let mut s = Stress::default();
        let mut signals = Vec::new();
        for i in 0..10 {
            s.turn();
            let mut o = Outcome::error("Traceback (most recent call last): ...");
            o.meta.exit = Some(1);
            signals.extend(s.act(&format!("bash python -c probe {i}"), None, &o));
        }
        assert!(!warned(&signals), "{signals:?}");
        let mut s = Stress::default();
        let mut signals = Vec::new();
        for i in 0..5 {
            s.turn();
            signals.extend(s.act(
                &format!("call {i}"),
                None,
                &Outcome::error("refused: outside your soil"),
            ));
        }
        assert!(warned(&signals), "refusals still warn: {signals:?}");
    }

    /// Exploring re-reads as it goes: re-reads with other work between them never stall; only
    /// [`REPIN`] back to back do.
    #[test]
    fn repin_counts_re_reads_back_to_back() {
        let mut s = Stress::default();
        for i in 0..10 {
            assert!(
                !stalled(&s.act("read a.rs", None, &reread())),
                "re-read {i}"
            );
            let fresh = Outcome::ok(format!("lines of b{i}.rs"), ToolClass::Read);
            assert!(!stalled(&s.act(&format!("read b{i}.rs"), None, &fresh)));
        }
        let mut s = Stress::default();
        let signals: Vec<bool> = (0..REPIN)
            .map(|i| stalled(&s.act(&format!("read c{i}.rs"), None, &reread())))
            .collect();
        assert_eq!(signals.last(), Some(&true), "{REPIN} back to back stall");
    }
}
