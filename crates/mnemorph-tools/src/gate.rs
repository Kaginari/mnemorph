//! The human gate: the one channel consent comes through.
//!
//! An outward or destructive act waits on a [`Pending`] request until the human answers on
//! some channel — the terminal, the board — or the deadline passes, which denies. The first
//! answer wins; a later one is told the request was answered elsewhere. Nothing a model, a
//! tool result or a report says can answer: only a [`Gate`] implementation holds that power,
//! and the harness builds those from channels the human controls.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{BufRead as _, BufReader, Write as _};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use mnemorph_races::{Race, ToolClass};

/// What the harness asks the human.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// The body asking.
    pub race: Race,
    /// The class of the act.
    pub class: ToolClass,
    /// The act itself, as it will run (a command line, a tool call).
    pub act: String,
    /// Why the act has its class: the classifier's reasons.
    pub why: Vec<String>,
    /// Paths outside the world the act writes or deletes: approving opens exactly these.
    pub outside: Vec<std::path::PathBuf>,
}

/// The human's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Run it, this once.
    Approve,
    /// Do not run it; the reason goes back to the body.
    Deny(String),
}

/// A request answered on whichever channel speaks first.
#[derive(Debug, Default)]
pub struct Pending {
    answer: Mutex<Option<Answer>>,
    ready: Condvar,
}

/// A second answer to a request already answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsweredElsewhere;

impl Pending {
    /// A request nobody has answered yet.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Answer it. Only the first answer counts.
    ///
    /// # Errors
    ///
    /// [`AnsweredElsewhere`] when another channel answered first.
    pub fn answer(&self, answer: Answer) -> Result<(), AnsweredElsewhere> {
        let mut slot = self.answer.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_some() {
            return Err(AnsweredElsewhere);
        }
        *slot = Some(answer);
        self.ready.notify_all();
        Ok(())
    }

    /// Wait for the answer until `deadline`; silence denies.
    pub fn wait(&self, deadline: Duration) -> Answer {
        let until = Instant::now() + deadline;
        let mut slot = self.answer.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(a) = slot.clone() {
                return a;
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                drop(slot);
                // A late answer after the deadline finds the slot taken and is told so.
                let deny =
                    Answer::Deny(format!("no answer within {}s: denied", deadline.as_secs()));
                let _ = self.answer(deny.clone());
                return self
                    .answer
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone()
                    .unwrap_or(deny);
            }
            slot = self
                .ready
                .wait_timeout(slot, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// A way to put a question to the human and wait for the answer.
pub trait Gate: Send + Sync {
    /// Ask, and wait.
    fn ask(&self, ask: &Ask) -> Answer;
}

/// Denies everything: the gate of a run with no human attached (headless, a benchmark).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHuman;

impl Gate for NoHuman {
    fn ask(&self, ask: &Ask) -> Answer {
        Answer::Deny(format!(
            "no human is attached to approve a {} act",
            ask.class
        ))
    }
}

/// Asks on the controlling terminal and waits until the deadline; silence denies.
#[derive(Debug, Clone, Copy)]
pub struct Terminal {
    /// How long the human has to answer.
    pub deadline: Duration,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            deadline: Duration::from_secs(300),
        }
    }
}

impl Gate for Terminal {
    fn ask(&self, ask: &Ask) -> Answer {
        let Ok(mut tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") else {
            return Answer::Deny("no terminal to ask on".into());
        };
        let mut card = format!(
            "\n── {} wants to run a {} act ──\n  {}\n",
            ask.race, ask.class, ask.act
        );
        for why in &ask.why {
            let _ = writeln!(card, "  · {why}");
        }
        for path in &ask.outside {
            let _ = writeln!(card, "  ⚠ approving lets it write {}", path.display());
        }
        let _ = write!(
            card,
            "Approve? [y/N] (denied in {}s) ",
            self.deadline.as_secs()
        );
        if tty.write_all(card.as_bytes()).is_err() {
            return Answer::Deny("the terminal could not be written".into());
        }
        let pending = Pending::new();
        let answering = Arc::clone(&pending);
        // A reader still waiting after the deadline answers into a taken slot, and is ignored.
        thread::spawn(move || {
            let mut line = String::new();
            let answer = match BufReader::new(tty).read_line(&mut line) {
                Ok(_) if matches!(line.trim(), "y" | "Y" | "yes") => Answer::Approve,
                _ => Answer::Deny("the human said no".into()),
            };
            let _ = answering.answer(answer);
        });
        pending.wait(self.deadline)
    }
}

/// A scripted gate for tests: answers in order, records every question.
#[derive(Debug, Default)]
pub struct Scripted {
    answers: Mutex<Vec<Answer>>,
    asked: Mutex<Vec<Ask>>,
}

impl Scripted {
    /// A gate that gives `answers` in order, then denies.
    #[must_use]
    pub fn new(answers: impl IntoIterator<Item = Answer>) -> Self {
        let mut answers: Vec<Answer> = answers.into_iter().collect();
        answers.reverse();
        Self {
            answers: Mutex::new(answers),
            asked: Mutex::new(Vec::new()),
        }
    }

    /// Every question asked so far.
    pub fn asked(&self) -> Vec<Ask> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Gate for Scripted {
    fn ask(&self, ask: &Ask) -> Answer {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(ask.clone());
        self.answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop()
            .unwrap_or_else(|| Answer::Deny("the script ran out of answers".into()))
    }
}
