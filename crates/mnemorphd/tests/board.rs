//! `mnemorphd board` through the binary: one board per machine — a second launch prints the
//! running one's URL —, loopback only, the key traded once over a real socket, and the human's
//! commands refused to a body.

#[path = "support/isolated.rs"]
mod isolated;

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
        // A short root: the control socket's path must fit a Unix socket's (104 bytes on
        // macOS, whose temporary directory is long).
        let tmp = if cfg!(unix) && std::path::Path::new("/tmp").is_dir() {
            PathBuf::from("/tmp")
        } else {
            std::env::temp_dir()
        };
        let root = tmp.join(format!("mnemorph-boardd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("world/.mnemorph")).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
            served: None,
        }
    }

    fn cmd(&self, args: &[&str], body: Option<&str>) -> Command {
        let mut c = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"));
        c.args(args)
            .current_dir(&self.root)
            .env_remove("MNEMORPH_BODY")
            .env("HOME", &self.root)
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
fn open_is_refused_without_a_built_board() {
    if mnemorph_server::board::built() {
        return;
    }
    let h = Home::new("open");
    let (code, out, err) = h.run(&["board", "--port", "0", "--open"], None);
    assert_eq!(code, Some(2), "{err}");
    assert_eq!(out, "");
    assert!(err.contains("no board built in"), "{err}");
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

/// The board the terminal app starts (`board.autostart`): one, reused; the world listed; a fresh
/// link opened by the board itself; no key in a status line or the log; `board stop` stopping only
/// this user's board, under this state directory. Every board here runs under a temporary state
/// directory, on a free port, and is stopped when its test ends.
#[cfg(unix)]
mod autostart {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use mnemorph_server::board::autostart::{self, Ask};
    use mnemorph_server::board::control;
    use mnemorph_server::world::dimension;

    use super::Home;

    /// Whether `text` holds anything shaped like a key: 64 hex characters in a row.
    fn keyed(text: &str) -> bool {
        text.as_bytes()
            .windows(64)
            .any(|w| w.iter().all(u8::is_ascii_hexdigit))
    }

    fn state(h: &Home) -> PathBuf {
        h.root.join("state/mnemorph")
    }

    /// An opener on `PATH` that writes the URL it is given to `<root>/opened`.
    fn opener(h: &Home) -> OsString {
        use std::os::unix::fs::PermissionsExt as _;
        let bin = h.root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\n",
            h.root.join("opened").display()
        );
        for name in ["xdg-open", "open"] {
            let p = bin.join(name);
            std::fs::write(&p, &script).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // No default browser known: the human's own is never asked.
        for name in ["xdg-settings", "xdg-mime"] {
            let p = bin.join(name);
            std::fs::write(&p, "#!/bin/sh\nexit 1\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut path = OsString::from(bin.as_os_str());
        path.push(":");
        path.push(std::env::var_os("PATH").unwrap_or_default());
        path
    }

    /// Stops the board under a state directory when dropped.
    struct Stop(PathBuf);
    impl Drop for Stop {
        fn drop(&mut self) {
            let _ = control::stop(&self.0);
        }
    }

    fn ensure(h: &Home, open: bool, port: Option<u16>) -> String {
        let path = opener(h);
        let config = h.root.join("config");
        // The board inherits the app's environment whole: every directory of the human's that it
        // could read is named here, so none of theirs is.
        let iso = crate::isolated::home();
        let (data, cache, run) = (
            iso.join(".local/share"),
            iso.join(".cache"),
            iso.join("run"),
        );
        let env: [(&str, &std::ffi::OsStr); 6] = [
            ("HOME", h.root.as_os_str()),
            ("XDG_CONFIG_HOME", config.as_os_str()),
            ("XDG_DATA_HOME", data.as_os_str()),
            ("XDG_CACHE_HOME", cache.as_os_str()),
            ("XDG_RUNTIME_DIR", run.as_os_str()),
            ("PATH", path.as_os_str()),
        ];
        autostart::ensure_in(
            Path::new(env!("CARGO_BIN_EXE_mnemorphd")),
            &state(h),
            &h.root.join("world"),
            Ask {
                open,
                port: Some(port.unwrap_or(0)),
            },
            Duration::from_secs(10),
            &env,
        )
    }

    #[test]
    fn one_board_is_started_and_reused_the_world_listed_and_no_key_printed() {
        let h = Home::new("autostart");
        let _stop = Stop(state(&h));
        let first = ensure(&h, false, None);
        assert!(first.starts_with("board: http://127.0.0.1:"), "{first}");
        assert!(
            first.ends_with("(run `mnemorphd board` for a sign-in link)"),
            "{first}"
        );
        let running = control::ping(&state(&h)).unwrap();
        assert_eq!(
            first,
            format!(
                "board: {} (run `mnemorphd board` for a sign-in link)",
                running.base
            )
        );

        // The world is on the board.
        let registry = state(&h).join("worlds.jsonl");
        let listed = dimension::listed(&registry, &dimension::hand_file(&registry));
        assert!(
            listed.iter().any(|l| l.world == h.root.join("world")),
            "{listed:?}"
        );

        // A second app reuses it: the same board, never a second; with `open`, the board itself
        // opens a fresh link.
        let printed = std::fs::read_to_string(state(&h).join("board.url")).unwrap();
        let second = ensure(&h, true, None);
        assert_eq!(control::ping(&state(&h)).unwrap().pid, running.pid);
        if mnemorph_server::board::built() {
            assert_eq!(
                second,
                format!("board: {} (opened in your browser)", running.base)
            );
            // The opener runs beside the board's answer: give it a moment to write.
            let at = h.root.join("opened");
            let began = Instant::now();
            while !at.exists() && began.elapsed() < Duration::from_secs(10) {
                std::thread::sleep(Duration::from_millis(50));
            }
            // The opener was handed a launcher file, never a key; the file holds a fresh one.
            let argv = std::fs::read_to_string(&at).unwrap();
            let argv = argv.trim();
            assert!(!keyed(argv), "a key on the opener's command line: {argv}");
            let file = PathBuf::from(argv);
            assert!(file.starts_with(state(&h)), "{argv}");
            let page = std::fs::read_to_string(&file).unwrap();
            let at = page.find("#k=").unwrap() + 3;
            let key = &page[at..at + 64];
            assert!(
                page.contains(&format!("{}/#k={key}", running.base)),
                "{page}"
            );
            assert!(!printed.contains(key), "a fresh key, never the printed one");
        } else {
            assert!(
                second.contains(mnemorph_server::board::UNOPENED),
                "{second}"
            );
            assert!(second.contains("no board built in"), "{second}");
        }

        // No key in either line or in the log; `mnemorphd board` prints the sign-in link.
        let log = std::fs::read_to_string(state(&h).join(autostart::LOG)).unwrap();
        for text in [&first, &second, &log] {
            assert!(!keyed(text), "a key: {text}");
        }
        let (code, out, err) = h.run(&["board"], None);
        assert_eq!(code, Some(0), "{err}");
        assert!(
            out.trim().starts_with(&format!("{}/#k=", running.base)),
            "{out}"
        );
    }

    #[test]
    fn the_control_socket_answers_only_its_nonce() {
        let h = Home::new("autostart-nonce");
        let _stop = Stop(state(&h));
        ensure(&h, false, None);
        let dir = state(&h);
        let running = control::ping(&dir).unwrap();
        // The nonce is the human's alone: a private file beside the socket.
        let nonce_file = dir.join(control::NONCE);
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&nonce_file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // Without it, or with another, `open`, `stop` and `ping` are refused, and nothing happens.
        for line in [
            "open".to_owned(),
            "stop".to_owned(),
            "ping".to_owned(),
            format!("{} open", "0".repeat(64)),
            format!("{} stop", "0".repeat(64)),
            String::new(),
        ] {
            assert_eq!(
                control::ask_raw(&dir, &line).unwrap(),
                "refused",
                "{line:?}"
            );
        }
        assert!(!h.root.join("opened").exists(), "no browser was opened");
        assert_eq!(
            control::ping(&dir).unwrap().pid,
            running.pid,
            "still running"
        );
        // With it, the human's own commands work.
        let nonce = std::fs::read_to_string(&nonce_file).unwrap();
        assert!(
            control::ask_raw(&dir, &format!("{nonce} ping"))
                .unwrap()
                .starts_with("ok ")
        );
        let (code, _, err) = h.run(&["board", "stop"], None);
        assert_eq!(code, Some(0), "{err}");
        assert!(!nonce_file.exists(), "the nonce goes with the board");
    }

    #[test]
    fn a_port_in_use_is_a_status_line() {
        let h = Home::new("autostart-port");
        let _stop = Stop(state(&h));
        let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = taken.local_addr().unwrap().port();
        let began = Instant::now();
        let line = ensure(&h, false, Some(port));
        assert!(began.elapsed() < Duration::from_secs(10), "no hang");
        assert!(line.starts_with("board: not started: "), "{line}");
        assert!(line.contains("cannot be bound"), "{line}");
        assert!(line.contains(autostart::LOG), "{line}");
        assert!(control::ping(&state(&h)).is_err());
    }

    #[test]
    fn no_mnemorphd_beside_is_a_status_line() {
        let h = Home::new("autostart-none");
        let line = autostart::ensure_in(
            &h.root.join("nowhere/mnemorphd"),
            &state(&h),
            &h.root.join("world"),
            Ask::default(),
            Duration::from_secs(1),
            &[],
        );
        assert!(
            line.starts_with("board: not started: no mnemorphd beside"),
            "{line}"
        );
    }

    #[test]
    fn board_stop_stops_only_this_users_board_under_this_state_directory() {
        let a = Home::new("stop-a");
        let b = Home::new("stop-b");
        let (_sa, _sb) = (Stop(state(&a)), Stop(state(&b)));
        ensure(&a, false, None);
        ensure(&b, false, None);
        let pa = control::ping(&state(&a)).unwrap().pid;
        let pb = control::ping(&state(&b)).unwrap().pid;
        assert_ne!(pa, pb);

        let (code, out, err) = a.run(&["board", "stop"], None);
        assert_eq!(code, Some(0), "{err}");
        assert_eq!(out.trim(), format!("board stopped (pid {pa})"));
        assert!(control::ping(&state(&a)).is_err());
        assert!(
            !state(&a).join(control::SOCKET).exists(),
            "its socket is gone"
        );
        assert!(
            !state(&a).join("board.lock").exists() || {
                // The lock file stays; its hold is let go: a new board may claim it.
                let again = ensure(&a, false, None);
                again.starts_with("board: http://")
            }
        );
        assert_eq!(
            control::ping(&state(&b)).unwrap().pid,
            pb,
            "the other board runs on"
        );

        // Nothing to stop, and a socket that is not one: refused, nothing signalled.
        let c = Home::new("stop-c");
        let (code, _, err) = c.run(&["board", "stop"], None);
        assert_eq!(code, Some(2), "{err}");
        assert!(err.contains("no board is running"), "{err}");
        std::fs::create_dir_all(state(&c)).unwrap();
        std::fs::write(state(&c).join(control::SOCKET), "not a socket").unwrap();
        let (code, _, err) = c.run(&["board", "stop"], None);
        assert_eq!(code, Some(2), "{err}");
        assert!(err.contains("not this user's board socket"), "{err}");
    }
}

#[test]
fn prune_forgets_only_the_missing_worlds_and_a_dry_run_changes_nothing() {
    let h = Home::new("prune");
    let gone = h.root.join("gone");
    std::fs::create_dir_all(gone.join(".mnemorph")).unwrap();
    for w in ["world", "gone"] {
        let (code, _, err) = h.run(&["board", "add", w], None);
        assert_eq!(code, Some(0), "{err}");
    }
    std::fs::remove_dir_all(&gone).unwrap();
    let hand = h.root.join("state/mnemorph/board/worlds.jsonl");
    let before = std::fs::read(&hand).unwrap();
    let (code, out, err) = h.run(&["board", "prune", "--dry-run"], None);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains(&format!("would forget: {}", gone.display())),
        "{out}"
    );
    assert!(!out.contains("world ("), "{out}");
    assert_eq!(
        std::fs::read(&hand).unwrap(),
        before,
        "a dry run writes nothing"
    );

    let (code, out, err) = h.run(&["board", "prune"], None);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains(&format!("forgotten: {}", gone.display())),
        "{out}"
    );
    assert!(h.root.join("world/.mnemorph").is_dir(), "no world touched");
    let (_, out, _) = h.run(&["board", "prune"], None);
    assert_eq!(out.trim(), "no missing world on the board");
    let (code, _, err) = h.run(&["board", "prune"], Some("orc-x"));
    assert_eq!(code, Some(2));
    assert!(err.contains("orc-x runs this"), "{err}");
}
