//! The Kijin's plan step: every outward act declares its undo, and a standing effect its expiry.
//!
//! The plan is typed, so the line between `outward` and `destructive` is computed from it rather
//! than listed: an outward act with no undo, or a standing effect with no expiry, is
//! `destructive`, and a destructive act is never covered by an Oni envelope — it goes to the
//! human, whatever the envelope says. The classifier's own destructive patterns stay the first
//! reading; the plan can only raise an act's class, never lower it.
//!
//! An undo is itself checked as an act, one level deep: it is not empty, not the act again, not
//! refused by the guard, outward (only an act that reaches outside can put the outside back), and
//! not destructive — its own undo is never asked for, so a destructive undo would be an
//! irreversible act with nothing behind it.

use std::fmt;
use std::path::PathBuf;

use mnemorph_ledger::Day;
use mnemorph_races::{Race, ToolClass};
use mnemorph_tools::{Ask, Place, Verdict, classify};

use crate::{ENVELOPE_DAYS_CEILING, Envelope, Lapse};

/// What the approval card says of an outward act with no undo.
pub const NO_UNDO: &str = "no undo — destructive";

/// What the approval card says of a standing effect with no expiry.
pub const NO_EXPIRY: &str = "standing effect with no expiry — destructive";

/// What the approval card says of a standing effect whose expiry is more than a year out.
pub const FAR_EXPIRY: &str = "standing effect expiring more than a year out — destructive";

/// How an outward act is put back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undo {
    /// A command that rolls the outside back (`gh pr close 12`, `kubectl rollout undo …`).
    Rollback(String),
    /// A command that reverts what the act landed (`git revert` and its push, a counter-commit).
    Revert(String),
    /// Nothing puts it back.
    None,
}

impl Undo {
    /// The undo's command, when there is one.
    #[must_use]
    pub fn command(&self) -> Option<&str> {
        match self {
            Undo::Rollback(c) | Undo::Revert(c) => Some(c),
            Undo::None => None,
        }
    }
}

/// How long an outward act's effect stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Done once and over: a push, a comment, a re-run.
    Once,
    /// It stays in force until something ends it: a deploy, a feature flag, a scheduled job.
    Standing {
        /// The first day it no longer stands; `None` when the plan names none.
        expires: Option<Day>,
    },
}

/// One outward act of a Kijin's plan, as the Kijin declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act {
    /// The command line, as it will run.
    pub command: String,
    /// How it is put back.
    pub undo: Undo,
    /// Whether its effect stands, and until when.
    pub effect: Effect,
}

/// Why a plan's act is refused before it is ever asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Malformed {
    /// The act names no command.
    EmptyAct,
    /// The guard refuses the act itself.
    ActRefused(String),
    /// The undo names no command.
    EmptyUndo,
    /// The undo is the act again.
    SameCommand,
    /// The guard refuses the undo.
    UndoRefused(String),
    /// The undo does not reach outside, so it cannot put the outside back.
    UndoInward(ToolClass),
    /// The undo is destructive, and an undo's own undo is never asked for.
    UndoDestructive,
    /// The standing effect expires on or before the day it is planned.
    Expired(Day),
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Malformed::EmptyAct => f.write_str("the act names no command"),
            Malformed::ActRefused(why) => write!(f, "the guard refuses the act: {why}"),
            Malformed::EmptyUndo => f.write_str(
                "the undo names no command; declare a rollback, a revert, or `none`",
            ),
            Malformed::SameCommand => f.write_str("the undo is the act itself"),
            Malformed::UndoRefused(why) => write!(f, "the guard refuses the undo: {why}"),
            Malformed::UndoInward(class) => write!(
                f,
                "the undo is a {class} act: only an act that reaches outside puts the outside back"
            ),
            Malformed::UndoDestructive => f.write_str(
                "the undo is destructive, and an undo has no undo of its own; declare `none` instead",
            ),
            Malformed::Expired(day) => {
                write!(f, "the standing effect expires on {day}, not after today")
            }
        }
    }
}

impl std::error::Error for Malformed {}

/// An act the plan step has classed. It can only be made by [`plan`], so whatever reads its
/// class reads the plan's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    command: String,
    class: ToolClass,
    why: Vec<String>,
    outside: Vec<PathBuf>,
    undo: Option<String>,
}

impl Planned {
    /// The command, as it will run.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Its class: the classifier's, raised to `destructive` by a missing undo or expiry.
    #[must_use]
    pub fn class(&self) -> ToolClass {
        self.class
    }

    /// Why it has its class; the plan's own reasons first.
    #[must_use]
    pub fn why(&self) -> &[String] {
        &self.why
    }

    /// The undo's command, when it declared one.
    #[must_use]
    pub fn undo(&self) -> Option<&str> {
        self.undo.as_deref()
    }

    /// The question the gate puts to the human: the plan's reasons lead the card, so an act
    /// with no undo reads `no undo — destructive` before anything else.
    #[must_use]
    pub fn ask(&self, body: &str, race: Race) -> Ask {
        let mut why = self.why.clone();
        if let Some(undo) = &self.undo {
            why.push(format!("undo: {undo}"));
        }
        Ask {
            body: body.to_owned(),
            race,
            class: self.class,
            act: self.command.clone(),
            why,
            outside: self.outside.clone(),
        }
    }
}

/// Words of a command line, so spacing alone never makes a different command.
fn words(command: &str) -> Vec<&str> {
    command.split_whitespace().collect()
}

/// Class an act of a Kijin's plan, at `place`, on the day it is planned.
///
/// An `outward` act with [`Undo::None`] is `destructive` ([`NO_UNDO`]); so is a standing effect
/// with no expiry ([`NO_EXPIRY`]) or one more than a year out ([`FAR_EXPIRY`]). A `destructive`
/// act stays destructive whatever it declares. A `read` or `write` act keeps its class: it does
/// not leave the machine, and the attic keeps what a write takes away.
///
/// # Errors
///
/// [`Malformed`] when the act or its undo is empty or refused by the guard, the undo is the act
/// again, does not reach outside or is destructive, or a standing expiry is not after `today`.
pub fn plan(act: &Act, place: &Place, today: Day) -> Result<Planned, Malformed> {
    if act.command.trim().is_empty() {
        return Err(Malformed::EmptyAct);
    }
    let (class, mut reasons, outside) = match classify(&act.command, place) {
        Verdict::Run {
            class,
            why,
            outside,
        } => (class, why, outside),
        Verdict::Refuse { reason, .. } => return Err(Malformed::ActRefused(reason)),
    };
    if !matches!(class, ToolClass::Outward | ToolClass::Destructive) {
        return Ok(Planned {
            command: act.command.clone(),
            class,
            why: reasons,
            outside,
            undo: None,
        });
    }
    let undo = match act.undo.command() {
        Some(command) => Some(check_undo(command, &act.command, place)?),
        None => None,
    };
    let mut plan_reasons = Vec::new();
    if undo.is_none() {
        plan_reasons.push(NO_UNDO.to_owned());
    }
    if let Effect::Standing { expires } = act.effect {
        match expires {
            None => plan_reasons.push(NO_EXPIRY.to_owned()),
            Some(day) if day <= today => return Err(Malformed::Expired(day)),
            Some(day) if day > today.plus(ENVELOPE_DAYS_CEILING) => {
                plan_reasons.push(FAR_EXPIRY.to_owned());
            }
            Some(_) => {}
        }
    }
    let class = if plan_reasons.is_empty() {
        class
    } else {
        ToolClass::Destructive
    };
    plan_reasons.append(&mut reasons);
    Ok(Planned {
        command: act.command.clone(),
        class,
        why: plan_reasons,
        outside,
        undo,
    })
}

/// An undo, checked as an act one level deep.
fn check_undo(undo: &str, act: &str, place: &Place) -> Result<String, Malformed> {
    if undo.trim().is_empty() {
        return Err(Malformed::EmptyUndo);
    }
    if words(undo) == words(act) {
        return Err(Malformed::SameCommand);
    }
    match classify(undo, place) {
        Verdict::Refuse { reason, .. } => Err(Malformed::UndoRefused(reason)),
        Verdict::Run {
            class: ToolClass::Destructive,
            ..
        } => Err(Malformed::UndoDestructive),
        Verdict::Run {
            class: ToolClass::Outward,
            ..
        } => Ok(undo.to_owned()),
        Verdict::Run { class, .. } => Err(Malformed::UndoInward(class)),
    }
}

/// Why an envelope does not cover a planned act.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uncovered {
    /// The act is destructive: it always goes to the human.
    Destructive,
    /// The act does not leave the machine; an envelope covers only outward acts.
    NotOutward(ToolClass),
    /// The envelope is not in force.
    Lapsed(Lapse),
    /// No pattern of the envelope matches the command.
    NoPattern,
}

impl fmt::Display for Uncovered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Uncovered::Destructive => {
                f.write_str("destructive acts always ask, whatever the envelope says")
            }
            Uncovered::NotOutward(class) => {
                write!(f, "a {class} act is not an envelope's to cover")
            }
            Uncovered::Lapsed(lapse) => write!(f, "the envelope is {lapse}"),
            Uncovered::NoPattern => f.write_str("no pattern of the envelope matches the act"),
        }
    }
}

impl Envelope {
    /// Whether the envelope covers a planned act on `today`, after `used` acts it already
    /// covered. A destructive act is refused first, before the envelope's own time, count or
    /// patterns are read.
    ///
    /// # Errors
    ///
    /// The [`Uncovered`] reason: destructive, not outward, lapsed, or matched by no pattern.
    pub fn covers(&self, act: &Planned, today: Day, used: u64) -> Result<(), Uncovered> {
        match act.class() {
            ToolClass::Destructive => return Err(Uncovered::Destructive),
            ToolClass::Outward => {}
            other => return Err(Uncovered::NotOutward(other)),
        }
        self.admits(today, used).map_err(Uncovered::Lapsed)?;
        if self.allow.iter().any(|p| matches(p, act.command())) {
            Ok(())
        } else {
            Err(Uncovered::NoPattern)
        }
    }
}

/// Characters that let the shell run, redirect, expand or glob beyond the words an envelope
/// pattern reads; a command holding one is never matched, and goes to the human.
const UNMATCHED: &[char] = &[
    ';', '&', '|', '<', '>', '$', '`', '(', ')', '{', '}', '\\', '*', '?', '[', ']', '~', '!', '#',
    '\n', '\r',
];

/// An envelope pattern against a command, word by word. A word that is a bare `*` and the
/// pattern's last matches the rest of the command (at least one word); a bare `*` elsewhere
/// matches one word; a word ending in `*` (`feat/*`) matches one word beginning with what comes
/// before it; every other word matches itself.
fn matches(pattern: &str, command: &str) -> bool {
    if command.contains(UNMATCHED) {
        return false;
    }
    let pattern = words(pattern);
    let command = words(command);
    for (i, p) in pattern.iter().enumerate() {
        let last = i + 1 == pattern.len();
        if *p == "*" && last {
            return command.len() > i;
        }
        let Some(word) = command.get(i) else {
            return false;
        };
        let fits = match p.strip_suffix('*') {
            Some("") => true,
            Some(prefix) => word.len() > prefix.len() && word.starts_with(prefix),
            None => p == word,
        };
        if !fits {
            return false;
        }
    }
    command.len() == pattern.len()
}
