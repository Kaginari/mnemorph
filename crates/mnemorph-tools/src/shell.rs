//! The `bash` tool: classify, check the shelf, ask the gate, then run in a process group.
//!
//! Every command runs in a process group of its own, started at the world's root with no input.
//! At the deadline the whole group is killed, descendants included, and when the shell exits
//! whatever it left in the background is killed too, so a child holding the output open cannot
//! hang the call. The environment reaches the command with its secrets removed.

use std::fmt::Write as _;
use std::fs;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use mnemorph_races::{OutwardKind, Race, ToolClass, check_class, check_post};
use mnemorph_sandbox::Policy;
use rustix::process::{Pid, Signal, kill_process_group};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::classify::{Place, Verdict, classify_reach};
use crate::fingerprint::Fingerprint;
use crate::gate::{Answer, Ask};
use crate::tool::{Context, Outcome, Tool, args};
use crate::tripwire::Snapshot;

/// How long a command may run when the call names no timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// The longest timeout a call may ask for.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(600);
/// How long output pipes get to drain once the group is dead.
const DRAIN: Duration = Duration::from_millis(500);
/// Bytes kept from each of stdout and stderr.
const MAX_OUTPUT: usize = 30_000;

/// Words that mark an environment variable as a secret, matched in its name.
const SECRET_WORDS: &[&str] = &["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL"];
/// Words that mark a secret only as a whole `_`-separated part of the name (`NPM_AUTH`, not
/// `AUTHOR`).
const SECRET_PARTS: &[&str] = &["AUTH", "PAT"];

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
        // Containment is never claimed, and never silently skipped: with no kernel guard in
        // force nothing runs, unless the human chose otherwise in their own configuration.
        if let mnemorph_sandbox::Sandbox::None(reason) = cx.sandbox()
            && !cx.allows_unguarded()
        {
            return Outcome::error(format!(
                "no kernel guard is in force ({reason}), so the command did not run; to run commands unguarded, set `sandbox.allow_unguarded = true` in your local or global config"
            ));
        }
        let timeout = a
            .timeout_ms
            .map_or(DEFAULT_TIMEOUT, Duration::from_millis)
            .min(MAX_TIMEOUT);
        // Every approval is bound to the world it was given in: a world that changed before the
        // spawn runs nothing and is asked again, a bounded number of times.
        let mut changed: Vec<String> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        for _ in 0..=MAX_REASKS {
            let admitted = match admit(&a.command, cx, &changed) {
                Ok(admitted) => admitted,
                Err(mut refused) => {
                    for note in &notes {
                        let _ = write!(refused.content, "\n[{note}]");
                    }
                    return refused;
                }
            };
            match attempt(&a.command, cx, timeout, admitted) {
                Attempt::Ran(mut out) => {
                    for note in &notes {
                        let _ = write!(out.content, "\n[{note}]");
                    }
                    return out;
                }
                Attempt::Changed { approved, now } => {
                    changed = approved.changes(&now);
                    notes.push(format!(
                        "the world changed since approval of plan {}, so the command did not run and the gate asked again: {}",
                        approved.id(),
                        changed.join("; ")
                    ));
                }
                Attempt::Failed(out) => return out,
            }
        }
        let mut out = Outcome::error(format!(
            "the world changed between each approval and the spawn, {} times: the command did not run",
            MAX_REASKS + 1
        ));
        for note in &notes {
            let _ = write!(out.content, "\n[{note}]");
        }
        out
    }
}

/// How many times an approval whose world changed before the spawn is asked again.
const MAX_REASKS: usize = 3;

/// What the classifier and the gate let through.
struct Admitted {
    class: ToolClass,
    /// The outside paths the command named, opened only when the act was approved.
    outside: Vec<PathBuf>,
    /// The plan the human approved, when the gate was asked.
    plan: Option<Fingerprint>,
}

/// One try at running an admitted command.
enum Attempt {
    /// It ran: the result to give the body.
    Ran(Outcome),
    /// The world no longer matched the approved plan at the spawn: nothing ran.
    Changed {
        approved: Fingerprint,
        now: Fingerprint,
    },
    /// It could not be prepared or started.
    Failed(Outcome),
}

fn attempt(command: &str, cx: &Context, timeout: Duration, admitted: Admitted) -> Attempt {
    let Admitted {
        class,
        outside,
        plan,
    } = admitted;
    let network = matches!(class, ToolClass::Outward | ToolClass::Destructive);
    // An approved act may write the outside paths it named; an unapproved one never has any.
    let opened = if network { outside } else { Vec::new() };
    // The shared shelf exists before the snapshot, so a queue the human starts while the
    // command runs is not taken for a directory the command made.
    let _ = std::fs::create_dir_all(cx.world().join(".mnemorph/shared"));
    if let Err(e) = std::fs::create_dir_all(cx.world().join(".mnemorph/tmp")) {
        return Attempt::Failed(Outcome::error(format!(
            "could not prepare the command's temporary directory: {e}"
        )));
    }
    let made = match make_approved(&opened) {
        Ok(m) => m,
        Err(e) => return Attempt::Failed(Outcome::error(e)),
    };
    let mut before = Snapshot::take(cx.world());
    // What the command may take away is kept first; a failed snapshot is said, not fatal: the
    // attic is recovery, the guard is containment.
    let line: String = command.chars().take(200).collect();
    let kept = cx
        .attic()
        .map(|attic| attic.snapshot(&format!("before bash: {line}")));
    let bound = plan.as_ref().map(|plan| (plan, made.as_slice()));
    let ran = match run(command, cx, timeout, network, &opened, bound) {
        Ok(r) => r,
        Err(stale @ (Spawn::Changed(_) | Spawn::Unreadable(_))) => {
            // Nothing ran: the files made for it go, and nothing is restored over the world.
            for path in &made {
                if fs::metadata(path).is_ok_and(|m| m.len() == 0) {
                    let _ = fs::remove_file(path);
                }
            }
            return match (stale, plan) {
                (Spawn::Changed(now), Some(approved)) => Attempt::Changed { approved, now },
                (Spawn::Unreadable(e), _) => Attempt::Failed(Outcome::error(format!(
                    "the world could not be read again before the spawn, so the approved command did not run: {e}"
                ))),
                _ => Attempt::Failed(Outcome::error("the command did not run")),
            };
        }
        Err(Spawn::Io(e)) => {
            return Attempt::Failed(Outcome::error(format!("could not start bash: {e}")));
        }
    };
    // A command that failed leaves no empty file behind it.
    if ran.timed_out || ran.interrupted || ran.code != Some(0) {
        for path in &made {
            if fs::metadata(path).is_ok_and(|m| m.len() == 0) {
                let _ = fs::remove_file(path);
            }
        }
    }
    // The command's whole process group is dead: nothing can race the restore.
    if class == ToolClass::Destructive {
        before.release_deleted_repository();
    }
    let tripped = before.restore();
    let attic_note = attic_note(cx, kept.as_ref(), class);
    let mut out = Outcome::ok(ran.render(), class);
    out.is_error = ran.timed_out || ran.interrupted || ran.code != Some(0);
    out.meta.exit = ran.code;
    out.meta.timed_out = ran.timed_out;
    // The journal keeps, with the act, the plan the human approved it against.
    if let Some(plan) = &plan {
        let _ = write!(
            out.content,
            "\n[approved by the human against {}, unchanged at the spawn]",
            plan.short()
        );
    }
    if let Some(note) = attic_note {
        let _ = write!(out.content, "\n[{note}]");
    }
    if !tripped.is_empty() {
        out.is_error = true;
        out.content.push_str("\n[the command changed protected files — git's executable configuration or the world's records; ");
        out.content.push_str(&tripped.join("; "));
        out.content.push(']');
    }
    Attempt::Ran(out)
}

/// Whether the command may run, and at what class: classified, shelved, asked when it must be.
/// Returns the class, the outside paths it named and the plan an approval was bound to, or the
/// refusal to give the body. `changed` is what made an earlier approval of the same command stale.
fn admit(command: &str, cx: &Context, changed: &[String]) -> Result<Admitted, Outcome> {
    let place = Place {
        world: cx.world().to_path_buf(),
        home: cx.home().to_path_buf(),
        cwd: cx.world().to_path_buf(),
        roots: cx.deploy_roots().to_vec(),
    };
    let (verdict, acts, pushes) = classify_reach(command, &place);
    let (class, why, outside, asked) = match verdict {
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
            return Err(Outcome::error(format!(
                "refused by guard rule `{}`: {reason}{hint}",
                rule.name()
            )));
        }
        Verdict::Run {
            class,
            why,
            outside,
        } => (class, why, outside, false),
    };
    check_class(cx.race(), class).map_err(Outcome::unheld)?;
    // A Kijin does only its post's outward acts: anything else is refused before the gate.
    if cx.race().base() == Race::Kijin {
        check_post(cx.body(), &acts).map_err(|e| Outcome::error(e.to_string()))?;
    }
    if !(asked || matches!(class, ToolClass::Outward | ToolClass::Destructive)) {
        return Ok(Admitted {
            class,
            outside,
            plan: None,
        });
    }
    let mut why = why;
    // A push goes where its remote's configuration says: resolved now, shown on the card, and
    // no post's act unless it pushes exactly where it fetches from.
    for push in &pushes {
        let url = crate::fingerprint::push_url(&push.dir, &push.remote).map_err(|e| {
            let refused = check_post(cx.body(), &[OutwardKind::Other])
                .err()
                .map_or_else(String::new, |r| format!("{r}: "));
            Outcome::error(format!("{refused}{e}"))
        })?;
        why.push(format!(
            "pushes to {} (remote `{}`)",
            crate::fingerprint::shown_url(&url),
            push.remote
        ));
    }
    if !changed.is_empty() {
        why.insert(
            0,
            format!(
                "the world changed since approval — {}; approving again binds to the world as it is now",
                changed.join("; ")
            ),
        );
    }
    // The plan is taken before the human is asked: what they approve is the world they were shown.
    // A plan that cannot be read is no plan: nothing is put to the human, and nothing runs.
    let plan = Fingerprint::of(command, cx.world()).map_err(|e| {
        Outcome::error(format!(
            "the plan this {class} command would be approved against could not be read, so the gate was not asked and the command did not run: {e}"
        ))
    })?;
    let ask = Ask {
        body: cx.body().to_owned(),
        race: cx.race(),
        class,
        act: command.to_owned(),
        why,
        outside: outside.clone(),
        fingerprint: Some(plan.clone()),
    };
    if let Answer::Deny(reason) = cx.gate().ask(&ask) {
        let mut denied = Outcome::error(format!(
            "the human gate did not approve this {class} command ({}): {reason}",
            plan.short()
        ));
        denied.meta.denied = true;
        return Err(denied);
    }
    Ok(Admitted {
        class,
        outside,
        plan: Some(plan),
    })
}

/// What the attic says after a command: the files it deleted and where they are kept, a
/// repository put back — or that no snapshot could be kept.
fn attic_note(
    cx: &Context,
    kept: Option<&Result<crate::attic::Snapshot, String>>,
    class: ToolClass,
) -> Option<String> {
    let attic = cx.attic()?;
    match kept? {
        Ok(snap) => {
            let mut notes = Vec::new();
            let lost = attic.lost_since(snap);
            if !lost.is_empty() {
                notes.push(format!(
                    "deleted {} nested repositor{} — NOT kept in the attic, which holds only their commit's name: {}",
                    lost.len(),
                    if lost.len() == 1 { "y" } else { "ies" },
                    lost.join(", ")
                ));
            }
            let gone = attic.deleted_since(snap);
            if !gone.is_empty() {
                let first: Vec<&str> = gone.iter().take(5).map(String::as_str).collect();
                notes.push(format!(
                    "deleted {} file(s) — kept in the attic, snapshot {}: {}{}",
                    gone.len(),
                    &snap.tree[..12.min(snap.tree.len())],
                    first.join(", "),
                    if gone.len() > 5 { ", …" } else { "" }
                ));
            }
            if !snap.skipped.is_empty() {
                notes.push(format!(
                    "{} file(s) over {} MiB were not kept in the attic: {}",
                    snap.skipped.len(),
                    crate::attic::MAX_FILE_BYTES / (1024 * 1024),
                    snap.skipped
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            // A destructive act the human approved is the human's: the attic keeps it, and does
            // not undo it.
            if class != ToolClass::Destructive {
                notes.extend(attic.repair_git());
            }
            (!notes.is_empty()).then(|| notes.join("; "))
        }
        Err(e) => Some(format!(
            "the attic could not keep a snapshot before this command: {e}"
        )),
    }
}

/// The sandbox can only name what exists, and opening a new file's folder would open everything
/// beside it: an approved file that does not exist yet is made here, empty, so that file alone
/// is opened. Returns the files it made.
fn make_approved(opened: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut made = Vec::new();
    for path in opened {
        if fs::symlink_metadata(path).is_err() {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| {
                    format!(
                        "could not prepare the approved path `{}`: {e}; nothing ran",
                        path.display()
                    )
                })?;
            made.push(path.clone());
        }
    }
    Ok(made)
}

struct Ran {
    stdout: Captured,
    stderr: Captured,
    code: Option<i32>,
    timed_out: bool,
    interrupted: bool,
    waited: Duration,
}

#[derive(Default)]
struct Captured {
    bytes: Vec<u8>,
    dropped: usize,
}

impl Ran {
    fn render(&self) -> String {
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
        if self.interrupted {
            s.push_str(
                "\n[interrupted by the human: the command and everything it started were stopped]",
            );
        } else if self.timed_out {
            let _ = write!(
                s,
                "\n[killed after {:.1}s: the command and everything it started]",
                self.waited.as_secs_f32()
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

/// Why a command did not start.
enum Spawn {
    /// The world no longer matched the approved plan: this is what it is now.
    Changed(Fingerprint),
    /// The world could not be read again: no plan to compare, so nothing runs.
    Unreadable(String),
    /// The process could not be started.
    Io(std::io::Error),
}

impl From<std::io::Error> for Spawn {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Run `command` in the context's sandbox: the world writable, a private temporary directory
/// inside it, the network open only for an approved outward or destructive act. An approved
/// command runs only if the world still matches the approved plan at the moment of the spawn,
/// the files the harness made for it read as the absence they were when it was approved.
fn run(
    command: &str,
    cx: &Context,
    timeout: Duration,
    network: bool,
    opened: &[PathBuf],
    approved: Option<(&Fingerprint, &[PathBuf])>,
) -> Result<Ran, Spawn> {
    let tmp = cx.scratch();
    std::fs::create_dir_all(&tmp)?;
    let mut policy = Policy::new(cx.world(), cx.home(), &tmp, network);
    // A commissioned creature writes what its soil admits; the rest of the world it only reads.
    let grants = cx.grants();
    if grants != [cx.world().to_path_buf()] {
        policy.write.retain(|p| p != cx.world());
        policy.write.extend(grants);
        policy.read.push(cx.world().to_path_buf());
    }
    // Each approved path exists by now and opens alone: never its folder.
    policy.write.extend(opened.iter().cloned());
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
        // Who runs it: a body's own `mnemorph q` refuses rather than act as the human.
        .env(
            "MNEMORPH_BODY",
            if cx.body().is_empty() {
                "a body"
            } else {
                cx.body()
            },
        )
        .env("TMPDIR", &tmp);
    // The recheck is the last thing before the spawn: everything else is already prepared, so
    // the window in which the world can change unseen is the recheck's own reading of it.
    if let Some((approved, made)) = approved {
        let now = Fingerprint::of_except(command, cx.world(), made).map_err(Spawn::Unreadable)?;
        if now != *approved {
            return Err(Spawn::Changed(now));
        }
    }
    let mut child = cmd.spawn()?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    // The call's own limit, or the run's clock, whichever comes first.
    let deadline = cx.deadline().map_or(Instant::now() + timeout, |d| {
        d.min(Instant::now() + timeout)
    });
    let started = Instant::now();
    let mut timed_out = false;
    let mut interrupted = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            timed_out = true;
            kill_group(&child);
            break child.wait().ok();
        }
        // Esc: the whole group stops within one poll, and what it printed so far is kept.
        if cx.cancelled() {
            interrupted = true;
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
        interrupted,
        waited: started.elapsed(),
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
#[must_use]
pub fn scrubbed_env() -> Vec<(String, String)> {
    std::env::vars().filter(|(k, v)| !is_secret(k, v)).collect()
}

fn is_secret(name: &str, value: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_WORDS.iter().any(|w| upper.contains(w))
        || upper.split('_').any(|part| SECRET_PARTS.contains(&part))
        || has_userinfo(value)
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
            ("GIT_AUTHOR_NAME", "me"),
        ] {
            assert!(!is_secret(name, value), "{name}={value}");
        }
    }
}
