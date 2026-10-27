//! The shared work queue: the durable order book. One append-only file of item snapshots, each
//! with its revision; the queue is their fold. A line that does not follow its item's last
//! revision is refused on read, so two sessions racing for one move both append and the first
//! wins — no lock file, no database.
//!
//! The file is `.mnemorph/shared/queue.jsonl`, harness soil, ignored by git: the queue belongs to
//! one machine. Every line the harness or the human's `mnemorphd q` writes is written again to a
//! witness under the human's state directory, out of every command's reach; when a body's command
//! has touched the queue, the tripwire puts it back keeping only the lines the witness holds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mnemorph_ledger::Journal;
use mnemorph_wire::fnv64a;
use serde::{Deserialize, Serialize};

use crate::roster::Roster;

/// Where the queue lives in a world.
pub const QUEUE: &str = ".mnemorph/shared/queue.jsonl";

/// Where the queue's witness lives: `$XDG_STATE_HOME/mnemorph/queue/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` when there is no home. The tripwire reads it at the same
/// place (`mnemorph_tools::queue_witness`).
#[must_use]
pub fn witness_of(world: &Path) -> Option<PathBuf> {
    state_file(world, "queue")
}

/// A record the harness keeps for `world` outside it, under the human's state directory:
/// `$XDG_STATE_HOME/mnemorph/<kind>/<world>-<fnv64a>.jsonl`, else under `~/.local/state`; `None`
/// when there is no home.
pub(crate) fn state_file(world: &Path, kind: &str) -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".local/state")))?;
    let h = fnv64a(world.as_os_str().as_encoded_bytes());
    let name = world
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(
        state
            .join("mnemorph")
            .join(kind)
            .join(format!("{name}-{h:016x}.jsonl")),
    )
}

/// The human, as a mover: they stand over every body.
pub const HUMAN: &str = "human";

/// The throne: work held by no one is its.
pub const THRONE: &str = "rimuru";

/// The harness itself, as a mover: the wake claims in the name of the body an order is for.
pub const HARNESS: &str = "harness";

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
    /// An order may go only down the ranks: `by` does not stand over `to`.
    Upward {
        /// Who ordered.
        by: String,
        /// The body ordered.
        to: String,
    },
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
    /// Only the human and the harness claim: work reaches a body claimed in its name.
    NotAClaimer(String),
    /// No living body has that name.
    NoSuchBody(String),
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
            Refusal::Upward { by, to } => {
                write!(f, "{by} does not stand over {to}: orders go down the ranks")
            }
            Refusal::Raced => f.write_str("another move landed first: read the queue again"),
            Refusal::NotAClaimer(who) => write!(
                f,
                "{who} cannot claim: work reaches a body claimed in its name, by the human or the harness"
            ),
            Refusal::NoSuchBody(name) => write!(f, "no living body is named `{name}`"),
            Refusal::Io(e) => write!(f, "the queue cannot be written: {e}"),
        }
    }
}

/// The queue of one world.
pub struct Queue {
    journal: Journal<Item>,
    path: PathBuf,
    witness: Option<Journal<Item>>,
}

impl Queue {
    /// The queue of `world`, witnessed under the human's state directory when there is one.
    #[must_use]
    pub fn of(world: &Path) -> Queue {
        let queue = Queue::at(&world.join(QUEUE));
        match witness_of(world) {
            Some(w) => queue.witnessed_at(&w),
            None => queue,
        }
    }

    /// The queue kept in `file`, unwitnessed.
    #[must_use]
    pub fn at(file: &Path) -> Queue {
        Queue {
            journal: Journal::new(file.to_path_buf()),
            path: file.to_path_buf(),
            witness: None,
        }
    }

    /// The same queue, every line it writes witnessed in `file` too.
    #[must_use]
    pub fn witnessed_at(mut self, file: &Path) -> Queue {
        self.witness = Some(Journal::new(file.to_path_buf()));
        self
    }

    /// Every item as the fold reads it: a line that does not follow its item's last revision is
    /// refused.
    #[must_use]
    pub fn items(&self) -> BTreeMap<String, Item> {
        self.fold().0
    }

    /// The fold, and each item's place of arrival.
    fn fold(&self) -> (BTreeMap<String, Item>, BTreeMap<String, usize>) {
        let mut items: BTreeMap<String, Item> = BTreeMap::new();
        let mut arrival = BTreeMap::new();
        let lines = self.journal.read().map(|r| r.rows).unwrap_or_default();
        for (n, line) in lines.into_iter().enumerate() {
            let follows = items
                .get(&line.id)
                .map_or(line.rev == 1, |now| line.rev == now.rev + 1);
            if follows {
                arrival.entry(line.id.clone()).or_insert(n);
                items.insert(line.id.clone(), line);
            }
        }
        (items, arrival)
    }

    /// The open items — held or claimed — by rank, then by order of arrival.
    #[must_use]
    pub fn open(&self) -> Vec<Item> {
        let (items, arrival) = self.fold();
        let mut open: Vec<Item> = items
            .into_values()
            .filter(|i| matches!(i.state, State::Held | State::Claimed))
            .collect();
        open.sort_by_key(|i| (i.rank, arrival.get(&i.id).copied()));
        open
    }

    /// Add an order, held, for `to` (`None`: the throne's), by `by`. `to` must be a living body
    /// of `roster`, and below `by` unless the human or the harness orders.
    ///
    /// # Errors
    ///
    /// No such body, an order up the ranks, the queue cannot be written, or another add took the id first.
    pub fn add(
        &self,
        roster: &Roster,
        to: Option<&str>,
        by: &str,
        ask: &str,
        rank: i64,
    ) -> Result<Item, Refusal> {
        if let Some(to) = to.filter(|t| !living(roster, t)) {
            return Err(Refusal::NoSuchBody(to.to_owned()));
        }
        let over_all = by == HUMAN || by == HARNESS;
        let down = to.is_some_and(|t| roster.is_above(by, t));
        if !over_all && !down {
            return Err(Refusal::Upward {
                by: by.to_owned(),
                to: to.unwrap_or(THRONE).to_owned(),
            });
        }
        let taken = self.items();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let id = (0..u32::MAX)
            .map(|salt| {
                let h = fnv64a(format!("{stamp}\0{salt}\0{by}\0{ask}").as_bytes());
                format!("{h:016x}")[..12].to_owned()
            })
            .find(|id| !taken.contains_key(id))
            .unwrap_or_default();
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
    /// human and the harness stand over every body); the move must follow its state.
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
        let stands = mover == HUMAN
            || mover == HARNESS
            || mover == holder
            || roster.is_above(mover, &holder);
        if !stands {
            return Err(Refusal::NotYours { holder });
        }
        // A body never closes or drops its own order: its report settles what it ran, and only a
        // body above it may discharge what it holds.
        if matches!(mv, Move::Close { .. })
            && mover != HUMAN
            && mover != HARNESS
            && !roster.is_above(mover, &holder)
        {
            return Err(Refusal::NotYours { holder });
        }
        if let Move::Claim { as_body } = mv {
            if mover != HUMAN && mover != HARNESS {
                return Err(Refusal::NotAClaimer(mover.to_owned()));
            }
            if let Some(body) = as_body.as_deref().filter(|b| !living(roster, b)) {
                return Err(Refusal::NoSuchBody(body.to_owned()));
            }
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
        // A link at the queue or its shelf would carry the harness's write elsewhere.
        for p in [Some(self.path.as_path()), self.path.parent()]
            .into_iter()
            .flatten()
        {
            if std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(Refusal::Io(format!(
                    "{} is a link: the queue is not written through it",
                    p.display()
                )));
            }
        }
        // The witness first, under its lock: a line the queue holds and its witness does not would
        // be dropped by the next command's tripwire, and the tripwire rebuilds the queue under the
        // same lock, so no move lands between its read and its write.
        let held = match &self.witness {
            Some(w) => {
                let lock = lock(w.path()).map_err(|e| Refusal::Io(e.to_string()))?;
                w.append(&item).map_err(|e| Refusal::Io(e.to_string()))?;
                Some(lock)
            }
            None => None,
        };
        self.journal
            .append(&item)
            .map_err(|e| Refusal::Io(e.to_string()))?;
        drop(held);
        match self.items().get(&item.id) {
            Some(now) if *now == item => Ok(item),
            _ => Err(Refusal::Raced),
        }
    }

    /// Where the queue is.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

/// Hold the witness's lock: the file opened (made if need be) and locked whole until dropped.
fn lock(witness: &Path) -> std::io::Result<std::fs::File> {
    if let Some(dir) = witness.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(witness)?;
    f.lock()?;
    Ok(f)
}

/// Whether `name` is a living body of `roster`, or the throne.
fn living(roster: &Roster, name: &str) -> bool {
    name == THRONE || roster.living().any(|c| c.name == name)
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
