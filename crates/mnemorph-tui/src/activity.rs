//! The living status: what the session is doing now, in words, from the moment an ask is sent
//! until its turn ends — the strip above the input (the spinner,
//! the verb in a shimmer, the elapsed time and the turn's tokens, then the body and its phase)
//! and the header's chip (`○ idle`, `● running bash 3s`, `◆ waiting for you`).
//!
//! The phase is read from the model, never stored apart from it: an approval on the card, a call
//! running (its arguments still streaming, or running), the answer streaming, the context past
//! its stress line with the next answer not begun (compacting), else thinking.

use std::time::Duration;

use mnemorph_races::Race;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::app::Model;
use crate::blocks::{elapsed, summary};
use crate::kit::shimmer::shimmer;
use crate::kit::text::{Lines, clip, truncate};
use crate::safe::one_line;

/// The turn as the status keeps it.
#[derive(Debug, Clone, Default)]
pub struct Activity {
    /// When the turn began, on the clock; `None` while idle.
    pub since: Option<Duration>,
    /// The tokens spent before it began: the turn's own are counted from here.
    pub base_tokens: u64,
    /// The turn ended; the next one to start restarts the count.
    pub ended: bool,
    /// The body the portrait last showed: a change of body restarts its loop.
    pub actor: Option<String>,
}

/// What the session is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// The model reasons before it answers.
    Thinking,
    /// The answer streams.
    Writing,
    /// A call's arguments stream.
    Calling(String),
    /// A call runs: its tool and its summary.
    Running(String, String),
    /// An act waits for the human.
    Approval,
    /// The context is past its stress line: the loop folds or summarises before the next turn.
    Compacting,
}

impl Phase {
    /// The phase as the strip's verb: `Thinking…`, `Running bash…`.
    #[must_use]
    pub fn verb(&self) -> String {
        match self {
            Phase::Thinking => "Thinking…".into(),
            Phase::Writing => "Writing…".into(),
            Phase::Calling(tool) => format!("Calling {}…", one_line(tool)),
            Phase::Running(tool, _) => format!("Running {}…", one_line(tool)),
            Phase::Approval => "Waiting for approval…".into(),
            Phase::Compacting => "Compacting…".into(),
        }
    }

    /// The phase in words: `thinking`, `running bash: pytest -q`.
    #[must_use]
    pub fn words(&self) -> String {
        match self {
            Phase::Thinking => "thinking".into(),
            Phase::Writing => "writing".into(),
            Phase::Calling(tool) => format!("calling {}", one_line(tool)),
            Phase::Running(tool, what) if what.is_empty() => format!("running {}", one_line(tool)),
            Phase::Running(tool, what) => format!("running {}: {}", one_line(tool), clip(what, 48)),
            Phase::Approval => "waiting for approval".into(),
            Phase::Compacting => "compacting".into(),
        }
    }
}

/// What `m` is doing now; `None` while idle.
#[must_use]
pub fn phase(m: &Model) -> Option<Phase> {
    if m.gate.is_some() {
        return Some(Phase::Approval);
    }
    if !m.busy() {
        return None;
    }
    let t = &m.transcript;
    if let Some(call) = t.current_call() {
        return Some(if call.input.is_null() {
            Phase::Calling(call.name.clone())
        } else {
            Phase::Running(call.name.clone(), summary(&call.name, &call.input))
        });
    }
    if t.writing() {
        return Some(Phase::Writing);
    }
    let u = &t.usage;
    if !t.thinking_now() && u.stress_line > 0 && u.occupancy >= u.stress_line {
        return Some(Phase::Compacting);
    }
    Some(Phase::Thinking)
}

/// The race of a body by its name: the roster's word, else the race its name begins with.
#[must_use]
pub fn race_of(m: &Model, name: &str) -> Race {
    if name == m.header.body {
        return Race::Rimuru;
    }
    if let Some(r) = m.sidebar.races.get(name) {
        return *r;
    }
    let parts: Vec<&str> = name.split('-').collect();
    (1..=parts.len())
        .rev()
        .find_map(|n| Race::from_name(&parts[..n].join("-")))
        .unwrap_or(Race::Slime)
}

/// The body working now and its race: the newest commissioned body that works, else the
/// session's.
#[must_use]
pub fn actor(m: &Model) -> (String, Race) {
    working(m).map_or_else(
        || (m.header.body.clone(), Race::Rimuru),
        |name| (name.to_owned(), race_of(m, name)),
    )
}

/// The newest commissioned body whose turn runs, while the session's does: one body's event
/// never takes the portrait from a sibling that still works, and the session shows only when
/// none does.
fn working(m: &Model) -> Option<&str> {
    if !m.busy() {
        return None;
    }
    m.sidebar
        .bodies
        .list()
        .iter()
        .rev()
        .find(|b| b.state == crate::sidebar::State::Working)
        .map(|b| b.name.as_str())
}

/// The time the turn has run.
#[must_use]
pub fn running_for(m: &Model) -> Duration {
    m.activity
        .since
        .map_or(Duration::ZERO, |s| m.transcript.clock.saturating_sub(s))
}

/// Tokens, short: `950`, `1.6k`, `12k`, `1.2M`.
#[must_use]
pub fn tokens(n: u64) -> String {
    // Display only: a rounding of a token count.
    #[allow(clippy::cast_precision_loss)]
    let f = n as f64;
    if n < 1000 {
        n.to_string()
    } else if n < 10_000 {
        format!("{:.1}k", f / 1e3)
    } else if n < 1_000_000 {
        format!("{:.0}k", f / 1e3)
    } else {
        format!("{:.1}M", f / 1e6)
    }
}

/// The strip above the input, `w` cells wide: two rows while work runs, none while idle.
#[must_use]
pub fn strip(m: &Model, w: usize) -> Lines {
    let Some(phase) = phase(m) else {
        return Vec::new();
    };
    let t = &m.theme;
    let (who, race) = actor(m);
    let used = m
        .transcript
        .usage
        .tokens
        .saturating_sub(m.activity.base_tokens);
    let mut meta = vec![elapsed(running_for(m))];
    if used > 0 {
        meta.push(format!("{} tokens", tokens(used)));
    }
    meta.push(if m.gate.is_some() {
        "esc denies".into()
    } else {
        "esc to interrupt".into()
    });
    let mut first = vec![
        Span::raw(" "),
        Span::styled(m.spinner.view(), t.holo()),
        Span::raw(" "),
    ];
    first.extend(shimmer(
        &phase.verb(),
        m.frames,
        t.p.text,
        t.p.holo,
        ratatui::style::Style::new().add_modifier(Modifier::BOLD),
    ));
    first.push(Span::styled(format!(" ({})", meta.join(" · ")), t.muted()));
    let mut second = vec![
        Span::raw("   "),
        Span::styled(one_line(&who), t.race(race)),
        Span::styled(" · ", t.frame()),
        Span::styled(doing(m, &phase), t.muted()),
    ];
    let u = &m.transcript.usage;
    if let Some(pct) = u.occupancy.saturating_mul(100).checked_div(u.stress_line)
        && pct >= 75
    {
        second.push(Span::styled(" · ", t.frame()));
        second.push(Span::styled(
            format!("ctx {pct}%"),
            crate::sidebar::band(t, pct),
        ));
    }
    [first, second]
        .into_iter()
        .map(|spans| truncate(Line::from(spans), w))
        .collect()
}

/// What the acting body does, in words: the session's phase, or a commissioned body's tool.
fn doing(m: &Model, phase: &Phase) -> String {
    let Some(name) = working(m) else {
        return phase.words();
    };
    m.sidebar
        .bodies
        .list()
        .iter()
        .rev()
        .find(|b| b.name == name)
        .and_then(|b| b.doing.as_deref())
        .map_or_else(
            || "working".to_owned(),
            |tool| format!("running {}", one_line(tool)),
        )
}

/// The header's chip: idle, working in words with the time, or waiting for the human.
#[must_use]
pub fn chip(m: &Model) -> Vec<Span<'static>> {
    let t = &m.theme;
    match phase(m) {
        None => vec![Span::styled("○ idle", t.muted())],
        Some(Phase::Approval) => vec![Span::styled(
            "◆ waiting for you",
            t.gold().add_modifier(Modifier::BOLD),
        )],
        Some(p) => {
            let words = match &p {
                Phase::Running(tool, _) => format!("running {}", one_line(tool)),
                other => other.words(),
            };
            vec![
                Span::styled(format!("● {}", clip(&words, 24)), t.holo()),
                Span::styled(format!(" {}", elapsed(running_for(m))), t.muted()),
            ]
        }
    }
}
