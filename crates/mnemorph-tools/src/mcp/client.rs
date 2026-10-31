//! The MCP client: one stdio server, spoken to in JSON-RPC, one line per message — the
//! `initialize`, `tools/list` and `tools/call` subset, nothing else.
//!
//! A server is a program the human's global configuration names. It runs inside the sandbox: the
//! world readable, its own scratch directory — outside the world — writable and its working
//! directory, the network only when the configuration opens it for that server, and an
//! environment scrubbed of secrets but for the names the configuration passes. Its program is
//! found on the harness's `PATH`, never through the world. Every answer waits at most the
//! server's timeout, and everything it sends is bounded: a dead, silent or flooding server is an
//! error naming it, never a hang.

use std::ffi::OsString;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use mnemorph_sandbox::Policy;
use rustix::process::{Pid, Signal, kill_process_group};
use serde_json::{Value, json};

use super::{Server, Timeouts};
use crate::tool::Context;

/// The protocol version the client speaks.
pub const PROTOCOL: &str = "2025-06-18";
/// The longest line a server may send, in bytes: past it the server is cut off.
pub const MAX_LINE: usize = 4 << 20;
/// The most pages of `tools/list` followed: a server whose cursor never ends is stopped there.
pub const MAX_PAGES: usize = 32;
/// The most tools one server may list: past it the server is refused whole.
pub const MAX_TOOLS: usize = 128;
/// The most lines held between the server and the client: a server that writes faster than they
/// are read waits on its own pipe.
pub const QUEUED_LINES: usize = 8;
/// The most messages a server may send that answer nothing, while one request waits: past it
/// the server is cut off as flooding.
pub const MAX_NOISE: usize = 256;

/// Why a request failed: the server is gone, or it answered with a refusal.
enum Fault {
    /// Silent past its timeout, closed, flooding or no longer reading.
    Dead(String),
    /// It answered the request with an error.
    Refused(String),
}

/// A running server.
pub struct Conn {
    server: String,
    child: Child,
    to_server: SyncSender<Vec<u8>>,
    lines: Receiver<Vec<u8>>,
    next: u64,
    timeouts: Timeouts,
    /// Whether the server still answers: false once it went silent, closed, flooded or stopped
    /// reading — then it is dropped. A refusal it answered leaves it alive.
    alive: bool,
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn")
            .field("server", &self.server)
            .finish_non_exhaustive()
    }
}

/// The policy a server runs under: the world and the toolchains readable, its scratch writable,
/// the network only when its configuration opens it, and the server killed with the harness.
#[must_use]
pub fn policy(server: &Server, cx: &Context, scratch: &Path) -> Policy {
    let mut policy = Policy::new(cx.world(), cx.home(), scratch, server.network);
    // The world is read, never written: an MCP server's effects are its answers.
    policy.write.retain(|p| p != cx.world());
    policy.read.push(cx.world().to_path_buf());
    policy.die_with_parent = true;
    policy
}

/// The directories of a `PATH` value a server's program may come from: absolute ones, never inside
/// the world, so a repository's `node_modules/.bin` or `./bin` is never searched.
#[must_use]
pub fn search_path(path: Option<&OsString>, world: &Path) -> Vec<PathBuf> {
    let world_real = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    path.map(|p| std::env::split_paths(p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.is_absolute())
        .filter(|d| {
            let real = d.canonicalize().unwrap_or_else(|_| d.clone());
            !real.starts_with(&world_real) && !d.starts_with(world)
        })
        .collect()
}

/// `command` as an absolute path: an absolute one kept, a bare name found on `dirs`; a relative
/// path is refused, since it would be found through the working directory.
///
/// # Errors
///
/// A relative path, a program found nowhere, or one inside the world.
pub fn resolve(command: &str, dirs: &[PathBuf], world: &Path) -> Result<PathBuf, String> {
    let found = if command.contains('/') {
        let p = PathBuf::from(command);
        if !p.is_absolute() {
            return Err(format!(
                "`{command}` is a relative path; name a program on your PATH or give its absolute path"
            ));
        }
        p
    } else {
        dirs.iter()
            .map(|d| d.join(command))
            .find(|p| p.is_file())
            .ok_or_else(|| {
                format!("`{command}` is on no directory of your PATH outside the world")
            })?
    };
    let real = found.canonicalize().unwrap_or_else(|_| found.clone());
    let world_real = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    if real.starts_with(&world_real) {
        return Err(format!(
            "`{command}` lies inside the world; a server's program is the human's, never the repository's"
        ));
    }
    Ok(found)
}

impl Conn {
    /// Start `server` in the sandbox of `cx`, working in `scratch`, and shake hands within
    /// `timeouts.start`; its lists wait at most `timeouts.list`, its calls `timeouts.call`.
    ///
    /// # Errors
    ///
    /// No kernel guard and no human permission to run unguarded; the program is not the human's
    /// or did not start; the handshake failed or timed out.
    pub fn start(
        server: &Server,
        cx: &Context,
        scratch: &Path,
        timeouts: Timeouts,
    ) -> Result<Self, String> {
        let name = &server.name;
        if let mnemorph_sandbox::Sandbox::None(reason) = cx.sandbox()
            && !cx.allows_unguarded()
        {
            return Err(format!(
                "MCP server `{name}` did not start: no kernel guard is in force ({reason}); to run \
                 it unguarded, set `sandbox.allow_unguarded = true` in your global config"
            ));
        }
        let harness_path = std::env::var_os("PATH");
        let dirs = search_path(harness_path.as_ref(), cx.world());
        let program = resolve(&server.command, &dirs, cx.world())
            .map_err(|e| format!("MCP server `{name}` did not start: {e}"))?;
        super::private_dir(scratch)
            .map_err(|e| format!("MCP server `{name}`: its scratch could not be made: {e}"))?;
        let policy = policy(server, cx, scratch);
        let args: Vec<&str> = server.args.iter().map(String::as_str).collect();
        let mut cmd = cx
            .sandbox()
            .command(&policy, &program.to_string_lossy(), &args);
        // Never the world: `npx`, `node` and `python -m` read their project from where they run.
        cmd.current_dir(scratch)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .env_clear()
            .envs(crate::shell::scrubbed_env())
            .env("PATH", std::env::join_paths(&dirs).unwrap_or_default())
            .env("TMPDIR", scratch)
            .env("PYTHONNOUSERSITE", "1")
            .env("PYTHONSAFEPATH", "1")
            .env("NODE_PATH", "")
            .env(
                "MNEMORPH_BODY",
                if cx.body().is_empty() {
                    "a body"
                } else {
                    cx.body()
                },
            );
        // The secrets the human passes this server by name, and only those.
        for var in &server.env {
            if let Some(value) = std::env::var_os(var) {
                cmd.env(var, value);
            }
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("MCP server `{name}` did not start: {e}"))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            kill(&child);
            return Err(format!("MCP server `{name}`: no pipes"));
        };
        let mut conn = Self {
            server: name.clone(),
            child,
            to_server: writer(stdin),
            lines: lines(stdout),
            next: 0,
            timeouts,
            alive: true,
        };
        conn.request(
            timeouts.start,
            "initialize",
            &json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {},
                "clientInfo": {"name": "mnemorph", "version": env!("CARGO_PKG_VERSION")},
            }),
        )?;
        conn.notify("notifications/initialized")?;
        Ok(conn)
    }

    /// Every tool the server lists, the cursor followed to its end (at most [`MAX_PAGES`]).
    ///
    /// # Errors
    ///
    /// The server failed, timed out, answered something that is no tool list, or listed more
    /// than [`MAX_TOOLS`].
    pub fn list(&mut self) -> Result<Vec<Value>, String> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let params = cursor
                .as_ref()
                .map_or_else(|| json!({}), |c| json!({"cursor": c}));
            let page = self.request(self.timeouts.list, "tools/list", &params)?;
            let Some(listed) = page["tools"].as_array() else {
                return Err(format!(
                    "MCP server `{}`: tools/list held no tools",
                    self.server
                ));
            };
            if tools.len() + listed.len() > MAX_TOOLS {
                return Err(format!(
                    "MCP server `{}` lists more than {MAX_TOOLS} tools; it is refused",
                    self.server
                ));
            }
            tools.extend(listed.iter().cloned());
            match page["nextCursor"].as_str() {
                Some(next) if !next.is_empty() => cursor = Some(next.to_owned()),
                _ => return Ok(tools),
            }
        }
        Ok(tools)
    }

    /// Call `tool` with `arguments`: the result's text and whether the server called it an error.
    ///
    /// # Errors
    ///
    /// The server failed, timed out or answered no result.
    pub fn call(&mut self, tool: &str, arguments: &Value) -> Result<(String, bool), String> {
        let result = self.request(
            self.timeouts.call,
            "tools/call",
            &json!({"name": tool, "arguments": arguments}),
        )?;
        let mut text = String::new();
        for item in result["content"].as_array().into_iter().flatten() {
            if !text.is_empty() {
                text.push('\n');
            }
            match item["type"].as_str() {
                Some("text") => text.push_str(item["text"].as_str().unwrap_or_default()),
                Some(other) => {
                    text.push('[');
                    text.push_str(&super::clean(&other.chars().take(40).collect::<String>()));
                    text.push_str(" content omitted]");
                }
                None => text.push_str("[unreadable content omitted]"),
            }
        }
        Ok((text, result["isError"].as_bool().unwrap_or(false)))
    }

    /// Whether the server still answers: a request it went silent on, closed during or flooded
    /// leaves it dead; one it refused leaves it alive.
    #[must_use]
    pub fn alive(&self) -> bool {
        self.alive
    }

    /// Send a request and wait at most `within` for its answer; anything else the server says
    /// meanwhile is passed over, at most [`MAX_NOISE`] messages. Every failure but a refusal the
    /// server answered marks it dead.
    fn request(&mut self, within: Duration, method: &str, params: &Value) -> Result<Value, String> {
        let answered = self.exchange(within, method, params);
        if let Err(Fault::Dead(_)) = &answered {
            self.alive = false;
        }
        answered.map_err(|f| match f {
            Fault::Dead(e) | Fault::Refused(e) => e,
        })
    }

    fn exchange(&mut self, within: Duration, method: &str, params: &Value) -> Result<Value, Fault> {
        self.next += 1;
        let id = self.next;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .map_err(Fault::Dead)?;
        let until = Instant::now() + within;
        let mut noise = 0usize;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(left) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(Fault::Dead(format!(
                        "MCP server `{}` did not answer {method} within {}",
                        self.server,
                        seconds(within)
                    )));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Fault::Dead(format!(
                        "MCP server `{}` closed before answering {method}",
                        self.server
                    )));
                }
            };
            let msg = serde_json::from_slice::<Value>(&line).ok();
            let Some(msg) =
                msg.filter(|m| m["id"].as_u64() == Some(id) && m.get("method").is_none())
            else {
                noise += 1;
                if noise > MAX_NOISE {
                    return Err(Fault::Dead(format!(
                        "MCP server `{}` sent more than {MAX_NOISE} messages without answering {method}; it is cut off",
                        self.server
                    )));
                }
                continue;
            };
            if let Some(err) = msg.get("error") {
                let said: String = err["message"]
                    .as_str()
                    .unwrap_or("no message")
                    .chars()
                    .take(300)
                    .collect();
                return Err(Fault::Refused(format!(
                    "MCP server `{}` refused {method}: {}",
                    self.server,
                    super::clean(&said)
                )));
            }
            return Ok(msg["result"].clone());
        }
    }

    fn notify(&mut self, method: &str) -> Result<(), String> {
        self.send(&json!({"jsonrpc": "2.0", "method": method}))
    }

    /// Hand a message to the writer; a server that stopped reading fills the queue, and the
    /// message is refused rather than waited on.
    fn send(&mut self, msg: &Value) -> Result<(), String> {
        let mut line = msg.to_string().into_bytes();
        line.push(b'\n');
        self.to_server
            .try_send(line)
            .map_err(|_| format!("MCP server `{}` stopped reading", self.server))
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        kill(&self.child);
        let _ = self.child.wait();
    }
}

/// A wait as a human reads it: whole seconds, or milliseconds under one.
fn seconds(d: Duration) -> String {
    if d >= Duration::from_secs(1) {
        format!("{}s", d.as_secs())
    } else {
        format!("{}ms", d.as_millis())
    }
}

fn kill(child: &Child) {
    // The group may be gone already; that is the outcome wanted.
    let _ = kill_process_group(Pid::from_child(child), Signal::KILL);
}

/// The server's input, written on a thread: a server that stops reading blocks that thread, never
/// the harness, and the thread ends when the server is killed.
fn writer(mut stdin: ChildStdin) -> SyncSender<Vec<u8>> {
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(QUEUED_LINES);
    thread::spawn(move || {
        for line in rx {
            if stdin.write_all(&line).and_then(|()| stdin.flush()).is_err() {
                break;
            }
        }
    });
    tx
}

/// The server's lines, read on a thread, each at most [`MAX_LINE`] bytes and at most
/// [`QUEUED_LINES`] waiting: a longer line ends the reading, and the server reads as closed.
fn lines(stdout: impl std::io::Read + Send + 'static) -> Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::sync_channel(QUEUED_LINES);
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut buf = Vec::new();
            let limit = u64::try_from(MAX_LINE + 1).unwrap_or(u64::MAX);
            match (&mut reader).take(limit).read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) if buf.len() > MAX_LINE => break,
                Ok(_) => {
                    if tx.send(buf).is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
}
