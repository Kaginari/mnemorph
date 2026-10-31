//! The board's metrics and integrations through the binary, on fixtures: the metrics' totals are
//! `mnemorphd usage --json`'s, the meters `meters --json`'s, unpriced never reads $0; no secret
//! value — a provider's key in the environment, an MCP server's token, a key written inline in the
//! global file — reaches any byte of `/api/integrations`; no bench path reads "none configured",
//! and a held-out suite's rows are never read.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use mnemorph_server::world::ledger::{Journal, Timestamp, Usage, UsageRow};
use serde_json::Value;

const FAKE_KEY: &str = "sk-or-v1-FAKEKEYVALUE0123456789abcdef0123456789abcdef";
const FAKE_TOKEN: &str = "ghp_FAKETOKEN16C7e42F292c6912E7710c838347Ae1";
const INLINE: &str = "sk-ant-INLINEFAKESECRET-0123456789abcdefABCDEF";
const PASSWORD: &str = "hunter2-FAKE-PASSWORD-77";
const HELDOUT: &str = "HELDOUT-TASK-NEVER-READ";

struct Home {
    root: PathBuf,
    world: PathBuf,
    served: Option<Child>,
}

fn usage(ts: i64, session: &str, race: &str, wt: u64, usd: Option<f64>) -> UsageRow {
    UsageRow {
        ts: Timestamp(ts),
        session: session.into(),
        run: "r".into(),
        body: format!("{race}-a"),
        race: race.into(),
        commission: None,
        model: if usd.is_some() { "priced" } else { "free" }.into(),
        provider: "p".into(),
        usage: Usage {
            input: wt / 2,
            output: wt / 10,
            cache_read: wt,
            cache_write: 0,
        },
        prefix: 0,
        wt,
        usd,
        envelope: None,
        purpose: None,
    }
}

impl Home {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-metricsd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let world = root.join("world");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::create_dir_all(root.join("config/mnemorph")).unwrap();
        let root = root.canonicalize().unwrap();
        let world = world.canonicalize().unwrap();
        Self {
            root,
            world,
            served: None,
        }
    }

    fn global(&self, text: &str) {
        std::fs::write(self.root.join("config/mnemorph/config.toml"), text).unwrap();
    }

    /// Calls over three days, priced and unpriced, and two creatures.
    fn furnish(&self) {
        let w = &self.world;
        for (seat, doc) in [
            (
                "dark-elf/glim",
                "# glim\n\n- **Race:** dark-elf\n- **Speciality:** law\n",
            ),
            ("slime/slime-a", "# slime-a\n\n- **Race:** slime\n"),
        ] {
            let at = w.join(".mnemorph").join(seat);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("README.md"), doc).unwrap();
        }
        let now = Timestamp::now().0;
        let ledger = Journal::<UsageRow>::chained(w.join(".mnemorph/instruments/usage.jsonl"));
        for row in [
            usage(now - 40 * 86_400, "s0", "orc", 9_000, Some(9.0)),
            usage(now - 3 * 86_400, "s1", "orc", 1_000, Some(0.10)),
            usage(now - 2 * 86_400, "s1", "slime", 2_000, None),
            usage(now - 600, "s2", "slime", 500, None),
            usage(now - 60, "s2", "orc", 300, Some(0.03)),
        ] {
            ledger.append(&row).unwrap();
        }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_mnemorphd"));
        c.args(args)
            .current_dir(&self.root)
            .env_remove("MNEMORPH_BODY")
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("OPENROUTER_API_KEY", FAKE_KEY)
            .env("GITHUB_TOKEN", FAKE_TOKEN)
            .env("DB_PASSWORD", PASSWORD);
        c
    }

    fn run(&self, args: &[&str]) -> String {
        let out = self.cmd(args).output().unwrap();
        assert!(out.status.success(), "{args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Serve a board on a free port, the world added; log in: the address and the cookie.
    fn serve(&mut self) -> (String, String) {
        let added = self.cmd(&["board", "add", "world"]).output().unwrap();
        assert!(added.status.success());
        let mut child = self
            .cmd(&["board", "--port", "0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        self.served = Some(child);
        let (host, key) = line
            .trim()
            .strip_prefix("http://")
            .and_then(|r| r.split_once("/#k="))
            .map(|(h, k)| (h.to_owned(), k.to_owned()))
            .unwrap();
        let body = format!("{{\"key\":\"{key}\"}}");
        let res = http(
            &host,
            &format!(
                "POST /session HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        );
        let cookie = res
            .lines()
            .find_map(|l| l.strip_prefix("set-cookie: "))
            .and_then(|c| c.split(';').next())
            .unwrap()
            .to_owned();
        (host, cookie)
    }

    fn id(&self) -> String {
        mnemorph_server::world::dimension::world_id(&self.world)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        if let Some(mut c) = self.served.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn http(addr: &str, request: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

/// `GET <path>` with the cookie: the whole response, head and body.
fn get(host: &str, cookie: &str, path: &str) -> String {
    http(
        host,
        &format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"
        ),
    )
}

fn json(res: &str) -> Value {
    assert!(res.starts_with("HTTP/1.1 200"), "{res}");
    serde_json::from_str(res.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

/// The CLI's totals and the board's, field by field.
fn same(cli: &Value, board: &Value, what: &str) {
    for k in [
        "calls",
        "wt",
        "input",
        "output",
        "cacheRead",
        "cacheWrite",
        "unpriced",
    ] {
        assert_eq!(cli[k], board[k], "{what}.{k}: {cli} / {board}");
    }
    match (cli["usd"].as_f64(), board["usd"].as_f64()) {
        (Some(a), Some(b)) => assert!((a - b).abs() < 1e-9, "{what}.usd {a} / {b}"),
        (a, b) => assert_eq!(a, b, "{what}.usd"),
    }
}

#[test]
fn the_metrics_total_what_the_cli_totals() {
    let mut h = Home::new("parity");
    h.furnish();
    let world = h.world.to_str().unwrap().to_owned();
    let meters: Vec<Value> = h
        .run(&["meters", "--world", &world, "--json"])
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut cli = Vec::new();
    for window in ["7d", "28d", "all"] {
        let out = h.run(&["usage", "--world", &world, "--window", window, "--json"]);
        cli.push((window, serde_json::from_str::<Value>(&out).unwrap()));
    }
    let text = h.run(&["usage", "--world", &world]);
    assert!(
        text.starts_with("usage 7d: $0.1300 (2 unpriced) · 3800 wt · 4 calls"),
        "{text}"
    );
    assert!(
        text.contains("slime: unpriced (2 calls)"),
        "unpriced never $0: {text}"
    );

    let (host, cookie) = h.serve();
    let id = h.id();
    for (window, c) in &cli {
        let m = json(&get(
            &host,
            &cookie,
            &format!("/api/metrics?world={id}&window={window}"),
        ));
        assert_eq!(m["window"], *window);
        let u = &m["usage"]["value"];
        same(&c["total"], &u["total"], "total");
        for (group, of) in [
            ("races", "races"),
            ("models", "models"),
            ("bodies", "bodies"),
        ] {
            let rows = u[group].as_array().unwrap();
            assert_eq!(rows.len(), c[of].as_object().unwrap().len(), "{group}");
            for r in rows {
                same(&c[of][r["name"].as_str().unwrap()], &r["spent"], group);
            }
        }
        let days = u["days"].as_array().unwrap();
        assert_eq!(days.len(), c["days"].as_object().unwrap().len());
        for d in days {
            same(&c["days"][d["day"].as_str().unwrap()], &d["spent"], "days");
        }
        let sessions = u["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), c["sessions"].as_object().unwrap().len());
        for s in sessions {
            same(
                &c["sessions"][s["session"].as_str().unwrap()],
                &s["spent"],
                "sessions",
            );
            assert_eq!(s["world"], id.as_str());
        }
        // Unpriced never reads $0: the slime's calls were all unpriced.
        let slime = u["races"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == "slime")
            .unwrap();
        assert_eq!(slime["spent"]["usd"], Value::Null, "{slime}");
        assert_eq!(slime["spent"]["unpriced"], 2);
        assert_eq!(m["usage"]["trusted"], false, "the ledger is in the world");
    }
    // The fixtures say something: the windows differ.
    assert_eq!(cli[0].1["total"]["calls"], 4);
    assert_eq!(cli[2].1["total"]["calls"], 5);

    let all = json(&get(&host, &cookie, "/api/metrics?world=all&window=all"));
    same(
        &cli[2].1["total"],
        &all["usage"]["value"]["total"],
        "all worlds",
    );
    // The meters are the CLI's, and a silent one knows since when.
    let shown: Vec<Value> = all["meters"]["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| serde_json::json!({"creature": c["creature"], "race": c["race"], "meters": c["meters"]}))
        .collect();
    assert_eq!(shown, meters);
    let slime = all["meters"]["value"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["creature"] == "slime-a")
        .unwrap();
    assert!(slime["meters"][0]["n"].is_null());
    assert!(slime["firstCall"].is_string(), "{slime}");
    assert_eq!(all["scores"]["trusted"], true);
    // No bench path: none configured.
    assert_eq!(all["bench"]["value"]["configured"], false);
    assert_eq!(all["bench"]["why"], "no bench results configured");

    let bad = get(&host, &cookie, "/api/metrics?window=1y");
    assert!(bad.starts_with("HTTP/1.1 400"), "{bad}");
    let none = get(&host, &cookie, "/api/metrics?world=nope");
    assert!(none.starts_with("HTTP/1.1 404"), "{none}");
}

fn bench_dir(root: &Path) -> PathBuf {
    let dir = root.join("bench/results");
    let row = |task: &str, passed: bool| {
        format!(
            "{{\"task\":\"{task}\",\"mode\":\"solo\",\"model\":\"m\",\"harness\":\"mnemorph\",\"passed\":{passed},\"valid\":true,\"usd\":0.5,\"stderr\":\"{INLINE}\"}}\n"
        )
    };
    std::fs::create_dir_all(dir.join("large/mnemorph")).unwrap();
    std::fs::write(
        dir.join("large/mnemorph/m.jsonl"),
        row("t1", true) + &row("t2", false),
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("heldout-campaign/mnemorph")).unwrap();
    std::fs::write(
        dir.join("heldout-campaign/mnemorph/m.jsonl"),
        row(HELDOUT, true),
    )
    .unwrap();
    dir
}

/// The global file: a provider key in the environment, credentials in a URL, a key written
/// inline, an MCP token in the arguments, and the bench's directory.
fn global_text(bench: &Path) -> String {
    format!(
        r#"[providers.openrouter]
kind = "openai"
base_url = "https://user:{PASSWORD}@openrouter.ai/api/v1"
key_env = "OPENROUTER_API_KEY"
api_key = "{INLINE}"

[providers.inline]
kind = "openai"
base_url = "https://example.com/v1"
key_env = "{INLINE}"

[models.tier]
fast = "openrouter:deepseek/deepseek-v4-flash"
strong = "{INLINE}"

[mcp.servers.gh]
command = "gh-mcp"
args = ["--stdio", "--token", "{FAKE_TOKEN}", "--auth={FAKE_TOKEN}"]
env = ["GITHUB_TOKEN"]
races = ["kijin"]
read_only = true

[board]
bench_results = "{}"
"#,
        bench.display()
    )
}

/// What `/api/integrations` shows of the global file: sources, holes, scope, origins.
fn integrations(i: &Value) {
    let c = &i["config"]["value"];
    let or = c["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "openrouter")
        .unwrap();
    assert_eq!(
        or["key"],
        serde_json::json!({"variable": "OPENROUTER_API_KEY", "state": "set"})
    );
    assert_eq!(or["baseUrl"], Value::Null, "credentials in the URL: hidden");
    let inline = c["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "inline")
        .unwrap();
    assert_eq!(inline["key"]["state"], "hidden");
    assert!(
        c["holes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["key"] == "providers.openrouter.api_key")
    );
    let gh = &c["mcp"]["value"]["servers"][0];
    assert_eq!(gh["races"], serde_json::json!(["kijin"]));
    assert_eq!(gh["class"], "read", "`read_only = true` is the read class");
    assert_eq!(gh["tools"], serde_json::json!([]));
    assert_eq!(gh["args"][0], "--stdio");
    assert_eq!(gh["args"][2], Value::Null);
    assert_eq!(gh["args"][3], Value::Null);
    assert_eq!(gh["env"][0]["state"], "set");
    let bench_key = c["effective"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "board.bench_results")
        .unwrap();
    assert_eq!(bench_key["globalOnly"], true);
    assert_eq!(bench_key["layer"], "global");
    assert!(i["board"]["port"].as_u64().unwrap() > 0);
}

#[test]
fn no_secret_value_reaches_the_integrations_and_held_out_is_never_read() {
    let mut h = Home::new("secrets");
    h.furnish();
    let bench = bench_dir(&h.root);
    h.global(&global_text(&bench));
    // A world's own file says it too: shown with its origin, or a hole.
    std::fs::create_dir_all(h.world.join(".mnemorph")).unwrap();
    std::fs::write(
        h.world.join(".mnemorph/config.local.toml"),
        format!("[providers.evil]\nkey_env = \"{INLINE}\"\n[board]\nbench_results = \"/\"\n"),
    )
    .unwrap();
    let (host, cookie) = h.serve();
    let id = h.id();
    let secrets = [
        FAKE_KEY,
        FAKE_TOKEN,
        INLINE,
        PASSWORD,
        "FAKEKEYVALUE",
        "INLINEFAKESECRET",
    ];
    let mut seen = Vec::new();
    for path in [
        "/api/integrations".to_owned(),
        format!("/api/integrations?world={id}"),
        "/api/metrics?world=all&window=all".to_owned(),
        format!("/api/w/{id}"),
        "/api/worlds".to_owned(),
    ] {
        let res = get(&host, &cookie, &path);
        assert!(res.starts_with("HTTP/1.1 200"), "{path}: {res}");
        for s in secrets {
            assert!(!res.contains(s), "{path} holds {s}: {res}");
        }
        seen.push(res);
    }
    integrations(&json(&seen[0]));
    // The world's local file: its board key and its provider are holes.
    let local = json(&seen[1]);
    let holes = local["config"]["value"]["holes"].as_array().unwrap();
    assert!(holes.iter().any(|h| h["key"] == "board.bench_results"));
    assert!(holes.iter().any(|h| h["key"] == "providers.evil.key_env"));

    // The bench: the configured rows, never the held-out suite's.
    let metrics = json(&seen[2]);
    let bench_rows = &metrics["bench"]["value"];
    assert_eq!(bench_rows["configured"], true);
    let rows = bench_rows["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["suite"], "large");
    assert_eq!(
        (rows[0]["passed"].as_u64(), rows[0]["valid"].as_u64()),
        (Some(1), Some(2))
    );
    assert!(!seen[2].contains(HELDOUT));
    assert!(!seen[2].contains("heldout"));
}

/// Where the harness keeps the world's usage witness.
fn witness(state: &Path, world: &Path) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world.file_name().unwrap().to_string_lossy();
    state.join(format!("mnemorph/usage/{name}-{h:016x}.jsonl"))
}

#[test]
fn the_usage_witness_is_read_and_trusted_over_the_worlds_copy() {
    let mut h = Home::new("witness");
    let at = witness(&h.root.join("state"), &h.world);
    let ledger = Journal::<UsageRow>::chained(h.world.join(".mnemorph/instruments/usage.jsonl"))
        .witnessed_at(&at);
    let now = Timestamp::now().0;
    for wt in [100, 200] {
        ledger
            .append(&usage(now - 60, "s1", "orc", wt, None))
            .unwrap();
    }
    // A body appends to the world's copy: not in the witness, never counted.
    let copy = h.world.join(".mnemorph/instruments/usage.jsonl");
    let mut text = std::fs::read_to_string(&copy).unwrap();
    text.push_str(&serde_json::to_string(&usage(now - 30, "s1", "orc", 9_999, None)).unwrap());
    text.push('\n');
    std::fs::write(&copy, text).unwrap();

    let world = h.world.to_str().unwrap().to_owned();
    let cli: Value = serde_json::from_str(&h.run(&["usage", "--world", &world, "--json"])).unwrap();
    assert_eq!(cli["trusted"], true, "{cli}");
    assert_eq!(cli["total"]["wt"], 300);
    let (host, cookie) = h.serve();
    let id = h.id();
    let m = json(&get(
        &host,
        &cookie,
        &format!("/api/metrics?world={id}&window=all"),
    ));
    assert_eq!(m["usage"]["trusted"], true, "{}", m["usage"]);
    same(&cli["total"], &m["usage"]["value"]["total"], "witness");
}

/// `<world's name>-<fnv64a of its path>`: where a world's MCP state lives under the state directory.
fn world_key(world: &Path) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world.file_name().unwrap().to_string_lossy().into_owned();
    format!("{name}-{h:016x}")
}

#[test]
fn usage_shows_each_mcp_servers_schema_loaded_against_used() {
    let h = Home::new("mcp-usage");
    let state = h.root.join("state/mnemorph/mcp").join(world_key(&h.world));
    std::fs::create_dir_all(&state).unwrap();
    let mut witness = state.file_name().unwrap().to_os_string();
    witness.push(".witness.jsonl");
    let journal = Journal::<Value>::chained(state.join("journal.jsonl"))
        .witnessed_at(state.with_file_name(witness));
    let row = |kind: &str, tool: &str, wt: u64, used: bool| {
        serde_json::json!({
            "ts": 0, "ask": "r#1", "kind": kind, "server": "hub", "tool": tool,
            "creature": "slime-a", "race": "slime", "fingerprint": "0000000000000000",
            "terms": ["find"], "wt": wt, "used": used, "outcome": "ok", "helped": true
        })
    };
    for r in [
        row("schema", "hub__find", 120, true),
        row("schema", "hub__idle", 80, false),
        row("call", "hub__find", 40, true),
    ] {
        journal.append(&r).unwrap();
    }
    let world = h.world.to_str().unwrap().to_owned();
    let out: Value = serde_json::from_str(&h.run(&["usage", "--world", &world, "--json"])).unwrap();
    assert_eq!(out["mcp"]["trusted"], true, "{out}");
    assert_eq!(
        out["mcp"]["servers"]["hub"],
        serde_json::json!({"schemaLoadedWt": 200, "schemaUsedWt": 120, "answersWt": 40})
    );
    let text = h.run(&["usage", "--world", &world]);
    assert!(
        text.contains("  hub: 200 wt of schema loaded, 120 wt of it used; 40 wt of answers"),
        "{text}"
    );
    // An edited row: the journal is not believed, and usage says so rather than a number.
    let file = state.join("journal.jsonl");
    let edited = std::fs::read_to_string(&file)
        .unwrap()
        .replacen("\"wt\":120", "\"wt\":1", 1);
    std::fs::write(&file, edited).unwrap();
    let out: Value = serde_json::from_str(&h.run(&["usage", "--world", &world, "--json"])).unwrap();
    assert_eq!(out["mcp"]["trusted"], false, "{out}");
    assert!(
        h.run(&["usage", "--world", &world])
            .contains("mcp: the journal is not believed")
    );
}
