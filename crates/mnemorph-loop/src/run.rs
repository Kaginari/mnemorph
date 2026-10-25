//! One body's run: the six beats, until it answers or stops honestly.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mnemorph_ledger::{Journal, Prices, Timestamp, UsageRow, occupancy};
use mnemorph_provider::{
    Block, Caller, Event, Message, Request, Role, StopReason, Tool as ToolSpec, usage_row,
};
use mnemorph_races::Race;
use mnemorph_tools::{Context, Tool, call};

use crate::beats::{Beat, block_json, clean, conversation, push_user};
use crate::compact::{fold, fold_point, foldable, mark_cache};
use crate::decisions::{BENCH_POINT, Decisions, bench_ask, bench_read};
use crate::gate::{EndGate, Tree, snapshot};
use crate::model::Model;
use crate::sequence::validate;
use crate::stress::{Read, Signal, Stress, bench_of, muted};

/// Who runs: the body's name, race, model and system prompt.
#[derive(Debug, Clone)]
pub struct Body {
    /// The creature's name (`rimuru`, `slime-wire`).
    pub name: String,
    /// Its race.
    pub race: Race,
    /// The model that rides it.
    pub model: String,
    /// Its system prompt: the stable, cached prefix.
    pub system: String,
}

/// The meters a run reads before every model call.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Model turns.
    pub steps: u32,
    /// Wall clock.
    pub deadline: Duration,
    /// The model's context window, in tokens.
    pub window: u64,
    /// The context, in tokens, at which the run stops: absolute, so it means the same whatever
    /// the model; never past 90% of the window.
    pub stress_tokens: u64,
    /// The most a turn may write.
    pub max_tokens: u32,
    /// The carved budget, in weighted tokens, if any.
    pub budget_wt: Option<u64>,
}

impl Limits {
    /// A race's steps, a session's clock, a 200k window.
    #[must_use]
    pub fn for_race(race: Race) -> Self {
        let steps = match race {
            Race::Kijin | Race::Oni => 20,
            Race::Elf | Race::HighElf | Race::Orc | Race::HighOrc => 30,
            Race::NamedSlime => 60,
            Race::Rimuru | Race::Slime | Race::DarkElf | Race::ArchDarkElf => 40,
        };
        Self {
            steps,
            deadline: Duration::from_hours(1),
            window: 200_000,
            stress_tokens: STRESS_TOKENS,
            max_tokens: 8192,
            budget_wt: None,
        }
    }
}

/// What an observer sees of a run.
#[derive(Debug, Clone, PartialEq)]
pub enum Seen {
    /// A beat, as it is journaled.
    Beat(Beat),
    /// One model turn's usage, priced, with the run's running totals.
    Usage {
        /// The turn's tokens.
        usage: mnemorph_ledger::Usage,
        /// Its price, when the model's prices are known.
        usd: Option<f64>,
        /// Its weighted tokens.
        wt: u64,
        /// The run's weighted tokens so far.
        spent_wt: u64,
        /// How full the context was at this turn.
        occupancy: u64,
        /// The stress line the context is measured against.
        stress_line: u64,
    },
    /// What a commissioned body saw, below the run: its name, its run, and what it saw.
    From {
        /// The body.
        body: String,
        /// Its run.
        run: String,
        /// What it saw.
        seen: Box<Seen>,
    },
}

/// The send lane: lines the human sends a running body, delivered at its next tool step —
/// steering, never a new task.
pub type Steer = Arc<std::sync::Mutex<std::collections::VecDeque<String>>>;

/// How a sent line reaches the body.
pub const SENT: &str = "[the human, while you work]";

/// Sees a run live: called on the run's own task, so it must not block.
pub type Observer = Arc<dyn Fn(&Seen) + Send + Sync>;

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    /// The model answered.
    Answered(String),
    /// An honest stop: the journal holds every beat and `resume` continues from it.
    Checkpoint {
        /// Which meter or instrument stopped it.
        why: String,
        /// What the body wrote in its salvage turn, after a clock death.
        salvage: Option<String>,
    },
    /// Too many failed acts: one hop up.
    Escalate(String),
    /// The model or the provider failed.
    Failed(String),
    /// The human interrupted the turn (Esc).
    Interrupted,
}

impl Ending {
    fn words(&self) -> String {
        match self {
            Ending::Answered(_) => "answered".into(),
            Ending::Checkpoint { why, .. } => format!("checkpoint: {why}"),
            Ending::Escalate(why) => format!("escalate: {why}"),
            Ending::Failed(why) => format!("failed: {why}"),
            Ending::Interrupted => "interrupted".into(),
        }
    }
}

/// The human's Esc, shared with the interface: fired from any thread, seen by the run between
/// beats and during a model call.
#[derive(Debug, Default)]
pub struct Interrupt {
    fired: Arc<AtomicBool>,
    notify: tokio::sync::Notify,
}

impl Interrupt {
    /// The flag itself, for the tools: a running command stops when it is set.
    #[must_use]
    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.fired)
    }

    /// Interrupt the turn in progress.
    pub fn fire(&self) {
        self.fired.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// Take the interrupt: whether it was fired since it was last taken.
    pub fn take(&self) -> bool {
        self.fired.swap(false, Ordering::SeqCst)
    }

    /// Wait until it is fired.
    pub async fn fired(&self) {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.fired.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }
}

/// Why perceive stops a run.
enum Stop {
    /// End here.
    End(Ending),
    /// The clock: one salvage turn first.
    Salvage(&'static str),
}

/// The tokens a clock death's salvage turn may write.
pub const SALVAGE_TOKENS: u32 = 4096;
/// The share of the window the stress line may never pass (percent).
pub const CONTEXT_STRESS_PCT: u64 = 90;
/// The stress line, in tokens, by default.
pub const STRESS_TOKENS: u64 = 150_000;
/// The share of the stress line at which the body is told to write down what is not durable.
pub const NOTICE_PCT: u64 = 75;
/// The share of the stress line at which old tool outputs fold.
pub const COMPACT_PCT: u64 = 60;

/// One body's run.
pub struct Run {
    body: Body,
    limits: Limits,
    model: Arc<dyn Model>,
    tools: Vec<Box<dyn Tool>>,
    cx: Context,
    journal: Journal<Beat>,
    ledger: Option<Journal<UsageRow>>,
    prices: Prices,
    caller: Caller,
    messages: Vec<Message>,
    stress: Stress,
    steps: u32,
    ask_steps: u32,
    spent_wt: u64,
    last_occupancy: u64,
    started: Instant,
    gate: Option<EndGate>,
    before: Option<Tree>,
    interrupt: Arc<Interrupt>,
    folded_upto: usize,
    noticed: bool,
    decisions: Option<Decisions>,
    observer: Option<Observer>,
    caps: Option<mnemorph_ledger::Caps>,
    cap_warned: bool,
    steer: Steer,
    /// What runs this body's commissions, when it may commission.
    commissioner: Option<Arc<crate::commission::Commissioner>>,
    /// What this body's commissions landed under their own gates, for its own report.
    landed: Vec<PathBuf>,
    /// The interrupt is its dispatcher's.
    shared_interrupt: bool,
    /// What the wake ran since the last turn, for the next one.
    woken: Vec<String>,
    /// Whose shared queue this body's `queue` calls move: every commissioned body has one, a
    /// dispatching one or not.
    queue_keeper: Option<Arc<crate::commission::Commissioner>>,
}

/// What a run needs that is not the body itself.
pub struct Setup {
    /// The model.
    pub model: Arc<dyn Model>,
    /// The tools on the body's shelf.
    pub tools: Vec<Box<dyn Tool>>,
    /// The tools' context: world, soil, gate, sandbox.
    pub cx: Context,
    /// The run's beat journal.
    pub journal: Journal<Beat>,
    /// The usage ledger, if the run records spend.
    pub ledger: Option<Journal<UsageRow>>,
    /// Prices for the ledger.
    pub prices: Prices,
    /// Who calls, for the ledger.
    pub caller: Caller,
}

impl Run {
    /// A fresh run.
    #[must_use]
    pub fn new(body: Body, limits: Limits, setup: Setup) -> Self {
        let interrupt = Arc::new(Interrupt::default());
        Self {
            body,
            limits,
            model: setup.model,
            tools: setup.tools,
            cx: setup.cx.with_cancel(Arc::clone(&interrupt.fired)),
            journal: setup.journal,
            ledger: setup.ledger,
            prices: setup.prices,
            caller: setup.caller,
            messages: Vec::new(),
            stress: Stress::default(),
            steps: 0,
            ask_steps: 0,
            spent_wt: 0,
            last_occupancy: 0,
            started: Instant::now(),
            gate: None,
            before: None,
            interrupt,
            folded_upto: 0,
            noticed: false,
            decisions: None,
            observer: None,
            caps: None,
            cap_warned: false,
            steer: Steer::default(),
            commissioner: None,
            landed: Vec::new(),
            shared_interrupt: false,
            woken: Vec::new(),
            queue_keeper: None,
        }
    }

    /// The run may commission: its `commission` calls run a child body under `commissioner`.
    #[must_use]
    pub fn with_commissioner(mut self, commissioner: Arc<crate::commission::Commissioner>) -> Self {
        if self.queue_keeper.is_none() {
            self.queue_keeper = Some(Arc::clone(&commissioner));
        }
        self.commissioner = Some(commissioner);
        self
    }

    /// The body's `queue` calls move `keeper`'s shared queue in its name.
    #[must_use]
    pub fn with_queue(mut self, keeper: Arc<crate::commission::Commissioner>) -> Self {
        self.queue_keeper = Some(keeper);
        self
    }

    /// The paths this run's commissions landed under their own gates.
    #[must_use]
    pub fn courts(&self) -> Vec<PathBuf> {
        self.landed.clone()
    }

    /// The run answers to `interrupt`: a dispatcher's Esc stops the body it commissioned.
    #[must_use]
    pub fn with_interrupt(mut self, interrupt: Arc<Interrupt>) -> Self {
        self.cx.set_cancel(interrupt.flag());
        self.interrupt = interrupt;
        self.shared_interrupt = true;
        self
    }

    /// Weighted tokens the run has spent, its commissions' included.
    #[must_use]
    pub fn spent_wt(&self) -> u64 {
        self.spent_wt
    }

    /// The run with an observer: it sees every beat as it is journaled, and every model turn's
    /// usage, so a client can show the run live without reading the journal back.
    #[must_use]
    pub fn with_observer(mut self, observer: Observer) -> Self {
        self.observer = Some(observer);
        self
    }

    /// The send lane: push a line, and the body reads it after its next tool step.
    #[must_use]
    pub fn steer(&self) -> Steer {
        Arc::clone(&self.steer)
    }

    /// The run under the human's caps: before every call it rereads the usage ledger and stops
    /// at a checkpoint once the session, the day or the month is spent — across every session
    /// that shares the world — and warns once at 80%. A ledger it cannot read stops it.
    #[must_use]
    pub fn with_caps(mut self, caps: mnemorph_ledger::Caps) -> Self {
        self.caps = Some(caps);
        self
    }

    /// The run with its decisions: when `decisions.bench` is on, a piped test run's output is
    /// read by its decider, and each decision is journaled and priced.
    #[must_use]
    pub fn with_decisions(mut self, decisions: Decisions) -> Self {
        self.decisions = Some(decisions);
        self
    }

    /// A piped bench run's output, read: green or red when the decider is sure for its basis,
    /// nothing when it is unsure, fails, runs out of time, or the human presses Esc while it
    /// decides. Output the shell cut short is never read: its summary is in the part dropped.
    /// Every answer is journaled and priced, and a warning for the human says who read it.
    async fn read_bench(&mut self, command: &str, output: &str) -> Option<Read> {
        let d = self.decisions.as_ref()?;
        if output.contains(" more bytes of output dropped")
            || output.contains(" more bytes of stderr dropped")
        {
            self.record(&Beat::Warning {
                text: format!("[{BENCH_POINT}] `{command}` stays muted: its output was cut short"),
            });
            return None;
        }
        let (decider, cutoffs) = (Arc::clone(&d.decider), d.cutoffs);
        let interrupt = Arc::clone(&self.interrupt);
        // A decision is meant to take milliseconds: it never outlives the run's own clock.
        let left = (self.started + self.limits.deadline).saturating_duration_since(Instant::now());
        let budget = left.min(mnemorph_provider::SystemOne::policy().turn);
        let ask = bench_ask(command, output);
        let decision = tokio::select! {
            d = tokio::time::timeout(budget, decider.decide(&ask)) => {
                d.unwrap_or(Err(mnemorph_provider::Error::Timeout(mnemorph_provider::Wait::Turn)))
            }
            () = interrupt.fired() => return None,
        };
        let decision = match decision {
            Ok(d) => d,
            Err(e) => {
                self.record(&Beat::Warning {
                    text: format!(
                        "[{BENCH_POINT}] `{command}` stays muted: the decision failed: {e}"
                    ),
                });
                return None;
            }
        };
        if let Some(d) = &self.decisions
            && let Err(e) =
                d.journal
                    .append(&decision.row(&self.caller, BENCH_POINT, Timestamp::now()))
        {
            eprintln!("mnemorph: the decision journal could not be written: {e}");
        }
        // What the decision cost reaches the meter like any call: unpriced (`usd: null`) for a
        // model the price table does not know.
        let row = crate::decisions::decision_usage(&decision, &self.caller, &self.prices);
        self.spend(&row);
        let read = bench_read(&decision, cutoffs);
        let answer = decision.answer("outcome");
        self.record(&Beat::Warning {
            text: format!(
                "[{BENCH_POINT}] `{command}` read {} by {} ({}, {}: {} at {:.2}){}",
                match read {
                    Some(Read::Green) => "green",
                    Some(Read::Red) => "red",
                    None => "muted",
                },
                decision.source.provider,
                decision.source.model,
                decision.source.basis.name(),
                answer.map_or("—", |a| a.choice()),
                answer.map_or(0.0, mnemorph_provider::Answer::confidence),
                decision
                    .fell_back
                    .as_deref()
                    .map_or_else(String::new, |why| format!("; fell back: {why}")),
            ),
        });
        read
    }

    /// A run continued from its journal: the conversation rebuilt beat by beat, the steps it
    /// already took counted. An act the journal shows no answer for is answered with an error.
    ///
    /// # Errors
    ///
    /// The journal cannot be read.
    pub fn resume(body: Body, limits: Limits, setup: Setup) -> std::io::Result<Self> {
        let beats = setup.journal.read()?.rows;
        let mut run = Self::new(body, limits, setup);
        run.steps = u32::try_from(
            beats
                .iter()
                .filter(|b| matches!(b, Beat::Model { .. }))
                .count(),
        )
        .unwrap_or(u32::MAX);
        run.messages = conversation(&beats);
        // The cost and context meters read the ledger, so a budget or context checkpoint does
        // not resume into one more call it already could not afford.
        if let Some(ledger) = &run.ledger {
            let rows: Vec<UsageRow> = ledger
                .read()?
                .rows
                .into_iter()
                .filter(|r| r.run == run.caller.run)
                .collect();
            run.spent_wt = rows.iter().map(|r| r.wt).sum();
            run.last_occupancy = rows.last().map_or(0, |r| occupancy(r.usage));
        }
        Ok(run)
    }

    /// Judge every answer that changed files with this end gate.
    #[must_use]
    pub fn with_gate(mut self, gate: EndGate) -> Self {
        self.gate = Some(gate);
        self
    }

    /// The handle the interface fires on Esc.
    #[must_use]
    pub fn interrupt(&self) -> Arc<Interrupt> {
        Arc::clone(&self.interrupt)
    }

    /// The conversation so far.
    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Model turns taken.
    #[must_use]
    pub fn steps(&self) -> u32 {
        self.steps
    }

    /// The tools' context.
    pub fn context(&mut self) -> &mut Context {
        &mut self.cx
    }

    /// Count a call's spend: the run's meter, the observer, the usage ledger.
    fn spend(&mut self, row: &mnemorph_ledger::UsageRow) {
        self.spent_wt = self.spent_wt.saturating_add(row.wt);
        if let Some(see) = &self.observer {
            see(&Seen::Usage {
                usage: row.usage,
                usd: row.usd,
                wt: row.wt,
                spent_wt: self.spent_wt,
                occupancy: self.last_occupancy,
                stress_line: self.stress_line(),
            });
        }
        if let Some(ledger) = &self.ledger
            && let Err(e) = ledger.append(row)
        {
            eprintln!("mnemorph: the usage ledger could not be written: {e}");
        }
    }

    fn record(&self, beat: &Beat) {
        if let Some(see) = &self.observer {
            see(&Seen::Beat(beat.clone()));
        }
        // The journal is the run's memory across a crash: a failed append is reported, and the
        // run goes on — losing the journal must not also lose the work in flight.
        if let Err(e) = self.journal.append(beat) {
            eprintln!("mnemorph: the run's journal could not be written: {e}");
        }
    }

    /// What the human sent while the body worked, given to it now, after its tool step.
    fn deliver_steering(&mut self) {
        let sent: Vec<String> = self
            .steer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
            .collect();
        for line in sent {
            self.nudge(&format!("{SENT} {line}"));
        }
    }

    /// Lines sent to a turn that ended before a tool step: never smuggled into the next turn,
    /// and the human is told.
    fn drop_steering(&self) {
        let left: Vec<String> = self
            .steer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
            .collect();
        for line in left {
            self.record(&Beat::Warning {
                text: format!("not delivered — the turn ended before a tool step: {line}"),
            });
        }
    }

    fn end(&self, ending: Ending) -> Ending {
        // A line sent too late for this turn is said with this turn's end.
        self.drop_steering();
        self.record(&Beat::End {
            ending: ending.words(),
        });
        ending
    }

    /// The request for the next turn: the system prompt and the tools as the cached prefix.
    fn request(&self, max_tokens: u32) -> Request {
        let count = self.tools.len();
        let tools = self
            .tools
            .iter()
            .enumerate()
            .map(|(i, t)| ToolSpec {
                name: t.name().to_owned(),
                description: t.description().to_owned(),
                input_schema: t.schema(),
                cache: i + 1 == count,
            })
            .collect();
        let mut request = Request {
            model: self.body.model.clone(),
            system: vec![Block::Text {
                text: self.body.system.clone(),
                cache: true,
            }],
            tools,
            messages: fold(&self.messages, self.folded_upto),
            max_tokens,
            effort: None,
        };
        let budget = mnemorph_provider::MAX_BREAKPOINTS.saturating_sub(request.breakpoints());
        mark_cache(&mut request.messages, budget);
        request
    }

    /// Hand the body `text` and run until it answers or stops; `on` sees text and tool starts as
    /// they stream.
    pub async fn ask(&mut self, text: &str, on: &mut (dyn FnMut(Event) + Send)) -> Ending {
        let text = &self.preface(text);
        self.record(&Beat::Ask {
            text: text.to_owned(),
        });
        push_user(
            &mut self.messages,
            vec![Block::Text {
                text: text.to_owned(),
                cache: false,
            }],
        );
        if self.gate.is_some() {
            self.before = Some(snapshot(self.cx.world()));
        }
        // The step budget and the stress instruments are each commission's: a human ask is one.
        self.ask_steps = 0;
        self.stress = Stress::default();
        self.noticed = false;
        // The clock reaches every command of this ask, the gate's verify lines included.
        self.cx
            .set_deadline(Some(self.started + self.limits.deadline));
        // Only an Esc fired during this ask counts; a line sent to a turn that ended is not this
        // ask's to read. A commissioned body shares its dispatcher's flag: an Esc fired while the
        // commission was being prepared is this ask's.
        if !self.shared_interrupt {
            self.interrupt.take();
        }
        self.drop_steering();
        let mut gate_retried = false;
        loop {
            // Perceive: the meters, read from the core, never from the model.
            match self.perceive() {
                Some(Stop::End(ending)) => return self.end(ending),
                Some(Stop::Salvage(why)) => return self.salvage(why).await,
                None => {}
            }
            // Plan: one model call, inside what is left of the clock, unless the human interrupts.
            if self.interrupt.take() {
                return self.end(Ending::Interrupted);
            }
            if let Err(e) = validate(&self.messages) {
                return self.end(Ending::Failed(format!(
                    "the conversation is malformed: {e}"
                )));
            }
            self.ease_context();
            let request = self.request(self.limits.max_tokens);
            let left = self.limits.deadline.saturating_sub(self.started.elapsed());
            let interrupt = Arc::clone(&self.interrupt);
            let call = tokio::time::timeout(left, self.model.complete(&request, on));
            let answered = tokio::select! {
                // An answer that has already arrived is kept: Esc is then honoured before any act
                // runs. Without `biased`, a tie would be broken at random.
                biased;
                answered = call => Some(answered),
                () = interrupt.fired() => None,
            };
            let Some(answered) = answered else {
                // Nothing half-streamed enters the conversation: it still ends with the user.
                self.interrupt.take();
                return self.end(Ending::Interrupted);
            };
            let response = match answered {
                Err(_) => {
                    return self
                        .salvage("clock: the deadline passed during the model call")
                        .await;
                }
                Ok(Err(e)) => return self.end(Ending::Failed(e.to_string())),
                Ok(Ok(r)) => r,
            };
            let content = self.record_turn(&response);
            let calls: Vec<(String, String, String)> = content
                .iter()
                .filter_map(|b| match b {
                    Block::ToolUse { id, name, input } => {
                        Some((id.clone(), name.clone(), input.to_string()))
                    }
                    _ => None,
                })
                .collect();
            if calls.is_empty() {
                match response.stop {
                    StopReason::Refusal => {
                        let why = response.stop_details.map_or_else(
                            || "the model refused".to_owned(),
                            |d| format!("the model refused: {d}"),
                        );
                        return self.end(Ending::Failed(why));
                    }
                    // Not an answer: a reply cut by the output ceiling, a paused turn, an empty
                    // reply. The body is told and goes on, bounded by the step meter.
                    StopReason::MaxTokens | StopReason::PauseTurn => {
                        self.nudge("Your reply was cut short; continue where you stopped.");
                        continue;
                    }
                    _ if text_of(&response.content).trim().is_empty() => {
                        self.nudge("Your reply was empty. Answer the ask, or call a tool.");
                        continue;
                    }
                    _ => match self.judge(text_of(&content), &mut gate_retried) {
                        Some(ending) => return self.end(ending),
                        None => continue,
                    },
                }
            }
            // Act, verify, record.
            if let Some(ending) = self.act(calls).await {
                return self.end(ending);
            }
        }
    }

    /// Tell the body something in its own turn, and journal it.
    fn nudge(&mut self, text: &str) {
        self.record(&Beat::Note {
            text: text.to_owned(),
        });
        push_user(
            &mut self.messages,
            vec![Block::Text {
                text: text.to_owned(),
                cache: false,
            }],
        );
    }

    /// The end gate on an answer: the answer, a failure, or — the first time it fails — `None`,
    /// with the failure handed back to the body to fix.
    fn judge(&mut self, answer: String, retried: &mut bool) -> Option<Ending> {
        let (Some(gate), Some(before)) = (&self.gate, &self.before) else {
            return Some(Ending::Answered(answer));
        };
        let verdict = gate.judge(before, &answer, &self.tools, &mut self.cx);
        // A verify command Esc stopped is not a verdict: the ask ends, interrupted.
        if self.interrupt.take() {
            return Some(Ending::Interrupted);
        }
        self.record(&Beat::Gate {
            changed: verdict
                .changed
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            failures: verdict.failures.clone(),
        });
        if verdict.passed() {
            return Some(Ending::Answered(answer));
        }
        if *retried {
            return Some(Ending::Failed(format!(
                "the end gate failed twice: {}",
                verdict.failures.join("; ")
            )));
        }
        *retried = true;
        let note = format!(
            "The end gate failed:\n- {}\nFix it, then answer again.",
            verdict.failures.join("\n- ")
        );
        self.record(&Beat::Note { text: note.clone() });
        push_user(
            &mut self.messages,
            vec![Block::Text {
                text: note,
                cache: false,
            }],
        );
        None
    }

    /// The stress line: the configured tokens, never past 90% of the window.
    fn stress_line(&self) -> u64 {
        self.limits
            .stress_tokens
            .min(self.limits.window * CONTEXT_STRESS_PCT / 100)
    }

    /// Before a request, as the context fills: fold old tool outputs in one batch (so the cached
    /// prefix holds between folds), and once per ask tell the body to write down what is not
    /// durable while there is room.
    fn ease_context(&mut self) {
        let line = self.stress_line();
        if self.last_occupancy * 100 >= line * COMPACT_PCT {
            let point = fold_point(&self.messages);
            // A fold rewrites the cached conversation from its first folded message on, so it
            // happens only when it takes out a fifth of the line or more: rare, large batches,
            // with the prefix still between them.
            let worth = usize::try_from(line)
                .unwrap_or(usize::MAX)
                .saturating_mul(4)
                / 5;
            if point > self.folded_upto
                && foldable(&self.messages, self.folded_upto, point) >= worth
            {
                self.folded_upto = point;
                self.record(&Beat::Warning {
                    text: format!("compacted: tool outputs before message {point} are sent folded"),
                });
            }
        }
        if !self.noticed && self.last_occupancy * 100 >= line * NOTICE_PCT {
            self.noticed = true;
            self.nudge("[context] The context is filling. Write down now what is not durable — decisions, findings, what is left — so it survives a fresh start.");
        }
    }

    /// The meters, before a model call: a reason to stop, or none.
    fn perceive(&mut self) -> Option<Stop> {
        let checkpoint = |why: String| Some(Stop::End(Ending::Checkpoint { why, salvage: None }));
        if self.ask_steps >= self.limits.steps {
            return checkpoint(format!("steps: {} taken", self.ask_steps));
        }
        if self.started.elapsed() >= self.limits.deadline {
            return Some(Stop::Salvage("clock: the deadline passed between turns"));
        }
        let line = self.stress_line();
        if self.last_occupancy >= line {
            return checkpoint(format!(
                "context: {} of {line} tokens — start a fresh session, or end the turn and say what is left",
                self.last_occupancy
            ));
        }
        if let Some(budget) = self.limits.budget_wt.filter(|b| self.spent_wt >= *b) {
            return checkpoint(format!("budget: {} of {budget} spent", self.spent_wt));
        }
        if let Some(why) = self.over_a_cap() {
            return checkpoint(why);
        }
        if let Some(Signal::Stall(which)) = self.stress.turn() {
            return checkpoint(format!("stall: {which}"));
        }
        None
    }

    /// The human's caps against the ledger as it is now: why the run must stop, if it must.
    fn over_a_cap(&mut self) -> Option<String> {
        let caps = self.caps?;
        // Lines the ledger cannot read are counted high, never skipped: the opener told the human.
        let (rows, unseen) = match self.ledger.as_ref().map(Journal::read) {
            Some(Ok(back)) => {
                let unseen = mnemorph_ledger::unseen_wt(&back.rows, back.skipped.len());
                (back.rows, unseen)
            }
            Some(Err(e)) => {
                return Some(format!(
                    "budget: the usage ledger could not be read ({e}), and no call runs on a cap it cannot see"
                ));
            }
            None => (Vec::new(), 0),
        };
        match mnemorph_ledger::check_caps(
            &rows,
            &self.caller.session,
            Timestamp::now(),
            caps,
            unseen,
        ) {
            mnemorph_ledger::CapReading::Stop { cap, used, of } => Some(format!(
                "budget: the {cap} cap is spent — {used} of {of} wt"
            )),
            mnemorph_ledger::CapReading::Warn { cap, used, of } if !self.cap_warned => {
                self.cap_warned = true;
                self.record(&Beat::Warning {
                    text: format!("budget: {used} of the {cap} cap's {of} wt spent"),
                });
                None
            }
            _ => None,
        }
    }

    /// Count the turn, price it into the ledger, journal it, add it to the conversation.
    fn record_turn(&mut self, response: &mnemorph_provider::Response) -> Vec<Block> {
        self.steps += 1;
        self.ask_steps += 1;
        self.last_occupancy = occupancy(response.usage);
        let row = usage_row(
            response,
            &self.caller,
            self.model.provider(),
            &self.prices,
            Timestamp::now(),
        );
        self.spend(&row);
        let stop = match &response.stop {
            StopReason::Other(s) => s.clone(),
            other => format!("{other:?}"),
        };
        let content = clean(response.content.clone());
        self.record(&Beat::Model {
            content: content.iter().map(block_json).collect(),
            stop,
        });
        for w in &response.warnings {
            self.record(&Beat::Warning { text: w.clone() });
        }
        self.messages.push(Message {
            role: Role::Assistant,
            content: content.clone(),
        });
        content
    }

    /// What the wake ran while the body waited opens its next turn.
    fn preface(&mut self, text: &str) -> String {
        let woken = std::mem::take(&mut self.woken);
        if woken.is_empty() {
            text.to_owned()
        } else {
            format!(
                "While you waited, the shared queue ran:\n- {}\n\n{text}",
                woken.join("\n- ")
            )
        }
    }

    /// The wake, at a turn's end: each held order on the shared queue that its body can carry out
    /// is claimed in that body's name and commissioned by the body it reports to, the law checked
    /// again; an order its body cannot carry out stays held, the reason noted. What each one came
    /// to is said now and opens the next turn. A turn the human interrupted wakes nothing, an Esc
    /// during the wake stops it, and a spent budget leaves every order waiting.
    pub async fn wake(&mut self, after: &Ending) {
        use mnemorph_world::queue::{HARNESS, Move, State};
        if matches!(after, Ending::Interrupted) {
            return;
        }
        let Some(c) = self.commissioner.clone() else {
            return;
        };
        let Some(q) = c.queue.as_ref() else {
            return;
        };
        let roster = mnemorph_world::Roster::load(&c.world);
        for item in q.open().into_iter().filter(|i| i.state == State::Held) {
            if self.interrupt.flag().load(Ordering::SeqCst) {
                break;
            }
            let left = self
                .limits
                .budget_wt
                .map(|b| b.saturating_sub(self.spent_wt));
            if left == Some(0) {
                break;
            }
            let parent = match crate::commission::carrier(&c.world, &roster, &item, left) {
                Ok(parent) => parent,
                Err(why) => {
                    // Said once: an order that keeps waiting does not grow a note every turn.
                    if item.notes.last() != Some(&format!("{HARNESS}: {why}")) {
                        let _ = q.move_item(&roster, HARNESS, &item.id, &Move::Touch { note: why });
                    }
                    continue;
                }
            };
            let claim = Move::Claim { as_body: None };
            if q.move_item(&roster, HARNESS, &item.id, &claim).is_err() {
                continue;
            }
            let Some(race) = roster.creatures.get(&parent).map(|p| p.race) else {
                continue;
            };
            let body = Body {
                name: parent,
                race,
                model: self.body.model.clone(),
                system: String::new(),
            };
            let input = serde_json::json!({
                "to": item.holder(),
                "ask": format!("{}\n\n(queue item {}, ordered by {})", item.ask, item.id, item.by),
            })
            .to_string();
            let interrupt = Arc::clone(&self.interrupt);
            let order =
                crate::commission::order(&c, &body, left, &self.cx, &interrupt, &input).await;
            self.spent_wt = self.spent_wt.saturating_add(order.spent);
            self.landed.extend(order.landed);
            let line = crate::commission::settle(q, &roster, &item, &order.outcome);
            self.record(&Beat::Note { text: line.clone() });
            self.woken.push(line);
            if order.interrupted {
                break;
            }
        }
    }

    /// One call: a commission the loop runs, anything else the tool runs.
    async fn call_one(&mut self, name: &str, input: &str) -> mnemorph_tools::Outcome {
        // The shared queue is the harness's: the loop moves it in this body's name.
        if name == crate::queue_tool::QUEUE
            && self
                .tools
                .iter()
                .any(|t| t.name() == crate::queue_tool::QUEUE)
            && let Some(k) = &self.queue_keeper
            && let Some(q) = &k.queue
        {
            return crate::queue_tool::run(q, &k.world, &self.body.name, input);
        }
        match self.commissioner.clone() {
            // A commission runs a whole child body: the loop runs it, not the tool.
            Some(c) if name == crate::commission::COMMISSION => {
                let left = self
                    .limits
                    .budget_wt
                    .map(|b| b.saturating_sub(self.spent_wt));
                let interrupt = Arc::clone(&self.interrupt);
                let order =
                    crate::commission::order(&c, &self.body, left, &self.cx, &interrupt, input)
                        .await;
                self.spent_wt = self.spent_wt.saturating_add(order.spent);
                // Court work and what was put back are judged already: the baseline takes them,
                // so the dispatcher's gate sees only what changes after — an edit to them included.
                let world = self.cx.world().to_path_buf();
                let judged: Vec<PathBuf> = order
                    .landed
                    .iter()
                    .chain(&order.put_back)
                    .cloned()
                    .collect();
                if let Some(before) = self.before.as_mut() {
                    before.refresh(&world, &judged, &order.at);
                }
                self.landed.extend(order.landed);
                if order.interrupted {
                    self.interrupt.fire();
                }
                order.outcome
            }
            _ => call(&self.tools, name, input, &mut self.cx),
        }
    }

    /// Run each call in order, give every result back, and read the instruments: a reason to
    /// stop, or none.
    async fn act(&mut self, calls: Vec<(String, String, String)>) -> Option<Ending> {
        let mut results = Vec::new();
        let mut signals = Vec::new();
        let mut interrupted = false;
        for (id, name, input) in calls {
            // Esc between calls: the calls not yet run are answered, so the conversation stays
            // whole, and none of them runs.
            if interrupted || self.interrupt.take() {
                interrupted = true;
                let content = "interrupted by the human before it ran".to_owned();
                self.record(&Beat::Result {
                    tool_use_id: id.clone(),
                    content: content.clone(),
                    is_error: true,
                });
                results.push(Block::ToolResult {
                    tool_use_id: id,
                    content,
                    is_error: true,
                    cache: false,
                });
                continue;
            }
            self.cx
                .set_deadline(Some(self.started + self.limits.deadline));
            let o = self.call_one(&name, &input).await;
            // An Esc that stopped this call stops the calls after it too.
            interrupted |= self.interrupt.take();
            self.record(&Beat::Result {
                tool_use_id: id.clone(),
                content: o.content.clone(),
                is_error: o.is_error,
            });
            let command = (name == "bash")
                .then(|| serde_json::from_str::<serde_json::Value>(&input).ok())
                .flatten()
                .and_then(|v| {
                    v.get("command")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                });
            let read = match command.as_deref() {
                // An Esc that stopped the command is not followed by a decision about it, and a
                // run the shell saw fail — a non-zero exit, a kill at the deadline — is never
                // read: no text can turn a seen failure green.
                Some(c)
                    if !interrupted
                        && !o.is_error
                        && self.decisions.as_ref().is_some_and(|d| d.bench)
                        && bench_of(c).is_some()
                        && muted(c) =>
                {
                    let read = self.read_bench(c, &o.content).await;
                    interrupted |= self.interrupt.take();
                    read
                }
                _ => None,
            };
            signals.extend(self.stress.act_read(
                &format!("{name} {input}"),
                command.as_deref(),
                &o,
                read,
            ));
            results.push(Block::ToolResult {
                tool_use_id: id,
                content: o.content,
                is_error: o.is_error,
                cache: false,
            });
        }
        push_user(&mut self.messages, results);
        if interrupted {
            return Some(Ending::Interrupted);
        }
        self.deliver_steering();
        for s in signals {
            match s {
                Signal::Warn(text) => {
                    self.record(&Beat::Note { text: text.clone() });
                    push_user(&mut self.messages, vec![Block::Text { text, cache: false }]);
                }
                Signal::Doom => {
                    return Some(Ending::Checkpoint {
                        why: "doom: guts warned three windows running".into(),
                        salvage: None,
                    });
                }
                Signal::Escalate => {
                    return Some(Ending::Escalate(
                        "more than two failed acts in a row".into(),
                    ));
                }
                Signal::Stall(which) => {
                    return Some(Ending::Checkpoint {
                        why: format!("stall: {which}"),
                        salvage: None,
                    });
                }
            }
        }
        None
    }

    /// A clock death: one more short turn, without tools, to write down what the body learned —
    /// then the checkpoint, carrying it.
    async fn salvage(&mut self, why: &str) -> Ending {
        let note =
            "Time is up. In a few lines: what you did, what you learned, what is left. No tools."
                .to_owned();
        self.record(&Beat::Note { text: note.clone() });
        push_user(
            &mut self.messages,
            vec![Block::Text {
                text: note,
                cache: false,
            }],
        );
        let mut request = self.request(SALVAGE_TOKENS);
        request.tools.clear();
        let grace =
            (self.limits.deadline / 10).clamp(Duration::from_secs(2), Duration::from_secs(60));
        let mut quiet = |_: Event| {};
        let salvage =
            match tokio::time::timeout(grace, self.model.complete(&request, &mut quiet)).await {
                Ok(Ok(r)) => {
                    // The salvage call is a call like any other: metered and ledgered.
                    let row = usage_row(
                        &r,
                        &self.caller,
                        self.model.provider(),
                        &self.prices,
                        Timestamp::now(),
                    );
                    self.spend(&row);
                    self.record(&Beat::Model {
                        content: r.content.iter().map(block_json).collect(),
                        stop: "salvage".into(),
                    });
                    Some(text_of(&r.content))
                }
                _ => None,
            };
        self.end(Ending::Checkpoint {
            why: why.to_owned(),
            salvage,
        })
    }
}

fn text_of(content: &[Block]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            Block::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}
