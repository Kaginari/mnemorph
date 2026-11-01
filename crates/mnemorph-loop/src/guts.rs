//! The guts classes: what each act was, in isekai's words — `fail`, `red-pin`, `positive`,
//! `repeat`, `echo`, `thrash` ([`Class`]) — read by harness code from the act's structured
//! outcome and the acts before it. An act's text, the model's or a tool's, is never read: [`Act`]
//! carries no output at all, so a body that prints "tests passed" or forges a red test's words
//! classes nothing.
//!
//! A test bench is only a verify line from a trusted source, matched exactly ([`Classifier::new`]):
//! a command a body chose — an inline `assert True`, a script it wrote, `cargo test --no-run` — is
//! a plain command, however it is named, so it earns no `positive` and dodges no `fail`. No source
//! of verify lines out of a body's reach exists yet: a creature's card is in the world. So the
//! harness classes with none, and no act is a bench until one does.
//!
//! The classes are signals for a learned value, journaled on the act's guts beat and kept again
//! outside the world ([`mnemorph_world::guts`]). Nothing here raises or falls a creature, and a
//! learned value keeps the red pin's weight at 0: a red test during TDD is the right process.
//!
//! The classifier spans the run, as isekai's spans its session, and is a pure function of the
//! run's beats: [`replay`] gives a run journal's classes again, the same as the run gave them.

use std::collections::{BTreeMap, HashMap, VecDeque};

pub use mnemorph_world::guts::Class;
use serde_json::Value;

use crate::beats::Beat;

/// How many recent acts the echo remembers.
pub const ECHO_RING: usize = 12;
/// Visits to one read window or command head among the recent acts, before this one, from which a
/// clean visit with no write since the last is an echo: the third visit is the first echo.
pub const ECHO_AFTER: usize = 2;
/// Visits to one trusted bench in a run from which every visit is thrash: the sixth is the first.
pub const THRASH_VISITS: u32 = 6;

/// What one act did, as the harness saw it: the facts the classes read, and nothing a body or a
/// tool wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Act<'a> {
    /// The tool.
    pub tool: &'a str,
    /// Its input, as the model sent it.
    pub input: &'a str,
    /// The act errored: an error result, a non-zero exit, a kill, a refusal.
    pub failed: bool,
    /// The human gate denied it: the human's choice, never the body's failure.
    pub denied: bool,
    /// It wrote a file.
    pub wrote: bool,
    /// A command's exit code; `None` for other tools, or a command killed by a signal.
    pub exit: Option<i32>,
}

impl<'a> Act<'a> {
    /// The act from the facts its result and guts beats journal: `refused` is the rule that
    /// refused it, if one did.
    #[must_use]
    pub fn of(
        tool: &'a str,
        input: &'a str,
        failed: bool,
        refused: Option<&str>,
        wrote: bool,
        exit: Option<i32>,
    ) -> Self {
        Act {
            tool,
            input,
            failed,
            denied: refused == Some("gate"),
            wrote,
            exit,
        }
    }
}

/// A command as a verify line is compared: trimmed, its runs of whitespace one space.
#[must_use]
pub fn normal(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The classifier's memory of a run's acts.
#[derive(Debug, Default, Clone)]
pub struct Classifier {
    /// The verify lines a bench must be, normalised: from a source out of every body's reach.
    trusted: Vec<String>,
    /// The act before: its tool and input.
    prev: Option<String>,
    /// The echo keys of the recent acts, oldest first.
    ring: VecDeque<String>,
    /// The writes counted at each echo key's last visit.
    wrote_at: HashMap<String, u32>,
    /// The run's writes so far.
    writes: u32,
    /// Each trusted bench's visits this run.
    benches: BTreeMap<String, u32>,
}

impl Classifier {
    /// A classifier whose benches are exactly `trusted`, verify lines read from a source no body
    /// can write. [`Classifier::default`] trusts none: no act is a bench.
    #[must_use]
    pub fn new(trusted: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Classifier {
            trusted: trusted
                .into_iter()
                .map(|l| normal(l.as_ref()))
                .filter(|l| !l.is_empty())
                .collect(),
            ..Classifier::default()
        }
    }

    /// The trusted verify line `act` runs, exactly: a `bash` act whose command, normalised, equals
    /// one. No prefix and no substring matches.
    fn bench(&self, act: &Act<'_>) -> Option<String> {
        if act.tool != "bash" || self.trusted.is_empty() {
            return None;
        }
        let command = serde_json::from_str::<Value>(act.input)
            .ok()?
            .get("command")?
            .as_str()
            .map(normal)?;
        self.trusted.contains(&command).then_some(command)
    }

    /// The classes of `act`, in [`Class::ALL`]'s order, remembered for the acts after it.
    pub fn classify(&mut self, act: &Act<'_>) -> Vec<Class> {
        let mut out = Vec::new();
        let bench = self.bench(act);
        // A trusted bench that exited non-zero is TDD's red: the red pin, in place of a fail. A
        // kill shows no exit code, and is a fail.
        let red = bench.is_some() && act.exit.is_some_and(|code| code != 0);
        if red {
            out.push(Class::RedPin);
        } else if act.failed && !act.denied {
            out.push(Class::Fail);
        }
        if bench.is_some() && !act.failed && act.exit == Some(0) {
            out.push(Class::Positive);
        }
        let fingerprint = format!("{}\0{}", act.tool, act.input);
        let repeat = self.prev.as_deref() == Some(fingerprint.as_str());
        if repeat {
            out.push(Class::Repeat);
        }
        self.prev = Some(fingerprint);
        self.writes += u32::from(act.wrote);
        let key = echo_key(act.tool, act.input);
        let visited = self.ring.iter().filter(|k| **k == key).count();
        let nothing_written = self.wrote_at.get(&key) == Some(&self.writes);
        if matches!(act.tool, "bash" | "read")
            && visited >= ECHO_AFTER
            && nothing_written
            && !act.failed
            && !repeat
        {
            out.push(Class::Echo);
        }
        self.wrote_at.insert(key.clone(), self.writes);
        self.ring.push_back(key);
        if self.ring.len() > ECHO_RING {
            self.ring.pop_front();
        }
        if let Some(b) = bench {
            let visits = self.benches.entry(b).or_default();
            *visits += 1;
            if *visits >= THRASH_VISITS {
                out.push(Class::Thrash);
            }
        }
        out.sort_unstable();
        out
    }
}

/// The first two words of `s`.
fn head(s: &str) -> String {
    s.split_whitespace().take(2).collect::<Vec<_>>().join(" ")
}

/// An act's echo key: a read keeps its whole input, so a walk through one file's pages is no echo
/// and the third read of one window is; a command keys on its first two words.
fn echo_key(tool: &str, input: &str) -> String {
    if tool == "read" {
        return format!("read\0{input}");
    }
    let command = serde_json::from_str::<Value>(input)
        .ok()
        .and_then(|v| v.get("command").and_then(Value::as_str).map(head));
    format!("{tool}\0{}", command.unwrap_or_else(|| input.to_owned()))
}

/// The classes a run journal's acts read, in order, each with its tool call's id, with no trusted
/// bench — as the run classes them.
#[must_use]
pub fn replay(beats: &[Beat]) -> Vec<(String, Vec<Class>)> {
    replayed(beats, Classifier::default()).1
}

/// `classifier` run over a run journal's facts: the classifier after the last act — what a resumed
/// run goes on with — and each act's classes, in order, with its tool call's id.
#[must_use]
pub fn replayed(
    beats: &[Beat],
    mut classifier: Classifier,
) -> (Classifier, Vec<(String, Vec<Class>)>) {
    let mut inputs: HashMap<&str, String> = HashMap::new();
    let mut refused: HashMap<&str, Option<&str>> = HashMap::new();
    let mut out = Vec::new();
    for beat in beats {
        match beat {
            Beat::Model { content, .. } => {
                for b in content {
                    if b.get("type").and_then(Value::as_str) == Some("tool_use")
                        && let (Some(id), Some(input)) =
                            (b.get("id").and_then(Value::as_str), b.get("input"))
                    {
                        inputs.insert(id, input.to_string());
                    }
                }
            }
            Beat::Result {
                tool_use_id,
                refused: r,
                ..
            } => {
                refused.insert(tool_use_id, r.as_deref());
            }
            Beat::Guts {
                tool_use_id,
                tool,
                failed,
                wrote,
                exit,
                ..
            } => {
                let input = inputs.get(tool_use_id.as_str()).map_or("", String::as_str);
                let act = Act::of(
                    tool,
                    input,
                    *failed,
                    refused.get(tool_use_id.as_str()).copied().flatten(),
                    *wrote > 0,
                    *exit,
                );
                out.push((tool_use_id.clone(), classifier.classify(&act)));
            }
            _ => {}
        }
    }
    (classifier, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERIFY: &str = "cargo test -p wire";

    fn act<'a>(tool: &'a str, input: &'a str) -> Act<'a> {
        Act {
            tool,
            input,
            failed: false,
            denied: false,
            wrote: false,
            exit: None,
        }
    }

    /// The classes of a `bash` act running `command` that exited `exit`.
    fn classed(c: &mut Classifier, command: &str, exit: Option<i32>) -> Vec<Class> {
        let input = serde_json::json!({ "command": command }).to_string();
        c.classify(&Act {
            failed: exit != Some(0),
            exit,
            ..act("bash", &input)
        })
    }

    fn trusted() -> Classifier {
        Classifier::new([VERIFY])
    }

    #[test]
    fn fail_an_act_that_errored() {
        let a = Act {
            failed: true,
            ..act("edit", r#"{"path":"a"}"#)
        };
        assert_eq!(trusted().classify(&a), [Class::Fail]);
        assert_eq!(trusted().classify(&act("edit", r#"{"path":"a"}"#)), []);
    }

    #[test]
    fn a_human_denial_is_no_fail() {
        let denied = Act::of("bash", "{}", true, Some("gate"), false, None);
        assert_eq!(trusted().classify(&denied), []);
        for rule in ["envelope", "sandbox", "shelf"] {
            let refused = Act::of("bash", "{}", true, Some(rule), false, None);
            assert_eq!(trusted().classify(&refused), [Class::Fail], "{rule}");
        }
    }

    #[test]
    fn red_pin_the_trusted_verify_line_exiting_non_zero() {
        assert_eq!(classed(&mut trusted(), VERIFY, Some(101)), [Class::RedPin]);
        // A bench killed at its deadline showed no exit code: a fail, never a red pin.
        assert_eq!(classed(&mut trusted(), VERIFY, None), [Class::Fail]);
    }

    #[test]
    fn positive_the_trusted_verify_line_exiting_zero() {
        assert_eq!(classed(&mut trusted(), VERIFY, Some(0)), [Class::Positive]);
        // Whitespace is normalised; nothing else is.
        assert_eq!(
            classed(&mut trusted(), "  cargo   test -p\twire ", Some(0)),
            [Class::Positive]
        );
    }

    /// A command the body chose is no bench, however it is named or wherever it starts: exiting 0
    /// it is plain, failing it is a fail — never a red pin.
    #[test]
    fn a_body_chosen_check_is_neither_positive_nor_red_pin() {
        for command in [
            "python3 -c \"assert True\"",
            "python3 -c \"import os; assert os.sep == '?'\"",
            "bash verify.sh",
            "cargo test --no-run",
            "cargo test",
            "pytest tests/test_trivial.py",
            "cargo test -p wire --no-run",
            "cargo test -p wire | tail -3",
            "cd . && cargo test -p wire",
            "cargo test -p wire; true",
            "echo 'test result: ok. 3 passed' && echo 'verify passed'",
        ] {
            assert_eq!(classed(&mut trusted(), command, Some(0)), [], "{command}");
            assert_eq!(
                classed(&mut trusted(), command, Some(1)),
                [Class::Fail],
                "{command}: a fail, never a red pin"
            );
        }
    }

    /// With no trusted source — the harness today — no act is a bench.
    #[test]
    fn no_trusted_source_no_bench() {
        let mut c = Classifier::default();
        assert_eq!(classed(&mut c, VERIFY, Some(0)), []);
        assert_eq!(
            classed(&mut Classifier::default(), VERIFY, Some(1)),
            [Class::Fail]
        );
        assert!(
            Classifier::new([" ", ""]).trusted.is_empty(),
            "a blank line trusts nothing"
        );
    }

    #[test]
    fn repeat_the_same_act_as_the_one_before() {
        let mut c = Classifier::default();
        let a = act("glob", r#"{"pattern":"*.rs"}"#);
        assert_eq!(c.classify(&a), []);
        assert_eq!(c.classify(&a), [Class::Repeat]);
        assert_eq!(c.classify(&act("glob", r#"{"pattern":"*.md"}"#)), []);
        assert_eq!(c.classify(&a), [], "not the act just before");
    }

    #[test]
    fn echo_the_third_clean_visit_with_no_write_since() {
        let mut c = Classifier::default();
        let look = act("bash", r#"{"command":"cat src/a.rs"}"#);
        let other = act("bash", r#"{"command":"ls"}"#);
        assert_eq!(c.classify(&look), []);
        assert_eq!(c.classify(&other), []);
        assert_eq!(c.classify(&look), []);
        assert_eq!(c.classify(&other), []);
        assert_eq!(c.classify(&look), [Class::Echo], "the third look");
        assert_eq!(
            c.classify(&look),
            [Class::Repeat],
            "a repeat, never also an echo"
        );
        // A write between visits is the debug rhythm, never an echo.
        let write = Act {
            wrote: true,
            ..act("write", r#"{"path":"src/a.rs"}"#)
        };
        assert_eq!(c.classify(&write), []);
        assert_eq!(c.classify(&look), []);
        assert_eq!(c.classify(&other), [], "written since its last visit");
        assert_eq!(
            c.classify(&look),
            [Class::Echo],
            "nothing written since its last visit"
        );
        // A walk through one file's pages is not an echo.
        let mut c = Classifier::default();
        for page in 0..6 {
            let input = format!(r#"{{"path":"a","offset":{}}}"#, page * 200);
            assert_eq!(c.classify(&act("read", &input)), [], "page {page}");
        }
    }

    #[test]
    fn echo_is_forgotten_past_the_ring() {
        let mut c = Classifier::default();
        let look = act("bash", r#"{"command":"cat a"}"#);
        c.classify(&look);
        c.classify(&act("bash", r#"{"command":"cat b"}"#));
        c.classify(&look);
        for i in 0..ECHO_RING {
            c.classify(&act("grep", &format!(r#"{{"pattern":"p{i}"}}"#)));
        }
        assert_eq!(c.classify(&look), [], "its visits left the ring");
    }

    #[test]
    fn thrash_from_the_sixth_visit_to_one_trusted_bench() {
        let mut c = trusted();
        let mut seen = Vec::new();
        for _ in 0..8 {
            let edit = Act {
                wrote: true,
                ..act("edit", r#"{"path":"a"}"#)
            };
            c.classify(&edit);
            seen.push(classed(&mut c, VERIFY, Some(101)));
        }
        let thrash: Vec<bool> = seen.iter().map(|s| s.contains(&Class::Thrash)).collect();
        assert_eq!(
            thrash,
            [false, false, false, false, false, true, true, true]
        );
        assert!(seen.iter().all(|s| s.contains(&Class::RedPin)));
        // A command the body chose never counts toward thrash.
        let mut c = trusted();
        for _ in 0..8 {
            assert!(!classed(&mut c, "cargo test", Some(1)).contains(&Class::Thrash));
        }
    }
}
