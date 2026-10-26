//! The approval card's state and drawing: the human sees the whole act before it can be
//! approved, a key typed before the card was on screen answers nothing, and the card leaves at
//! the gate's deadline.
//!
//! - The act scrolls inside the card: a cut act shows its head, the cut marked, and its last
//!   line; `y` is refused until the act has been read to its end.
//! - Keys are ignored until [`ARM`] after the card was first drawn — keys stamped earlier too, so
//!   a line typed ahead never reaches it. Only a bare `y` or `n`, then Enter, decides; any other
//!   key clears the choice. Esc denies at once.

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
    /// Whether the human has had the whole act in view.
    pub viewed: bool,
    /// `y` was pressed before the act was read to its end.
    pub unread: bool,
    /// When the card was first drawn.
    pub(crate) armed: Cell<Option<Instant>>,
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
            viewed: false,
            unread: false,
            armed: Cell::new(None),
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

    /// Clamp the scroll to the act at this size, and note when its end is in view.
    pub fn fit(&mut self, w: usize, h: usize) {
        let total = self.act(w).len();
        let rows = act_rows(h);
        self.scroll = self.scroll.min(total.saturating_sub(rows));
        if self.scroll + rows >= total {
            self.viewed = true;
        }
    }

    /// The card was drawn at `now`; the first draw arms it.
    pub fn drawn(&self, now: Instant) {
        if self.armed.get().is_none() {
            self.armed.set(Some(now));
        }
    }

    /// A key stamped `at`, the main area `w`×`h`.
    pub fn key(&mut self, key: KeyEvent, at: Instant, w: usize, h: usize) -> Decision {
        if key.code == KeyCode::Esc {
            return Decision::Answered(GateAnswer::Deny("the human said no".into()));
        }
        // Typed before the card was on screen, or too soon after: not an answer to it.
        if self.armed.get().is_none_or(|t| at < t + ARM) {
            return Decision::Open;
        }
        let rows = act_rows(h);
        match key.code {
            KeyCode::Char('y') => {
                self.unread = !self.viewed;
                self.choice = self.viewed.then_some(true);
            }
            KeyCode::Char('n') => self.choice = Some(false),
            KeyCode::Enter => match self.choice.take() {
                Some(true) => return Decision::Answered(GateAnswer::Approve),
                Some(false) => {
                    return Decision::Answered(GateAnswer::Deny("the human said no".into()));
                }
                None => {}
            },
            code => {
                self.choice = None;
                match code {
                    KeyCode::Down | KeyCode::Char('j') => self.scroll += 1,
                    KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                    KeyCode::PageDown | KeyCode::Char(' ') => self.scroll += rows,
                    KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(rows),
                    KeyCode::End => self.scroll = usize::MAX,
                    KeyCode::Home => self.scroll = 0,
                    _ => {}
                }
                self.fit(w, h);
            }
        }
        Decision::Open
    }

    /// The seconds left before silence denies, at `now`.
    #[must_use]
    pub fn left(&self, now: Instant) -> Duration {
        self.card.deadline.saturating_duration_since(now)
    }

    /// The card, `w` cells wide and at most `h` rows tall, at `now`.
    #[must_use]
    pub fn view(&self, t: &Theme, w: usize, h: usize, now: Instant) -> Lines {
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
                "⚠ the human gate",
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
        let next = if self.unread && !self.viewed {
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
