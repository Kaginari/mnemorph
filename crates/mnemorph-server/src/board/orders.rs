//! A world's shared work queue, as `GET /api/w/<id>/queue` shows it: every order, folded as
//! `mnemorphd q list --all` folds it — the open ones (held, claimed) by rank then arrival, then the
//! closed ones (done, dropped).
//!
//! The queue file lives inside the world (`.mnemorph/shared/queue.jsonl`): it is copied first as a
//! regular file under the cap, and the fold reads the copy; what it holds is shown and grants
//! nothing. Each line the harness or the human's `q` wrote is also written to the queue's witness
//! under the human's state directory, out of every command's reach: an item the witness holds
//! exactly as the queue does is marked witnessed, one it does not is marked unwitnessed — a line a
//! body wrote itself, which the tripwire drops.

use std::collections::BTreeMap;
use std::path::Path;

use mnemorph_loop::world::queue::{Item, QUEUE, Queue, State, witness_of};
use serde::Serialize;
use ts_rs::TS;

use super::snapshot;

/// The most characters of an order or a note shown.
pub const ASK_CHARS: usize = 2000;

/// An order's state, as `q list` prints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum OrderState {
    /// Waiting: nothing dispatches it mid-turn.
    Held,
    /// Claimed in the name of the body it is ordered to.
    Claimed,
    /// Its report said DONE.
    Done,
    /// Dropped.
    Dropped,
}

impl From<State> for OrderState {
    fn from(s: State) -> Self {
        match s {
            State::Held => OrderState::Held,
            State::Claimed => OrderState::Claimed,
            State::Done => OrderState::Done,
            State::Dropped => OrderState::Dropped,
        }
    }
}

/// One order on the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardOrder {
    /// Its id.
    pub id: String,
    /// Its revision.
    #[ts(type = "number")]
    pub rev: u64,
    /// Its state.
    pub state: OrderState,
    /// The body holding it: the body it is ordered to, else the throne.
    pub holder: String,
    /// Whether it was ordered to the throne, no body named.
    pub throne: bool,
    /// Who ordered it.
    pub by: String,
    /// Its rank: lower runs first.
    #[ts(type = "number")]
    pub rank: i64,
    /// The order, cut.
    pub ask: String,
    /// Its notes, in order: who moved it and why.
    pub notes: Vec<String>,
    /// Whether the witness holds it exactly so; `null` when there is no witness to ask.
    pub witnessed: Option<bool>,
}

/// How many orders stand in each state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OrderCounts {
    /// Held.
    pub held: u32,
    /// Claimed.
    pub claimed: u32,
    /// Done.
    pub done: u32,
    /// Dropped.
    pub dropped: u32,
}

/// `GET /api/w/<id>/queue`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardQueue {
    /// The open orders by rank then arrival, then the closed ones by id.
    pub orders: Vec<BoardOrder>,
    /// How many stand in each state.
    pub counts: OrderCounts,
    /// Where the queue was read: inside the world, shown only.
    pub source: String,
    /// Where its witness is kept, when there is a home; trusted.
    pub witness: Option<String>,
    /// Why the witness could not be asked, when it could not.
    pub witness_why: Option<String>,
}

/// `text` cut to `max` characters, with an ellipsis when cut.
fn cut(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}

/// The items of the queue journal at `file`, folded, and the open ones in their order.
fn fold(file: &Path) -> (BTreeMap<String, Item>, Vec<Item>) {
    let q = Queue::at(file);
    (q.items(), q.open())
}

/// `world`'s queue.
///
/// # Errors
///
/// The queue file is not a regular file, or is past the cap.
pub fn queue(world: &Path) -> Result<BoardQueue, String> {
    let snap = snapshot::file(world, QUEUE)?;
    let (items, open) = fold(&snap.path().join(QUEUE));
    let (witness, witness_why, held) = match witness_of(world) {
        None => (
            None,
            Some("no home: the queue's witness is kept under the human's state directory".into()),
            None,
        ),
        Some(at) => {
            let shown = at.display().to_string();
            if at.is_file() {
                (Some(shown), None, Some(fold(&at).0))
            } else {
                (
                    Some(shown),
                    Some("the witness holds nothing yet".into()),
                    Some(BTreeMap::new()),
                )
            }
        }
    };
    let order = |i: &Item| BoardOrder {
        id: i.id.clone(),
        rev: i.rev,
        state: i.state.into(),
        holder: i.holder().to_owned(),
        throne: i.to.is_none(),
        by: i.by.clone(),
        rank: i.rank,
        ask: cut(&i.ask, ASK_CHARS),
        notes: i.notes.iter().map(|n| cut(n, ASK_CHARS)).collect(),
        witnessed: held.as_ref().map(|h| h.get(&i.id) == Some(i)),
    };
    let mut orders: Vec<BoardOrder> = open.iter().map(order).collect();
    orders.extend(
        items
            .values()
            .filter(|i| matches!(i.state, State::Done | State::Dropped))
            .map(order),
    );
    let mut counts = OrderCounts::default();
    for o in &orders {
        let c = match o.state {
            OrderState::Held => &mut counts.held,
            OrderState::Claimed => &mut counts.claimed,
            OrderState::Done => &mut counts.done,
            OrderState::Dropped => &mut counts.dropped,
        };
        *c = c.saturating_add(1);
    }
    Ok(BoardQueue {
        orders,
        counts,
        source: QUEUE.to_owned(),
        witness,
        witness_why,
    })
}
