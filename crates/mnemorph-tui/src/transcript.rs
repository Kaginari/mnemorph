//! The conversation as the screen keeps it: entries folded from the core's events, the text
//! still streaming, the reasoning as it comes, the calls in flight from their first byte, the
//! spend. Each entry's lines are laid out once per width and kept; a frame lays out only the
//! entries it shows, counted up from the tail.
//!
//! Time is the model's clock ([`Transcript::clock`]), advanced by its ticks: a thought's and a
//! call's durations are read from it, so a test drives them with ticks.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Call {
    /// The call's id.
    pub id: String,
    /// The tool.
    pub name: String,
    /// Its input; `Null` while it streams.
    pub input: Value,
    /// Its input as it streams, raw JSON text not yet whole.
    pub json: String,
    /// Its class as the classifier read it (`read`, `write`, `outward`, `destructive`), once read.
    pub class: Option<String>,
    /// When it started, on the transcript's clock.
    pub began: Duration,
    /// How long it ran, once it answered.
    pub took: Option<Duration>,
    /// What it returned, once it has.
    pub outcome: Option<Outcome>,
    /// The turn ended before it answered.
    pub stopped: bool,
    /// Its folded output or diff shown whole.
    pub expanded: bool,
    /// It holds the focus: a key expands or folds it.
    pub focused: bool,
}

impl Call {
    fn new(id: &str, name: &str, input: Value, began: Duration) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            input,
            began,
            ..Self::default()
        }
    }

    /// Whether it runs still.
    #[must_use]
    pub fn running(&self) -> bool {
        self.outcome.is_none() && !self.stopped
    }
}

/// A stretch of the model's reasoning: live while it comes, then folded to its duration.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Thought {
    /// The reasoning's text; empty when the provider hides it.
    pub text: String,
    /// When the model began, on the transcript's clock.
    pub began: Duration,
    /// How long it thought, once it moved on.
    pub took: Option<Duration>,
    /// Shown whole.
    pub expanded: bool,
    /// It holds the focus.
    pub focused: bool,
}

/// Consecutive reads, greps and globs: one compact card.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Group {
    /// The calls, oldest first.
    pub calls: Vec<Call>,
    /// Each call listed.
    pub expanded: bool,
    /// It holds the focus.
    pub focused: bool,
}

impl Group {
    /// Whether any of its calls runs still.
    #[must_use]
    pub fn running(&self) -> bool {
        self.calls.iter().any(Call::running)
    }
}

/// The tools a group gathers: they look, and change nothing.
pub const EXPLORE: [&str; 3] = ["read", "grep", "glob"];

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
    /// Consecutive looks — reads, greps, globs — as one card.
    Group(Group),
    /// The model's reasoning.
    Thought(Thought),
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
        /// What failed.
        failures: Vec<String>,
        /// The checks that gave no result.
        unverified: Vec<String>,
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

impl Entry {
    /// Whether it is a card the focus can take and a key expand: a call, a group, a thought.
    #[must_use]
    pub fn card(&self) -> bool {
        matches!(self, Entry::Tool(_) | Entry::Group(_) | Entry::Thought(_))
    }

    /// Whether expanding it would show more.
    #[must_use]
    pub fn foldable(&self) -> bool {
        match self {
            Entry::Tool(c) => blocks::foldable(c),
            Entry::Group(g) => !g.calls.is_empty(),
            Entry::Thought(t) => t.took.is_some() && !t.text.trim().is_empty(),
            _ => false,
        }
    }

    /// Whether it moves with time — a spinner, an elapsed count — and lays out again each frame.
    fn live(&self) -> bool {
        match self {
            Entry::Tool(c) => c.running(),
            Entry::Group(g) => g.running(),
            Entry::Thought(t) => t.took.is_none(),
            _ => false,
        }
    }

    fn set_focus(&mut self, on: bool) {
        match self {
            Entry::Tool(Call { focused, .. })
            | Entry::Group(Group { focused, .. })
            | Entry::Thought(Thought { focused, .. }) => *focused = on,
            _ => {}
        }
    }
}

/// What an entry's layout depends on beyond the entry: the width, the theme, the spinner's frame
/// and the clock's second while it is live, and whether the next entry continues its panel.
type Key = (usize, Variant, usize, u64, bool);

/// A laid-out entry.
#[derive(Debug, Clone)]
struct Laid {
    key: Key,
    lines: Lines,
}

/// The model step in progress: where it began among the entries and on the clock, when its first
/// output came, and whether its reasoning has an entry.
#[derive(Debug, Clone, Copy, Default)]
struct Step {
    at: usize,
    began: Duration,
    output: Option<Duration>,
    thought: Option<usize>,
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
    /// The clock: time the model's ticks have counted.
    pub clock: Duration,
    /// The model step in progress.
    step: Step,
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

    /// Add an entry; the one before it lays out again when the new one continues its panel.
    pub fn push(&mut self, entry: Entry) {
        if matches!(entry, Entry::Assistant { head: false, .. }) {
            let last = self.entries.len().checked_sub(1);
            if let Some(last) = last {
                self.touch(last);
            }
        }
        self.entries.push(entry);
        self.cache.get_mut().push(None);
    }

    /// Put an entry at `at`, every later index moved along.
    fn insert(&mut self, at: usize, entry: Entry) {
        let at = at.min(self.entries.len());
        self.entries.insert(at, entry);
        self.cache.get_mut().insert(at, None);
        if at > 0 {
            self.touch(at - 1);
        }
        self.touch(at + 1);
        for i in self.running.values_mut() {
            if *i >= at {
                *i += 1;
            }
        }
        if let Some(f) = &mut self.focus
            && *f >= at
        {
            *f += 1;
        }
    }

    /// A new model step begins now, after the last entry.
    fn begin_step(&mut self) {
        self.step = Step {
            at: self.entries.len(),
            began: self.clock,
            output: None,
            thought: None,
        };
    }

    /// The step's first output: its reasoning, if any shows, is done.
    fn output(&mut self) {
        if self.step.output.is_none() {
            self.step.output = Some(self.clock);
        }
        let clock = self.clock;
        if let Some(i) = self.step.thought
            && let Some(Entry::Thought(t)) = self.entries.get_mut(i)
            && t.took.is_none()
        {
            t.took = Some(clock.saturating_sub(t.began));
            self.touch(i);
        }
    }

    /// A piece of reasoning as it streams; an empty one says reasoning began, its text hidden.
    pub fn thinking(&mut self, piece: &str) {
        if let Some(i) = self.step.thought {
            if let Some(Entry::Thought(t)) = self.entries.get_mut(i)
                && t.took.is_none()
            {
                t.text.push_str(piece);
                self.touch(i);
            }
        } else {
            self.settle_stream();
            self.push(Entry::Thought(Thought {
                text: piece.to_owned(),
                began: self.step.began,
                ..Thought::default()
            }));
            self.step.thought = Some(self.entries.len() - 1);
        }
    }

    /// The model's turn, whole, carried reasoning in its blocks: the step's thought takes the
    /// whole text — what streamed may differ, or be doubled by a retry that streamed it again — or — when nothing streamed — a thought is placed where the step began, its
    /// duration the time to the step's first output.
    pub fn reasoned(&mut self, text: &str) {
        if let Some(i) = self.step.thought {
            if let Some(Entry::Thought(t)) = self.entries.get_mut(i)
                && t.text != text
            {
                text.clone_into(&mut t.text);
                self.touch(i);
            }
        } else {
            let end = self.step.output.unwrap_or(self.clock);
            let at = self.step.at;
            self.insert(
                at,
                Entry::Thought(Thought {
                    text: text.to_owned(),
                    began: self.step.began,
                    took: Some(end.saturating_sub(self.step.began)),
                    ..Thought::default()
                }),
            );
            self.step.thought = Some(at);
        }
    }

    /// A piece of a started call's input, as raw JSON.
    pub fn tool_input(&mut self, id: &str, piece: &str) {
        if let Some(&i) = self.running.get(id) {
            if let Some(c) = self.call_mut(i, id)
                && c.input.is_null()
            {
                c.json.push_str(piece);
            }
            self.touch(i);
        }
    }

    /// The classifier read call `id`'s class.
    pub fn classed(&mut self, id: &str, class: &str) {
        let at = self.entries.iter().rposition(|e| match e {
            Entry::Tool(c) => c.id == id,
            Entry::Group(g) => g.calls.iter().any(|c| c.id == id),
            _ => false,
        });
        if let Some(i) = at {
            if let Some(c) = self.call_mut(i, id) {
                c.class = Some(class.to_owned());
            }
            self.touch(i);
        }
    }

    /// Call `id` in entry `i`: the entry's own call, or one of its group's.
    fn call_mut(&mut self, i: usize, id: &str) -> Option<&mut Call> {
        match self.entries.get_mut(i)? {
            Entry::Tool(c) if c.id == id => Some(c),
            Entry::Group(g) => g.calls.iter_mut().find(|c| c.id == id),
            _ => None,
        }
    }

    /// The call running now, the newest: its name and input, for the activity line.
    #[must_use]
    pub fn current_call(&self) -> Option<&Call> {
        let newest = self.running.values().max()?;
        match self.entries.get(*newest)? {
            Entry::Tool(c) => Some(c),
            Entry::Group(g) => g.calls.iter().rev().find(|c| c.running()),
            _ => None,
        }
    }

    /// Whether the model is reasoning now, its thought live.
    #[must_use]
    pub fn thinking_now(&self) -> bool {
        self.step
            .thought
            .and_then(|i| self.entries.get(i))
            .is_some_and(|e| matches!(e, Entry::Thought(t) if t.took.is_none()))
    }

    /// Whether the step's answer streams now.
    #[must_use]
    pub fn writing(&self) -> bool {
        !self.streaming.is_empty()
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
                self.begin_step();
            }
            Event::Queued { text } => self.queued.push_back(text.clone()),
            Event::TextDelta { text } => {
                self.output();
                self.delta(text);
            }
            Event::ToolStart { id, name } => {
                self.output();
                self.start_call(id, name, Value::Null);
            }
            Event::Assistant { text, .. } => {
                self.output();
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
            Event::ToolCall { id, name, input } => {
                self.output();
                self.start_call(id, name, input.clone());
            }
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
                failures: failures.clone(),
                unverified: unverified.clone(),
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
            if let Some(c) = self.call_mut(i, id)
                && !input.is_null()
            {
                c.input = input;
                c.json.clear();
            }
            self.touch(i);
            return;
        }
        self.settle_stream();
        let call = Call::new(id, name, input, self.clock);
        let explore = EXPLORE.contains(&name);
        // A look after a look: the two become one group, or the group takes it.
        let joins = |e: &Entry| match e {
            Entry::Group(_) => true,
            Entry::Tool(c) => EXPLORE.contains(&c.name.as_str()),
            _ => false,
        };
        let last = self
            .entries
            .len()
            .checked_sub(1)
            .filter(|&l| explore && joins(&self.entries[l]));
        let Some(l) = last else {
            self.push(Entry::Tool(call));
            self.running.insert(id.to_owned(), self.entries.len() - 1);
            return;
        };
        let e = &mut self.entries[l];
        if let Entry::Group(g) = e {
            g.calls.push(call);
        } else if let Entry::Tool(first) = std::mem::replace(e, Entry::Info(String::new())) {
            *e = Entry::Group(Group {
                focused: first.focused,
                calls: vec![first, call],
                expanded: false,
            });
        }
        self.touch(l);
        self.running.insert(id.to_owned(), l);
    }

    fn result(&mut self, id: &str, content: &str, is_error: bool, cut: usize) {
        let outcome = Outcome {
            content: content.to_owned(),
            is_error,
            cut,
        };
        let clock = self.clock;
        if let Some(i) = self.running.remove(id) {
            if let Some(c) = self.call_mut(i, id) {
                c.took = Some(clock.saturating_sub(c.began));
                c.outcome = Some(outcome);
            }
            self.touch(i);
            if self.running.is_empty() {
                // Every call of the step answered: the model is asked again.
                self.begin_step();
            }
            return;
        }
        let mut call = Call::new(id, "tool", Value::Null, clock);
        call.outcome = Some(outcome);
        call.took = Some(Duration::ZERO);
        self.push(Entry::Tool(call));
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
        self.output();
        self.settle_stream();
        self.flushed.clear();
        let stopped: Vec<(String, usize)> = self.running.drain().collect();
        for (id, i) in stopped {
            if let Some(call) = self.call_mut(i, &id) {
                call.stopped = true;
            }
            self.touch(i);
        }
        self.step.thought = None;
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

    /// Whether entry `i`'s panel goes on in the next entry, or in the text still streaming.
    fn joins(&self, i: usize) -> bool {
        if !matches!(self.entries.get(i), Some(Entry::Assistant { .. })) {
            return false;
        }
        match self.entries.get(i + 1) {
            Some(next) => matches!(next, Entry::Assistant { head: false, .. }),
            None => !self.streaming.trim().is_empty() && self.headed,
        }
    }

    /// Entry `i`'s lines at `ctx`, laid out once per width, theme, live frame and join.
    fn laid(&self, i: usize, ctx: &Ctx<'_>) -> Lines {
        let mut cache = self.cache.borrow_mut();
        let entry = &self.entries[i];
        let (frame, second) = if entry.live() {
            (ctx.frame, ctx.now.as_secs())
        } else {
            (0, 0)
        };
        let key = (ctx.width, ctx.theme.variant, frame, second, self.joins(i));
        let fresh = cache[i].as_ref().is_some_and(|l| l.key == key);
        if !fresh {
            let prev = i.checked_sub(1).map(|p| &self.entries[p]);
            cache[i] = Some(Laid {
                key,
                lines: blocks::render(entry, prev, key.4, ctx),
            });
        }
        cache[i]
            .as_ref()
            .map(|l| l.lines.clone())
            .unwrap_or_default()
    }

    /// The text still streaming, laid out: never cached, it changes with every delta. Only the
    /// tail paragraph streams — a finished one has left for an entry — so only it reflows.
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
            false,
            ctx,
        )
    }

    /// The window `height` lines tall, `offset` lines up from the tail, at `ctx`'s width; `head`
    /// sits above the first entry — the welcome.
    #[must_use]
    pub fn window(&self, ctx: &Ctx<'_>, height: usize, offset: usize, head: &Lines) -> Window {
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
        let with_head = i == 0 && have < want;
        if with_head {
            have += head.len();
            tail.push((None, head.clone()));
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

    /// The cards — calls, groups, thoughts — by entry, oldest first.
    pub fn cards(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.card())
            .map(|(i, _)| i)
    }

    /// The card holding the focus.
    #[must_use]
    pub fn focused(&self) -> Option<usize> {
        self.focus
    }

    /// Give entry `i` the focus, or no entry: each card whose look changes is laid out again.
    pub fn focus(&mut self, i: Option<usize>) {
        let i = i.filter(|&i| self.entries.get(i).is_some_and(Entry::card));
        if i == self.focus {
            return;
        }
        for (at, on) in [(self.focus, false), (i, true)] {
            if let Some(at) = at
                && let Some(e) = self.entries.get_mut(at)
            {
                e.set_focus(on);
                self.touch(at);
            }
        }
        self.focus = i;
    }

    /// Expand entry `i`'s folded part, or fold it again; `false` when it is not a card with
    /// anything folded.
    pub fn toggle(&mut self, i: usize) -> bool {
        let Some(e) = self.entries.get(i) else {
            return false;
        };
        if !e.foldable() {
            return false;
        }
        match &mut self.entries[i] {
            Entry::Tool(Call { expanded, .. })
            | Entry::Group(Group { expanded, .. })
            | Entry::Thought(Thought { expanded, .. }) => *expanded = !*expanded,
            _ => return false,
        }
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
