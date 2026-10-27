//! The human gate in the terminal: an act that asks waits on the UI's approval card, and only
//! a key the human presses there answers it. Silence denies after [`GATE_WAIT`]; so does Esc,
//! Ctrl+C, the turn ending, and the UI going away.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use mnemorph_loop::{Gate, GateAnswer, GateAsk, shown};
use mnemorph_tools::gate::Pending;

pub use mnemorph_server::GATE_WAIT;

use crate::app::Msg;

/// An approval as the card shows it: every field the model's choice reaches already passed
/// through [`shown`], so no character in it can move, hide or restyle what the human reads.
#[derive(Debug, Clone)]
pub struct Card {
    /// The body asking.
    pub body: String,
    /// The act's class (`outward`, `destructive`).
    pub class: String,
    /// The act, as it will run.
    pub act: String,
    /// Why it has its class.
    pub why: Vec<String>,
    /// Paths outside the world it opens.
    pub outside: Vec<String>,
    /// The plan an approval binds to, in short: the command runs only if the world still
    /// matches it when it is spawned.
    pub plan: Option<String>,
    /// Where the answer goes.
    pub pending: Arc<Pending>,
    /// When silence denies: the gate's own wait, from the moment it asked.
    pub deadline: Instant,
}

impl Card {
    /// The card of `ask`, its answer to come through `pending`.
    #[must_use]
    pub fn of(ask: &GateAsk, pending: Arc<Pending>) -> Self {
        let body = if ask.body.is_empty() {
            mnemorph_server::SESSION_BODY.to_owned()
        } else {
            ask.body.clone()
        };
        Self {
            body: shown(&body),
            class: shown(&ask.class.to_string()),
            act: shown(&ask.act),
            why: ask.why.iter().map(|w| shown(w)).collect(),
            outside: ask
                .outside
                .iter()
                .map(|p| shown(&p.display().to_string()))
                .collect(),
            plan: ask.fingerprint.as_ref().map(|f| shown(&f.short())),
            pending,
            deadline: Instant::now() + GATE_WAIT,
        }
    }

    /// Answer it; a second answer (the deadline came first) is ignored.
    pub fn answer(&self, answer: GateAnswer) {
        let _ = self.pending.answer(answer);
    }
}

/// The gate the session asks through: a card sent to the UI's loop, then a wait.
#[derive(Debug, Clone)]
pub struct TuiGate {
    tx: Sender<Msg>,
}

impl TuiGate {
    /// A gate that posts its cards to `tx`.
    #[must_use]
    pub fn new(tx: Sender<Msg>) -> Self {
        Self { tx }
    }
}

impl Gate for TuiGate {
    fn ask(&self, ask: &GateAsk) -> GateAnswer {
        let pending = Pending::new();
        if self
            .tx
            .send(Msg::Gate(Card::of(ask, Arc::clone(&pending))))
            .is_err()
        {
            return GateAnswer::Deny("the terminal UI is gone".into());
        }
        pending.wait(GATE_WAIT)
    }
}
