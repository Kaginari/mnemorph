//! A world's past chats from its run journals, through the board's router: the runs listed newest
//! first with where each stands, a run's transcript as the protocol's events, and every read
//! bounded — a symlink never followed, a pipe never waited on, a journal past its cap not read,
//! a name that is not a run's refused, a world not listed unknown.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt as _;
use mnemorph_server::board::runs::{self, RunState, TRANSCRIPT_BYTES};
use mnemorph_server::board::{Board, Lifetimes};
use mnemorph_server::world::dimension::{hand_file, register_at, world_id};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const HOST: &str = "127.0.0.1:7878";
const ORIGIN: &str = "http://127.0.0.1:7878";

struct Fixture {
    root: PathBuf,
    world: PathBuf,
    board: Arc<Board>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-runs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("world/.mnemorph/instruments/loop")).unwrap();
        let root = root.canonicalize().unwrap();
        let world = root.join("world");
        let registry = root.join("state/mnemorph/worlds.jsonl");
        register_at(&registry, &world, None).unwrap();
        let addrs: Vec<SocketAddr> = vec![HOST.parse().unwrap()];
        let board = Board::living(
            &addrs,
            registry.clone(),
            hand_file(&registry),
            Box::new(|_| {}),
            Lifetimes::default(),
        )
        .unwrap();
        Self { root, world, board }
    }

    fn journals(&self) -> PathBuf {
        self.world.join(".mnemorph/instruments/loop")
    }

    /// A journal of `beats`, last written `ago` seconds back.
    fn journal(&self, run: &str, beats: &[Value], ago: u64) -> PathBuf {
        let path = self.journals().join(format!("{run}.jsonl"));
        let text: String = beats.iter().map(|b| b.to_string() + "\n").collect();
        std::fs::write(&path, text).unwrap();
        let at = SystemTime::now() - Duration::from_secs(ago);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(at)
            .unwrap();
        path
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        let key = self.board.url().split_once("#k=").unwrap().1.to_owned();
        let res = self
            .board
            .router()
            .oneshot(
                Request::post("/session")
                    .header(header::HOST, HOST)
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({ "key": key }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap();
        let cookie = cookie.split(';').next().unwrap().to_owned();
        let res = self
            .board
            .router()
            .oneshot(
                Request::get(path)
                    .header(header::HOST, HOST)
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
        (status, v)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn ask(text: &str) -> Value {
    json!({"beat": "ask", "text": text})
}

fn end(ending: &str) -> Value {
    json!({"beat": "end", "ending": ending})
}

/// A full turn: an ask, a read and its result, the answer, the gate, the end.
fn turn(text: &str) -> Vec<Value> {
    vec![
        ask(text),
        json!({"beat": "model", "stop": "tool_use", "content": [
            {"type": "text", "text": "Reading.", "cache": false},
            {"type": "opaque", "value": {"type": "thinking", "thinking": "hm"}},
            {"type": "tool_use", "id": "t1", "name": "read", "input": {"path": "notes.txt"}}]}),
        json!({"beat": "result", "tool_use_id": "t1", "content": "1\ta\n", "is_error": false}),
        json!({"beat": "recall", "anchors": [], "cut": 0}),
        json!({"beat": "model", "stop": "end_turn", "content": [
            {"type": "text", "text": "It says a.", "cache": false}]}),
        json!({"beat": "gate", "changed": [], "failures": [], "unverified": ["no checks"], "verdict": "NOT_VERIFIED"}),
        end("answered"),
    ]
}

#[tokio::test]
async fn the_runs_are_listed_newest_first_with_where_each_stands() {
    let f = Fixture::new("list");
    f.journal("r1000-1", &turn("the first\nask"), 300);
    f.journal("r2000-2", &[ask("half done")], 200);
    f.journal("r3000-3", &[ask("stop"), end("interrupted")], 100);
    // A body's run, and a file that is no journal, are not a chat.
    f.journal("slime-x", &[ask("mine")], 10);
    std::fs::write(f.journals().join("notes.txt"), "x").unwrap();
    let id = world_id(&f.world);
    let (status, body) = f.get(&format!("/api/w/{id}/runs")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let runs = body["runs"].as_array().unwrap();
    let ids: Vec<&str> = runs.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["r3000-3", "r2000-2", "r1000-1"]);
    assert_eq!(runs[0]["state"], "stopped");
    assert_eq!(runs[0]["why"], "interrupted");
    assert_eq!(runs[1]["state"], "stopped");
    assert_eq!(runs[1]["why"], "cut mid-turn");
    assert_eq!(runs[2]["state"], "ended");
    assert_eq!(runs[2]["why"], "answered");
    assert_eq!(runs[2]["ask"], "the first ask");
    assert_eq!(runs[2]["turns"], 1);
    assert!(runs[2]["when"].as_str().unwrap().ends_with('Z'));
    assert_eq!(body["more"], 0);
}

#[tokio::test]
async fn a_held_run_reads_live() {
    let f = Fixture::new("held");
    f.journal("r1000-1", &[ask("working")], 5);
    let lock = std::fs::File::create(f.journals().join("r1000-1.lock")).unwrap();
    lock.lock().unwrap();
    let list = runs::list(&f.world);
    assert_eq!(list.runs[0].state, RunState::Live);
    assert_eq!(
        runs::transcript(&f.world, "r1000-1").unwrap().state,
        RunState::Live
    );
    drop(lock);
    assert_eq!(runs::list(&f.world).runs[0].state, RunState::Stopped);
}

#[tokio::test]
async fn a_transcript_is_the_protocols_events_in_order() {
    let f = Fixture::new("transcript");
    f.journal("r1000-1", &turn("what does it say?"), 0);
    let id = world_id(&f.world);
    let (status, body) = f.get(&format!("/api/w/{id}/runs/r1000-1")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let methods: Vec<&str> = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["method"].as_str().unwrap())
        .collect();
    assert_eq!(
        methods,
        [
            "event/turnStarted",
            "event/assistant",
            "event/toolCall",
            "event/toolResult",
            "event/assistant",
            "event/gateVerdict",
            "event/turnEnded",
        ]
    );
    let events = body["events"].as_array().unwrap();
    let seqs: Vec<u64> = events
        .iter()
        .map(|e| e["params"]["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(seqs, (1..=7).collect::<Vec<_>>());
    assert_eq!(events[0]["params"]["text"], "what does it say?");
    assert_eq!(events[0]["params"]["run"], "r1000-1");
    assert_eq!(events[2]["params"]["input"]["path"], "notes.txt");
    assert_eq!(events[3]["params"]["isError"], false);
    assert_eq!(events[5]["params"]["verdict"], "NOT_VERIFIED");
    assert_eq!(events[5]["params"]["passed"], false);
    assert_eq!(body["state"], "ended");
    assert_eq!(body["dropped"], 0);
    assert_eq!(body["skipped"], json!([]));
}

#[tokio::test]
async fn a_line_that_does_not_read_is_counted_never_guessed() {
    let f = Fixture::new("skipped");
    let path = f.journal("r1000-1", &turn("go"), 0);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.insert_str(0, "{not json\n");
    std::fs::write(&path, text).unwrap();
    let t = runs::transcript(&f.world, "r1000-1").unwrap();
    assert_eq!(t.skipped, [1]);
    assert_eq!(t.events.len(), 7);
    let list = runs::list(&f.world);
    assert_eq!(list.runs[0].state, RunState::Unknown);
    assert!(
        list.runs[0].why.contains("do not read"),
        "{}",
        list.runs[0].why
    );
}

#[tokio::test]
async fn a_name_that_is_no_run_and_a_world_not_listed_are_refused() {
    let f = Fixture::new("names");
    let id = world_id(&f.world);
    for bad in ["..", "r1-2.jsonl", "slime-x", "%2e%2e%2fetc"] {
        let (status, _) = f.get(&format!("/api/w/{id}/runs/{bad}")).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
            "{bad}: {status}"
        );
    }
    let (status, _) = f.get(&format!("/api/w/{id}/runs/r9-9")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = f.get("/api/w/ghost-0/runs").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_is_never_followed_and_a_pipe_never_waited_on() {
    let f = Fixture::new("planted");
    let secret = f.root.join("secret.jsonl");
    std::fs::write(&secret, format!("{}\n", ask("the secret"))).unwrap();
    std::os::unix::fs::symlink(&secret, f.journals().join("r1000-1.jsonl")).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(f.journals().join("r2000-2.jsonl"))
            .status()
            .unwrap()
            .success()
    );
    // A pipe for a lock is never waited on either.
    f.journal("r3000-3", &[ask("fine")], 0);
    assert!(
        std::process::Command::new("mkfifo")
            .arg(f.journals().join("r3000-3.lock"))
            .status()
            .unwrap()
            .success()
    );
    let list = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::task::spawn_blocking({
            let w = f.world.clone();
            move || runs::list(&w)
        }),
    )
    .await
    .expect("the listing never waits on a pipe")
    .unwrap();
    let by = |id: &str| list.runs.iter().find(|r| r.id == id).unwrap().clone();
    assert_eq!(by("r1000-1").state, RunState::Unknown);
    assert_eq!(by("r1000-1").ask, "");
    assert_eq!(by("r2000-2").state, RunState::Unknown);
    assert_eq!(by("r3000-3").state, RunState::Stopped);
    for run in ["r1000-1", "r2000-2"] {
        let (status, why) = runs::transcript(&f.world, run).unwrap_err();
        assert_eq!(status, 422, "{run}: {why}");
    }
    // A journals directory reached through a symlink is not read at all.
    let elsewhere = f.root.join("elsewhere");
    std::fs::rename(f.journals(), &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, f.journals()).unwrap();
    assert_eq!(runs::list(&f.world).runs, vec![]);
    assert_eq!(runs::transcript(&f.world, "r3000-3").unwrap_err().0, 404);
}

#[tokio::test]
async fn a_journal_past_its_cap_is_not_read() {
    let f = Fixture::new("cap");
    let path = f.journal("r1000-1", &[ask("big")], 0);
    let file = std::fs::File::options().write(true).open(&path).unwrap();
    file.set_len(TRANSCRIPT_BYTES + 1).unwrap();
    let (status, why) = runs::transcript(&f.world, "r1000-1").unwrap_err();
    assert_eq!(status, 422);
    assert!(why.contains("MiB"), "{why}");
    assert_eq!(runs::list(&f.world).runs[0].state, RunState::Unknown);
}

#[test]
fn a_world_without_journals_lists_none() {
    let w = Path::new("/nonexistent/mnemorph-world");
    assert_eq!(runs::list(w).runs, vec![]);
    assert_eq!(runs::transcript(w, "r1-2").unwrap_err().0, 404);
}
