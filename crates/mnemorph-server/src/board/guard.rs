//! The guard every request passes before a route sees it, and the headers every response
//! carries. The policy is by method and path, so a route added later is held to it unnamed:
//!
//! - every request: a `Host` that is a bound loopback address, literally (`127.0.0.1:<port>`,
//!   `[::1]:<port>`), or `localhost:<port>`, which a browser resolves to loopback itself — a
//!   rebound DNS name never is; no cross-site or same-site fetch
//!   (`Sec-Fetch-Site`; another port of `127.0.0.1` is the same *site*); an `Origin`, when sent,
//!   that is `http://` + that `Host`;
//! - a mutation (any method but `GET` and `HEAD`) and a WebSocket upgrade: an `Origin` is
//!   required;
//! - `/api/…` but `/api/health`, `/ws`, `/ws/…`, every mutation and every upgrade: the session
//!   cookie — but `POST /session`, the key exchange, which is how a cookie is had;
//! - every mutation and upgrade but the key exchange: the session's CSRF token, in the
//!   `x-mnemorph-csrf` header, or for an upgrade as a `Sec-WebSocket-Protocol` entry
//!   `mnemorph.csrf.<token>`.
//!
//! A refused page load (a navigation, not a fetch) is answered with the same status as a page that
//! says why and what to do, styled by `/refused.css` — the one path served before the guard, a
//! fixed public stylesheet, so a page refused for its `Host` is readable too.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};

use super::{Board, COOKIE, CSRF_HEADER, CSRF_PROTOCOL, refuse, same};

/// Why a request is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The `Host` is not a bound loopback address, literally, nor `localhost`.
    Host,
    /// The fetch came from another site, or another origin on this one.
    Site,
    /// The `Origin` is missing where required, or is not this one.
    Origin,
    /// No valid session cookie.
    Session,
    /// The CSRF token is missing or wrong.
    Csrf,
}

impl Refusal {
    fn status(self) -> StatusCode {
        match self {
            Refusal::Session => StatusCode::UNAUTHORIZED,
            Refusal::Host | Refusal::Site | Refusal::Origin | Refusal::Csrf => {
                StatusCode::FORBIDDEN
            }
        }
    }

    fn why(self) -> &'static str {
        match self {
            Refusal::Host => "refused: the board answers only at its loopback address",
            Refusal::Site => "refused: a request from another site",
            Refusal::Origin => "refused: a missing or foreign Origin",
            Refusal::Session => "refused: no session; open the URL `mnemorphd board` prints",
            Refusal::Csrf => "refused: a missing or wrong CSRF token",
        }
    }
}

/// The values of every `mnemorph_board` cookie the request carries.
pub fn cookie(headers: &HeaderMap) -> impl Iterator<Item = &str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value)
}

/// The CSRF token a request carries: the header, or for an upgrade the subprotocol entry.
fn csrf(headers: &HeaderMap, upgrade: bool) -> Option<&str> {
    if let Some(v) = headers.get(CSRF_HEADER).and_then(|v| v.to_str().ok()) {
        return Some(v);
    }
    if !upgrade {
        return None;
    }
    headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .find_map(|p| p.trim().strip_prefix(CSRF_PROTOCOL))
}

/// The one value of `name`, or `None` when absent; `Err` when it is sent twice or is not text.
fn single<'h>(headers: &'h HeaderMap, name: &header::HeaderName) -> Result<Option<&'h str>, ()> {
    let mut all = headers.get_all(name).iter();
    let Some(first) = all.next() else {
        return Ok(None);
    };
    if all.next().is_some() {
        return Err(());
    }
    first.to_str().map(Some).map_err(|_| ())
}

/// Whether `board` admits a request with `method`, `path`, `authority` (an absolute-form
/// target's) and `headers`.
///
/// # Errors
///
/// The first [`Refusal`] that applies.
pub fn admit(
    board: &Board,
    method: &Method,
    path: &str,
    authority: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), Refusal> {
    // DNS rebinding: a name never passes, only the literal address the board is bound at, and
    // `localhost`, which browsers resolve to loopback without asking DNS.
    let host = single(headers, &header::HOST)
        .ok()
        .flatten()
        .filter(|h| board.hosts.iter().any(|ok| ok == h))
        .ok_or(Refusal::Host)?;
    if authority.is_some_and(|a| a != host) {
        return Err(Refusal::Host);
    }
    match single(headers, &header::HeaderName::from_static("sec-fetch-site")) {
        Ok(None | Some("same-origin" | "none")) => {}
        _ => return Err(Refusal::Site),
    }
    let mutation = method != Method::GET && method != Method::HEAD;
    let upgrade = headers.contains_key(header::UPGRADE);
    match single(headers, &header::ORIGIN) {
        Ok(Some(origin)) if origin.strip_prefix("http://") == Some(host) => {}
        Ok(None) if !mutation && !upgrade => {}
        _ => return Err(Refusal::Origin),
    }
    let exchange = path == "/session" && method == Method::POST && !upgrade;
    if exchange {
        return Ok(());
    }
    let open = path == "/api/health"
        || !(path.starts_with("/api/") || path == "/ws" || path.starts_with("/ws/"));
    if open && !mutation && !upgrade {
        return Ok(());
    }
    let token = cookie(headers)
        .find_map(|c| board.csrf_of(c))
        .ok_or(Refusal::Session)?;
    if mutation || upgrade {
        let sent = csrf(headers, upgrade).ok_or(Refusal::Csrf)?;
        if !same(&token, sent) {
            return Err(Refusal::Csrf);
        }
    }
    Ok(())
}

/// Set the hardening headers of every response.
pub fn harden(board: &Board, headers: &mut HeaderMap) {
    headers.insert(header::CONTENT_SECURITY_POLICY, board.csp.clone());
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
}

/// The stylesheet of the refusal pages: fixed and public, so it is served before the guard.
pub const REFUSED_CSS: &str = "/refused.css";

/// Whether `headers` are a browser loading a page (a navigation), not a script's fetch.
fn navigation(method: &Method, headers: &HeaderMap) -> bool {
    if method != Method::GET && method != Method::HEAD {
        return false;
    }
    match headers.get("sec-fetch-dest").and_then(|v| v.to_str().ok()) {
        Some(dest) => dest == "document",
        None => headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|a| a.contains("text/html")),
    }
}

/// The page a refused navigation is answered with: what happened and what to do.
fn page(board: &Board, r: Refusal) -> Response {
    let base = board.base();
    let (title, body) = match r {
        Refusal::Host => {
            let at: Vec<String> = board
                .hosts()
                .iter()
                .map(|h| format!("<li><a href=\"http://{h}/\">http://{h}</a></li>"))
                .collect();
            (
                "This address is refused",
                format!(
                    "<p>Open the board at <a href=\"{base}/\">{base}</a>: other host names are \
                     refused, to block DNS rebinding.</p><p>It answers at:</p><ul>{}</ul>\
                     <p>Each address signs in on its own: open a link <code>mnemorphd board</code> \
                     prints for it.</p>",
                    at.join("")
                ),
            )
        }
        Refusal::Session => (
            "This tab isn\u{2019}t signed in",
            "<p>Run <code>mnemorphd board</code> and open the link it prints, in this browser. \
             Each link works once.</p>"
                .to_owned(),
        ),
        Refusal::Site | Refusal::Origin | Refusal::Csrf => (
            "This request is refused",
            format!(
                "<p>{}.</p><p>Open the board at <a href=\"{base}/\">{base}</a> in this \
                 browser, from a link <code>mnemorphd board</code> prints.</p>",
                r.why()
            ),
        ),
    };
    let html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Mnemorph board \u{2014} {title}</title>\
         <link rel=\"stylesheet\" href=\"{REFUSED_CSS}\"></head>\
         <body><main class=\"refused\"><p class=\"mark\">Mnemorph board</p>\
         <h1>{title}</h1>{body}</main></body></html>\n"
    );
    (
        r.status(),
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

/// The middleware: [`admit`], then the route, then [`harden`] — a refusal hardened too, and a
/// refused page load answered with a page that says why.
pub(crate) async fn guard(State(board): State<Arc<Board>>, req: Request, next: Next) -> Response {
    if req.uri().path() == REFUSED_CSS
        && (req.method() == Method::GET || req.method() == Method::HEAD)
    {
        let mut res = (
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            include_str!("refused.css"),
        )
            .into_response();
        harden(&board, res.headers_mut());
        return res;
    }
    let verdict = admit(
        &board,
        req.method(),
        req.uri().path(),
        req.uri()
            .authority()
            .map(axum::http::uri::Authority::as_str),
        req.headers(),
    );
    let mut res = match verdict {
        Ok(()) => next.run(req).await,
        Err(r) if navigation(req.method(), req.headers()) => page(&board, r),
        Err(r) => refuse(r.status(), r.why()),
    };
    harden(&board, res.headers_mut());
    res
}
