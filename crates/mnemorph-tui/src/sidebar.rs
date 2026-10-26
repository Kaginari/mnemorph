//! The sidebar: a column beside the transcript, toggled with Ctrl+O and folded away on a narrow
//! terminal. Four sections, each drawn only from what the observer or the world gave:
//!
//! - **session** — the spend, the context against its stress line as a gauge, the model;
//! - **memory** — the notes recalled for this ask, the grounded and shared counts, the claim
//!   grounded last, read as `mnemorphd memory status` reads it, off the UI thread ([`survey`]);
//! - **files** — the paths the session's acts changed, marked added, modified or deleted;
//! - **bodies** — the session's body and the bodies it commissioned, as `Seen::From` nests them:
//!   each in its race's colour, with what it is doing.
//!
//! Every name, path and claim passes through [`crate::safe`] before it is drawn.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use mnemorph_loop::world::memory::{Standing, standing};
use mnemorph_loop::world::roster::Roster;
use mnemorph_races::Race;
use mnemorph_server::protocol::Event;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

use crate::app::Model;
use crate::kit::gauge::Gauge;
use crate::kit::text::{Lines, cells, clip, clip_left, truncate, wrap_text};
use crate::safe::one_line;
use crate::theme::Theme;

/// The sidebar's width, its gutter included.
pub const WIDTH: u16 = 32;
/// The narrowest frame that keeps the sidebar: below it, the transcript takes the whole width.
pub const FOLD_BELOW: u16 = 100;

/// What the world says, read off the UI thread: the memory's standing and each creature's race.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Survey {
    /// The memory's standing.
    pub memory: Standing,
    /// Each creature's race, by name.
    pub races: BTreeMap<String, Race>,
}

/// Read `world`'s memory standing and its roster's races: file reads, for a runtime thread.
#[must_use]
pub fn survey(world: &Path) -> Survey {
    let races = Roster::load(world)
        .creatures
        .into_iter()
        .map(|(name, c)| (name, c.race))
        .collect();
    Survey {
        memory: standing(world),
        races,
    }
}

/// How an act left a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// It did not exist before the session.
    Added,
    /// It existed and changed.
    Modified,
    /// It existed and is gone.
    Deleted,
}

impl Mark {
    /// The mark as `git status` writes it.
    #[must_use]
    pub fn letter(self) -> &'static str {
        match self {
            Mark::Added => "A",
            Mark::Modified => "M",
            Mark::Deleted => "D",
        }
    }

    /// `self`, then `next` on the same path: the mark the session's acts leave together, or
    /// `None` when a path added this session is deleted again.
    fn then(self, next: Mark) -> Option<Mark> {
        Some(match (self, next) {
            (Mark::Added, Mark::Deleted) => return None,
            (Mark::Added, _) => Mark::Added,
            (Mark::Deleted, Mark::Added | Mark::Modified) | (Mark::Modified, Mark::Added) => {
                Mark::Modified
            }
            (_, next) => next,
        })
    }
}

/// The paths changed this session: each act's paths held until its result says it landed.
#[derive(Debug, Clone, Default)]
pub struct Files {
    /// The paths, the latest touched last.
    list: Vec<(String, Mark)>,
    /// Calls in flight: the paths each will change.
    pending: HashMap<String, Vec<(String, Mark)>>,
    /// Whether a `write`'s path existed, by call, when a look came back before the call.
    existed: HashMap<String, bool>,
    /// `write` calls whose look has not come back: once landed, the path and whether it was
    /// listed before the write.
    awaiting: HashMap<String, Option<(String, bool)>>,
}

impl Files {
    /// The paths changed, the latest touched last.
    #[must_use]
    pub fn list(&self) -> &[(String, Mark)] {
        &self.list
    }

    /// A look at `write` call `id`'s path came back: it existed, or not. The look runs off the
    /// session's task, so it may race the write: "not there" proves the write created the file,
    /// whenever it arrives; "there" proves nothing, and the path stays modified.
    pub fn prior(&mut self, id: &str, existed: bool) {
        match self.awaiting.remove(id) {
            // Called, not landed: the mark waiting for the result is set now.
            Some(None) => {
                if !existed && let Some(marks) = self.pending.get_mut(id) {
                    for (_, mark) in marks.iter_mut() {
                        *mark = Mark::Added;
                    }
                }
            }
            // Landed as modified: a path the session had not touched before becomes added.
            Some(Some((path, listed))) => {
                if !existed
                    && !listed
                    && let Some(entry) = self
                        .list
                        .iter_mut()
                        .find(|(p, m)| *p == path && *m == Mark::Modified)
                {
                    entry.1 = Mark::Added;
                }
            }
            None => {
                self.existed.insert(id.to_owned(), existed);
            }
        }
    }

    /// A call `id` of tool `name` with `input`; `root` is the world, for absolute paths.
    pub fn call(&mut self, id: &str, name: &str, input: &Value, root: Option<&Path>) {
        let field = |k: &str| {
            input
                .get(k)
                .and_then(Value::as_str)
                .map(|p| relative(p, root))
        };
        let marks: Vec<(String, Mark)> = match name {
            "edit" | "multiedit" => field("path")
                .map(|p| (p, Mark::Modified))
                .into_iter()
                .collect(),
            "delete" => field("path")
                .map(|p| (p, Mark::Deleted))
                .into_iter()
                .collect(),
            "write" => {
                // Until a look says the path was not there, the write changed it.
                let mark = match self.existed.remove(id) {
                    Some(false) => Mark::Added,
                    Some(true) => Mark::Modified,
                    None => {
                        self.awaiting.insert(id.to_owned(), None);
                        Mark::Modified
                    }
                };
                field("path").map(|p| (p, mark)).into_iter().collect()
            }
            "move" => field("from")
                .map(|p| (p, Mark::Deleted))
                .into_iter()
                .chain(field("to").map(|p| (p, Mark::Added)))
                .collect(),
            "patch" => input
                .get("patch")
                .and_then(Value::as_str)
                .map(|p| patch_marks(p, root))
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        if !marks.is_empty() {
            self.pending.insert(id.to_owned(), marks);
        }
    }

    /// Call `id` returned: its paths count when it did not fail.
    pub fn result(&mut self, id: &str, is_error: bool) {
        let Some(marks) = self.pending.remove(id) else {
            return;
        };
        if is_error {
            self.awaiting.remove(id);
            return;
        }
        for (path, mark) in marks {
            let was = self.list.iter().position(|(p, _)| *p == path);
            if let Some(wait) = self.awaiting.get_mut(id) {
                *wait = Some((path.clone(), was.is_some()));
            }
            let mark = match was {
                Some(at) => {
                    let (_, before) = self.list.remove(at);
                    before.then(mark)
                }
                None => Some(mark),
            };
            if let Some(mark) = mark {
                self.list.push((path, mark));
            }
        }
    }
}

/// `path` relative to `root` when it lies inside it; `./` dropped.
fn relative(path: &str, root: Option<&Path>) -> String {
    let p = Path::new(path);
    let rel = root
        .and_then(|r| p.strip_prefix(r).ok())
        .map_or_else(|| path.to_owned(), |r| r.display().to_string());
    rel.strip_prefix("./").map(str::to_owned).unwrap_or(rel)
}

/// The paths a unified diff changes: `/dev/null` as the old side adds, as the new side deletes;
/// a renamed file is deleted under its old name and added under its new one.
fn patch_marks(patch: &str, root: Option<&Path>) -> Vec<(String, Mark)> {
    let side = |rest: &str| -> Option<String> {
        let p = rest.split('\t').next().unwrap_or("").trim();
        if p == "/dev/null" {
            return None;
        }
        let p = p
            .strip_prefix("a/")
            .or_else(|| p.strip_prefix("b/"))
            .unwrap_or(p);
        Some(relative(p, root))
    };
    let mut out = Vec::new();
    let mut old: Option<Option<String>> = None;
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("--- ") {
            old = Some(side(rest));
        } else if let Some(rest) = line.strip_prefix("+++ ")
            && let Some(was) = old.take()
        {
            match (was, side(rest)) {
                (None, Some(new)) => out.push((new, Mark::Added)),
                (Some(was), None) => out.push((was, Mark::Deleted)),
                (Some(was), Some(new)) if was != new => {
                    out.push((was, Mark::Deleted));
                    out.push((new, Mark::Added));
                }
                (Some(_), Some(new)) => out.push((new, Mark::Modified)),
                (None, None) => {}
            }
        }
    }
    out
}

/// Where a body stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Its turn runs.
    Working,
    /// It answered.
    Done,
    /// The human stopped it.
    Stopped,
    /// It failed, escalated or stopped at a checkpoint.
    Failed,
}

/// A commissioned body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    /// Its name.
    pub name: String,
    /// The body that commissioned it; `None` for the session's own.
    pub parent: Option<String>,
    /// Where it stands.
    pub state: State,
    /// The tool it runs now.
    pub doing: Option<String>,
}

/// The bodies below the session, first seen first, indexed as they fold in — by name, and by
/// the body that commissioned them — so a frame walks the tree without searching it.
#[derive(Debug, Clone, Default)]
pub struct Bodies {
    list: Vec<Body>,
    /// Each body's place in `list`, by name.
    at: HashMap<String, usize>,
    /// The bodies each commissioned, first seen first; the session's own under `None`.
    children: HashMap<Option<usize>, Vec<usize>>,
    /// How many work.
    working: usize,
}

impl Bodies {
    /// The bodies, first seen first.
    #[must_use]
    pub fn list(&self) -> &[Body] {
        &self.list
    }

    /// How many still work.
    #[must_use]
    pub fn live(&self) -> usize {
        self.working
    }

    /// The tree, depth first: each body after the one that commissioned it, siblings first seen
    /// first, with its depth (the session's own commissions at 1). Lazy: a frame that shows ten
    /// rows walks ten bodies.
    pub fn walk(&self) -> impl Iterator<Item = (usize, &Body)> + '_ {
        let roots = self.children.get(&None).map_or(&[][..], Vec::as_slice);
        let mut stack: Vec<(usize, usize)> = roots.iter().rev().map(|&i| (1, i)).collect();
        std::iter::from_fn(move || {
            let (depth, i) = stack.pop()?;
            if let Some(kids) = self.children.get(&Some(i)) {
                stack.extend(kids.iter().rev().map(|&k| (depth + 1, k)));
            }
            Some((depth, &self.list[i]))
        })
    }

    /// An event from the innermost body of `chain`, outermost first.
    pub fn fold(&mut self, chain: &[String], event: &Event) {
        let mut parent: Option<usize> = None;
        for name in chain {
            let i = if let Some(&i) = self.at.get(name) {
                i
            } else {
                let i = self.list.len();
                self.list.push(Body {
                    name: name.clone(),
                    parent: parent.map(|p| self.list[p].name.clone()),
                    state: State::Working,
                    doing: None,
                });
                self.at.insert(name.clone(), i);
                self.children.entry(parent).or_default().push(i);
                self.working += 1;
                i
            };
            parent = Some(i);
        }
        let Some(i) = parent else {
            return;
        };
        let b = &mut self.list[i];
        let was = b.state;
        match event {
            Event::TurnStarted { .. } => {
                b.state = State::Working;
                b.doing = None;
            }
            Event::ToolStart { name, .. } | Event::ToolCall { name, .. } => {
                b.state = State::Working;
                b.doing = Some(name.clone());
            }
            Event::ToolResult { .. } => b.doing = None,
            Event::TurnEnded { ending } => {
                b.doing = None;
                b.state = match ending.as_str() {
                    "answered" => State::Done,
                    "interrupted" => State::Stopped,
                    _ => State::Failed,
                };
            }
            _ => {}
        }
        match (was == State::Working, b.state == State::Working) {
            (true, false) => self.working -= 1,
            (false, true) => self.working += 1,
            _ => {}
        }
    }
}

/// The sidebar's state.
#[derive(Debug, Clone)]
pub struct Sidebar {
    /// Whether the human wants it shown (Ctrl+O); a narrow terminal folds it all the same.
    pub open: bool,
    /// The world, for making absolute paths relative.
    pub root: Option<PathBuf>,
    /// The memory's standing, once read.
    pub memory: Option<Standing>,
    /// Notes recall gave this ask; `None` before the first ask.
    pub recalled: Option<usize>,
    /// Each creature's race, by name.
    pub races: BTreeMap<String, Race>,
    /// The files changed.
    pub files: Files,
    /// The bodies below the session.
    pub bodies: Bodies,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            open: true,
            root: None,
            memory: None,
            recalled: None,
            races: BTreeMap::new(),
            files: Files::default(),
            bodies: Bodies::default(),
        }
    }
}

impl Sidebar {
    /// An event; `chain` names the bodies it came through, outermost first, empty for the
    /// session's own.
    pub fn fold(&mut self, event: &Event, chain: &[String]) {
        match event {
            Event::TurnStarted { .. } if chain.is_empty() => self.recalled = Some(0),
            Event::ToolCall { id, name, input } => {
                self.files.call(id, name, input, self.root.as_deref());
            }
            Event::ToolResult { id, is_error, .. } => self.files.result(id, *is_error),
            _ => {}
        }
        if !chain.is_empty() {
            self.bodies.fold(chain, event);
        }
    }

    /// The world's survey arrived.
    pub fn surveyed(&mut self, s: Survey) {
        self.memory = Some(s.memory);
        self.races = s.races;
    }

    /// The sidebar's width in a frame whose inside is `inner` cells wide (the frame's own width
    /// less its border): none when closed or when the frame is narrower than [`FOLD_BELOW`].
    #[must_use]
    pub fn width(&self, inner: u16) -> u16 {
        if self.open && inner + 4 >= FOLD_BELOW {
            WIDTH
        } else {
            0
        }
    }
}

/// A section's title: its name in its hue, a count, a quiet rule to the edge.
fn title(t: &Theme, name: &str, hue: Style, count: Option<String>, w: usize) -> Line<'static> {
    let mut spans = vec![Span::styled(
        name.to_owned(),
        hue.add_modifier(Modifier::BOLD),
    )];
    if let Some(c) = count {
        spans.push(Span::styled(format!(" · {c}"), t.muted()));
    }
    let used: usize = spans.iter().map(|s| cells(&s.content)).sum();
    if used + 2 <= w {
        spans.push(Span::raw(" "));
        spans.push(Span::styled("─".repeat(w - used - 1), t.line()));
    }
    truncate(Line::from(spans), w)
}

/// A row: `left` with `right` set against the edge.
fn row(left: Vec<Span<'static>>, right: Vec<Span<'static>>, w: usize) -> Line<'static> {
    let rw: usize = right.iter().map(|s| cells(&s.content)).sum();
    let mut left = truncate(Line::from(left), w.saturating_sub(rw + 1).max(1));
    let lw: usize = left.spans.iter().map(|s| cells(&s.content)).sum();
    if rw > 0 && lw + 1 + rw <= w {
        left.spans.push(Span::raw(" ".repeat(w - lw - rw)));
        left.spans.extend(right);
    }
    left
}

/// Tokens, short: 950, 12.3k, 4.1M.
#[must_use]
pub fn short(n: u64) -> String {
    // Display only: a rounding of a token count.
    #[allow(clippy::cast_precision_loss)]
    let f = n as f64;
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", f / 1e3)
    } else {
        format!("{:.1}M", f / 1e6)
    }
}

/// The context's style at `pct` of its stress line: quiet, then gold from 75%, alarm from 90%.
#[must_use]
pub fn band(t: &Theme, pct: u64) -> Style {
    match pct {
        0..75 => t.muted(),
        75..90 => t.gold(),
        _ => t.alarm(),
    }
}

fn session(m: &Model, w: usize) -> Lines {
    let t = &m.theme;
    let u = &m.transcript.usage;
    let mut out = vec![title(t, "session", t.holo(), None, w)];
    if u.tokens > 0 || u.usd > 0.0 {
        let cost = if u.unpriced && u.usd == 0.0 {
            "unpriced".to_owned()
        } else {
            format!("${:.2}{}", u.usd, if u.unpriced { "+" } else { "" })
        };
        out.push(Line::from(vec![
            Span::styled(cost, t.text().add_modifier(Modifier::BOLD)),
            Span::styled(" · ", t.frame()),
            Span::styled(format!("{} tok", short(u.tokens)), t.muted()),
        ]));
    } else {
        out.push(Line::from(Span::styled("no spend yet", t.muted())));
    }
    let mut ctx = vec![Span::styled("ctx ", t.muted())];
    match u.occupancy.saturating_mul(100).checked_div(u.stress_line) {
        Some(pct) => {
            let label = format!(" {pct}%");
            let bar = w.saturating_sub(4 + cells(&label)).min(20);
            // Display only: a fraction of the stress line.
            #[allow(clippy::cast_precision_loss)]
            let frac = u.occupancy as f64 / u.stress_line as f64;
            let gauge = Gauge { track: t.line() };
            ctx.extend(gauge.view(frac, bar, |x| {
                // Each cell warms by where it sits against the stress line.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let at = (x * 100.0).round() as u64;
                if band(t, at) == t.muted() {
                    t.holo()
                } else {
                    band(t, at)
                }
            }));
            ctx.push(Span::styled(
                label,
                band(t, pct).add_modifier(Modifier::BOLD),
            ));
        }
        None => ctx.push(Span::styled("—", t.muted())),
    }
    out.push(Line::from(ctx));
    out.push(Line::from(Span::styled(
        clip(&one_line(&m.header.model), w),
        t.muted(),
    )));
    out
}

fn memory(m: &Model, w: usize) -> Lines {
    let t = &m.theme;
    let side = &m.sidebar;
    let mut out = vec![title(t, "memory", t.race(Race::Elf), None, w)];
    out.push(match side.recalled {
        Some(count) => Line::from(vec![
            Span::styled("recalled ", t.muted()),
            Span::styled(count.to_string(), t.text().add_modifier(Modifier::BOLD)),
            Span::styled(" this ask", t.muted()),
        ]),
        None => Line::from(Span::styled("no ask yet", t.muted())),
    });
    let Some(st) = &side.memory else {
        out.push(Line::from(Span::styled("reading the world…", t.muted())));
        return out;
    };
    let num = |n: usize| Span::styled(n.to_string(), t.text().add_modifier(Modifier::BOLD));
    out.push(truncate(
        Line::from(vec![
            Span::styled("grounded ", t.muted()),
            num(st.grounded),
            Span::styled(" · shared ", t.muted()),
            num(st.shared),
        ]),
        w,
    ));
    if let Some(claim) = &st.latest {
        let text = format!("“{}”", one_line(claim));
        let mut lines = wrap_text(
            &text,
            w.saturating_sub(2),
            t.muted().add_modifier(Modifier::ITALIC),
        );
        if lines.len() > 2 {
            lines.truncate(2);
            let last = crate::kit::text::plain(&lines[1]);
            lines[1] = Line::from(Span::styled(
                clip(&format!("{last}…"), w.saturating_sub(2)),
                t.muted().add_modifier(Modifier::ITALIC),
            ));
        }
        for (i, l) in lines.into_iter().enumerate() {
            let lead = if i == 0 {
                Span::styled("✓ ", t.ok())
            } else {
                Span::raw("  ")
            };
            let mut spans = vec![lead];
            spans.extend(l.spans);
            out.push(Line::from(spans));
        }
    }
    out
}

fn files(m: &Model, w: usize, rows: usize) -> Lines {
    let t = &m.theme;
    let list = m.sidebar.files.list();
    let count = (!list.is_empty()).then(|| list.len().to_string());
    let mut out = vec![title(t, "files", t.gold(), count, w)];
    if list.is_empty() {
        out.push(Line::from(Span::styled("· nothing changed yet", t.muted())));
        return out;
    }
    let room = rows.saturating_sub(1).max(1);
    let (hidden, shown) = if list.len() > room {
        let keep = room.saturating_sub(1).max(1);
        (list.len() - keep, &list[list.len() - keep..])
    } else {
        (0, list)
    };
    if hidden > 0 {
        out.push(Line::from(Span::styled(
            format!("… +{hidden} earlier"),
            t.muted(),
        )));
    }
    for (path, mark) in shown {
        let style = match mark {
            Mark::Added => t.ok(),
            Mark::Modified => t.gold(),
            Mark::Deleted => t.alarm(),
        };
        out.push(Line::from(vec![
            Span::styled(mark.letter(), style.add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(clip_left(&one_line(path), w.saturating_sub(2)), t.text()),
        ]));
    }
    out
}

fn bodies(m: &Model, w: usize, rows: usize) -> Lines {
    let t = &m.theme;
    let all = &m.sidebar.bodies;
    let count = match (all.list().len(), all.live()) {
        (0, _) => None,
        (seen, 0) => Some(seen.to_string()),
        (seen, live) => Some(format!("{seen} · {live} live")),
    };
    let mut out = vec![title(t, "bodies", t.rimuru(), count, w)];
    let me = one_line(&m.header.body);
    let race = |name: &str| m.sidebar.races.get(name).copied();
    let state = if m.busy() {
        vec![Span::styled(
            format!("{} working", m.spinner.view()),
            t.holo(),
        )]
    } else {
        vec![Span::styled("idle", t.muted())]
    };
    out.push(row(
        vec![
            Span::styled("◆ ", t.race(race(&m.header.body).unwrap_or(Race::Rimuru))),
            Span::styled(me, t.text().add_modifier(Modifier::BOLD)),
        ],
        state,
        w,
    ));
    let room = rows.saturating_sub(2);
    let hidden = all.list().len().saturating_sub(room);
    let shown = if hidden > 0 {
        room.saturating_sub(1)
    } else {
        all.list().len()
    };
    for (depth, body) in all.walk().take(shown) {
        let hue = race(&body.name).map_or_else(|| t.frame(), |r| t.race(r));
        let (dot, name) = match body.state {
            State::Working => (hue, t.text()),
            _ => (t.line(), t.muted()),
        };
        let right = match body.state {
            State::Working => vec![Span::styled(
                format!(
                    "{} {}",
                    m.spinner.view(),
                    clip(&one_line(body.doing.as_deref().unwrap_or("working")), 10)
                ),
                hue,
            )],
            State::Done => vec![Span::styled("✓", t.ok())],
            State::Stopped => vec![Span::styled("◌", t.muted())],
            State::Failed => vec![Span::styled("✗", t.alarm())],
        };
        let indent = "  ".repeat(depth.min(4));
        out.push(row(
            vec![
                Span::raw(indent),
                Span::styled("● ", dot),
                Span::styled(one_line(&body.name), name),
            ],
            right,
            w,
        ));
    }
    if hidden > 0 {
        out.push(Line::from(Span::styled(
            format!("… +{} more", hidden + 1),
            t.muted(),
        )));
    }
    out
}

/// The sidebar's lines, `w` cells wide and at most `h` rows tall: the session and the memory
/// whole, the bodies and the files sharing the rows left.
#[must_use]
pub fn view(m: &Model, w: usize, h: usize) -> Lines {
    let gap = || Line::from("");
    let head = session(m, w);
    let mem = memory(m, w);
    let fixed = head.len() + mem.len() + 3;
    let left = h.saturating_sub(fixed);
    // Each wants a row a path or a body, and its title; short of room, the bodies keep at least
    // half of what is left, and the files take the rest, the latest last.
    let need_files = m.sidebar.files.list().len().max(1) + 1;
    let need_bodies = m.sidebar.bodies.list().len() + 2;
    let body_rows = if need_files + need_bodies <= left {
        need_bodies
    } else {
        need_bodies.min((left / 2).max(3))
    };
    let file_rows = left.saturating_sub(body_rows).max(2);
    let mut out = head;
    out.push(gap());
    out.extend(mem);
    out.push(gap());
    out.extend(files(m, w, file_rows));
    out.push(gap());
    out.extend(bodies(m, w, body_rows));
    out.truncate(h);
    out
}
