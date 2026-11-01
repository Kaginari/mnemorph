//! The board's bodies, queue, timeline and journal through the binary, on fixtures: the colony is
//! the roster's, with each Mind's standing from the witness; the queue's states are `q list`'s,
//! each order marked witnessed or not; the timeline is the run journals' commissions and the gate's
//! verdicts; the journal is `log.md`'s entries, capped; the race art is served by its exact name.

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

/// A commission's journal name for `body`, started at `secs`.
fn run_of(body: &str, secs: u64, n: u32) -> String {
    format!("{body}-{:x}-{n}", u128::from(secs) * 1_000_000_000)
}

impl Home {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-bodiesd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let world = root.join("world");
        std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
        let root = root.canonicalize().unwrap();
        let world = world.canonicalize().unwrap();
        let home = Self {
            root,
            world,
            served: None,
        };
        home.colony();
        home
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn put(&self, rel: &str, text: &str) {
        let at = self.world.join(rel);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }

    /// A throne, an Elf, an Orc wearing a Mind, its Slime, a Kijin and a Dark Elf; two Minds.
    fn colony(&self) {
        for (seat, doc) in [
            ("rimuru/rimuru", "# rimuru\n\n- **Race:** rimuru\n"),
            (
                "elf/faces",
                "# elf-faces\n\n- **Race:** elf\n- **Reports to:** rimuru\n- **Lane:** `src/`\n",
            ),
            (
                "orc/wire",
                "# orc-wire\n\n- **Race:** orc\n- **Reports to:** elf-faces\n- **Domain:** `src/`\n- **Minds:** `wire-craft` `gone-mind`\n",
            ),
            (
                "slime/wire",
                "# slime-wire\n\n- **Race:** slime\n- **Reports to:** orc-wire\n- **Zone:** `src/`\n- **Verify:** `true`\n",
            ),
            ("kijin/forge", "# kijin-forge\n\n- **Race:** kijin\n"),
            (
                "dark-elf/supply",
                "# dark-elf-supply\n\n- **Race:** dark-elf\n",
            ),
        ] {
            self.put(&format!(".mnemorph/{seat}/README.md"), doc);
        }
        self.put(
            ".mnemorph/minds/wire-craft.md",
            "# wire-craft\n\nHow the wire is read.\n",
        );
        self.put(
            ".mnemorph/minds/house.md",
            "# house\n\n- **Shared:** yes\n\nThe house style.\n",
        );
    }

    /// Two commissions — one ended with a passing gate, one cut — and two gate verdicts.
    fn journals(&self) -> (String, String) {
        let done = run_of("slime-wire", 1_792_782_000, 0);
        let cut = run_of("orc-wire", 1_792_782_100, 1);
        let beats = [
            json!({"beat": "ask", "text": "read @W lines into Report.Wrote"}),
            json!({"beat": "gate", "changed": ["src/a.rs"], "failures": [], "verdict": "PASS"}),
            json!({"beat": "end", "ending": "answered"}),
        ];
        let lines: Vec<String> = beats.iter().map(Value::to_string).collect();
        self.put(
            &format!(".mnemorph/instruments/loop/{done}.jsonl"),
            &(lines.join("\n") + "\n"),
        );
        self.put(
            &format!(".mnemorph/instruments/loop/{cut}.jsonl"),
            &(json!({"beat": "ask", "text": "judge the landing"}).to_string() + "\n"),
        );
        // A session's own journal is no commission.
        self.put(
            ".mnemorph/instruments/loop/r1792782000000-7.jsonl",
            &(json!({"beat": "ask", "text": "hi"}).to_string() + "\n"),
        );
        let gate = journal_at(state_file(&self.state(), &self.world, "gate"));
        for (at, verdict, body) in [
            (1_792_782_050, "PASS", "slime-wire"),
            (1_792_782_200, "FAIL, handed back", "orc-wire"),
        ] {
            gate.append(&VerdictRow {
                ts: Timestamp(at),
                body: body.into(),
                verdict: verdict.into(),
                changed: vec!["src/a.rs".into()],
                failures: if verdict == "PASS" {
                    vec![]
                } else {
                    vec!["cargo test".into()]
                },
                unverified: vec![],
                entry: None,
                build: mnemorph_server::world::ledger::Build::default(),
                run: None,
            })
            .unwrap();
        }
        (done, cut)
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
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

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

    fn raw(host: &str, cookie: &str, path: &str) -> String {
        http(
            host,
            &format!(
                "GET {path} HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"
            ),
        )
    }

    fn get(&self, host: &str, cookie: &str, path: &str) -> Value {
        let res = Self::raw(host, cookie, &format!("/api/w/{}/{path}", self.id()));
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
    let mut out = Vec::new();
    s.read_to_end(&mut out).unwrap();
    String::from_utf8_lossy(&out).into_owned()
}

fn body<'a>(page: &'a Value, name: &str) -> &'a Value {
    page["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == name)
        .unwrap_or_else(|| panic!("no body {name} in {page}"))
}

#[test]
fn the_colony_is_the_rosters_with_its_minds_live_states_and_acts() {
    let mut home = Home::new("colony");
    let (done, cut) = home.journals();
    let (host, cookie) = home.serve();
    let page = home.get(&host, &cookie, "bodies");
    let names: Vec<&str> = page["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["name"].as_str().unwrap())
        .collect();
    for n in [
        "rimuru",
        "elf-faces",
        "orc-wire",
        "slime-wire",
        "kijin-forge",
        "dark-elf-supply",
    ] {
        assert!(names.contains(&n), "{n} in {names:?}");
    }
    let orc = body(&page, "orc-wire");
    assert_eq!(orc["parent"], "elf-faces");
    assert_eq!(orc["lane"], "orc");
    assert_eq!(orc["ground"][0], json!({"kind": "domain", "path": "src/"}));
    let minds: Vec<(&str, &str, bool)> = orc["minds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["name"].as_str().unwrap(),
                m["standing"].as_str().unwrap(),
                m["shared"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        minds,
        [
            ("wire-craft", "unadopted", false),
            ("gone-mind", "absent", false),
            ("house", "unadopted", true)
        ]
    );
    assert_eq!(page["minds"]["trusted"], true);
    assert_eq!(page["roster"]["trusted"], false);
    // The acts block is no more trusted than its least: each act carries its own flag.
    assert_eq!(page["acts"]["trusted"], false);
    assert_eq!(body(&page, "kijin-forge")["post"], "forge");
    assert_eq!(body(&page, "dark-elf-supply")["speciality"], "supply");

    // Live states: an ended commission is idle, one without an end and no session cut, none never.
    let slime = body(&page, "slime-wire");
    assert_eq!(slime["live"]["state"], "idle");
    assert_eq!(slime["live"]["run"], done.as_str());
    assert_eq!(orc["live"]["state"], "cut");
    assert_eq!(orc["live"]["run"], cut.as_str());
    assert_eq!(body(&page, "kijin-forge")["live"]["state"], "never");
    assert_eq!(body(&page, "rimuru")["live"]["state"], "idle");
    assert_eq!(page["throne"]["live"], false);
    // The slime's acts: its verdict, then its commission, newest first.
    let acts = slime["acts"].as_array().unwrap();
    assert_eq!(acts.len(), 2);
    assert_eq!(
        (&acts[0]["kind"], &acts[0]["trusted"]),
        (&json!("verdict"), &json!(true))
    );
    assert_eq!(acts[1]["kind"], "commission");
    assert_eq!(acts[1]["trusted"], false);
    assert_eq!(acts[1]["ask"], "read @W lines into Report.Wrote");
    assert_eq!(acts[1]["verdict"], "PASS");

    // Every race has its card, born here or not, its art named exactly.
    let races = page["races"].as_array().unwrap();
    assert_eq!(races.len(), 11);
    let oni = races.iter().find(|r| r["race"] == "oni").unwrap();
    assert_eq!(oni["living"], 0);
    assert_eq!(oni["art"], "/art/cards/oni.webp");
    assert_eq!(
        races.iter().find(|r| r["race"] == "slime").unwrap()["living"],
        1
    );
    assert_eq!(page["loops"], json!([]));
}

#[test]
fn a_loop_in_the_chain_is_reported() {
    let mut home = Home::new("loop");
    home.put(
        ".mnemorph/orc/a/README.md",
        "# orc-a\n\n- **Race:** orc\n- **Reports to:** orc-b\n",
    );
    home.put(
        ".mnemorph/orc/b/README.md",
        "# orc-b\n\n- **Race:** orc\n- **Reports to:** orc-a\n",
    );
    let (host, cookie) = home.serve();
    let page = home.get(&host, &cookie, "bodies");
    assert_eq!(page["loops"], json!(["orc-a", "orc-b"]));
}

#[test]
fn the_queue_states_are_q_lists() {
    let mut home = Home::new("queue");
    let world = home.world.to_string_lossy().into_owned();
    let add = |ask: &str, to: &str, rank: &str| -> String {
        let out = home.run(&[
            "q", "--world", &world, "add", ask, "--for", to, "--rank", rank, "--json",
        ]);
        serde_json::from_str::<Value>(&out).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let claimed = add("wire the reader", "slime-wire", "2");
    let judged = add("judge the reader", "orc-wire", "1");
    let hostile = add("<img src=x onerror=alert(1)>", "slime-wire", "3");
    let dropped = add("drop me", "elf-faces", "4");
    home.run(&["q", "--world", &world, "claim", &claimed]);
    home.run(&["q", "--world", &world, "close", &hostile]);
    home.run(&["q", "--world", &world, "close", &dropped, "--drop"]);
    // A line a body wrote itself, the witness never saw.
    let planted = json!({"id": "planted000001", "rev": 1, "state": "held", "to": "slime-wire", "by": "orc-wire", "rank": 0, "ask": "planted"});
    let file = home.world.join(".mnemorph/shared/queue.jsonl");
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str(&planted.to_string());
    text.push('\n');
    std::fs::write(&file, text).unwrap();
    let (host, cookie) = home.serve();
    let page = home.get(&host, &cookie, "queue");
    let lines = |args: &[&str]| -> Vec<Value> {
        home.run(args)
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let open = lines(&["q", "--world", &world, "list", "--json"]);
    let all = lines(&["q", "--world", &world, "list", "--all", "--json"]);
    let orders = page["orders"].as_array().unwrap();
    assert_eq!(orders.len(), all.len());
    // The open orders first, in q list's order.
    for (o, i) in orders.iter().zip(&open) {
        assert_eq!(o["id"], i["id"]);
        assert_eq!(o["state"], i["state"]);
        assert_eq!(o["rank"], i["rank"]);
        assert_eq!(o["ask"], i["ask"]);
        assert_eq!(o["holder"], i["to"]);
    }
    // Every order, as q list --all has it.
    for i in &all {
        let o = orders.iter().find(|o| o["id"] == i["id"]).unwrap();
        assert_eq!(
            (&o["state"], &o["rev"], &o["by"]),
            (&i["state"], &i["rev"], &i["by"])
        );
    }
    assert_eq!(
        page["counts"],
        json!({"held": 2, "claimed": 1, "done": 1, "dropped": 1})
    );
    let planted = orders.iter().find(|o| o["id"] == "planted000001").unwrap();
    assert_eq!(planted["witnessed"], false);
    assert!(
        orders
            .iter()
            .filter(|o| o["id"] != "planted000001")
            .all(|o| o["witnessed"] == true)
    );
    assert!(orders.iter().any(|o| o["id"] == judged.as_str()));
}

#[test]
fn the_timeline_is_the_journals_and_the_verdicts() {
    let mut home = Home::new("timeline");
    let (done, cut) = home.journals();
    let (host, cookie) = home.serve();
    let t = home.get(&host, &cookie, "timeline");
    let entries = t["entries"].as_array().unwrap();
    // Two commissions (the session's own journal is none) and two verdicts, newest first.
    assert_eq!(
        (t["commissions"].as_u64(), t["verdicts"].as_u64()),
        (Some(2), Some(2))
    );
    let order: Vec<(String, String)> = entries
        .iter()
        .map(|e| {
            (
                e["kind"].as_str().unwrap().to_owned(),
                e["body"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            ("verdict".to_owned(), "orc-wire".to_owned()),
            ("commission".to_owned(), "orc-wire".to_owned()),
            ("verdict".to_owned(), "slime-wire".to_owned()),
            ("commission".to_owned(), "slime-wire".to_owned()),
        ]
    );
    assert_eq!(entries[0]["n"], 2);
    assert_eq!(entries[0]["failures"], json!(["cargo test"]));
    assert_eq!(entries[1]["run"], cut.as_str());
    assert_eq!(entries[1]["state"], "open");
    assert_eq!(entries[3]["run"], done.as_str());
    assert_eq!(entries[3]["trusted"], false);
    assert_eq!(entries[3]["at"], "2026-10-23T19:00:00Z");
    let v = home.get(&host, &cookie, "timeline?kind=verdict&body=slime-wire");
    assert_eq!(v["total"], 1);
    assert_eq!(v["entries"][0]["n"], 1);
    let p = home.get(&host, &cookie, "timeline?limit=1&offset=1");
    assert_eq!(
        (p["total"].as_u64(), p["entries"].as_array().unwrap().len()),
        (Some(4), 1)
    );
    let bad = Home::raw(
        &host,
        &cookie,
        &format!("/api/w/{}/timeline?kind=x", home.id()),
    );
    assert!(bad.starts_with("HTTP/1.1 400"), "{bad}");
}

#[test]
fn the_journal_is_log_md_by_entry_capped_and_refused_through_a_link() {
    let mut home = Home::new("journal");
    let hostile = "<script>alert(1)</script> [x](javascript:alert(1)) ![i](http://beacon/x.png)";
    home.put(
        ".mnemorph/log.md",
        &format!(
            "# Journal\n\n---\n\n### [2026-10-01T10:00:00+02:00] rimuru — the first\n- **Done:** one\n\n---\n\n### [2026-10-02T10:00:00+02:00] orc-wire — the gate\n- **Done:** {hostile}\n\n---\n\n### [2026-10-03T10:00:00+02:00] slime-wire — the long one\n{}\n",
            "word ".repeat(5000)
        ),
    );
    let (host, cookie) = home.serve();
    let j = home.get(&host, &cookie, "journal");
    assert_eq!(j["all"], 4);
    let titles: Vec<&str> = j["toc"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        [
            "the long one",
            "the gate",
            "the first",
            "the journal's head"
        ]
    );
    // An entry's text is cut at its cap, and says so; hostile text is carried as text.
    assert_eq!(j["entries"][0]["cut"], true);
    assert_eq!(
        j["entries"][0]["text"].as_str().unwrap().chars().count(),
        16_000
    );
    assert_eq!(j["entries"][1]["text"], format!("- **Done:** {hostile}"));
    let s = home.get(&host, &cookie, "journal?q=GATE");
    assert_eq!(s["total"], 1);
    assert_eq!(s["entries"][0]["body"], "orc-wire");
    // A link in its place is refused at once, never followed.
    let log = home.world.join(".mnemorph/log.md");
    std::fs::remove_file(&log).unwrap();
    std::os::unix::fs::symlink("/dev/zero", &log).unwrap();
    let res = Home::raw(&host, &cookie, &format!("/api/w/{}/journal", home.id()));
    assert!(res.starts_with("HTTP/1.1 503"), "{res}");
    assert!(res.contains("not a regular file"), "{res}");
}

#[test]
fn the_race_art_is_served_by_its_exact_name() {
    let mut home = Home::new("art");
    let (host, _) = home.serve();
    let card = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/board/public/art/cards/elf.webp"),
    )
    .unwrap();
    let res = http(
        &host,
        &format!("GET /art/cards/elf.webp HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
    );
    assert!(
        res.starts_with("HTTP/1.1 200"),
        "{}",
        &res[..80.min(res.len())]
    );
    assert!(res.contains("content-type: image/webp"));
    assert!(res.len() > card.len());
    for name in [
        "/art/cards/ELF.webp",
        "/art/cards/elf.png",
        "/art/cards/../cards/elf.webp",
        "/art/cards/%65lf.webp",
        "/art/cards/elf.webp/",
        "/art/cards/back.webp",
    ] {
        let res = http(
            &host,
            &format!("GET {name} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
        );
        assert!(
            res.starts_with("HTTP/1.1 404"),
            "{name}: {}",
            &res[..60.min(res.len())]
        );
    }
}

#[test]
fn a_directory_link_on_the_way_reads_nothing_outside_the_world() {
    let mut home = Home::new("dirlink");
    // Outside the world: a queue and a journal a body would have the board read.
    let outside = home.root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let order = json!({"id": "outside00001", "rev": 1, "state": "held", "by": "human", "rank": 0, "ask": "from outside"});
    std::fs::write(outside.join("queue.jsonl"), format!("{order}\n")).unwrap();
    std::fs::write(outside.join("log.md"), "### [t] x — from outside\n").unwrap();
    std::os::unix::fs::symlink(&outside, home.world.join(".mnemorph/shared")).unwrap();
    let (host, cookie) = home.serve();
    let res = Home::raw(&host, &cookie, &format!("/api/w/{}/queue", home.id()));
    assert!(res.starts_with("HTTP/1.1 503"), "{res}");
    assert!(res.contains("a link on the way"), "{res}");
    assert!(!res.contains("from outside"), "{res}");
    // The whole .mnemorph a link out: no world of its own, so the board never lists it.
    let other = Home::new("dirlink-log");
    std::fs::remove_dir_all(other.world.join(".mnemorph")).unwrap();
    std::os::unix::fs::symlink(&outside, other.world.join(".mnemorph")).unwrap();
    let added = other.cmd(&["board", "add", "world"]).output().unwrap();
    assert!(!added.status.success());
    let err = String::from_utf8_lossy(&added.stderr);
    assert!(err.contains("holds no .mnemorph/"), "{err}");
}
