//! The Kijin's receipt: an outward act is done when a read-only probe sees its effect, never
//! because its command exited 0.
//!
//! An act moves through its post journal (`.mnemorph/kijin/<post>/post.jsonl`) as rows, one per
//! state: `committed` (approved, its target pinned, about to run), then `pending observation`
//! (it ran: its exit and what it left behind), then `observed` (a probe saw the effect the plan
//! promised) or `reconcile` (the probe saw something else, failed, timed out, or could not
//! tell). An untyped [`Act::Command`] has no probe: its receipt is `unobserved`, said plainly.
//!
//! The probe is the harness's, never the model's: a fixed argv per typed kind, its arguments the
//! act's own checked fields and the target pinned when the act was committed — the push URLs git
//! resolved then, the commit HEAD named then, the `gh` host then — never read again from
//! configuration a body can write. It runs outside every repository, with no input, under a
//! deadline, and inside the kernel guard when one is in force: network open, nothing writable
//! but its own empty scratch directory.
//!
//! An act cut between `committed` and its probe (a crash, a kill, a resumed run) is never
//! replayed: the act runner ([`crate::Runner`]) probes it before its post's next act and moves it
//! to `observed` (it landed), `absent` (it never did: the same request is asked again as a new
//! act) or `reconcile` (the probe cannot tell).
//!
//! A `reconcile` is shown to the human (`mnemorph status`) until a later row settles the act.
//! It is the event M11 folds into `outward_clean_rate`, and where it suspends the envelope that
//! covered the act and may make an Oni fall ([`Tally`]).

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mnemorph_ledger::{Journal, Timestamp};
use mnemorph_races::Post;
use mnemorph_tools::shell::scrubbed_env;
use mnemorph_tools::{Policy, Sandbox, kijin_witness};
use rustix::process::{Pid, Signal, kill_process_group};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::outward::sha_ok;
use crate::{Act, Done, Planned};

/// How long a probe may run before it is killed and the act goes to `reconcile`.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
/// The most of a probe's output, in bytes, the journal keeps.
pub const SEEN_BYTES: usize = 1024;
/// The most of a probe's output, in bytes, read for judging; past it the probe cannot tell.
const READ_BYTES: usize = 256 * 1024;
/// The longest command the journal keeps, in characters.
const COMMAND_CHARS: usize = 512;
/// The `gh` host when the environment names none.
pub(crate) const DEFAULT_HOST: &str = "github.com";
/// Transports a push probe may use: every other, a remote helper (`ext::`) above all, is refused.
const PROTOCOLS: &[&str] = &["https", "http", "ssh", "git", "file"];
/// Credentials a probe needs to read the outside as the act wrote it: read, never written.
const PROBE_CREDENTIALS: &[&str] = &[
    ".ssh",
    ".netrc",
    ".git-credentials",
    ".config/git/credentials",
    ".config/gh",
];

/// Where an act stands in its post journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum State {
    /// Approved and pinned; about to run.
    #[serde(rename = "committed")]
    Committed,
    /// It ran; its effect is not yet seen.
    #[serde(rename = "pending observation")]
    PendingObservation,
    /// A probe saw the effect the plan promised.
    #[serde(rename = "observed")]
    Observed,
    /// A probe saw something else, failed, timed out, or could not tell: the human looks.
    #[serde(rename = "reconcile")]
    Reconcile,
    /// A command, which has no probe: its effect is not observed.
    #[serde(rename = "unobserved")]
    Unobserved,
    /// A cut act the probe found never landed: nothing of it is outside, and the same request is
    /// asked again as a new act.
    #[serde(rename = "absent")]
    Absent,
    /// An act the harness put back: its undo's probe saw the effect gone. The row supersedes the
    /// act's earlier ones, appended, never edited, and names the undo (`by`).
    #[serde(rename = "corrected")]
    Corrected,
    /// A `reconcile` the human settled as no incident: nothing is undone, and it leaves their list.
    #[serde(rename = "settled")]
    Settled,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            State::Committed => "committed",
            State::PendingObservation => "pending observation",
            State::Observed => "observed",
            State::Reconcile => "reconcile",
            State::Unobserved => "unobserved",
            State::Absent => "absent",
            State::Corrected => "corrected",
            State::Settled => "settled",
        })
    }
}

/// Where a typed act lands, read when it is committed. A probe reads this, never the
/// configuration again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pin {
    /// A push: every URL git pushes the remote to, `insteadOf` and `pushInsteadOf` applied, and
    /// the commit HEAD named — the commit the push sends.
    Push {
        /// The push URLs.
        urls: Vec<String>,
        /// The commit, its full hex object name.
        sha: String,
    },
    /// A `gh` act: the host it acts on. The repository is pinned in the act itself.
    Forge {
        /// The host, `github.com` unless `GH_HOST` named another.
        host: String,
    },
    /// A command: nothing is probed.
    Unpinned,
}

impl Pin {
    /// What the journal says of the target: URLs with any credentials in them blanked.
    fn shown(&self) -> Vec<String> {
        match self {
            Pin::Push { urls, sha } => urls
                .iter()
                .map(|u| redact(u))
                .chain([format!("commit {sha}")])
                .collect(),
            Pin::Forge { host } => vec![format!("host {host}")],
            Pin::Unpinned => Vec::new(),
        }
    }
}

/// A word git and the probe read as one plain argument: no control character or space, not an
/// option, not too long.
fn url_ok(url: &str) -> bool {
    !url.is_empty()
        && url.len() <= 2048
        && !url.starts_with('-')
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// A host as `gh` takes it: letters, digits, `.`, `-`, and a port.
fn host_ok(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with(['-', '.'])
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':'))
}

/// Git in the world, read-only and deaf to what it could run, in the environment the act gets.
pub(crate) fn world_git(world: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args([
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(world)
        .env_clear()
        .envs(scrubbed_env())
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            redact(String::from_utf8_lossy(&out.stderr).trim())
        ))
    }
}

impl Planned {
    /// Pin where this act lands, read in `world` now. Read it with the plan the human approves:
    /// the approval's fingerprint binds the remote's push URLs, the refs and `GH_HOST` until the
    /// spawn, so what is pinned is where the act went.
    ///
    /// # Errors
    ///
    /// The target cannot be read, or is not one a probe can reach plainly: a typed act whose
    /// target cannot be pinned does not run, since its receipt could never be observed. What the
    /// act would make, already there, is refused too, read through `probe`: a branch on any push
    /// URL, a label on the pull request. The command would exit 0 doing nothing, the probe would
    /// see what someone else made, and the undo would remove it.
    pub fn pin(&self, world: &Path, probe: &dyn Probe) -> Result<Pin, String> {
        let pin = self.aim(world)?;
        self.vacant(&pin, probe)?;
        Ok(pin)
    }

    /// Where this act lands, read in `world` now without reaching outside: the remote's push URLs
    /// and the commit HEAD names, or the `gh` host.
    pub(crate) fn aim(&self, world: &Path) -> Result<Pin, String> {
        match self.act() {
            Act::PushNewBranch { remote, .. } => {
                let urls: Vec<String> =
                    world_git(world, &["remote", "get-url", "--push", "--all", remote])?
                        .lines()
                        .map(str::to_owned)
                        .filter(|l| !l.is_empty())
                        .collect();
                if urls.is_empty() {
                    return Err(format!("remote `{remote}` has no push URL"));
                }
                if let Some(bad) = urls.iter().find(|u| !url_ok(u)) {
                    return Err(format!(
                        "remote `{remote}` pushes to `{}`, which a probe cannot read plainly",
                        redact(bad)
                    ));
                }
                let sha = world_git(world, &["rev-parse", "-q", "--verify", "HEAD^{commit}"])?
                    .trim()
                    .to_owned();
                if !sha_ok(&sha) {
                    return Err("HEAD names no commit to push".into());
                }
                Ok(Pin::Push { urls, sha })
            }
            Act::AddLabel { .. } | Act::Comment { .. } => {
                let host = scrubbed_env()
                    .into_iter()
                    .find(|(k, _)| k == "GH_HOST")
                    .map_or_else(|| DEFAULT_HOST.to_owned(), |(_, v)| v);
                if !host_ok(&host) {
                    return Err(format!("GH_HOST `{host}` is not a plain host name"));
                }
                Ok(Pin::Forge { host })
            }
            Act::Command(_) => Ok(Pin::Unpinned),
        }
    }

    /// Whether what the act would make is still absent at `pin`, read through `probe`.
    pub(crate) fn vacant(&self, pin: &Pin, probe: &dyn Probe) -> Result<(), String> {
        match (self.act(), pin) {
            (Act::PushNewBranch { name, .. }, Pin::Push { urls, .. }) => {
                // A branch already there is refused: the push would exit 0 ("Everything
                // up-to-date") when it is at HEAD's commit, the probe would see it, and the undo
                // would delete a branch the act never made.
                for url in urls {
                    let shown = redact(url);
                    let seen = probe
                        .run(&ls_remote_argv(url, name), PROBE_TIMEOUT)
                        .map_err(|e| format!("{shown} cannot be read: {}", redact(&e)))?;
                    if seen.code != Some(0) || seen.cut {
                        return Err(format!(
                            "{shown} cannot be read: {}",
                            excerpt(&seen.err, 200)
                        ));
                    }
                    if !branch_tips(&seen.out, name).is_empty() {
                        return Err(format!(
                            "branch {name} is already on {shown}: nothing to push, nothing to undo"
                        ));
                    }
                }
                Ok(())
            }
            (Act::AddLabel { repo, pr, label }, Pin::Forge { host }) => {
                let argv = labels_argv(repo, *pr, host);
                let seen = probe
                    .run(&argv, PROBE_TIMEOUT)
                    .map_err(|e| format!("the labels of {repo}#{pr} cannot be read: {e}"))?;
                if seen.code != Some(0) || seen.cut {
                    return Err(format!(
                        "the labels of {repo}#{pr} cannot be read: {}",
                        excerpt(&seen.err, 200)
                    ));
                }
                if carries(&seen.out, label)? {
                    return Err(format!(
                        "label {label:?} is already on {repo}#{pr}: nothing to add, nothing to undo"
                    ));
                }
                Ok(())
            }
            (Act::Comment { .. }, Pin::Forge { .. }) | (Act::Command(_), Pin::Unpinned) => Ok(()),
            _ => Err("the pin is not of this act".into()),
        }
    }
}

/// The commits `refs/heads/<name>` names in what `git ls-remote` printed.
pub(crate) fn branch_tips(out: &[u8], name: &str) -> Vec<String> {
    let want = format!("refs/heads/{name}");
    String::from_utf8_lossy(out)
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(_, r)| *r == want)
        .map(|(s, _)| s.to_owned())
        .collect()
}

/// What a probe printed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    /// Its exit code; `None` when a signal ended it.
    pub code: Option<i32>,
    /// Its standard output, at most `READ_BYTES` of it.
    pub out: Vec<u8>,
    /// Its standard error, as much.
    pub err: Vec<u8>,
    /// Whether the output went past what is read.
    pub cut: bool,
}

/// What runs a probe's argv. The harness's is [`Observer`]; a test may stand in its own.
pub trait Probe {
    /// Run `argv` — its program first — within `timeout`.
    ///
    /// # Errors
    ///
    /// It could not start, or did not end in time (it is killed).
    fn run(&self, argv: &[String], timeout: Duration) -> Result<Seen, String>;
}

/// The harness's probe runner: no input, no shell, outside every repository, a deadline, and the
/// kernel guard when one is in force — network open, nothing writable but its scratch directory,
/// the credentials a read needs readable.
#[derive(Debug, Clone)]
pub struct Observer {
    sandbox: Sandbox,
    home: PathBuf,
}

impl Observer {
    /// A runner contained by `sandbox`, for the human whose home is `home`.
    #[must_use]
    pub fn new(sandbox: Sandbox, home: impl Into<PathBuf>) -> Self {
        Self {
            sandbox,
            home: home.into(),
        }
    }
}

/// A fresh, private, empty directory for one probe, outside every repository.
fn scratch() -> io::Result<PathBuf> {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "mnemorph-probe-{}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    fs::DirBuilder::new().mode(0o700).create(&dir).map(|()| dir)
}

/// The environment a probe gets: the act's, without anything that steers git or `gh` elsewhere,
/// and with every prompt off.
fn probe_env(scratch: &Path) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = scrubbed_env()
        .into_iter()
        .filter(|(k, _)| !k.starts_with("GIT_") && (!k.starts_with("GH_") || k == "GH_CONFIG_DIR"))
        .collect();
    let parent = scratch.parent().unwrap_or(scratch);
    env.extend(
        [
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GH_PROMPT_DISABLED", "1"),
            ("GH_NO_UPDATE_NOTIFIER", "1"),
            ("NO_COLOR", "1"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned())),
    );
    env.push((
        "GIT_CEILING_DIRECTORIES".into(),
        parent.to_string_lossy().into_owned(),
    ));
    env
}

impl Probe for Observer {
    fn run(&self, argv: &[String], timeout: Duration) -> Result<Seen, String> {
        let (program, rest) = argv.split_first().ok_or("the probe names no program")?;
        let dir = scratch().map_err(|e| format!("the probe's scratch directory: {e}"))?;
        let ran = (|| {
            let mut policy = Policy::new(&dir, &self.home, &dir, true);
            let credentials: Vec<PathBuf> = PROBE_CREDENTIALS
                .iter()
                .map(|c| self.home.join(c))
                .collect();
            policy.hidden.retain(|h| !credentials.contains(h));
            policy.read.extend(credentials);
            let words: Vec<&str> = rest.iter().map(String::as_str).collect();
            let mut cmd = self.sandbox.command(&policy, program, &words);
            cmd.current_dir(&dir)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0)
                .env_clear()
                .envs(probe_env(&dir));
            let mut child = cmd.spawn().map_err(|e| format!("{program}: {e}"))?;
            let out = drain(child.stdout.take());
            let err = drain(child.stderr.take());
            let deadline = Instant::now() + timeout;
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) if Instant::now() >= deadline => {
                        kill_group(&child);
                        let _ = child.wait();
                        return Err(format!(
                            "timed out after {:.1}s and was killed",
                            timeout.as_secs_f32()
                        ));
                    }
                    Ok(None) => thread::sleep(Duration::from_millis(10)),
                    Err(e) => {
                        kill_group(&child);
                        return Err(format!("{program}: {e}"));
                    }
                }
            };
            kill_group(&child);
            let wait = Duration::from_millis(500);
            let (out, cut_out) = out.recv_timeout(wait).unwrap_or_default();
            let (err, _) = err.recv_timeout(wait).unwrap_or_default();
            Ok(Seen {
                code: status.code(),
                out,
                err,
                cut: cut_out,
            })
        })();
        let _ = fs::remove_dir_all(&dir);
        ran
    }
}

fn kill_group(child: &Child) {
    // The group may be gone already: that is the outcome wanted.
    let _ = kill_process_group(Pid::from_child(child), Signal::KILL);
}

/// Read a pipe to its end on a thread, keeping the first `READ_BYTES`; whether more came.
fn drain(pipe: Option<impl io::Read + Send + 'static>) -> mpsc::Receiver<(Vec<u8>, bool)> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut cut = false;
        if let Some(mut pipe) = pipe {
            let mut buf = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let room = READ_BYTES.saturating_sub(kept.len());
                kept.extend_from_slice(&buf[..n.min(room)]);
                cut |= n > room;
            }
        }
        let _ = tx.send((kept, cut));
    });
    rx
}

/// `scheme://user:secret@host` with its credentials blanked, wherever it sits in `text`.
#[must_use]
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| {
                c == '/' || c == '?' || c == '#' || c.is_whitespace() || "'\"".contains(c)
            })
            .unwrap_or(tail.len());
        match tail[..end].rfind('@') {
            Some(user) => {
                out.push_str("***");
                out.push_str(&tail[user..end]);
            }
            None => out.push_str(&tail[..end]),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// At most `max` bytes of `bytes` as one escaped line: control characters (line breaks too)
/// written as escapes, credentials in URLs blanked, what was left out counted.
#[must_use]
pub fn excerpt(bytes: &[u8], max: usize) -> String {
    let text = redact(&String::from_utf8_lossy(bytes));
    let mut out = String::new();
    let mut taken = 0;
    for c in text.chars() {
        if out.len() >= max {
            break;
        }
        if c.is_control() {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
        taken += c.len_utf8();
    }
    if taken < text.len() {
        let _ = write!(out, " … {} more bytes", text.len() - taken);
    }
    out
}

/// The argv a probe runs: the act's own checked fields and its pinned target in fixed places.
fn probe_argv(act: &Act, pin: &Pin, done: &Done, url: Option<&str>) -> Option<Vec<String>> {
    let words = |w: &[&str]| w.iter().map(|&w| w.to_owned()).collect::<Vec<_>>();
    match (act, pin, done) {
        (Act::PushNewBranch { name, .. }, Pin::Push { .. }, Done::Pushed { .. }) => {
            Some(ls_remote_argv(url?, name))
        }
        (Act::AddLabel { repo, pr, .. }, Pin::Forge { host }, Done::Labelled) => {
            Some(labels_argv(repo, *pr, host))
        }
        (Act::Comment { repo, .. }, Pin::Forge { host }, Done::Commented { id }) => {
            let mut argv = words(&["gh", "api", "--hostname", host, "--method", "GET"]);
            argv.push(format!("repos/{repo}/issues/comments/{id}"));
            Some(argv)
        }
        _ => None,
    }
}

/// The read of a branch on a remote, at a URL: no transport but `PROTOCOLS`.
pub(crate) fn ls_remote_argv(url: &str, name: &str) -> Vec<String> {
    let mut argv: Vec<String> = ["git", "-c", "protocol.allow=never"]
        .map(str::to_owned)
        .to_vec();
    for p in PROTOCOLS {
        argv.push("-c".into());
        argv.push(format!("protocol.{p}.allow=always"));
    }
    argv.extend(["ls-remote", "--end-of-options", url].map(str::to_owned));
    argv.push(format!("refs/heads/{name}"));
    argv
}

/// The read of a pull request's labels: at most 100, all a pull request may carry.
pub(crate) fn labels_argv(repo: &str, pr: u64, host: &str) -> Vec<String> {
    [
        "gh",
        "api",
        "--hostname",
        host,
        "--method",
        "GET",
        &format!("repos/{repo}/issues/{pr}/labels?per_page=100"),
    ]
    .map(str::to_owned)
    .to_vec()
}

/// Whether the labels `gh api` printed include `label`, as GitHub compares them: ignoring case.
pub(crate) fn carries(out: &[u8], label: &str) -> Result<bool, String> {
    #[derive(Deserialize)]
    struct Label {
        name: String,
    }
    let labels: Vec<Label> =
        serde_json::from_slice(out).map_err(|e| format!("the labels do not read: {e}"))?;
    let want = label.to_lowercase();
    Ok(labels.iter().any(|l| l.name.to_lowercase() == want))
}

pub(crate) fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// What the plan promised, as the journal says it.
fn expected(act: &Act, pin: &Pin, done: Option<&Done>) -> String {
    match (act, pin, done) {
        (Act::PushNewBranch { name, .. }, pin, _) => match pin {
            Pin::Push { sha, .. } => format!("refs/heads/{name} at {sha}"),
            _ => format!("refs/heads/{name}, at a commit not pinned"),
        },
        (Act::AddLabel { repo, pr, label }, _, _) => format!("label {label:?} on {repo}#{pr}"),
        (Act::Comment { repo, pr, body }, _, Some(Done::Commented { id })) => {
            format!("comment {id} on {repo}#{pr}, body sha256 {}", sha256(body))
        }
        (Act::Comment { repo, pr, body }, _, _) => {
            format!("a comment on {repo}#{pr}, body sha256 {}", sha256(body))
        }
        (Act::Command(_), _, _) => "nothing: a command has no probe".into(),
    }
}

/// Whether a probe's output shows the effect the plan promised; why not, when it does not.
fn judge(act: &Act, pin: &Pin, seen: &Seen) -> Result<(), String> {
    if seen.code != Some(0) {
        let err = excerpt(&seen.err, 200);
        return Err(match seen.code {
            Some(c) => format!("the probe exited {c}: {err}"),
            None => format!("a signal ended the probe: {err}"),
        });
    }
    if seen.cut {
        return Err(format!(
            "the probe printed more than {READ_BYTES} bytes: it cannot tell"
        ));
    }
    let text = std::str::from_utf8(&seen.out).map_err(|_| "the probe's output is not UTF-8")?;
    match (act, pin) {
        (Act::PushNewBranch { name, .. }, Pin::Push { sha, .. }) => {
            let want = format!("refs/heads/{name}");
            let found: Vec<&str> = text
                .lines()
                .filter_map(|l| l.split_once('\t'))
                .filter(|(_, r)| *r == want)
                .map(|(s, _)| s)
                .collect();
            match found.as_slice() {
                [] => Err(format!("{want} is not on the remote")),
                [one] if one == sha => Ok(()),
                [one] if sha_ok(one) => Err(format!("{want} is at {one}, not the pushed {sha}")),
                [_] => Err(format!(
                    "{want} is at an object name that does not read, not the pushed {sha}"
                )),
                _ => Err(format!("the remote lists {want} more than once")),
            }
        }
        (Act::AddLabel { label, .. }, Pin::Forge { .. }) => {
            if carries(text.as_bytes(), label)? {
                Ok(())
            } else {
                Err(format!("the pull request does not carry label {label:?}"))
            }
        }
        (Act::Comment { repo, pr, body }, Pin::Forge { .. }) => {
            #[derive(Deserialize)]
            struct Comment {
                body: String,
                issue_url: String,
            }
            let c: Comment = serde_json::from_str(text)
                .map_err(|e| format!("the comment does not read: {e}"))?;
            let on = format!("/repos/{repo}/issues/{pr}").to_ascii_lowercase();
            if !c.issue_url.to_ascii_lowercase().ends_with(&on) {
                return Err(format!(
                    "the comment is on {}, not {repo}#{pr}",
                    excerpt(c.issue_url.as_bytes(), 200)
                ));
            }
            let (want, got) = (sha256(body), sha256(&c.body));
            if want == got {
                Ok(())
            } else {
                Err(format!("the comment's body sha256 is {got}, not {want}"))
            }
        }
        _ => Err("the pin is not of this act".into()),
    }
}

/// One row of a post journal: one state of one act.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// The act's receipt id.
    pub act: String,
    /// The state it entered.
    pub state: State,
    /// When.
    pub at: Timestamp,
    /// The command, bounded and escaped; an untyped one only by its program and digest (on
    /// `committed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Where it lands, credentials blanked (on `committed`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target: Vec<String>,
    /// The plan the human approved it against (on `committed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// The command's exit code (on `pending observation` and `unobserved`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    /// What the plan promised the probe would see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<String>,
    /// The probe that ran, credentials blanked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<String>,
    /// What it printed, bounded to [`SEEN_BYTES`] and escaped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen: Option<String>,
    /// Why the act is in this state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// On an undo's rows: the receipt of the act it undoes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undoes: Option<String>,
    /// On a `corrected` row: the receipt of the undo whose probe saw the effect gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

impl Row {
    pub(crate) fn new(act: &str, state: State) -> Self {
        Self {
            act: act.to_owned(),
            state,
            at: Timestamp::now(),
            command: None,
            target: Vec::new(),
            plan: None,
            exit: None,
            expect: None,
            probe: None,
            seen: None,
            why: None,
            undoes: None,
            by: None,
        }
    }
}

/// An act on its way through the post journal, as the harness holds it between rows.
#[derive(Debug, Clone)]
pub struct Receipt {
    pub(crate) id: String,
    pub(crate) planned: Planned,
    pub(crate) pin: Pin,
    pub(crate) done: Option<Done>,
    pub(crate) state: State,
}

impl Receipt {
    /// The receipt's id in the post journal.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Where the act stands.
    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// What the act left behind, as the harness recorded it: what its undo needs.
    #[must_use]
    pub fn done(&self) -> Option<&Done> {
        self.done.as_ref()
    }
}

/// A Kijin post's journal: `.mnemorph/kijin/<post>/post.jsonl`, append-only, written by the
/// harness alone, and read by the post's bodies. Every row is written first to a witness under
/// the human's state directory (`mnemorph_tools::kijin_witness`), out of every command's reach:
/// the tripwire rebuilds the journal from it after each command, and [`PostJournal::tally`] reads
/// the witness alone, so a row a body forges never counts.
#[derive(Debug, Clone)]
pub struct PostJournal {
    /// The post whose acts it journals: it commits no other.
    pub(crate) post: Post,
    /// The world it is in: where an incident's lesson is read.
    pub(crate) world: PathBuf,
    journal: Journal<Row>,
    pub(crate) witness: Option<Journal<Row>>,
}

/// A file beside the witness, private to the human: its folder `0700` (made so if need be), the
/// file `0600`, opened for appending (made if need be). Nothing in it is anyone else's to read.
pub(crate) fn private_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    if let Some(dir) = path.parent() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    let f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    f.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(f)
}

/// Hold a witness's lock: the file opened (made if need be, private to the human) and locked
/// whole until dropped. The tripwire rebuilds the journal under the same lock, so no row lands
/// between its read and its write.
pub(crate) fn lock(witness: &Path) -> io::Result<fs::File> {
    let f = private_file(witness)?;
    f.lock()?;
    Ok(f)
}

/// Where a post's journal lives in a world.
#[must_use]
pub fn post_journal(world: &Path, post: Post) -> PathBuf {
    world
        .join(".mnemorph/kijin")
        .join(post.creature())
        .join("post.jsonl")
}

pub(crate) fn receipt_id(command: &str) -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    sha256(&format!(
        "{nanos} {} {} {command}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))[..12]
        .to_owned()
}

/// The act's command as the journal keeps it.
pub(crate) fn command_line(command: &str) -> String {
    let short: String = command.chars().take(COMMAND_CHARS).collect();
    let mut line = excerpt(short.as_bytes(), COMMAND_CHARS * 4);
    if command.chars().count() > COMMAND_CHARS {
        line.push_str(" …");
    }
    line
}

impl PostJournal {
    /// The journal of `post` in `world`, witnessed under the human's state directory when there
    /// is one.
    #[must_use]
    pub fn of(world: &Path, post: Post) -> Self {
        Self {
            post,
            world: world.to_path_buf(),
            journal: Journal::new(post_journal(world, post)),
            witness: kijin_witness(world, post.creature()).map(Journal::new),
        }
    }

    /// The same journal, witnessed in `file` instead.
    #[must_use]
    pub fn witnessed_at(mut self, file: &Path) -> Self {
        self.witness = Some(Journal::new(file));
        self
    }

    /// Where its witness is.
    #[must_use]
    pub fn witness(&self) -> Option<&Path> {
        self.witness.as_ref().map(Journal::path)
    }

    /// One row: the witness first, under its lock — a row the journal holds and its witness does
    /// not would be dropped by the next command's tripwire — then the journal.
    pub(crate) fn append(&self, row: &Row) -> io::Result<()> {
        let held = match &self.witness {
            Some(w) => {
                let held = lock(w.path())?;
                w.append(row)?;
                Some(held)
            }
            None => None,
        };
        self.journal.append(row)?;
        drop(held);
        Ok(())
    }

    /// Where it lives.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.journal.path()
    }

    /// Journal an approved act as `committed`, its target pinned ([`Planned::pin`]), before it
    /// runs. `plan` is the approval's fingerprint, in its short form.
    ///
    /// # Errors
    ///
    /// The act is not this post's, the pin is not of this act, or the row cannot be written:
    /// the act must not run.
    pub fn commit(&self, planned: &Planned, pin: Pin, plan: Option<&str>) -> io::Result<Receipt> {
        self.commit_then(planned, pin, plan, &mut |_| Ok(()))
    }

    /// [`Self::commit`], with `first` run on the receipt's id before its row is written: the act
    /// runner keeps the request's key and intent there. Its failure writes no row.
    pub(crate) fn commit_then(
        &self,
        planned: &Planned,
        pin: Pin,
        plan: Option<&str>,
        first: &mut dyn FnMut(&str) -> io::Result<()>,
    ) -> io::Result<Receipt> {
        planned
            .check_post(self.post.creature())
            .map_err(|e| io::Error::other(e.to_string()))?;
        let fits = matches!(
            (planned.act(), &pin),
            (Act::PushNewBranch { .. }, Pin::Push { .. })
                | (
                    Act::AddLabel { .. } | Act::Comment { .. },
                    Pin::Forge { .. }
                )
                | (Act::Command(_), Pin::Unpinned)
        );
        if !fits {
            return Err(io::Error::other("the pin is not of this act"));
        }
        let id = receipt_id(planned.command());
        let mut row = Row::new(&id, State::Committed);
        // A typed act's command is the harness's own rendering of checked fields; an untyped
        // command is the model's text and may carry a secret, so the tracked journal keeps only
        // its program and its digest.
        row.command = Some(match planned.act() {
            Act::Command(c) => format!(
                "{} … (an untyped command, sha256 {})",
                c.split_whitespace()
                    .next()
                    .map_or("", |w| w)
                    .chars()
                    .take(64)
                    .collect::<String>(),
                sha256(c)
            ),
            _ => command_line(planned.command()),
        });
        row.target = pin.shown();
        row.plan = plan.map(str::to_owned);
        row.expect = Some(expected(planned.act(), &pin, None));
        first(&id)?;
        self.append(&row)?;
        Ok(Receipt {
            id,
            planned: planned.clone(),
            pin,
            done: None,
            state: State::Committed,
        })
    }

    /// Journal that the act ran, with its exit code and what it printed. A typed act that exited
    /// 0 moves to `pending observation`: only a probe says what it did. One that did not — or a
    /// signal ended — moves straight to `reconcile`, leaving nothing recorded: what it did
    /// outside is unknown, so it is neither probed (the outside may already hold what the plan
    /// promised, from before) nor undone without the human. A command moves to `unobserved`, and
    /// stays there.
    ///
    /// # Errors
    ///
    /// The receipt is not `committed`, or the row cannot be written.
    pub fn ran(&self, receipt: &mut Receipt, exit: Option<i32>, printed: &str) -> io::Result<()> {
        if receipt.state != State::Committed {
            return Err(io::Error::other(format!(
                "the act is {}, not committed",
                receipt.state
            )));
        }
        let act = receipt.planned.act();
        let failed = exit != Some(0);
        let done = match (act, &receipt.pin) {
            _ if failed => None,
            (Act::PushNewBranch { .. }, Pin::Push { sha, .. }) => {
                Some(Done::Pushed { sha: sha.clone() })
            }
            (Act::AddLabel { .. }, _) => Some(Done::Labelled),
            (Act::Comment { .. }, Pin::Forge { host }) => receipt.planned.commented(printed, host),
            _ => None,
        };
        let state = match act {
            Act::Command(_) => State::Unobserved,
            _ if failed => State::Reconcile,
            _ => State::PendingObservation,
        };
        let mut row = Row::new(&receipt.id, state);
        row.exit = exit;
        row.why = Some(match (state, &done) {
            (State::Unobserved, _) => {
                "a command has no probe: whatever its exit, its effect is not observed".into()
            }
            (State::Reconcile, _) => format!(
                "the act {}: what it did outside is unknown, so it is not probed, and nothing is undone without the human",
                exit.map_or_else(
                    || "was ended by a signal".to_owned(),
                    |c| format!("exited {c}")
                )
            ),
            (_, None) => "the act printed no comment id: nothing can be probed".into(),
            _ => "ran; its effect is not yet observed".into(),
        });
        self.append(&row)?;
        receipt.state = state;
        receipt.done = done;
        Ok(())
    }

    /// Probe a `pending observation` act through `probe` within `timeout`, and journal what it
    /// saw: `observed` only when every probe shows the effect the plan promised; `reconcile`
    /// when one shows anything else, fails, times out, or cannot be made.
    ///
    /// # Errors
    ///
    /// The receipt is not `pending observation`, or the row cannot be written.
    pub fn observe(
        &self,
        receipt: &mut Receipt,
        probe: &dyn Probe,
        timeout: Duration,
    ) -> io::Result<State> {
        if receipt.state != State::PendingObservation {
            return Err(io::Error::other(format!(
                "the act is {}, not pending observation",
                receipt.state
            )));
        }
        let act = receipt.planned.act();
        let mut row = Row::new(&receipt.id, State::Reconcile);
        row.expect = Some(expected(act, &receipt.pin, receipt.done.as_ref()));
        let urls: Vec<Option<&str>> = match &receipt.pin {
            Pin::Push { urls, .. } => urls.iter().map(|u| Some(u.as_str())).collect(),
            _ => vec![None],
        };
        let mut probes = Vec::new();
        let mut seen = Vec::new();
        let mut verdict: Result<(), String> = Ok(());
        for url in urls {
            let argv = receipt
                .done
                .as_ref()
                .and_then(|done| probe_argv(act, &receipt.pin, done, url));
            let Some(argv) = argv else {
                verdict = Err("nothing the act left can be probed".into());
                break;
            };
            probes.push(redact(&argv.join(" ")));
            let one = match probe.run(&argv, timeout) {
                Ok(s) => {
                    seen.push(excerpt(&s.out, SEEN_BYTES));
                    judge(act, &receipt.pin, &s)
                }
                Err(e) => Err(format!("the probe failed: {}", redact(&e))),
            };
            if let Err(why) = one {
                verdict = Err(why);
                break;
            }
        }
        if verdict.is_ok() && probes.is_empty() {
            verdict = Err("no probe ran".into());
        }
        row.probe = (!probes.is_empty()).then(|| probes.join(" ; "));
        row.seen = (!seen.is_empty()).then(|| excerpt(seen.join(" ; ").as_bytes(), SEEN_BYTES));
        let state = match verdict {
            Ok(()) => {
                row.why = Some("the probe saw the effect the plan promised".into());
                State::Observed
            }
            Err(why) => {
                row.why = Some(why);
                State::Reconcile
            }
        };
        row.state = state;
        self.append(&row)?;
        receipt.state = state;
        Ok(state)
    }

    /// Each act's latest row, by receipt id, in the order the acts were first journaled; and
    /// the numbers of lines that did not read. Read from the witness when it exists, else from
    /// the journal in the world.
    ///
    /// # Errors
    ///
    /// The file exists but cannot be read.
    pub fn standing(&self) -> io::Result<(Vec<Row>, Vec<usize>)> {
        match &self.witness {
            Some(w) if w.path().exists() => fold(w),
            _ => fold(&self.journal),
        }
    }

    /// The acts by where they stand, read from the witness alone: what M11 folds into
    /// `outward_clean_rate`. A world with no witness counts nothing.
    ///
    /// # Errors
    ///
    /// The witness exists but cannot be read.
    pub fn tally(&self) -> io::Result<Tally> {
        let mut t = Tally::default();
        let Some(witness) = &self.witness else {
            return Ok(t);
        };
        let (rows, _) = fold(witness)?;
        for row in rows {
            match row.state {
                State::Committed | State::PendingObservation => t.open += 1,
                State::Observed => t.observed += 1,
                State::Reconcile => t.reconcile += 1,
                State::Unobserved => t.unobserved += 1,
                State::Absent => t.absent += 1,
                State::Corrected => t.corrected += 1,
                State::Settled => t.settled += 1,
            }
        }
        Ok(t)
    }
}

/// Each act's latest row in `journal`, in the order the acts were first journaled; and the
/// numbers of lines that did not read.
pub(crate) fn fold(journal: &Journal<Row>) -> io::Result<(Vec<Row>, Vec<usize>)> {
    let back = journal.read()?;
    let mut order: Vec<String> = Vec::new();
    let mut last: BTreeMap<String, Row> = BTreeMap::new();
    for row in back.rows {
        if !last.contains_key(&row.act) {
            order.push(row.act.clone());
        }
        last.insert(row.act.clone(), row);
    }
    let rows = order
        .into_iter()
        .filter_map(|id| last.remove(&id))
        .collect();
    Ok((rows, back.skipped))
}

/// A post's acts by where they stand. A `reconcile` is not a clean outward act: M11 folds it into
/// `outward_clean_rate`, suspends the envelope that covered it and may make an Oni fall.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    /// Seen as planned.
    pub observed: u32,
    /// Waiting for the human.
    pub reconcile: u32,
    /// Commands: no probe.
    pub unobserved: u32,
    /// Committed or ran, not yet probed — a run cut short; never replayed.
    pub open: u32,
    /// Cut acts the probe found never landed.
    pub absent: u32,
    /// Acts put back by an undo whose probe saw the effect gone.
    pub corrected: u32,
    /// `reconcile`s the human settled as no incident.
    pub settled: u32,
}

/// One line per act any post of `world` holds in `reconcile`, for the human (`mnemorph status`),
/// and one per post journal that does not read whole.
#[must_use]
pub fn reconciling(world: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    for post in Post::ALL {
        let journal = PostJournal::of(world, post);
        match journal.standing() {
            Ok((rows, skipped)) => {
                for row in rows.iter().filter(|r| r.state == State::Reconcile) {
                    lines.push(format!(
                        "{} act {} is in reconcile since {}: {}",
                        post.creature(),
                        excerpt(row.act.as_bytes(), 64),
                        row.at,
                        excerpt(
                            row.why
                                .as_deref()
                                .unwrap_or("its effect was not seen")
                                .as_bytes(),
                            300
                        )
                    ));
                }
                if !skipped.is_empty() {
                    lines.push(format!(
                        "{} has {} unreadable line(s) ({skipped:?}): an act there may be unseen",
                        journal.path().display(),
                        skipped.len()
                    ));
                }
            }
            Err(e) => lines.push(format!("{}: {e}", journal.path().display())),
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{excerpt, redact};

    #[test]
    fn credentials_in_urls_are_blanked_and_output_is_one_bounded_line() {
        assert_eq!(
            redact("fatal: 'https://me:tok@github.com/a/b' and ssh://git@host/x"),
            "fatal: 'https://***@github.com/a/b' and ssh://***@host/x"
        );
        assert_eq!(redact("https://github.com/a@b"), "https://github.com/a@b");
        assert_eq!(excerpt(b"a\nb\x1b[1m", 100), "a\\nb\\u{1b}[1m");
        let long = excerpt(&[b'x'; 50], 10);
        assert!(long.starts_with("xxxxxxxxxx …"), "{long}");
        assert!(long.ends_with("40 more bytes"), "{long}");
    }
}
