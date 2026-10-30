//! The board's server, driven through its router without a socket: the guard refuses a foreign
//! Host or Origin, a cross-site fetch, a traversal, a missing cookie, a reused key and a missing
//! CSRF token; every response carries the same headers; `/api/worlds` lists the registry's worlds
//! and the human's hand.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use http_body_util::BodyExt as _;
use mnemorph_server::board::{Board, Lifetimes, listen, loopback};
use mnemorph_server::world::dimension::{Hand, hand, hand_file, register_at};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const HOST: &str = "127.0.0.1:7878";
const ORIGIN: &str = "http://127.0.0.1:7878";

struct Fixture {
    root: PathBuf,
    board: Arc<Board>,
    keys: Arc<Mutex<Vec<String>>>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-board-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let registry = root.join("state/mnemorph/worlds.jsonl");
        let keys = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&keys);
        let addrs: Vec<SocketAddr> = vec![HOST.parse().unwrap(), "[::1]:7878".parse().unwrap()];
        let board = Board::at(
            &addrs,
            registry.clone(),
            hand_file(&registry),
            Box::new(move |url| seen.lock().unwrap().push(url.to_owned())),
        )
        .unwrap();
        Self { root, board, keys }
    }

    fn registry(&self) -> PathBuf {
        self.root.join("state/mnemorph/worlds.jsonl")
    }

    fn key(&self) -> String {
        self.board.url().split_once("#k=").unwrap().1.to_owned()
    }

    async fn send(&self, req: Request<Body>) -> Response<Body> {
        self.board.router().oneshot(req).await.unwrap()
    }

    /// Trade the waiting key: the cookie and the CSRF token.
    async fn login(&self) -> (String, String) {
        let res = self.send(exchange(&self.key())).await;
        assert_eq!(res.status(), StatusCode::OK);
        let cookie = res.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .to_owned();
        let cookie = cookie.split(';').next().unwrap().to_owned();
        let csrf = body(res).await["csrf"].as_str().unwrap().to_owned();
        (cookie, csrf)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn get(path: &str) -> axum::http::request::Builder {
    Request::get(path).header(header::HOST, HOST)
}

fn exchange(key: &str) -> Request<Body> {
    Request::post("/session")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "key": key }).to_string()))
        .unwrap()
}

async fn body(res: Response<Body>) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn a_foreign_host_is_refused_on_every_route() {
    let f = Fixture::new("host");
    for host in [
        "evil.example:7878",
        "localhost:7878",
        "127.0.0.1:7879",
        "127.0.0.1",
        "127.1:7878",
        "[::ffff:127.0.0.1]:7878",
    ] {
        for path in ["/", "/api/health", "/api/worlds", "/nothing"] {
            let res = f
                .send(
                    Request::get(path)
                        .header(header::HOST, host)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await;
            assert_eq!(res.status(), StatusCode::FORBIDDEN, "{host} {path}");
        }
    }
    // No Host, two of them, and an absolute-form target naming another.
    let res = f.send(Request::get("/").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = f
        .send(
            get("/")
                .header(header::HOST, "evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = f
        .send(
            get("http://evil.example/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    // Both loopback literals the board is bound at pass.
    for host in [HOST, "[::1]:7878"] {
        let res = f
            .send(
                Request::get("/api/health")
                    .header(header::HOST, host)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK, "{host}");
    }
}

#[tokio::test]
async fn a_foreign_origin_or_site_is_refused() {
    let f = Fixture::new("origin");
    let key = f.key();
    for origin in [
        "http://evil.example",
        "http://127.0.0.1:3000",
        "http://localhost:7878",
        "https://127.0.0.1:7878",
        "null",
    ] {
        let mut req = exchange(&key);
        req.headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
        assert_eq!(
            f.send(req).await.status(),
            StatusCode::FORBIDDEN,
            "{origin}"
        );
        let res = f
            .send(
                get("/")
                    .header(header::ORIGIN, origin)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "GET with {origin}");
    }
    // A mutation without an Origin.
    let mut req = exchange(&key);
    req.headers_mut().remove(header::ORIGIN);
    assert_eq!(f.send(req).await.status(), StatusCode::FORBIDDEN);
    // A fetch from another site, or another port of this one (the same site).
    for site in ["cross-site", "same-site"] {
        let res = f
            .send(
                get("/")
                    .header("sec-fetch-site", site)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{site}");
    }
    // None of it spent the key.
    assert_eq!(f.key(), key);
    let res = f
        .send(
            get("/")
                .header("sec-fetch-site", "none")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_traversal_names_nothing() {
    let f = Fixture::new("traversal");
    for path in [
        "/../Cargo.toml",
        "/..%2f..%2fCargo.toml",
        "/%2e%2e/%2e%2e/Cargo.toml",
        "/assets/../index.html",
        "//etc/passwd",
        "/index.html/",
        "/.env",
        "/src/board/placeholder.html",
        "/build.rs",
    ] {
        let res = f.send(get(path).body(Body::empty()).unwrap()).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
    }
    for path in ["/", "/index.html"] {
        let res = f.send(get(path).body(Body::empty()).unwrap()).await;
        assert_eq!(res.status(), StatusCode::OK, "{path}");
        assert_eq!(
            res.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
    }
}

#[tokio::test]
async fn no_cookie_no_api() {
    let f = Fixture::new("cookie");
    for cookie in [
        None,
        Some("mnemorph_board=0000"),
        Some("other=1"),
        Some("mnemorph_board="),
    ] {
        for path in [
            "/api/worlds",
            "/api/session",
            "/api/anything",
            "/ws/session/x",
        ] {
            let mut req = get(path);
            if let Some(c) = cookie {
                req = req.header(header::COOKIE, c);
            }
            let res = f.send(req.body(Body::empty()).unwrap()).await;
            assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{path} {cookie:?}");
        }
    }
    // The key itself is no cookie.
    let key = f.key();
    let res = f
        .send(
            get("/api/worlds")
                .header(header::COOKIE, format!("mnemorph_board={key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_key_works_once() {
    let f = Fixture::new("key");
    let key = f.key();
    // A wrong key, or the right one in a form other than JSON, spends nothing.
    assert_eq!(
        f.send(exchange(&"0".repeat(64))).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let mut form = exchange(&key);
    form.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    assert_eq!(
        f.send(form).await.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert_eq!(f.key(), key);

    let res = f.send(exchange(&key)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let set = res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    let (pair, attrs) = set.split_once("; ").unwrap();
    assert_eq!(attrs, "HttpOnly; SameSite=Strict; Path=/");
    let csrf = body(res).await["csrf"].as_str().unwrap().to_owned();
    assert_eq!(csrf.len(), 64);
    assert!(!pair.contains(&key) && !pair.contains(&csrf));

    // The key died: reused, it is refused, and a new one waits, told to the URL keeper.
    assert_eq!(
        f.send(exchange(&key)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_ne!(f.key(), key);
    assert_eq!(f.keys.lock().unwrap().as_slice(), [f.board.url()]);

    // The cookie opens the API, and gives the CSRF token back to a reloaded page.
    let res = f
        .send(
            get("/api/session")
                .header(header::COOKIE, pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body(res).await, json!({ "csrf": csrf }));
}

#[tokio::test]
async fn a_mutation_or_upgrade_needs_the_csrf_token() {
    let f = Fixture::new("csrf");
    let (cookie, csrf) = f.login().await;
    let end = |token: Option<&str>| {
        let mut r = Request::post("/api/session/end")
            .header(header::HOST, HOST)
            .header(header::ORIGIN, ORIGIN)
            .header(header::COOKIE, &cookie);
        if let Some(t) = token {
            r = r.header("x-mnemorph-csrf", t);
        }
        r.body(Body::empty()).unwrap()
    };
    assert_eq!(f.send(end(None)).await.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        f.send(end(Some(&"0".repeat(64)))).await.status(),
        StatusCode::FORBIDDEN
    );
    // A mutation on an asset's path is held to the same.
    let res = f
        .send(
            Request::post("/index.html")
                .header(header::HOST, HOST)
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // An upgrade: the token as a subprotocol entry, an Origin required.
    let upgrade = |origin: Option<&str>, protocol: Option<&str>| {
        let mut r = get("/ws/board")
            .header(header::UPGRADE, "websocket")
            .header(header::CONNECTION, "upgrade")
            .header(header::COOKIE, &cookie);
        if let Some(o) = origin {
            r = r.header(header::ORIGIN, o);
        }
        if let Some(p) = protocol {
            r = r.header(header::SEC_WEBSOCKET_PROTOCOL, p);
        }
        r.body(Body::empty()).unwrap()
    };
    let token = format!("mnemorph.v1, mnemorph.csrf.{csrf}");
    assert_eq!(
        f.send(upgrade(None, Some(&token))).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.send(upgrade(Some(ORIGIN), None)).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.send(upgrade(Some(ORIGIN), Some("mnemorph.csrf.0")))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    // Admitted by the guard, the route answers: with no real socket under it, the upgrade
    // itself cannot be made (400), which is past every refusal of the guard's.
    assert_eq!(
        f.send(upgrade(Some(ORIGIN), Some(&token))).await.status(),
        StatusCode::BAD_REQUEST
    );

    // With the token, the session ends and its cookie opens nothing more.
    let res = f.send(end(Some(&csrf))).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(
        res.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let res = f
        .send(
            get("/api/worlds")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// Every header but the date and the length, sorted, as text.
fn snapshot(res: &Response<Body>) -> String {
    let mut out: Vec<String> = res
        .headers()
        .iter()
        .filter(|(k, _)| *k != header::CONTENT_LENGTH && *k != header::DATE)
        .map(|(k, v)| format!("{k}: {}", v.to_str().unwrap()))
        .collect();
    out.sort();
    out.join("\n")
}

#[tokio::test]
async fn every_response_carries_the_same_headers() {
    let f = Fixture::new("headers");
    let common = "cache-control: no-store
content-security-policy: default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self' ws://127.0.0.1:7878; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; require-trusted-types-for 'script'
cross-origin-opener-policy: same-origin
cross-origin-resource-policy: same-origin
referrer-policy: no-referrer
x-content-type-options: nosniff";
    let text = |extra: &str| {
        let mut v: Vec<&str> = common.lines().chain(extra.lines()).collect();
        v.sort_unstable();
        v.join("\n")
    };
    let page = f.send(get("/").body(Body::empty()).unwrap()).await;
    assert_eq!(
        snapshot(&page),
        text("content-type: text/html; charset=utf-8")
    );
    let missing = f.send(get("/nothing").body(Body::empty()).unwrap()).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        snapshot(&missing),
        text("content-type: text/plain; charset=utf-8")
    );
    let foreign = f
        .send(
            Request::get("/")
                .header(header::HOST, "evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        snapshot(&foreign),
        text("content-type: text/plain; charset=utf-8")
    );
    let health = f
        .send(get("/api/health").body(Body::empty()).unwrap())
        .await;
    assert_eq!(snapshot(&health), text("content-type: application/json"));
}

#[tokio::test]
async fn the_worlds_are_the_registry_and_the_human_hand() {
    let f = Fixture::new("worlds");
    let world = |name: &str| {
        let w = f.root.join(name);
        std::fs::create_dir_all(&w).unwrap();
        w
    };
    let (alpha, beta, gamma, gone) = (world("alpha"), world("beta"), world("gamma"), world("gone"));
    let registry = f.registry();
    let hands = hand_file(&registry);
    register_at(&registry, &alpha, Some("github.com/o/alpha".to_owned())).unwrap();
    register_at(&registry, &alpha, None).unwrap();
    register_at(&registry, &gone, None).unwrap();
    register_at(&registry, &gamma, None).unwrap();
    std::fs::remove_dir(&gone).unwrap();
    let add = |w: &Path, act| hand(&hands, &registry, act, w);
    add(&beta, Hand::Add).unwrap();
    add(&gamma, Hand::Forget).unwrap();
    assert!(add(&f.root.join("never"), Hand::Forget).is_err());
    assert!(add(&f.root.join("never"), Hand::Add).is_err());

    let (cookie, _) = f.login().await;
    let res = f
        .send(
            get("/api/worlds")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let mut got = body(res).await;
    for w in got["worlds"].as_array_mut().unwrap() {
        if !w["last"].is_null() {
            w["last"] = json!("<at>");
        }
    }
    let row = |p: &Path, remote: Value, last: Value, sessions: u64, state: &str| {
        json!({
            "id": mnemorph_server::world::dimension::world_id(p),
            "name": p.file_name().unwrap().to_str().unwrap(),
            "path": p.to_str().unwrap(),
            "remote": remote,
            "last": last,
            "sessions": sessions,
            "state": state,
        })
    };
    let worlds = got["worlds"].as_array().unwrap();
    assert_eq!(worlds.len(), 3, "{got}");
    // gamma was forgotten; the vanished world shows missing; beta, only added, goes last.
    assert!(worlds.contains(&row(
        &alpha,
        json!("github.com/o/alpha"),
        json!("<at>"),
        2,
        "present"
    )));
    assert!(worlds.contains(&row(&gone, json!(null), json!("<at>"), 1, "missing")));
    assert_eq!(
        worlds[2],
        row(&beta, json!(null), json!(null), 0, "present")
    );

    // Added again, gamma is back.
    add(&gamma, Hand::Add).unwrap();
    let res = f
        .send(
            get("/api/worlds")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(body(res).await["worlds"].as_array().unwrap().len(), 4);
}

#[test]
fn only_loopback_is_bound() {
    for ip in [
        "0.0.0.0",
        "::",
        "192.168.1.10",
        "10.0.0.1",
        "127.0.0.2",
        "::ffff:127.0.0.1",
    ] {
        let ip = ip.parse().unwrap();
        assert!(loopback(ip).is_err(), "{ip}");
        assert!(listen(0, Some(ip)).is_err(), "{ip}");
        let addr = SocketAddr::new(ip, 7878);
        assert!(
            Board::at(&[addr], PathBuf::new(), PathBuf::new(), Box::new(|_| {})).is_err(),
            "{ip}"
        );
    }
    let l = listen(0, Some("127.0.0.1".parse().unwrap())).unwrap();
    assert_eq!(l.len(), 1);
    assert!(l[0].local_addr().unwrap().ip().is_loopback());
}

/// A board whose secrets live `lifetimes`, and the URLs told to its keeper.
const MIN: std::time::Duration = std::time::Duration::from_mins(1);

fn living(lifetimes: Lifetimes) -> (Arc<Board>, Arc<Mutex<Vec<String>>>) {
    let told = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&told);
    let board = Board::living(
        &[HOST.parse().unwrap()],
        PathBuf::from("/nonexistent/worlds.jsonl"),
        PathBuf::from("/nonexistent/board/worlds.jsonl"),
        Box::new(move |url| seen.lock().unwrap().push(url.to_owned())),
        lifetimes,
    )
    .unwrap();
    (board, told)
}

fn key_of(url: &str) -> String {
    url.split_once("#k=").unwrap().1.to_owned()
}

async fn trade(board: &Arc<Board>, key: &str) -> StatusCode {
    board
        .router()
        .oneshot(exchange(key))
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn an_untraded_key_dies_and_a_fresh_one_is_kept() {
    let (board, told) = living(Lifetimes::default());
    let first = key_of(&board.url());
    board.pass(MIN * 9);
    board.refresh().unwrap();
    assert_eq!(key_of(&board.url()), first);
    assert!(told.lock().unwrap().is_empty());
    board.pass(MIN);
    board.refresh().unwrap();
    let now = key_of(&board.url());
    assert_ne!(now, first);
    assert_eq!(told.lock().unwrap().as_slice(), [board.url()]);
    assert_eq!(trade(&board, &first).await, StatusCode::UNAUTHORIZED);
    // Expired between ticks, it is refused all the same.
    board.pass(MIN * 10);
    assert_eq!(trade(&board, &now).await, StatusCode::UNAUTHORIZED);
    assert_eq!(trade(&board, &key_of(&board.url())).await, StatusCode::OK);
}

#[tokio::test]
async fn the_opened_key_works_once_and_briefly() {
    let (board, _) = living(Lifetimes::default());
    let printed = key_of(&board.url());
    let opened = key_of(&board.open_url().unwrap());
    assert_ne!(opened, printed);
    assert_eq!(trade(&board, &opened).await, StatusCode::OK);
    assert_eq!(trade(&board, &opened).await, StatusCode::UNAUTHORIZED);
    let late = key_of(&board.open_url().unwrap());
    board.pass(MIN);
    assert_eq!(trade(&board, &late).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_session_ends_idle_or_old() {
    let worlds = |board: &Arc<Board>, cookie: &str| {
        board.router().oneshot(
            get("/api/worlds")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
    };
    let login = |board: Arc<Board>| async move {
        let res = board
            .router()
            .oneshot(exchange(&key_of(&board.url())))
            .await
            .unwrap();
        let set = res.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .to_owned();
        set.split(';').next().unwrap().to_owned()
    };
    let hour = MIN * 60;
    // Idle: used within 12 hours, it lives; left 12 hours, it ends.
    let (board, _) = living(Lifetimes::default());
    let cookie = login(Arc::clone(&board)).await;
    for _ in 0..3 {
        board.pass(hour * 11);
        assert_eq!(
            worlds(&board, &cookie).await.unwrap().status(),
            StatusCode::OK
        );
    }
    board.pass(hour * 12);
    assert_eq!(
        worlds(&board, &cookie).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    // Old: used all along, it ends 7 days after it began.
    let (board, _) = living(Lifetimes::default());
    let cookie = login(Arc::clone(&board)).await;
    for _ in 0..15 {
        board.pass(hour * 11);
        assert_eq!(
            worlds(&board, &cookie).await.unwrap().status(),
            StatusCode::OK
        );
    }
    board.pass(hour * 3);
    assert_eq!(
        worlds(&board, &cookie).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}
