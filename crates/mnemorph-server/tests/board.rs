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
        "localhost.evil.com:7878",
        "foo.localhost:7878",
        "localhost:7879",
        "localhost",
        "127.0.0.2:7878",
        "0.0.0.0:7878",
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
content-security-policy: default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self' ws://127.0.0.1:7878 ws://localhost:7878; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; require-trusted-types-for 'script'
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
    // A refused page load is a page, under the same headers.
    let page = f
        .send(
            Request::get("/")
                .header(header::HOST, "evil.example")
                .header("sec-fetch-dest", "document")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(page.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        snapshot(&page),
        text("content-type: text/html; charset=utf-8")
    );
}

/// The text of `res`.
async fn text_of(res: Response<Body>) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn localhost_is_a_loopback_name_and_signs_in_on_its_own() {
    const LOCAL: &str = "localhost:7878";
    let f = Fixture::new("localhost");
    let at = |path: &str| Request::get(path).header(header::HOST, LOCAL);
    let res = f.send(at("/api/health").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::OK);
    // Its own origin passes; another origin of the board's, with this Host, does not.
    let (cookie, csrf) = f.login().await;
    let upgrade = |origin: &str| {
        at("/ws/board")
            .header(header::UPGRADE, "websocket")
            .header(header::CONNECTION, "upgrade")
            .header(header::COOKIE, &cookie)
            .header(header::ORIGIN, origin)
            .header(
                header::SEC_WEBSOCKET_PROTOCOL,
                format!("mnemorph.v1, mnemorph.csrf.{csrf}"),
            )
            .body(Body::empty())
            .unwrap()
    };
    // Past every refusal of the guard's: with no real socket under it, the upgrade is a 400.
    assert_eq!(
        f.send(upgrade("http://localhost:7878")).await.status(),
        StatusCode::BAD_REQUEST
    );
    for foreign in [ORIGIN, "http://evil.example", "http://localhost:7879"] {
        assert_eq!(
            f.send(upgrade(foreign)).await.status(),
            StatusCode::FORBIDDEN,
            "{foreign}"
        );
    }
    let mutation = Request::post("/session")
        .header(header::HOST, LOCAL)
        .header(header::ORIGIN, "http://localhost:7878")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "key": f.key() }).to_string()))
        .unwrap();
    assert_eq!(f.send(mutation).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_refused_page_load_says_why_and_what_to_do() {
    let f = Fixture::new("pages");
    let nav = |host: &str, path: &str| {
        Request::get(path)
            .header(header::HOST, host)
            .header("sec-fetch-dest", "document")
            .header(header::ACCEPT, "text/html")
            .body(Body::empty())
            .unwrap()
    };
    let res = f.send(nav("mnemorph.test:7878", "/")).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let page = text_of(res).await;
    assert!(
        page.contains("Open the board at <a href=\"http://127.0.0.1:7878/\">"),
        "{page}"
    );
    assert!(page.contains("to block DNS rebinding"), "{page}");
    for at in ["127.0.0.1:7878", "[::1]:7878", "localhost:7878"] {
        assert!(page.contains(&format!("http://{at}")), "{at}: {page}");
    }
    assert!(page.contains("<link rel=\"stylesheet\" href=\"/refused.css\">"));
    assert!(!page.contains("<script"), "no script, inline or not");

    let res = f.send(nav(HOST, "/api/worlds")).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let page = text_of(res).await;
    assert!(page.contains("This tab isn\u{2019}t signed in"), "{page}");
    assert!(page.contains("Each link works once."), "{page}");
    assert!(!page.contains(&f.key()), "the key never reaches a page");

    // A script's fetch is answered as before: plain text, the same status.
    let res = f
        .send(get("/api/worlds").body(Body::empty()).unwrap())
        .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );

    // The stylesheet is served to every Host, so the refused page reads; it holds no secret.
    for host in ["mnemorph.test:7878", HOST] {
        let res = f
            .send(
                Request::get("/refused.css")
                    .header(header::HOST, host)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK, "{host}");
        assert_eq!(
            res.headers()[header::CONTENT_TYPE],
            "text/css; charset=utf-8"
        );
        assert!(res.headers().contains_key(header::CONTENT_SECURITY_POLICY));
        assert!(!text_of(res).await.contains(&f.key()));
    }
    // Any other method on it is the guard's as ever.
    let res = f
        .send(
            Request::post("/refused.css")
                .header(header::HOST, "mnemorph.test:7878")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "one listing, followed through its changes"
)]
async fn the_worlds_are_the_registry_and_the_human_hand() {
    let f = Fixture::new("worlds");
    let world = |name: &str| {
        let w = f.root.join(name);
        std::fs::create_dir_all(w.join(".mnemorph")).unwrap();
        w
    };
    let (alpha, beta, gamma, gone) = (world("alpha"), world("beta"), world("gamma"), world("gone"));
    let bare = f.root.join("bare");
    std::fs::create_dir_all(&bare).unwrap();
    let registry = f.registry();
    let hands = hand_file(&registry);
    register_at(&registry, &alpha, Some("github.com/o/alpha".to_owned())).unwrap();
    register_at(&registry, &alpha, None).unwrap();
    register_at(&registry, &gone, None).unwrap();
    register_at(&registry, &gamma, None).unwrap();
    register_at(&registry, &bare, None).unwrap();
    std::fs::remove_dir_all(&gone).unwrap();
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
    assert_eq!(worlds.len(), 2, "{got}");
    // gamma was forgotten; beta, only added, goes last; the vanished world and the one with no
    // `.mnemorph/` are apart, as missing.
    assert_eq!(
        worlds[0],
        row(
            &alpha,
            json!("github.com/o/alpha"),
            json!("<at>"),
            2,
            "present"
        )
    );
    assert_eq!(
        worlds[1],
        row(&beta, json!(null), json!(null), 0, "present")
    );
    let missing = got["missing"].as_array().unwrap();
    let paths: Vec<&str> = missing
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths.len(), 2, "{got}");
    assert!(paths.contains(&gone.to_str().unwrap()) && paths.contains(&bare.to_str().unwrap()));
    for m in missing {
        assert!(
            mnemorph_server::world::ledger::Timestamp::parse(m["since"].as_str().unwrap())
                .is_some(),
            "{m}"
        );
    }

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
    assert_eq!(body(res).await["worlds"].as_array().unwrap().len(), 3);

    // The vanished world comes back: live again, and no longer missing.
    std::fs::create_dir_all(gone.join(".mnemorph")).unwrap();
    let res = f
        .send(
            get("/api/worlds")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let got = body(res).await;
    assert_eq!(got["worlds"].as_array().unwrap().len(), 4, "{got}");
    assert_eq!(got["missing"].as_array().unwrap().len(), 1, "{got}");
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

/// The key in a launcher page.
fn key_in(page: &str) -> String {
    let at = page.find("#k=").unwrap() + 3;
    page[at..at + 64].to_owned()
}

#[tokio::test]
async fn the_browser_is_opened_on_a_file_never_a_key_on_a_command_line() {
    let dir = tempfile::tempdir().unwrap();
    let (board, _) = living(Lifetimes::default());
    let printed = key_of(&board.url());
    let file = board.launch(dir.path()).unwrap();
    assert!(file.starts_with(dir.path()));
    let page = std::fs::read_to_string(&file).unwrap();
    let key = key_in(&page);
    assert_ne!(key, printed, "a key of its own");
    assert!(page.contains(&format!(
        "http-equiv=\"refresh\" content=\"0;url=http://{HOST}/#k={key}\""
    )));
    assert!(
        page.contains(&format!("<a href=\"http://{HOST}/#k={key}\">")),
        "a link too"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // The opener's whole command line: the program and the file, never the key.
    let cmd = mnemorph_server::board::opener(&file);
    let mut argv: Vec<String> = vec![cmd.get_program().to_string_lossy().into_owned()];
    argv.extend(cmd.get_args().map(|a| a.to_string_lossy().into_owned()));
    assert_eq!(argv.len(), 2, "{argv:?}");
    assert_eq!(argv[1], file.to_string_lossy());
    assert!(argv.iter().all(|a| !a.contains(&key)), "{argv:?}");

    // Spent, its file goes; a second launcher replaces the first.
    assert_eq!(trade(&board, &key).await, StatusCode::OK);
    assert!(!file.exists(), "spent: removed");
    let first = board.launch(dir.path()).unwrap();
    let second = board.launch(dir.path()).unwrap();
    assert!(!first.exists() && second.exists());
    // Dead past its minute, it goes too.
    board.pass(MIN);
    board.refresh().unwrap();
    assert!(!second.exists(), "dead: removed");
}

/// A pretend opener for [`Board::open_in`]: keeps the path it was handed, launches nothing.
fn keep(into: &Mutex<Vec<PathBuf>>) -> impl FnOnce(&Path) -> Result<(), String> + '_ {
    move |file| {
        into.lock().unwrap().push(file.to_path_buf());
        Ok(())
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_snap_browser_is_handed_a_launcher_in_its_own_common_directory() {
    use mnemorph_server::board::browser::Browser;
    use std::os::unix::fs::PermissionsExt as _;
    let home = tempfile::tempdir().unwrap();
    let state = home.path().join(".local/state/mnemorph");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    let common = home.path().join("snap/chromium/common");
    std::fs::create_dir_all(&common).unwrap();
    let (board, _) = living(Lifetimes::default());
    let snap = Browser::Snap("chromium".to_owned());
    let handed = Mutex::new(Vec::new());
    let file = board
        .open_in(&state, Some(home.path()), &snap, keep(&handed))
        .unwrap();
    assert_eq!(*handed.lock().unwrap(), vec![file.clone()]);
    assert_eq!(file.parent().unwrap(), common.join("mnemorph"));
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&common.join("mnemorph")), 0o700);
    let key = key_in(&std::fs::read_to_string(&file).unwrap());
    // Nothing of it in the state directory, and the opener saw only the path.
    assert!(std::fs::read_dir(&state).unwrap().next().is_none());
    assert!(!file.to_string_lossy().contains(&key));
    // Spent, it goes, as in the state directory.
    assert_eq!(trade(&board, &key).await, StatusCode::OK);
    assert!(!file.exists(), "spent: removed");
    // Dead past its minute, too.
    let file = board
        .open_in(&state, Some(home.path()), &snap, keep(&handed))
        .unwrap();
    board.pass(MIN);
    board.refresh().unwrap();
    assert!(!file.exists(), "dead: removed");

    // A plain browser: the state directory, as before.
    let file = board
        .open_in(&state, Some(home.path()), &Browser::Plain, keep(&handed))
        .unwrap();
    assert_eq!(file.parent().unwrap(), state);
    assert_eq!(mode(&file), 0o600);
}

#[cfg(unix)]
#[tokio::test]
async fn a_launcher_is_created_exclusively_and_never_through_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let (board, _) = living(Lifetimes::default());
    // A link in the directory's place is refused, and nothing is written through it.
    let link = dir.path().join("linked");
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(board.launch(&link).is_err());
    assert!(std::fs::read_dir(&real).unwrap().next().is_none());
}

#[tokio::test]
async fn an_opener_that_fails_or_no_known_place_is_an_error_and_leaves_no_launcher() {
    use mnemorph_server::board::browser::Browser;
    let home = tempfile::tempdir().unwrap();
    let state = home.path().join("state");
    std::fs::create_dir(&state).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let (board, _) = living(Lifetimes::default());
    let e = board
        .open_in(&state, Some(home.path()), &Browser::Plain, |_| {
            Err("xdg-open failed (exit status: 4)".to_owned())
        })
        .unwrap_err();
    assert!(e.contains("xdg-open failed"), "{e}");
    assert!(!e.contains("#k="), "{e}");
    assert!(
        std::fs::read_dir(&state).unwrap().next().is_none(),
        "removed"
    );
    // A snap with no directory of its own under the home: no place it reads is known.
    let handed = Mutex::new(Vec::new());
    let snap = Browser::Snap("chromium".to_owned());
    assert!(
        board
            .open_in(&state, Some(home.path()), &snap, keep(&handed))
            .is_err()
    );
    assert!(board.open_in(&state, None, &snap, keep(&handed)).is_err());
    assert!(handed.lock().unwrap().is_empty(), "nothing opened");
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

#[test]
fn a_global_file_that_does_not_read_is_said_without_its_secret() {
    let f = Fixture::new("metrics-config");
    let global = f.root.join("config.toml");
    let token = "ghp_FAKETOKEN16C7e42F292c6912E7710c838347Ae1";
    std::fs::write(&global, format!("[gate]\nask_rules = [\"{token}\"]\n")).unwrap();
    f.board.configure(global);
    let now = mnemorph_server::world::ledger::Timestamp::now();
    let m = f
        .board
        .metrics("all", mnemorph_server::world::spend::Window::All, now)
        .unwrap();
    let why = m.bench.why.clone().unwrap_or_default();
    assert!(m.bench.value.is_none());
    assert!(why.starts_with("your global file does not read"), "{why}");
    let all = serde_json::to_string(&m).unwrap();
    assert!(!all.contains(token), "{all}");
}
