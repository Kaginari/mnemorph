//! The board's server: `mnemorphd board` serves the built board and its API on loopback, to the
//! one browser that traded the printed key for a cookie.
//!
//! Every request passes one guard before any route sees it ([`guard`]): a loopback-literal
//! `Host` (DNS rebinding), a matching `Origin` (required on a mutation and a WebSocket upgrade),
//! no cross-site fetch, the session cookie on every route but the page, its assets, the key
//! exchange and the health check, and the CSRF token on every mutation and upgrade. Every
//! response, a refusal included, carries the same hardening headers. Nothing read from inside a
//! world grants trust: the worlds come from the registry and the human's list under the state
//! directory, and only their directories' presence is read.
//!
//! Routes are added in [`Board::router`]; the guard's policy is by path, so a new route under
//! `/api/` or `/ws/` is guarded without being named in it.

mod assets;
pub mod bodies;
pub mod bridge;
pub mod facts;
pub mod guard;
pub mod instance;
pub mod integrations;
pub mod journal;
pub mod knowledge;
pub mod memory;
pub mod metrics;
pub mod orders;
pub mod overview;
pub mod runs;
pub mod snapshot;
pub mod timeline;
pub mod watch;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::Json;
use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::json;
use subtle::ConstantTimeEq;
use ts_rs::TS;

use crate::world::dimension;
pub use overview::{BoardOverview, Chats};
pub use watch::Change;

/// The port the board listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 7878;

/// The session cookie's name.
pub const COOKIE: &str = "mnemorph_board";

/// The header a mutation carries its CSRF token in.
pub const CSRF_HEADER: &str = "x-mnemorph-csrf";

/// The `Sec-WebSocket-Protocol` entry an upgrade carries its CSRF token in: `mnemorph.csrf.<token>`
/// (a browser's `WebSocket` cannot set a header).
pub const CSRF_PROTOCOL: &str = "mnemorph.csrf.";

/// The most browsers holding a session at once; the oldest is dropped past it.
const SESSIONS: usize = 8;

/// The largest body the key exchange reads.
const EXCHANGE_BYTES: usize = 1024;

/// The changes a slow socket may fall behind by before it is told everything moved.
const CHANGES: usize = 256;
/// The largest body `POST /api/w/<world>/sessions` reads.
const START_BYTES: usize = 1024;

/// A world, as `GET /api/worlds` shows it to the browser holding the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardWorld {
    /// Its id across the state directory: `<name>-<fnv64a of the path>`.
    pub id: String,
    /// Its directory's name.
    pub name: String,
    /// Its canonical path.
    pub path: String,
    /// Its `origin` remote, normalised (`host/owner/repo`), if a session saw one.
    pub remote: Option<String>,
    /// When the latest session opened in it (RFC 3339, UTC); `null` for a world only added by
    /// hand.
    pub last: Option<String>,
    /// How many sessions opened in it.
    #[ts(type = "number")]
    pub sessions: u64,
    /// `present`, or `missing` when its directory is gone: never dropped silently.
    pub state: WorldState,
}

/// Whether a listed world's directory is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum WorldState {
    /// The directory is there.
    Present,
    /// The directory is gone.
    Missing,
}

/// What `GET /api/worlds` answers: the worlds, the latest session first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardWorlds {
    /// The worlds.
    pub worlds: Vec<BoardWorld>,
}

/// How long each secret lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lifetimes {
    /// The printed key, untraded: past it, a fresh one waits in its place.
    pub key: Duration,
    /// The key `--open` hands the browser: it never waits for a second.
    pub open: Duration,
    /// A session unused this long ends.
    pub idle: Duration,
    /// A session ends this long after it began, used or not.
    pub absolute: Duration,
}

impl Default for Lifetimes {
    fn default() -> Self {
        Self {
            key: Duration::from_mins(10),
            open: Duration::from_secs(60),
            idle: Duration::from_hours(12),
            absolute: Duration::from_hours(7 * 24),
        }
    }
}

/// One browser's session: its cookie, the CSRF token bound to it, when it began and was last
/// used.
struct Session {
    cookie: String,
    csrf: String,
    began: Instant,
    used: Instant,
}

/// The secrets: the one-time key waiting to be traded and when it was minted, the key handed to
/// an opened browser if one waits, and the sessions traded for.
struct Keys {
    pending: String,
    minted: Instant,
    open: Option<(String, Instant)>,
    sessions: Vec<Session>,
}

/// The board's server state.
pub struct Board {
    /// The address the printed URL names.
    host: SocketAddr,
    /// The `Host` values accepted: every bound address, literally.
    hosts: Vec<String>,
    /// The Content-Security-Policy, its port filled in.
    csp: HeaderValue,
    registry: PathBuf,
    hand: PathBuf,
    keys: Mutex<Keys>,
    lifetimes: Lifetimes,
    /// The board's clock: the system's, only ever moved on by [`Board::pass`].
    clock: bridge::Clock,
    /// The live chats.
    bridge: Arc<bridge::Bridge>,
    /// Told the URL with each new key, for a second launch to print.
    on_key: Box<dyn Fn(&str) + Send + Sync>,
    /// What the watcher tells the board's sockets.
    changes: tokio::sync::broadcast::Sender<Change>,
    /// The board sockets open.
    sockets: AtomicUsize,
    /// The worlds the watcher looks at slowly: past its budget.
    slowed: Mutex<std::collections::BTreeSet<String>>,
    /// The human's global configuration file; `None` reads it where every reader resolves it.
    global: Mutex<Option<PathBuf>>,
}

/// A fresh secret: 32 random bytes, in hex.
///
/// # Errors
///
/// The system's random source fails.
fn secret() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("the random source fails: {e}"))?;
    Ok(bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    }))
}

/// Whether `a` and `b` are the same secret, in time independent of where they differ.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}

/// `Ok` for a loopback literal the board may bind: `127.0.0.1` or `::1`. Anything else is
/// refused: remote use is an SSH port-forward.
///
/// # Errors
///
/// Any other address.
pub fn loopback(ip: IpAddr) -> Result<(), String> {
    if ip == IpAddr::V4(Ipv4Addr::LOCALHOST) || ip == IpAddr::V6(Ipv6Addr::LOCALHOST) {
        Ok(())
    } else {
        Err(loopback_only(&ip.to_string()))
    }
}

/// Why `addr` is not bound: the board binds loopback only.
#[must_use]
pub fn loopback_only(addr: &str) -> String {
    format!(
        "the board binds loopback only (127.0.0.1 or ::1), not {addr}; for another machine, \
         forward the port over SSH: ssh -L 7878:127.0.0.1:7878 <host>"
    )
}

/// Listeners on `port` (0 picks one) at `127.0.0.1` and `::1` — or only at `only`, which must be
/// one of them. A machine without IPv6 loopback is served on IPv4 alone.
///
/// # Errors
///
/// A non-loopback `only`, or the port cannot be bound.
pub fn listen(port: u16, only: Option<IpAddr>) -> Result<Vec<std::net::TcpListener>, String> {
    let ips = match only {
        Some(ip) => {
            loopback(ip)?;
            vec![ip]
        }
        None => vec![
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ],
    };
    let mut out: Vec<std::net::TcpListener> = Vec::new();
    let mut port = port;
    for ip in ips {
        let addr = SocketAddr::new(ip, port);
        loopback(addr.ip())?;
        match std::net::TcpListener::bind(addr) {
            Ok(l) => {
                port = l.local_addr().map_err(|e| e.to_string())?.port();
                out.push(l);
            }
            // No IPv6 on this machine: IPv4 serves alone.
            Err(e)
                if only.is_none()
                    && !out.is_empty()
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
                    ) => {}
            Err(e) => return Err(format!("{addr} cannot be bound: {e}")),
        }
    }
    Ok(out)
}

impl Board {
    /// The board over `listeners`' addresses, its worlds read from the registry at `registry`
    /// and the human's list at `hand`; `on_key` is told the URL with each new one-time key.
    ///
    /// # Errors
    ///
    /// No listener, an address that is not loopback, or the random source fails.
    pub fn new(
        listeners: &[std::net::TcpListener],
        registry: PathBuf,
        hand: PathBuf,
        on_key: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Result<Arc<Self>, String> {
        let addrs: Vec<SocketAddr> = listeners
            .iter()
            .map(std::net::TcpListener::local_addr)
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        Self::at(&addrs, registry, hand, on_key)
    }

    /// As [`Board::new`], over addresses alone: for a test that drives the router without a
    /// socket.
    ///
    /// # Errors
    ///
    /// No address, one that is not loopback, or the random source fails.
    pub fn at(
        addrs: &[SocketAddr],
        registry: PathBuf,
        hand: PathBuf,
        on_key: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Result<Arc<Self>, String> {
        Self::living(addrs, registry, hand, on_key, Lifetimes::default())
    }

    /// As [`Board::at`], with its secrets living `lifetimes`.
    ///
    /// # Errors
    ///
    /// No address, one that is not loopback, or the random source fails.
    pub fn living(
        addrs: &[SocketAddr],
        registry: PathBuf,
        hand: PathBuf,
        on_key: Box<dyn Fn(&str) + Send + Sync>,
        lifetimes: Lifetimes,
    ) -> Result<Arc<Self>, String> {
        Self::bridged(
            addrs,
            (registry, hand),
            on_key,
            lifetimes,
            bridge::Child::this(),
            bridge::Limits::default(),
        )
    }

    /// As [`Board::living`], its chats' children started as `child` says, within `limits`.
    ///
    /// # Errors
    ///
    /// No address, one that is not loopback, or the random source fails.
    pub fn bridged(
        addrs: &[SocketAddr],
        (registry, hand): (PathBuf, PathBuf),
        on_key: Box<dyn Fn(&str) + Send + Sync>,
        lifetimes: Lifetimes,
        child: bridge::Child,
        limits: bridge::Limits,
    ) -> Result<Arc<Self>, String> {
        let Some(&host) = addrs.first() else {
            return Err("the board has no address".to_owned());
        };
        for a in addrs {
            loopback(a.ip())?;
        }
        let csp = format!(
            "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; \
             font-src 'self'; connect-src 'self' ws://{host}; base-uri 'none'; \
             form-action 'none'; frame-ancestors 'none'; require-trusted-types-for 'script'"
        );
        let clock = bridge::Clock::default();
        Ok(Arc::new(Self {
            host,
            hosts: addrs.iter().map(ToString::to_string).collect(),
            csp: HeaderValue::from_str(&csp).map_err(|e| e.to_string())?,
            registry,
            hand,
            keys: Mutex::new(Keys {
                pending: secret()?,
                minted: Instant::now(),
                open: None,
                sessions: Vec::new(),
            }),
            lifetimes,
            bridge: Arc::new(bridge::Bridge::new(child, limits, clock.clone())),
            clock,
            on_key,
            changes: tokio::sync::broadcast::channel(CHANGES).0,
            sockets: AtomicUsize::new(0),
            slowed: Mutex::new(std::collections::BTreeSet::new()),
            global: Mutex::new(None),
        }))
    }

    /// The URL to open now: `http://127.0.0.1:<port>/#k=<one-time key>`. The key rides in the
    /// fragment, which a browser never sends nor logs.
    #[must_use]
    pub fn url(&self) -> String {
        let keys = self.keys.lock().unwrap_or_else(PoisonError::into_inner);
        format!("http://{}/#k={}", self.host, keys.pending)
    }

    /// The router: the key exchange, the API, and the assets, all behind the guard.
    pub fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route(
                "/session",
                post(exchange).layer(DefaultBodyLimit::max(EXCHANGE_BYTES)),
            )
            .route("/api/health", get(health))
            .route("/api/session", get(session))
            .route("/api/session/end", post(end))
            .route("/api/worlds", get(worlds))
            .route("/api/w/{world}", get(world_overview))
            .route("/api/w/{world}/memory", get(knowledge::get_memory))
            .route(
                "/api/w/{world}/memory/{note}/confirm",
                post(knowledge::confirm).layer(DefaultBodyLimit::max(knowledge::ACT_BYTES)),
            )
            .route(
                "/api/w/{world}/memory/{note}/wake",
                post(knowledge::wake).layer(DefaultBodyLimit::max(knowledge::ACT_BYTES)),
            )
            .route("/api/w/{world}/facts", get(knowledge::get_facts))
            .route(
                "/api/w/{world}/facts/{fact}/settle",
                post(knowledge::settle).layer(DefaultBodyLimit::max(knowledge::ACT_BYTES)),
            )
            .route("/api/w/{world}/bodies", get(knowledge::get_bodies))
            .route("/api/w/{world}/queue", get(knowledge::get_queue))
            .route("/api/w/{world}/timeline", get(knowledge::get_timeline))
            .route("/api/w/{world}/journal", get(knowledge::get_journal))
            .route("/api/metrics", get(metrics_of))
            .route("/api/integrations", get(integrations_of))
            .route("/ws/board", get(watch::socket))
            .route(
                "/api/w/{world}/sessions",
                post(start).layer(DefaultBodyLimit::max(START_BYTES)),
            )
            .route("/api/sessions", get(sessions))
            .route("/api/w/{world}/runs", get(runs_of))
            .route("/api/w/{world}/runs/{run}", get(run_of))
            .route("/ws/session/{id}", get(socket))
            .fallback(asset)
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(self),
                guard::guard,
            ))
            .with_state(Arc::clone(self))
    }

    /// Now, on the board's clock.
    fn now(&self) -> Instant {
        self.clock.now()
    }

    /// How long ago `at` was, on the board's clock.
    fn since(&self, at: Instant) -> Duration {
        self.now().saturating_duration_since(at)
    }

    /// Move the board's clock on by `by`, as if that much time had passed: for a test of the
    /// lifetimes that need not sleep through them.
    #[doc(hidden)]
    pub fn pass(&self, by: Duration) {
        self.clock.pass(by);
    }

    /// The live chats.
    #[must_use]
    pub fn bridge(&self) -> &Arc<bridge::Bridge> {
        &self.bridge
    }

    /// A URL for `--open` to hand the browser: a key of its own, valid once and for
    /// [`Lifetimes::open`] — a command line is readable by every local user while it runs, so
    /// this key is spent by the browser it opens or dies within the minute. It replaces any
    /// earlier one.
    ///
    /// # Errors
    ///
    /// The random source fails.
    pub fn open_url(&self) -> Result<String, String> {
        let key = secret()?;
        let mut keys = self.keys.lock().unwrap_or_else(PoisonError::into_inner);
        keys.open = Some((key.clone(), self.now()));
        Ok(format!("http://{}/#k={key}", self.host))
    }

    /// Put a fresh key in place of a printed one past [`Lifetimes::key`], and tell `on_key`;
    /// end the sessions past their lifetimes and drop an open key past its own.
    ///
    /// # Errors
    ///
    /// The random source fails.
    pub fn refresh(&self) -> Result<(), String> {
        let fresh = secret()?;
        let url = {
            let mut keys = self.keys.lock().unwrap_or_else(PoisonError::into_inner);
            self.expire(&mut keys);
            if self.since(keys.minted) < self.lifetimes.key {
                return Ok(());
            }
            keys.pending = fresh;
            keys.minted = self.now();
            format!("http://{}/#k={}", self.host, keys.pending)
        };
        (self.on_key)(&url);
        Ok(())
    }

    /// Drop the open key and the sessions past their lifetimes.
    fn expire(&self, keys: &mut Keys) {
        let l = self.lifetimes;
        if keys
            .open
            .as_ref()
            .is_some_and(|(_, at)| self.since(*at) >= l.open)
        {
            keys.open = None;
        }
        keys.sessions
            .retain(|s| self.since(s.began) < l.absolute && self.since(s.used) < l.idle);
    }

    /// Trade `key` for a session: the printed key, or the open one, dies and a printed one
    /// waits fresh, whatever comes next. `None` for a key that is neither, or is past its life.
    fn trade(&self, key: &str) -> Result<Option<Session>, String> {
        self.refresh()?;
        let (fresh, cookie, csrf) = (secret()?, secret()?, secret()?);
        let url = {
            let mut keys = self.keys.lock().unwrap_or_else(PoisonError::into_inner);
            // Both compared always, so the time taken says nothing of which matched.
            let printed = same(&keys.pending, key);
            let opened = keys.open.as_ref().is_some_and(|(k, _)| same(k, key));
            if !printed && !opened {
                return Ok(None);
            }
            if opened {
                keys.open = None;
            }
            keys.pending = fresh;
            keys.minted = self.now();
            if keys.sessions.len() >= SESSIONS {
                keys.sessions.remove(0);
            }
            let now = self.now();
            keys.sessions.push(Session {
                cookie: cookie.clone(),
                csrf: csrf.clone(),
                began: now,
                used: now,
            });
            format!("http://{}/#k={}", self.host, keys.pending)
        };
        (self.on_key)(&url);
        let now = self.now();
        Ok(Some(Session {
            cookie,
            csrf,
            began: now,
            used: now,
        }))
    }

    /// The CSRF token of the session `cookie` names, if it names one still alive; its use is
    /// recorded.
    fn csrf_of(&self, cookie: &str) -> Option<String> {
        let mut keys = self.keys.lock().unwrap_or_else(PoisonError::into_inner);
        self.expire(&mut keys);
        // Every session is compared, so the time taken says nothing of which matched.
        let mut found = None;
        for s in &mut keys.sessions {
            if same(&s.cookie, cookie) {
                s.used = self.now();
                found = Some(s.csrf.clone());
            }
        }
        found
    }

    /// Whether `cookie` names a session still alive — its use not recorded: a socket's check
    /// keeps no session from idling out.
    fn alive(&self, cookie: &str) -> bool {
        let mut keys = self.keys.lock().unwrap_or_else(PoisonError::into_inner);
        self.expire(&mut keys);
        keys.sessions
            .iter()
            .fold(false, |found, s| same(&s.cookie, cookie) | found)
    }

    /// The overview of the listed world `id`; `None` when no listed world has it.
    #[must_use]
    pub fn overview(&self, id: &str) -> Option<BoardOverview> {
        let world = self.worlds().worlds.into_iter().find(|w| w.id == id)?;
        let chats = self
            .bridge
            .list()
            .sessions
            .iter()
            .filter(|s| s.world == id)
            .fold(Chats::default(), |c, s| Chats {
                open: c.open.saturating_add(1),
                asks: c
                    .asks
                    .saturating_add(u32::try_from(s.asks).unwrap_or(u32::MAX)),
            });
        let slowed = self
            .slowed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(id);
        Some(overview::overview(world, chats, slowed))
    }

    /// Read the human's global configuration from `file` rather than where every reader
    /// resolves it: for a test.
    #[doc(hidden)]
    pub fn configure(&self, file: PathBuf) {
        *self.global.lock().unwrap_or_else(PoisonError::into_inner) = Some(file);
    }

    /// The human's global configuration file.
    fn global_file(&self) -> PathBuf {
        self.global
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .unwrap_or_else(integrations::global_file)
    }

    /// The board's metrics over `window`, of the listed world `world` or of `all`; `None` when no
    /// listed world has the id.
    #[must_use]
    pub fn metrics(
        &self,
        world: &str,
        window: mnemorph_loop::world::spend::Window,
        now: mnemorph_loop::world::ledger::Timestamp,
    ) -> Option<metrics::BoardMetrics> {
        let all = self.worlds().worlds;
        let of: Vec<BoardWorld> = if world == "all" {
            all
        } else {
            vec![all.into_iter().find(|w| w.id == world)?]
        };
        let global = self.global_file();
        let paths = mnemorph_loop::Paths::with_global(global, std::path::Path::new("/nonexistent"));
        let bench = mnemorph_loop::Config::load_global(&paths)
            .map(|c| c.bench_results())
            .map_err(|e| {
                format!(
                    "your global file does not read: {}",
                    integrations::first_line(&e)
                )
            });
        Some(metrics::metrics(world, &of, window, now, bench))
    }

    /// The integrations, of the listed world `world`'s files over the global one, or of the global
    /// file alone; `None` when no listed, present world has the id.
    #[must_use]
    pub fn integrations(&self, world: Option<&str>) -> Option<integrations::BoardIntegrations> {
        let at = match world {
            Some(id) => {
                let w = self.worlds().worlds.into_iter().find(|w| w.id == id)?;
                if w.state == WorldState::Missing {
                    return None;
                }
                Some((w.id, PathBuf::from(w.path)))
            }
            None => None,
        };
        let board = integrations::BoardSettings {
            port: self.host.port(),
            hosts: self.hosts.clone(),
        };
        let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        // A key file may lie in no world this machine knows, listed or not.
        let files = integrations::KeyFiles {
            home: std::env::var("HOME").ok(),
            worlds: mnemorph_loop::key::worlds_at(&self.registry),
        };
        Some(integrations::integrations(
            &self.global_file(),
            at.as_ref().map(|(id, p)| (id.as_str(), p.as_path())),
            board,
            &set,
            &files,
        ))
    }

    /// Tell the board's sockets `change`, as the watcher does: for a test.
    #[doc(hidden)]
    pub fn tell(&self, change: Change) {
        let _ = self.changes.send(change);
    }

    /// End the session `cookie` names.
    fn end(&self, cookie: &str) {
        let mut keys = self.keys.lock().unwrap_or_else(PoisonError::into_inner);
        keys.sessions.retain(|s| !same(&s.cookie, cookie));
    }

    /// The worlds the board lists, the latest session first.
    #[must_use]
    pub fn worlds(&self) -> BoardWorlds {
        let mut worlds: Vec<BoardWorld> = dimension::listed(&self.registry, &self.hand)
            .into_iter()
            .map(|l| BoardWorld {
                name: l
                    .world
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: l.world.to_string_lossy().into_owned(),
                state: if l.world.is_dir() {
                    WorldState::Present
                } else {
                    WorldState::Missing
                },
                id: l.id,
                remote: l.remote,
                last: l.last,
                sessions: l.sessions,
            })
            .collect();
        // RFC 3339 in UTC sorts as text; a world never opened goes last.
        worlds.sort_by(|a, b| b.last.cmp(&a.last));
        BoardWorlds { worlds }
    }
}

/// The key exchange's body.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    key: String,
}

/// `POST /session {"key"}`: the one-time key traded for the session cookie, answered with the
/// CSRF token bound to it.
async fn exchange(State(board): State<Arc<Board>>, Json(body): Json<Key>) -> Response {
    match board.trade(&body.key) {
        Ok(Some(s)) => (
            [(
                header::SET_COOKIE,
                format!("{COOKIE}={}; HttpOnly; SameSite=Strict; Path=/", s.cookie),
            )],
            Json(json!({ "csrf": s.csrf })),
        )
            .into_response(),
        Ok(None) => refuse(
            StatusCode::UNAUTHORIZED,
            "the key is not valid: it works once; `mnemorphd board` prints the current one",
        ),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

/// `GET /api/health`: the server answers. Says nothing else.
async fn health() -> Response {
    Json(json!({ "ok": true })).into_response()
}

/// `GET /api/session`: the CSRF token of the session the cookie holds — for a page reloaded,
/// which lost it.
async fn session(State(board): State<Arc<Board>>, headers: axum::http::HeaderMap) -> Response {
    match guard::cookie(&headers).find_map(|c| board.csrf_of(c)) {
        Some(csrf) => Json(json!({ "csrf": csrf })).into_response(),
        None => refuse(StatusCode::UNAUTHORIZED, "no session"),
    }
}

/// `POST /api/session/end`: the session ends and its cookie is cleared.
async fn end(State(board): State<Arc<Board>>, headers: axum::http::HeaderMap) -> Response {
    for c in guard::cookie(&headers) {
        board.end(c);
    }
    (
        StatusCode::NO_CONTENT,
        [(
            header::SET_COOKIE,
            format!("{COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
        )],
    )
        .into_response()
}

/// `GET /api/worlds`: [`BoardWorlds`].
async fn worlds(State(board): State<Arc<Board>>) -> Response {
    match tokio::task::spawn_blocking(move || board.worlds()).await {
        Ok(w) => Json(w).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// `GET /api/w/<id>`: [`BoardOverview`] of a listed world, or 404.
async fn world_overview(State(board): State<Arc<Board>>, Path(id): Path<String>) -> Response {
    match tokio::task::spawn_blocking(move || board.overview(&id)).await {
        Ok(Some(o)) => Json(o).into_response(),
        Ok(None) => refuse(StatusCode::NOT_FOUND, "no listed world has this id"),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// The value of `name` in a query string, percent-decoded; `None` when it is absent or does not
/// decode.
fn query(uri: &Uri, name: &str) -> Option<String> {
    let q = uri.query()?;
    let raw = q
        .split('&')
        .filter_map(|p| p.split_once('=').or(Some((p, ""))))
        .find(|(k, _)| *k == name)?
        .1;
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// `value` as JSON, unless it holds the value of a secret variable in the board's environment:
/// then it is withheld whole.
fn withheld<T: Serialize>(value: &T) -> Response {
    let Ok(bytes) = serde_json::to_vec(value) else {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the answer did not serialise",
        );
    };
    if integrations::leaks(&bytes, &integrations::secret_values(std::env::vars_os())) {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "withheld: the answer held the value of a secret variable",
        );
    }
    ([(header::CONTENT_TYPE, "application/json")], bytes).into_response()
}

/// `GET /api/metrics?world=<id|all>&window=7d|28d|all`: [`metrics::BoardMetrics`]; `all` and
/// `7d` unless asked.
async fn metrics_of(State(board): State<Arc<Board>>, uri: Uri) -> Response {
    use mnemorph_loop::world::spend::Window;
    let world = query(&uri, "world").unwrap_or_else(|| "all".to_owned());
    let Some(window) = Window::parse(query(&uri, "window").as_deref().unwrap_or("7d")) else {
        return refuse(StatusCode::BAD_REQUEST, "`window` is 7d, 28d or all");
    };
    let now = mnemorph_loop::world::ledger::Timestamp::now();
    match tokio::task::spawn_blocking(move || board.metrics(&world, window, now)).await {
        Ok(Some(m)) => withheld(&m),
        Ok(None) => refuse(StatusCode::NOT_FOUND, "no listed world has this id"),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// `GET /api/integrations[?world=<id>]`: [`integrations::BoardIntegrations`], read-only and
/// redacted.
async fn integrations_of(State(board): State<Arc<Board>>, uri: Uri) -> Response {
    let world = query(&uri, "world");
    match tokio::task::spawn_blocking(move || board.integrations(world.as_deref())).await {
        Ok(Some(i)) => withheld(&i),
        Ok(None) => refuse(
            StatusCode::NOT_FOUND,
            "no listed, present world has this id",
        ),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// `POST /api/w/<world>/sessions`'s body: `{}` for a new chat, `{"resume": "<run>|last"}` for
/// a past run.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    #[serde(default)]
    resume: Option<String>,
}

/// Whether `run` may name a run to resume: `last`, or a session's run id (`r<ms>-<pid>`) — never
/// a path, `.` or `..`.
fn run_name(run: &str) -> bool {
    run == "last" || mnemorph_loop::resume::is_session_run(run)
}

/// The cookie on `headers` that names a live session.
fn live_cookie(board: &Board, headers: &axum::http::HeaderMap) -> Option<String> {
    guard::cookie(headers)
        .find(|c| board.csrf_of(c).is_some())
        .map(str::to_owned)
}

/// `POST /api/w/<world>/sessions`: a chat in a listed world, new or resuming a past run, for
/// this browser to control. Answers [`bridge::BoardStarted`].
async fn start(
    State(board): State<Arc<Board>>,
    Path(world): Path<String>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Start>,
) -> Response {
    if let Some(run) = &body.resume
        && !run_name(run)
    {
        return refuse(
            StatusCode::BAD_REQUEST,
            "`resume` names a run's id, or `last`",
        );
    }
    let Some(owner) = live_cookie(&board, &headers) else {
        return refuse(StatusCode::UNAUTHORIZED, "no session");
    };
    let listing = Arc::clone(&board);
    let Ok(worlds) = tokio::task::spawn_blocking(move || listing.worlds()).await else {
        return refuse(StatusCode::INTERNAL_SERVER_ERROR, "the worlds did not read");
    };
    let Some(w) = worlds.worlds.into_iter().find(|w| w.id == world) else {
        return refuse(StatusCode::NOT_FOUND, "no such world on the board");
    };
    if w.state == WorldState::Missing {
        return refuse(StatusCode::CONFLICT, "the world's directory is gone");
    }
    match board
        .bridge
        .start(
            std::path::Path::new(&w.path),
            &w.id,
            body.resume.as_deref(),
            &owner,
        )
        .await
    {
        Ok(started) => (StatusCode::CREATED, Json(started)).into_response(),
        Err((status, why)) => refuse(
            StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
            &why,
        ),
    }
}

/// The path of the listed world `id`, if it is listed and present.
fn present(board: &Board, id: &str) -> Result<std::path::PathBuf, Box<Response>> {
    let Some(w) = board.worlds().worlds.into_iter().find(|w| w.id == id) else {
        return Err(Box::new(refuse(
            StatusCode::NOT_FOUND,
            "no such world on the board",
        )));
    };
    if w.state == WorldState::Missing {
        return Err(Box::new(refuse(
            StatusCode::CONFLICT,
            "the world's directory is gone",
        )));
    }
    Ok(std::path::PathBuf::from(w.path))
}

/// `GET /api/w/<world>/runs`: [`runs::BoardRuns`], the world's past chats from its journals.
async fn runs_of(State(board): State<Arc<Board>>, Path(world): Path<String>) -> Response {
    let read = tokio::task::spawn_blocking(move || present(&board, &world).map(|p| runs::list(&p)));
    match read.await {
        Ok(Ok(r)) => Json(r).into_response(),
        Ok(Err(refused)) => *refused,
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// `GET /api/w/<world>/runs/<run>`: [`runs::BoardTranscript`], a past chat read-only.
async fn run_of(
    State(board): State<Arc<Board>>,
    Path((world, run)): Path<(String, String)>,
) -> Response {
    let read = tokio::task::spawn_blocking(move || {
        present(&board, &world).map(|p| runs::transcript(&p, &run))
    });
    match read.await {
        Ok(Ok(Ok(t))) => Json(t).into_response(),
        Ok(Ok(Err((status, why)))) => refuse(
            StatusCode::from_u16(status).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY),
            &why,
        ),
        Ok(Err(refused)) => *refused,
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// `GET /api/sessions`: [`bridge::BoardSessions`].
async fn sessions(State(board): State<Arc<Board>>) -> Response {
    Json(board.bridge.list()).into_response()
}

/// `WS /ws/session/<id>`: a tab on a live chat. The guard has checked the Host, the Origin, the
/// cookie and the CSRF subprotocol entry; the socket speaks [`bridge::SUBPROTOCOL`].
async fn socket(
    State(board): State<Arc<Board>>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let offered = headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|p| p.trim() == bridge::SUBPROTOCOL);
    if !offered {
        return refuse(
            StatusCode::BAD_REQUEST,
            "the chat's socket speaks the subprotocol mnemorph.v1",
        );
    }
    let Some(cookie) = live_cookie(&board, &headers) else {
        return refuse(StatusCode::UNAUTHORIZED, "no session");
    };
    let limits = board.bridge.limits();
    let bridge = Arc::clone(&board.bridge);
    upgrade
        .protocols([bridge::SUBPROTOCOL])
        .max_message_size(limits.inbound)
        .max_frame_size(limits.inbound)
        .on_upgrade(move |ws| bridge.serve(board, id, cookie, ws))
}

/// Any other path: an embedded asset by its exact name, or 404. `/` is `/index.html`.
async fn asset(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return refuse(StatusCode::METHOD_NOT_ALLOWED, "method not allowed");
    }
    let name = match uri.path() {
        "/" => "/index.html",
        p => p,
    };
    match assets::find(name) {
        Some((ty, bytes)) => ([(header::CONTENT_TYPE, ty)], bytes).into_response(),
        None => refuse(StatusCode::NOT_FOUND, "not found"),
    }
}

/// Whether the real board was embedded at build time, not the placeholder page.
#[must_use]
pub fn built() -> bool {
    assets::BUILT
}

/// A refusal: its status and a short plain-text reason.
fn refuse(status: StatusCode, why: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        why.to_owned(),
    )
        .into_response()
}

/// Serve `board` on `listeners` until Ctrl-C or SIGTERM.
///
/// # Errors
///
/// The runtime cannot start, or a listener cannot be served.
pub fn run(listeners: Vec<std::net::TcpListener>, board: &Arc<Board>) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let router = board.router();
    rt.block_on(async move {
        let (stop, stopped) = tokio::sync::watch::channel(false);
        let mut tasks = Vec::new();
        for l in listeners {
            l.set_nonblocking(true).map_err(|e| e.to_string())?;
            let l = tokio::net::TcpListener::from_std(l).map_err(|e| e.to_string())?;
            let mut stopped = stopped.clone();
            let serve = axum::serve(l, router.clone()).with_graceful_shutdown(async move {
                let _ = stopped.changed().await;
            });
            tasks.push(tokio::spawn(async move { serve.await }));
        }
        // A printed key past its life is replaced, and the new URL kept, on the minute.
        let refresher = Arc::clone(board);
        let ticker = tokio::spawn(async move {
            let mut every = tokio::time::interval(Duration::from_secs(30));
            loop {
                every.tick().await;
                let _ = refresher.refresh();
            }
        });
        // The worlds' files, watched: each change is told on `WS /ws/board`.
        let watcher = tokio::spawn(watch::watch(Arc::clone(board)));
        signal().await;
        ticker.abort();
        watcher.abort();
        // Every chat's child asked to leave, then its group killed: none outlives the board.
        board.bridge.end_all("the board stopped").await;
        let _ = stop.send(true);
        for t in tasks {
            t.await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

/// Ctrl-C, or SIGTERM where there are signals.
async fn signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
