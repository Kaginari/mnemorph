//! The board's world overview through the binary, on fixtures: its numbers are the CLI's own —
//! `memory status`, `challenge list`, `meters --json` — and a `log.md` write reaches a tab on
//! `WS /ws/board` as `world/changed` within a second, after which the overview reads it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use mnemorph_server::world::Roster;
use mnemorph_server::world::challenge::{Challenges, Opening, route};
use mnemorph_server::world::ledger::{Journal, Timestamp, Usage, UsageRow};
use mnemorph_server::world::verdicts::{VerdictRow, journal_at};
use serde_json::Value;

struct Home {
    root: PathBuf,
    world: PathBuf,
    served: Option<Child>,
}

/// Where the harness keeps one of a world's journals: `<state>/mnemorph/<kind>/<name>-<fnv64a>.jsonl`.
fn state_file(state: &Path, world: &Path, kind: &str) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in world.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = world.file_name().unwrap().to_string_lossy();
    state
        .join("mnemorph")
        .join(kind)
        .join(format!("{name}-{h:016x}.jsonl"))
}

fn usage(session: &str, wt: u64, usd: Option<f64>) -> UsageRow {
    UsageRow {
        ts: Timestamp(1_792_782_420),
        session: session.into(),
        run: "r".into(),
        body: "rimuru".into(),
        race: "rimuru".into(),
        commission: None,
        model: "m".into(),
        provider: "p".into(),
        usage: Usage::default(),
        prefix: 0,
        wt,
        usd,
        envelope: None,
    }
}

impl Home {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-overviewd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let world = root.join("world");
        std::fs::create_dir_all(&world).unwrap();
        let root = root.canonicalize().unwrap();
        let world = world.canonicalize().unwrap();
        Self {
            root,
            world,
            served: None,
        }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    /// Memory, a challenge, a verdict, creatures and spend, written as the harness writes them.
    fn furnish(&self) {
        let w = &self.world;
        let creature = w.join(".mnemorph/memory/creature");
        std::fs::create_dir_all(&creature).unwrap();
        let create = |id: &str, t: u64, note: &str, text: &str| {
            format!(
                r#"{{"id":"{id}","time":{t},"note":"{note}","by":"record","kind":"create","text":"{text}","tags":[],"owner":"slime-a","polarity":"support","salience":0.5,"confidence":0.5,"source":null,"links":[]}}"#
            )
        };
        let lines = [
            // Grounded by a plain record: an unproven event, the writes frozen.
            create("e1", 1, "n1", "done"),
            r#"{"id":"e2","time":2,"note":"n1","by":"record","kind":"status","status":"grounded"}"#
                .to_owned(),
            // Proposed.
            create("e3", 3, "n2", "the lexer owns quoting"),
            // Grounded by the human, then retired by the gate.
            create("e4", 4, "n3", "skip the fixture"),
            r#"{"id":"e5","time":5,"note":"n3","by":"human","kind":"status","status":"grounded"}"#
                .to_owned(),
            r#"{"id":"e6","time":6,"note":"n3","by":"gate","proof":"log.md two","kind":"status","status":"dormant"}"#
                .to_owned(),
            // Grounded by the human.
            create("e7", 7, "n4", "tests live in tests/"),
            r#"{"id":"e8","time":8,"note":"n4","by":"human","kind":"status","status":"grounded"}"#
                .to_owned(),
        ];
        std::fs::write(creature.join("slime-a.jsonl"), lines.join("\n") + "\n").unwrap();
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
        let landing = route(
            &Roster::default(),
            "run-1:2",
            &["crates/mnemorph-tools/src/files.rs".to_owned()],
        );
        let opening = Opening {
            tree: "ab".repeat(32),
            by: "orc-o".to_owned(),
            ..Default::default()
        };
        Challenges::at(&state_file(&self.state(), w, "challenges"))
            .open_at(&landing, &opening)
            .unwrap();
        let gate = journal_at(state_file(&self.state(), w, "gate"));
        for verdict in ["FAIL, handed back", "PASS"] {
            gate.append(&VerdictRow {
                ts: Timestamp(1_792_782_000),
                body: "orc-wire".into(),
                verdict: verdict.into(),
                changed: vec!["a.rs".into(), "b.rs".into()],
                failures: vec![],
                unverified: vec![],
                entry: None,
                build: mnemorph_server::world::ledger::Build::default(),
            })
            .unwrap();
        }
        let ledger = Journal::<UsageRow>::chained(w.join(".mnemorph/instruments/usage.jsonl"));
        for row in [
            usage("s1", 100, Some(0.10)),
            usage("s2", 200, Some(0.25)),
            usage("s2", 50, None),
        ] {
            ledger.append(&row).unwrap();
        }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_mnemorphd"));
        c.args(args)
            .current_dir(&self.root)
            .env_remove("MNEMORPH_BODY")
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.root.join("config"));
        c
    }

    fn run(&self, args: &[&str]) -> String {
        let out = self.cmd(args).output().unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Serve a board on a free port, the world added; log in over the socket: the address, the
    /// cookie and the CSRF token.
    fn serve(&mut self) -> (String, String, String) {
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
        let json: Value = serde_json::from_str(res.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        let csrf = json["csrf"].as_str().unwrap().to_owned();
        (host, cookie, csrf)
    }

    fn overview(&self, host: &str, cookie: &str) -> Value {
        let id = mnemorph_server::world::dimension::world_id(&self.world);
        let res = http(
            host,
            &format!(
                "GET /api/w/{id} HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"
            ),
        );
        assert!(res.starts_with("HTTP/1.1 200"), "{res}");
        serde_json::from_str(res.split("\r\n\r\n").nth(1).unwrap()).unwrap()
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

/// The numbers in `memory status`'s first line: shared, proposed, grounded.
fn standing_line(text: &str) -> (u64, u64, u64) {
    let nums: Vec<u64> = text
        .lines()
        .next()
        .unwrap()
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse().ok())
        .collect();
    assert_eq!(nums.len(), 3, "{text}");
    (nums[0], nums[1], nums[2])
}

#[test]
fn the_overview_counts_what_the_cli_counts() {
    let mut h = Home::new("parity");
    h.furnish();
    let world = h.world.to_str().unwrap().to_owned();
    let text = h.run(&["memory", "status", "--world", &world]);
    let json: Value =
        serde_json::from_str(&h.run(&["memory", "status", "--world", &world, "--json"])).unwrap();
    let challenges = h.run(&["challenge", "list", "--world", &world]);
    let meters: Vec<Value> = h
        .run(&["meters", "--world", &world, "--json"])
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let (host, cookie, _) = h.serve();
    let ov = h.overview(&host, &cookie);

    let mem = &ov["memory"]["value"];
    let (shared, proposed, grounded) = standing_line(&text);
    assert_eq!(
        (
            mem["shared"].as_u64(),
            mem["proposed"].as_u64(),
            mem["grounded"].as_u64()
        ),
        (Some(shared), Some(proposed), Some(grounded)),
        "{text}"
    );
    let rows = json.as_array().unwrap();
    let unproven: usize = rows
        .iter()
        .map(|r| r["unproven"].as_array().unwrap().len())
        .sum();
    let retired = rows
        .iter()
        .flat_map(|r| r["pathologies"].as_array().unwrap())
        .filter(|p| p.as_str().unwrap().starts_with("retired, kept dormant"))
        .count();
    assert_eq!(mem["unproven"].as_u64(), Some(unproven as u64));
    assert_eq!(mem["retired"].as_u64(), Some(retired as u64));
    // The fixtures hold one of each, so the equality says something.
    assert_eq!((unproven, retired, proposed), (1, 1, 1), "{json}");

    let ch = &ov["challenges"]["value"];
    let open = challenges
        .lines()
        .filter(|l| !l.is_empty() && *l != "no challenge holds a landing")
        .count();
    let counted = ch["pending"].as_u64().unwrap()
        + ch["blocked"].as_u64().unwrap()
        + ch["verifying"].as_u64().unwrap();
    assert_eq!(counted, open as u64, "{challenges}");
    assert_eq!(ch["pending"], 1);
    assert_eq!(ov["challenges"]["trusted"], true);

    let shown: Vec<Value> = ov["meters"]["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|ch| serde_json::json!({"creature": ch["creature"], "race": ch["race"], "meters": ch["meters"]}))
        .collect();
    assert_eq!(shown, meters);
    assert_eq!(meters.len(), 2, "{meters:?}");

    assert_eq!(ov["gate"]["value"]["verdicts"], 2);
    assert_eq!(ov["gate"]["value"]["last"]["verdict"], "PASS");
    assert_eq!(ov["gate"]["value"]["last"]["changed"], 2);
    assert_eq!(ov["gate"]["trusted"], true);

    let sp = &ov["spend"]["value"];
    assert_eq!(sp["session"], "s2");
    assert_eq!(sp["calls"], 2);
    assert_eq!(sp["wt"], 250);
    assert_eq!(sp["unpriced"], 1);
    assert!((sp["usd"].as_f64().unwrap() - 0.25).abs() < 1e-9, "{sp}");
    assert_eq!(ov["spend"]["trusted"], false, "the ledger is in the world");
}

/// The upgrade, by hand: the response's head, and the stream to read frames from.
fn upgrade(host: &str, cookie: &str, csrf: &str) -> (String, TcpStream) {
    let mut s = TcpStream::connect(host).unwrap();
    s.write_all(
        format!(
            "GET /ws/board HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: {cookie}\r\n\
             Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Protocol: mnemorph.board.v1, mnemorph.csrf.{csrf}\r\n\r\n"
        )
        .as_bytes(),
    )
    .unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        s.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    (String::from_utf8(head).unwrap(), s)
}

/// One text frame from the server (unmasked), or `None` past the read timeout.
fn frame(s: &mut TcpStream) -> Option<String> {
    let mut hdr = [0u8; 2];
    s.read_exact(&mut hdr).ok()?;
    assert_eq!(hdr[0], 0x81, "a final text frame");
    let len = match hdr[1] & 0x7f {
        126 => {
            let mut l = [0u8; 2];
            s.read_exact(&mut l).ok()?;
            usize::from(u16::from_be_bytes(l))
        }
        n => usize::from(n),
    };
    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload).ok()?;
    Some(String::from_utf8(payload).unwrap())
}

#[test]
fn a_log_write_reaches_the_tab_within_a_second() {
    let mut h = Home::new("watch");
    std::fs::create_dir_all(h.world.join(".mnemorph")).unwrap();
    std::fs::write(h.world.join(".mnemorph/log.md"), "# log\n").unwrap();
    // A session is live in it: a terminal's run lock, held — the watcher then looks fast.
    let journals = h.world.join(".mnemorph/instruments/loop");
    std::fs::create_dir_all(&journals).unwrap();
    let lock = std::fs::File::create(journals.join("r1730000000000-8.lock")).unwrap();
    lock.lock().unwrap();
    let (host, cookie, csrf) = h.serve();
    let before = h.overview(&host, &cookie);

    let (head, mut s) = upgrade(&host, &cookie, &csrf);
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    assert!(
        head.to_ascii_lowercase()
            .contains("sec-websocket-protocol: mnemorph.board.v1\r\n"),
        "{head}"
    );
    // The watcher looks only while a tab listens: give it its first, slow look at the world as it
    // is (it then finds the session and looks fast), then write.
    std::thread::sleep(Duration::from_millis(2600));
    s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let wrote = Instant::now();
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(h.world.join(".mnemorph/log.md"))
        .unwrap();
    writeln!(
        log,
        "\n### [2026-10-30T12:00:00+00:00] rimuru — a new entry"
    )
    .unwrap();
    drop(log);
    let told = frame(&mut s).expect("told within a second");
    assert!(wrote.elapsed() < Duration::from_secs(1));
    let id = mnemorph_server::world::dimension::world_id(&h.world);
    let v: Value = serde_json::from_str(&told).unwrap();
    assert_eq!(v["method"], "world/changed");
    assert_eq!(v["params"]["id"], id);

    let after = h.overview(&host, &cookie);
    assert_eq!(after["activity"]["value"]["what"], "log.md written");
    assert_eq!(after["activity"]["source"], ".mnemorph/log.md");
    assert!(
        after["activity"]["value"]["at"].as_str() >= before["activity"]["value"]["at"].as_str()
    );

    // A wrong token is refused before any upgrade.
    let (head, _) = upgrade(&host, &cookie, &"0".repeat(64));
    assert!(head.starts_with("HTTP/1.1 403"), "{head}");
}

#[test]
fn a_held_lock_reads_tui_live() {
    let mut h = Home::new("live");
    let journals = h.world.join(".mnemorph/instruments/loop");
    std::fs::create_dir_all(&journals).unwrap();
    let lock = std::fs::File::create(journals.join("r1730000000000-9.lock")).unwrap();
    lock.lock().unwrap();
    let (host, cookie, _) = h.serve();
    assert_eq!(h.overview(&host, &cookie)["status"]["value"], "tui-live");
    drop(lock);
    assert_eq!(h.overview(&host, &cookie)["status"]["value"], "idle");
}
