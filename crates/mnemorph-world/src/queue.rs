//! The shared work queue: the durable order book. One append-only file of item snapshots, each
//! with its revision; the queue is their fold. A line that does not follow its item's last
//! revision is refused on read, so two sessions racing for one move both append and the first
//! wins — no lock file, no database.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mnemorph_ledger::Journal;
use mnemorph_wire::fnv64a;
use serde::{Deserialize, Serialize};

use crate::roster::Roster;

/// Where the queue lives: the shared shelf, harness soil.
pub const QUEUE: &str = ".mnemorph/shared/queue.jsonl";

/// The human, as a mover: they stand over every body.
pub const HUMAN: &str = "human";

/// The throne: work held by no one is its.
pub const THRONE: &str = "rimuru";

/// An item's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Waiting: nothing dispatches it mid-turn.
    Held,
    /// Claimed in the name of the body it is ordered to.
    Claimed,
    /// Its report said DONE.
    Done,
    /// Dropped.
    Dropped,
}

impl State {
    /// The state, in words.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            State::Held => "held",
            State::Claimed => "claimed",
            State::Done => "done",
            State::Dropped => "dropped",
        }
    }
}

/// One item's snapshot: every move appends the whole of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Its id.
    pub id: String,
    /// Its revision: 1 when added, one more on every move.
    pub rev: u64,
    /// Its state.
    pub state: State,
    /// The body it is ordered to; `None` is the throne's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Who ordered it.
    pub by: String,
    /// Its rank: lower runs first.
    #[serde(default)]
    pub rank: i64,
    /// The order.
    pub ask: String,
    /// Notes, in order: who moved it and why.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl Item {
    /// The body holding it: the body it is ordered to, else the throne.
    #[must_use]
    pub fn holder(&self) -> &str {
        self.to.as_deref().unwrap_or(THRONE)
    }
}

/// A move the queue refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No item has that id.
    Unknown(String),
    /// The mover does not hold the item and does not stand over its holder.
    NotYours {
        /// The holder.
        holder: String,
    },
    /// The move does not follow the item's state.
    State {
        /// The item's state now.
        now: State,
        /// The move asked.
        wanted: &'static str,
    },
    /// Another move landed first.
    Raced,
    /// The queue file cannot be written.
    Io(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Unknown(id) => write!(f, "no item `{id}` on the queue"),
            Refusal::NotYours { holder } => write!(
                f,
                "{holder} holds it: it moves by {holder} or by anyone up {holder}'s chain"
            ),
            Refusal::State { now, wanted } => write!(f, "it is {}: it cannot {wanted}", now.word()),
            Refusal::Raced => f.write_str("another move landed first: read the queue again"),
            Refusal::Io(e) => write!(f, "the queue cannot be written: {e}"),
        }
    }
}

/// The queue of one world.
pub struct Queue {
    journal: Journal<Item>,
    world: PathBuf,
}

impl Queue {
    /// The queue of `world`.
    #[must_use]
    pub fn of(world: &Path) -> Queue {
        Queue {
            journal: Journal::new(world.join(QUEUE)),
            world: world.to_path_buf(),
        }
    }

    /// Every item as the fold reads it: a line that does not follow its item's last revision is
    /// refused.
    #[must_use]
    pub fn items(&self) -> BTreeMap<String, Item> {
        let mut items: BTreeMap<String, Item> = BTreeMap::new();
        for line in self.journal.read().map(|r| r.rows).unwrap_or_default() {
            let follows = items
                .get(&line.id)
                .map_or(line.rev == 1, |now| line.rev == now.rev + 1);
            if follows {
                items.insert(line.id.clone(), line);
            }
        }
        items
    }

    /// The open items — held or claimed — by rank, then by order of arrival.
    #[must_use]
    pub fn open(&self) -> Vec<Item> {
        let mut open: Vec<Item> = self
            .items()
            .into_values()
            .filter(|i| matches!(i.state, State::Held | State::Claimed))
            .collect();
        open.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.id.cmp(&b.id)));
        open
    }

    /// Add an order, held, for `to` (`None`: the throne's), by `by`.
    ///
    /// # Errors
    ///
    /// The queue cannot be written, or another add took the id first.
    pub fn add(&self, to: Option<&str>, by: &str, ask: &str, rank: i64) -> Result<Item, Refusal> {
        let n = self
            .journal
            .read()
            .map(|r| r.rows.len())
            .unwrap_or_default();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let id = format!(
            "{:016x}",
            fnv64a(format!("{stamp}\0{n}\0{by}\0{ask}").as_bytes())
        )[..8]
            .to_owned();
        let item = Item {
            id,
            rev: 1,
            state: State::Held,
            to: to.map(str::to_owned),
            by: by.to_owned(),
            rank,
            ask: ask.to_owned(),
            notes: Vec::new(),
        };
        self.land(item)
    }

    /// Move an item: `mover` must hold it or stand over its holder in `roster`'s chain (the
    /// human stands over every body); the move must follow its state.
    ///
    /// # Errors
    ///
    /// Unknown id, not the mover's, a move its state does not allow, a race, or the file.
    pub fn move_item(
        &self,
        roster: &Roster,
        mover: &str,
        id: &str,
        mv: &Move,
    ) -> Result<Item, Refusal> {
        let now = self
            .items()
            .remove(id)
            .ok_or_else(|| Refusal::Unknown(id.to_owned()))?;
        let holder = now.holder().to_owned();
        let stands = mover == HUMAN || mover == holder || roster.is_above(mover, &holder);
        if !stands {
            return Err(Refusal::NotYours { holder });
        }
        // A body never claims in another's name: only the human gives work to a body.
        if let Move::Claim {
            as_body: Some(other),
        } = mv
            && mover != HUMAN
            && *other != holder
        {
            return Err(Refusal::NotYours {
                holder: other.clone(),
            });
        }
        let mut next = now.clone();
        next.rev += 1;
        let (state, word) = match mv {
            Move::Claim { .. } => (State::Claimed, "be claimed"),
            Move::Touch { .. } => (now.state, "be touched"),
            Move::Requeue => (State::Held, "be requeued"),
            Move::Close { drop } => (
                if *drop { State::Dropped } else { State::Done },
                "be closed",
            ),
        };
        let allowed = match mv {
            Move::Claim { .. } => now.state == State::Held,
            Move::Touch { .. } => matches!(now.state, State::Held | State::Claimed),
            Move::Requeue => now.state == State::Claimed,
            Move::Close { .. } => matches!(now.state, State::Held | State::Claimed),
        };
        if !allowed {
            return Err(Refusal::State {
                now: now.state,
                wanted: word,
            });
        }
        next.state = state;
        if let Move::Claim {
            as_body: Some(body),
        } = mv
        {
            next.to = Some(body.clone());
        }
        let note = match mv {
            Move::Claim { .. } => format!("claimed by {mover}"),
            Move::Touch { note } => format!("{mover}: {note}"),
            Move::Requeue => format!("requeued by {mover}"),
            Move::Close { drop: true } => format!("dropped by {mover}"),
            Move::Close { drop: false } => format!("done, closed by {mover}"),
        };
        next.notes.push(note);
        self.land(next)
    }

    /// Append a snapshot and read it back: the fold decides whether it landed.
    fn land(&self, item: Item) -> Result<Item, Refusal> {
        self.journal
            .append(&item)
            .map_err(|e| Refusal::Io(e.to_string()))?;
        match self.items().get(&item.id) {
            Some(now) if *now == item => Ok(item),
            _ => Err(Refusal::Raced),
        }
    }

    /// Where the queue is.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.world.join(QUEUE)
    }
}

/// A move on an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Move {
    /// Claim it, in the name of `as_body` or the body it is ordered to.
    Claim {
        /// The body it is claimed for.
        as_body: Option<String>,
    },
    /// Add a note.
    Touch {
        /// The note.
        note: String,
    },
    /// Put a claimed item back on hold.
    Requeue,
    /// Close it: done, or dropped.
    Close {
        /// Dropped rather than done.
        drop: bool,
    },
}
