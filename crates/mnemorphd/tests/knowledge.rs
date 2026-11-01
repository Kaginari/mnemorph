//! The board's memory and ontology pages through the binary, on fixtures: their counts are the
//! CLI's own — `memory status --json`, `onto check` — the statuses follow the facts table's
//! partition, and the human's acts (confirm, wake, settle) write the CLI's events, journaled as the
//! human's from the board, behind the CSRF token, with a settle refused without its proof.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use mnemorph_server::world::ledger::Timestamp;
use mnemorph_server::world::verdicts::{VerdictRow, journal_at};
use serde_json::{Value, json};

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

fn create(id: &str, t: u64, note: &str, owner: &str, text: &str, extra: &str) -> String {
    format!(
        r#"{{"id":"{id}","time":{t},"note":"{note}","by":"record","kind":"create","text":"{text}","tags":[],"owner":"{owner}","polarity":"support","salience":0.5,"confidence":0.5,"source":null,"links":[]{extra}}}"#
    )
}

fn support(id: &str, about: &str, text: &str, by: &str, at: Option<&str>) -> Value {
    let mut e = json!({"event": "support", "id": id, "about": about, "text": text, "by": by});
    if let Some(at) = at {
        e["source"] = json!({"at": at});
    }
    e
}

impl Home {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-knowledged-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let world = root.join("world");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
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

    /// Two creatures' memory: an unproven write, a proposed claim, a retirement, a human's
    /// grounding; a claim resting on a file that moved, and one waiting to be confirmed.
    fn memory(&self) {
        let creature = self.world.join(".mnemorph/memory/creature");
        std::fs::create_dir_all(&creature).unwrap();
        let a = [
            create("e1", 1, "n1", "slime-a", "done", ""),
            r#"{"id":"e2","time":2,"note":"n1","by":"record","kind":"status","status":"grounded"}"#
                .to_owned(),
            create("e3", 3, "n2", "slime-a", "the lexer owns quoting", ""),
            create("e4", 4, "n3", "slime-a", "skip the fixture", ""),
            r#"{"id":"e5","time":5,"note":"n3","by":"human","kind":"status","status":"grounded"}"#
                .to_owned(),
            r#"{"id":"e6","time":6,"note":"n3","by":"gate","proof":"log.md two","kind":"status","status":"dormant"}"#
                .to_owned(),
            create("e7", 7, "n4", "slime-a", "tests live in tests/", ""),
            r#"{"id":"e8","time":8,"note":"n4","by":"human","kind":"status","status":"grounded"}"#
                .to_owned(),
        ];
        std::fs::write(creature.join("slime-a.jsonl"), a.join("\n") + "\n").unwrap();
        std::fs::create_dir_all(self.world.join("src")).unwrap();
        std::fs::write(self.world.join("src/a.rs"), "fn a() {}\n").unwrap();
        let b = [
            create(
                "e9",
                9,
                "n6",
                "slime-b",
                "a() is pure",
                r#","depends_on":[{"on":"file","path":"src/a.rs","hash":"0000000000000000"}]"#,
            ),
            create(
                "e10",
                10,
                "n7",
                "slime-b",
                "<img src=x onerror=alert(1)>",
                "",
            ),
        ];
        std::fs::write(creature.join("slime-b.jsonl"), b.join("\n") + "\n").unwrap();
        for (seat, doc) in [
            (
                "slime/slime-a",
                "# slime-a\n\n- **Race:** slime\n- **Territory:** `src/`\n",
            ),
            ("slime/slime-b", "# slime-b\n\n- **Race:** slime\n"),
        ] {
            let at = self.world.join(".mnemorph").join(seat);
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("README.md"), doc).unwrap();
        }
    }

    /// One fact of each status, and the gate's verdicts: a failure and a pass on `src/a.rs`.
    fn ontology(&self) {
        let events = [
            support(
                "fsingle",
                "src/",
                "src holds the code",
                "slime-a",
                Some("src/a.rs:1"),
            ),
            support("fagreed", "src/a.rs", "a is small", "slime-a", None),
            support("fagreed", "src/a.rs", "a is small", "slime-b", None),
            support(
                "fcontest",
                "src/a.rs",
                "a is generated",
                "slime-a",
                Some("src/a.rs:1"),
            ),
            json!({"event": "counter", "id": "fcontest", "by": "slime-b", "reason": "<b>it is hand-written</b>"}),
            support("fcounter", "src/", "src is empty", "slime-b", None),
            json!({"event": "counter", "id": "fcounter", "by": "slime-b", "reason": "it is not"}),
            support("fsettled", "src/a.rs", "a builds", "slime-a", None),
            json!({"event": "settle", "id": "fsettled", "by": "orc-o", "proof": "orc-o's verdict: settle #fsettled"}),
        ];
        // The trusted facts, as the harness keeps them under the state directory: the world's own
        // copy is never read for them.
        let world = self.world.canonicalize().unwrap();
        let trusted = mnemorph_server::world::ledger::Journal::<Value>::chained(state_file(
            &self.state(),
            &world,
            "facts",
        ));
        for e in &events {
            trusted.append(e).unwrap();
        }
        let gate = journal_at(state_file(&self.state(), &self.world, "gate"));
        for (verdict, changed) in [("FAIL, handed back", "src/a.rs"), ("PASS", "src/a.rs")] {
            gate.append(&VerdictRow {
                ts: Timestamp(1_792_782_000),
                body: "slime-a".into(),
                verdict: verdict.into(),
                changed: vec![changed.into()],
                failures: vec![],
                unverified: vec![],
                entry: None,
                build: mnemorph_server::world::ledger::Build::default(),
            })
            .unwrap();
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

    /// Serve a board on a free port, the world added; log in: the address, the cookie and the
    /// CSRF token.
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
        let res = post(
            &host,
            "/session",
            "",
            None,
            &format!("{{\"key\":\"{key}\"}}"),
        );
        let cookie = res
            .lines()
            .find_map(|l| l.strip_prefix("set-cookie: "))
            .and_then(|c| c.split(';').next())
            .unwrap()
            .to_owned();
        let csrf = body(&res)["csrf"].as_str().unwrap().to_owned();
        (host, cookie, csrf)
    }

    fn id(&self) -> String {
        mnemorph_server::world::dimension::world_id(&self.world)
    }

    fn get(&self, host: &str, cookie: &str, path: &str) -> Value {
        let res = http(
            host,
            &format!(
                "GET /api/w/{}/{path} HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n",
                self.id()
            ),
        );
        assert!(res.starts_with("HTTP/1.1 200"), "{res}");
        body(&res)
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

/// `POST path body` with the cookie, and the CSRF token when given.
fn post(host: &str, path: &str, cookie: &str, csrf: Option<&str>, body: &str) -> String {
    let csrf = csrf.map_or_else(String::new, |t| format!("x-mnemorph-csrf: {t}\r\n"));
    let cookie = if cookie.is_empty() {
        String::new()
    } else {
        format!("Cookie: {cookie}\r\n")
    };
    http(
        host,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\n{cookie}{csrf}\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
}

/// The fact `id` of a page.
fn fact<'a>(page: &'a Value, id: &str) -> &'a Value {
    page["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == id)
        .unwrap_or_else(|| panic!("no fact {id} in {page}"))
}

/// `onto check`'s head line and its findings: severity, code, where.
fn check_lines(check: &str) -> (&str, Vec<(String, String, String)>) {
    let mut lines = check.lines();
    let head = lines.next().unwrap();
    let found = lines
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[0].to_owned(), f[1].to_owned(), f[2].to_owned())
        })
        .collect();
    (head, found)
}

/// A count of an answer, as a length.
fn size(v: &Value) -> usize {
    usize::try_from(v.as_u64().unwrap()).unwrap()
}

fn body(res: &str) -> Value {
    serde_json::from_str(res.split("\r\n\r\n").nth(1).unwrap()).unwrap()
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

/// The note `id` of a page.
fn note<'a>(page: &'a Value, id: &str, scope: &str) -> &'a Value {
    page["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id && n["scope"] == scope)
        .unwrap_or_else(|| panic!("no {scope} note {id} in {page}"))
}

/// The events of a tier's file, as JSON.
fn events(world: &Path, rel: &str) -> Vec<Value> {
    std::fs::read_to_string(world.join(rel))
        .unwrap_or_default()
        .lines()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            // A chained row's links are the chain's, not the event's.
            if let Some(o) = v.as_object_mut() {
                for k in ["prev", "genesis", "torn", "digest"] {
                    o.remove(k);
                }
            }
            v
        })
        .collect()
}

#[test]
fn the_memory_page_counts_what_the_cli_counts() {
    let mut h = Home::new("parity");
    h.memory();
    let world = h.world.to_str().unwrap().to_owned();
    let text = h.run(&["memory", "status", "--world", &world]);
    let json: Value =
        serde_json::from_str(&h.run(&["memory", "status", "--world", &world, "--json"])).unwrap();
    let (host, cookie, _) = h.serve();
    let page = h.get(&host, &cookie, "memory");

    let (shared, proposed, grounded) = standing_line(&text);
    let s = &page["standing"];
    assert_eq!(
        (
            s["shared"].as_u64(),
            s["proposed"].as_u64(),
            s["grounded"].as_u64()
        ),
        (Some(shared), Some(proposed), Some(grounded)),
        "{text}"
    );
    let cli = json.as_array().unwrap();
    let board = page["creatures"].as_array().unwrap();
    assert_eq!(
        cli.iter().map(|r| &r["creature"]).collect::<Vec<_>>(),
        board.iter().map(|r| &r["creature"]).collect::<Vec<_>>()
    );
    for (c, b) in cli.iter().zip(board) {
        assert_eq!(c["frozen"], b["frozen"], "{c} {b}");
        let ids = |v: &Value, k: &str| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|e| e[k].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&c["unproven"], "event"), ids(&b["events"], "event"));
        assert_eq!(
            size(&b["unproven"]),
            c["unproven"].as_array().unwrap().len()
        );
        assert_eq!(
            size(&b["proposed"]),
            c["proposed"].as_array().unwrap().len()
        );
        // The CLI says the traces' findings, each retirement and each review as lines of one list.
        let lines = c["pathologies"].as_array().unwrap();
        let retired = lines
            .iter()
            .filter(|p| p.as_str().unwrap().starts_with("retired, kept dormant"))
            .count();
        let review = lines
            .iter()
            .filter(|p| p.as_str().unwrap().starts_with("needs review"))
            .count();
        assert_eq!(size(&b["retired"]), retired);
        assert_eq!(size(&b["review"]), review);
        assert_eq!(
            b["pathologies"].as_array().unwrap().len() + retired + review,
            lines.len()
        );
    }
    // The fixtures hold one of each, so the equality says something.
    let unproven: u64 = board.iter().map(|b| b["unproven"].as_u64().unwrap()).sum();
    let retired: u64 = board.iter().map(|b| b["retired"].as_u64().unwrap()).sum();
    let review: u64 = board.iter().map(|b| b["review"].as_u64().unwrap()).sum();
    assert_eq!(
        (unproven, retired, review, proposed),
        (1, 1, 1, 3),
        "{page}"
    );
    assert_eq!(s["unproven"], 1);
    assert_eq!(s["retired"], 1);

    // Each note with its standing, route and graph state.
    let n3 = note(&page, "n3", "personal");
    assert_eq!(n3["standing"], "retired");
    assert_eq!(n3["wake"], true);
    assert_eq!(n3["confirm"], false);
    let n6 = note(&page, "n6", "personal");
    assert_eq!(n6["standing"], "proposed");
    assert_eq!(n6["depends"][0], json!({"on": "file", "what": "src/a.rs"}));
    assert!(n6["review"].as_str().unwrap().contains("src/a.rs"), "{n6}");
    let n7 = note(&page, "n7", "personal");
    assert_eq!(n7["confirm"], true);
    assert_eq!(n7["text"], "<img src=x onerror=alert(1)>", "kept as text");
    assert_eq!(page["total"], 6);

    // Filtered and paged.
    let only = h.get(
        &host,
        &cookie,
        "memory?creature=slime-b&standing=proposed&limit=1",
    );
    assert_eq!(only["total"], 2);
    assert_eq!(only["notes"].as_array().unwrap().len(), 1);
    let retired = h.get(&host, &cookie, "memory?standing=retired");
    assert_eq!(retired["total"], 1);
}

#[test]
fn confirm_and_wake_write_the_clis_events_behind_csrf() {
    let mut h = Home::new("acts");
    h.memory();
    // A twin claim the CLI confirms: the board's events must have its shape.
    let twin = create("e11", 11, "n8", "slime-b", "b is pure", "");
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(h.world.join(".mnemorph/memory/creature/slime-b.jsonl"))
        .unwrap();
    writeln!(f, "{twin}").unwrap();
    drop(f);
    let world = h.world.to_str().unwrap().to_owned();
    let cli = h
        .cmd(&["memory", "confirm", "n8", "--world", &world])
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let (host, cookie, csrf) = h.serve();
    let page = h.get(&host, &cookie, "memory");
    let seen = note(&page, "n7", "personal")["seen"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = format!("/api/w/{}/memory/n7/confirm", h.id());
    let body = format!("{{\"seen\":\"{seen}\"}}");

    // No CSRF token, or a wrong one: refused before any write.
    let before = events(&h.world, ".mnemorph/memory/creature/slime-b.jsonl").len();
    let res = post(&host, &path, &cookie, None, &body);
    assert!(res.starts_with("HTTP/1.1 403"), "{res}");
    let res = post(&host, &path, &cookie, Some(&"0".repeat(64)), &body);
    assert!(res.starts_with("HTTP/1.1 403"), "{res}");
    // A digest of another text: the note changed since it was read.
    let res = post(
        &host,
        &path,
        &cookie,
        Some(&csrf),
        &format!("{{\"seen\":\"{}\"}}", "a".repeat(64)),
    );
    assert!(res.starts_with("HTTP/1.1 409"), "{res}");
    // A note that is no proposed claim: nothing to confirm.
    let res = post(
        &host,
        &format!("/api/w/{}/memory/n4/confirm", h.id()),
        &cookie,
        Some(&csrf),
        &format!(
            "{{\"seen\":\"{}\"}}",
            note(&page, "n4", "personal")["seen"].as_str().unwrap()
        ),
    );
    assert!(res.starts_with("HTTP/1.1 409"), "{res}");
    assert_eq!(
        events(&h.world, ".mnemorph/memory/creature/slime-b.jsonl").len(),
        before
    );

    let res = post(&host, &path, &cookie, Some(&csrf), &body);
    assert!(res.starts_with("HTTP/1.1 200"), "{res}");
    assert_eq!(
        self::body(&res)["done"],
        "confirmed n7: slime-b's claim, grounded and shared"
    );
    let tier = events(&h.world, ".mnemorph/memory/creature/slime-b.jsonl");
    let shape = |note: &str| -> Vec<(Value, Value, String)> {
        tier.iter()
            .chain(events(&h.world, ".mnemorph/memory/shared/notes.jsonl").iter())
            .filter(|e| e["note"] == note && e["by"] == "route")
            .map(|e| {
                (
                    e["by"].clone(),
                    e["kind"].clone(),
                    e["proof"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    };
    let board = shape("n7");
    let by_cli = shape("n8");
    assert_eq!(board.len(), by_cli.len(), "{board:?} {by_cli:?}");
    assert_ne!(board.len(), 0);
    for ((b_by, b_kind, b_proof), (c_by, c_kind, c_proof)) in board.iter().zip(&by_cli) {
        assert_eq!((b_by, b_kind), (c_by, c_kind));
        assert_eq!(c_proof, "confirmed: the human's word");
        assert_eq!(b_proof, "confirmed: the human's word (board)");
    }
    let after = h.get(&host, &cookie, "memory");
    assert_eq!(note(&after, "n7", "personal")["standing"], "grounded");
    assert_eq!(note(&after, "n7", "shared")["route"], "confirmed");
}

#[test]
fn wake_undoes_a_retirement_as_the_human_from_the_board() {
    let mut h = Home::new("wake");
    h.memory();
    let (host, cookie, csrf) = h.serve();
    let after = h.get(&host, &cookie, "memory");
    // Wake: the gate's retirement undone, cited as the human's from the board.
    let seen = note(&after, "n3", "personal")["seen"]
        .as_str()
        .unwrap()
        .to_owned();
    let res = post(
        &host,
        &format!("/api/w/{}/memory/n3/wake", h.id()),
        &cookie,
        Some(&csrf),
        &format!("{{\"seen\":\"{seen}\"}}"),
    );
    assert!(res.starts_with("HTTP/1.1 200"), "{res}");
    let woke = events(&h.world, ".mnemorph/memory/creature/slime-a.jsonl");
    let last = woke.last().unwrap();
    assert_eq!(
        (&last["note"], &last["by"], &last["status"], &last["proof"]),
        (
            &json!("n3"),
            &json!("human"),
            &json!("grounded"),
            &json!("woken by the human (board)")
        )
    );
    // Once woken, it is no retirement: a second wake is refused.
    let res = post(
        &host,
        &format!("/api/w/{}/memory/n3/wake", h.id()),
        &cookie,
        Some(&csrf),
        &format!("{{\"seen\":\"{seen}\"}}"),
    );
    assert!(res.starts_with("HTTP/1.1 409"), "{res}");
    // A path for an id: refused.
    let res = post(
        &host,
        &format!("/api/w/{}/memory/..%2Fx/wake", h.id()),
        &cookie,
        Some(&csrf),
        &format!("{{\"seen\":\"{seen}\"}}"),
    );
    assert!(res.starts_with("HTTP/1.1 400"), "{res}");
}

#[test]
fn the_ontology_page_follows_the_partition_and_counts_what_onto_check_finds() {
    let mut h = Home::new("onto");
    h.memory();
    h.ontology();
    let world = h.world.to_str().unwrap().to_owned();
    let check = h.run(&["onto", "check", "--world", &world]);
    let (host, cookie, _) = h.serve();
    let page = h.get(&host, &cookie, "facts");

    let statuses: Vec<(String, String)> = page["facts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["id"].as_str().unwrap().to_owned(),
                f["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let want = [
        ("fsettled", "settled"),
        ("fagreed", "agreed"),
        ("fsingle", "single"),
        ("fcontest", "contested"),
        ("fcounter", "countered"),
    ];
    assert_eq!(
        statuses,
        want.iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect::<Vec<_>>()
    );
    let c = &page["counts"];
    for s in ["settled", "agreed", "single", "contested", "countered"] {
        assert_eq!(c[s], 1, "{s}: {c}");
        let only = h.get(&host, &cookie, &format!("facts?status={s}"));
        assert_eq!(only["total"], 1);
        assert_eq!(only["facts"][0]["status"], s);
    }
    assert_eq!(page["total"], 5, "every live fact in exactly one status");

    // `onto check`'s findings, line for line.
    let (head, found) = check_lines(&check);
    assert!(
        head.ends_with(&format!(" {} findings", found.len())),
        "{head}"
    );
    assert_eq!(size(&page["found"]), found.len());
    assert_eq!(page["failed"], head.starts_with("@S FAIL"));
    let shown: Vec<(String, String, String)> = page["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["severity"].as_str().unwrap().to_owned(),
                f["code"].as_str().unwrap().to_owned(),
                f["at"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(shown, found);
    assert!(shown.iter().any(|(_, code, _)| code == "contested-fact"));

    // A contested fact shows both sides, each with its anchor or its reason.
    let contested = fact(&page, "fcontest");
    assert_eq!(contested["supporters"][0]["creature"], "slime-a");
    assert_eq!(contested["supporters"][0]["anchor"]["at"], "src/a.rs:1");
    assert_eq!(contested["counters"][0]["creature"], "slime-b");
    assert_eq!(
        contested["counters"][0]["reason"],
        "<b>it is hand-written</b>"
    );
    assert_eq!(contested["owner"]["owner"], "slime-a");
    let settled = fact(&page, "fsettled");
    assert_eq!(
        settled["settled"],
        "orc-o: orc-o's verdict: settle #fsettled"
    );
    let file = h.world.join("src/a.rs");
    let owners = h.run(&["onto", "owner", file.to_str().unwrap(), "--world", &world]);
    let o = page["owners"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["path"] == "src/a.rs")
        .unwrap();
    let said: Vec<&str> = owners.trim_end().split('\t').skip(1).collect();
    assert_eq!(
        said,
        [o["owner"].as_str().unwrap(), o["why"].as_str().unwrap()],
        "{owners}"
    );
    assert_eq!(page["verdicts"][0]["n"], 2, "the pass, offered as a proof");
}

#[test]
fn a_settle_needs_a_proof_and_is_journaled_as_the_humans_from_the_board() {
    let mut h = Home::new("settle");
    h.memory();
    h.ontology();
    let (host, cookie, csrf) = h.serve();
    let page = h.get(&host, &cookie, "facts");
    let contested = fact(&page, "fcontest");
    let seen = contested["seen"].as_str().unwrap().to_owned();
    let path = format!("/api/w/{}/facts/fcontest/settle", h.id());
    let journal = || events(&h.world, ".mnemorph/ontology/facts.jsonl").len();
    let before = journal();

    for (body, status) in [
        (format!("{{\"seen\":\"{seen}\"}}"), "400"),
        (
            format!("{{\"seen\":\"{seen}\",\"statement\":\"   \"}}"),
            "400",
        ),
        (
            format!(
                "{{\"seen\":\"{seen}\",\"statement\":\"{}\"}}",
                "x".repeat(1001)
            ),
            "400",
        ),
        (
            format!("{{\"seen\":\"{seen}\",\"verdict\":2,\"statement\":\"both\"}}"),
            "400",
        ),
        (format!("{{\"seen\":\"{seen}\",\"verdict\":1}}"), "422"),
        (format!("{{\"seen\":\"{seen}\",\"verdict\":9}}"), "422"),
        (
            format!(
                "{{\"seen\":\"{}\",\"statement\":\"I checked\"}}",
                "b".repeat(64)
            ),
            "409",
        ),
    ] {
        let res = post(&host, &path, &cookie, Some(&csrf), &body);
        assert!(
            res.starts_with(&format!("HTTP/1.1 {status}")),
            "{body}: {res}"
        );
    }
    let res = post(
        &host,
        &path,
        &cookie,
        None,
        &format!("{{\"seen\":\"{seen}\",\"statement\":\"I checked\"}}"),
    );
    assert!(res.starts_with("HTTP/1.1 403"), "no CSRF: {res}");
    assert_eq!(journal(), before, "nothing filed");

    let res = post(
        &host,
        &path,
        &cookie,
        Some(&csrf),
        &format!("{{\"seen\":\"{seen}\",\"statement\":\"I read src/a.rs: hand-written\"}}"),
    );
    assert!(res.starts_with("HTTP/1.1 200"), "{res}");
    let last = events(&h.world, ".mnemorph/ontology/facts.jsonl")
        .pop()
        .unwrap();
    assert_eq!(
        last,
        json!({"event": "settle", "id": "fcontest", "by": "human (board)",
               "proof": "the human's word (board): I read src/a.rs: hand-written"})
    );
    let after = h.get(&host, &cookie, "facts");
    assert_eq!(after["counts"]["contested"], 0);
    assert_eq!(after["counts"]["settled"], 2);

    // A passing verdict that landed on the fact's path proves it.
    let single = fact(&after, "fagreed");
    let res = post(
        &host,
        &format!("/api/w/{}/facts/fagreed/settle", h.id()),
        &cookie,
        Some(&csrf),
        &format!(
            "{{\"seen\":\"{}\",\"verdict\":2}}",
            single["seen"].as_str().unwrap()
        ),
    );
    assert!(res.starts_with("HTTP/1.1 200"), "{res}");
    let last = events(&h.world, ".mnemorph/ontology/facts.jsonl")
        .pop()
        .unwrap();
    assert_eq!(last["by"], "human (board)");
    assert!(
        last["proof"]
            .as_str()
            .unwrap()
            .starts_with("the gate's verdict 2: PASS on slime-a's landing"),
        "{last}"
    );
}

#[cfg(unix)]
#[test]
fn a_link_in_the_world_is_refused_at_once_and_said() {
    let mut h = Home::new("hostile");
    h.memory();
    h.ontology();
    let tier = h.world.join(".mnemorph/memory/creature/slime-b.jsonl");
    std::fs::remove_file(&tier).unwrap();
    std::os::unix::fs::symlink("/dev/zero", &tier).unwrap();
    let facts = h.world.join(".mnemorph/ontology/facts.jsonl");
    let _ = std::fs::remove_file(&facts);
    std::fs::create_dir_all(facts.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink("/dev/zero", &facts).unwrap();
    let (host, cookie, csrf) = h.serve();
    for (path, file) in [("memory", "slime-b.jsonl"), ("facts", "facts.jsonl")] {
        let res = http(
            &host,
            &format!(
                "GET /api/w/{}/{path} HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n",
                h.id()
            ),
        );
        assert!(res.starts_with("HTTP/1.1 503"), "{res}");
        assert!(
            res.contains(&format!("{file}: not a regular file")),
            "{res}"
        );
    }
    // An act is refused the same way, and nothing is written through the link.
    let res = post(
        &host,
        &format!("/api/w/{}/memory/n2/confirm", h.id()),
        &cookie,
        Some(&csrf),
        &format!("{{\"seen\":\"{}\"}}", "a".repeat(64)),
    );
    assert!(res.starts_with("HTTP/1.1 503"), "{res}");
}

#[test]
fn a_claim_rewritten_after_the_page_read_it_is_not_confirmed() {
    let mut h = Home::new("rewrite");
    h.memory();
    let (host, cookie, csrf) = h.serve();
    // A body rewrites the claim after the page showed it: the old digest is refused.
    let twin8 = create("e12", 12, "n9", "slime-b", "c is pure", "");
    let rewrite = r#"{"id":"e13","time":13,"note":"n9","by":"record","kind":"version","text":"c is impure","depends_on":[]}"#;
    let tier = h.world.join(".mnemorph/memory/creature/slime-b.jsonl");
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&tier)
        .unwrap();
    writeln!(f, "{twin8}").unwrap();
    drop(f);
    let shown = h.get(&host, &cookie, "memory");
    let read_as = note(&shown, "n9", "personal")["seen"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&tier)
        .unwrap();
    writeln!(f, "{rewrite}").unwrap();
    drop(f);
    let res = post(
        &host,
        &format!("/api/w/{}/memory/n9/confirm", h.id()),
        &cookie,
        Some(&csrf),
        &format!("{{\"seen\":\"{read_as}\"}}"),
    );
    assert!(res.starts_with("HTTP/1.1 409"), "{res}");
    assert!(res.contains("changed since it was read"), "{res}");
    let mut lines = events(&h.world, ".mnemorph/memory/creature/slime-b.jsonl");
    assert_eq!(
        lines.pop().unwrap()["id"],
        "e13",
        "nothing written after the rewrite"
    );
}
