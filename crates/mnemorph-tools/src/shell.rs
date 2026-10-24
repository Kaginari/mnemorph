//! The `bash` tool: classify, check the shelf, ask the gate, then run in a process group.
//!
//! Every command runs in a process group of its own, started at the world's root with no input.
//! At the deadline the whole group is killed, descendants included, and when the shell exits
//! whatever it left in the background is killed too, so a child holding the output open cannot
//! hang the call. The environment reaches the command with its secrets removed.

use std::fmt::Write as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use mnemorph_races::{ToolClass, check_class};
use mnemorph_sandbox::Policy;
use rustix::process::{Pid, Signal, kill_process_group};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::classify::{Place, Verdict, classify};
use crate::gate::{Answer, Ask};
use crate::tool::{Context, Outcome, Tool, args};
use crate::tripwire::Snapshot;

/// How long a command may run when the call names no timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// The longest timeout a call may ask for.
const MAX_TIMEOUT: Duration = Duration::from_secs(600);
/// How long output pipes get to drain once the group is dead.
const DRAIN: Duration = Duration::from_millis(500);
/// Bytes kept from each of stdout and stderr.
const MAX_OUTPUT: usize = 30_000;

/// Words that mark an environment variable as a secret, matched in its name.
const SECRET_WORDS: &[&str] = &[
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "AUTH",
];

/// Run a shell command inside the world.
pub struct Bash;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BashArgs {
    command: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

impl Tool for Bash {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Run a bash command at the world's root. Each command is classified first: refused commands never run, outward and destructive ones wait for the human. Each call is a fresh shell (a `cd` does not carry over); background processes end with the call."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "The command line."},
                "timeout_ms": {"type": "integer", "minimum": 1, "maximum": 600_000, "description": "Kill the command after this many milliseconds (default 120000)."}
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    fn class(&self) -> ToolClass {
        ToolClass::Read
    }

    fn example(&self) -> Value {
        json!({"command": "cargo test", "timeout_ms": 300_000})
    }

    fn run(&self, raw: Value, cx: &mut Context) -> Outcome {
        let a: BashArgs = match args(self.name(), raw) {
            Ok(a) => a,
            Err(o) => return o,
        };
        let place = Place {
            world: cx.world().to_path_buf(),
            home: cx.home().to_path_buf(),
            cwd: cx.world().to_path_buf(),
        };
        let (class, why, outside, asked) = match classify(&a.command, &place) {
            Verdict::Refuse {
                rule,
                reason,
                class,
            } if cx.asks(rule) => {
                let why = vec![format!(
                    "guard rule `{}` asks instead of refusing: {reason}",
                    rule.name()
                )];
                (class, why, Vec::new(), true)
            }
            Verdict::Refuse { rule, reason, .. } => {
                let hint = if rule.floor() {
                    String::new()
                } else {
                    format!(" (the human may let rule `{}` ask instead)", rule.name())
                };
                return Outcome::error(format!(
                    "refused by guard rule `{}`: {reason}{hint}",
                    rule.name()
                ));
            }
            Verdict::Run {
                class,
                why,
                outside,
            } => (class, why, outside, false),
        };
        if let Err(refusal) = check_class(cx.race(), class) {
            return Outcome::error(refusal.to_string());
        }
        if asked || matches!(class, ToolClass::Outward | ToolClass::Destructive) {
            let ask = Ask {
                race: cx.race(),
                class,
                act: a.command.clone(),
                why,
                outside: outside.clone(),
            };
            if let Answer::Deny(reason) = cx.gate().ask(&ask) {
                return Outcome::error(format!(
                    "the human gate did not approve this {class} command: {reason}"
                ));
            }
        }
        let timeout = a
            .timeout_ms
            .map_or(DEFAULT_TIMEOUT, Duration::from_millis)
            .min(MAX_TIMEOUT);
        let network = matches!(class, ToolClass::Outward | ToolClass::Destructive);
        // An approved act may write the outside paths it named; an unapproved one never has any.
        let opened = if network { outside } else { Vec::new() };
        if let Err(e) = std::fs::create_dir_all(cx.world().join(".mnemorph/tmp")) {
            return Outcome::error(format!(
                "could not prepare the command's temporary directory: {e}"
            ));
        }
        let before = Snapshot::take(cx.world());
        let ran = match run(&a.command, cx, timeout, network, &opened) {
            Ok(r) => r,
            Err(e) => return Outcome::error(format!("could not start bash: {e}")),
        };
        // The command's whole process group is dead: nothing can race the restore.
        let tripped = before.restore();
        let mut out = Outcome::ok(ran.render(timeout), class);
        out.is_error = ran.timed_out || ran.code != Some(0);
        out.meta.exit = ran.code;
        if !tripped.is_empty() {
            out.is_error = true;
            out.content.push_str("\n[the command changed protected files — git's executable configuration or the world's records; ");
            out.content.push_str(&tripped.join("; "));
            out.content.push(']');
        }
        out
    }
}

struct Ran {
    stdout: Captured,
    stderr: Captured,
    code: Option<i32>,
    timed_out: bool,
}

#[derive(Default)]
struct Captured {
    bytes: Vec<u8>,
    dropped: usize,
}

impl Ran {
    fn render(&self, timeout: Duration) -> String {
        let mut s = String::from_utf8_lossy(&self.stdout.bytes).into_owned();
        if self.stdout.dropped > 0 {
            let _ = write!(
                s,
                "\n… {} more bytes of output dropped",
                self.stdout.dropped
            );
        }
        if !self.stderr.bytes.is_empty() {
            if !s.is_empty() && !s.ends_with('\n') {
                s.push('\n');
            }
            s.push_str("[stderr]\n");
            s.push_str(&String::from_utf8_lossy(&self.stderr.bytes));
            if self.stderr.dropped > 0 {
                let _ = write!(
                    s,
                    "\n… {} more bytes of stderr dropped",
                    self.stderr.dropped
                );
            }
        }
        if self.timed_out {
            let _ = write!(
                s,
                "\n[killed after {}s: the command and everything it started]",
                timeout.as_secs_f32()
            );
        } else {
            match self.code {
                Some(0) => {}
                Some(c) => {
                    let _ = write!(s, "\n[exit {c}]");
                }
                None => s.push_str("\n[ended by a signal]"),
            }
        }
        if s.trim().is_empty() {
            s = "[no output]".into();
        }
        s
    }
}

/// Run `command` in the context's sandbox: the world writable, a private temporary directory
/// inside it, the network open only for an approved outward or destructive act.
fn run(
    command: &str,
    cx: &Context,
    timeout: Duration,
    network: bool,
    opened: &[PathBuf],
) -> std::io::Result<Ran> {
    let tmp = cx.world().join(".mnemorph/tmp");
    std::fs::create_dir_all(&tmp)?;
    let mut policy = Policy::new(cx.world(), cx.home(), &tmp, network);
    // A file that exists opens itself; one still to be made opens its directory.
    policy.write.extend(opened.iter().map(|p| {
        if p.exists() {
            p.clone()
        } else {
            p.parent().map_or_else(|| p.clone(), Path::to_path_buf)
        }
    }));
    let mut cmd = cx
        .sandbox()
        .command(&policy, "bash", &["--noprofile", "--norc", "-c", command]);
    cmd.current_dir(cx.world())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .env_clear()
        .envs(scrubbed_env())
        .env("TMPDIR", &tmp);
    let mut child = cmd.spawn()?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            timed_out = true;
            kill_group(&child);
            break child.wait().ok();
        }
        thread::sleep(Duration::from_millis(10));
    };
    // Whatever the shell left running in the background ends with the call.
    kill_group(&child);
    Ok(Ran {
        stdout: stdout.recv_timeout(DRAIN).unwrap_or_default(),
        stderr: stderr.recv_timeout(DRAIN).unwrap_or_default(),
        code: status.and_then(|s| s.code()),
        timed_out,
    })
}

fn kill_group(child: &Child) {
    // The group may already be gone; that is the outcome we want.
    let _ = kill_process_group(Pid::from_child(child), Signal::KILL);
}

/// Read a pipe to its end on a thread, keeping the first `MAX_OUTPUT` bytes.
fn drain(pipe: Option<impl std::io::Read + Send + 'static>) -> mpsc::Receiver<Captured> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut got = Captured::default();
        if let Some(mut pipe) = pipe {
            let mut buf = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let room = MAX_OUTPUT.saturating_sub(got.bytes.len());
                got.bytes.extend_from_slice(&buf[..n.min(room)]);
                got.dropped += n.saturating_sub(room);
            }
        }
        let _ = tx.send(got);
    });
    rx
}

/// The process's environment without its secrets: names carrying a secret word, and values that
/// are URLs with credentials in them.
pub(crate) fn scrubbed_env() -> Vec<(String, String)> {
    std::env::vars().filter(|(k, v)| !is_secret(k, v)).collect()
}

fn is_secret(name: &str, value: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_WORDS.iter().any(|w| upper.contains(w)) || has_userinfo(value)
}

/// `scheme://user:pass@host` — a URL carrying credentials.
fn has_userinfo(value: &str) -> bool {
    value.split_once("://").is_some_and(|(scheme, rest)| {
        !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
            && rest
                .split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| authority.contains('@'))
    })
}

#[cfg(test)]
mod tests {
    use super::{has_userinfo, is_secret};

    #[test]
    fn secrets_are_known_by_name_and_by_credentialed_urls() {
        for name in [
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
            "DB_PASSWORD",
            "OPENROUTER_API_KEY",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "NPM_AUTH",
        ] {
            assert!(is_secret(name, "x"), "{name}");
        }
        assert!(is_secret("DATABASE_URL", "postgres://me:hunter2@db/app"));
        assert!(has_userinfo("https://token@github.com/x/y.git"));
        for (name, value) in [
            ("PATH", "/usr/bin"),
            ("HOME", "/h"),
            ("SITE", "https://example.com/a@b"),
            ("LANG", "C.UTF-8"),
        ] {
            assert!(!is_secret(name, value), "{name}={value}");
        }
    }
}
