#![allow(clippy::similar_names, clippy::assert_is_empty)]
//! #48, the last wave, against the fake stdio server: a dead or hung server is a hole naming it
//! within its own timeout, never a hang; restarts are bounded and wait their backoff; a server
//! whose schemas pass the race's prefix ceiling is level 0 only; a habit joins the stable prefix
//! only as chosen once, at the session's start; a deferred tool's level 2 is a reference; a
//! level's price counts against the envelope; and a schema is normalised once.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

use mnemorph_races::{Race, ToolClass};
use mnemorph_tools::mcp::{
    CALL, Frame, Kind, Mcp, OPEN, Restarts, Row, SCHEMA, SEARCH, Server, Settings, Timeouts,
    normalised_schema,
};
use mnemorph_tools::{Context, NoHuman, Outcome, Sandbox, Tool, call};
use serde_json::{Value, json};

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str, tools: &Value) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-mcp-health-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("world")).unwrap();
        std::fs::write(root.join("tools.json"), tools.to_string()).unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    /// The MCP state: under the test's own folder, never the human's state directory.
    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn server(&self, name: &str, races: &[Race], mode: Option<&str>) -> Server {
        let mut s = Server::new(name, "bash", races);
        s.args = vec![
            fixture().display().to_string(),
            self.root.join("tools.json").display().to_string(),
            self.root.join("log").display().to_string(),
        ];
        s.args.extend(mode.map(str::to_owned));
        s.class = Some(ToolClass::Read);
        s
    }

    fn mcp(&self, servers: Vec<Server>, settings: Settings) -> Arc<Mcp> {
        Mcp::new(
            Settings {
                servers,
                ..settings
            },
            self.state(),
        )
    }

    fn cx(&self, race: Race) -> Context {
        Context::new(race, self.world(), &["."])
            .with_sandbox(Sandbox::None("the test runs unguarded".into()))
            .with_unguarded(true)
            .with_gate(Arc::new(NoHuman))
            .with_body("slime-scout")
    }

    fn starts(&self) -> usize {
        std::fs::read_to_string(self.root.join("log"))
            .unwrap_or_default()
            .lines()
            .filter(|l| *l == "start")
            .count()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-mcp.sh")
}

fn tool(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": {"q": {"type": "string"}}}
    })
}

fn few() -> Value {
    json!([
        tool("issue_find", "Find an issue in a tracker"),
        tool("web_search", "Search the web for pages"),
    ])
}

fn door(tools: &[Box<dyn Tool>], name: &str, args: &Value, cx: &mut Context) -> Outcome {
    call(tools, name, &args.to_string(), cx)
}

/// Short waits: each step of a hung server costs this, never more.
fn quick() -> Settings {
    Settings {
        timeouts: Timeouts {
            start: Duration::from_millis(300),
            list: Duration::from_millis(300),
            call: Duration::from_millis(300),
        },
        restarts: Restarts {
            max: 2,
            backoff: Duration::from_millis(50),
        },
        ..Settings::default()
    }
}

#[test]
fn a_hung_server_is_a_hole_naming_it_within_each_steps_timeout() {
    for (mode, step) in [
        ("hang-start", "initialize"),
        ("hang-list", "tools/list"),
        ("hang-call", "tools/call"),
    ] {
        let w = World::new(mode, &few());
        let m = w.mcp(vec![w.server("slow", &[Race::Slime], Some(mode))], quick());
        let mut cx = w.cx(Race::Slime);
        let offer = m.offer(Race::Slime, &cx);
        let began = Instant::now();
        let o = if mode == "hang-call" {
            // Listed from a fresh cache by another server first, so only the call hangs.
            let _ = door(&offer.tools, OPEN, &json!({"server": "slow"}), &mut cx);
            door(
                &offer.tools,
                CALL,
                &json!({"name": "slow__issue_find", "arguments": {}}),
                &mut cx,
            )
        } else {
            door(&offer.tools, OPEN, &json!({"server": "slow"}), &mut cx)
        };
        assert!(o.is_error, "{mode}: {}", o.content);
        assert!(
            o.content.contains(&format!(
                "MCP server `slow` did not answer {step} within 300ms"
            )),
            "{mode}: {}",
            o.content
        );
        assert!(
            began.elapsed() < Duration::from_secs(3),
            "{mode}: a hole at once, never a hang: {:?}",
            began.elapsed()
        );
        assert!(!m.started("slow"), "{mode}: the hung server is dropped");
        // Level 0 says it is down, naming why.
        let line = m.level0_lines(Race::Slime, &cx).join("\n");
        assert!(line.contains("· down: MCP server `slow`"), "{mode}: {line}");
    }
}

#[test]
fn hang_list_needs_the_list_timeout_not_the_call_one() {
    // A server whose start is quick: the list's own wait is the one that ends it.
    let w = World::new("list-wait", &few());
    let mut settings = quick();
    settings.timeouts.call = Duration::from_secs(30);
    let m = w.mcp(
        vec![w.server("slow", &[Race::Slime], Some("hang-list"))],
        settings,
    );
    let mut cx = w.cx(Race::Slime);
    let offer = m.offer(Race::Slime, &cx);
    let began = Instant::now();
    let o = door(&offer.tools, OPEN, &json!({"server": "slow"}), &mut cx);
    assert!(
        o.content.contains("tools/list within 300ms"),
        "{}",
        o.content
    );
    assert!(began.elapsed() < Duration::from_secs(3));
}

#[test]
fn restarts_are_bounded_and_wait_their_backoff() {
    let w = World::new("restarts", &few());
    let m = w.mcp(
        vec![w.server("flaky", &[Race::Slime], Some("die-call"))],
        quick(),
    );
    let mut cx = w.cx(Race::Slime);
    let offer = m.offer(Race::Slime, &cx);
    let call_it = |cx: &mut Context| {
        door(
            &offer.tools,
            CALL,
            &json!({"name": "flaky__issue_find", "arguments": {}}),
            cx,
        )
    };
    let first = call_it(&mut cx);
    assert!(
        first
            .content
            .contains("MCP server `flaky` closed before answering tools/call"),
        "{}",
        first.content
    );
    assert_eq!(w.starts(), 1);
    // At once again: the backoff has not run out, so nothing starts and the hole says when.
    let waiting = call_it(&mut cx);
    assert!(
        waiting
            .content
            .contains("MCP server `flaky` is down: it failed 1 of 3 allowed times"),
        "{}",
        waiting.content
    );
    assert_eq!(w.starts(), 1, "no start inside the backoff");
    // Past each backoff it starts again — twice, its restarts — and then never in this session.
    for _ in 0..6 {
        std::thread::sleep(Duration::from_millis(250));
        let _ = call_it(&mut cx);
    }
    assert_eq!(w.starts(), 3, "one start and two restarts, no more");
    let dead = call_it(&mut cx);
    assert!(
        dead.content.contains(
            "MCP server `flaky` is a hole for this session: it failed 3 times, past its 2 restarts"
        ),
        "{}",
        dead.content
    );
    assert!(m.dead("flaky"));
    let line = m.level0_lines(Race::Slime, &cx).join("\n");
    assert!(line.contains("· a hole this session: "), "{line}");
}

#[test]
fn a_refusal_the_server_answered_keeps_it_running() {
    let w = World::new("refused", &few());
    let m = w.mcp(
        vec![w.server("strict", &[Race::Slime], Some("refuse-call"))],
        quick(),
    );
    let mut cx = w.cx(Race::Slime);
    let offer = m.offer(Race::Slime, &cx);
    for _ in 0..5 {
        let o = door(
            &offer.tools,
            CALL,
            &json!({"name": "strict__issue_find", "arguments": {}}),
            &mut cx,
        );
        assert!(
            o.content.contains("refused tools/call: bad arguments"),
            "{}",
            o.content
        );
    }
    assert!(m.started("strict"), "an answer is no failure");
    assert_eq!(w.starts(), 1);
    assert!(!m.dead("strict"));
}

/// Fourteen tools whose schemas together pass a Slime's 4,000-token prefix ceiling but not a
/// Kijin's 8,000, though each fits under the schema cap.
fn heavy() -> Value {
    let props: serde_json::Map<String, Value> = (0..10)
        .map(|i| {
            (
                format!("field_{i}"),
                json!({"type": "string", "enum": ["alpha", "beta", "gamma", "delta"]}),
            )
        })
        .collect();
    Value::Array(
        (0..14)
            .map(|i| {
                json!({
                    "name": format!("heavy_{i}"),
                    "description": format!("Find heavy issue number {i}"),
                    "inputSchema": {"type": "object", "properties": props.clone()}
                })
            })
            .collect(),
    )
}

#[test]
fn a_server_past_the_prefix_ceiling_is_level_zero_only_with_the_reason_shown() {
    let w = World::new("ceiling", &heavy());
    let m = w.mcp(
        vec![w.server("big", &[Race::Slime, Race::Kijin], None)],
        Settings::default(),
    );
    let mut cx = w.cx(Race::Slime);
    let offer = m.offer(Race::Slime, &cx);
    let o = door(&offer.tools, OPEN, &json!({"server": "big"}), &mut cx);
    assert!(o.is_error, "{}", o.content);
    assert!(
        o.content
            .contains("MCP server `big` is level 0 only for a slime")
            && o.content
                .contains("past the slime prefix ceiling of 4000 wt"),
        "{}",
        o.content
    );
    let line = m.level0_lines(Race::Slime, &cx).join("\n");
    assert!(line.contains("· level 0 only: MCP server `big`"), "{line}");
    // Never ranked, searched, loaded or called.
    assert!(m.ranked(Race::Slime, "find heavy issue", &cx).is_empty());
    let found = door(
        &offer.tools,
        SEARCH,
        &json!({"query": "heavy issue"}),
        &mut cx,
    );
    assert!(!found.content.contains("big__heavy"), "{}", found.content);
    for (d, args) in [
        (SCHEMA, json!({"name": "big__heavy_1"})),
        (CALL, json!({"name": "big__heavy_1", "arguments": {}})),
    ] {
        let o = door(&offer.tools, d, &args, &mut cx);
        assert!(o.content.contains("level 0 only"), "{d}: {}", o.content);
    }
    // A Kijin's ceiling is higher: the same server opens for it.
    let mut kx = w.cx(Race::Kijin);
    let offer = m.offer(Race::Kijin, &kx);
    let o = door(&offer.tools, OPEN, &json!({"server": "big"}), &mut kx);
    assert!(!o.is_error, "{}", o.content);
}

/// Rows that say `creature` called `tool` on `called` of its `asks` asks.
fn habit_rows(creature: &str, tool: &str, asks: usize, called: usize) -> Vec<Row> {
    let row = |ask: usize, kind: Kind, tool: Option<&str>| Row {
        ts: 0,
        ask: format!("r#{ask}"),
        kind,
        server: "hub".into(),
        tool: tool.map(str::to_owned),
        creature: creature.into(),
        race: "slime".into(),
        fingerprint: "0".repeat(16),
        terms: vec!["find".into(), "issue".into()],
        wt: 10,
        used: true,
        outcome: if kind == Kind::Ask { "ended" } else { "ok" }.into(),
        helped: true,
        why: None,
    };
    (0..asks)
        .flat_map(|i| {
            let mut rows = Vec::new();
            if i < called {
                rows.push(row(i, Kind::Call, Some(tool)));
            }
            rows.push(row(i, Kind::Ask, None));
            rows
        })
        .collect()
}

fn names(tools: &[Box<dyn Tool>]) -> Vec<(&'static str, bool)> {
    tools.iter().map(|t| (t.name(), t.defer())).collect()
}

/// A hub of two tools, its catalog listed before the session: what a fresh cache gives.
fn listed_hub(w: &World) -> Arc<Mcp> {
    let m = w.mcp(
        vec![w.server("hub", &[Race::Slime], None)],
        Settings::default(),
    );
    let mut cx = w.cx(Race::Slime);
    let offer = m.offer(Race::Slime, &cx);
    let _ = door(&offer.tools, OPEN, &json!({"server": "hub"}), &mut cx);
    m
}

#[test]
fn a_habit_joins_the_stable_prefix_only_as_chosen_at_the_sessions_start() {
    let w = World::new("prefix", &few());
    let cx = w.cx(Race::Slime);
    let room = Frame {
        native: false,
        room: 1_000,
    };
    // Called on six of the last ten asks: past 60%.
    let m = listed_hub(&w);
    assert!(m.freeze_habits(Ok(habit_rows("slime-scout", "hub__issue_find", 10, 6))));
    let offer = m.offer_framed(Race::Slime, &cx, room);
    assert_eq!(offer.prefix, ["hub__issue_find"]);
    assert!(
        names(&offer.tools).contains(&("hub__issue_find", false)),
        "{:?}",
        names(&offer.tools)
    );
    assert!(
        offer.said[0].starts_with("hub__issue_find joins slime-scout's stable prefix"),
        "{:?}",
        offer.said
    );
    // Mid-session the journal changes; the choice does not: the request only grows.
    assert!(!m.freeze_habits(Ok(habit_rows("slime-scout", "hub__web_search", 10, 10))));
    let again = m.offer_framed(Race::Slime, &cx, room);
    assert_eq!(again.prefix, ["hub__issue_find"], "never mid-session");
    // The next session chooses afresh.
    let next = listed_hub(&w);
    next.freeze_habits(Ok(habit_rows("slime-scout", "hub__web_search", 10, 10)));
    assert_eq!(
        next.offer_framed(Race::Slime, &cx, room).prefix,
        ["hub__web_search"]
    );
    // Five of ten, or fewer than ten asks journaled: no habit.
    for rows in [
        habit_rows("slime-scout", "hub__issue_find", 10, 5),
        habit_rows("slime-scout", "hub__issue_find", 9, 9),
        habit_rows("another", "hub__issue_find", 10, 10),
    ] {
        let m = listed_hub(&w);
        m.freeze_habits(Ok(rows));
        assert!(m.offer_framed(Race::Slime, &cx, room).prefix.is_empty());
    }
    // A journal not believed promotes nothing, and says why.
    let m = listed_hub(&w);
    m.freeze_habits(Err("the MCP journal's head is not its witness's".into()));
    let offer = m.offer_framed(Race::Slime, &cx, room);
    assert!(offer.prefix.is_empty());
    assert!(
        offer.said[0].contains("the journal was not believed at the session's start"),
        "{:?}",
        offer.said
    );
    // Not chosen at all — no session start read it — promotes nothing either.
    let m = listed_hub(&w);
    assert!(m.offer_framed(Race::Slime, &cx, room).prefix.is_empty());
}

#[test]
fn a_habit_that_does_not_fit_the_ceilings_room_stays_out() {
    let w = World::new("room", &few());
    let cx = w.cx(Race::Slime);
    let m = listed_hub(&w);
    m.freeze_habits(Ok(habit_rows("slime-scout", "hub__issue_find", 10, 10)));
    let offer = m.offer_framed(
        Race::Slime,
        &cx,
        Frame {
            native: false,
            room: 10,
        },
    );
    assert!(offer.prefix.is_empty());
    assert!(
        offer.said[0].contains("passes the 10 wt the prefix ceiling leaves"),
        "{:?}",
        offer.said
    );
}

#[test]
fn a_promoted_tool_is_called_by_its_own_name_through_the_scope_and_gate() {
    let w = World::new("promoted-call", &few());
    let mut cx = w.cx(Race::Slime);
    let m = listed_hub(&w);
    m.freeze_habits(Ok(habit_rows("slime-scout", "hub__issue_find", 10, 10)));
    let offer = m.offer_framed(
        Race::Slime,
        &cx,
        Frame {
            native: false,
            room: 1_000,
        },
    );
    let o = door(&offer.tools, "hub__issue_find", &json!({"q": "x"}), &mut cx);
    assert!(!o.is_error, "{}", o.content);
    assert!(
        o.content
            .starts_with("<mcp-output server=\"hub\" tool=\"issue_find\"")
    );
    assert_eq!(o.meta.mcp_touched.map(|t| t.kind), Some(Kind::Call));
}

#[test]
fn on_a_native_provider_the_rest_of_the_scope_is_deferred_and_level_two_is_a_reference() {
    let w = World::new("native", &few());
    let mut cx = w.cx(Race::Slime);
    let m = listed_hub(&w);
    m.freeze_habits(Ok(habit_rows("slime-scout", "hub__issue_find", 10, 10)));
    let offer = m.offer_framed(
        Race::Slime,
        &cx,
        Frame {
            native: true,
            room: 1_000,
        },
    );
    let framed = names(&offer.tools);
    assert!(framed.contains(&("hub__issue_find", false)), "{framed:?}");
    assert!(framed.contains(&("hub__web_search", true)), "{framed:?}");
    let level2 = door(
        &offer.tools,
        SCHEMA,
        &json!({"name": "hub__web_search"}),
        &mut cx,
    );
    assert_eq!(level2.meta.references(), ["hub__web_search"]);
    assert!(
        level2.content.contains("level 2"),
        "the text stays for any other provider"
    );
    // Client-side, the same door loads text and references nothing.
    let plain = m.offer(Race::Slime, &cx);
    let level2 = door(
        &plain.tools,
        SCHEMA,
        &json!({"name": "hub__web_search"}),
        &mut cx,
    );
    assert!(level2.meta.references().is_empty());
    assert!(plain.tools.iter().all(|t| !t.defer()));
}

#[test]
fn a_levels_price_counts_against_the_envelope() {
    let w = World::new("envelope", &few());
    let m = listed_hub(&w);
    let left = Arc::new(AtomicU64::new(5));
    let mut cx = w.cx(Race::Slime).with_budget_left(Arc::clone(&left));
    let offer = m.offer(Race::Slime, &cx);
    let o = door(&offer.tools, OPEN, &json!({"server": "hub"}), &mut cx);
    assert!(
        o.is_error
            && o.content
                .contains("your envelope has 5 wt left; nothing was loaded"),
        "{}",
        o.content
    );
    assert!(
        o.meta.mcp_touched.is_none(),
        "nothing loaded, nothing journaled"
    );
    let o = door(
        &offer.tools,
        SCHEMA,
        &json!({"name": "hub__issue_find"}),
        &mut cx,
    );
    assert!(o.is_error, "{}", o.content);
    left.store(u64::MAX, std::sync::atomic::Ordering::Relaxed);
    let o = door(&offer.tools, OPEN, &json!({"server": "hub"}), &mut cx);
    assert!(!o.is_error, "{}", o.content);
}

#[test]
fn a_schema_is_normalised_once_and_a_propertys_own_name_is_never_touched() {
    let raw = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "x",
        "title": "Args",
        "type": "object",
        "description": "   padded   ",
        "properties": {
            "title": {"type": "string", "title": "The title", "examples": ["a"], "description": "  t  "},
            "nested": {"type": "object", "properties": {
                "deep": {"type": "array", "items": {"$comment": "c", "type": "string", "description": "d".repeat(900)}}
            }},
            "pick": {"anyOf": [{"$ref": "#/$defs/Mode", "title": "M"}, {"type": "null"}]},
            "empty": {"type": "string", "description": "   "}
        },
        "$defs": {"Mode": {"type": "string", "enum": ["a", "b"], "example": "a"}},
        "required": ["title"]
    });
    let n = normalised_schema(Some(&raw));
    assert_eq!(
        n,
        json!({
            "$id": "x",
            "type": "object",
            "description": "padded",
            "properties": {
                "title": {"type": "string", "description": "t"},
                "nested": {"type": "object", "properties": {
                    "deep": {"type": "array", "items": {"type": "string", "description": "d".repeat(512)}}
                }},
                "pick": {"anyOf": [{"$ref": "#/$defs/Mode"}, {"type": "null"}]},
                "empty": {"type": "string"}
            },
            "$defs": {"Mode": {"type": "string", "enum": ["a", "b"]}},
            "required": ["title"]
        })
    );
    assert_eq!(
        normalised_schema(Some(&n)),
        n,
        "normalising twice changes nothing"
    );
    assert_eq!(normalised_schema(None), json!({"type": "object"}));
}

#[test]
fn a_refused_listing_keeps_the_server_running() {
    let w = World::new("refused-list", &few());
    let m = w.mcp(
        vec![w.server("shy", &[Race::Slime], Some("refuse-list"))],
        quick(),
    );
    let mut cx = w.cx(Race::Slime);
    let offer = m.offer(Race::Slime, &cx);
    for _ in 0..5 {
        let o = door(&offer.tools, OPEN, &json!({"server": "shy"}), &mut cx);
        assert!(
            o.content.contains("refused tools/list: not now"),
            "{}",
            o.content
        );
    }
    assert!(
        m.started("shy") && !m.dead("shy"),
        "an answer is no failure"
    );
    assert_eq!(w.starts(), 1);
}

#[test]
fn a_promoted_write_tool_still_asks_the_human_and_a_denial_runs_nothing() {
    let w = World::new("promoted-gate", &few());
    let mut hub = w.server("hub", &[Race::Slime], None);
    hub.class = Some(ToolClass::Write);
    let m = w.mcp(vec![hub], Settings::default());
    // NoHuman denies every act that asks.
    let mut cx = w.cx(Race::Slime);
    let offer = m.offer(Race::Slime, &cx);
    let _ = door(&offer.tools, OPEN, &json!({"server": "hub"}), &mut cx);
    m.freeze_habits(Ok(habit_rows("slime-scout", "hub__issue_find", 10, 10)));
    let offer = m.offer_framed(
        Race::Slime,
        &cx,
        Frame {
            native: false,
            room: 1_000,
        },
    );
    assert_eq!(offer.prefix, ["hub__issue_find"]);
    let o = door(&offer.tools, "hub__issue_find", &json!({"q": "x"}), &mut cx);
    assert!(o.meta.denied, "{}", o.content);
    let log = std::fs::read_to_string(w.root.join("log")).unwrap_or_default();
    assert!(
        !log.contains("call issue_find"),
        "the server never saw it: {log}"
    );
}

#[test]
fn a_promoted_tool_off_the_shelf_is_refused_by_its_own_name() {
    // Promoted for a Kijin's frame; a Slime's context calling it is held to its own shelf.
    let w = World::new("promoted-shelf", &few());
    let mut hub = w.server("hub", &[Race::Kijin, Race::Slime], None);
    hub.class = Some(ToolClass::Outward);
    let m = w.mcp(vec![hub], Settings::default());
    let mut kx = w.cx(Race::Kijin);
    let offer = m.offer(Race::Kijin, &kx);
    let _ = door(&offer.tools, OPEN, &json!({"server": "hub"}), &mut kx);
    m.freeze_habits(Ok(habit_rows("slime-scout", "hub__issue_find", 10, 10)));
    let offer = m.offer_framed(
        Race::Kijin,
        &kx,
        Frame {
            native: false,
            room: 1_000,
        },
    );
    assert_eq!(offer.prefix, ["hub__issue_find"]);
    let mut sx = w.cx(Race::Slime);
    let o = door(&offer.tools, "hub__issue_find", &json!({"q": "x"}), &mut sx);
    assert!(o.is_error, "{}", o.content);
    let log = std::fs::read_to_string(w.root.join("log")).unwrap_or_default();
    assert!(!log.contains("call issue_find"), "{log}");
}
