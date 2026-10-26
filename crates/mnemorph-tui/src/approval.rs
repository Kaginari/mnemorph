//! The approval card's state and drawing: the human sees the whole act before it can be
//! approved, a key typed before the card was on screen answers nothing, and the card leaves at
//! the gate's deadline.
//!
//! - The act scrolls inside the card: a cut act shows its head, the cut marked, and its last
//!   line. `y` is refused until every row of the act has been drawn on screen, read in order: a
//!   jump past rows never shown does not count them.
//! - Keys are ignored until [`ARM`] after the card was first drawn — keys stamped earlier too, so
//!   a line typed ahead never reaches it. Any key that is not a pick, Enter or a scroll — a
//!   paste too — clears the choice and arms the card again, so a word typed into it (`okay`,
//!   `deploy`) never ends in an answer. Enter decides only [`CONFIRM`] after its pick. Esc
//!   denies at once.
//! - It is drawn as Isekai's modal: a float over the dimmed view, a numbered list of answers —
//!   `1` yes (or `y`), `2` no (or `n`), `3` no with a reason typed for the model. A digit, a
//!   letter, or ↑↓ while the whole act is in view, picks; every pick passes the same arming, the
//!   same reading rule and the same confirmation as the letters.

use std::cell::Cell;
use std::time::{Duration, Instant};

use mnemorph_loop::GateAnswer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::gate::Card;
use crate::kit::slab::{Slab, Surface};
use crate::kit::text::{Align, Lines, cells, clip, clip_left, fit, truncate, wrap_text};
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
    /// The `n` chosen is the third answer: no, with a reason for the model.
    pub why: bool,
    /// The reason being typed, once the third answer was confirmed.
    pub reason: Option<String>,
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

/// The act's text width inside a card `w` cells wide: the slab's frame, and the act's indent.
fn act_width(w: usize) -> usize {
    Slab::inner(w).saturating_sub(2).max(8)
}

/// The rows of the act shown in a card at most `h` rows tall.
#[must_use]
pub fn act_rows(h: usize) -> usize {
    h.saturating_sub(12).clamp(1, ACT_ROWS)
}

/// The card's size over a main area `w`×`h`: a modal, centred, at most 92 cells wide.
#[must_use]
pub fn modal(w: usize, h: usize) -> (usize, usize) {
    let mw = w.saturating_sub(4).min(92).max(w.min(20));
    (mw, h.saturating_sub(2).max(1))
}

/// The answers, in their order on the card.
const ANSWERS: [(&str, &str); 3] = [
    ("1", "Yes, run it once"),
    ("2", "No"),
    ("3", "No, and tell the model why"),
];

impl Approval {
    /// A card just arrived, laid out for a main area of `w`×`h`.
    #[must_use]
    pub fn new(card: Card, w: usize, h: usize) -> Self {
        let mut a = Self {
            card,
            choice: None,
            why: false,
            reason: None,
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
        self.why = false;
        if let Some(t) = self.armed.get() {
            self.armed.set(Some(t.max(at)));
        }
    }

    /// Text pasted while the card is open: never an answer.
    pub fn pasted(&mut self, at: Instant) {
        self.rearm(at);
    }

    /// The answer picked at `n` (0 yes, 1 no, 2 no with a reason), stamped `at`: yes only once
    /// the whole act has been on screen.
    fn pick(&mut self, n: usize, at: Instant) {
        if n == 0 {
            self.unread = !self.viewed();
            self.choice = self.viewed().then_some(true);
            self.why = false;
        } else {
            self.choice = Some(false);
            self.why = n == 2;
        }
        self.chosen_at = Some(at);
    }

    /// The answer the card's highlight sits on.
    fn picked(&self) -> Option<usize> {
        match self.choice {
            Some(true) => Some(0),
            Some(false) if self.why => Some(2),
            Some(false) => Some(1),
            None => None,
        }
    }

    /// A key while the reason is typed: Enter sends it with the denial, Esc denies without it.
    fn reason_key(&mut self, code: KeyCode) -> Decision {
        let Some(reason) = &mut self.reason else {
            return Decision::Open;
        };
        match code {
            KeyCode::Enter => {
                let why = reason.trim();
                let deny = if why.is_empty() {
                    "the human said no".to_owned()
                } else {
                    format!("the human said no: {why}")
                };
                Decision::Answered(GateAnswer::Deny(deny))
            }
            KeyCode::Backspace => {
                reason.pop();
                Decision::Open
            }
            KeyCode::Char(c) if reason.chars().count() < 400 => {
                reason.push(c);
                Decision::Open
            }
            _ => Decision::Open,
        }
    }

    /// A key stamped `at`, the card `w`×`h`.
    pub fn key(&mut self, key: KeyEvent, at: Instant, w: usize, h: usize) -> Decision {
        let code = key.code;
        if code == KeyCode::Esc {
            return Decision::Answered(GateAnswer::Deny("the human said no".into()));
        }
        if self.reason.is_some() {
            return self.reason_key(code);
        }
        let rows = act_rows(h);
        // The whole act in view: ↑↓ pick an answer. Cut: they read it.
        let whole = self.act(w).len() <= rows;
        let step = match code {
            KeyCode::Down if whole => Some(1),
            KeyCode::Up if whole => Some(ANSWERS.len() - 1),
            _ => None,
        };
        let scroll = match code {
            _ if step.is_some() => None,
            KeyCode::Down | KeyCode::Char('j') => Some(self.scroll + 1),
            KeyCode::Up | KeyCode::Char('k') => Some(self.scroll.saturating_sub(1)),
            KeyCode::PageDown | KeyCode::Char(' ') => Some(self.scroll + rows),
            KeyCode::PageUp => Some(self.scroll.saturating_sub(rows)),
            KeyCode::End => Some(usize::MAX),
            KeyCode::Home => Some(0),
            _ => None,
        };
        let answer = step.is_some()
            || matches!(
                code,
                KeyCode::Char('y' | 'n' | '1' | '2' | '3') | KeyCode::Enter
            );
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
            self.why = false;
            self.scroll = to;
            self.fit(w, h);
            return Decision::Open;
        }
        match code {
            KeyCode::Char('y' | '1') => self.pick(0, at),
            KeyCode::Char('n' | '2') => self.pick(1, at),
            KeyCode::Char('3') => self.pick(2, at),
            KeyCode::Enter => {
                let settled = self.chosen_at.is_some_and(|c| at >= c + CONFIRM);
                let why = self.why;
                match self.choice.take() {
                    Some(true) if settled => return Decision::Answered(GateAnswer::Approve),
                    Some(false) if settled && why => {
                        self.why = true;
                        self.reason = Some(String::new());
                    }
                    Some(false) if settled => {
                        return Decision::Answered(GateAnswer::Deny("the human said no".into()));
                    }
                    // An Enter on the heels of its pick: typing, not a decision.
                    Some(_) => self.rearm(at),
                    None => {}
                }
            }
            _ => {
                if let Some(s) = step {
                    let next = self.picked().map_or(0, |p| (p + s) % ANSWERS.len());
                    self.pick(next, at);
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

    /// The card, `w` cells wide and at most `h` rows tall, at `now`, `after` more acts waiting:
    /// a float, its header strip naming the gate, the act's class in its colour, the act, the
    /// answers numbered, the keys.
    #[must_use]
    pub fn view(&self, t: &Theme, w: usize, h: usize, now: Instant, after: usize) -> Lines {
        let inner = Slab::inner(w);
        let act = self.act(w);
        let rows = act_rows(h);
        let cut = act.len() > rows;
        let bold = t.text().add_modifier(Modifier::BOLD);
        // Each line with its priority: the higher, the sooner it goes when the card must shrink.
        let mut parts = self.head(t, inner, now, after);
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
                // The act's end, where a payload hides: cut from the left, so its last words show.
                let tail = format!("  last: {}", clip_left(&last, inner.saturating_sub(8)));
                parts.push((3, Line::from(Span::styled(tail, t.muted()))));
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
        parts.extend(self.answers(t, inner));
        parts.push((0, self.keys(t, inner, cut)));
        // Shrink to fit: the edges take two rows; drop the least needed lines first.
        let room = h.saturating_sub(2);
        while parts.len() > room {
            let Some(worst) = parts.iter().map(|(p, _)| *p).max().filter(|p| *p > 0) else {
                break;
            };
            let at = parts.iter().rposition(|(p, _)| *p == worst).unwrap_or(0);
            parts.remove(at);
        }
        Slab::new(Surface::float(t, Some(t.p.holo), 0.06))
            .band(1)
            .render(parts.into_iter().map(|(_, l)| l).collect(), w)
    }

    /// The card's head: the strip naming the gate with its countdown, the act's class in its
    /// colour and who asks.
    fn head(
        &self,
        t: &Theme,
        inner: usize,
        now: Instant,
        after: usize,
    ) -> Vec<(u8, Line<'static>)> {
        let hue = crate::blocks::class_hue(t, Some(&self.card.class));
        let secs = self.left(now).as_secs();
        let title = "Approval · ⚠ the human gate";
        let mut clock = format!("denies in {}:{:02}", secs / 60, secs % 60);
        if after > 0 {
            clock = format!("{after} more waiting · {clock}");
        }
        let room = inner.saturating_sub(cells(&clock) + 1);
        let shown = clip(title, room);
        let band = truncate(
            Line::from(vec![
                Span::styled(shown.clone(), t.holo().add_modifier(Modifier::BOLD)),
                Span::raw(" ".repeat(inner.saturating_sub(cells(&shown) + cells(&clock)).max(1))),
                Span::styled(clock, t.muted()),
            ]),
            inner,
        );
        vec![
            (0, band),
            (
                2,
                truncate(
                    Line::from(vec![
                        Span::styled(
                            self.card.class.clone(),
                            ratatui::style::Style::new()
                                .fg(hue.color())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("  ", t.text()),
                        Span::styled(
                            format!(
                                "{} asks to run this {} act:",
                                self.card.body, self.card.class
                            ),
                            t.text(),
                        ),
                    ]),
                    inner,
                ),
            ),
        ]
    }

    /// The answers, numbered, the picked one lit; or, once the third was confirmed, the reason
    /// as it is typed.
    fn answers(&self, t: &Theme, inner: usize) -> Vec<(u8, Line<'static>)> {
        let bold = t.text().add_modifier(Modifier::BOLD);
        let mut out = Vec::new();
        match &self.reason {
            Some(reason) => {
                out.push((
                    1,
                    Line::from(Span::styled("why — the model reads it:", t.text())),
                ));
                out.push((
                    0,
                    truncate(
                        Line::from(vec![
                            Span::styled("› ", t.holo()),
                            Span::styled(crate::safe::one_line(reason), bold),
                            Span::styled("▏", t.holo()),
                        ]),
                        inner,
                    ),
                ));
            }
            None => {
                for (i, (n, words)) in ANSWERS.iter().enumerate() {
                    let on = self.picked() == Some(i);
                    let style = if on {
                        ratatui::style::Style::new()
                            .fg(t.p.bg.color())
                            .bg(t.p.holo.color())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        t.text()
                    };
                    let line = Line::from(Span::styled(format!(" {n} · {words}"), style));
                    let fill = if on {
                        ratatui::style::Style::new().bg(t.p.holo.color())
                    } else {
                        ratatui::style::Style::new()
                    };
                    out.push((1, fit(line, inner, Align::Left, fill)));
                }
            }
        }
        out
    }

    /// The keys that answer now — `y` and `n` always named, so a narrow card still says them.
    fn keys(&self, t: &Theme, inner: usize, cut: bool) -> Line<'static> {
        let key = t.gold().add_modifier(Modifier::BOLD);
        let next = if self.reason.is_some() {
            Span::styled("enter sends · esc denies without it", t.muted())
        } else if self.unread && !self.viewed() {
            Span::styled("read the whole act first: ↓", t.alarm())
        } else if self.choice.is_some() {
            Span::styled("enter confirms · esc denies", t.muted())
        } else if cut {
            Span::styled("↓ reads · 1-3 picks · enter confirms", t.muted())
        } else {
            Span::styled("↑↓ or a digit picks · enter confirms", t.muted())
        };
        truncate(
            Line::from(vec![
                Span::styled(" y ", key),
                Span::styled("yes", t.text()),
                Span::styled(" · ", t.frame()),
                Span::styled(" n ", key),
                Span::styled("no", t.text()),
                Span::styled(" · ", t.frame()),
                next,
            ]),
            inner,
        )
    }
}
