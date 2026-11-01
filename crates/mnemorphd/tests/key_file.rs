//! A board's chat whose provider key lives in a file: the board's own environment lacks the
//! variable, and the child — the real `mnemorphd`, started with the environment the board gives
//! its children — reads the key from the file the human's global file names, and sends it to the
//! provider. No output of the child, nor any hole it reports, carries the key's bytes.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use mnemorph_server::board::bridge::{child_env, provider_keys};
use serde_json::{Value, json};

/// A fake key: never a real one, searched for in every output.
const FAKE: &str = "sk-or-v1-fakekeyfile0123456789abcdefXYZ";

struct Ground {
    root: tempfile::TempDir,
}

impl Ground {
    fn new() -> Self {
        let g = Self {
            root: tempfile::tempdir().unwrap(),
        };
        for d in ["home/.config/mnemorph", "state", "world/.mnemorph"] {
            std::fs::create_dir_all(g.at(d)).unwrap();
        }
        g
    }

    fn at(&self, p: &str) -> PathBuf {
        self.root.path().canonicalize().unwrap().join(p)
    }

    /// The human's global file, `~/.config/mnemorph/config.toml` under the fake home, sending
    /// the session's model to `base_url`.
    fn global(&self, base_url: &str) -> PathBuf {
        let p = self.at("home/.config/mnemorph/config.toml");
        std::fs::write(
            &p,
            format!(
                "[sandbox]\nallow_unguarded = true\n\n[models]\nprovider = \"openrouter\"\n\n\
                 [providers.openrouter]\nkind = \"openai\"\nbase_url = \"{base_url}\"\n\
                 key_env = \"OPENROUTER_API_KEY\"\nkey_file = \"~/.config/mnemorph/openrouter.key\"\n"
            ),
        )
        .unwrap();
        p
    }

    fn key(&self, mode: u32) {
        let p = self.at("home/.config/mnemorph/openrouter.key");
        std::fs::write(&p, format!("{FAKE}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    /// The board's own environment — no `OPENROUTER_API_KEY` in it, a secret it must not pass
    /// on — made into a child's as the board makes it.
    fn child_env(&self, global: &Path) -> Vec<(String, String)> {
        let board = [
            ("PATH", std::env::var("PATH").unwrap_or_default()),
            ("HOME", self.at("home").display().to_string()),
            ("XDG_STATE_HOME", self.at("state").display().to_string()),
            ("OTHER_API_TOKEN", "hunter2-not-for-children".to_owned()),
            ("LANG", "C".to_owned()),
        ]
        .map(|(k, v)| (k.to_owned(), v));
        let keys = provider_keys(global, &self.at("world"));
        assert_eq!(keys, ["ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"]);
        let env = child_env(&board, &keys);
        assert!(env.iter().all(|(k, _)| k != "OPENROUTER_API_KEY"));
        env
    }
}

/// A fake provider: each request's head is sent on, and answered with a 400 that is not retried.
fn provider() -> (String, mpsc::Receiver<String>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") && s.read(&mut byte).is_ok_and(|n| n == 1) {
                head.push(byte[0]);
            }
            let _ = tx.send(String::from_utf8_lossy(&head).into_owned());
            let body = r#"{"error":{"message":"bad request"}}"#;
            let _ = write!(
                s,
                "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (url, rx)
}

/// The child, as the board starts it: in the world, its environment cleared and made anew.
fn spawn(g: &Ground, env: &[(String, String)]) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_mnemorphd"))
        .arg("--world")
        .arg(g.at("world"))
        .current_dir(g.at("world"))
        .env_clear()
        .envs(env.iter().cloned())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// Every line the child says, sent on as it comes.
fn lines(child: &mut std::process::Child) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    let out = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

fn send(child: &mut std::process::Child, v: &Value) {
    let to = child.stdin.as_mut().unwrap();
    writeln!(to, "{v}").unwrap();
    to.flush().unwrap();
}

/// Lines until one answers request `id`, each kept in `seen`.
fn answer(rx: &mpsc::Receiver<String>, id: u64, seen: &mut Vec<String>) -> Value {
    loop {
        let line = rx
            .recv_timeout(Duration::from_secs(60))
            .unwrap_or_else(|_| panic!("no answer to {id}: {seen:?}"));
        seen.push(line.clone());
        if let Ok(v) = serde_json::from_str::<Value>(&line)
            && v["id"] == id
        {
            return v;
        }
    }
}

fn stderr(mut child: std::process::Child) -> String {
    let _ = child.kill();
    let _ = child.wait();
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    err
}

#[test]
fn a_board_child_reads_its_key_from_the_file_and_never_says_it() {
    let g = Ground::new();
    let (url, heads) = provider();
    let global = g.global(&url);
    g.key(0o600);
    let env = g.child_env(&global);
    let mut child = spawn(&g, &env);
    let rx = lines(&mut child);
    let mut seen = Vec::new();
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    let hello = answer(&rx, 1, &mut seen);
    let key = hello["result"]["key"].as_str().unwrap_or_default();
    assert!(
        key.starts_with("file ~/.config/mnemorph/openrouter.key, named by providers.openrouter.key_file in global"),
        "{hello}"
    );
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "session/prompt", "params": {"text": "hi"}}),
    );
    let head = heads
        .recv_timeout(Duration::from_secs(60))
        .expect("the provider was asked");
    let auth = head
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
        .unwrap_or_default();
    assert_eq!(
        auth.split_once(':').unwrap().1.trim(),
        format!("Bearer {FAKE}")
    );
    // What the session says of the failed turn, then its end.
    while let Ok(line) = rx.recv_timeout(Duration::from_secs(5)) {
        seen.push(line);
    }
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": {}}),
    );
    answer(&rx, 3, &mut seen);
    let err = stderr(child);
    for out in seen.iter().chain([&err]) {
        assert!(!out.contains(FAKE), "the key leaked: {out}");
        assert!(
            !out.contains("fakekeyfile"),
            "part of the key leaked: {out}"
        );
    }
}

#[test]
fn a_refused_key_file_is_a_hole_naming_both_sources_never_the_key() {
    let g = Ground::new();
    let (url, heads) = provider();
    let global = g.global(&url);
    g.key(0o644);
    let env = g.child_env(&global);
    let mut child = spawn(&g, &env);
    let rx = lines(&mut child);
    let mut seen = Vec::new();
    send(
        &mut child,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    let hello = answer(&rx, 1, &mut seen);
    let text = hello.to_string();
    assert!(
        text.contains(
            "OPENROUTER_API_KEY is not set and ~/.config/mnemorph/openrouter.key is unreadable"
        ) && text.contains("chmod 600 ~/.config/mnemorph/openrouter.key"),
        "{hello}"
    );
    assert!(heads.try_recv().is_err(), "nothing was sent");
    let err = stderr(child);
    for out in seen.iter().chain([&err]) {
        assert!(!out.contains(FAKE), "the key leaked: {out}");
    }
}

/// One HTTP/1.1 exchange over a real socket: the whole answer.
fn http(addr: &str, request: &str) -> String {
    let mut s = std::net::TcpStream::connect(addr).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

/// The board, served on a free port over the fake home, logged in: it, its address and cookie.
fn board(g: &Ground) -> (std::process::Child, String, String) {
    let cmd = |args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_mnemorphd"));
        c.args(args)
            .current_dir(g.at(""))
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", g.at("home"))
            .env("XDG_STATE_HOME", g.at("state"))
            .env("XDG_CONFIG_HOME", g.at("home/.config"));
        c
    };
    assert!(
        cmd(&["board", "add", "world"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut child = cmd(&["board", "--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let (host, key) = line
        .trim()
        .strip_prefix("http://")
        .and_then(|r| r.split_once("/#k="))
        .map_or_else(|| panic!("{line}"), |(h, k)| (h.to_owned(), k.to_owned()));
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
        .unwrap_or_else(|| panic!("{res}"))
        .to_owned();
    (child, host, cookie)
}

#[test]
fn the_integrations_show_a_key_files_state_never_its_contents() {
    let g = Ground::new();
    let global = g.global("http://127.0.0.1:9");
    g.key(0o600);
    let open = g.at("home/open.key");
    std::fs::write(&open, FAKE).unwrap();
    std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o644)).unwrap();
    let inside = g.at("world/inside.key");
    std::fs::write(&inside, FAKE).unwrap();
    std::fs::set_permissions(&inside, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut text = std::fs::read_to_string(&global).unwrap();
    std::fmt::Write::write_fmt(&mut text, format_args!(
        "\n[providers.open]\nkind = \"openai\"\nbase_url = \"http://127.0.0.1:9\"\nkey_file = \"~/open.key\"\n\
         \n[providers.inside]\nkind = \"openai\"\nbase_url = \"http://127.0.0.1:9\"\nkey_file = \"{}\"\n",
        inside.display()
    ))
    .unwrap();
    std::fs::write(&global, text).unwrap();
    let (mut child, host, cookie) = board(&g);
    let res = http(
        &host,
        &format!(
            "GET /api/integrations HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"
        ),
    );
    let _ = child.kill();
    let _ = child.wait();
    assert!(res.starts_with("HTTP/1.1 200"), "{res}");
    assert!(
        !res.contains(FAKE) && !res.contains("fakekeyfile"),
        "the key leaked: {res}"
    );
    let v: Value = serde_json::from_str(res.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let providers = v["config"]["value"]["providers"].as_array().unwrap();
    let file = |name: &str| {
        providers
            .iter()
            .find(|p| p["name"] == name)
            .unwrap_or_else(|| panic!("{name}: {providers:?}"))["keyFile"]
            .clone()
    };
    assert_eq!(
        file("openrouter"),
        json!({"path": "~/.config/mnemorph/openrouter.key", "state": "readable", "detail": "0600"})
    );
    let open = file("open");
    assert_eq!(open["state"], "refused");
    assert!(
        open["detail"]
            .as_str()
            .unwrap()
            .contains("chmod 600 ~/open.key"),
        "{open}"
    );
    let inside = file("inside");
    assert_eq!(inside["state"], "refused");
    assert!(
        inside["detail"]
            .as_str()
            .unwrap()
            .contains("inside the world"),
        "{inside}"
    );
    assert_eq!(file("anthropic"), Value::Null);
    let effective = v["config"]["value"]["effective"].as_array().unwrap();
    let shown = effective
        .iter()
        .find(|s| s["key"] == "providers.openrouter.key_file")
        .unwrap();
    assert_eq!(shown["value"], "\"~/.config/mnemorph/openrouter.key\"");
    assert_eq!(shown["globalOnly"], true);
}
