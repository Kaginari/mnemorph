//! The routes over a world's knowledge: its memory ([`super::memory`]) and its ontology
//! ([`super::facts`]), read; and the human's acts on them — confirm, wake, settle — as POSTs.
//!
//! An act is the human's: the guard has checked the loopback Host, the Origin, the cookie and the
//! CSRF token before a handler runs. A board started inside a body's command (`MNEMORPH_BODY` set)
//! refuses every act, as the CLI does. The id is checked to be one, the world to be listed and
//! present; what the page showed is checked again on a fresh copy; the files the act writes are
//! checked to be no links; then the CLI's own function does it, journaled as the human's from the
//! board.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Board, WorldState, facts, memory, refuse};

/// The largest body an act reads.
pub const ACT_BYTES: usize = 4096;

/// What an act answers: what was done, in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BoardDone {
    /// What was done, as the CLI says it.
    pub done: String,
}

/// Whether `id` may name a note or a fact: 1 to 128 ASCII letters, digits, `.`, `_`, `:` or `-`.
#[must_use]
pub fn is_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

/// `Ok` when no step from `world` down to each of `files` is a link, and each that exists is a
/// directory on the way and a regular file at the end: an act appends to what the world holds,
/// never through a link a body planted.
///
/// # Errors
///
/// The first step that is a link, or anything but a directory or a regular file.
pub fn no_links(world: &Path, files: &[String]) -> Result<(), String> {
    for rel in files {
        let parts: Vec<Component> = Path::new(rel).components().collect();
        let mut at = PathBuf::from(world);
        for (i, c) in parts.iter().enumerate() {
            let Component::Normal(name) = c else {
                return Err(format!("{rel}: not a path inside the world"));
            };
            at.push(name);
            let last = i + 1 == parts.len();
            match std::fs::symlink_metadata(&at) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(format!(
                        "{} is a link: the board writes through no link",
                        at.strip_prefix(world).unwrap_or(&at).display()
                    ));
                }
                Ok(m) if (last && !m.is_file()) || (!last && !m.is_dir()) => {
                    return Err(format!(
                        "{} is not a plain {}",
                        at.strip_prefix(world).unwrap_or(&at).display(),
                        if last { "file" } else { "directory" }
                    ));
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => return Err(format!("{rel}: {e}")),
            }
        }
    }
    Ok(())
}

/// A refusal: its status and why.
type No = (u16, String);

/// The listed, present world `id`'s directory, or the refusal.
fn present(board: &Board, id: &str) -> Result<PathBuf, No> {
    let Some(w) = board.worlds().worlds.into_iter().find(|w| w.id == id) else {
        return Err((404, "no listed world has this id".to_owned()));
    };
    if w.state == WorldState::Missing {
        return Err((409, "the world's directory is gone".to_owned()));
    }
    Ok(PathBuf::from(w.path))
}

/// The refusal of an act inside a body's command: the board's acts are the human's.
fn in_a_body() -> Option<Response> {
    std::env::var("MNEMORPH_BODY").ok().map(|body| {
        refuse(
            StatusCode::FORBIDDEN,
            &format!(
                "{body} runs this board: confirming, waking and settling are the human's acts, \
                 never a body's"
            ),
        )
    })
}

/// A refusal from `(status, why)`.
fn refused((status, why): No) -> Response {
    refuse(
        StatusCode::from_u16(status).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY),
        &why,
    )
}

/// Run `f` on the blocking pool and answer what it returns as JSON, or its refusal; a panic there
/// is a 500.
async fn blocking<T: Serialize + Send + 'static>(
    f: impl FnOnce() -> Result<T, No> + Send + 'static,
) -> Response {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(no)) => refused(no),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// `GET /api/w/<id>/memory[?creature=&scope=&standing=&review=&offset=&limit=]`:
/// [`memory::BoardMemory`].
pub async fn get_memory(
    State(board): State<Arc<Board>>,
    UrlPath(world): UrlPath<String>,
    uri: Uri,
) -> Response {
    let filter = match memory::Filter::parse(uri.query()) {
        Ok(f) => f,
        Err(e) => return refuse(StatusCode::BAD_REQUEST, &e),
    };
    blocking(move || {
        let path = present(&board, &world)?;
        memory::memory(&path, &filter)
            .map_err(|e| (503, format!("the world's memory cannot be read: {e}")))
    })
    .await
}

/// `GET /api/w/<id>/facts[?status=&offset=&limit=]`: [`facts::BoardFacts`].
pub async fn get_facts(
    State(board): State<Arc<Board>>,
    UrlPath(world): UrlPath<String>,
    uri: Uri,
) -> Response {
    let filter = match facts::Filter::parse(uri.query()) {
        Ok(f) => f,
        Err(e) => return refuse(StatusCode::BAD_REQUEST, &e),
    };
    blocking(move || {
        let path = present(&board, &world)?;
        facts::facts(&path, &filter)
            .map_err(|e| (503, format!("the world's ontology cannot be read: {e}")))
    })
    .await
}

/// The body of a confirm or a wake: the note's `seen` digest, as the page read it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seen {
    seen: String,
}

/// The body of a settle: the fact's `seen` digest, and one proof — `verdict` (a passing end-gate
/// verdict's number) or `statement` (the human's own words).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settle {
    seen: String,
    #[serde(default)]
    verdict: Option<u32>,
    #[serde(default)]
    statement: Option<String>,
}

async fn note_act(
    board: Arc<Board>,
    world: String,
    note: String,
    seen: String,
    act: memory::Act,
) -> Response {
    if let Some(r) = in_a_body() {
        return r;
    }
    if !is_id(&note) {
        return refuse(StatusCode::BAD_REQUEST, "that is no note's id");
    }
    blocking(move || {
        let path = present(&board, &world)?;
        memory::act(&path, &note, &seen, act).map(|done| BoardDone { done })
    })
    .await
}

/// `POST /api/w/<id>/memory/<note>/confirm {"seen"}`: the confirmed route, as `mnemorphd memory
/// confirm` takes it, citing the human's word from the board.
pub async fn confirm(
    State(board): State<Arc<Board>>,
    UrlPath((world, note)): UrlPath<(String, String)>,
    Json(body): Json<Seen>,
) -> Response {
    note_act(board, world, note, body.seen, memory::Act::Confirm).await
}

/// `POST /api/w/<id>/memory/<note>/wake {"seen"}`: a note the gate retired woken, as `mnemorphd
/// memory wake` does, citing the human from the board.
pub async fn wake(
    State(board): State<Arc<Board>>,
    UrlPath((world, note)): UrlPath<(String, String)>,
    Json(body): Json<Seen>,
) -> Response {
    note_act(board, world, note, body.seen, memory::Act::Wake).await
}

/// `POST /api/w/<id>/facts/<fact>/settle {"seen", "verdict" | "statement"}`: the fact settled on
/// the proof given, `by: human (board)`. No proof, or two, is refused.
pub async fn settle(
    State(board): State<Arc<Board>>,
    UrlPath((world, fact)): UrlPath<(String, String)>,
    Json(body): Json<Settle>,
) -> Response {
    if let Some(r) = in_a_body() {
        return r;
    }
    let fact = fact.trim_start_matches('#').to_owned();
    if !is_id(&fact) {
        return refuse(StatusCode::BAD_REQUEST, "that is no fact's id");
    }
    let proof = match (body.verdict, body.statement) {
        (Some(n), None) => facts::Proof::Verdict(n),
        (None, Some(s)) => facts::Proof::Statement(s),
        (None, None) => {
            return refuse(
                StatusCode::BAD_REQUEST,
                "a settle needs a proof: a passing gate verdict (`verdict`) or your own statement \
                 (`statement`)",
            );
        }
        (Some(_), Some(_)) => {
            return refuse(
                StatusCode::BAD_REQUEST,
                "one proof: a gate verdict or your statement, not both",
            );
        }
    };
    let seen = body.seen;
    blocking(move || {
        let path = present(&board, &world)?;
        facts::settle(&path, &fact, &seen, &proof).map(|done| BoardDone { done })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_plain_words() {
        assert!(is_id("c1730000000000-1"));
        assert!(is_id("9f86d081884c7d65"));
        for bad in ["", "../x", "a/b", "a b", "a\u{202e}b", &"x".repeat(129)] {
            assert!(!is_id(bad), "{bad}");
        }
    }

    #[test]
    fn an_act_writes_through_no_link() {
        let w = tempfile::tempdir().unwrap();
        let mem = w.path().join(".mnemorph/memory/shared");
        std::fs::create_dir_all(&mem).unwrap();
        std::fs::write(mem.join("notes.jsonl"), "").unwrap();
        let rel = ".mnemorph/memory/shared/notes.jsonl".to_owned();
        assert_eq!(no_links(w.path(), std::slice::from_ref(&rel)), Ok(()));
        assert_eq!(
            no_links(w.path(), &[".mnemorph/memory/creature/x.jsonl".to_owned()]),
            Ok(()),
            "what is not there yet is created plain"
        );
        assert!(no_links(w.path(), &["../x".to_owned()]).is_err());
        #[cfg(unix)]
        {
            std::fs::remove_file(mem.join("notes.jsonl")).unwrap();
            std::os::unix::fs::symlink("/etc/passwd", mem.join("notes.jsonl")).unwrap();
            let e = no_links(w.path(), std::slice::from_ref(&rel)).unwrap_err();
            assert!(e.contains("is a link"), "{e}");
            std::fs::remove_file(mem.join("notes.jsonl")).unwrap();
            std::fs::remove_dir(&mem).unwrap();
            let elsewhere = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(elsewhere.path(), &mem).unwrap();
            let e = no_links(w.path(), &[rel]).unwrap_err();
            assert!(e.contains("shared is a link"), "{e}");
        }
    }
}
