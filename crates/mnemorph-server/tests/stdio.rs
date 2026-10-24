//! The client protocol over a pair of pipes, as the CLI drives it: a scripted model, a real loop.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use mnemorph_loop::{Bound, Config, Paths, ScriptedModel, open_with};
use serde_json::{Value, json};

struct Client {
    to: std::io::PipeWriter,
    lines: mpsc::Receiver<Value>,
    root: PathBuf,
    next: u64,
}

impl Client {
    /// A server over a fresh world, its model answering from `script`; `config` is the world's
    /// local configuration.
    fn start(name: &str, config: &str, script: Vec<mnemorph_loop::model::Reply>) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-server-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let world = root.join("world");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        std::fs::write(world.join(".mnemorph/config.local.toml"), config).unwrap();
        std::fs::write(world.join("notes.txt"), "a\n").unwrap();
        let (server_in, to) = std::io::pipe().unwrap();
        let (from, server_out) = std::io::pipe().unwrap();
        let global = root.join("global.toml");
        let attic = root.join("attic");
        std::thread::spawn(move || {
            let opener: mnemorph_server::Opener = Box::new(move |gate, observer| {
                let config = Config::load(&Paths::with_global(global, &world))?;
                let mut opened = open_with(
                    &world,
                    &config,
                    Bound {
                        model: Arc::new(ScriptedModel::new(script)),
                        model_id: "scripted-model",
                        // Tests keep their attic to themselves.
                        attic: Some(&attic),
                    },
                    gate,
                    observer,
                    |_| None,
                )?;
                opened.model = "scripted-model".into();
                Ok(opened)
            });
            mnemorph_server::serve(BufReader::new(server_in), server_out, "test", opener).unwrap();
        });
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(from).lines() {
                let Ok(line) = line else { break };
                if tx.send(serde_json::from_str(&line).unwrap()).is_err() {
                    break;
                }
            }
        });
        Self {
            to,
            lines,
            root,
            next: 0,
        }
    }

    fn send(&mut self, v: &Value) {
        writeln!(self.to, "{v}").unwrap();
        self.to.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: &Value) -> u64 {
        self.next += 1;
        let id = self.next;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    /// Lines until one matches, each kept.
    fn until(&mut self, seen: &mut Vec<Value>, mut done: impl FnMut(&Value) -> bool) -> Value {
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(20))
                .unwrap_or_else(|_| panic!("timed out; saw {seen:#?}"));
            seen.push(line.clone());
            if done(&line) {
                return line;
            }
        }
    }

    fn response(&mut self, id: u64) -> Value {
        self.until(&mut Vec::new(), |l| l["id"] == json!(id))
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn method(l: &Value) -> &str {
    l["method"].as_str().unwrap_or_default()
}

const UNGUARDED: &str = "[sandbox]\nallow_unguarded = true\n";

#[test]
fn a_session_streams_its_turn_and_numbers_every_event() {
    let mut c = Client::start(
        "turn",
        UNGUARDED,
        vec![
            ScriptedModel::tool("t1", "read", json!({"path": "notes.txt"})),
            ScriptedModel::tool("t2", "write", json!({"path": "out.txt", "content": "b\n"})),
            ScriptedModel::text("It says a."),
        ],
    );
    let id = c.request("initialize", &json!({"clientInfo": {"name": "test"}}));
    let hello = c.response(id);
    assert_eq!(hello["result"]["protocol"], 1, "{hello}");
    assert_eq!(hello["result"]["model"], "scripted-model");
    assert!(
        hello["result"]["world"]
            .as_str()
            .unwrap()
            .ends_with("world")
    );

    let id = c.request(
        "session/prompt",
        &json!({"text": "what does notes.txt say?"}),
    );
    let mut seen = Vec::new();
    c.until(&mut seen, |l| l["id"] == json!(id));
    c.until(&mut seen, |l| method(l) == "event/turnEnded");
    let methods: Vec<&str> = seen.iter().map(method).filter(|m| !m.is_empty()).collect();
    for want in [
        "event/turnStarted",
        "event/toolCall",
        "event/toolResult",
        "event/usage",
        "event/assistant",
        "event/turnEnded",
    ] {
        assert!(methods.contains(&want), "{want} in {methods:?}");
    }
    let result = seen
        .iter()
        .find(|l| method(l) == "event/toolResult")
        .unwrap();
    assert!(
        result["params"]["content"]
            .as_str()
            .unwrap()
            .ends_with("1\ta\n"),
        "{result}"
    );
    let ended = seen.last().unwrap();
    assert_eq!(ended["params"]["ending"], "answered");
    // Every event names its session and run, and seq rises by one.
    let seqs: Vec<u64> = seen
        .iter()
        .filter(|l| method(l).starts_with("event/"))
        .map(|l| {
            assert_eq!(l["params"]["session"], hello["result"]["session"]);
            assert_eq!(l["params"]["run"], hello["result"]["run"]);
            l["params"]["seq"].as_u64().unwrap()
        })
        .collect();
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");

    // The write was kept in this test's own attic, never the human's.
    assert!(c.root.join("attic").exists(), "the attic is the test's");

    // A client that reconnects asks for what it missed and loses nothing.
    let id = c.request("session/events", &json!({"since": seqs[2]}));
    let replay = c.response(id);
    let got: Vec<u64> = replay["result"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["params"]["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(got, seqs[3..].to_vec());
}

/// The human gate is a request to the client; its answer is the gate's, and a denial's reason
/// reaches the body.
#[test]
fn the_gate_asks_the_client_and_its_denial_reaches_the_body() {
    let config = format!("{UNGUARDED}[gate]\nask_rules = [\"in-place-edit\"]\n");
    let mut c = Client::start(
        "gate",
        &config,
        vec![
            ScriptedModel::tool("t1", "bash", json!({"command": "sed -i s/a/b/ notes.txt"})),
            ScriptedModel::text("I will not."),
        ],
    );
    let id = c.request("initialize", &json!({}));
    c.response(id);
    c.request("session/prompt", &json!({"text": "change a to b"}));
    let mut seen = Vec::new();
    let ask = c.until(&mut seen, |l| method(l) == "gate/ask");
    assert_eq!(ask["params"]["act"], "sed -i s/a/b/ notes.txt", "{ask}");
    assert!(ask["params"]["class"].is_string());
    c.send(&json!({"jsonrpc": "2.0", "id": ask["id"], "result": {"allow": false, "reason": "keep the a"}}));
    let result = c.until(&mut seen, |l| method(l) == "event/toolResult");
    assert_eq!(result["params"]["isError"], true);
    assert!(
        result["params"]["content"]
            .as_str()
            .unwrap()
            .contains("keep the a"),
        "{result}"
    );
    c.until(&mut seen, |l| method(l) == "event/turnEnded");
    assert_eq!(
        std::fs::read_to_string(c.root.join("world/notes.txt")).unwrap(),
        "a\n"
    );
}

/// Lines sent while a turn runs wait their turn and are taken strictly in order.
#[test]
fn lines_sent_during_a_turn_are_queued_in_order() {
    let mut c = Client::start(
        "queue",
        UNGUARDED,
        vec![
            ScriptedModel::text("one"),
            ScriptedModel::text("two"),
            ScriptedModel::text("three"),
        ],
    );
    let id = c.request("initialize", &json!({}));
    c.response(id);
    for text in ["first", "second", "third"] {
        c.request("session/prompt", &json!({"text": text}));
    }
    let mut seen = Vec::new();
    let mut ended = 0;
    c.until(&mut seen, |l| {
        if method(l) == "event/turnEnded" {
            ended += 1;
        }
        ended == 3
    });
    let started: Vec<&str> = seen
        .iter()
        .filter(|l| method(l) == "event/turnStarted")
        .map(|l| l["params"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(started, ["first", "second", "third"]);
}

#[test]
fn a_bad_request_is_answered_with_an_error_and_the_session_goes_on() {
    let mut c = Client::start("errors", UNGUARDED, vec![ScriptedModel::text("ok")]);
    let id = c.request("session/prompt", &json!({"text": "too early"}));
    assert_eq!(c.response(id)["error"]["code"], -32002);
    let id = c.request("no/such", &json!({}));
    assert_eq!(c.response(id)["error"]["code"], -32601);
    c.send(&json!("not an object"));
    let id = c.request("initialize", &json!({}));
    assert!(c.response(id)["result"]["protocol"].is_number());
    let id = c.request("initialize", &json!({}));
    assert_eq!(c.response(id)["error"]["code"], -32600);
    let id = c.request("shutdown", &json!({}));
    assert!(c.response(id)["result"].is_object());
}
