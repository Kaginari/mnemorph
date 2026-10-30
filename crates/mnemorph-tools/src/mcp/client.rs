//! The MCP client: one stdio server, spoken to in JSON-RPC, one line per message — the
//! `initialize`, `tools/list` and `tools/call` subset, nothing else.
//!
//! A server is a program the human's own configuration names. It runs inside the sandbox: the
//! world readable, its own scratch directory writable, the network only when the configuration
//! opens it for that server, and an environment scrubbed of secrets but for the names the
//! configuration passes. Every answer waits at most the server's timeout: a dead or silent server
//! is an error naming it, never a hang.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::os::unix::process::CommandExt as _;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use mnemorph_sandbox::Policy;
use rustix::process::{Pid, Signal, kill_process_group};
use serde_json::{Value, json};

use super::Server;
use crate::tool::Context;

/// The protocol version the client speaks.
pub const PROTOCOL: &str = "2025-06-18";
/// The longest line a server may send, in bytes: past it the server is cut off.
pub const MAX_LINE: usize = 4 << 20;
/// The most pages of `tools/list` followed: a server whose cursor never ends is stopped there.
pub const MAX_PAGES: usize = 32;

/// A running server.
pub struct Conn {
    server: String,
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Vec<u8>>,
    next: u64,
    timeout: Duration,
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn")
            .field("server", &self.server)
            .finish_non_exhaustive()
    }
}

/// The policy a server runs under: the world and the toolchains readable, its scratch writable,
/// the network only when its configuration opens it.
#[must_use]
pub fn policy(server: &Server, cx: &Context, scratch: &std::path::Path) -> Policy {
    let mut policy = Policy::new(cx.world(), cx.home(), scratch, server.network);
    // The world is read, never written: an MCP server's effects are its answers.
    policy.write.retain(|p| p != cx.world());
    policy.read.push(cx.world().to_path_buf());
    policy
}

impl Conn {
    /// Start `server` in the sandbox of `cx` and shake hands.
    ///
    /// # Errors
    ///
    /// No kernel guard and no human permission to run unguarded; the program did not start; the
    /// handshake failed or timed out.
    pub fn start(server: &Server, cx: &Context) -> Result<Self, String> {
        let name = &server.name;
        if let mnemorph_sandbox::Sandbox::None(reason) = cx.sandbox()
            && !cx.allows_unguarded()
        {
            return Err(format!(
                "MCP server `{name}` did not start: no kernel guard is in force ({reason}); to run \
                 it unguarded, set `sandbox.allow_unguarded = true` in your local or global config"
            ));
        }
        let scratch = cx.world().join(".mnemorph/tmp/mcp").join(name);
        std::fs::create_dir_all(&scratch)
            .map_err(|e| format!("MCP server `{name}`: its scratch could not be made: {e}"))?;
        let policy = policy(server, cx, &scratch);
        let args: Vec<&str> = server.args.iter().map(String::as_str).collect();
        let mut cmd = cx.sandbox().command(&policy, &server.command, &args);
        cmd.current_dir(cx.world())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .env_clear()
            .envs(crate::shell::scrubbed_env())
            .env("TMPDIR", &scratch)
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
            stdin,
            lines: lines(stdout),
            next: 0,
            timeout: server.timeout,
        };
        conn.request(
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
    /// The server failed, timed out or answered something that is no tool list.
    pub fn list(&mut self) -> Result<Vec<Value>, String> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let params = cursor
                .as_ref()
                .map_or_else(|| json!({}), |c| json!({"cursor": c}));
            let page = self.request("tools/list", &params)?;
            let Some(listed) = page["tools"].as_array() else {
                return Err(format!(
                    "MCP server `{}`: tools/list held no tools",
                    self.server
                ));
            };
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
        let result = self.request("tools/call", &json!({"name": tool, "arguments": arguments}))?;
        let mut text = String::new();
        for item in result["content"].as_array().into_iter().flatten() {
            if !text.is_empty() {
                text.push('\n');
            }
            match item["type"].as_str() {
                Some("text") => text.push_str(item["text"].as_str().unwrap_or_default()),
                Some(other) => {
                    text.push('[');
                    text.push_str(other);
                    text.push_str(" content omitted]");
                }
                None => text.push_str("[unreadable content omitted]"),
            }
        }
        Ok((text, result["isError"].as_bool().unwrap_or(false)))
    }

    /// Send a request and wait for its answer; anything else the server says meanwhile is
    /// passed over.
    fn request(&mut self, method: &str, params: &Value) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        let until = Instant::now() + self.timeout;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(left) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!(
                        "MCP server `{}` did not answer {method} within {}s",
                        self.server,
                        self.timeout.as_secs()
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(format!(
                        "MCP server `{}` closed before answering {method}",
                        self.server
                    ));
                }
            };
            let Ok(msg) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            if msg["id"].as_u64() != Some(id) || msg.get("method").is_some() {
                continue;
            }
            if let Some(err) = msg.get("error") {
                let said: String = err["message"]
                    .as_str()
                    .unwrap_or("no message")
                    .chars()
                    .take(300)
                    .collect();
                return Err(format!(
                    "MCP server `{}` refused {method}: {}",
                    self.server,
                    super::clean(&said)
                ));
            }
            return Ok(msg["result"].clone());
        }
    }

    fn notify(&mut self, method: &str) -> Result<(), String> {
        self.send(&json!({"jsonrpc": "2.0", "method": method}))
    }

    fn send(&mut self, msg: &Value) -> Result<(), String> {
        let mut line = msg.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|()| self.stdin.flush())
            .map_err(|e| format!("MCP server `{}` stopped reading: {e}", self.server))
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        kill(&self.child);
        let _ = self.child.wait();
    }
}

fn kill(child: &Child) {
    // The group may be gone already; that is the outcome wanted.
    let _ = kill_process_group(Pid::from_child(child), Signal::KILL);
}

/// The server's lines, read on a thread, each at most [`MAX_LINE`] bytes: a longer one ends the
/// reading, and the server reads as closed.
fn lines(stdout: impl std::io::Read + Send + 'static) -> Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
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
