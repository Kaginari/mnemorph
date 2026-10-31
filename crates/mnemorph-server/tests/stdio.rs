//! The client protocol over a pair of pipes, as the CLI drives it: a scripted model, a real loop.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use mnemorph_loop::{Bound, Config, Paths, ScriptedModel, open_with};
use serde_json::{Value, json};

struct Client {
    to: Option<std::io::PipeWriter>,
    lines: mpsc::Receiver<Value>,
    done: mpsc::Receiver<()>,
    root: PathBuf,
    next: u64,
}

impl Client {
    /// A server over a fresh world, its model answering from `script`; `config` is the world's
    /// local configuration.
    fn start(name: &str, config: &str, script: Vec<mnemorph_loop::model::Reply>) -> Self {
        Self::start_failing(name, config, script, 0)
    }

    /// As [`Client::start`], the first `fails` openings failing.
    fn start_failing(
        name: &str,
        config: &str,
        script: Vec<mnemorph_loop::model::Reply>,
        mut fails: u32,
    ) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-server-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let world = root.join("world");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        // The test's settings — running unguarded among them — are the human's global file's.
        std::fs::write(root.join("global.toml"), config).unwrap();
        std::fs::write(world.join("notes.txt"), "a\n").unwrap();
        let (server_in, to) = std::io::pipe().unwrap();
        let (from, server_out) = std::io::pipe().unwrap();
        let global = root.join("global.toml");
        let attic = root.join("attic");
        let (finished, done) = mpsc::channel();
        let mut script = Some(script);
        std::thread::spawn(move || {
            let opener: mnemorph_server::Opener = Box::new(move |gate, observer| {
                if fails > 0 {
                    fails -= 1;
                    return Err("the backend is not reachable".into());
                }
                let config = Config::load(&Paths::with_global(global.clone(), &world))?;
                let mut opened = open_with(
                    &world,
                    &config,
                    Bound {
                        model: Arc::new(ScriptedModel::new(script.take().unwrap_or_default())),
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
            let _ = finished.send(());
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
            to: Some(to),
            lines,
            done,
            root,
            next: 0,
        }
    }

    fn send(&mut self, v: &Value) {
        let to = self.to.as_mut().unwrap();
        writeln!(to, "{v}").unwrap();
        to.flush().unwrap();
    }

    /// Whether the server returned within `secs`.
    fn ended_within(&self, secs: u64) -> bool {
        self.done.recv_timeout(Duration::from_secs(secs)).is_ok()
    }

    /// Lines that arrive within `ms`.
    fn quiet_for(&self, ms: u64) -> Vec<Value> {
        let mut out = Vec::new();
        while let Ok(l) = self.lines.recv_timeout(Duration::from_millis(ms)) {
            out.push(l);
        }
        out
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
    let events = replay["result"]["events"].as_array().unwrap();
    let got: Vec<u64> = events
        .iter()
        .map(|e| e["params"]["seq"].as_u64().unwrap())
        .collect();
    // What came after the turn ended — the queue's held count — may already be there too.
    assert_eq!(got[..seqs.len() - 3], seqs[3..]);
    assert!(
        events[seqs.len() - 3..]
            .iter()
            .all(|e| method(e) == "event/held"),
        "{events:?}"
    );
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

/// Two answers to one ask — the board's and another front end's, racing — reach the core: the
/// first wins, and the second is no answer to anything.
#[test]
fn the_first_answer_to_an_ask_wins() {
    let config = format!("{UNGUARDED}[gate]\nask_rules = [\"in-place-edit\"]\n");
    let mut c = Client::start(
        "gate-race",
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
    c.send(
        &json!({"jsonrpc": "2.0", "id": ask["id"], "result": {"allow": false, "reason": "first"}}),
    );
    c.send(&json!({"jsonrpc": "2.0", "id": ask["id"], "result": {"allow": true}}));
    let result = c.until(&mut seen, |l| method(l) == "event/toolResult");
    assert_eq!(result["params"]["isError"], true, "{result}");
    c.until(&mut seen, |l| method(l) == "event/turnEnded");
    assert_eq!(
        std::fs::read_to_string(c.root.join("world/notes.txt")).unwrap(),
        "a\n"
    );
}

/// Lines sent while a turn runs wait their turn and are taken strictly in order.
#[test]
fn lines_sent_during_a_turn_are_queued_in_order() {
    // The first turn waits at the gate, so the next two arrive while it runs.
    let config = format!("{UNGUARDED}[gate]\nask_rules = [\"in-place-edit\"]\n");
    let mut c = Client::start(
        "queue",
        &config,
        vec![
            ScriptedModel::tool("t1", "bash", json!({"command": "sed -i s/a/b/ notes.txt"})),
            ScriptedModel::text("one"),
            ScriptedModel::text("two"),
            ScriptedModel::text("three"),
        ],
    );
    let id = c.request("initialize", &json!({}));
    c.response(id);
    let first = c.request("session/prompt", &json!({"text": "first"}));
    let mut seen = Vec::new();
    let ask = c.until(&mut seen, |l| method(l) == "gate/ask");
    assert_eq!(
        seen.iter().find(|l| l["id"] == json!(first)).unwrap()["result"]["queued"],
        false
    );
    for text in ["second", "third"] {
        let id = c.request("session/prompt", &json!({"text": text}));
        let reply = c.until(&mut seen, |l| l["id"] == json!(id));
        assert_eq!(reply["result"]["queued"], true, "{text}");
    }
    c.send(&json!({"jsonrpc": "2.0", "id": ask["id"], "result": {"allow": false}}));
    let mut ended = 0;
    c.until(&mut seen, |l| {
        if method(l) == "event/turnEnded" {
            ended += 1;
        }
        ended == 3
    });
    let queued: Vec<&str> = seen
        .iter()
        .filter(|l| method(l) == "event/queued")
        .map(|l| l["params"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(queued, ["second", "third"]);
    let started: Vec<&str> = seen
        .iter()
        .filter(|l| method(l) == "event/turnStarted")
        .map(|l| l["params"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(started, ["first", "second", "third"]);
    // Events reach the wire in the order of their numbers.
    let seqs: Vec<u64> = seen
        .iter()
        .filter_map(|l| l["params"]["seq"].as_u64())
        .collect();
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");
}

#[test]
fn a_bad_request_is_answered_with_an_error_and_the_session_goes_on() {
    let mut c = Client::start_failing("errors", UNGUARDED, vec![ScriptedModel::text("ok")], 1);
    let id = c.request("session/prompt", &json!({"text": "too early"}));
    assert_eq!(c.response(id)["error"]["code"], -32002);
    let id = c.request("no/such", &json!({}));
    assert_eq!(c.response(id)["error"]["code"], -32601);
    // Not an object, and a batch: both refused, by name, with no id to answer.
    c.send(&json!("not an object"));
    let l = c.until(&mut Vec::new(), |l| l["error"].is_object());
    assert_eq!(l["error"]["code"], -32600);
    c.send(&json!([{"jsonrpc": "2.0", "id": 99, "method": "shutdown"}]));
    let l = c.until(&mut Vec::new(), |l| l["error"].is_object());
    assert!(
        l["error"]["message"].as_str().unwrap().contains("batches"),
        "{l}"
    );
    // A notification is never answered.
    c.send(&json!({"jsonrpc": "2.0", "method": "session/interrupt"}));
    assert_eq!(c.quiet_for(300), [] as [serde_json::Value; 0]);
    // A failed opening can be tried again.
    let id = c.request("initialize", &json!({}));
    assert_eq!(c.response(id)["error"]["code"], -32001);
    let id = c.request("initialize", &json!({}));
    assert!(c.response(id)["result"]["protocol"].is_number());
    let id = c.request("initialize", &json!({}));
    assert_eq!(c.response(id)["error"]["code"], -32600);
    // After shutdown the server ends, though its input is still open.
    let id = c.request("shutdown", &json!({}));
    assert!(c.response(id)["result"].is_object());
    assert!(c.ended_within(5), "shutdown ends the server");
}

/// The client leaving — by `shutdown` or by closing its end — releases a waiting gate at once and
/// runs nothing queued behind it.
#[test]
fn leaving_releases_the_gate_and_drops_what_was_queued() {
    for by_shutdown in [true, false] {
        let config = format!("{UNGUARDED}[gate]\nask_rules = [\"in-place-edit\"]\n");
        let mut c = Client::start(
            if by_shutdown {
                "leave-shutdown"
            } else {
                "leave-eof"
            },
            &config,
            vec![
                ScriptedModel::tool("t1", "bash", json!({"command": "sed -i s/a/b/ notes.txt"})),
                ScriptedModel::text("stopped"),
                ScriptedModel::text("never"),
            ],
        );
        let id = c.request("initialize", &json!({}));
        c.response(id);
        c.request("session/prompt", &json!({"text": "first"}));
        c.until(&mut Vec::new(), |l| method(l) == "gate/ask");
        c.request("session/prompt", &json!({"text": "queued behind it"}));
        if by_shutdown {
            c.request("shutdown", &json!({}));
        } else {
            c.to = None;
        }
        assert!(
            c.ended_within(5),
            "by_shutdown={by_shutdown}: the server ends at once"
        );
        let rest = c.quiet_for(200);
        assert!(
            !rest
                .iter()
                .any(|l| l["params"]["text"] == "queued behind it"
                    && method(l) == "event/turnStarted"),
            "nothing queued runs after the client left: {rest:#?}"
        );
        assert_eq!(
            std::fs::read_to_string(c.root.join("world/notes.txt")).unwrap(),
            "a\n"
        );
    }
}

/// Esc stops a turn that waits at the gate: the waiting act is denied, and the turn ends.
#[test]
fn esc_releases_a_waiting_gate() {
    let config = format!("{UNGUARDED}[gate]\nask_rules = [\"in-place-edit\"]\n");
    let mut c = Client::start(
        "esc-gate",
        &config,
        vec![
            ScriptedModel::tool("t1", "bash", json!({"command": "sed -i s/a/b/ notes.txt"})),
            ScriptedModel::text("never"),
        ],
    );
    let id = c.request("initialize", &json!({}));
    c.response(id);
    c.request("session/prompt", &json!({"text": "change it"}));
    let mut seen = Vec::new();
    c.until(&mut seen, |l| method(l) == "gate/ask");
    c.request("session/interrupt", &json!({}));
    let ended = c.until(&mut seen, |l| method(l) == "event/turnEnded");
    assert_eq!(ended["params"]["ending"], "interrupted", "{seen:#?}");
}

/// The human's cap stops a session at a checkpoint once it is spent, and the hello says what is
/// left, so the status line can.
#[test]
fn a_session_stops_at_the_humans_cap() {
    let config = format!("{UNGUARDED}[budget]\nsession = 1\n");
    let mut c = Client::start(
        "budget",
        &config,
        vec![
            ScriptedModel::tool("t1", "read", json!({"path": "notes.txt"})),
            ScriptedModel::text("never reached"),
        ],
    );
    let id = c.request("initialize", &json!({}));
    let hello = c.response(id);
    assert_eq!(
        hello["result"]["budget"],
        json!({"leftWt": 1, "cap": "session"}),
        "{hello}"
    );
    c.request("session/prompt", &json!({"text": "read it"}));
    let ended = c.until(&mut Vec::new(), |l| method(l) == "event/turnEnded");
    let ending = ended["params"]["ending"].as_str().unwrap();
    assert!(ending.starts_with("checkpoint: budget"), "{ending}");
}

/// A ledger with a line it cannot read still opens a capped session: the line is counted high,
/// against every cap (20 000 wt at least), and the hello says so and names the lines to repair.
#[test]
fn a_torn_ledger_is_counted_high() {
    let config = format!("{UNGUARDED}[budget]\nday = 1000000\n");
    let mut c = Client::start("torn-ledger", &config, vec![ScriptedModel::text("never")]);
    let instruments = c.root.join("world/.mnemorph/instruments");
    std::fs::create_dir_all(&instruments).unwrap();
    std::fs::write(instruments.join("usage.jsonl"), "{\"torn\n").unwrap();
    let id = c.request("initialize", &json!({}));
    let hello = c.response(id);
    let holes = hello["result"]["holes"].to_string();
    assert!(holes.contains("unreadable line(s) ([1])"), "{hello}");
    assert_eq!(
        hello["result"]["budget"],
        json!({"leftWt": 980_000, "cap": "day"}),
        "{hello}"
    );
}

/// A send with nothing running is refused, by name: a line sent steers a turn that runs.
#[test]
fn a_send_with_nothing_running_is_refused() {
    let mut c = Client::start("send-idle", UNGUARDED, vec![ScriptedModel::text("ok")]);
    let id = c.request("initialize", &json!({}));
    c.response(id);
    let id = c.request("session/send", &json!({"text": "faster"}));
    assert_eq!(c.response(id)["error"]["code"], -32003);
}

/// A session in a world with creatures streams every commissioned body's events under that body's
/// own name and run, and its hello names the session's own body.
#[test]
fn a_commissioned_body_reaches_the_wire_under_its_own_name() {
    let tool = |id: &str, to: &str| {
        ScriptedModel::tool(
            id,
            "commission",
            json!({"to": to, "ask": "leave src/done.txt"}),
        )
    };
    let mut c = Client::start(
        "below",
        UNGUARDED,
        vec![
            tool("c1", "elf-e"),
            tool("c2", "orc-o"),
            tool("c3", "slime-s"),
            ScriptedModel::tool(
                "w1",
                "write",
                json!({"path": "src/done.txt", "content": "done\n"}),
            ),
            ScriptedModel::text("@S DONE\n@E"),
            ScriptedModel::text("@S DONE\n@E"),
            ScriptedModel::text("@S DONE\n@E"),
            ScriptedModel::text("Done."),
        ],
    );
    let world = c.root.join("world");
    let doc = |race: &str, short: &str, body: &str| {
        let dir = world.join(".mnemorph").join(race).join(short);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("README.md"), body).unwrap();
    };
    doc("rimuru", "rimuru", "# rimuru\n\n- **Race:** rimuru\n");
    doc(
        "elf",
        "e",
        "# elf-e\n\n- **Race:** elf\n- **Reports to:** rimuru\n- **Territory:** src/\n",
    );
    std::fs::write(
        world.join(".mnemorph/elf/e/plan.md"),
        "- [ ] orc-o: the marker\n",
    )
    .unwrap();
    doc(
        "orc",
        "o",
        "# orc-o\n\n- **Race:** orc\n- **Reports to:** elf-e\n- **Domain:** src/\n",
    );
    doc(
        "slime",
        "s",
        "# slime-s\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n- **Verify:** `true`\n",
    );
    std::fs::create_dir_all(world.join("src")).unwrap();
    let id = c.request("initialize", &json!({}));
    let hello = c.response(id);
    assert_eq!(hello["result"]["body"], "rimuru", "{hello}");
    let session_run = hello["result"]["run"].clone();
    c.request("session/prompt", &json!({"text": "leave a marker"}));
    let mut seen = Vec::new();
    c.until(&mut seen, |l| {
        method(l) == "event/turnEnded" && l["params"]["body"] == "rimuru"
    });
    let wrote = seen
        .iter()
        .find(|l| method(l) == "event/toolCall" && l["params"]["name"] == "write")
        .expect("the Slime's write reached the wire");
    assert_eq!(wrote["params"]["body"], "slime-s", "{wrote}");
    assert_ne!(
        wrote["params"]["run"], session_run,
        "a child's run is its own"
    );
}

/// An outward act a probe left in `reconcile` is shown to the human in the hello, which
/// `mnemorph status` prints, until a later row settles it.
#[test]
fn an_act_in_reconcile_is_shown_to_the_human() {
    let mut c = Client::start("reconcile", UNGUARDED, vec![ScriptedModel::text("never")]);
    let post = c.root.join("world/.mnemorph/kijin/kijin-forge");
    std::fs::create_dir_all(&post).unwrap();
    std::fs::write(
        post.join("post.jsonl"),
        concat!(
            r#"{"act":"a1","state":"committed","at":"2026-10-27T10:00:00Z"}"#,
            "\n",
            r#"{"act":"a1","state":"reconcile","at":"2026-10-27T10:00:05Z","why":"refs/heads/feat/x is not on the remote"}"#,
            "\n",
            r#"{"act":"a2","state":"observed","at":"2026-10-27T10:01:00Z"}"#,
            "\n",
        ),
    )
    .unwrap();
    let id = c.request("initialize", &json!({}));
    let hello = c.response(id);
    let holes = hello["result"]["holes"].to_string();
    assert!(
        holes.contains(
            "kijin-forge act a1 is in reconcile since 2026-10-27T10:00:05Z: refs/heads/feat/x is not on the remote"
        ),
        "{hello}"
    );
    assert!(!holes.contains("act a2"), "{hello}");
}
