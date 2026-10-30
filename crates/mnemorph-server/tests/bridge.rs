//! The board's chats over a real socket: a child per chat — here this test binary re-run as a
//! stand-in `mnemorphd` (`fake_child`) — bridged to tabs on `WS /ws/session/<sid>`. A killed
//! socket replays every event once, a waiting ask survives a reload and is denied past the grace,
//! an observer acts on nothing, the upgrade is guarded, the child's environment is scrubbed and
//! its group dies with the chat and the board, and the child's text passes untouched within the
//! caps.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{BufRead as _, Write as _};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use futures_util::{SinkExt as _, StreamExt as _};
use http_body_util::BodyExt as _;
use mnemorph_server::board::bridge::{Child, Limits};
use mnemorph_server::board::{Board, Lifetimes, listen};
use mnemorph_server::world::dimension::{hand_file, register_at};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{self, Message, client::IntoClientRequest as _};
use tower::ServiceExt as _;

// ---------------------------------------------------------------------------------------------
// The stand-in child: the protocol's side of `mnemorphd`, scripted by the prompt's text.
// ---------------------------------------------------------------------------------------------

/// The marker that turns this test into the stand-in child.
const FAKE: &str = "MNEMORPH_FAKE_CHILD";

/// The hostile text the child says: markup, a bidi override, an escape, a NUL.
const HOSTILE: &str = "<script>alert(1)</script><img src=x onerror=alert(2)> \u{202e}gnp.exe \u{1b}[31mred\u{1b}[0m \u{0} javascript:alert(3)";

struct Fake {
    seq: u64,
    out: std::io::Stdout,
}

impl Fake {
    fn say(&mut self, v: &Value) {
        let mut out = self.out.lock();
        let _ = writeln!(out, "{v}");
        let _ = out.flush();
    }

    fn event(&mut self, kind: &str, fields: &Value) {
        self.seq += 1;
        let mut params =
            json!({"session": "fake-s", "run": "fake-r", "body": "rimuru", "seq": self.seq});
        for (k, v) in fields.as_object().unwrap() {
            params[k] = v.clone();
        }
        self.say(&json!({"jsonrpc": "2.0", "method": format!("event/{kind}"), "params": params}));
    }

    fn prompt(&mut self, text: &str) {
        let words: Vec<&str> = text.split_whitespace().collect();
        self.event("turnStarted", &json!({"text": text}));
        match words.as_slice() {
            ["events", n, ms] => {
                for i in 0..n.parse::<u64>().unwrap() {
                    self.event("textDelta", &json!({"text": format!("{i} ")}));
                    std::thread::sleep(Duration::from_millis(ms.parse().unwrap()));
                }
            }
            ["big", n] => {
                for _ in 0..n.parse::<u64>().unwrap() {
                    self.event("note", &json!({"text": "b".repeat(60_000)}));
                }
            }
            ["ask"] => {
                self.say(
                    &json!({"jsonrpc": "2.0", "id": "gate-1", "method": "gate/ask", "params": {
                    "body": "rimuru", "race": "rimuru", "class": "write", "act": "rm -rf build",
                    "why": ["writes"], "outside": []}}),
                );
                // The turn ends once the answer comes.
                return;
            }
            ["env"] => {
                let env: std::collections::BTreeMap<String, String> = std::env::vars().collect();
                self.event(
                    "note",
                    &json!({"text": serde_json::to_string(&env).unwrap()}),
                );
            }
            ["spawn"] => {
                let sleeper = std::process::Command::new("sleep")
                    .arg("300")
                    .spawn()
                    .unwrap();
                self.event(
                    "note",
                    &json!({"text": format!("{} {}", std::process::id(), sleeper.id())}),
                );
                std::mem::forget(sleeper);
            }
            ["crash"] => {
                eprintln!("panicked: \u{1b}[31mthe model's reply did not parse\u{1b}[0m");
                std::process::exit(3);
            }
            ["hostile"] => {
                self.event("note", &json!({"text": HOSTILE}));
                self.event("note", &json!({"text": "x".repeat(200_000)}));
                self.event("note", &json!({"text": "after"}));
            }
            _ => {}
        }
        self.event("turnEnded", &json!({"ending": "answered"}));
    }
}

/// The stand-in child, when this binary is run as one; a no-op test otherwise.
#[test]
fn fake_child() {
    if std::env::var_os(FAKE).is_none() {
        return;
    }
    let mut fake = Fake {
        seq: 0,
        out: std::io::stdout(),
    };
    // libtest's own `test fake_child ... ` has no newline: end it, so the bridge drops it whole.
    fake.say(&json!(null));
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let msg: Value = serde_json::from_str(&line).unwrap();
        let id = msg["id"].clone();
        match msg["method"].as_str() {
            Some("initialize") => fake.say(&json!({"jsonrpc": "2.0", "id": id, "result": {
                "protocol": 1, "version": "fake", "world": std::env::current_dir().unwrap(),
                "model": "m", "provider": "p", "session": "fake-s", "run": "fake-r",
                "holes": [], "budget": null, "key": null, "sandbox": "none", "body": "rimuru",
                "held": 0}})),
            Some("session/prompt") => {
                fake.say(&json!({"jsonrpc": "2.0", "id": id, "result": {"queued": false}}));
                fake.prompt(msg["params"]["text"].as_str().unwrap_or_default());
            }
            Some("shutdown") => {
                fake.say(&json!({"jsonrpc": "2.0", "id": id, "result": {}}));
                break;
            }
            Some(_) => fake.say(&json!({"jsonrpc": "2.0", "id": id, "result": {}})),
            None => {
                let r = &msg["result"];
                fake.event(
                    "note",
                    &json!({"text": format!("answer {} allow={} reason={}", id, r["allow"], r["reason"])}),
                );
                fake.event("turnEnded", &json!({"ending": "answered"}));
            }
        }
    }
    std::process::exit(0);
}

// ---------------------------------------------------------------------------------------------
// The rig: a board on a real loopback socket, a world, a browser's cookie and token.
// ---------------------------------------------------------------------------------------------

struct Rig {
    root: PathBuf,
    board: Arc<Board>,
    addr: SocketAddr,
    world: String,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A browser: its cookie and CSRF token.
#[derive(Clone)]
struct Browser {
    cookie: String,
    csrf: String,
}

fn rig(name: &str, limits: Limits) -> Rig {
    let root = std::env::temp_dir().join(format!("mnemorph-bridge-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("world")).unwrap();
    let root = root.canonicalize().unwrap();
    let world = root.join("world");
    let registry = root.join("state/mnemorph/worlds.jsonl");
    register_at(&registry, &world, None).unwrap();
    // The human's global file names the provider key: the only secret the child gets.
    let global = root.join("global.toml");
    std::fs::write(
        &global,
        "[providers.fake]\nkind = \"openai\"\nbase_url = \"http://127.0.0.1:9\"\nkey_env = \"FAKE_PROVIDER_KEY\"\n",
    )
    .unwrap();
    let env = [
        ("PATH", std::env::var("PATH").unwrap_or_default()),
        (FAKE, "1".to_owned()),
        ("FAKE_PROVIDER_KEY", "sk-fake-provider".to_owned()),
        ("OTHER_API_TOKEN", "hunter2".to_owned()),
        ("DEPLOY_URL", "https://deploy:pw@example.com/x".to_owned()),
        ("AWS_PROFILE", "prod".to_owned()),
        ("SSH_AUTH_SOCK", "/tmp/ssh-x/agent".to_owned()),
        ("LANG", "C".to_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let child = Child {
        program: std::env::current_exe().unwrap(),
        args: Arc::new(|_, _| {
            [
                "--exact",
                "fake_child",
                "--nocapture",
                "--test-threads",
                "1",
            ]
            .map(OsString::from)
            .to_vec()
        }),
        env,
        global,
    };
    let listeners = listen(0, Some("127.0.0.1".parse().unwrap())).unwrap();
    let addr = listeners[0].local_addr().unwrap();
    let board = Board::bridged(
        &[addr],
        (registry.clone(), hand_file(&registry)),
        Box::new(|_| {}),
        Lifetimes::default(),
        child,
        limits,
    )
    .unwrap();
    let l = listeners.into_iter().next().unwrap();
    l.set_nonblocking(true).unwrap();
    let l = tokio::net::TcpListener::from_std(l).unwrap();
    tokio::spawn(axum::serve(l, board.router()).into_future());
    Rig {
        world: mnemorph_server::world::dimension::world_id(&world),
        root,
        board,
        addr,
    }
}

impl Rig {
    fn origin(&self) -> String {
        format!("http://{}", self.addr)
    }

    async fn call(&self, req: Request<Body>) -> (StatusCode, Value) {
        let res = self.board.router().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes))),
        )
    }

    /// Trade the waiting key: a browser.
    async fn login(&self) -> Browser {
        let key = self.board.url().split_once("#k=").unwrap().1.to_owned();
        let req = Request::post("/session")
            .header(header::HOST, self.addr.to_string())
            .header(header::ORIGIN, self.origin())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "key": key }).to_string()))
            .unwrap();
        let res = self.board.router().oneshot(req).await.unwrap();
        let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap();
        let cookie = cookie.split(';').next().unwrap().to_owned();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let csrf = serde_json::from_slice::<Value>(&bytes).unwrap()["csrf"]
            .as_str()
            .unwrap()
            .to_owned();
        Browser { cookie, csrf }
    }

    /// Start a chat for `b`: its id.
    async fn start(&self, b: &Browser) -> String {
        let (status, body) = self.post_start(b).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["hello"]["session"], "fake-s");
        body["session"]["id"].as_str().unwrap().to_owned()
    }

    async fn post_start(&self, b: &Browser) -> (StatusCode, Value) {
        self.post_body(b, "{}").await
    }

    async fn post_body(&self, b: &Browser, body: &'static str) -> (StatusCode, Value) {
        self.call(
            Request::post(format!("/api/w/{}/sessions", self.world))
                .header(header::HOST, self.addr.to_string())
                .header(header::ORIGIN, self.origin())
                .header(header::COOKIE, &b.cookie)
                .header("x-mnemorph-csrf", &b.csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
    }

    async fn live(&self, b: &Browser) -> Vec<Value> {
        let (status, body) = self
            .call(
                Request::get("/api/sessions")
                    .header(header::HOST, self.addr.to_string())
                    .header(header::COOKIE, &b.cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        body["sessions"].as_array().unwrap().clone()
    }

    /// A tab of `b` on chat `sid`, its upgrade carrying `origin` and `protocols`.
    async fn connect(
        &self,
        b: &Browser,
        sid: &str,
        origin: &str,
        protocols: &str,
    ) -> Result<Tab, tungstenite::Error> {
        let mut req = format!("ws://{}/ws/session/{sid}", self.addr)
            .into_client_request()
            .unwrap();
        let h = req.headers_mut();
        h.insert(header::ORIGIN, origin.parse().unwrap());
        h.insert(header::COOKIE, b.cookie.parse().unwrap());
        if !protocols.is_empty() {
            h.insert(header::SEC_WEBSOCKET_PROTOCOL, protocols.parse().unwrap());
        }
        let (ws, _) = tokio_tungstenite::connect_async(req).await?;
        Ok(Tab { ws, closed: None })
    }

    async fn tab(&self, b: &Browser, sid: &str) -> Tab {
        self.connect(
            b,
            sid,
            &self.origin(),
            &format!("mnemorph.v1, mnemorph.csrf.{}", b.csrf),
        )
        .await
        .unwrap()
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Tab {
    ws: Socket,
    /// The close frame's code, once closed.
    closed: Option<u16>,
}

impl Tab {
    async fn send(&mut self, v: &Value) {
        self.ws.send(Message::text(v.to_string())).await.unwrap();
    }

    async fn request(&mut self, id: u64, method: &str, params: &Value) {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
    }

    /// The next message as text; `None` once closed.
    async fn text(&mut self) -> Option<String> {
        loop {
            let next = tokio::time::timeout(Duration::from_secs(20), self.ws.next())
                .await
                .expect("a message within 20 s");
            match next {
                Some(Ok(Message::Text(t))) => return Some(t.as_str().to_owned()),
                Some(Ok(Message::Close(frame))) => {
                    self.closed = frame.map(|f| u16::from(f.code));
                    return None;
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return None,
            }
        }
    }

    async fn next(&mut self) -> Option<Value> {
        self.text().await.map(|t| serde_json::from_str(&t).unwrap())
    }

    /// Messages until one matches `done`, it included.
    async fn until(&mut self, mut done: impl FnMut(&Value) -> bool) -> Vec<Value> {
        let mut seen = Vec::new();
        while let Some(m) = self.next().await {
            let stop = done(&m);
            seen.push(m);
            if stop {
                return seen;
            }
        }
        panic!(
            "closed ({:?}) before the awaited message: {seen:?}",
            self.closed
        );
    }

    /// The role it is told next.
    async fn role(&mut self) -> String {
        let m = self.until(|m| m["method"] == "board/role").await;
        m.last().unwrap()["params"]["role"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}

fn method(m: &Value) -> &str {
    m["method"].as_str().unwrap_or_default()
}

fn note(m: &Value) -> Option<&str> {
    (method(m) == "event/note")
        .then(|| m["params"]["text"].as_str())
        .flatten()
}

/// Whether process `pid` is gone (or a zombie, waiting to be reaped by whoever adopted it): asked
/// of `ps`, which reads it on Linux and macOS alike.
fn dead(pid: u32) -> bool {
    let out = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let stat = String::from_utf8_lossy(&out.stdout);
    let stat = stat.trim();
    stat.is_empty() || stat.starts_with('Z')
}

async fn all_dead(pids: &[u32]) -> bool {
    for _ in 0..100 {
        if pids.iter().all(|&p| dead(p)) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_socket_killed_mid_turn_delivers_every_event_once_after_reconnect() {
    // Small pages, so the replay takes several.
    let r = rig(
        "replay",
        Limits {
            page: 2048,
            ..Limits::default()
        },
    );
    let b = r.login().await;
    let sid = r.start(&b).await;
    let mut tab = r.tab(&b, &sid).await;
    assert_eq!(tab.role().await, "controller");
    tab.request(1, "session/prompt", &json!({"text": "events 300 2"}))
        .await;
    // The client's view: every event it delivered, by seq.
    let mut delivered: Vec<u64> = Vec::new();
    while delivered.len() < 40 {
        let m = tab.next().await.unwrap();
        if let Some(seq) = m["params"]["seq"].as_u64()
            && method(&m).starts_with("event/")
        {
            delivered.push(seq);
        }
    }
    // Killed: no close frame, the turn still running.
    drop(tab);
    tokio::time::sleep(Duration::from_millis(300)).await;

    let mut tab = r.tab(&b, &sid).await;
    assert_eq!(
        tab.role().await,
        "controller",
        "the reloaded tab controls again"
    );
    let mut last = *delivered.last().unwrap();
    let mut held: Vec<(u64, String)> = Vec::new();
    let mut pages = 0;
    let mut end = None;
    // Replay, page by page; live events wait until it is done, as the SDK holds them.
    loop {
        tab.request(100 + pages, "session/events", &json!({"since": last}))
            .await;
        let answer = loop {
            let m = tab.next().await.unwrap();
            if m["id"] == json!(100 + pages) {
                break m;
            }
            if method(&m).starts_with("event/") {
                held.push((m["params"]["seq"].as_u64().unwrap(), method(&m).to_owned()));
            }
        };
        pages += 1;
        let result = &answer["result"];
        assert_eq!(result["complete"], true);
        for e in result["events"].as_array().unwrap() {
            let seq = e["params"]["seq"].as_u64().unwrap();
            assert!(seq > last, "a page repeats nothing");
            if e["method"] == "event/turnEnded" {
                end = Some(seq);
            }
            delivered.push(seq);
            last = seq;
        }
        if result["more"] != true {
            break;
        }
    }
    assert!(pages > 1, "the replay came in pages");
    for (seq, m) in held {
        if seq > last {
            if m == "event/turnEnded" {
                end = Some(seq);
            }
            delivered.push(seq);
            last = seq;
        }
    }
    while end.is_none() {
        let m = tab.next().await.unwrap();
        if let Some(seq) = m["params"]["seq"].as_u64()
            && seq > last
        {
            if method(&m) == "event/turnEnded" {
                end = Some(seq);
            }
            delivered.push(seq);
            last = seq;
        }
    }
    let end = end.unwrap();
    assert_eq!(end, 302, "turnStarted, 300 deltas, turnEnded");
    assert_eq!(
        delivered,
        (1..=end).collect::<Vec<_>>(),
        "every event once, in order"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_waiting_ask_survives_a_reload_and_the_reloaded_tab_answers_it() {
    let r = rig("reload", Limits::default());
    let b = r.login().await;
    let sid = r.start(&b).await;
    let mut tab = r.tab(&b, &sid).await;
    tab.role().await;
    tab.request(1, "session/prompt", &json!({"text": "ask"}))
        .await;
    let first = tab.until(|m| method(m) == "gate/ask").await.pop().unwrap();
    assert_eq!(first["id"], "gate-1");
    assert_eq!(first["params"]["act"], "rm -rf build");
    let left = first["params"]["leftMs"].as_u64().unwrap();
    assert!(left > 590_000 && left <= 600_000, "{left}");
    drop(tab);
    tokio::time::sleep(Duration::from_millis(300)).await;

    r.board.pass(Duration::from_secs(30));
    let mut tab = r.tab(&b, &sid).await;
    let seen = tab.until(|m| method(m) == "gate/ask").await;
    assert!(seen.iter().any(|m| m["params"]["role"] == "controller"));
    let again = seen.last().unwrap();
    assert_eq!(again["id"], "gate-1");
    let left = again["params"]["leftMs"].as_u64().unwrap();
    assert!(
        left <= 570_000 && left > 560_000,
        "time left counts down: {left}"
    );
    tab.send(&json!({"jsonrpc": "2.0", "id": "gate-1", "result": {"allow": true}}))
        .await;
    let seen = tab.until(|m| note(m).is_some()).await;
    assert!(seen.iter().any(|m| method(m) == "board/askGone"));
    assert_eq!(
        note(seen.last().unwrap()),
        Some("answer \"gate-1\" allow=true reason=null")
    );
    // One answer per ask: a second is refused.
    tab.send(&json!({"jsonrpc": "2.0", "id": "gate-1", "result": {"allow": false}}))
        .await;
    let refused = tab
        .until(|m| method(m) == "board/refused")
        .await
        .pop()
        .unwrap();
    assert!(
        refused["params"]["why"]
            .as_str()
            .unwrap()
            .contains("no such ask")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn with_no_controller_past_the_grace_the_ask_is_denied_and_the_chat_ends() {
    let r = rig("grace", Limits::default());
    let owner = r.login().await;
    let other = r.login().await;
    let sid = r.start(&owner).await;
    let mut tab = r.tab(&owner, &sid).await;
    tab.role().await;
    tab.request(1, "session/prompt", &json!({"text": "ask"}))
        .await;
    tab.until(|m| method(m) == "gate/ask").await;
    // Another browser watches; the owner's tab is gone.
    let mut watcher = r.tab(&other, &sid).await;
    assert_eq!(watcher.role().await, "observer");
    drop(tab);
    let told = watcher.until(|m| method(m) == "board/role").await;
    assert_eq!(told.last().unwrap()["params"]["controlled"], false);

    // Within the grace, nothing happens.
    r.board.pass(Duration::from_secs(50));
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(r.live(&owner).await.len(), 1);

    r.board.pass(Duration::from_secs(11));
    let seen = watcher.until(|m| method(m) == "board/ended").await;
    let denied = seen
        .iter()
        .find_map(note)
        .expect("the child heard the denial");
    assert!(
        denied.starts_with("answer \"gate-1\" allow=false"),
        "{denied}"
    );
    assert!(denied.contains("no tab held the chat for 60 s"), "{denied}");
    assert!(watcher.next().await.is_none());
    assert_eq!(watcher.closed, Some(4001));
    assert_eq!(r.live(&owner).await, Vec::<Value>::new());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_observer_cannot_answer_the_gate_nor_act() {
    let r = rig("observer", Limits::default());
    let owner = r.login().await;
    let other = r.login().await;
    let sid = r.start(&owner).await;
    let mut tab = r.tab(&owner, &sid).await;
    assert_eq!(tab.role().await, "controller");
    let mut stranger = r.tab(&other, &sid).await;
    assert_eq!(stranger.role().await, "observer");
    tab.request(1, "session/prompt", &json!({"text": "ask"}))
        .await;
    let ask = stranger
        .until(|m| method(m) == "gate/ask")
        .await
        .pop()
        .unwrap();
    assert_eq!(ask["id"], "gate-1");

    stranger
        .send(&json!({"jsonrpc": "2.0", "id": "gate-1", "result": {"allow": true}}))
        .await;
    let refused = stranger
        .until(|m| method(m) == "board/refused")
        .await
        .pop()
        .unwrap();
    assert!(
        refused["params"]["why"]
            .as_str()
            .unwrap()
            .contains("observes")
    );
    for (id, m) in [
        (2, "session/prompt"),
        (3, "session/interrupt"),
        (4, "shutdown"),
        (5, "board/takeOver"),
    ] {
        stranger.request(id, m, &json!({"text": "x"})).await;
        let err = stranger.until(|v| v["id"] == id).await.pop().unwrap();
        let code = err["error"]["code"].as_i64().unwrap();
        assert!(code == -32011 || code == -32010, "{m}: {err}");
    }

    // The owner's second tab observes until it takes over; then the first observes.
    let mut second = r.tab(&owner, &sid).await;
    assert_eq!(second.role().await, "observer");
    second.request(1, "board/takeOver", &json!({})).await;
    second.until(|m| m["params"]["role"] == "controller").await;
    // The ask is sent again to the tab that now controls.
    second.until(|m| method(m) == "gate/ask").await;
    tab.until(|m| m["params"]["role"] == "observer").await;
    second
        .send(&json!({"jsonrpc": "2.0", "id": "gate-1", "result": {"allow": false, "reason": "not now"}}))
        .await;
    let heard = second.until(|m| note(m).is_some()).await;
    assert_eq!(
        note(heard.last().unwrap()),
        Some("answer \"gate-1\" allow=false reason=\"not now\"")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_upgrade_is_guarded() {
    let r = rig("guard", Limits::default());
    let b = r.login().await;
    let sid = r.start(&b).await;
    let status = |e: tungstenite::Error| match e {
        tungstenite::Error::Http(res) => res.status().as_u16(),
        other => panic!("{other}"),
    };
    let good = format!("mnemorph.v1, mnemorph.csrf.{}", b.csrf);
    let e = r
        .connect(&b, &sid, "http://evil.example", &good)
        .await
        .err()
        .unwrap();
    assert_eq!(status(e), 403, "a foreign Origin");
    let e = r
        .connect(&b, &sid, "http://127.0.0.1:1", &good)
        .await
        .err()
        .unwrap();
    assert_eq!(status(e), 403, "another port's Origin");
    let e = r
        .connect(&b, &sid, &r.origin(), "mnemorph.v1")
        .await
        .err()
        .unwrap();
    assert_eq!(status(e), 403, "no CSRF token");
    let e = r
        .connect(&b, &sid, &r.origin(), "mnemorph.v1, mnemorph.csrf.00")
        .await
        .err()
        .unwrap();
    assert_eq!(status(e), 403, "a wrong CSRF token");
    let e = r
        .connect(
            &Browser {
                cookie: "mnemorph_board=00".into(),
                csrf: b.csrf.clone(),
            },
            &sid,
            &r.origin(),
            &good,
        )
        .await
        .err()
        .unwrap();
    assert_eq!(status(e), 401, "no session");
    let e = r
        .connect(&b, &sid, &r.origin(), &format!("mnemorph.csrf.{}", b.csrf))
        .await
        .err()
        .unwrap();
    assert_eq!(status(e), 400, "not the chat's subprotocol");
    // An unknown chat is closed with a code that says so.
    let mut tab = r.tab(&b, "nope").await;
    assert!(tab.next().await.is_none());
    assert_eq!(tab.closed, Some(4404));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_child_gets_a_scrubbed_env_and_dies_with_the_chat_and_the_board() {
    let r = rig("child", Limits::default());
    let b = r.login().await;
    let sid = r.start(&b).await;
    let mut tab = r.tab(&b, &sid).await;
    tab.role().await;
    tab.request(1, "session/prompt", &json!({"text": "env"}))
        .await;
    let env = tab.until(|m| note(m).is_some()).await;
    let env: Value = serde_json::from_str(note(env.last().unwrap()).unwrap()).unwrap();
    let names: BTreeSet<&str> = env
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    // The named provider key, and nothing secret beside it.
    assert_eq!(env["FAKE_PROVIDER_KEY"], "sk-fake-provider");
    assert!(names.contains("PATH") && names.contains("LANG") && names.contains(FAKE));
    assert!(!names.contains("OTHER_API_TOKEN"), "{names:?}");
    assert!(!names.contains("DEPLOY_URL"), "{names:?}");
    // Allowed by name only: what a body could use to reach the human's accounts stays out.
    assert!(
        !names.contains("AWS_PROFILE") && !names.contains("SSH_AUTH_SOCK"),
        "{names:?}"
    );
    assert!(names.contains("MNEMORPH_BOARD_PID"), "{names:?}");

    tab.request(2, "session/prompt", &json!({"text": "spawn"}))
        .await;
    let pids = tab.until(|m| note(m).is_some()).await;
    let pids: Vec<u32> = note(pids.last().unwrap())
        .unwrap()
        .split(' ')
        .map(|p| p.parse().unwrap())
        .collect();
    assert!(!pids.iter().any(|&p| dead(p)));
    tab.request(3, "shutdown", &json!({})).await;
    let seen = tab.until(|m| method(m) == "board/ended").await;
    assert!(seen.iter().any(|m| m["id"] == 3));
    assert!(
        all_dead(&pids).await,
        "the child and what it started: {pids:?}"
    );
    assert_eq!(r.live(&b).await, Vec::<Value>::new());

    // The board stops: every chat's group goes with it.
    let sid = r.start(&b).await;
    let mut tab = r.tab(&b, &sid).await;
    tab.role().await;
    tab.request(1, "session/prompt", &json!({"text": "spawn"}))
        .await;
    let pids = tab.until(|m| note(m).is_some()).await;
    let pids: Vec<u32> = note(pids.last().unwrap())
        .unwrap()
        .split(' ')
        .map(|p| p.parse().unwrap())
        .collect();
    r.board.bridge().end_all("the board stopped").await;
    assert!(all_dead(&pids).await, "{pids:?}");
    let seen = tab.until(|m| method(m) == "board/ended").await;
    assert_eq!(seen.last().unwrap()["params"]["why"], "the board stopped");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hostile_text_passes_untouched_within_the_caps() {
    let r = rig(
        "hostile",
        Limits {
            line: 64 << 10,
            ..Limits::default()
        },
    );
    let b = r.login().await;
    let sid = r.start(&b).await;
    let mut tab = r.tab(&b, &sid).await;
    tab.role().await;
    tab.request(1, "session/prompt", &json!({"text": "hostile"}))
        .await;
    let mut raw = Vec::new();
    loop {
        let t = tab.text().await.unwrap();
        let done = t.contains("\"after\"");
        raw.push(t);
        if done {
            break;
        }
    }
    // Byte for byte as the child wrote it: rendering it inert is the page's job.
    let expected = json!({"jsonrpc": "2.0", "method": "event/note", "params": {
        "session": "fake-s", "run": "fake-r", "body": "rimuru", "seq": 2, "text": HOSTILE}})
    .to_string();
    assert!(raw.contains(&expected), "{raw:?}");
    // The line past the cap is dropped and said; the next one arrives.
    let over: Vec<Value> = raw
        .iter()
        .map(|t| serde_json::from_str(t).unwrap())
        .filter(|m: &Value| method(m) == "board/oversize")
        .collect();
    assert_eq!(over.len(), 1);
    assert!(over[0]["params"]["bytes"].as_u64().unwrap() > 200_000);
    assert!(raw.iter().all(|t| t.len() < 64 << 10));

    // A message past the inbound cap closes the tab, never reaches the child.
    tab.ws
        .send(Message::text(format!(
            r#"{{"jsonrpc":"2.0","id":9,"method":"session/prompt","params":{{"text":"{}"}}}}"#,
            "y".repeat(2 << 20)
        )))
        .await
        .ok();
    while tab.next().await.is_some() {}
    assert_eq!(r.live(&b).await.len(), 1, "the chat goes on");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_tab_is_dropped_and_chats_are_capped() {
    let r = rig(
        "slow",
        Limits {
            sessions: 1,
            queue_bytes: 1 << 20,
            ..Limits::default()
        },
    );
    let b = r.login().await;
    let sid = r.start(&b).await;
    let (status, why) = r.post_start(&b).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{why}");

    let mut tab = r.tab(&b, &sid).await;
    tab.role().await;
    // 12 MB of events to a tab that reads nothing for a while.
    tab.request(1, "session/prompt", &json!({"text": "big 200"}))
        .await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    while tab.text().await.is_some() {}
    assert_eq!(tab.closed, Some(4008), "dropped, not buffered");
    // It reconnects, and the chat is still there.
    let mut tab = r.tab(&b, &sid).await;
    assert_eq!(tab.role().await, "controller");
}

async fn tabs(r: &Rig, b: &Browser) -> u64 {
    r.live(b).await[0]["tabs"].as_u64().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stalled_tab_is_dropped_by_ping_and_by_send_timeout() {
    let r = rig(
        "stalled",
        Limits {
            send: Duration::from_millis(500),
            ping: Duration::from_millis(200),
            queue_bytes: 64 << 20,
            ..Limits::default()
        },
    );
    let b = r.login().await;
    let sid = r.start(&b).await;
    // An idle tab that reads nothing answers no ping: it is dropped.
    let idle = r.tab(&b, &sid).await;
    assert_eq!(tabs(&r, &b).await, 1);
    let mut gone = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if tabs(&r, &b).await == 0 {
            gone = true;
            break;
        }
    }
    assert!(gone, "the idle tab that answers no ping is dropped");
    drop(idle);

    // A tab that stops reading under a flood is dropped once a send stalls.
    let mut flood = r.tab(&b, &sid).await;
    flood.role().await;
    flood
        .request(1, "session/prompt", &json!({"text": "big 300"}))
        .await;
    let mut gone = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if tabs(&r, &b).await == 0 {
            gone = true;
            break;
        }
    }
    assert!(gone, "a stalled send drops the tab");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_crash_says_why_and_a_resume_names_only_a_run() {
    let r = rig("crash", Limits::default());
    let b = r.login().await;
    for bad in [
        r#"{"resume": ".."}"#,
        r#"{"resume": "."}"#,
        r#"{"resume": "../r1-2"}"#,
        r#"{"resume": "--world"}"#,
    ] {
        let (status, _) = r.post_body(&b, bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let sid = r.start(&b).await;
    let mut tab = r.tab(&b, &sid).await;
    tab.role().await;
    tab.request(1, "session/prompt", &json!({"text": "crash"}))
        .await;
    let ended = tab
        .until(|m| method(m) == "board/ended")
        .await
        .pop()
        .unwrap();
    let why = ended["params"]["why"].as_str().unwrap();
    assert!(why.starts_with("the chat's process ended"), "{why}");
    assert!(why.contains("the model's reply did not parse"), "{why}");
    assert!(!why.contains('\u{1b}'), "escaped: {why}");
}
