//! The guts classes: what each act was, in isekai's words — `fail`, `red-pin`, `positive`,
//! `repeat`, `echo`, `thrash` ([`Class`]) — read by harness code from the act's structured
//! outcome and the acts before it. An act's text, the model's or a tool's, is never read: [`Act`]
//! carries no output at all, so a body that prints "tests passed" or forges a red test's words
//! classes nothing.
//!
//! The classes are signals for a learned value, journaled on the act's guts beat and kept again
//! outside the world ([`mnemorph_world::guts`]). Nothing here raises or falls a creature, and a
//! learned value keeps the red pin's weight at 0: a red test during TDD is the right process.
//!
//! The classifier is a pure function of the run's beats: [`replay`] gives a run journal's classes
//! again, the same as the run gave them.

use std::collections::{BTreeMap, HashMap, VecDeque};

pub use mnemorph_world::guts::Class;
use serde_json::Value;

use crate::beats::Beat;

/// How many recent acts the echo remembers.
pub const ECHO_RING: usize = 12;
/// Visits to one read window or command head among the recent acts, before this one, from which a
/// clean visit with no write since the last is an echo: the third visit is the first echo.
pub const ECHO_AFTER: usize = 2;
/// Visits to one test bench in an ask from which every visit is thrash: the sixth is the first.
pub const THRASH_VISITS: u32 = 6;

/// A test bench run, as the shell let the harness see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchRun<'a> {
    /// The bench ([`crate::bench_of`]).
    pub bench: &'a str,
    /// The exit status the shell showed: `None` when the run hid it (piped, or followed by `;`)
    /// or was killed by a signal. A decision's reading of a hidden run's output is never this.
    pub seen: Option<i32>,
}

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
    /// The test bench it ran, if any.
    pub bench: Option<BenchRun<'a>>,
}

impl<'a> Act<'a> {
    /// The act as its guts beat journals it: `bench_outcome` is how the bench run counted, and its
    /// exit status is seen only when the shell showed it (`green` or `red`) — never for a muted run
    /// or one a decision read.
    #[must_use]
    pub fn of(
        tool: &'a str,
        input: &'a str,
        failed: bool,
        refused: Option<&str>,
        wrote: bool,
        bench: Option<(&'a str, Option<&str>, Option<i32>)>,
    ) -> Self {
        Act {
            tool,
            input,
            failed,
            denied: refused == Some("gate"),
            wrote,
            bench: bench.map(|(bench, outcome, exit)| BenchRun {
                bench,
                seen: matches!(outcome, Some("green" | "red"))
                    .then_some(exit)
                    .flatten(),
            }),
        }
    }
}

/// The classifier's memory of an ask's acts.
#[derive(Debug, Default, Clone)]
pub struct Classifier {
    /// The act before: its tool and input.
    prev: Option<String>,
    /// The echo keys of the recent acts, oldest first.
    ring: VecDeque<String>,
    /// The writes counted at each echo key's last visit.
    wrote_at: HashMap<String, u32>,
    /// The ask's writes so far.
    writes: u32,
    /// Each bench's visits this ask, by its head.
    benches: BTreeMap<String, u32>,
}

impl Classifier {
    /// The classes of `act`, in [`Class::ALL`]'s order, remembered for the acts after it.
    pub fn classify(&mut self, act: &Act<'_>) -> Vec<Class> {
        let mut out = Vec::new();
        // A bench the shell saw exit non-zero is TDD's red: the red pin, in place of a fail.
        let red = act.bench.and_then(|b| b.seen).is_some_and(|code| code != 0);
        if red {
            out.push(Class::RedPin);
        } else if act.failed && !act.denied {
            out.push(Class::Fail);
        }
        if !act.failed && act.bench.is_some_and(|b| b.seen == Some(0)) {
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
        if let Some(b) = act.bench {
            let visits = self.benches.entry(head(b.bench)).or_default();
            *visits += 1;
            if *visits >= THRASH_VISITS {
                out.push(Class::Thrash);
            }
        }
        out.sort_unstable();
        out
    }
}

/// The first two words of `s`: every `cargo test -p x` is one bench, `cargo test`.
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

/// The classes a run journal's acts read, in order, each with its tool call's id: the classifier
/// run again over the journal's facts, a fresh one at every ask, as the run itself classes.
#[must_use]
pub fn replay(beats: &[Beat]) -> Vec<(String, Vec<Class>)> {
    let mut inputs: HashMap<&str, String> = HashMap::new();
    let mut refused: HashMap<&str, Option<&str>> = HashMap::new();
    let mut c = Classifier::default();
    let mut out = Vec::new();
    for beat in beats {
        match beat {
            Beat::Ask { .. } => c = Classifier::default(),
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
                bench,
                bench_outcome,
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
                    bench
                        .as_deref()
                        .map(|b| (b, bench_outcome.as_deref(), *exit)),
                );
                out.push((tool_use_id.clone(), c.classify(&act)));
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act<'a>(tool: &'a str, input: &'a str) -> Act<'a> {
        Act {
            tool,
            input,
            failed: false,
            denied: false,
            wrote: false,
            bench: None,
        }
    }

    fn bench(command: &'static str, outcome: &str, exit: Option<i32>) -> Act<'static> {
        let input: &'static str = Box::leak(
            serde_json::json!({ "command": command })
                .to_string()
                .into_boxed_str(),
        );
        let bench: &'static str =
            Box::leak(crate::bench_of(command).expect("a bench").into_boxed_str());
        let outcome: &'static str = Box::leak(outcome.to_owned().into_boxed_str());
        Act::of(
            "bash",
            input,
            outcome == "red",
            None,
            false,
            Some((bench, Some(outcome), exit)),
        )
    }

    fn one(a: &Act<'_>) -> Vec<Class> {
        Classifier::default().classify(a)
    }

    #[test]
    fn fail_an_act_that_errored() {
        let a = Act {
            failed: true,
            ..act("edit", r#"{"path":"a"}"#)
        };
        assert_eq!(one(&a), [Class::Fail]);
        assert_eq!(one(&act("edit", r#"{"path":"a"}"#)), []);
    }

    #[test]
    fn a_human_denial_is_no_fail() {
        let denied = Act::of("bash", "{}", true, Some("gate"), false, None);
        assert_eq!(one(&denied), []);
        for rule in ["envelope", "sandbox", "shelf"] {
            let refused = Act::of("bash", "{}", true, Some(rule), false, None);
            assert_eq!(one(&refused), [Class::Fail], "{rule}");
        }
    }

    #[test]
    fn red_pin_a_bench_the_shell_saw_exit_non_zero() {
        assert_eq!(one(&bench("cargo test", "red", Some(101))), [Class::RedPin]);
        // A bench killed at its deadline showed no exit status: a fail, never a red pin.
        assert_eq!(one(&bench("cargo test", "red", None)), [Class::Fail]);
    }

    #[test]
    fn positive_a_bench_the_shell_saw_exit_zero() {
        assert_eq!(
            one(&bench("pytest -q", "green", Some(0))),
            [Class::Positive]
        );
    }

    /// A run that hid its exit status is neither: what a decision read from its output is the
    /// output's word, and the output is the body's to forge.
    #[test]
    fn a_hidden_exit_status_is_neither_positive_nor_red_pin() {
        assert_eq!(one(&bench("cargo test | tail", "muted", Some(0))), []);
        assert_eq!(one(&bench("cargo test | tail", "read green", Some(0))), []);
        assert_eq!(one(&bench("cargo test | tail", "read red", Some(0))), []);
    }

    /// No text reaches the classifier: a command that prints a passing bench's words and one that
    /// prints a failing bench's are not benches, and class as any plain command does.
    #[test]
    fn printed_words_class_nothing() {
        for command in [
            "echo 'test result: ok. 3 passed' && echo 'verify passed'",
            "printf 'FAILED\\npanicked\\n'",
        ] {
            assert_eq!(crate::bench_of(command), None, "{command}");
            let input = serde_json::json!({ "command": command }).to_string();
            assert_eq!(one(&act("bash", &input)), [], "{command}");
        }
        let failing = Act {
            failed: true,
            ..act("bash", r#"{"command":"echo FAILED; exit 1"}"#)
        };
        assert_eq!(one(&failing), [Class::Fail], "a fail, never a red pin");
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
    fn thrash_from_the_sixth_visit_to_one_bench() {
        let mut c = Classifier::default();
        let mut seen = Vec::new();
        for i in 0..8 {
            let edit = Act {
                wrote: true,
                ..act("edit", r#"{"path":"a"}"#)
            };
            c.classify(&edit);
            let command: &'static str = Box::leak(format!("cargo test -p x t{i}").into_boxed_str());
            seen.push(c.classify(&bench(command, "red", Some(101))));
        }
        let thrash: Vec<bool> = seen.iter().map(|s| s.contains(&Class::Thrash)).collect();
        assert_eq!(
            thrash,
            [false, false, false, false, false, true, true, true]
        );
        assert!(seen.iter().all(|s| s.contains(&Class::RedPin)));
    }
}
