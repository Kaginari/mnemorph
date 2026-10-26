//! The conversation as the screen keeps it: entries folded from the core's events, the text
//! still streaming, the calls in flight, the spend. Each entry's lines are laid out once per
//! width and kept; a frame lays out only the entries it shows, counted up from the tail.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use mnemorph_server::protocol::Event;
use serde_json::Value;

use crate::blocks::{self, Ctx};
use crate::kit::text::Lines;
use crate::theme::Variant;

/// What a tool returned.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Its text.
    pub content: String,
    /// Whether it failed.
    pub is_error: bool,
    /// Bytes the core cut from it.
    pub cut: usize,
}

/// A tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// The call's id.
    pub id: String,
    /// The tool.
    pub name: String,
    /// Its input; `Null` while it streams.
    pub input: Value,
    /// What it returned, once it has.
    pub outcome: Option<Outcome>,
    /// The turn ended before it answered.
    pub stopped: bool,
    /// Its folded output or diff shown whole.
    pub expanded: bool,
    /// It holds the focus: a key expands or folds it.
    pub focused: bool,
}

/// One thing in the transcript.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// The human's line.
    User(String),
    /// The body's text; `head` names the body above the turn's first piece.
    Assistant {
        /// The Markdown.
        text: String,
        /// Whether it opens the turn's answer.
        head: bool,
    },
    /// A tool call, running or done.
    Tool(Call),
    /// A note the loop added.
    Note(String),
    /// A warning for the human.
    Warning(String),
    /// The end gate's verdict.
    Verdict {
        /// PASS, FAIL or `NOT_VERIFIED`.
        verdict: String,
        /// The paths changed.
        changed: usize,
        /// What failed or gave no result.
        failures: Vec<String>,
    },
    /// A turn that ended other than by answering.
    Ending(String),
    /// A line the interface writes.
    Info(String),
    /// A line about a body the session commissioned.
    Below {
        /// The body.
        body: String,
        /// What it did.
        text: String,
        /// Whether it failed.
        alarm: bool,
    },
}

/// The spend, as the status bar reads it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Usage {
    /// Dollars, where calls were priced.
    pub usd: f64,
    /// Some call had no price.
    pub unpriced: bool,
    /// Tokens through the model: input, cache and output.
    pub tokens: u64,
    /// How full the context is.
    pub occupancy: u64,
    /// The line it is measured against.
    pub stress_line: u64,
}

/// A laid-out entry.
#[derive(Debug, Clone)]
struct Laid {
    key: (usize, Variant, usize),
    lines: Lines,
}

/// The transcript.
#[derive(Debug, Default)]
pub struct Transcript {
    entries: Vec<Entry>,
    cache: RefCell<Vec<Option<Laid>>>,
    /// The text streaming, not yet a whole paragraph.
    pub streaming: String,
    /// This model turn's text already moved into entries.
    flushed: String,
    /// Calls in flight, by id: their entry.
    running: HashMap<String, usize>,
    /// Whether the turn's answer has named its body yet.
    headed: bool,
    /// Lines waiting their turn.
    pub queued: VecDeque<String>,
    /// The spend.
    pub usage: Usage,
    /// The orders held on the shared work queue.
    pub held: u32,
    /// The call holding the focus.
    focus: Option<usize>,
}

impl Transcript {
    /// The entries.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// A transcript with no entries, keeping the queue and the spend.
    #[must_use]
    pub fn cleared(&self) -> Self {
        Self {
            queued: self.queued.clone(),
            usage: self.usage,
            held: self.held,
            ..Self::default()
        }
    }

    /// Add an entry.
    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
        self.cache.get_mut().push(None);
    }

    fn touch(&mut self, i: usize) {
        if let Some(slot) = self.cache.get_mut().get_mut(i) {
            *slot = None;
        }
    }

    /// Whether a call runs.
    #[must_use]
    pub fn running(&self) -> bool {
        !self.running.is_empty()
    }

    /// Text as the body's answer: the first piece of a turn names the body.
    fn say(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let head = !self.headed;
        self.headed = true;
        self.push(Entry::Assistant {
            text: text.to_owned(),
            head,
        });
    }

    /// Whatever streamed and is not yet an entry becomes one.
    fn settle_stream(&mut self) {
        let s = std::mem::take(&mut self.streaming);
        self.flushed.push_str(&s);
        self.say(&s);
    }

    /// Fold one event in; `below` names the commissioned body it came from, if any.
    pub fn fold(&mut self, event: &Event, below: Option<&str>) {
        if let Some(who) = below {
            self.fold_below(who, event);
            return;
        }
        match event {
            Event::TurnStarted { text } => {
                if self
                    .queued
                    .front()
                    .is_some_and(|q| q == text || text.ends_with(q.as_str()))
                {
                    self.queued.pop_front();
                }
                self.streaming.clear();
                self.headed = false;
                self.push(Entry::User(text.clone()));
            }
            Event::Queued { text } => self.queued.push_back(text.clone()),
            Event::TextDelta { text } => self.delta(text),
            Event::ToolStart { id, name } => self.start_call(id, name, Value::Null),
            Event::Assistant { text, .. } => {
                let rest = if self.flushed.is_empty() {
                    text.clone()
                } else if let Some(r) = text.strip_prefix(self.flushed.as_str()) {
                    r.to_owned()
                } else {
                    self.streaming.clone()
                };
                self.streaming.clear();
                self.flushed.clear();
                self.say(&rest);
            }
            Event::ToolCall { id, name, input } => self.start_call(id, name, input.clone()),
            Event::ToolResult {
                id,
                content,
                is_error,
                cut,
            } => self.result(id, content, *is_error, *cut),
            Event::Usage { .. } => self.usage(event, false),
            Event::Note { text } => self.push(Entry::Note(text.clone())),
            Event::Warning { text } => self.push(Entry::Warning(text.clone())),
            Event::GateVerdict {
                verdict,
                changed,
                failures,
                unverified,
                ..
            } => self.push(Entry::Verdict {
                verdict: verdict.clone(),
                changed: changed.len(),
                failures: failures.iter().chain(unverified).cloned().collect(),
            }),
            Event::TurnEnded { ending } => self.end_turn(ending),
            Event::Held { held } => self.held = *held,
        }
    }

    /// A piece of streaming text: a finished paragraph, outside any open fence, becomes an
    /// entry at once, so the live part stays short.
    fn delta(&mut self, text: &str) {
        self.streaming.push_str(text);
        let Some(cut) = self.streaming.rfind("\n\n") else {
            return;
        };
        let done = &self.streaming[..cut + 2];
        if open_fence(done) {
            return;
        }
        let done = done.to_owned();
        self.streaming.drain(..cut + 2);
        self.flushed.push_str(&done);
        self.say(&done);
    }

    fn start_call(&mut self, id: &str, name: &str, input: Value) {
        if let Some(&i) = self.running.get(id) {
            if let Some(Entry::Tool(call)) = self.entries.get_mut(i) {
                if !input.is_null() {
                    call.input = input;
                }
                self.touch(i);
            }
            return;
        }
        self.settle_stream();
        self.push(Entry::Tool(Call {
            id: id.to_owned(),
            name: name.to_owned(),
            input,
            outcome: None,
            stopped: false,
            expanded: false,
            focused: false,
        }));
        self.running.insert(id.to_owned(), self.entries.len() - 1);
    }

    fn result(&mut self, id: &str, content: &str, is_error: bool, cut: usize) {
        let outcome = Outcome {
            content: content.to_owned(),
            is_error,
            cut,
        };
        if let Some(i) = self.running.remove(id)
            && let Some(Entry::Tool(call)) = self.entries.get_mut(i)
        {
            call.outcome = Some(outcome);
            self.touch(i);
            return;
        }
        self.push(Entry::Tool(Call {
            id: id.to_owned(),
            name: "tool".into(),
            input: Value::Null,
            outcome: Some(outcome),
            stopped: false,
            expanded: false,
            focused: false,
        }));
    }

    fn usage(&mut self, event: &Event, below: bool) {
        let Event::Usage {
            input,
            output,
            cache_read,
            cache_write,
            usd,
            occupancy,
            stress_line,
            ..
        } = event
        else {
            return;
        };
        let u = &mut self.usage;
        u.usd += usd.unwrap_or(0.0);
        u.unpriced |= usd.is_none();
        // A body below spends dollars of its own; its tokens are its own context's.
        if !below {
            u.tokens += input + output + cache_read + cache_write;
            u.occupancy = *occupancy;
            u.stress_line = *stress_line;
        }
    }

    fn end_turn(&mut self, ending: &str) {
        self.settle_stream();
        self.flushed.clear();
        let stopped: Vec<usize> = self.running.drain().map(|(_, i)| i).collect();
        for i in stopped {
            if let Some(Entry::Tool(call)) = self.entries.get_mut(i) {
                call.stopped = true;
            }
            self.touch(i);
        }
        if ending != "answered" {
            self.push(Entry::Ending(ending.to_owned()));
        }
    }

    fn fold_below(&mut self, who: &str, event: &Event) {
        let line = |text: String, alarm: bool| Entry::Below {
            body: who.to_owned(),
            text,
            alarm,
        };
        match event {
            Event::TurnStarted { text } => {
                self.push(line(format!("is working on: {}", first_line(text)), false));
            }
            Event::ToolCall { name, input, .. } => {
                self.push(line(
                    format!("{name} {}", blocks::summary(name, input))
                        .trim_end()
                        .to_owned(),
                    false,
                ));
            }
            Event::ToolResult {
                content,
                is_error: true,
                ..
            } => self.push(line(format!("✗ {}", first_line(content)), true)),
            Event::Warning { text } => self.push(Entry::Warning(format!("{who}: {text}"))),
            Event::GateVerdict {
                passed: false,
                verdict,
                failures,
                unverified,
                ..
            } => {
                let why = failures.first().or(unverified.first()).cloned();
                self.push(line(
                    format!("gate {verdict}: {}", why.unwrap_or_else(|| "failed".into())),
                    true,
                ));
            }
            Event::TurnEnded { ending } => self.push(line(ending.clone(), ending != "answered")),
            Event::Usage { .. } => self.usage(event, true),
            _ => {}
        }
    }

    /// Entry `i`'s lines at `ctx`, laid out once per width, theme and spinner frame.
    fn laid(&self, i: usize, ctx: &Ctx<'_>) -> Lines {
        let mut cache = self.cache.borrow_mut();
        let entry = &self.entries[i];
        let frame = match entry {
            Entry::Tool(c) if c.outcome.is_none() && !c.stopped => ctx.frame,
            _ => 0,
        };
        let key = (ctx.width, ctx.theme.variant, frame);
        let fresh = cache[i].as_ref().is_some_and(|l| l.key == key);
        if !fresh {
            let prev = i.checked_sub(1).map(|p| &self.entries[p]);
            cache[i] = Some(Laid {
                key,
                lines: blocks::render(entry, prev, ctx),
            });
        }
        cache[i]
            .as_ref()
            .map(|l| l.lines.clone())
            .unwrap_or_default()
    }

    /// The text still streaming, laid out: never cached, it changes with every delta.
    fn live(&self, ctx: &Ctx<'_>) -> Lines {
        if self.streaming.trim().is_empty() {
            return Vec::new();
        }
        blocks::render(
            &Entry::Assistant {
                text: self.streaming.clone(),
                head: !self.headed,
            },
            self.entries.last(),
            ctx,
        )
    }

    /// The window `height` lines tall, `offset` lines up from the tail, at `ctx`'s width.
    #[must_use]
    pub fn window(&self, ctx: &Ctx<'_>, height: usize, offset: usize) -> Window {
        let want = height + offset;
        let mut tail: Vec<(Option<usize>, Lines)> = Vec::new();
        let live = self.live(ctx);
        let mut have = live.len();
        tail.push((None, live));
        let mut i = self.entries.len();
        while i > 0 && have < want {
            i -= 1;
            let lines = self.laid(i, ctx);
            have += lines.len();
            tail.push((Some(i), lines));
        }
        let top = (i == 0).then(|| have.saturating_sub(height));
        let mut lines: Lines = Vec::with_capacity(have);
        let mut owners: Vec<Option<usize>> = Vec::with_capacity(have);
        for (owner, block) in tail.into_iter().rev() {
            owners.extend(std::iter::repeat_n(owner, block.len()));
            lines.extend(block);
        }
        let end = lines.len().saturating_sub(offset.min(lines.len()));
        let start = end.saturating_sub(height);
        lines.truncate(end);
        owners.truncate(end);
        Window {
            lines: lines.split_off(start),
            owners: owners.split_off(start),
            top,
        }
    }

    /// Entry `i`'s height and the lines below it to the tail, at `ctx`: where a scroll must go
    /// to show it. Lays out only the entries from `i` down.
    #[must_use]
    pub fn place(&self, i: usize, ctx: &Ctx<'_>) -> (usize, usize) {
        let below: usize = (i + 1..self.entries.len())
            .map(|j| self.laid(j, ctx).len())
            .sum::<usize>()
            + self.live(ctx).len();
        (self.laid(i, ctx).len(), below)
    }

    /// The tool calls, by entry, oldest first.
    pub fn cards(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, Entry::Tool(_)))
            .map(|(i, _)| i)
    }

    /// The call holding the focus.
    #[must_use]
    pub fn focused(&self) -> Option<usize> {
        self.focus
    }

    /// Give entry `i` the focus, or no entry: each call whose look changes is laid out again.
    pub fn focus(&mut self, i: Option<usize>) {
        let i = i.filter(|&i| matches!(self.entries.get(i), Some(Entry::Tool(_))));
        if i == self.focus {
            return;
        }
        for (at, on) in [(self.focus, false), (i, true)] {
            if let Some(at) = at
                && let Some(Entry::Tool(c)) = self.entries.get_mut(at)
            {
                c.focused = on;
                self.touch(at);
            }
        }
        self.focus = i;
    }

    /// Expand entry `i`'s folded output, or fold it again; `false` when it is not a call with
    /// anything folded.
    pub fn toggle(&mut self, i: usize) -> bool {
        let Some(Entry::Tool(c)) = self.entries.get_mut(i) else {
            return false;
        };
        if !blocks::foldable(c) {
            return false;
        }
        c.expanded = !c.expanded;
        self.touch(i);
        true
    }
}

/// What a frame shows of the transcript.
#[derive(Debug, Clone, Default)]
pub struct Window {
    /// Its lines.
    pub lines: Lines,
    /// The entry each line belongs to; `None` for the text still streaming.
    pub owners: Vec<Option<usize>>,
    /// How far up the transcript goes, once a frame has laid it out to the top.
    pub top: Option<usize>,
}

/// Whether `text` ends inside a fenced code block: a fence opens with three or more backticks
/// or tildes and closes only with a run of the same character at least as long, alone on its
/// line — so a four-backtick fence holding three backticks stays open.
fn open_fence(text: &str) -> bool {
    let mut open: Option<(char, usize)> = None;
    for line in text.lines() {
        let indent = line.len() - line.trim_start_matches(' ').len();
        if indent > 3 {
            continue;
        }
        let t = &line[indent..];
        let Some(c) = t.chars().next().filter(|c| *c == '`' || *c == '~') else {
            continue;
        };
        let run = t.chars().take_while(|x| *x == c).count();
        if run < 3 {
            continue;
        }
        let rest = &t[run..];
        match open {
            None if !(c == '`' && rest.contains('`')) => open = Some((c, run)),
            Some((fence_char, fence_len))
                if fence_char == c && run >= fence_len && rest.trim().is_empty() =>
            {
                open = None;
            }
            _ => {}
        }
    }
    open.is_some()
}

fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or("");
    crate::kit::text::clip(line, 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(t: &mut Transcript, s: &str) {
        t.fold(&Event::TextDelta { text: s.into() }, None);
    }

    #[test]
    fn a_finished_paragraph_leaves_the_stream_but_not_inside_a_fence() {
        let mut t = Transcript::default();
        t.fold(&Event::TurnStarted { text: "hi".into() }, None);
        delta(&mut t, "one\n\n");
        assert_eq!(t.entries.len(), 2);
        delta(&mut t, "```rust\nfn a() {}\n\nfn b");
        assert_eq!(t.entries.len(), 2, "the fence is open");
        delta(&mut t, "() {}\n```\n\nafter");
        assert_eq!(t.entries.len(), 3);
        assert_eq!(t.streaming, "after");
        t.fold(
            &Event::Assistant {
                text: "one\n\n```rust\nfn a() {}\n\nfn b() {}\n```\n\nafter".into(),
                stop: "end_turn".into(),
            },
            None,
        );
        assert_eq!(
            t.entries.last(),
            Some(&Entry::Assistant {
                text: "after".into(),
                head: false
            })
        );
    }

    #[test]
    fn a_long_fence_holding_a_short_one_stays_open() {
        assert!(open_fence("````md\n```rust\nfn a() {}\n```\n"));
        assert!(!open_fence("````md\n```\n````\n"));
        assert!(open_fence("~~~\n```\n"));
        assert!(!open_fence("```\na\n```\n"));
        let mut t = Transcript::default();
        delta(&mut t, "````md\n```rust\nx\n```\n\nstill inside\n\n");
        assert!(t.entries.is_empty(), "nothing leaves from inside the fence");
        delta(&mut t, "````\n\nafter");
        assert_eq!(t.entries.len(), 1);
    }

    #[test]
    fn a_queued_line_leaves_the_queue_when_its_turn_starts() {
        let mut t = Transcript::default();
        t.fold(
            &Event::Queued {
                text: "next".into(),
            },
            None,
        );
        assert_eq!(t.queued.len(), 1);
        t.fold(
            &Event::TurnStarted {
                text: "next".into(),
            },
            None,
        );
        assert!(t.queued.is_empty());
    }

    #[test]
    fn a_body_below_spends_dollars_not_context() {
        let mut t = Transcript::default();
        let usage = Event::Usage {
            input: 10,
            output: 5,
            cache_read: 0,
            cache_write: 0,
            usd: Some(0.5),
            wt: 1,
            spent_wt: 1,
            occupancy: 900,
            stress_line: 1000,
        };
        t.fold(&usage, Some("elf-a"));
        assert!((t.usage.usd - 0.5).abs() < f64::EPSILON);
        assert_eq!(t.usage.occupancy, 0);
        t.fold(&usage, None);
        assert_eq!(t.usage.occupancy, 900);
        assert_eq!(t.usage.tokens, 15);
    }
}
