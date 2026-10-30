//! `GET /api/w/<id>` through the router: behind the guard like every API route, a listed world's
//! overview — live, TUI-live, idle or missing, each field with its source, unknown said, never
//! guessed — and 404 for an id no listed world has. `WS /ws/board` is behind the guard too.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use http_body_util::BodyExt as _;
use mnemorph_server::board::{Board, Chats, overview::overview};
use mnemorph_server::world::dimension::{hand_file, register_at, world_id};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const HOST: &str = "127.0.0.1:7878";
const ORIGIN: &str = "http://127.0.0.1:7878";

struct Fixture {
    root: PathBuf,
    board: Arc<Board>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-overview-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let registry = root.join("state/mnemorph/worlds.jsonl");
        let addrs: Vec<SocketAddr> = vec![HOST.parse().unwrap()];
        let board = Board::at(
            &addrs,
            registry.clone(),
            hand_file(&registry),
            Box::new(|_| {}),
        )
        .unwrap();
        Self { root, board }
    }

    /// A world directory under the fixture, opened once in the registry.
    fn world(&self, name: &str) -> PathBuf {
        let w = self.root.join(name);
        std::fs::create_dir_all(&w).unwrap();
        register_at(&self.root.join("state/mnemorph/worlds.jsonl"), &w, None).unwrap();
        w
    }

    async fn send(&self, req: Request<Body>) -> Response<Body> {
        self.board.router().oneshot(req).await.unwrap()
    }

    async fn login(&self) -> (String, String) {
        let key = self.board.url().split_once("#k=").unwrap().1.to_owned();
        let res = self
            .send(
                Request::post("/session")
                    .header(header::HOST, HOST)
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({ "key": key }).to_string()))
                    .unwrap(),
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap();
        let cookie = cookie.split(';').next().unwrap().to_owned();
        let csrf = body(res).await["csrf"].as_str().unwrap().to_owned();
        (cookie, csrf)
    }

    async fn overview(&self, cookie: &str, world: &Path) -> Value {
        let res = self
            .send(get(&format!("/api/w/{}", world_id(world)), Some(cookie)))
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        body(res).await
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn get(path: &str, cookie: Option<&str>) -> Request<Body> {
    let mut r = Request::get(path).header(header::HOST, HOST);
    if let Some(c) = cookie {
        r = r.header(header::COOKIE, c);
    }
    r.body(Body::empty()).unwrap()
}

async fn body(res: Response<Body>) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn the_overview_and_the_push_channel_are_behind_the_guard() {
    let f = Fixture::new("guard");
    let w = f.world("alpha");
    let path = format!("/api/w/{}", world_id(&w));
    for cookie in [None, Some("mnemorph_board=0000")] {
        for p in [path.as_str(), "/api/w/nothing", "/ws/board"] {
            let res = f.send(get(p, cookie)).await;
            assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{p} {cookie:?}");
        }
    }
    // A foreign Host never reaches it.
    let res = f
        .send(
            Request::get(&path)
                .header(header::HOST, "evil.example:7878")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let (cookie, csrf) = f.login().await;
    let upgrade = |origin: Option<&str>, protocols: &str| {
        let mut r = Request::get("/ws/board")
            .header(header::HOST, HOST)
            .header(header::UPGRADE, "websocket")
            .header(header::CONNECTION, "upgrade")
            .header(header::COOKIE, &cookie)
            .header(header::SEC_WEBSOCKET_PROTOCOL, protocols);
        if let Some(o) = origin {
            r = r.header(header::ORIGIN, o);
        }
        r.body(Body::empty()).unwrap()
    };
    let good = format!("mnemorph.board.v1, mnemorph.csrf.{csrf}");
    // No Origin, a foreign one, no token, a wrong token: refused by the guard.
    for (origin, protocols, want) in [
        (None, good.as_str(), StatusCode::FORBIDDEN),
        (
            Some("http://evil.example"),
            good.as_str(),
            StatusCode::FORBIDDEN,
        ),
        (Some(ORIGIN), "mnemorph.board.v1", StatusCode::FORBIDDEN),
        (
            Some(ORIGIN),
            "mnemorph.board.v1, mnemorph.csrf.00",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let res = f.send(upgrade(origin, protocols)).await;
        assert_eq!(res.status(), want, "{origin:?} {protocols}");
    }
    // Admitted, the socket asks for its subprotocol, or for a real connection to upgrade.
    let res = f
        .send(upgrade(Some(ORIGIN), &format!("mnemorph.csrf.{csrf}")))
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let res = f.send(upgrade(Some(ORIGIN), &good)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // An id no listed world has.
    let res = f.send(get("/api/w/nothing", Some(&cookie))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_silent_world_says_what_it_does_not_know() {
    let f = Fixture::new("silent");
    let w = f.world("quiet");
    let (cookie, _) = f.login().await;
    let o = f.overview(&cookie, &w).await;
    assert_eq!(o["world"]["id"], world_id(&w));
    assert_eq!(o["status"]["value"], "idle");
    assert_eq!(o["status"]["trusted"], false);
    // The only thing that happened: a session opened, as the registry says.
    assert_eq!(o["activity"]["value"]["what"], "a session opened");
    assert_eq!(o["activity"]["source"], "the worlds registry");
    assert_eq!(o["activity"]["trusted"], true);
    assert_eq!(
        o["memory"]["value"],
        json!({"shared": 0, "grounded": 0, "proposed": 0, "retired": 0, "unproven": 0})
    );
    // No call booked: unknown, never a zero spend.
    assert_eq!(o["spend"]["value"], Value::Null);
    assert_eq!(o["spend"]["why"], "no call is booked yet");
    assert_eq!(o["meters"]["value"], json!([]));
    assert_eq!(o["waiting"]["value"], 0);
    for field in [
        "status",
        "activity",
        "memory",
        "challenges",
        "gate",
        "meters",
        "spend",
        "waiting",
    ] {
        assert!(
            o[field]["source"].as_str().is_some_and(|s| !s.is_empty()),
            "{field}"
        );
        assert!(o[field]["trusted"].is_boolean(), "{field}");
        if o[field]["value"].is_null() {
            assert!(o[field]["why"].is_string(), "{field}: unknown says why");
        }
    }
}

#[tokio::test]
async fn a_held_run_reads_tui_live_and_a_vanished_directory_missing() {
    let f = Fixture::new("live");
    let live = f.world("busy");
    let gone = f.world("gone");
    std::fs::create_dir_all(gone.join(".mnemorph")).unwrap();
    std::fs::remove_dir_all(&gone).unwrap();
    let (cookie, _) = f.login().await;

    let hold = mnemorph_loop::resume::hold(&live, "r1730000000000-41").unwrap();
    std::fs::create_dir_all(live.join(".mnemorph")).unwrap();
    std::fs::write(live.join(".mnemorph/log.md"), "# log\n").unwrap();
    let o = f.overview(&cookie, &live).await;
    assert_eq!(o["status"]["value"], "tui-live");
    assert_eq!(
        o["status"]["why"],
        "run r1730000000000-41 is held by a live session"
    );
    assert_eq!(o["status"]["trusted"], false, "the lock is the world's");
    drop(hold);
    let o = f.overview(&cookie, &live).await;
    assert_eq!(o["status"]["value"], "idle");

    // Gone, it is shown, never dropped: what lived in it is unknown.
    let res = f.send(get("/api/worlds", Some(&cookie))).await;
    let listed = body(res).await;
    assert!(
        listed["worlds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["id"] == world_id(&gone) && w["state"] == "missing")
    );
    let o = f.overview(&cookie, &gone).await;
    assert_eq!(o["status"]["value"], "missing");
    assert_eq!(o["status"]["trusted"], true);
    for field in ["memory", "meters", "spend"] {
        assert_eq!(o[field]["value"], Value::Null, "{field}");
        assert_eq!(o[field]["why"], "the directory is gone", "{field}");
    }
}

#[test]
fn a_board_chat_reads_live_and_its_asks_wait() {
    let root = std::env::temp_dir().join(format!("mnemorph-overview-chat-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let world = mnemorph_server::board::BoardWorld {
        id: "w-1".into(),
        name: "w".into(),
        path: root.to_string_lossy().into_owned(),
        remote: None,
        last: None,
        sessions: 1,
        state: mnemorph_server::board::WorldState::Present,
    };
    let o = overview(world, Chats { open: 1, asks: 2 });
    let v = serde_json::to_value(&o).unwrap();
    assert_eq!(v["status"]["value"], "live");
    assert_eq!(v["waiting"]["value"], 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_hostile_name_is_carried_as_text() {
    let f = Fixture::new("hostile");
    let w = f.world("<img src=x onerror=alert(1)>\u{202e}gnp.exe");
    let (cookie, _) = f.login().await;
    let res = f
        .send(get(
            &format!("/api/w/{}", urlencode(&world_id(&w))),
            Some(&cookie),
        ))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "application/json",
        "JSON, never HTML"
    );
    let o = body(res).await;
    assert_eq!(
        o["world"]["name"],
        "<img src=x onerror=alert(1)>\u{202e}gnp.exe"
    );
}

/// `s` percent-encoded but for the unreserved characters.
fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
