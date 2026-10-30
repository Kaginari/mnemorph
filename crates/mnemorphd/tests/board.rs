//! `mnemorphd board` through the binary: one board per machine — a second launch prints the
//! running one's URL —, loopback only, the key traded once over a real socket, and the human's
//! commands refused to a body.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

struct Home {
    root: PathBuf,
    served: Option<Child>,
}

impl Home {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-boardd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("world")).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
            served: None,
        }
    }

    fn cmd(&self, args: &[&str], body: Option<&str>) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_mnemorphd"));
        c.args(args)
            .current_dir(&self.root)
            .env_remove("MNEMORPH_BODY")
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_CONFIG_HOME", self.root.join("config"));
        if let Some(b) = body {
            c.env("MNEMORPH_BODY", b);
        }
        c
    }

    fn run(&self, args: &[&str], body: Option<&str>) -> (Option<i32>, String, String) {
        let out = self.cmd(args, body).output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    /// Start a board on a free port; its printed URL.
    fn serve(&mut self) -> String {
        let mut child = self
            .cmd(&["board", "--port", "0"], None)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        self.served = Some(child);
        line.trim().to_owned()
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

/// One HTTP/1.1 exchange over a real socket: the status line and the whole answer.
fn http(addr: &str, request: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn a_second_launch_prints_the_running_url() {
    let mut h = Home::new("second");
    let url = h.serve();
    let (host, key) = url
        .strip_prefix("http://")
        .and_then(|r| r.split_once("/#k="))
        .unwrap();
    assert!(host.starts_with("127.0.0.1:"), "{url}");
    assert_eq!(key.len(), 64);

    let (code, out, err) = h.run(&["board"], None);
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(out.trim(), url);
    assert!(err.contains("already running"), "{err}");

    // Over the socket: a foreign Host is refused, the health check answers.
    let foreign = http(
        host,
        "GET /api/health HTTP/1.1\r\nHost: evil.example\r\nConnection: close\r\n\r\n",
    );
    assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");
    let ok = http(
        host,
        &format!("GET /api/health HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
    );
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");

    // The key is traded once; the second launch then prints the key waiting now.
    let trade = || {
        let body = format!("{{\"key\":\"{key}\"}}");
        http(
            host,
            &format!(
                "POST /session HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        )
    };
    let first = trade();
    assert!(first.starts_with("HTTP/1.1 200"), "{first}");
    assert!(
        first.contains("HttpOnly; SameSite=Strict; Path=/"),
        "{first}"
    );
    let again = trade();
    assert!(again.starts_with("HTTP/1.1 401"), "{again}");
    let (_, now, _) = h.run(&["board"], None);
    let now = now.trim();
    assert!(now.starts_with(&format!("http://{host}/#k=")), "{now}");
    assert_ne!(now, url);
}

#[test]
fn a_non_loopback_bind_is_refused() {
    let h = Home::new("bind");
    for addr in ["0.0.0.0", "::", "192.168.1.10", "localhost", "example.com"] {
        let (code, out, err) = h.run(&["board", "--bind", addr], None);
        assert_eq!(code, Some(2), "{addr}: {err}");
        assert_eq!(out, "");
        assert!(err.contains("loopback only"), "{err}");
    }
    // Nothing was claimed: no lock left behind.
    assert!(!h.root.join("state/mnemorph/board.lock").exists());
}

#[test]
fn a_body_never_runs_the_board() {
    let h = Home::new("body");
    for args in [
        &["board", "add", "world"][..],
        &["board", "forget", "world"],
        &["board", "--port", "0"],
    ] {
        let (code, _, err) = h.run(args, Some("slime-s"));
        assert_eq!(code, Some(2), "{args:?}: {err}");
        assert!(err.contains("slime-s runs this"), "{err}");
    }
    assert!(!h.root.join("state/mnemorph/board").exists());
    assert!(!h.root.join("state/mnemorph/board.lock").exists());

    // The human adds and forgets.
    let (code, out, err) = h.run(&["board", "add", "world"], None);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.starts_with("on the board: "), "{out}");
    let (code, out, err) = h.run(&["board", "forget", "world"], None);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.starts_with("forgotten: "), "{out}");
    let (code, _, err) = h.run(&["board", "forget", "world"], None);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("not on the board"), "{err}");
}
