//! #48, the last wave, through the loop — no network, no key: each provider's encoder
//! round-trips real-shaped MCP schemas; on Anthropic, level 2 is a native deferred tool, recorded
//! as the requests a scripted transport received (a cassette of the exchange); a creature's
//! habitual tool joins its stable prefix only from the chained journal under the state directory,
//! only when the session starts, and the frame never moves while it lives.

#![allow(
    clippy::assert_is_empty,
    clippy::format_collect,
    reason = "test fixtures, read once"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_ledger::Journal;
use mnemorph_loop::{
    AnthropicModel, Beat, Body, Ending, Limits, Run, ScriptedModel, Setup, mcp_journal,
};
use mnemorph_provider::{
    Anthropic, Block, Caller, Message, Policy, Reply, Request, Role, Scripted, Tool,
    anthropic_prices, encode, encode_chat,
};
use mnemorph_races::{Race, ToolClass};
use mnemorph_tools::mcp::{Frame, Kind, Mcp, OPEN, Server, Settings, Spec, Touched};
use mnemorph_tools::{Context, Sandbox, call, registry};
use serde_json::{Value, json};

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-mcp-native-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world")).unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    /// The MCP state, under the test's folder: never the human's state directory.
    fn state(&self) -> PathBuf {
        self.root.join("state/mnemorph/mcp/world-0")
    }

    fn server(&self) -> Server {
        let list = self.root.join("tools.json");
        fs::write(
            &list,
            json!([
                {"name": "issue_find", "description": "Find an issue in a tracker", "inputSchema": {"type": "object", "properties": {"q": {"type": "string"}}}},
                {"name": "web_search", "description": "Search the web for pages", "inputSchema": {"type": "object", "properties": {"q": {"type": "string"}}}},
            ])
            .to_string(),
        )
        .unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../mnemorph-tools/tests/fixtures/fake-mcp.sh");
        let mut hub = Server::new("hub", "bash", &[Race::Slime]);
        hub.args = vec![
            fixture.display().to_string(),
            list.display().to_string(),
            self.root.join("mcp-log").display().to_string(),
        ];
        hub.class = Some(ToolClass::Read);
        hub
    }

    /// A session's MCP: its catalog listed once already — a fresh cache — and each creature's
    /// habits chosen as a session start chooses them, from the journal as the loop reads it.
    fn session(&self) -> Arc<Mcp> {
        let mcp = Mcp::new(
            Settings {
                servers: vec![self.server()],
                ..Settings::default()
            },
            self.state(),
        );
        let mut cx = self.cx();
        let offer = mcp.offer(Race::Slime, &cx);
        let _ = call(
            &offer.tools,
            OPEN,
            &json!({"server": "hub"}).to_string(),
            &mut cx,
        );
        mcp.freeze_habits(mcp_journal::read(mcp.state()));
        mcp
    }

    fn cx(&self) -> Context {
        Context::new(Race::Slime, self.world(), &["."])
            .with_sandbox(Sandbox::None("loop tests run unguarded".into()))
            .with_unguarded(true)
            .with_body("slime-test")
    }

    fn run(
        &self,
        model: Arc<dyn mnemorph_loop::model::Model>,
        mcp: &Arc<Mcp>,
        model_id: &str,
    ) -> Run {
        let cx = self.cx();
        let system = "You are a test body.";
        let mut tools = registry();
        let frame = mcp_journal::frame(
            mcp,
            Race::Slime,
            &cx,
            system,
            &tools,
            model.defers(model_id),
        );
        tools.extend(mcp.offer_framed(Race::Slime, &cx, frame).tools);
        let setup = Setup {
            model,
            tools,
            cx,
            journal: Journal::new(self.root.join("run.jsonl")),
            ledger: None,
            prices: anthropic_prices().unwrap(),
            caller: Caller {
                session: "s".into(),
                run: "r".into(),
                body: "slime-test".into(),
                race: "slime".into(),
                commission: None,
                prefix: 0,
            },
        };
        Run::new(
            Body {
                name: "slime-test".into(),
                race: Race::Slime,
                model: model_id.into(),
                system: system.into(),
            },
            Limits::for_race(Race::Slime),
            setup,
        )
        .with_mcp(Some(Arc::clone(mcp)))
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.root.join("mcp-log"))
            .unwrap_or_default()
            .lines()
            .filter(|l| l.starts_with("call "))
            .map(str::to_owned)
            .collect()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

async fn ask(run: &mut Run, text: &str) -> Ending {
    let mut quiet = |_| {};
    run.ask(text, &mut quiet).await
}

// --- The encoders, round-tripped ---------------------------------------------------------------

/// Tools as servers really list them: nested objects and arrays of objects, enums, `$ref` into
/// `$defs`, `anyOf` with null, patterns with backslashes, quotes and newlines in descriptions,
/// odd property names, and names with dashes and digits.
fn real_shaped() -> Vec<Value> {
    vec![
        json!({
            "name": "create_issue",
            "description": "Create a new issue in a GitHub repository.\n\nUse \"labels\" for triage.",
            "inputSchema": {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "title": "CreateIssueArgs",
                "properties": {
                    "owner": {"type": "string", "description": "Repository owner"},
                    "repo": {"type": "string", "pattern": "^[\\w.-]+$"},
                    "title": {"type": "string", "description": "Issue title", "examples": ["Bug: x"]},
                    "labels": {"type": "array", "items": {"type": "string"}},
                    "assignees": {"type": "array", "items": {"type": "object", "properties": {
                        "login": {"type": "string"},
                        "role": {"type": "string", "enum": ["owner", "member", "outside"]}
                    }, "required": ["login"], "additionalProperties": false}},
                    "milestone": {"anyOf": [{"type": "integer", "minimum": 1}, {"type": "null"}]}
                },
                "required": ["owner", "repo", "title"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "query-v2",
            "description": "Run a query",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filter": {"$ref": "#/$defs/Filter"},
                    "x-request-id": {"type": "string", "format": "uuid"},
                    "with space": {"type": "boolean", "default": false},
                    "ünïcode": {"type": "number", "exclusiveMaximum": 1.5},
                    "a.b": {"type": "string", "const": "fixed"},
                    "$ref": {"type": "string", "description": "a property named like a keyword"}
                },
                "$defs": {
                    "Filter": {"type": "object", "properties": {
                        "field": {"type": "string"},
                        "op": {"type": "string", "enum": ["eq", "ne", "lt", "gt"]},
                        "and": {"type": "array", "items": {"$ref": "#/$defs/Filter"}}
                    }, "required": ["field", "op"]}
                }
            }
        }),
        json!({"name": "ping", "description": "No arguments at all"}),
    ]
}

/// The specs as the harness keeps them: listed by a server named with a dash and digits, each
/// normalised once.
fn specs() -> Vec<Spec> {
    let server = Server::new("git-hub2", "bash", &[Race::Kijin]);
    real_shaped()
        .iter()
        .map(|raw| Spec::of(&server, raw, false).unwrap())
        .collect()
}

fn request(tools: Vec<Tool>) -> Request {
    Request {
        model: "claude-sonnet-5-5".into(),
        system: vec![Block::Text {
            text: "sys".into(),
            cache: true,
        }],
        tools,
        messages: vec![Message {
            role: Role::User,
            content: vec![Block::Text {
                text: "go".into(),
                cache: false,
            }],
        }],
        max_tokens: 1024,
        effort: None,
    }
}

fn provider_tools(specs: &[Spec], defer: bool) -> Vec<Tool> {
    let mut tools = vec![Tool {
        name: "read".into(),
        description: "read a file".into(),
        input_schema: json!({"type": "object"}),
        cache: true,
        defer: false,
    }];
    tools.extend(specs.iter().map(|s| Tool {
        name: s.name.clone(),
        description: mnemorph_tools::mcp::Native::description_of(s),
        input_schema: s.schema.clone(),
        cache: false,
        defer,
    }));
    tools
}

/// Through bytes, as the client sends them, and back.
fn wire(v: &Value) -> Value {
    serde_json::from_slice(&serde_json::to_vec(v).unwrap()).unwrap()
}

#[test]
fn the_anthropic_encoder_round_trips_real_shaped_schemas() {
    let specs = specs();
    for defer in [false, true] {
        let sent = wire(&encode(&request(provider_tools(&specs, defer))).unwrap());
        let back = sent["tools"].as_array().unwrap();
        assert_eq!(back.len(), specs.len() + 1);
        for (spec, tool) in specs.iter().zip(&back[1..]) {
            assert_eq!(tool["name"], json!(spec.name));
            assert_eq!(
                tool["description"],
                json!(mnemorph_tools::mcp::Native::description_of(spec))
            );
            assert_eq!(tool["input_schema"], spec.schema, "{}", spec.name);
            assert_eq!(tool.get("defer_loading").is_some(), defer, "{}", spec.name);
            assert!(tool.get("cache_control").is_none());
        }
    }
    // What normalising kept, the encoder kept: the `$ref`, the odd names, the pattern's backslash.
    let q = &specs[1].schema;
    assert_eq!(q["properties"]["filter"]["$ref"], json!("#/$defs/Filter"));
    assert_eq!(
        q["$defs"]["Filter"]["properties"]["op"]["enum"][3],
        json!("gt")
    );
    for name in ["x-request-id", "with space", "ünïcode", "a.b", "$ref"] {
        assert!(q["properties"].get(name).is_some(), "{name}");
    }
    assert_eq!(
        specs[0].schema["properties"]["repo"]["pattern"],
        json!("^[\\w.-]+$")
    );
    assert!(
        specs[0].schema.get("title").is_none(),
        "an annotation keyword is dropped"
    );
    assert!(
        specs[0].schema["properties"].get("title").is_some(),
        "a property is not"
    );
    assert_eq!(specs[2].schema, json!({"type": "object"}));
}

#[test]
fn the_openai_compatible_encoder_round_trips_real_shaped_schemas() {
    let specs = specs();
    let sent = wire(&encode_chat(&request(provider_tools(&specs, false))));
    let back = sent["tools"].as_array().unwrap();
    assert_eq!(back.len(), specs.len() + 1);
    for (spec, tool) in specs.iter().zip(&back[1..]) {
        assert_eq!(tool["type"], json!("function"));
        assert_eq!(tool["function"]["name"], json!(spec.name));
        assert_eq!(
            tool["function"]["description"],
            json!(mnemorph_tools::mcp::Native::description_of(spec))
        );
        assert_eq!(tool["function"]["parameters"], spec.schema, "{}", spec.name);
    }
    // A deferred tool has no native form there: left out, reached through the levels instead.
    let sent = wire(&encode_chat(&request(provider_tools(&specs, true))));
    assert_eq!(sent["tools"].as_array().unwrap().len(), 1);
}

#[test]
fn a_reference_is_native_on_anthropic_and_its_text_everywhere_else() {
    let mut req = request(provider_tools(&specs(), true));
    req.messages.push(Message {
        role: Role::Assistant,
        content: vec![Block::ToolUse {
            id: "t1".into(),
            name: "mcp_schema".into(),
            input: json!({"name": "git-hub2__ping"}),
        }],
    });
    req.messages.push(Message {
        role: Role::User,
        content: vec![Block::ToolResult {
            tool_use_id: "t1".into(),
            content: "git-hub2__ping — level 2 text".into(),
            is_error: false,
            cache: false,
            references: vec!["git-hub2__ping".into()],
        }],
    });
    let a = wire(&encode(&req).unwrap());
    assert_eq!(
        a["messages"][2]["content"][0]["content"],
        json!([{"type": "tool_reference", "tool_name": "git-hub2__ping"}])
    );
    let o = wire(&encode_chat(&req));
    let tool_msg = o["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "tool")
        .unwrap();
    assert_eq!(tool_msg["content"], json!("git-hub2__ping — level 2 text"));
    // Every tool deferred is refused before it is sent: the API needs one up front.
    let mut all = request(provider_tools(&specs(), true));
    all.tools.remove(0);
    assert!(encode(&all).is_err());
}

// --- Native deferral, recorded ------------------------------------------------------------------

fn sse(events: &[Value]) -> String {
    events
        .iter()
        .map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap()))
        .collect()
}

fn start() -> Value {
    json!({"type": "message_start", "message": {"id": "msg", "model": "claude-sonnet-5-5", "usage": {"input_tokens": 10, "output_tokens": 1}}})
}

fn tool_use(id: &str, name: &str, input: &Value) -> String {
    sse(&[
        start(),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": input.to_string()}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 5}}),
        json!({"type": "message_stop"}),
    ])
}

fn text(t: &str) -> String {
    sse(&[
        start(),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": t}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 2}}),
        json!({"type": "message_stop"}),
    ])
}

#[tokio::test]
async fn on_anthropic_level_two_is_a_native_deferred_tool_and_the_prefix_never_moves() {
    let w = World::new("cassette");
    let mcp = w.session();
    let script = Arc::new(Scripted::new([
        Reply::ok(&tool_use(
            "t1",
            "mcp_schema",
            &json!({"name": "hub__web_search"}),
        )),
        Reply::ok(&tool_use(
            "t2",
            "hub__web_search",
            &json!({"q": "mnemorph"}),
        )),
        Reply::ok(&text("done")),
    ]));
    let model = Arc::new(AnthropicModel(Anthropic::new(
        script.clone(),
        "https://api.example",
        "sk-test".into(),
        Policy::default(),
    )));
    let mut run = w.run(model, &mcp, "claude-sonnet-5-5");
    assert_eq!(
        ask(&mut run, "search the web").await,
        Ending::Answered("done".into())
    );
    let sent: Vec<Value> = script
        .sent()
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    assert_eq!(sent.len(), 3);
    let tools = sent[0]["tools"].as_array().unwrap();
    let deferred: Vec<&str> = tools
        .iter()
        .filter(|t| t["defer_loading"] == json!(true))
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(deferred, ["hub__issue_find", "hub__web_search"]);
    // One breakpoint on the tools, on the last tool loaded up front — never on a deferred one.
    let cached: Vec<usize> = (0..tools.len())
        .filter(|i| tools[*i].get("cache_control").is_some())
        .collect();
    let last_up_front = tools
        .iter()
        .rposition(|t| t.get("defer_loading").is_none())
        .unwrap();
    assert_eq!(cached, [last_up_front]);
    // Level 2 is the reference alone: the API expands the definition, out of the cached prefix.
    let results = |body: &Value| -> Vec<Value> {
        body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|m| m["content"].as_array().cloned().unwrap_or_default())
            .filter(|b| b["type"] == "tool_result")
            .collect()
    };
    assert_eq!(
        results(&sent[1])[0]["content"],
        json!([{"type": "tool_reference", "tool_name": "hub__web_search"}])
    );
    // The deferred tool is called by its own name, through the scope, the shelf and the gate.
    let answer = results(&sent[2])[1]["content"].as_str().unwrap().to_owned();
    assert!(
        answer.starts_with("<mcp-output server=\"hub\" tool=\"web_search\""),
        "{answer}"
    );
    assert_eq!(w.calls(), ["call web_search"]);
    // The request only grows: the tools and the system prompt are byte-identical throughout.
    for body in &sent[1..] {
        assert_eq!(body["tools"], sent[0]["tools"]);
        assert_eq!(body["system"], sent[0]["system"]);
    }
    // The run's journal keeps the reference: a resumed conversation sends it again.
    let kept = Journal::<Beat>::new(w.root.join("run.jsonl"))
        .read()
        .unwrap()
        .rows;
    assert!(kept.iter().any(|b| matches!(
        b,
        Beat::Result { references, .. } if references == &["hub__web_search".to_owned()]
    )));
    let back = mnemorph_loop::conversation(&kept);
    assert!(back.iter().flat_map(|m| &m.content).any(|b| matches!(
        b,
        Block::ToolResult { references, .. } if references == &["hub__web_search".to_owned()]
    )));
}

#[tokio::test]
async fn elsewhere_the_same_levels_run_client_side() {
    let w = World::new("client-side");
    let mcp = w.session();
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::tool("t1", "mcp_schema", json!({"name": "hub__web_search"})),
        ScriptedModel::text("done"),
    ]));
    let mut run = w.run(model.clone(), &mcp, "claude-sonnet-5-5");
    assert_eq!(
        ask(&mut run, "search the web").await,
        Ending::Answered("done".into())
    );
    let requests = model.requests();
    assert!(requests[0].tools.iter().all(|t| !t.defer));
    assert!(
        !requests[0]
            .tools
            .iter()
            .any(|t| t.name.starts_with("hub__"))
    );
    let level2 = requests[1]
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .find_map(|b| match b {
            Block::ToolResult {
                content,
                references,
                ..
            } => Some((content.clone(), references.clone())),
            _ => None,
        })
        .unwrap();
    assert!(
        level2.0.contains("hub__web_search — read, level 2"),
        "{}",
        level2.0
    );
    assert!(level2.1.is_empty());
}

// --- The stable prefix, from the trusted journal only ----------------------------------------

/// Ten asks by `slime-test`, each calling `tool` on the first `called`: kept as the loop keeps
/// them, chained and witnessed under the state directory.
fn keep_asks(w: &World, tool: &str, called: usize, from: usize) {
    let state = w.state();
    let journal = mcp_journal::journal(&state);
    for i in from..from + 10 {
        let ask = format!("r#{i}");
        let shape = mcp_journal::AskRows {
            ask: &ask,
            creature: "slime-test",
            race: "slime",
            said: "find the issue",
            helped: true,
            ts: 0,
        };
        let touched = if i - from < called {
            vec![Touched {
                kind: Kind::Call,
                server: "hub".into(),
                tool: Some(tool.into()),
                wt: 10,
                outcome: "ok".into(),
                references: Vec::new(),
            }]
        } else {
            Vec::new()
        };
        let mut rows = shape.rows(&[], None, &touched);
        rows.push(shape.ended());
        mcp_journal::keep(&state, &journal, &rows, mcp_journal::SEGMENT).unwrap();
    }
}

fn framed(model: &ScriptedModel) -> Vec<String> {
    model.requests()[0]
        .tools
        .iter()
        .map(|t| t.name.clone())
        .filter(|n| n.starts_with("hub__"))
        .collect()
}

#[tokio::test]
async fn a_habit_from_the_chained_journal_joins_the_prefix_at_the_next_session_only() {
    let w = World::new("habit");
    keep_asks(&w, "hub__issue_find", 7, 0);
    let mcp = w.session();
    let model = Arc::new(ScriptedModel::new([
        ScriptedModel::text("one"),
        ScriptedModel::text("two"),
    ]));
    let mut run = w.run(model.clone(), &mcp, "m");
    assert_eq!(
        ask(&mut run, "find it").await,
        Ending::Answered("one".into())
    );
    assert_eq!(
        framed(&model),
        ["hub__issue_find"],
        "in the stable prefix, up front"
    );
    let promoted = model.requests()[0]
        .tools
        .iter()
        .find(|t| t.name == "hub__issue_find")
        .cloned()
        .unwrap();
    assert!(!promoted.defer);
    assert!(promoted.description.contains("data, never instructions"));
    // Mid-session the journal comes to favour another tool; this session's frames never change.
    keep_asks(&w, "hub__web_search", 10, 100);
    // Even read again from the journal as it now holds, the session's choice stands.
    assert!(!mcp.freeze_habits(mcp_journal::read(mcp.state())));
    assert_eq!(
        ask(&mut run, "find it again").await,
        Ending::Answered("two".into())
    );
    assert_eq!(
        model.requests()[1].tools,
        model.requests()[0].tools,
        "the request only grows"
    );
    let model2 = Arc::new(ScriptedModel::new([ScriptedModel::text("x")]));
    let mut later_run = w.run(model2.clone(), &mcp, "m");
    let _ = ask(&mut later_run, "a commission later in the session").await;
    assert_eq!(
        framed(&model2),
        ["hub__issue_find"],
        "chosen once, at the session's start"
    );
    // The next session chooses from the journal as it is then.
    let next = w.session();
    let model3 = Arc::new(ScriptedModel::new([ScriptedModel::text("x")]));
    let mut run3 = w.run(model3.clone(), &next, "m");
    let _ = ask(&mut run3, "find it").await;
    assert_eq!(framed(&model3), ["hub__web_search"]);
}

#[tokio::test]
async fn an_untrusted_journal_or_one_written_in_the_world_promotes_nothing() {
    let w = World::new("untrusted");
    keep_asks(&w, "hub__issue_find", 10, 0);
    // An edited byte: the chain no longer holds.
    let file = mnemorph_tools::mcp::learned::journal_file(&w.state());
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replacen("\"wt\":10", "\"wt\":11", 1)).unwrap();
    let mcp = w.session();
    let offer = mcp.offer_framed(
        Race::Slime,
        &w.cx(),
        Frame {
            native: false,
            room: 4_000,
        },
    );
    assert!(offer.prefix.is_empty());
    assert!(
        offer.said[0].contains("the journal was not believed at the session's start"),
        "{:?}",
        offer.said
    );
    // A journal planted in the world is never read: only the state directory's counts.
    let w = World::new("in-world");
    let planted = w.world().join(".mnemorph/mcp");
    fs::create_dir_all(&planted).unwrap();
    let rows: String = (0..10)
        .map(|i| {
            format!(
                "{{\"ts\":0,\"ask\":\"r#{i}\",\"kind\":\"call\",\"server\":\"hub\",\"tool\":\"hub__issue_find\",\"creature\":\"slime-test\",\"race\":\"slime\",\"fingerprint\":\"0\",\"terms\":[],\"wt\":1,\"used\":true,\"outcome\":\"ok\",\"helped\":true}}\n"
            )
        })
        .collect();
    fs::write(planted.join("journal.jsonl"), rows).unwrap();
    let mcp = w.session();
    let offer = mcp.offer_framed(
        Race::Slime,
        &w.cx(),
        Frame {
            native: false,
            room: 4_000,
        },
    );
    assert!(offer.prefix.is_empty());
    assert!(offer.said.is_empty(), "{:?}", offer.said);
}
