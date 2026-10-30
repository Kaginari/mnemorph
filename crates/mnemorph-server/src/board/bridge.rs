//! The live chats. Each is a child `mnemorphd` on its world, speaking the unchanged protocol on its
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
//! - **Approvals**: every field of an ask reaches the tabs already passed through `shown`, with
//!   the SHA-256 of that text. The page renders it as text; its card acknowledges the render
//!   (`board/seen`) when it is first visible. An allow is taken only from the controller, once,
//!   naming that digest, from the tab that acknowledged, at least [`ARM`] after it did — the
//!   client is never trusted to have enforced its own card. Every answer, taken or refused, is
//!   journaled with its connection, chained, under the human's state directory.
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

use mnemorph_loop::wire::shown;
use mnemorph_loop::world::answers::{self, AnswerRow};
use mnemorph_loop::world::ledger::{Journal, Timestamp, evolution::digest as sha256};

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

/// How long after a tab's render acknowledgement of an ask its allow is first taken: the card's
/// arming, the TUI's 300 ms, held by the server too.
pub const ARM: Duration = Duration::from_millis(300);

/// The refused answers journaled for one ask: past these, a tab that keeps sending them is
/// still refused, and no longer written down.
const REFUSALS_KEPT: u8 = 8;

/// The ids of answered asks remembered, so a late answer hears it was answered elsewhere.
const ANSWERED: usize = 64;

/// The most bytes of the act an answer's journal row keeps.
const ACT_KEPT: usize = 4096;

/// The most bytes of an act, as shown, the web may approve: past it, only the terminal can.
pub const ACT_BYTES: usize = 32 << 10;

/// The most rows of an act, as shown, the web may approve: a row of the card is
/// [`ROW_CHARS`] characters, or ends at an escaped newline.
pub const ACT_ROWS: usize = 400;

/// The characters of the act a row of the card holds (the page's `ROW_CHARS`).
const ROW_CHARS: usize = 64;

/// A run of plain spaces at least this long is shown as a marker naming its length.
const SPACE_RUN: usize = 16;

/// `n` with its thousands apart: `9,800`.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A field as the card shows it: [`shown`]'s escapes, `⟨` escaped too, then each run of
/// [`SPACE_RUN`] plain spaces or more as `⟨… <n> spaces⟩` — a marker no text can forge, since
/// every `⟨` the text wrote is escaped.
#[must_use]
pub fn on_card(text: &str) -> String {
    use std::fmt::Write as _;
    let escaped = shown(text).replace('\u{27e8}', "\\u{27e8}");
    let mut out = String::with_capacity(escaped.len());
    let mut run = 0usize;
    let flush = |out: &mut String, run: usize| {
        if run >= SPACE_RUN {
            let _ = write!(out, "⟨… {} spaces⟩", thousands(run));
        } else {
            out.extend(std::iter::repeat_n(' ', run));
        }
    };
    for c in escaped.chars() {
        if c == ' ' {
            run += 1;
            continue;
        }
        flush(&mut out, run);
        run = 0;
        out.push(c);
    }
    flush(&mut out, run);
    out
}

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
    /// How long one message may take to reach a tab before the tab is dropped.
    pub send: Duration,
    /// How often a tab is pinged; two pings unanswered drop it.
    pub ping: Duration,
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
            send: Duration::from_secs(30),
            ping: Duration::from_secs(5),
        }
    }
}

/// The arguments a child is started with, for a world and the run it resumes.
pub type Args = Arc<dyn Fn(&Path, Option<&str>) -> Vec<OsString> + Send + Sync>;

/// Where a world's answers to the gate are journaled; `None` with no home to keep them under.
pub type Answers = Arc<dyn Fn(&Path) -> Option<PathBuf> + Send + Sync>;

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
    /// Where each world's answers to the gate are journaled: under the human's state directory
    /// ([`answers::answers_of`]).
    pub answers: Answers,
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
            answers: Arc::new(answers::answers_of),
        }
    }
}

/// The variables a child may have from the board's environment, by name.
const ALLOWED: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "LANG",
    "TERM",
    "TMPDIR",
    "TZ",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
];

/// Whether a child may have variable `name`: one of [`ALLOWED`], `LC_*`, `XDG_*`, or `MNEMORPH_*`
/// but `MNEMORPH_BODY`.
fn allowed(name: &str) -> bool {
    ALLOWED.contains(&name)
        || name.starts_with("LC_")
        || name.starts_with("XDG_")
        || (name.starts_with("MNEMORPH_") && name != "MNEMORPH_BODY")
}

/// The variable a child reads the board's process id from, to die with it.
pub const BOARD_PID: &str = "MNEMORPH_BOARD_PID";

/// The child's environment: from `base`, only what [`allowed`] names — and of it nothing secret
/// (`is_secret`: a credential in a proxy's URL is dropped too) — and the variables in `keys`,
/// the provider keys it needs.
#[must_use]
pub fn child_env(base: &[(String, String)], keys: &[String]) -> Vec<(String, String)> {
    base.iter()
        .filter(|(k, v)| keys.contains(k) || (allowed(k) && !mnemorph_loop::is_secret(k, v)))
        .cloned()
        .collect()
}

/// For a child of the board (`MNEMORPH_BOARD_PID` set): die when the board does. On Linux the
/// parent-death signal is set, then the parent checked, so a board gone in between is caught.
///
/// # Errors
///
/// The board is already gone, or the signal cannot be set.
pub fn die_with_board() -> Result<(), String> {
    let Some(pid) = std::env::var(BOARD_PID)
        .ok()
        .and_then(|p| p.parse::<i32>().ok())
    else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    {
        use rustix::process::{Pid, Signal, getppid, set_parent_process_death_signal};
        set_parent_process_death_signal(Some(Signal::TERM))
            .map_err(|e| format!("the parent-death signal: {e}"))?;
        if Pid::as_raw(getppid()) != pid {
            return Err("the board that started this chat is gone".into());
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = pid;
    Ok(())
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

/// An ask as a human is shown it: every field the child sent, passed through `shown`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Shown {
    /// The body asking.
    pub body: String,
    /// Its race.
    pub race: String,
    /// The act's class.
    pub class: String,
    /// The act as it will run.
    pub act: String,
    /// Why it has its class.
    pub why: Vec<String>,
    /// Paths outside the world it opens.
    pub outside: Vec<String>,
    /// The plan the approval binds to, in short.
    pub plan: Option<String>,
}

impl Shown {
    /// The ask in the child's `params`, each field escaped; a field missing or of another type
    /// reads empty.
    #[must_use]
    pub fn of(params: &Value) -> Self {
        let text = |k: &str| on_card(params[k].as_str().unwrap_or_default());
        let list = |k: &str| {
            params[k]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|v| on_card(v.as_str().unwrap_or_default()))
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            body: on_card(params["body"].as_str().unwrap_or(SESSION_BODY)),
            race: text("race"),
            class: text("class"),
            act: text("act"),
            why: list("why"),
            outside: list("outside"),
            plan: params["plan"].as_str().map(on_card),
        }
    }

    /// The rows the card draws the act in: each escaped newline ends one, and a longer run is cut
    /// every [`ROW_CHARS`] characters.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.act
            .split("\\u{a}")
            .map(|line| line.chars().count().div_ceil(ROW_CHARS).max(1))
            .sum()
    }

    /// Whether the act is too large to approve from the web: over [`ACT_BYTES`] or [`ACT_ROWS`]
    /// as shown. Such an ask is answered in the terminal; the web may only deny it.
    #[must_use]
    pub fn too_large(&self) -> bool {
        self.act.len() > ACT_BYTES || self.rows() > ACT_ROWS
    }

    /// The SHA-256 of ask `id` as shown, in lowercase hex: each field as its length in UTF-8
    /// bytes, a colon, the text and a newline, in a fixed order after a version line; each list
    /// first its count; the plan `-` when absent, `+` before it when present. The board's page
    /// computes the same over what it renders (`packages/board/src/data/approval.ts`).
    #[must_use]
    pub fn digest(&self, id: &str) -> String {
        fn field(out: &mut Vec<u8>, s: &str) {
            out.extend_from_slice(format!("{}:", s.len()).as_bytes());
            out.extend_from_slice(s.as_bytes());
            out.push(b'\n');
        }
        let mut out = b"mnemorph.ask.v1\n".to_vec();
        for s in [id, &self.body, &self.race, &self.class, &self.act] {
            field(&mut out, s);
        }
        for list in [&self.why, &self.outside] {
            out.extend_from_slice(format!("{}\n", list.len()).as_bytes());
            for s in list {
                field(&mut out, s);
            }
        }
        match &self.plan {
            None => out.extend_from_slice(b"-\n"),
            Some(p) => {
                out.push(b'+');
                field(&mut out, p);
            }
        }
        sha256(&out)
    }
}

/// An ask waiting for the human.
struct Ask {
    /// The body asking, as the child named it: the turn whose end withdraws the ask.
    body: String,
    /// The ask as it is shown.
    shown: Shown,
    /// The SHA-256 of what is shown ([`Shown::digest`]).
    digest: String,
    /// When the child asked, on the board's clock.
    at: Instant,
    /// The controller tab that acknowledged rendering the card, and when; cleared whenever
    /// control moves.
    seen: Option<(u64, Instant)>,
    /// The refused answers journaled.
    refusals: u8,
    /// An answer to it is being journaled, outside the chat's lock: no other is taken meanwhile.
    taking: bool,
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
    /// The asks answered lately, for a late answer to hear so.
    answered: VecDeque<String>,
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

/// Milliseconds, for the wire and the journal.
fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// An ask as a tab receives it: the child's request as shown, its digest, and the time it has
/// left.
fn ask_line(id: &str, ask: &Ask, now: Instant) -> Utf8Bytes {
    let left = GATE_WAIT.saturating_sub(now.saturating_duration_since(ask.at));
    let s = &ask.shown;
    let params = json!({
        "body": s.body, "race": s.race, "class": s.class, "act": s.act, "why": s.why,
        "outside": s.outside, "plan": s.plan, "leftMs": ms(left), "digest": ask.digest,
        "tooLarge": s.too_large(),
    });
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
            self.control(None);
            self.orphaned = Some(now);
            self.roles(limits, now);
        }
    }

    /// Control moves to `conn`: every card must be seen again, by the tab that now controls,
    /// before an allow is taken.
    fn control(&mut self, conn: Option<u64>) {
        self.controller = conn;
        for ask in self.asks.values_mut() {
            ask.seen = None;
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

    /// Drop the asks `which` picks, and tell every tab they are gone, and `why`.
    fn withdraw(&mut self, which: impl Fn(&Ask) -> bool, why: &str, limits: &Limits, now: Instant) {
        let gone: Vec<String> = self
            .asks
            .iter()
            .filter(|(_, a)| which(a))
            .map(|(id, _)| id.clone())
            .collect();
        for id in gone {
            self.asks.remove(&id);
            self.gone(id, why, limits, now);
        }
    }

    /// Ask `id` left the list, `why`: every tab is told, and a late answer hears it.
    fn gone(&mut self, id: String, why: &str, limits: &Limits, now: Instant) {
        let msg = notice("board/askGone", &json!({"id": id, "why": why}));
        if self.answered.len() == ANSWERED {
            self.answered.pop_front();
        }
        self.answered.push_back(id);
        self.broadcast(&msg, limits, now);
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

/// An answer being taken: its ask, its journal row, and its line with the place held for it in
/// the child's input.
struct Take {
    id: String,
    row: AnswerRow,
    allow: bool,
    line: String,
    permit: mpsc::OwnedPermit<String>,
}

/// What a tab's answer leaves to do once the chat's lock is let go.
#[derive(Default)]
struct After {
    /// Refused answers, to journal.
    rows: Vec<AnswerRow>,
    /// The answer taken.
    take: Option<Take>,
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
    /// Where the answers to its asks are journaled; `None` with no home.
    answers: Option<Journal<AnswerRow>>,
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
    /// The last bytes the child wrote on stderr.
    last_words: Mutex<VecDeque<u8>>,
    /// Set once its stderr closed.
    spoke: watch::Sender<bool>,
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
                    st.withdraw(
                        |a| body == SESSION_BODY || a.body == body,
                        "withdrawn: the asking turn ended",
                        limits,
                        now,
                    );
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
                let params = &value["params"];
                if !params.is_object() {
                    return;
                }
                let shown = Shown::of(params);
                let ask = Ask {
                    body: params["body"].as_str().unwrap_or(SESSION_BODY).to_owned(),
                    digest: shown.digest(id),
                    shown,
                    at: now,
                    seen: None,
                    refusals: 0,
                    taking: false,
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
                            // The child's words reach the page escaped, as its last words do.
                            (None, e) => Err(mnemorph_loop::wire::shown(cut(
                                e.and_then(|e| e["message"].as_str())
                                    .unwrap_or("the chat did not open"),
                                REFUSAL_BYTES,
                            ))),
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
            st.control(Some(id));
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
            let after = self.gate_reply(&mut st, conn, controller, &value, now);
            drop(st);
            return self.finish(after, conn, now);
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
            "board/seen" => self.seen(&mut st, conn, controller, id.as_ref(), &value, now),
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
            st.control(Some(conn));
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
            st.withdraw(|_| true, "denied: the turn was interrupted", limits, now);
        }
    }

    /// `board/seen {id, digest}`: the controller's card for ask `id` is on its screen, showing
    /// the act whose digest it names. The first acknowledgement of the tab that controls starts
    /// the [`ARM`] an allow waits for; control moving clears it.
    fn seen(
        &self,
        st: &mut State,
        conn: u64,
        controller: bool,
        id: Option<&Value>,
        value: &Value,
        now: Instant,
    ) {
        let limits = &self.limits;
        let ask = value["params"]["id"].as_str().unwrap_or_default();
        let named = value["params"]["digest"].as_str();
        let outcome = if controller {
            match st.asks.get_mut(ask) {
                None => Err((
                    -32014,
                    "no such ask waits: answered, withdrawn or past its time",
                )),
                Some(a) if named != Some(a.digest.as_str()) => Err((
                    -32015,
                    "the digest names another act than the one the board showed",
                )),
                Some(a) => {
                    if a.seen.is_none_or(|(c, _)| c != conn) {
                        a.seen = Some((conn, now));
                    }
                    Ok(())
                }
            }
        } else {
            Err((-32011, "this tab observes: take over to answer"))
        };
        let Some(id) = id else { return };
        let msg = match outcome {
            Ok(()) => answer(id, &json!({})),
            Err((code, why)) => error(id, code, why),
        };
        st.push(conn, &msg, limits, now);
    }

    /// Why an allow from tab `conn` for `ask` is not taken, if it is not: it must name the
    /// digest of the act as shown, and come from the tab that acknowledged the card, at least
    /// [`ARM`] after it did.
    fn unready(ask: &Ask, conn: u64, named: Option<&str>, now: Instant) -> Option<&'static str> {
        if ask.shown.too_large() {
            return Some(
                "too large to approve from the web: answer in the terminal (`mnemorph`), or deny it here",
            );
        }
        if named != Some(ask.digest.as_str()) {
            return Some(
                "the answer names another act than the one the board showed (its digest differs)",
            );
        }
        let Some((_, at)) = ask.seen.filter(|(by, _)| *by == conn) else {
            return Some(
                "the card was not seen in this tab: an allow follows its render acknowledgement",
            );
        };
        (now.saturating_duration_since(at) < ARM)
            .then_some("too fast: an allow comes at least 300 ms after the card was seen")
    }

    /// The row journaling tab `conn`'s answer to `ask`.
    fn row(
        &self,
        id: &str,
        ask: &Ask,
        conn: u64,
        reply: &GateReply,
        outcome: &str,
        now: Instant,
    ) -> AnswerRow {
        AnswerRow {
            ts: Timestamp::now(),
            chat: self.id.clone(),
            ask: id.to_owned(),
            conn,
            body: ask.shown.body.clone(),
            class: ask.shown.class.clone(),
            act: cut(&ask.shown.act, ACT_KEPT).to_owned(),
            shown_digest: ask.digest.clone(),
            named_digest: reply.digest.as_deref().map(|d| cut(d, 128).to_owned()),
            allow: reply.allow,
            outcome: outcome.to_owned(),
            asked_ms: ms(now.saturating_duration_since(ask.at)),
            seen_ms: ask
                .seen
                .filter(|(by, _)| *by == conn)
                .map(|(_, at)| ms(now.saturating_duration_since(at))),
            build: mnemorph_loop::world::ledger::Build::default(),
        }
    }

    /// Journal `row`: `Err` when it could not be written. No home to keep it under writes
    /// nothing, and is no error: an allow is refused before it gets here.
    fn journal(&self, row: AnswerRow) -> Result<(), String> {
        match &self.answers {
            Some(j) => answers::record(j, row).map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    /// A tab's answer to an ask: the controller's alone, once; an allow only as [`Self::unready`]
    /// says. What it leaves to do outside the chat's lock — the journal's rows, and the answer
    /// itself — is [`Self::finish`]'s.
    fn gate_reply(
        &self,
        st: &mut State,
        conn: u64,
        controller: bool,
        value: &Value,
        now: Instant,
    ) -> After {
        let limits = &self.limits;
        let mut after = After::default();
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
            refuse(st, "not an answer to an ask");
            return after;
        };
        let reply = match (value.get("result"), value.get("error")) {
            (Some(r), _) => serde_json::from_value::<GateReply>(r.clone()).unwrap_or(GateReply {
                allow: false,
                reason: Some("the client's answer did not read".into()),
                digest: None,
            }),
            _ => GateReply {
                allow: false,
                reason: Some("the client could not ask the human".into()),
                digest: None,
            },
        };
        let Some(ask) = st.asks.get_mut(id) else {
            let why = if st.answered.iter().any(|a| a == id) {
                "answered elsewhere: another answer, or the core, came first"
            } else {
                "no such ask waits: answered, withdrawn or past its time"
            };
            refuse(st, why);
            return after;
        };
        if ask.taking {
            refuse(st, "answered elsewhere: another answer is being taken");
            return after;
        }
        let refusal = if !controller {
            Some("this tab observes: only the controller answers the gate")
        } else if !reply.allow {
            None
        } else if self.answers.is_none() {
            Some(
                "no home to journal the answer in, so no allow is taken: deny, or answer in the terminal",
            )
        } else {
            Self::unready(ask, conn, reply.digest.as_deref(), now)
        };
        if let Some(why) = refusal {
            if ask.refusals < REFUSALS_KEPT {
                ask.refusals += 1;
                after
                    .rows
                    .push(self.row(id, ask, conn, &reply, &format!("refused: {why}"), now));
            }
            refuse(st, why);
            return after;
        }
        // A place in the child's input first, then the journal, then the answer: an answer is
        // journaled before the core hears it, and only once it surely will.
        let input = lock(&self.input).clone();
        let Some(permit) = input.and_then(|i| i.try_reserve_owned().ok()) else {
            refuse(st, "the chat is busy or ended: answer again");
            return after;
        };
        ask.taking = true;
        after.take = Some(Take {
            id: id.to_owned(),
            row: self.row(id, ask, conn, &reply, "answered", now),
            allow: reply.allow,
            line: json!({"jsonrpc": "2.0", "id": id, "result": {
                "allow": reply.allow,
                "reason": reply.reason.as_deref().map(|r| cut(r, REASON_BYTES)),
            }})
            .to_string(),
            permit,
        });
        after
    }

    /// What an answer left to do, outside the chat's lock: the refused rows journaled, then the
    /// answer taken — journaled, and only then sent to the core. An allow whose row cannot be
    /// written is refused, and the ask waits again.
    fn finish(&self, after: After, conn: u64, now: Instant) {
        for row in after.rows {
            let _ = self.journal(row);
        }
        let Some(take) = after.take else { return };
        let limits = &self.limits;
        if let Err(e) = self.journal(take.row)
            && take.allow
        {
            drop(take.permit);
            let mut st = lock(&self.state);
            if let Some(ask) = st.asks.get_mut(&take.id) {
                ask.taking = false;
            }
            let why = format!("the answer could not be journaled, so it is not taken: {e}");
            st.push(
                conn,
                &notice("board/refused", &json!({"id": take.id, "why": why})),
                limits,
                now,
            );
            return;
        }
        take.permit.send(take.line);
        let mut st = lock(&self.state);
        if st.asks.remove(&take.id).is_some() {
            st.gone(take.id, "answered", limits, now);
        }
    }

    /// The clock's tick: asks past the gate's wait are dropped; past the grace with no
    /// controller, the reason the chat ends.
    fn tick(&self, now: Instant) -> Option<String> {
        let limits = &self.limits;
        let mut st = lock(&self.state);
        st.withdraw(
            |a| now.saturating_duration_since(a.at) >= GATE_WAIT,
            "denied: no answer in 10 minutes",
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

/// The most bytes of the child's stderr kept, for why it ended.
const LAST_WORDS: usize = 8 << 10;

/// The child's stderr, its last [`LAST_WORDS`] kept.
async fn read_stderr(live: Arc<Live>, mut stderr: tokio::process::ChildStderr) {
    use tokio::io::AsyncReadExt as _;
    let mut buf = [0u8; 4096];
    while let Ok(n) = stderr.read(&mut buf).await {
        if n == 0 {
            break;
        }
        let mut words = lock(&live.last_words);
        words.extend(&buf[..n]);
        let over = words.len().saturating_sub(LAST_WORDS);
        words.drain(..over);
    }
    let _ = live.spoke.send(true);
}

/// `why`, and the child's last words on stderr if it said any, escaped.
fn with_last_words(why: &str, live: &Live) -> String {
    let words = lock(&live.last_words);
    let (a, b) = words.as_slices();
    let text = String::from_utf8_lossy(&[a, b].concat()).trim().to_owned();
    if text.is_empty() {
        why.to_owned()
    } else {
        format!(
            "{why} — its stderr ended: {}",
            mnemorph_loop::wire::shown(&text)
        )
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

    /// The child's command: its program and arguments, in the world, with its scrubbed
    /// environment and the board's pid to die with, in a group of its own.
    fn command(&self, world: &Path, resume: Option<&str>) -> tokio::process::Command {
        let keys = provider_keys(&self.child.global, world);
        let mut cmd = tokio::process::Command::new(&self.child.program);
        cmd.args((self.child.args)(world, resume))
            .current_dir(world)
            .env_clear()
            .envs(child_env(&self.child.env, &keys))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env(BOARD_PID, std::process::id().to_string())
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);
        cmd
    }

    async fn open(
        self: &Arc<Self>,
        world: &Path,
        world_id: &str,
        resume: Option<&str>,
        owner: &str,
    ) -> Result<BoardStarted, Refused> {
        let limits = self.limits;
        let mut cmd = self.command(world, resume);
        let mut child = cmd
            .spawn()
            .map_err(|e| (502, format!("the chat's process did not start: {e}")))?;
        let group = child.id();
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
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
            answers: (self.child.answers)(world).map(answers::journal_at),
            state: Mutex::new(State {
                init: Some(init),
                ..State::default()
            }),
            limits,
            ends,
            exited,
            child: tokio::sync::Mutex::new(Some(child)),
            group,
            ending: AtomicBool::new(false),
            gone: watch::channel(false).0,
            last_words: Mutex::new(VecDeque::new()),
            spoke: watch::channel(false).0,
        });
        tokio::spawn(read_stderr(Arc::clone(&live), stderr));
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
                return Err((502, with_last_words(&e, &live)));
            }
        };
        let _ = live.hello.set(hello.clone());
        // The grace counts from the opening: a slow one eats none of it.
        lock(&live.state).orphaned = Some(self.clock.now());
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
        let mut spoke = live.spoke.subscribe();
        let _ = tokio::time::timeout(Duration::from_millis(500), spoke.wait_for(|s| *s)).await;
        let why = &with_last_words(why, live);
        {
            let now = self.clock.now();
            let mut st = lock(&live.state);
            st.broadcast(
                &notice("board/ended", &json!({"why": why})),
                &self.limits,
                now,
            );
            st.control(None);
            for (_, c) in std::mem::take(&mut st.conns) {
                let _ = c.close.set((close::ENDED, "the chat ended"));
            }
            st.calls.clear();
            // No tab can reach an ended chat: what it kept goes now.
            st.replay = VecDeque::new();
            st.kept = 0;
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
                self.limits.send,
            )
            .await;
        };
        let (tx, mut rx) = mpsc::channel(self.limits.queue);
        let bytes = Arc::new(AtomicUsize::new(0));
        let (conn, why) =
            match live.attach(cookie.clone(), tx, Arc::clone(&bytes), self.clock.now()) {
                Ok(c) => c,
                Err((code, why)) => return shut(&mut socket, code, why, self.limits.send).await,
            };
        let limit = self.limits.send;
        let mut every = tokio::time::interval(self.limits.ping);
        every.tick().await;
        // Pings sent and not answered: two, and the tab is gone.
        let mut unanswered = 0u8;
        loop {
            tokio::select! {
                out = rx.recv() => {
                    let Some(msg) = out else {
                        let (code, reason) = why.get().copied().unwrap_or((close::ENDED, "the chat ended"));
                        shut(&mut socket, code, reason, limit).await;
                        break;
                    };
                    bytes.fetch_sub(msg.len(), Ordering::SeqCst);
                    // A tab that stops reading is dropped, never waited on.
                    if !matches!(tokio::time::timeout(limit, socket.send(Message::Text(msg))).await, Ok(Ok(()))) {
                        break;
                    }
                }
                inbound = socket.recv() => match inbound {
                    Some(Ok(Message::Pong(_))) => unanswered = 0,
                    Some(Ok(Message::Text(text))) => {
                        if board.csrf_of(&cookie).is_none() {
                            shut(&mut socket, close::SIGNED_OUT, "the board session ended", limit).await;
                            break;
                        }
                        live.client_line(conn, &text, self.clock.now());
                    }
                    Some(Ok(Message::Binary(_))) => {
                        shut(&mut socket, 1003, "text messages only", limit).await;
                        break;
                    }
                    Some(Ok(Message::Ping(_))) => {}
                    Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                },
                _ = every.tick() => {
                    if board.csrf_of(&cookie).is_none() {
                        shut(&mut socket, close::SIGNED_OUT, "the board session ended", limit).await;
                        break;
                    }
                    if unanswered >= 2 {
                        break;
                    }
                    unanswered += 1;
                    let ping = socket.send(Message::Ping(axum::body::Bytes::new()));
                    if !matches!(tokio::time::timeout(limit, ping).await, Ok(Ok(()))) {
                        break;
                    }
                }
            }
        }
        live.detach(conn, self.clock.now());
    }
}

/// Close a socket with `code` and `reason`, waiting at most `limit` for a tab that reads nothing.
async fn shut(socket: &mut WebSocket, code: u16, reason: &str, limit: Duration) {
    let close = socket.send(Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    })));
    let _ = tokio::time::timeout(limit, close).await;
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
            ("AWS_PROFILE", "prod"),
            ("SSH_AUTH_SOCK", "/tmp/ssh-x/agent"),
            ("KUBECONFIG", "/h/.kube/config"),
            ("DOCKER_HOST", "unix:///var/run/docker.sock"),
            ("LC_ALL", "C.UTF-8"),
            ("XDG_STATE_HOME", "/h/.local/state"),
            ("MNEMORPH_BOARD_PID", "1"),
            ("https_proxy", "http://proxy:3128"),
            ("HTTPS_PROXY", "http://me:pw@proxy:3128"),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
        let env = child_env(&base, &["OPENROUTER_API_KEY".to_owned()]);
        let names: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            names,
            [
                "PATH",
                "HOME",
                "OPENROUTER_API_KEY",
                "LC_ALL",
                "XDG_STATE_HOME",
                "MNEMORPH_BOARD_PID",
                "https_proxy"
            ]
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
