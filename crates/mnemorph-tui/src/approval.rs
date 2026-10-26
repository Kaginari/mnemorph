//! The approval card's state and drawing: the human sees the whole act before it can be
//! approved, a key typed before the card was on screen answers nothing, and the card leaves at
//! the gate's deadline.
//!
//! - The act scrolls inside the card: a cut act shows its head, the cut marked, and its last
//!   line. `y` is refused until every row of the act has been drawn on screen, read in order: a
//!   jump past rows never shown does not count them.
//! - Keys are ignored until [`ARM`] after the card was first drawn — keys stamped earlier too, so
//!   a line typed ahead never reaches it. Any key that is not `y`, `n`, Enter or a scroll — a
//!   paste too — clears the choice and arms the card again, so a word typed into it (`okay`,
//!   `deploy`) never ends in an answer. Enter decides only [`CONFIRM`] after its `y` or `n`. Esc
//!   denies at once.

use std::cell::Cell;
use std::time::{Duration, Instant};

use mnemorph_loop::GateAnswer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::gate::Card;
use crate::kit::style::{Border, Sides, Style as Boxed};
use crate::kit::text::{Lines, clip, truncate, wrap_text};
use crate::theme::Theme;

/// How long the card ignores keys once it is on screen.
pub const ARM: Duration = Duration::from_millis(300);
/// How long after its `y` or `n` an Enter may confirm it: a `y` and a carriage return that come
/// in one read are typing, not a decision.
pub const CONFIRM: Duration = Duration::from_millis(150);
/// The most rows of the act the card shows at once.
pub const ACT_ROWS: usize = 6;

/// An approval on screen.
#[derive(Debug)]
pub struct Approval {
    /// The act and its answer.
    pub card: Card,
    /// `y` (true) or `n` (false), chosen and not yet confirmed.
    pub choice: Option<bool>,
    /// The first act row in view.
    pub scroll: usize,
    /// When the choice was made.
    chosen_at: Option<Instant>,
    /// `y` was pressed before the act was read to its end.
    pub unread: bool,
    /// When the card was first drawn, or last armed again by a key that answers nothing.
    pub(crate) armed: Cell<Option<Instant>>,
    /// The act rows drawn on screen so far, from the first, without a gap.
    seen_to: Cell<usize>,
    /// The width the act was wrapped at when those rows were drawn.
    seen_width: Cell<usize>,
    /// The act's rows, as last drawn.
    total: Cell<usize>,
}

/// What a key did to the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Nothing decided yet.
    Open,
    /// The human answered.
    Answered(GateAnswer),
}

/// The act's text width inside a card `w` cells wide.
fn act_width(w: usize) -> usize {
    w.saturating_sub(6).max(8)
}

/// The rows of the act shown in a frame whose main area is `h` rows tall.
#[must_use]
pub fn act_rows(h: usize) -> usize {
    h.saturating_sub(9).clamp(1, ACT_ROWS)
}

impl Approval {
    /// A card just arrived, laid out for a main area of `w`×`h`.
    #[must_use]
    pub fn new(card: Card, w: usize, h: usize) -> Self {
        let mut a = Self {
            card,
            choice: None,
            scroll: 0,
            chosen_at: None,
            unread: false,
            armed: Cell::new(None),
            seen_to: Cell::new(0),
            seen_width: Cell::new(0),
            total: Cell::new(0),
        };
        a.fit(w, h);
        a
    }

    fn act(&self, w: usize) -> Vec<String> {
        wrap_text(&self.card.act, act_width(w), ratatui::style::Style::new())
            .iter()
            .map(crate::kit::text::plain)
            .collect()
    }

    /// Clamp the scroll to the act at this size.
    pub fn fit(&mut self, w: usize, h: usize) {
        let total = self.act(w).len();
        self.scroll = self.scroll.min(total.saturating_sub(act_rows(h)));
    }

    /// The card was drawn at `now` in a main area of `w`×`h`: the first draw arms it, and the
    /// rows drawn count as seen when they continue the rows seen before.
    pub fn drawn(&self, now: Instant, w: usize, h: usize) {
        if self.armed.get().is_none() {
            self.armed.set(Some(now));
        }
        let total = self.act(w).len();
        // Wrapped at another width the rows are other rows: reading starts again.
        if self.seen_width.get() != act_width(w) {
            self.seen_width.set(act_width(w));
            self.seen_to.set(0);
        }
        let seen = self.seen_to.get();
        if self.scroll <= seen {
            self.seen_to
                .set(seen.max(self.scroll + act_rows(h)).min(total));
        }
        self.total.set(total);
    }

    /// Whether every row of the act has been on screen.
    #[must_use]
    pub fn viewed(&self) -> bool {
        self.total.get() > 0 && self.seen_to.get() >= self.total.get()
    }

    /// Arm the card again from `at`, its choice cleared: a key that answers nothing. A card not
    /// yet drawn stays unarmed — its first draw arms it, never a key that came before it.
    fn rearm(&mut self, at: Instant) {
        self.choice = None;
        if let Some(t) = self.armed.get() {
            self.armed.set(Some(t.max(at)));
        }
    }

    /// Text pasted while the card is open: never an answer.
    pub fn pasted(&mut self, at: Instant) {
        self.rearm(at);
    }

    /// A key stamped `at`, the main area `w`×`h`.
    pub fn key(&mut self, key: KeyEvent, at: Instant, w: usize, h: usize) -> Decision {
        let code = key.code;
        if code == KeyCode::Esc {
            return Decision::Answered(GateAnswer::Deny("the human said no".into()));
        }
        let rows = act_rows(h);
        let scroll = match code {
            KeyCode::Down | KeyCode::Char('j') => Some(self.scroll + 1),
            KeyCode::Up | KeyCode::Char('k') => Some(self.scroll.saturating_sub(1)),
            KeyCode::PageDown | KeyCode::Char(' ') => Some(self.scroll + rows),
            KeyCode::PageUp => Some(self.scroll.saturating_sub(rows)),
            KeyCode::End => Some(usize::MAX),
            KeyCode::Home => Some(0),
            _ => None,
        };
        let answer = matches!(code, KeyCode::Char('y' | 'n') | KeyCode::Enter);
        if scroll.is_none() && !answer {
            self.rearm(at);
            return Decision::Open;
        }
        // Typed before the card was on screen, or too soon after: not an answer to it.
        if self.armed.get().is_none_or(|t| at < t + ARM) {
            return Decision::Open;
        }
        if let Some(to) = scroll {
            self.choice = None;
            self.scroll = to;
            self.fit(w, h);
            return Decision::Open;
        }
        match code {
            KeyCode::Char('y') => {
                self.unread = !self.viewed();
                self.choice = self.viewed().then_some(true);
                self.chosen_at = Some(at);
            }
            KeyCode::Char('n') => {
                self.choice = Some(false);
                self.chosen_at = Some(at);
            }
            _ => {
                let settled = self.chosen_at.is_some_and(|c| at >= c + CONFIRM);
                match self.choice.take() {
                    Some(true) if settled => return Decision::Answered(GateAnswer::Approve),
                    Some(false) if settled => {
                        return Decision::Answered(GateAnswer::Deny("the human said no".into()));
                    }
                    // An Enter on the heels of its letter: typing, not a decision.
                    Some(_) => self.rearm(at),
                    None => {}
                }
            }
        }
        Decision::Open
    }

    /// The seconds left before silence denies, at `now`.
    #[must_use]
    pub fn left(&self, now: Instant) -> Duration {
        self.card.deadline.saturating_duration_since(now)
    }

    /// The card, `w` cells wide and at most `h` rows tall, at `now`, `after` more acts waiting.
    #[must_use]
    pub fn view(&self, t: &Theme, w: usize, h: usize, now: Instant, after: usize) -> Lines {
        let inner = w.saturating_sub(4);
        let act = self.act(w);
        let rows = act_rows(h);
        let cut = act.len() > rows;
        let bold = t.text().add_modifier(Modifier::BOLD);
        // Each line with its priority: the higher, the sooner it goes when the card must shrink.
        let mut parts: Vec<(u8, Line<'static>)> = vec![(
            2,
            truncate(
                Line::from(Span::styled(
                    format!(
                        "{} asks to run this {} act:",
                        self.card.body, self.card.class
                    ),
                    t.text(),
                )),
                inner,
            ),
        )];
        for l in act.iter().skip(self.scroll).take(rows) {
            parts.push((
                0,
                Line::from(vec![Span::raw("  "), Span::styled(l.clone(), bold)]),
            ));
        }
        if cut {
            let (from, to) = (self.scroll + 1, (self.scroll + rows).min(act.len()));
            let more = if to < act.len() {
                " · ↓ reads on"
            } else {
                ""
            };
            let mark = format!("┄ lines {from}–{to} of {} ┄{more}", act.len());
            parts.push((1, Line::from(Span::styled(clip(&mark, inner), t.gold()))));
            if to < act.len() {
                let last = act.last().cloned().unwrap_or_default();
                let tail = format!("  last: {last}");
                parts.push((3, Line::from(Span::styled(clip(&tail, inner), t.muted()))));
            }
        }
        if !self.card.outside.is_empty() {
            let text = format!("opens outside the world: {}", self.card.outside.join(", "));
            parts.extend(
                wrap_text(&text, inner, t.alarm())
                    .into_iter()
                    .map(|l| (4, l)),
            );
        }
        for why in &self.card.why {
            parts.extend(
                wrap_text(&format!("· {why}"), inner, t.muted())
                    .into_iter()
                    .map(|l| (5, l)),
            );
        }
        parts.push((6, Line::from("")));
        parts.push((0, self.choices(t, inner)));
        // Shrink to fit: borders take two rows; drop the least needed lines first.
        let room = h.saturating_sub(2);
        while parts.len() > room {
            let Some(worst) = parts.iter().map(|(p, _)| *p).max().filter(|p| *p > 0) else {
                break;
            };
            let at = parts.iter().rposition(|(p, _)| *p == worst).unwrap_or(0);
            parts.remove(at);
        }
        let secs = self.left(now).as_secs();
        Boxed::new()
            .border(Border::Thick, t.gold())
            .padding(Sides::sym(0, 1))
            .width(w)
            .title(Line::from(Span::styled(
                if after > 0 {
                    format!("⚠ the human gate · {after} more waiting")
                } else {
                    "⚠ the human gate".to_owned()
                },
                t.gold().add_modifier(Modifier::BOLD),
            )))
            .title_right(Line::from(Span::styled(
                format!("denies in {}:{:02}", secs / 60, secs % 60),
                t.muted(),
            )))
            .render(parts.into_iter().map(|(_, l)| l).collect())
    }

    fn choices(&self, t: &Theme, inner: usize) -> Line<'static> {
        let pick = |on: bool, key: &str| {
            let bold = t.gold().add_modifier(Modifier::BOLD);
            let style = if on {
                bold.add_modifier(Modifier::REVERSED)
            } else {
                bold
            };
            Span::styled(format!(" {key} "), style)
        };
        let next = if self.unread && !self.viewed() {
            Span::styled("  read the whole act first: ↓", t.alarm())
        } else if self.choice.is_some() {
            Span::styled("  enter confirms · esc denies", t.muted())
        } else {
            Span::styled("  choose, then enter · esc denies", t.muted())
        };
        // Narrow, the words shorten so both keys stay in view.
        let (run, deny) = if inner < 40 {
            (" run ", " deny")
        } else {
            (" run it once  ", " deny")
        };
        truncate(
            Line::from(vec![
                pick(self.choice == Some(true), "y"),
                Span::styled(run, t.text()),
                pick(self.choice == Some(false), "n"),
                Span::styled(deny, t.text()),
                next,
            ]),
            inner,
        )
    }
}
