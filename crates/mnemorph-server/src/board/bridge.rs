//! The live chats. Each is a child `mnemorphd --world <dir>` on the unchanged protocol over its
//! stdio, bridged line for line to the tabs on `WS /ws/session/<sid>`:
//!
//! - **The child** starts in its own process group with the board's environment scrubbed of
//!   secrets, given back only the provider keys the human's global file names; it hardens itself.
//!   It is asked to leave when the chat ends or the board stops, and its group is killed after.
//! - **Events** go to every tab as the child wrote them, and are kept — 10,000 of them, under a
//!   byte cap — so a tab that reconnects asks `session/events {since}` and the bridge answers it in
//!   pages, `more` saying another follows.
//! - **Asks**: `gate/ask` is a request, not an event, so the bridge keeps each waiting ask and
//!   sends it again to a tab that connects, with its time left (`leftMs`). With no controller for
//!   the grace (60 s), the waiting asks are denied and the chat ends.
//! - **Roles**: one controller per chat, which alone may prompt, steer, interrupt, end the chat
//!   and answer the gate; every other tab observes. Control belongs to the browser that opened
//!   the chat (its board cookie): a tab of that browser takes it with `board/takeOver`.
//! - **Limits**: a line from the child longer than the cap is dropped and said; a tab whose queue
//!   fills is dropped (it reconnects and replays); a tab's message, the tabs per chat and the
//!   chats at once are capped.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::sync::{mpsc, oneshot, watch};
use ts_rs::TS;

use super::same;
use crate::protocol::{GateReply, PROTOCOL, REPLAY};
use crate::{GATE_WAIT, SESSION_BODY};

/// The subprotocol a tab's socket speaks; the CSRF entry rides beside it.
pub const SUBPROTOCOL: &str = "mnemorph.v1";

/// The id the bridge's own `initialize` carries.
const INIT: &str = "board-init";

/// The id the bridge's own `shutdown` carries.
const END: &str = "board-end";

/// The most bytes of a reason a gate answer carries to the child.
const REASON_BYTES: usize = 4096;

/// The most bytes of the child's refusal to open that reaches the browser.
const REFUSAL_BYTES: usize = 4096;

/// The longest request id a tab may use.
const ID_BYTES: usize = 128;

/// Lines waiting for the child's input.
const INPUT: usize = 256;

/// Close codes a tab reads: past these, it does not reconnect.
pub mod close {
    /// The chat ended (or the board stopped).
    pub const ENDED: u16 = 4001;
    /// The board session behind the cookie ended.
    pub const SIGNED_OUT: u16 = 4401;
    /// No such chat.
    pub const UNKNOWN: u16 = 4404;
    /// Too many tabs on the chat.
    pub const CROWDED: u16 = 4429;
    /// The tab fell too far behind and was dropped: it may reconnect and replay.
    pub const SLOW: u16 = 4008;
}

/// The bridge's bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Live chats at once.
    pub sessions: usize,
    /// Tabs on one chat at once.
    pub tabs: usize,
    /// The longest line read from a child; a longer one is dropped and said.
    pub line: usize,
    /// The bytes the replay keeps, beside its 10,000 events.
    pub replay_bytes: usize,
    /// The bytes of events one `session/events` answer carries before it says `more`.
    pub page: usize,
    /// The messages waiting for one tab before it is dropped.
    pub queue: usize,
    /// The bytes waiting for one tab before it is dropped.
    pub queue_bytes: usize,
    /// The largest message a tab may send.
    pub inbound: usize,
    /// The requests one tab may have waiting on the child.
    pub calls: usize,
    /// How long a chat waits with no controller before its asks are denied and it ends.
    pub grace: Duration,
    /// How long a child may take to open the session.
    pub open: Duration,
    /// How long a child may take to leave once asked, before its group is killed.
    pub leave: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            sessions: 8,
            tabs: 8,
            line: 4 << 20,
            replay_bytes: 64 << 20,
            page: 1 << 20,
            queue: 4096,
            queue_bytes: 16 << 20,
            inbound: 1 << 20,
            calls: 64,
            grace: Duration::from_secs(60),
            open: Duration::from_secs(60),
            leave: Duration::from_secs(10),
        }
    }
}

/// The arguments a child is started with, for a world and the run it resumes.
pub type Args = Arc<dyn Fn(&Path, Option<&str>) -> Vec<OsString> + Send + Sync>;

/// How a chat's child is started.
#[derive(Clone)]
pub struct Child {
    /// The program: this `mnemorphd`.
    pub program: PathBuf,
    /// Its arguments: `--world <dir>`, and `--resume <run>` for a past run.
    pub args: Args,
    /// The environment the child's is made from: the board's own.
    pub env: Vec<(String, String)>,
    /// The human's global configuration, where the provider keys are named.
    pub global: PathBuf,
}

impl Child {
    /// This process's program, environment and global configuration.
    #[must_use]
    pub fn this() -> Self {
        Self {
            program: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mnemorphd")),
            args: Arc::new(|world, resume| {
                let mut a: Vec<OsString> = vec!["--world".into(), world.into()];
                if let Some(run) = resume {
                    a.extend(["--resume".into(), run.into()]);
                }
                a
            }),
            env: std::env::vars_os()
                .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
                .collect(),
            global: mnemorph_loop::Paths::resolve(Path::new("/")).global,
        }
    }
}

/// The child's environment: `base` without its secrets, and without `MNEMORPH_BODY`, but for the
/// variables in `keys`, which it needs.
#[must_use]
pub fn child_env(base: &[(String, String)], keys: &[String]) -> Vec<(String, String)> {
    base.iter()
        .filter(|(k, v)| {
            k != "MNEMORPH_BODY" && (keys.contains(k) || !mnemorph_loop::is_secret(k, v))
        })
        .cloned()
        .collect()
}

/// The variables the human's global file (and the built-in defaults) name as provider keys: a
/// file inside the world never adds one.
#[must_use]
pub fn provider_keys(global: &Path, world: &Path) -> Vec<String> {
    mnemorph_loop::Config::load_global(&mnemorph_loop::Paths::with_global(
        global.to_path_buf(),
        world,
    ))
    .map(|c| {
        c.providers()
            .into_values()
            .filter_map(|p| p.key_env)
            .collect()
    })
    .unwrap_or_default()
}

/// The board's clock: the system's, moved on by a test.
#[derive(Clone, Default)]
pub(crate) struct Clock(Arc<Mutex<Duration>>);

impl Clock {
    pub(crate) fn now(&self) -> Instant {
        Instant::now() + *lock(&self.0)
    }

    pub(crate) fn pass(&self, by: Duration) {
        *lock(&self.0) += by;
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A tab's role on a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Role {
    /// It may prompt, steer, interrupt, end the chat and answer the gate.
    Controller,
    /// It sees everything and acts on nothing.
    Observer,
}

/// `board/role`: the tab's role, sent on connect and whenever control moves.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct BoardRole {
    /// This tab's.
    pub role: Role,
    /// Whether any tab controls the chat.
    pub controlled: bool,
}

/// A live chat, as `GET /api/sessions` lists it.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct BoardSession {
    /// The chat's id: `WS /ws/session/<id>`.
    pub id: String,
    /// Its world's id.
    pub world: String,
    /// The session's id, as the child's hello names it.
    pub session: String,
    /// The run's.
    pub run: String,
    /// Whether a tab controls it.
    pub controlled: bool,
    /// The tabs on it.
    #[ts(type = "number")]
    pub tabs: usize,
    /// The asks waiting.
    #[ts(type = "number")]
    pub asks: usize,
}

/// `GET /api/sessions`.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct BoardSessions {
    /// The live chats.
    pub sessions: Vec<BoardSession>,
}

/// `POST /api/w/<world>/sessions`: the chat, and the child's hello.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct BoardStarted {
    /// The chat.
    pub session: BoardSession,
    /// What the child's `initialize` answered.
    #[ts(as = "crate::protocol::Hello")]
    pub hello: Value,
}

/// An ask waiting for the human.
struct Ask {
    /// The body asking.
    body: String,
    /// The request's parameters, as the child sent them.
    params: Value,
    /// When the child asked, on the board's clock.
    at: Instant,
}

/// Why a tab was closed, for its socket to say.
type Closer = Arc<OnceLock<(u16, &'static str)>>;

/// A tab on a chat.
struct Conn {
    tx: mpsc::Sender<Utf8Bytes>,
    /// The bytes queued for it.
    bytes: Arc<AtomicUsize>,
    /// The board cookie it came with.
    cookie: String,
    /// Its requests waiting on the child.
    calls: usize,
    /// Why it was closed, for its socket to say.
    close: Closer,
}

/// A chat's state, under one lock.
#[derive(Default)]
struct State {
    replay: VecDeque<(u64, Utf8Bytes)>,
    kept: usize,
    asks: BTreeMap<String, Ask>,
    conns: BTreeMap<u64, Conn>,
    controller: Option<u64>,
    /// Since when no tab controls the chat.
    orphaned: Option<Instant>,
    /// The child-side id's number → the tab and the id it used.
    calls: HashMap<u64, (u64, Value)>,
    next_conn: u64,
    next_call: u64,
    init: Option<oneshot::Sender<Result<Value, String>>>,
}

fn notice(method: &str, params: &Value) -> Utf8Bytes {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
        .to_string()
        .into()
}

fn answer(id: &Value, result: &Value) -> Utf8Bytes {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
        .to_string()
        .into()
}

fn error(id: &Value, code: i64, message: &str) -> Utf8Bytes {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
        .to_string()
        .into()
}

/// The longest prefix of `s` within `max` bytes, on a character boundary.
fn cut(s: &str, max: usize) -> &str {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// An ask as a tab receives it: the child's request, with the time it has left.
fn ask_line(id: &str, ask: &Ask, now: Instant) -> Utf8Bytes {
    let left = GATE_WAIT.saturating_sub(now.saturating_duration_since(ask.at));
    let mut params = ask.params.clone();
    if let Some(o) = params.as_object_mut() {
        o.insert(
            "leftMs".into(),
            json!(u64::try_from(left.as_millis()).unwrap_or(u64::MAX)),
        );
    }
    json!({"jsonrpc": "2.0", "id": id, "method": "gate/ask", "params": params})
        .to_string()
        .into()
}

/// The child's answer to `id`: deny, with `why`.
fn deny_line(id: &str, why: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "result": {"allow": false, "reason": why}}).to_string()
}

impl State {
    /// Queue `msg` for tab `conn`; a tab that cannot take it is dropped.
    fn push(&mut self, conn: u64, msg: &Utf8Bytes, limits: &Limits, now: Instant) {
        let Some(c) = self.conns.get(&conn) else {
            return;
        };
        let len = msg.len();
        let before = c.bytes.fetch_add(len, Ordering::SeqCst);
        if before + len <= limits.queue_bytes && c.tx.try_send(msg.clone()).is_ok() {
            return;
        }
        c.bytes.fetch_sub(len, Ordering::SeqCst);
        self.drop_conn(
            conn,
            (close::SLOW, "too slow: reconnect to catch up"),
            limits,
            now,
        );
    }

    fn broadcast(&mut self, msg: &Utf8Bytes, limits: &Limits, now: Instant) {
        let ids: Vec<u64> = self.conns.keys().copied().collect();
        for id in ids {
            self.push(id, msg, limits, now);
        }
    }

    /// Tab `conn` leaves: its calls are forgotten, and if it controlled the chat, the grace
    /// begins.
    fn drop_conn(&mut self, conn: u64, why: (u16, &'static str), limits: &Limits, now: Instant) {
        if let Some(c) = self.conns.remove(&conn) {
            let _ = c.close.set(why);
        }
        self.calls.retain(|_, (c, _)| *c != conn);
        if self.controller == Some(conn) {
            self.controller = None;
            self.orphaned = Some(now);
            self.roles(limits, now);
        }
    }

    /// Tell every tab its role.
    fn roles(&mut self, limits: &Limits, now: Instant) {
        let controlled = self.controller.is_some();
        let ids: Vec<u64> = self.conns.keys().copied().collect();
        for id in ids {
            let role = if self.controller == Some(id) {
                Role::Controller
            } else {
                Role::Observer
            };
            let msg = notice("board/role", &json!(BoardRole { role, controlled }));
            self.push(id, &msg, limits, now);
        }
    }

    /// Send every waiting ask to tab `conn`, with its time left.
    fn asks_to(&mut self, conn: u64, limits: &Limits, now: Instant) {
        let lines: Vec<Utf8Bytes> = self
            .asks
            .iter()
            .map(|(id, ask)| ask_line(id, ask, now))
            .collect();
        for l in lines {
            self.push(conn, &l, limits, now);
        }
    }

    /// Drop the asks `which` picks, and tell every tab they are gone.
    fn withdraw(&mut self, which: impl Fn(&Ask) -> bool, limits: &Limits, now: Instant) {
        let gone: Vec<String> = self
            .asks
            .iter()
            .filter(|(_, a)| which(a))
            .map(|(id, _)| id.clone())
            .collect();
        for id in gone {
            self.asks.remove(&id);
            self.broadcast(&notice("board/askGone", &json!({"id": id})), limits, now);
        }
    }

    /// Keep an event for a replay: 10,000 at most, and within the byte cap.
    fn keep(&mut self, seq: u64, msg: Utf8Bytes, limits: &Limits) {
        self.kept += msg.len();
        self.replay.push_back((seq, msg));
        while self.replay.len() > REPLAY
            || (self.kept > limits.replay_bytes && self.replay.len() > 1)
        {
            if let Some((_, m)) = self.replay.pop_front() {
                self.kept -= m.len();
            }
        }
    }

    /// `session/events {since}`: the kept events after `since`, a page of them; `more` when
    /// another page follows, `complete: false` when the tab asked from further back than is kept.
    fn page(&self, id: &Value, since: u64, limits: &Limits) -> Utf8Bytes {
        let oldest = self.replay.front().map(|(s, _)| *s);
        let mut events = String::from("[");
        let (mut bytes, mut n, mut more) = (0usize, 0usize, false);
        for (_, msg) in self.replay.iter().filter(|(s, _)| *s > since) {
            if n > 0 && bytes + msg.len() > limits.page {
                more = true;
                break;
            }
            if n > 0 {
                events.push(',');
            }
            // Each was read as a JSON object from the child: it embeds as one.
            events.push_str(msg);
            bytes += msg.len();
            n += 1;
        }
        events.push(']');
        let complete = oldest.is_none_or(|o| o <= since.saturating_add(1));
        format!(
            r#"{{"jsonrpc":"2.0","id":{},"result":{{"events":{events},"oldest":{},"complete":{complete},"more":{more}}}}}"#,
            id,
            json!(oldest)
        )
        .into()
    }
}

/// One live chat.
struct Live {
    id: String,
    world: String,
    /// The board cookie of the browser that opened it: its tabs may control it.
    owner: String,
    hello: OnceLock<Value>,
    /// The child's input; `None` once the chat ends.
    input: Mutex<Option<mpsc::Sender<String>>>,
    state: Mutex<State>,
    limits: Limits,
    /// Asked to end, with why.
    ends: mpsc::UnboundedSender<String>,
    /// The child's output closed.
    exited: watch::Receiver<bool>,
    child: tokio::sync::Mutex<Option<tokio::process::Child>>,
    /// The child's process group.
    group: Option<u32>,
    ending: AtomicBool,
    /// Set once the chat has ended whole.
    gone: watch::Sender<bool>,
}

impl Live {
    fn listing(&self) -> BoardSession {
        let hello = self.hello.get();
        let field = |k: &str| {
            hello
                .and_then(|h| h[k].as_str())
                .unwrap_or_default()
                .to_owned()
        };
        let st = lock(&self.state);
        BoardSession {
            id: self.id.clone(),
            world: self.world.clone(),
            session: field("session"),
            run: field("run"),
            controlled: st.controller.is_some(),
            tabs: st.conns.len(),
            asks: st.asks.len(),
        }
    }

    /// A line the child wrote.
    fn child_line(&self, line: String, now: Instant) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            return;
        };
        let Some(obj) = value.as_object() else {
            return;
        };
        let limits = &self.limits;
        let mut st = lock(&self.state);
        match obj.get("method").and_then(Value::as_str) {
            Some(m) if m.starts_with("event/") => {
                let Some(seq) = value["params"]["seq"].as_u64() else {
                    return;
                };
                if m == "event/turnEnded" {
                    // A turn that ended waits on nothing: the session's own ends them all.
                    let body = value["params"]["body"].as_str().unwrap_or(SESSION_BODY);
                    st.withdraw(|a| body == SESSION_BODY || a.body == body, limits, now);
                }
                let msg = Utf8Bytes::from(line);
                st.keep(seq, msg.clone(), limits);
                st.broadcast(&msg, limits, now);
            }
            Some("gate/ask") => {
                let Some(id) = obj
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|i| i.starts_with("gate-"))
                else {
                    return;
                };
                let params = value["params"].clone();
                if !params.is_object() {
                    return;
                }
                let ask = Ask {
                    body: params["body"].as_str().unwrap_or(SESSION_BODY).to_owned(),
                    params,
                    at: now,
                };
                let msg = ask_line(id, &ask, now);
                st.asks.insert(id.to_owned(), ask);
                st.broadcast(&msg, limits, now);
            }
            // Nothing else is expected of the child.
            Some(_) => {}
            None => {
                let id = obj.get("id").and_then(Value::as_str);
                if id == Some(INIT) {
                    if let Some(tx) = st.init.take() {
                        let _ = tx.send(match (obj.get("result"), obj.get("error")) {
                            (Some(r), _) => Ok(r.clone()),
                            (None, e) => Err(cut(
                                e.and_then(|e| e["message"].as_str())
                                    .unwrap_or("the chat did not open"),
                                REFUSAL_BYTES,
                            )
                            .to_owned()),
                        });
                    }
                    return;
                }
                let Some(n) = id
                    .and_then(|s| s.strip_prefix("board-"))
                    .and_then(|n| n.parse::<u64>().ok())
                else {
                    return;
                };
                let Some((conn, original)) = st.calls.remove(&n) else {
                    return;
                };
                if let Some(c) = st.conns.get_mut(&conn) {
                    c.calls = c.calls.saturating_sub(1);
                }
                let mut v = value;
                v["id"] = original;
                st.push(conn, &v.to_string().into(), limits, now);
            }
        }
    }

    /// A line longer than the cap: dropped, and said to every tab.
    fn oversize(&self, bytes: usize, now: Instant) {
        let mut st = lock(&self.state);
        st.broadcast(
            &notice("board/oversize", &json!({"bytes": bytes})),
            &self.limits,
            now,
        );
    }

    /// A tab joins: the controller if it is the owner's and no tab controls the chat. It is told
    /// its role and sent every waiting ask.
    fn attach(
        &self,
        cookie: String,
        tx: mpsc::Sender<Utf8Bytes>,
        bytes: Arc<AtomicUsize>,
        now: Instant,
    ) -> Result<(u64, Closer), (u16, &'static str)> {
        if self.ending.load(Ordering::SeqCst) {
            return Err((close::ENDED, "the chat ended"));
        }
        let limits = &self.limits;
        let mut st = lock(&self.state);
        if st.conns.len() >= limits.tabs {
            return Err((close::CROWDED, "too many tabs on this chat"));
        }
        st.next_conn += 1;
        let id = st.next_conn;
        let owner = same(&cookie, &self.owner);
        let shut = Arc::new(OnceLock::new());
        st.conns.insert(
            id,
            Conn {
                tx,
                bytes,
                cookie,
                calls: 0,
                close: Arc::clone(&shut),
            },
        );
        if owner && st.controller.is_none() {
            st.controller = Some(id);
            st.orphaned = None;
        }
        st.roles(limits, now);
        st.asks_to(id, limits, now);
        Ok((id, shut))
    }

    fn detach(&self, conn: u64, now: Instant) {
        lock(&self.state).drop_conn(conn, (close::ENDED, "left"), &self.limits, now);
    }

    /// A message from tab `conn`.
    fn client_line(&self, conn: u64, text: &str, now: Instant) {
        let limits = &self.limits;
        let mut st = lock(&self.state);
        if !st.conns.contains_key(&conn) {
            return;
        }
        let null = Value::Null;
        let value: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                return st.push(
                    conn,
                    &error(&null, -32700, &format!("not JSON: {e}")),
                    limits,
                    now,
                );
            }
        };
        let Some(obj) = value.as_object() else {
            return st.push(
                conn,
                &error(&null, -32600, "a message is one JSON object"),
                limits,
                now,
            );
        };
        let id = obj.get("id").cloned();
        if let Some(i) = &id
            && !(i.is_u64() || i.as_str().is_some_and(|s| s.len() <= ID_BYTES))
        {
            return st.push(
                conn,
                &error(&null, -32600, "an id is a number or a short string"),
                limits,
                now,
            );
        }
        let controller = st.controller == Some(conn);
        let Some(method) = obj.get("method").and_then(Value::as_str) else {
            return self.gate_reply(&mut st, conn, controller, &value, now);
        };
        let reply_to = id.clone().unwrap_or(Value::Null);
        match method {
            "initialize" => {
                if id.is_some() {
                    let hello = self.hello.get().cloned().unwrap_or(Value::Null);
                    st.push(conn, &answer(&reply_to, &hello), limits, now);
                }
            }
            "session/events" => {
                if id.is_some() {
                    let since = value["params"]["since"].as_u64().unwrap_or(0);
                    let page = st.page(&reply_to, since, limits);
                    st.push(conn, &page, limits, now);
                }
            }
            "board/takeOver" => self.take_over(&mut st, conn, id.as_ref(), now),
            "session/prompt" | "session/send" | "session/interrupt" | "shutdown" => {
                if !controller {
                    if id.is_some() {
                        st.push(
                            conn,
                            &error(&reply_to, -32011, "this tab observes: take over to act"),
                            limits,
                            now,
                        );
                    }
                    return;
                }
                if method == "shutdown" {
                    if id.is_some() {
                        st.push(conn, &answer(&reply_to, &json!({})), limits, now);
                    }
                    let _ = self.ends.send("the human ended the chat".into());
                    return;
                }
                self.forward(&mut st, conn, method, id, &value, now);
            }
            other => {
                if id.is_some() {
                    st.push(
                        conn,
                        &error(
                            &reply_to,
                            -32601,
                            &format!("no method `{}`", cut(other, 64)),
                        ),
                        limits,
                        now,
                    );
                }
            }
        }
    }

    /// `board/takeOver`: a tab of the owner's browser takes control, and is sent what waits.
    fn take_over(&self, st: &mut State, conn: u64, id: Option<&Value>, now: Instant) {
        let limits = &self.limits;
        let mine = st
            .conns
            .get(&conn)
            .is_some_and(|c| same(&c.cookie, &self.owner));
        if !mine {
            if let Some(id) = id {
                st.push(
                    conn,
                    &error(
                        id,
                        -32010,
                        "only the browser that opened this chat may control it",
                    ),
                    limits,
                    now,
                );
            }
            return;
        }
        if let Some(id) = id {
            st.push(conn, &answer(id, &json!({})), limits, now);
        }
        if st.controller != Some(conn) {
            st.controller = Some(conn);
            st.orphaned = None;
            st.roles(limits, now);
            st.asks_to(conn, limits, now);
        }
    }

    /// A controller's request, on to the child under an id of the bridge's.
    fn forward(
        &self,
        st: &mut State,
        conn: u64,
        method: &str,
        id: Option<Value>,
        value: &Value,
        now: Instant,
    ) {
        let limits = &self.limits;
        let reply_to = id.clone().unwrap_or(Value::Null);
        let Some(input) = lock(&self.input).clone() else {
            if id.is_some() {
                st.push(
                    conn,
                    &error(&reply_to, -32001, "the chat ended"),
                    limits,
                    now,
                );
            }
            return;
        };
        let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
        let mut out = json!({"jsonrpc": "2.0", "method": method, "params": params});
        let mut call = None;
        if let Some(original) = id {
            let waiting = st.conns.get(&conn).map_or(0, |c| c.calls);
            if waiting >= limits.calls {
                st.push(
                    conn,
                    &error(
                        &reply_to,
                        -32012,
                        "too many requests waiting: wait for answers",
                    ),
                    limits,
                    now,
                );
                return;
            }
            st.next_call += 1;
            let n = st.next_call;
            st.calls.insert(n, (conn, original));
            if let Some(c) = st.conns.get_mut(&conn) {
                c.calls += 1;
            }
            out["id"] = json!(format!("board-{n}"));
            call = Some(n);
        }
        if input.try_send(out.to_string()).is_err() {
            if let Some(n) = call {
                st.calls.remove(&n);
                if let Some(c) = st.conns.get_mut(&conn) {
                    c.calls = c.calls.saturating_sub(1);
                }
                st.push(
                    conn,
                    &error(&reply_to, -32013, "the chat is busy: try again"),
                    limits,
                    now,
                );
            }
            return;
        }
        // Esc denies what waits at the gate, in the child as here.
        if method == "session/interrupt" {
            st.withdraw(|_| true, limits, now);
        }
    }

    /// A tab's answer to an ask: the controller's alone, once.
    fn gate_reply(&self, st: &mut State, conn: u64, controller: bool, value: &Value, now: Instant) {
        let limits = &self.limits;
        let id = value["id"].as_str().filter(|i| i.starts_with("gate-"));
        let refuse = |st: &mut State, why: &str| {
            st.push(
                conn,
                &notice("board/refused", &json!({"id": value["id"], "why": why})),
                limits,
                now,
            );
        };
        let Some(id) = id else {
            return refuse(st, "not an answer to an ask");
        };
        if !controller {
            return refuse(
                st,
                "this tab observes: only the controller answers the gate",
            );
        }
        if !st.asks.contains_key(id) {
            return refuse(
                st,
                "no such ask waits: answered, withdrawn or past its time",
            );
        }
        let reply = match (value.get("result"), value.get("error")) {
            (Some(r), _) => serde_json::from_value::<GateReply>(r.clone()).unwrap_or(GateReply {
                allow: false,
                reason: Some("the client's answer did not read".into()),
            }),
            _ => GateReply {
                allow: false,
                reason: Some("the client could not ask the human".into()),
            },
        };
        let line = json!({"jsonrpc": "2.0", "id": id, "result": {
            "allow": reply.allow,
            "reason": reply.reason.as_deref().map(|r| cut(r, REASON_BYTES)),
        }})
        .to_string();
        let sent = lock(&self.input)
            .as_ref()
            .is_some_and(|input| input.try_send(line).is_ok());
        if !sent {
            return refuse(st, "the chat is busy or ended: answer again");
        }
        st.asks.remove(id);
        st.broadcast(&notice("board/askGone", &json!({"id": id})), limits, now);
    }

    /// The clock's tick: asks past the gate's wait are dropped; past the grace with no
    /// controller, the reason the chat ends.
    fn tick(&self, now: Instant) -> Option<String> {
        let limits = &self.limits;
        let mut st = lock(&self.state);
        st.withdraw(
            |a| now.saturating_duration_since(a.at) >= GATE_WAIT,
            limits,
            now,
        );
        let orphaned = st.controller.is_none()
            && st
                .orphaned
                .is_some_and(|at| now.saturating_duration_since(at) >= limits.grace);
        orphaned.then(|| {
            format!(
                "no tab held the chat for {} s: what waited at the gate is denied, and the chat \
                 ended",
                limits.grace.as_secs()
            )
        })
    }
}

/// Read one line of at most `cap` bytes into `buf`: `None` at the end of the input, `Ok` for a
/// line, `Err(bytes)` for one longer than `cap`, read to its end and dropped.
async fn read_capped(
    r: &mut (impl AsyncBufRead + Unpin),
    cap: usize,
    buf: &mut Vec<u8>,
) -> std::io::Result<Option<Result<(), usize>>> {
    buf.clear();
    let mut over = 0usize;
    loop {
        let avail = r.fill_buf().await?;
        if avail.is_empty() {
            return Ok(match (over, buf.is_empty()) {
                (0, true) => None,
                (0, false) => Some(Ok(())),
                (n, _) => Some(Err(n)),
            });
        }
        let newline = avail.iter().position(|&b| b == b'\n');
        let chunk = &avail[..newline.unwrap_or(avail.len())];
        if over == 0 && buf.len() + chunk.len() <= cap {
            buf.extend_from_slice(chunk);
        } else {
            over += buf.len() + chunk.len();
            buf.clear();
        }
        let used = newline.map_or(avail.len(), |i| i + 1);
        r.consume(used);
        if newline.is_some() {
            return Ok(Some(if over == 0 { Ok(()) } else { Err(over) }));
        }
    }
}

/// The child's output, line by line, until it closes: then the chat ends.
async fn read_child(
    live: Arc<Live>,
    stdout: tokio::process::ChildStdout,
    clock: Clock,
    exited: watch::Sender<bool>,
) {
    let mut r = BufReader::new(stdout);
    let mut buf = Vec::new();
    loop {
        match read_capped(&mut r, live.limits.line, &mut buf).await {
            Ok(Some(Ok(()))) => {
                if let Ok(line) = String::from_utf8(std::mem::take(&mut buf)) {
                    live.child_line(line, clock.now());
                }
            }
            Ok(Some(Err(n))) => live.oversize(n, clock.now()),
            Ok(None) | Err(_) => break,
        }
    }
    let _ = exited.send(true);
    if let Some(tx) = lock(&live.state).init.take() {
        let _ = tx.send(Err("the chat's process left before it opened".into()));
    }
    let _ = live.ends.send("the chat's process ended".into());
}

/// Wait until process `pid` has exited, without reaping it: its id, and so its group's, stays
/// its own until it is reaped.
async fn gone_unreaped(pid: Option<u32>) {
    #[cfg(unix)]
    {
        use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
        let Some(pid) = pid
            .and_then(|p| i32::try_from(p).ok())
            .and_then(Pid::from_raw)
        else {
            return std::future::pending().await;
        };
        loop {
            let opts = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
            match waitid(WaitId::Pid(pid), opts) {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        std::future::pending().await
    }
}

/// Kill a process group, what is left of it.
fn kill_group(group: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = group
        .and_then(|g| i32::try_from(g).ok())
        .and_then(rustix::process::Pid::from_raw)
    {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
    #[cfg(not(unix))]
    let _ = group;
}

/// Why a chat did not start: an HTTP status and a reason.
pub type Refused = (u16, String);

/// The live chats.
pub struct Bridge {
    child: Child,
    limits: Limits,
    clock: Clock,
    sessions: Mutex<(HashMap<String, Arc<Live>>, usize)>,
}

impl Bridge {
    pub(crate) fn new(child: Child, limits: Limits, clock: Clock) -> Self {
        Self {
            child,
            limits,
            clock,
            sessions: Mutex::new((HashMap::new(), 0)),
        }
    }

    /// The bridge's bounds.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// The live chats.
    #[must_use]
    pub fn list(&self) -> BoardSessions {
        let lives: Vec<Arc<Live>> = lock(&self.sessions).0.values().cloned().collect();
        let mut sessions: Vec<BoardSession> = lives.iter().map(|l| l.listing()).collect();
        sessions.sort_by(|a, b| a.id.cmp(&b.id));
        BoardSessions { sessions }
    }

    fn find(&self, id: &str) -> Option<Arc<Live>> {
        lock(&self.sessions).0.get(id).cloned()
    }

    /// Start a chat in `world` (its id `world_id`), resuming `resume` if given, for the browser
    /// holding `owner`: a child is started and its session opened.
    ///
    /// # Errors
    ///
    /// Too many chats; the child does not start, refuses to open, or takes too long.
    pub async fn start(
        self: &Arc<Self>,
        world: &Path,
        world_id: &str,
        resume: Option<&str>,
        owner: &str,
    ) -> Result<BoardStarted, Refused> {
        {
            let mut s = lock(&self.sessions);
            if s.0.len() + s.1 >= self.limits.sessions {
                return Err((
                    429,
                    format!(
                        "{} chats are live, the most at once: end one first",
                        self.limits.sessions
                    ),
                ));
            }
            s.1 += 1;
        }
        let out = self.open(world, world_id, resume, owner).await;
        lock(&self.sessions).1 -= 1;
        out
    }

    async fn open(
        self: &Arc<Self>,
        world: &Path,
        world_id: &str,
        resume: Option<&str>,
        owner: &str,
    ) -> Result<BoardStarted, Refused> {
        let limits = self.limits;
        let keys = provider_keys(&self.child.global, world);
        let mut cmd = tokio::process::Command::new(&self.child.program);
        cmd.args((self.child.args)(world, resume))
            .current_dir(world)
            .env_clear()
            .envs(child_env(&self.child.env, &keys))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd
            .spawn()
            .map_err(|e| (502, format!("the chat's process did not start: {e}")))?;
        let group = child.id();
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            kill_group(group);
            return Err((502, "the chat's process has no pipes".into()));
        };
        let (input, mut lines) = mpsc::channel::<String>(INPUT);
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = lines.recv().await {
                let mut bytes = line.into_bytes();
                bytes.push(b'\n');
                if stdin.write_all(&bytes).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
            // Dropped here: the child's input ends, and it leaves.
        });
        let (ends, ends_rx) = mpsc::unbounded_channel();
        let (exited_tx, exited) = watch::channel(false);
        let (init, init_rx) = oneshot::channel();
        let id = super::secret().map_err(|e| (500, e))?[..32].to_owned();
        let live = Arc::new(Live {
            id: id.clone(),
            world: world_id.to_owned(),
            owner: owner.to_owned(),
            hello: OnceLock::new(),
            input: Mutex::new(Some(input.clone())),
            state: Mutex::new(State {
                init: Some(init),
                orphaned: Some(self.clock.now()),
                ..State::default()
            }),
            limits,
            ends,
            exited,
            child: tokio::sync::Mutex::new(Some(child)),
            group,
            ending: AtomicBool::new(false),
            gone: watch::channel(false).0,
        });
        tokio::spawn(read_child(
            Arc::clone(&live),
            stdout,
            self.clock.clone(),
            exited_tx,
        ));
        let hello = json!({"jsonrpc": "2.0", "id": INIT, "method": "initialize",
            "params": {"clientInfo": {"name": "mnemorph-board"}}});
        let opened = match input.send(hello.to_string()).await {
            Ok(()) => match tokio::time::timeout(limits.open, init_rx).await {
                Ok(Ok(Ok(hello))) if hello["protocol"] == json!(PROTOCOL) => Ok(hello),
                Ok(Ok(Ok(hello))) => Err(format!(
                    "the chat's process speaks protocol {}, not {PROTOCOL}",
                    hello["protocol"]
                )),
                Ok(Ok(Err(e))) => Err(e),
                Ok(Err(_)) => Err("the chat's process left before it opened".into()),
                Err(_) => Err(format!(
                    "the chat did not open within {} s",
                    limits.open.as_secs()
                )),
            },
            Err(_) => Err("the chat's process left before it opened".into()),
        };
        drop(input);
        let hello = match opened {
            Ok(h) => h,
            Err(e) => {
                self.end(&live, "the chat did not open").await;
                return Err((502, e));
            }
        };
        let _ = live.hello.set(hello.clone());
        lock(&self.sessions).0.insert(id, Arc::clone(&live));
        tokio::spawn(Arc::clone(self).watch(Arc::clone(&live), ends_rx));
        Ok(BoardStarted {
            session: live.listing(),
            hello,
        })
    }

    /// A chat's keeper: it ends the chat when asked, when the child leaves, or past the grace.
    async fn watch(self: Arc<Self>, live: Arc<Live>, mut ends: mpsc::UnboundedReceiver<String>) {
        let mut every = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                why = ends.recv() => {
                    let why = why.unwrap_or_else(|| "the chat ended".into());
                    return self.end(&live, &why).await;
                }
                _ = every.tick() => {
                    if let Some(why) = live.tick(self.clock.now()) {
                        return self.end(&live, &why).await;
                    }
                }
            }
        }
    }

    /// End a chat: every waiting ask denied with `why`, the child asked to leave and its input
    /// closed, its process group killed, every tab told and closed.
    async fn end(&self, live: &Arc<Live>, why: &str) {
        if live.ending.swap(true, Ordering::SeqCst) {
            let mut gone = live.gone.subscribe();
            let _ = tokio::time::timeout(self.limits.leave * 2, gone.wait_for(|g| *g)).await;
            return;
        }
        let leave = self.limits.leave;
        let input = lock(&live.input).take();
        let asks: Vec<String> = std::mem::take(&mut lock(&live.state).asks)
            .into_keys()
            .collect();
        if let Some(input) = input {
            let deadline = tokio::time::Instant::now() + leave;
            for id in &asks {
                let _ = tokio::time::timeout_at(deadline, input.send(deny_line(id, why))).await;
            }
            let end = json!({"jsonrpc": "2.0", "id": END, "method": "shutdown"}).to_string();
            let _ = tokio::time::timeout_at(deadline, input.send(end)).await;
        }
        // Left: its output closed, or it exited while something it started holds the output.
        let mut exited = live.exited.clone();
        let _ = tokio::time::timeout(leave, async {
            tokio::select! {
                _ = exited.wait_for(|e| *e) => {}
                () = gone_unreaped(live.group) => {}
            }
        })
        .await;
        // The child is not reaped yet, so its group id is still its own: what it left behind
        // goes with it, and the child too if it did not leave.
        kill_group(live.group);
        if let Some(mut child) = live.child.lock().await.take() {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(leave, child.wait()).await;
        }
        {
            let now = self.clock.now();
            let mut st = lock(&live.state);
            st.broadcast(
                &notice("board/ended", &json!({"why": why})),
                &self.limits,
                now,
            );
            st.controller = None;
            for (_, c) in std::mem::take(&mut st.conns) {
                let _ = c.close.set((close::ENDED, "the chat ended"));
            }
            st.calls.clear();
        }
        lock(&self.sessions).0.remove(&live.id);
        let _ = live.gone.send(true);
    }

    /// End every chat: the board stops.
    pub async fn end_all(self: &Arc<Self>, why: &str) {
        let lives: Vec<Arc<Live>> = lock(&self.sessions).0.values().cloned().collect();
        let mut set = tokio::task::JoinSet::new();
        for live in lives {
            let bridge = Arc::clone(self);
            let why = why.to_owned();
            set.spawn(async move { bridge.end(&live, &why).await });
        }
        while set.join_next().await.is_some() {}
    }

    /// Serve one tab's socket on chat `id` until either end leaves.
    pub(crate) async fn serve(
        self: Arc<Self>,
        board: Arc<super::Board>,
        id: String,
        cookie: String,
        mut socket: WebSocket,
    ) {
        let Some(live) = self.find(&id) else {
            return shut(
                &mut socket,
                close::UNKNOWN,
                "no such chat: it ended, or never began",
            )
            .await;
        };
        let (tx, mut rx) = mpsc::channel(self.limits.queue);
        let bytes = Arc::new(AtomicUsize::new(0));
        let (conn, why) =
            match live.attach(cookie.clone(), tx, Arc::clone(&bytes), self.clock.now()) {
                Ok(c) => c,
                Err((code, why)) => return shut(&mut socket, code, why).await,
            };
        let mut every = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                out = rx.recv() => {
                    let Some(msg) = out else {
                        let (code, reason) = why.get().copied().unwrap_or((close::ENDED, "the chat ended"));
                        shut(&mut socket, code, reason).await;
                        break;
                    };
                    bytes.fetch_sub(msg.len(), Ordering::SeqCst);
                    if socket.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                }
                inbound = socket.recv() => match inbound {
                    Some(Ok(Message::Text(text))) => {
                        if board.csrf_of(&cookie).is_none() {
                            shut(&mut socket, close::SIGNED_OUT, "the board session ended").await;
                            break;
                        }
                        live.client_line(conn, &text, self.clock.now());
                    }
                    Some(Ok(Message::Binary(_))) => {
                        shut(&mut socket, 1003, "text messages only").await;
                        break;
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                },
                _ = every.tick() => {
                    if board.csrf_of(&cookie).is_none() {
                        shut(&mut socket, close::SIGNED_OUT, "the board session ended").await;
                        break;
                    }
                }
            }
        }
        live.detach(conn, self.clock.now());
    }
}

/// Close a socket with `code` and `reason`.
async fn shut(socket: &mut WebSocket, code: u16, reason: &str) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

#[cfg(test)]
mod tests {
    use super::{child_env, read_capped};

    #[test]
    fn the_child_env_keeps_only_the_named_keys() {
        let base: Vec<(String, String)> = [
            ("PATH", "/usr/bin"),
            ("HOME", "/h"),
            ("OPENROUTER_API_KEY", "sk-or"),
            ("ANTHROPIC_API_KEY", "sk-ant"),
            ("GITHUB_TOKEN", "ghp"),
            ("NPM_AUTH", "x"),
            ("DATABASE_URL", "postgres://me:pw@db/app"),
            ("MNEMORPH_BODY", "orc-x"),
            ("GIT_AUTHOR_NAME", "me"),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
        let env = child_env(&base, &["OPENROUTER_API_KEY".to_owned()]);
        let names: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            names,
            ["PATH", "HOME", "OPENROUTER_API_KEY", "GIT_AUTHOR_NAME"]
        );
    }

    #[tokio::test]
    async fn a_long_line_is_dropped_whole_and_the_next_read() {
        let input = format!("{}\nshort\n{}", "x".repeat(100), "tail");
        let mut r = tokio::io::BufReader::with_capacity(7, input.as_bytes());
        let mut buf = Vec::new();
        assert_eq!(
            read_capped(&mut r, 10, &mut buf).await.unwrap(),
            Some(Err(100))
        );
        assert_eq!(
            read_capped(&mut r, 10, &mut buf).await.unwrap(),
            Some(Ok(()))
        );
        assert_eq!(buf, b"short");
        assert_eq!(
            read_capped(&mut r, 10, &mut buf).await.unwrap(),
            Some(Ok(()))
        );
        assert_eq!(buf, b"tail");
        assert_eq!(read_capped(&mut r, 10, &mut buf).await.unwrap(), None);
    }
}
