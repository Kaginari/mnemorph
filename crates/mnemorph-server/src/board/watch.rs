//! The board's push channel: a watcher over every listed world's files, and `WS /ws/board`, which
//! tells each tab `world/changed {id}` when one moved — the page then refetches that world's
//! overview — and `worlds/changed` when the list itself did.
//!
//! The watcher polls: every [`TICK`] it takes each world's signature — the size and write time of
//! `log.md`, the run journals and their locks, the memory tiers and the usage ledger in the world,
//! and of the challenge records and gate verdicts under the state directory — and a world whose
//! signature moved is told once it holds still for a tick (the debounce: a burst of writes is one
//! push), or after [`SETTLE`] at the latest. Polling needs no platform's notification API, costs
//! a few `stat`s per world a tick, and a world cannot exhaust a watch table by making directories.
//! A world reads at most [`ENTRIES`] entries per directory.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, UNIX_EPOCH};

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use serde_json::json;
use tokio::sync::broadcast;

use mnemorph_loop::world::log::LOG;
use mnemorph_loop::world::{challenge, verdicts, worth::USAGE};

use super::{Board, guard, refuse};

/// How often the watcher looks.
pub const TICK: Duration = Duration::from_millis(150);

/// The longest a moving world waits before it is told anyway.
pub const SETTLE: Duration = Duration::from_millis(600);

/// The most entries read from one directory of a world.
pub const ENTRIES: usize = 4096;

/// The subprotocol `WS /ws/board` speaks, offered beside the CSRF entry.
pub const SUBPROTOCOL: &str = "mnemorph.board.v1";

/// The most board sockets open at once.
pub const SOCKETS: usize = 16;

/// The largest message a tab may send (it sends none but a close).
const INBOUND: usize = 1024;

/// How often a socket checks its session still lives.
const ALIVE: Duration = Duration::from_secs(15);

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

/// A moving world's state between ticks.
struct Moving {
    sig: u64,
    since: Instant,
}

/// The watcher's memory between ticks: each world's signature, and those still moving.
#[derive(Default)]
pub struct Watcher {
    seen: BTreeMap<String, u64>,
    moving: BTreeMap<String, Moving>,
    list: Option<u64>,
}

/// One file's size and write time, folded into `h`; its absence too.
fn file(h: &mut DefaultHasher, path: &Path) {
    path.hash(h);
    match std::fs::metadata(path) {
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
}

/// Every entry of `dir`, up to [`ENTRIES`], in name order, folded into `h`; `deep` follows one
/// level of subdirectories.
fn dir(h: &mut DefaultHasher, at: &Path, deep: bool) {
    let Ok(rd) = std::fs::read_dir(at) else {
        0u8.hash(h);
        return;
    };
    let mut paths: Vec<PathBuf> = rd
        .take(ENTRIES)
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    paths.sort();
    for p in paths {
        let sub = deep && std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir());
        if sub {
            dir(h, &p, false);
        } else {
            file(h, &p);
        }
    }
}

/// A world's signature: what moves when anything the overview reads moves.
#[must_use]
pub fn signature(world: &Path) -> u64 {
    let mut h = DefaultHasher::new();
    world.is_dir().hash(&mut h);
    file(&mut h, &world.join(LOG));
    file(&mut h, &world.join(USAGE));
    dir(&mut h, &mnemorph_loop::resume::journals(world), false);
    dir(&mut h, &world.join(".mnemorph/memory"), true);
    for trusted in [verdicts::verdicts_of(world), challenge::records_of(world)]
        .into_iter()
        .flatten()
    {
        file(&mut h, &trusted);
    }
    h.finish()
}

impl Watcher {
    /// One look at `worlds` (id and path) and the list's files, `now`: what to tell. A world seen
    /// for the first time is remembered, not told — its tab fetched it already; a world gone from
    /// the list is forgotten.
    pub fn tick(
        &mut self,
        worlds: &[(String, PathBuf)],
        list: &[&Path],
        now: Instant,
    ) -> Vec<Change> {
        let mut out = Vec::new();
        let mut h = DefaultHasher::new();
        for p in list {
            file(&mut h, p);
        }
        let list_sig = h.finish();
        if self.list.is_some_and(|s| s != list_sig) {
            out.push(Change::Worlds);
        }
        self.list = Some(list_sig);
        let ids: BTreeSet<&str> = worlds.iter().map(|(id, _)| id.as_str()).collect();
        self.seen.retain(|id, _| ids.contains(id.as_str()));
        self.moving.retain(|id, _| ids.contains(id.as_str()));
        for (id, path) in worlds {
            let sig = signature(path);
            let Some(&was) = self.seen.get(id) else {
                self.seen.insert(id.clone(), sig);
                continue;
            };
            if sig != was {
                self.seen.insert(id.clone(), sig);
                let since = self.moving.get(id).map_or(now, |m| m.since);
                if now.duration_since(since) >= SETTLE {
                    self.moving.remove(id);
                    out.push(Change::World(id.clone()));
                } else {
                    self.moving.insert(id.clone(), Moving { sig, since });
                }
            } else if self.moving.get(id).is_some_and(|m| m.sig == sig) {
                // Still for a tick: the burst is over.
                self.moving.remove(id);
                out.push(Change::World(id.clone()));
            }
        }
        out
    }
}

/// Watch `board`'s worlds until the runtime stops, telling each change on its channel.
pub async fn watch(board: Arc<Board>) {
    let mut w = Watcher::default();
    let mut every = tokio::time::interval(TICK);
    every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        every.tick().await;
        let b = Arc::clone(&board);
        let Ok((back, changes)) = tokio::task::spawn_blocking(move || {
            let worlds: Vec<(String, PathBuf)> = b
                .worlds()
                .worlds
                .into_iter()
                .map(|w| (w.id, PathBuf::from(w.path)))
                .collect();
            let list = [b.registry.as_path(), b.hand.as_path()];
            let changes = w.tick(&worlds, &list, Instant::now());
            (w, changes)
        })
        .await
        else {
            return;
        };
        w = back;
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

/// Tell one tab every change until it leaves, its session ends or the board stops.
async fn serve(
    board: &Board,
    cookie: &str,
    mut ws: WebSocket,
    mut changes: broadcast::Receiver<Change>,
) {
    let mut alive = tokio::time::interval(ALIVE);
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
                if ws.send(Message::Text(line.into())).await.is_err() {
                    return;
                }
            }
            _ = alive.tick() => {
                if !board.alive(cookie) {
                    break "the session ended";
                }
            }
            inbound = ws.recv() => match inbound {
                None | Some(Err(_) | Ok(Message::Close(_))) => return,
                // A ping is answered by the socket; a tab has nothing else to say.
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(_)) => break "the board's socket only speaks",
            },
        }
    };
    let _ = ws
        .send(Message::Close(Some(CloseFrame {
            code: 4001,
            reason: why.into(),
        })))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Vec<Change> = Vec::new();

    #[test]
    fn a_burst_is_told_once_when_it_holds_still() {
        let root = std::env::temp_dir().join(format!("mnemorph-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let world = root.join("w");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        let list = root.join("worlds.jsonl");
        let worlds = vec![("w-1".to_owned(), world.clone())];
        let mut w = Watcher::default();
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        // First sight: remembered, not told.
        assert_eq!(w.tick(&worlds, &[&list], at(0)), NONE);
        assert_eq!(w.tick(&worlds, &[&list], at(150)), NONE);
        std::fs::write(world.join(LOG), "a").unwrap();
        assert_eq!(w.tick(&worlds, &[&list], at(300)), NONE, "moving");
        std::fs::write(world.join(LOG), "ab").unwrap();
        assert_eq!(w.tick(&worlds, &[&list], at(450)), NONE, "still moving");
        assert_eq!(
            w.tick(&worlds, &[&list], at(600)),
            vec![Change::World("w-1".into())]
        );
        assert_eq!(w.tick(&worlds, &[&list], at(750)), NONE, "told once");

        // A world that never holds still is told after SETTLE.
        let mut told = Vec::new();
        for i in 0..8u64 {
            std::fs::write(world.join(LOG), "x".repeat(usize::try_from(i).unwrap() + 3)).unwrap();
            told.extend(w.tick(&worlds, &[&list], at(900 + i * 150)));
        }
        assert!(told.contains(&Change::World("w-1".into())), "{told:?}");

        // The list moving is told, and a world gone from it is forgotten.
        std::fs::write(&list, "{}\n").unwrap();
        let out = w.tick(&[], &[&list], at(3000));
        assert_eq!(out, vec![Change::Worlds]);
        assert!(w.seen.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn each_source_moves_the_signature() {
        let root = std::env::temp_dir().join(format!("mnemorph-sig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut last = signature(&root);
        for (path, text) in [
            (LOG, "x"),
            (USAGE, "{}"),
            (".mnemorph/instruments/loop/r1-2.jsonl", "{}"),
            (".mnemorph/memory/creature/slime.jsonl", "{}"),
            (".mnemorph/memory/shared/notes.jsonl", "{}"),
        ] {
            let at = root.join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(&at, text).unwrap();
            let now = signature(&root);
            assert_ne!(now, last, "{path}");
            last = now;
        }
        std::fs::remove_dir_all(&root).unwrap();
        assert_ne!(signature(&root), last, "gone");
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
