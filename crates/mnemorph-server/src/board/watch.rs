//! The board's push channel: a watcher over every listed world's files, and `WS /ws/board`, which
//! tells each tab `world/changed {id}` when one moved — the page then refetches that world's
//! overview — and `worlds/changed` when the list itself did.
//!
//! The watcher polls, and only while a tab listens: every [`FAST`] while a session is live in some
//! world (a board chat, or a run lock held), else every [`SLOW`]. Each look takes every world's
//! signature — the size and write time of `log.md`, the run journals and their locks, the memory
//! tiers, the facts journal, the creature docs and the usage ledger in the world, and of the
//! challenge records and gate verdicts under the state directory — within a budget of [`BUDGET`]
//! entries a world. A world past it is
//! *slowed*: looked at every [`SLOWED`], and its overview says so. While looking fast, a world
//! whose signature moved is told once it holds still for a look (a burst of writes is one push),
//! or after [`SETTLE`] at the latest; looking slow, it is told at once. The list of worlds is read
//! again only when the registry or the human's list is written. Polling needs no platform's
//! notification API, and a world cannot exhaust a watch table by making directories.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant, UNIX_EPOCH};

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use serde_json::json;
use tokio::sync::broadcast;

use mnemorph_loop::world::facts::FACTS;
use mnemorph_loop::world::log::LOG;
use mnemorph_loop::world::queue::{QUEUE, witness_of};
use mnemorph_loop::world::races::Race;
use mnemorph_loop::world::{challenge, verdicts, worth::USAGE};

use super::{Board, guard, refuse};

/// How often the watcher looks while a session is live somewhere.
pub const FAST: Duration = Duration::from_millis(250);

/// How often it looks otherwise.
pub const SLOW: Duration = Duration::from_secs(2);

/// How often a world past its budget is looked at.
pub const SLOWED: Duration = Duration::from_secs(10);

/// The longest a moving world waits before it is told anyway.
pub const SETTLE: Duration = Duration::from_millis(600);

/// The most entries — files stat'ed, directory entries read, locks probed — one look spends on
/// one world.
pub const BUDGET: usize = 2000;

/// The subprotocol `WS /ws/board` speaks, offered beside the CSRF entry.
pub const SUBPROTOCOL: &str = "mnemorph.board.v1";

/// The most board sockets open at once.
pub const SOCKETS: usize = 16;

/// The largest message a tab may send (it sends none but a close).
const INBOUND: usize = 1024;

/// What the watcher tells the tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A world's files moved: its id.
    World(String),
    /// The list of worlds moved: one added, forgotten or opened for the first time.
    Worlds,
}

impl Change {
    /// The notification, as JSON-RPC 2.0 without an id.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Change::World(id) => {
                json!({"jsonrpc": "2.0", "method": "world/changed", "params": {"id": id}})
            }
            Change::Worlds => json!({"jsonrpc": "2.0", "method": "worlds/changed"}),
        }
        .to_string()
    }
}

/// A moving world's state between looks.
struct Moving {
    sig: u64,
    since: Instant,
}

/// What one look found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Look {
    /// What to tell.
    pub changes: Vec<Change>,
    /// Whether a session holds a run in some world.
    pub live: bool,
    /// The worlds past their budget.
    pub slowed: BTreeSet<String>,
}

/// The watcher's memory between looks.
#[derive(Default)]
pub struct Watcher {
    seen: BTreeMap<String, u64>,
    moving: BTreeMap<String, Moving>,
    /// The list's signature, and the worlds read for it.
    list: Option<(u64, Vec<(String, PathBuf)>)>,
    /// The slowed worlds, and when each was last looked at.
    slowed: BTreeMap<String, Instant>,
    /// The worlds held live at the last look.
    live: BTreeSet<String>,
}

/// One file's size and write time, folded into `h`, its absence too — a link not followed —
/// within `budget`; `false` when the budget is spent.
fn file(h: &mut DefaultHasher, path: &Path, budget: &mut usize) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    path.hash(h);
    match std::fs::symlink_metadata(path) {
        Ok(m) => {
            m.len().hash(h);
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .hash(h);
        }
        Err(_) => 0u8.hash(h),
    }
    true
}

/// Every entry of `at`, in name order, folded into `h` within `budget` (each entry read spends
/// one); `deep` follows one level of subdirectories, never a link. `false` when the budget ran out.
fn dir(h: &mut DefaultHasher, at: &Path, deep: bool, budget: &mut usize) -> bool {
    let Ok(rd) = std::fs::read_dir(at) else {
        0u8.hash(h);
        return true;
    };
    let mut entries: Vec<(PathBuf, bool)> = Vec::new();
    for e in rd {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        let Ok(e) = e else { continue };
        entries.push((e.path(), e.file_type().is_ok_and(|t| t.is_dir())));
    }
    entries.sort();
    for (p, is_dir) in entries {
        let whole = if deep && is_dir {
            dir(h, &p, false, budget)
        } else {
            file(h, &p, budget)
        };
        if !whole {
            return false;
        }
    }
    true
}

/// A world's signature — what moves when anything the overview reads moves — and whether it was
/// taken whole within `budget`.
#[must_use]
pub fn signature(world: &Path, budget: &mut usize) -> (u64, bool) {
    let mut h = DefaultHasher::new();
    world.is_dir().hash(&mut h);
    let mut whole = file(&mut h, &world.join(LOG), budget)
        && file(&mut h, &world.join(USAGE), budget)
        && dir(
            &mut h,
            &mnemorph_loop::resume::journals(world),
            false,
            budget,
        )
        && dir(&mut h, &world.join(".mnemorph/memory"), true, budget)
        && file(&mut h, &world.join(QUEUE), budget)
        && file(&mut h, &world.join(FACTS), budget);
    // The creature docs, `.mnemorph/<race>/<name>/README.md`: the colony the bodies page draws.
    for race in Race::ALL {
        whole = whole
            && dir(
                &mut h,
                &world.join(".mnemorph").join(race.name()),
                true,
                budget,
            );
    }
    for trusted in [
        verdicts::verdicts_of(world),
        challenge::records_of(world),
        witness_of(world),
    ]
    .into_iter()
    .flatten()
    {
        whole = whole && file(&mut h, &trusted, budget);
    }
    (h.finish(), whole)
}

impl Watcher {
    /// One look, `now`: the list's files `list` signed, its worlds read with `load` only when
    /// they moved; each world signed within [`BUDGET`]. `debounce` holds a moving world until it
    /// is still for a look. A world seen for the first time is remembered, not told — its tab
    /// fetched it already; a world gone from the list is forgotten.
    pub fn look(
        &mut self,
        list: &[&Path],
        load: impl FnOnce() -> Vec<(String, PathBuf)>,
        now: Instant,
        debounce: bool,
    ) -> Look {
        let mut out = Look::default();
        let mut h = DefaultHasher::new();
        let mut unlimited = usize::MAX;
        for p in list {
            file(&mut h, p, &mut unlimited);
        }
        let list_sig = h.finish();
        match &self.list {
            Some((s, _)) if *s == list_sig => {}
            was => {
                if was.is_some() {
                    out.changes.push(Change::Worlds);
                }
                self.list = Some((list_sig, load()));
            }
        }
        let worlds = self
            .list
            .as_ref()
            .map(|(_, w)| w.clone())
            .unwrap_or_default();
        let ids: BTreeSet<&str> = worlds.iter().map(|(id, _)| id.as_str()).collect();
        self.seen.retain(|id, _| ids.contains(id.as_str()));
        self.moving.retain(|id, _| ids.contains(id.as_str()));
        self.slowed.retain(|id, _| ids.contains(id.as_str()));
        self.live.retain(|id| ids.contains(id.as_str()));
        for (id, path) in &worlds {
            if self
                .slowed
                .get(id)
                .is_some_and(|at| now.duration_since(*at) < SLOWED)
            {
                out.slowed.insert(id.clone());
                continue;
            }
            let mut budget = BUDGET;
            let (sig, whole) = signature(path, &mut budget);
            let held = !mnemorph_loop::resume::live_within(path, budget).is_empty();
            if held {
                self.live.insert(id.clone());
            } else {
                self.live.remove(id);
            }
            if whole {
                self.slowed.remove(id);
            } else {
                self.slowed.insert(id.clone(), now);
                out.slowed.insert(id.clone());
            }
            let Some(&was) = self.seen.get(id) else {
                self.seen.insert(id.clone(), sig);
                continue;
            };
            if sig != was {
                self.seen.insert(id.clone(), sig);
                let since = self.moving.get(id).map_or(now, |m| m.since);
                if !debounce || now.duration_since(since) >= SETTLE {
                    self.moving.remove(id);
                    out.changes.push(Change::World(id.clone()));
                } else {
                    self.moving.insert(id.clone(), Moving { sig, since });
                }
            } else if self.moving.get(id).is_some_and(|m| m.sig == sig) {
                // Still for a look: the burst is over.
                self.moving.remove(id);
                out.changes.push(Change::World(id.clone()));
            }
        }
        out.live = !self.live.is_empty();
        out
    }
}

/// Watch `board`'s worlds until the runtime stops, telling each change on its channel — looking
/// only while a tab listens, fast while a session is live, slow otherwise. The first look after
/// the watcher rested tells `worlds/changed`: a write between a tab's first read and that look is
/// never lost.
pub async fn watch(board: Arc<Board>) {
    let mut w = Some(Watcher::default());
    let mut pause = SLOW;
    // A fresh watcher's first look only remembers: what moved between a tab's first read and that
    // look would be told to no one, so that look tells every tab to read again, once.
    let mut fresh = true;
    loop {
        tokio::time::sleep(pause).await;
        if board.sockets.load(Ordering::SeqCst) == 0 {
            // No one listens: nothing is looked at, and what was seen is forgotten — a tab that
            // comes reads the overview fresh.
            w = Some(Watcher::default());
            fresh = true;
            pause = SLOW;
            continue;
        }
        let b = Arc::clone(&board);
        let mut watcher = w.take().unwrap_or_default();
        let debounce = pause == FAST;
        let Ok((back, look)) = tokio::task::spawn_blocking(move || {
            let list = [b.registry.as_path(), b.hand.as_path()];
            let load = || {
                b.worlds()
                    .worlds
                    .into_iter()
                    .map(|w| (w.id, PathBuf::from(w.path)))
                    .collect()
            };
            let look = watcher.look(&list, load, Instant::now(), debounce);
            (watcher, look)
        })
        .await
        else {
            return;
        };
        w = Some(back);
        let chats = !board.bridge().list().sessions.is_empty();
        pause = if look.live || chats { FAST } else { SLOW };
        *board.slowed.lock().unwrap_or_else(PoisonError::into_inner) = look.slowed;
        let changes = if std::mem::take(&mut fresh) {
            vec![Change::Worlds]
        } else {
            look.changes
        };
        for c in changes {
            // No tab listening is no error.
            let _ = board.changes.send(c);
        }
    }
}

/// Whether `headers` offer the board's subprotocol.
fn offers(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|p| p.trim() == SUBPROTOCOL)
}

/// `WS /ws/board`: the push channel. The guard has checked the Host, the Origin, the cookie and
/// the CSRF subprotocol entry; the socket speaks [`SUBPROTOCOL`], and only the server talks.
pub(crate) async fn socket(
    State(board): State<Arc<Board>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !offers(&headers) {
        return refuse(
            StatusCode::BAD_REQUEST,
            "the board's socket speaks the subprotocol mnemorph.board.v1",
        );
    }
    let Some(cookie) = guard::cookie(&headers)
        .find(|c| board.alive(c))
        .map(str::to_owned)
    else {
        return refuse(StatusCode::UNAUTHORIZED, "no session");
    };
    let open = board.sockets.fetch_add(1, Ordering::SeqCst);
    if open >= SOCKETS {
        board.sockets.fetch_sub(1, Ordering::SeqCst);
        return refuse(
            StatusCode::TOO_MANY_REQUESTS,
            "too many board sockets are open",
        );
    }
    let changes = board.changes.subscribe();
    upgrade
        .protocols([SUBPROTOCOL])
        .max_message_size(INBOUND)
        .max_frame_size(INBOUND)
        .on_upgrade(move |ws| async move {
            serve(&board, &cookie, ws, changes).await;
            board.sockets.fetch_sub(1, Ordering::SeqCst);
        })
}

/// Tell one tab every change until it leaves, its session ends or the board stops. A tab that
/// stops reading is dropped, never waited on: a send that takes longer than the chats' send limit
/// ends it, and so do two pings unanswered at the chats' ping pace.
async fn serve(
    board: &Board,
    cookie: &str,
    mut ws: WebSocket,
    mut changes: broadcast::Receiver<Change>,
) {
    let limits = board.bridge().limits();
    let (limit, pace) = (limits.send, limits.ping);
    let mut every = tokio::time::interval(pace);
    every.tick().await;
    let mut unanswered = 0u8;
    let why = loop {
        tokio::select! {
            change = changes.recv() => {
                let line = match change {
                    Ok(c) => c.line(),
                    // Fell behind: everything may have moved.
                    Err(broadcast::error::RecvError::Lagged(_)) => Change::Worlds.line(),
                    Err(broadcast::error::RecvError::Closed) => break "the board stopped",
                };
                if !board.alive(cookie) {
                    break "the session ended";
                }
                let sent = tokio::time::timeout(limit, ws.send(Message::Text(line.into()))).await;
                if !matches!(sent, Ok(Ok(()))) {
                    return;
                }
            }
            _ = every.tick() => {
                if !board.alive(cookie) {
                    break "the session ended";
                }
                if unanswered >= 2 {
                    return;
                }
                unanswered += 1;
                let ping = ws.send(Message::Ping(axum::body::Bytes::new()));
                if !matches!(tokio::time::timeout(limit, ping).await, Ok(Ok(()))) {
                    return;
                }
            }
            inbound = ws.recv() => match inbound {
                None | Some(Err(_) | Ok(Message::Close(_))) => return,
                Some(Ok(Message::Pong(_))) => unanswered = 0,
                // A ping is answered by the socket; a tab has nothing else to say.
                Some(Ok(Message::Ping(_))) => {}
                Some(Ok(_)) => break "the board's socket only speaks",
            },
        }
    };
    let close = ws.send(Message::Close(Some(CloseFrame {
        code: 4001,
        reason: why.into(),
    })));
    let _ = tokio::time::timeout(limit, close).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Vec<Change> = Vec::new();

    /// One look at `worlds`, with `list` the list's file; only the changes.
    fn changes(
        w: &mut Watcher,
        worlds: &[(String, PathBuf)],
        list: &Path,
        now: Instant,
        debounce: bool,
    ) -> Vec<Change> {
        w.look(&[list], || worlds.to_vec(), now, debounce).changes
    }

    #[test]
    fn a_burst_is_told_once_when_it_holds_still() {
        let root = tempfile::tempdir().unwrap();
        let world = root.path().join("w");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        let list = root.path().join("worlds.jsonl");
        let worlds = vec![("w-1".to_owned(), world.clone())];
        let mut w = Watcher::default();
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut look =
            |ms: u64, worlds: &[(String, PathBuf)]| changes(&mut w, worlds, &list, at(ms), true);
        // First sight: remembered, not told.
        assert_eq!(look(0, &worlds), NONE);
        assert_eq!(look(250, &worlds), NONE);
        std::fs::write(world.join(LOG), "a").unwrap();
        assert_eq!(look(500, &worlds), NONE, "moving");
        std::fs::write(world.join(LOG), "ab").unwrap();
        assert_eq!(look(750, &worlds), NONE, "still moving");
        assert_eq!(look(1000, &worlds), vec![Change::World("w-1".into())]);
        assert_eq!(look(1250, &worlds), NONE, "told once");

        // A world that never holds still is told after SETTLE.
        let mut told = Vec::new();
        for i in 0..6u64 {
            std::fs::write(world.join(LOG), "x".repeat(usize::try_from(i).unwrap() + 3)).unwrap();
            told.extend(look(1500 + i * 250, &worlds));
        }
        assert!(told.contains(&Change::World("w-1".into())), "{told:?}");

        // The list moving is told, and a world gone from it is forgotten.
        std::fs::write(&list, "{}\n").unwrap();
        assert_eq!(look(4000, &[]), vec![Change::Worlds]);
        assert!(w.seen.is_empty());
    }

    #[test]
    fn looking_slow_tells_at_once_and_the_list_is_read_only_when_it_moves() {
        let root = tempfile::tempdir().unwrap();
        let world = root.path().join("w");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        let list = root.path().join("worlds.jsonl");
        let worlds = vec![("w-1".to_owned(), world.clone())];
        let mut w = Watcher::default();
        let t0 = Instant::now();
        let mut loads = 0;
        for ms in [0, 2000] {
            w.look(
                &[&list],
                || {
                    loads += 1;
                    worlds.clone()
                },
                t0 + Duration::from_millis(ms),
                false,
            );
        }
        assert_eq!(loads, 1, "the registry is read again only when it moves");
        std::fs::write(world.join(LOG), "a").unwrap();
        assert_eq!(
            changes(&mut w, &worlds, &list, t0 + Duration::from_secs(4), false),
            vec![Change::World("w-1".into())]
        );
    }

    #[test]
    fn a_world_past_its_budget_is_slowed_and_said() {
        let root = tempfile::tempdir().unwrap();
        let world = root.path().join("w");
        let recalls = world.join(".mnemorph/memory/short");
        std::fs::create_dir_all(&recalls).unwrap();
        for i in 0..BUDGET {
            std::fs::write(recalls.join(format!("{i}")), "").unwrap();
        }
        let list = root.path().join("worlds.jsonl");
        let worlds = vec![("w-1".to_owned(), world.clone())];
        let mut budget = BUDGET;
        let (_, whole) = signature(&world, &mut budget);
        assert!(!whole);
        assert_eq!(budget, 0, "never past the budget");
        let mut w = Watcher::default();
        let t0 = Instant::now();
        let look = w.look(&[&list], || worlds.clone(), t0, true);
        assert!(look.slowed.contains("w-1"));
        // Within SLOWED it is not looked at: a write is not seen yet.
        std::fs::write(world.join(LOG), "a").unwrap();
        let look = w.look(
            &[&list],
            || worlds.clone(),
            t0 + Duration::from_secs(1),
            false,
        );
        assert_eq!(look.changes, NONE);
        assert!(look.slowed.contains("w-1"));
        let look = w.look(&[&list], || worlds.clone(), t0 + SLOWED, false);
        assert_eq!(look.changes, vec![Change::World("w-1".into())]);
    }

    #[test]
    fn a_held_run_makes_the_look_live() {
        let root = tempfile::tempdir().unwrap();
        let world = root.path().join("w");
        std::fs::create_dir_all(&world).unwrap();
        let list = root.path().join("worlds.jsonl");
        let worlds = vec![("w-1".to_owned(), world.clone())];
        let mut w = Watcher::default();
        let t0 = Instant::now();
        assert!(!w.look(&[&list], || worlds.clone(), t0, false).live);
        let hold = mnemorph_loop::resume::hold(&world, "r1730000000000-5").unwrap();
        assert!(w.look(&[&list], || worlds.clone(), t0, false).live);
        drop(hold);
        assert!(!w.look(&[&list], || worlds.clone(), t0, false).live);
    }

    #[test]
    fn each_source_moves_the_signature() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().join("w");
        std::fs::create_dir_all(&root).unwrap();
        let sig = |w: &Path| signature(w, &mut BUDGET.clone()).0;
        let mut last = sig(&root);
        for (path, text) in [
            (LOG, "x"),
            (USAGE, "{}"),
            (".mnemorph/instruments/loop/r1-2.jsonl", "{}"),
            (".mnemorph/memory/creature/slime.jsonl", "{}"),
            (".mnemorph/memory/shared/notes.jsonl", "{}"),
            (FACTS, "{}"),
            (".mnemorph/orc/orc-a/README.md", "# orc-a\n"),
            (".mnemorph/slime/slime-a/README.md", "# slime-a\n"),
        ] {
            let at = root.join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(&at, text).unwrap();
            let now = sig(&root);
            assert_ne!(now, last, "{path}");
            last = now;
        }
        std::fs::remove_dir_all(&root).unwrap();
        assert_ne!(sig(&root), last, "gone");
    }

    #[test]
    fn the_notifications_read_as_json_rpc() {
        assert_eq!(
            Change::World("a-1".into()).line(),
            r#"{"jsonrpc":"2.0","method":"world/changed","params":{"id":"a-1"}}"#
        );
        assert_eq!(
            Change::Worlds.line(),
            r#"{"jsonrpc":"2.0","method":"worlds/changed"}"#
        );
    }
}
