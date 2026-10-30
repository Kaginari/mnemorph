//! Oni envelopes at the gate: whether a standing permission the human signed admits a typed
//! outward act without asking, read at the moment of use.
//!
//! The admission reads, in this order, and stops at the first that fails — the act then goes to
//! the human, the card saying why no envelope admitted it:
//!
//! 1. the body is an Oni: an ascended Kijin, by the race the roster gives it;
//! 2. the act is a typed act, not an undo, and classed `outward` — never `destructive`;
//! 3. the envelopes, read afresh from the human's global file alone — never a file in the world,
//!    which whatever runs there may write, nor a file git tracks, a link, a file another user
//!    owns, or one others may write or sit in a folder others may write;
//! 4. an envelope whose pattern reads the act and that is signed for it: for this world (its
//!    canonical path) and/or this forge repository (`owner/name`) — one naming neither admits
//!    nothing — and, when it names them, this post and this body;
//! 5. the compiled exclusions ([`excluded`]), whatever any envelope or `gate.approve` says: only a
//!    branch push, a pull request, a comment or a label; no deploy (every environment is
//!    `protected` until a typed deploy names its own); no outbound text above
//!    [`COVERED_TEXT`] bytes or holding what looks like a secret; no push or pull request whose
//!    diff touches CI, deploy or release configuration or a manifest — read only once an
//!    envelope could apply, so no remote is asked for an act no envelope reads;
//! 6. that envelope in force: not revoked, signed, not expired, and its uses so far, counted in
//!    the uses ledger outside the world, under its cap;
//! 7. that envelope not suspended by an open incident ([`crate::suspended`]).
//!
//! The runner reads it once to decide whether to ask, and again at the moment of use — after the
//! plan's recheck, the uses ledger held — and only that second reading admits: it records the use
//! and the act's intent under the same lock, so two acts never both take an envelope's last use.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_ledger::{Day, Held, Timestamp, UseRow, Uses};
use mnemorph_races::{OutwardKind, Race, ToolClass};

use crate::forge::{forge_get, probed};
use crate::outward::{COVERED_TEXT, envelope_reads, face_of};
use crate::receipt::{command_line, redact, world_git, world_git_with};
use crate::{Act, Config, Envelope, Layer, Paths, Pin, Planned, PostJournal, Probe};

/// Where the gate reads the Oni's envelopes and counts their uses.
#[derive(Debug, Clone)]
pub struct EnvelopeGate {
    paths: Paths,
    uses: Uses,
}

/// An act an envelope admitted: the envelope, and which of its uses the act is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admitted {
    /// The envelope.
    pub envelope: Envelope,
    /// Its use this act is, from 1.
    pub n: u64,
}

impl fmt::Display for Admitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.envelope.uses {
            Some(cap) => write!(
                f,
                "admitted by envelope {} (use {}/{cap})",
                self.envelope.name, self.n
            ),
            None => write!(
                f,
                "admitted by envelope {} (use {}, no cap)",
                self.envelope.name, self.n
            ),
        }
    }
}

/// The human's state directory: `$XDG_STATE_HOME` when absolute, else `~/.local/state`.
fn state_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".local/state")))
}

/// Where envelope uses are counted for every world: `$XDG_STATE_HOME/mnemorph/oni/uses.jsonl`.
/// One journal for all worlds, since an envelope in the global file covers them all.
#[must_use]
pub fn uses_path() -> Option<PathBuf> {
    state_dir().map(|s| s.join("mnemorph/oni/uses.jsonl"))
}

impl EnvelopeGate {
    /// The human's files for `world` and the uses ledger under the state directory; `None` when
    /// there is no state directory to count uses in, and then no envelope admits anything.
    #[must_use]
    pub fn of(world: &Path) -> Option<Self> {
        Some(Self::new(Paths::resolve(world), uses_path()?))
    }

    /// Envelopes read from `paths`, their uses counted in `uses`.
    #[must_use]
    pub fn new(paths: Paths, uses: impl Into<PathBuf>) -> Self {
        Self {
            paths,
            uses: Uses::at(uses),
        }
    }

    /// The uses ledger.
    #[must_use]
    pub fn uses(&self) -> &Uses {
        &self.uses
    }

    /// The envelopes the human signed, read afresh from their global file — so one revoked a
    /// moment ago is seen revoked. A file in the world is never read for them: whatever runs
    /// there, as the human, may write it.
    ///
    /// # Errors
    ///
    /// The global file does not load, or is not the human's own: tracked by git in `world`, a
    /// link, another user's, or writable by others.
    pub fn signed(&self, world: &Path) -> Result<Vec<Envelope>, String> {
        let config = Config::load_global(&self.paths)
            .map_err(|e| format!("the configuration does not load, so no envelope is read: {e}"))?;
        let Some(leaf) = config.get("oni.envelopes") else {
            return Ok(Vec::new());
        };
        if leaf.origin.layer != Layer::Global {
            return Err(format!(
                "envelopes come only from the human's global file, not the {} one",
                leaf.origin.layer
            ));
        }
        let file = leaf
            .origin
            .file
            .clone()
            .ok_or("envelopes come only from the human's own file")?;
        own_file(&file, world)?;
        Ok(config.envelopes())
    }

    /// Whether the world's local file signs envelopes, which the gate never honours.
    #[must_use]
    pub fn local_ignored(&self) -> bool {
        fs::read_to_string(&self.paths.local)
            .ok()
            .and_then(|t| toml::from_str::<toml::Table>(&t).ok())
            .is_some_and(|t| {
                t.get("oni")
                    .and_then(toml::Value::as_table)
                    .is_some_and(|o| o.contains_key("envelopes"))
            })
    }
}

/// What an envelope is signed for, as the human reads it.
fn scope(e: &Envelope) -> String {
    let mut parts = Vec::new();
    if let Some(w) = &e.world {
        parts.push(format!("world {}", w.display()));
    }
    if let Some(r) = &e.repo {
        parts.push(format!("repo {r}"));
    }
    if let Some(p) = &e.post {
        parts.push(format!("post {p}"));
    }
    if let Some(b) = &e.body {
        parts.push(format!("body {b}"));
    }
    if parts.is_empty() {
        "no world or repo".into()
    } else {
        parts.join(", ")
    }
}

/// The repository an act lands on, `owner/name`: its own field, or for a push the one every
/// pinned URL names. `None` when it names none plainly.
fn act_repo(planned: &Planned, pin: &Pin) -> Option<String> {
    match (planned.act(), pin) {
        (
            Act::AddLabel { repo, .. }
            | Act::Comment { repo, .. }
            | Act::OpenPullRequest { repo, .. },
            _,
        ) => Some(repo.clone()),
        (Act::PushNewBranch { .. }, Pin::Push { urls, .. }) => {
            let mut repos = urls.iter().map(|u| url_repo(u));
            let first = repos.next()??;
            repos
                .all(|r| r.is_some_and(|r| r.eq_ignore_ascii_case(&first)))
                .then_some(first)
        }
        _ => None,
    }
}

/// The `owner/name` a push URL names on its own host.
fn url_repo(url: &str) -> Option<String> {
    let host = match url.split_once("://") {
        Some((_, rest)) => rest.split('/').next()?,
        None => url.split_once(':')?.0,
    };
    crate::closure::forge_repo(url, host)
}

/// Whether `envelope` is signed for this act: from `world`, on its repository, by `post` and
/// `body`. One naming neither a world nor a repository is signed for nothing.
fn bound(
    envelope: &Envelope,
    world: &Path,
    planned: &Planned,
    pin: &Pin,
    post: &str,
    body: &str,
) -> Result<(), String> {
    if envelope.world.is_none() && envelope.repo.is_none() {
        return Err(
            "it names neither a `world` nor a `repo`, so it is signed for nothing: sign it for one"
                .into(),
        );
    }
    if let Some(w) = &envelope.world {
        let canon = |p: &Path| p.canonicalize().ok();
        if canon(w).is_none() || canon(w) != canon(world) {
            return Err(format!(
                "it is signed for the world {}, not this one",
                w.display()
            ));
        }
    }
    if let Some(r) = &envelope.repo
        && !act_repo(planned, pin).is_some_and(|a| a.eq_ignore_ascii_case(r))
    {
        return Err(format!(
            "it is signed for the repository {r}, not this act's"
        ));
    }
    if let Some(p) = &envelope.post
        && p != post
    {
        return Err(format!("it is signed for the post {p}, not {post}"));
    }
    if let Some(b) = &envelope.body
        && b != body
    {
        return Err(format!("it is signed for the body {b}, not {body}"));
    }
    Ok(())
}

/// A revocation, journaled beside the uses: which envelope, when, and the file it was written in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revocation {
    /// The envelope's name.
    pub envelope: String,
    /// The day it was signed.
    pub signed: String,
    /// The day it was revoked.
    pub revoked: String,
    /// The file `revoked = <day>` was written in.
    pub file: String,
    /// When.
    pub at: Timestamp,
}

/// Free text as one escaped line of at most `max` bytes: no character of it moves or restyles
/// what the human reads.
fn said(text: &str, max: usize) -> String {
    crate::excerpt(text.as_bytes(), max)
}

impl EnvelopeGate {
    /// Where revocations are journaled: beside the uses.
    #[must_use]
    pub fn revocations(&self) -> PathBuf {
        self.uses.path().with_file_name("revoked.jsonl")
    }

    /// One block per envelope signed for `world`: its name, scope, signing and expiry, its uses
    /// against its cap, whether it is in force, revoked, expired, spent or suspended — and every
    /// act it admitted, newest last. Every word escaped.
    #[must_use]
    pub fn lines(&self, world: &Path) -> Vec<String> {
        let today = Timestamp::now().day();
        let mut out = Vec::new();
        if self.local_ignored() {
            out.push(format!(
                "the envelopes in {} are ignored: only the global file signs one, since a file in the world may be written by whatever runs there",
                said(&self.paths.local.display().to_string(), 300)
            ));
        }
        let signed = match self.signed(world) {
            Ok(s) => s,
            Err(e) => {
                out.push(format!("no envelope is honoured: {}", said(&e, 400)));
                // Shown anyway, from what the global file holds, so the human sees what is there.
                Config::load_global(&self.paths)
                    .map(|c| c.envelopes())
                    .unwrap_or_default()
            }
        };
        if signed.is_empty() && out.is_empty() {
            out.push("no envelope is signed".into());
        }
        let rows = self.uses.rows();
        if let Err(e) = &rows {
            out.push(format!(
                "the uses cannot be counted, so no envelope admits anything: {}",
                said(&e.to_string(), 300)
            ));
        }
        let rows = rows.unwrap_or_default();
        for e in &signed {
            let mine: Vec<&UseRow> = rows.iter().filter(|r| r.of(&e.name, e.signed)).collect();
            let used = mine.len() as u64;
            let cap = e
                .uses
                .map_or_else(|| "no cap".to_owned(), |c| c.to_string());
            let state = match e.admits(today, used) {
                Err(lapse) => lapse.to_string(),
                Ok(()) if e.world.is_none() && e.repo.is_none() => {
                    "signed for no world or repo, so it admits nothing".into()
                }
                Ok(()) if crate::suspended(world, e) => {
                    "suspended: an incident on an act it reads is open".into()
                }
                Ok(()) => "in force".into(),
            };
            out.push(format!(
                "{} — {}; signed {}, expires {}; uses {used}/{cap}",
                said(&e.name, 80),
                said(&state, 120),
                e.signed,
                e.expires
            ));
            out.push(format!("    for: {}", said(&scope(e), 400)));
            for pattern in &e.allow {
                out.push(format!("    allows: {}", said(pattern, 200)));
            }
            for r in mine {
                out.push(format!(
                    "    · use {} at {}: {} act {} — {} [{}]",
                    r.n,
                    r.at,
                    said(&r.post, 40),
                    said(&r.act, 40),
                    said(&r.command, 200),
                    said(&r.world, 200)
                ));
            }
        }
        out
    }

    /// Revoke the envelope `name` today, in the human's file that holds it, and journal it. The
    /// envelope stays on record, `revoked = <day>`, and admits nothing from then on.
    ///
    /// # Errors
    ///
    /// The configuration does not load, no envelope has that name, it is revoked already, or the
    /// file or journal cannot be written.
    pub fn revoke(&self, name: &str) -> Result<String, String> {
        let config = Config::load_global(&self.paths)?;
        let today = Timestamp::now().day();
        let envelope = config
            .envelopes()
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| format!("no envelope `{}` is signed", said(name, 80)))?;
        let file = config.revoke_envelope(name, today)?;
        let row = Revocation {
            envelope: name.to_owned(),
            signed: envelope.signed.to_string(),
            revoked: today.to_string(),
            file: file.display().to_string(),
            at: Timestamp::now(),
        };
        mnemorph_ledger::Journal::chained(self.revocations())
            .append(&row)
            .map_err(|e| {
                format!(
                    "envelope `{}` is revoked in {}, but the revocation was not journaled: {e}",
                    said(name, 80),
                    file.display()
                )
            })?;
        Ok(format!(
            "envelope `{}` revoked on {today} in {}; it stays on record and admits nothing",
            said(name, 80),
            file.display()
        ))
    }
}

/// Whether `file` is the human's own: not a link, owned by this user, neither it nor its folder
/// writable by group or others, and not a file git tracks in `world` — a file committed to the
/// repository is the project's, whatever its name.
fn own_file(file: &Path, world: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(file)
        .map_err(|e| format!("{}: cannot be read ({e})", file.display()))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(format!(
            "{} is a link or not a plain file: envelopes are read only from the human's own file",
            file.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        use std::os::unix::fs::PermissionsExt as _;
        if meta.uid() != rustix::process::geteuid().as_raw() {
            return Err(format!(
                "{} is another user's: no envelope in it is honoured",
                file.display()
            ));
        }
        if meta.permissions().mode() & 0o022 != 0 {
            return Err(format!(
                "{} may be written by others than its owner: no envelope in it is honoured",
                file.display()
            ));
        }
        let parent = file
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let dir = fs::metadata(parent)
            .map_err(|e| format!("{}: cannot be read ({e})", parent.display()))?;
        if dir.permissions().mode() & 0o022 != 0 {
            return Err(format!(
                "{} may be written by others than its owner, so the file in it may be replaced: no envelope in it is honoured",
                parent.display()
            ));
        }
    }
    let world = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    let resolved = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    if let Ok(inside) = resolved.strip_prefix(&world) {
        let rel = inside.to_string_lossy();
        let tracked = world_git(&world, &["ls-files", "-z", "--", &rel]).map_err(|e| {
            format!("whether git tracks {rel} cannot be read, so no envelope is honoured: {e}")
        })?;
        if !tracked.is_empty() {
            return Err(format!(
                "{rel} is tracked by git: a file the repository carries is the project's, and never signs an envelope"
            ));
        }
    }
    Ok(())
}

/// Read the envelopes for `planned`, an act of the body of race `race` landing at `pin` from
/// `world`, on `today`. With `held`, the uses ledger is held and this is the moment of use;
/// without it, a preview: whether to ask the human at all.
///
/// # Errors
///
/// Why no envelope admits it, in the order of the module's steps.
#[allow(clippy::too_many_arguments)]
pub(crate) fn admit(
    gate: Option<&EnvelopeGate>,
    race: Race,
    planned: &Planned,
    pin: &Pin,
    world: &Path,
    probe: &dyn Probe,
    journal: &PostJournal,
    body: &str,
    today: Day,
    held: Option<&Held<'_>>,
) -> Result<Admitted, String> {
    // 1. An Oni.
    if race != Race::Oni {
        return Err(format!(
            "only an Oni acts under an envelope, and this body is a {race}"
        ));
    }
    let Some(gate) = gate else {
        return Err("there is no state directory to count envelope uses in".into());
    };
    // 2. A typed outward act, never an undo or a destructive one.
    if planned.is_undo() {
        return Err("an undo always asks the human".into());
    }
    match planned.class() {
        ToolClass::Outward => {}
        ToolClass::Destructive => {
            return Err("destructive acts always ask, whatever the envelope says".into());
        }
        other => return Err(format!("a {other} act is not an envelope's to cover")),
    }
    // 3. The envelopes, from the human's global file.
    let signed = gate.signed(world)?;
    let reading: Vec<&Envelope> = signed
        .iter()
        .filter(|e| envelope_reads(e, planned.act()))
        .collect();
    if reading.is_empty() {
        return Err(if signed.is_empty() {
            "no envelope is signed".into()
        } else {
            "no envelope's pattern reads this act".into()
        });
    }
    // 4. Signed for this world or repository, post and body.
    let post = journal.post.creature();
    let mut why = Vec::new();
    let reading: Vec<&Envelope> = reading
        .into_iter()
        .filter(|e| match bound(e, world, planned, pin, post, body) {
            Ok(()) => true,
            Err(e2) => {
                why.push(format!("{}: {e2}", e.name));
                false
            }
        })
        .collect();
    if reading.is_empty() {
        return Err(why.join("; "));
    }
    // 5. What no standing approval covers: only now, when an envelope could apply.
    excluded(planned, pin, world, probe)?;
    // 6 and 7. In force, under its cap, and not suspended.
    for envelope in reading {
        let used = match held {
            Some(held) => held.count(&envelope.name, envelope.signed),
            None => gate.uses.rows().map(|rows| {
                rows.iter()
                    .filter(|r| r.of(&envelope.name, envelope.signed))
                    .count() as u64
            }),
        };
        let used = match used {
            Ok(n) => n,
            Err(e) => {
                why.push(format!(
                    "{}: its uses cannot be counted ({e})",
                    envelope.name
                ));
                continue;
            }
        };
        if let Err(uncovered) = envelope.covers(planned, today, used) {
            why.push(format!("{}: {uncovered}", envelope.name));
            continue;
        }
        if journal.suspends(envelope) || crate::suspended(world, envelope) {
            why.push(format!(
                "{}: suspended while an incident on an act it reads is open",
                envelope.name
            ));
            continue;
        }
        return Ok(Admitted {
            envelope: envelope.clone(),
            n: used + 1,
        });
    }
    Err(why.join("; "))
}

/// The use row of an admitted act.
pub(crate) fn use_row(
    admitted: &Admitted,
    act: &str,
    journal: &PostJournal,
    planned: &Planned,
) -> UseRow {
    UseRow {
        envelope: admitted.envelope.name.clone(),
        signed: admitted.envelope.signed.to_string(),
        act: act.to_owned(),
        post: journal.post.creature().to_owned(),
        world: journal.world.display().to_string(),
        command: command_line(planned.command()),
        at: Timestamp::now(),
        n: admitted.n,
    }
}

/// The compiled exclusions, read before any envelope: what no standing approval — an envelope,
/// or `gate.approve` once the gate reads it — ever covers, whatever it says.
///
/// # Errors
///
/// Why the act always asks the human.
pub fn excluded(
    planned: &Planned,
    pin: &Pin,
    world: &Path,
    probe: &dyn Probe,
) -> Result<(), String> {
    for kind in planned.kinds() {
        match kind {
            OutwardKind::PushBranch
            | OutwardKind::PullRequest
            | OutwardKind::Comment
            | OutwardKind::Label => {}
            OutwardKind::Deploy | OutwardKind::Rollback => {
                return Err(
                    "a deploy always asks: every environment is protected until a typed deploy names its own"
                        .into(),
                );
            }
            other => return Err(format!("a {other} act is never an envelope's")),
        }
    }
    if planned.kinds().is_empty() {
        return Err("the classifier read no outward act in it".into());
    }
    let Some(face) = face_of(planned.act()) else {
        return Err("the act's words do not render".into());
    };
    let mut text: Vec<&str> = face.iter().map(String::as_str).collect();
    if let Act::OpenPullRequest { title, .. } = planned.act() {
        text.push(title);
    }
    for word in &text {
        if word.len() > COVERED_TEXT {
            return Err(format!(
                "outbound text above {COVERED_TEXT} bytes always asks"
            ));
        }
        if let Some(what) = secret_in(word) {
            return Err(format!(
                "outbound text holding what looks like a secret ({what}) always asks"
            ));
        }
    }
    let paths = match (planned.act(), pin) {
        (Act::PushNewBranch { .. }, Pin::Push { urls, sha }) => {
            pushed_paths(world, urls, sha, probe)?
        }
        (
            Act::OpenPullRequest {
                repo, base, head, ..
            },
            Pin::Forge { host },
        ) => compared_paths(repo, base, head, host, probe)?,
        (Act::PushNewBranch { .. } | Act::OpenPullRequest { .. }, _) => {
            return Err("the act's target is not pinned".into());
        }
        _ => BTreeSet::new(),
    };
    for path in &paths {
        if let Some(what) = guarded(path) {
            return Err(format!(
                "its diff touches {what} ({}): such a push always asks",
                command_line(path)
            ));
        }
    }
    Ok(())
}

/// The paths the commits a push would bring to each pinned URL touch: every commit from `sha`
/// that no branch on that URL already holds, merges diffed against each parent, renames as a
/// delete and an add.
///
/// # Errors
///
/// A URL whose branches cannot be read, or a diff that cannot: the push then asks.
fn pushed_paths(
    world: &Path,
    urls: &[String],
    sha: &str,
    probe: &dyn Probe,
) -> Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    for url in urls {
        let (mut probes, mut seen) = (Vec::new(), Vec::new());
        let heads = probed(
            probe,
            &crate::receipt::ls_heads_argv(url),
            &mut probes,
            &mut seen,
        )
        .map_err(|e| format!("the branches on {} cannot be read: {e}", redact(url)))?;
        // `^<tip>` lines, never `--not`: every git reads them on its input.
        let mut input = format!("{sha}\n");
        for line in String::from_utf8_lossy(&heads).lines() {
            if let Some((tip, _)) = line.split_once('\t')
                && crate::outward::sha_ok(tip)
            {
                input.push('^');
                input.push_str(tip);
                input.push('\n');
            }
        }
        let listed = world_git_with(
            world,
            &[
                "log",
                "-m",
                "--no-renames",
                "--name-only",
                "--format=",
                "-z",
                "--ignore-missing",
                "--stdin",
            ],
            Some(input.as_bytes()),
        )
        .map_err(|e| format!("the push's diff cannot be read: {e}"))?;
        for path in listed.split('\0') {
            let path = path.trim_matches('\n');
            if !path.is_empty() {
                out.insert(path.to_owned());
            }
        }
    }
    Ok(out)
}

/// The most files the forge lists in a comparison; one at the limit may have more unlisted.
const COMPARED_FILES: usize = 300;

/// The paths a pull request from `head` into `base` changes, as the forge compares them.
fn compared_paths(
    repo: &str,
    base: &str,
    head: &str,
    host: &str,
    probe: &dyn Probe,
) -> Result<BTreeSet<String>, String> {
    let (mut probes, mut seen) = (Vec::new(), Vec::new());
    let argv = forge_get(host, &format!("repos/{repo}/compare/{base}...{head}"));
    let out = probed(probe, &argv, &mut probes, &mut seen)
        .map_err(|e| format!("the pull request's diff cannot be read: {e}"))?;
    let json: serde_json::Value = serde_json::from_slice(&out)
        .map_err(|e| format!("the pull request's diff does not read: {e}"))?;
    let files = json
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or("the forge listed no files for the pull request's diff")?;
    if files.len() >= COMPARED_FILES {
        return Err(format!(
            "the pull request changes {COMPARED_FILES} files or more, and the forge lists no more: its diff cannot be read whole"
        ));
    }
    let mut paths = BTreeSet::new();
    for file in files {
        for key in ["filename", "previous_filename"] {
            if let Some(p) = file.get(key).and_then(serde_json::Value::as_str) {
                paths.insert(p.to_owned());
            }
        }
        if file
            .get("filename")
            .and_then(serde_json::Value::as_str)
            .is_none()
        {
            return Err("a file of the pull request's diff names no path".into());
        }
    }
    Ok(paths)
}

/// Directories, at any depth, whose files configure CI or the build.
const GUARDED_DIRS: &[(&str, &str)] = &[
    (".github", "CI configuration"),
    (".gitlab", "CI configuration"),
    (".circleci", "CI configuration"),
    (".buildkite", "CI configuration"),
    (".forgejo", "CI configuration"),
    (".gitea", "CI configuration"),
    (".woodpecker", "CI configuration"),
    (".azure-pipelines", "CI configuration"),
    (".cargo", "a manifest"),
    (".changeset", "release configuration"),
];

/// Top-level directories that hold deploy configuration.
const DEPLOY_DIRS: &[&str] = &[
    "deploy",
    "deploys",
    "deployment",
    "deployments",
    "k8s",
    "kubernetes",
    "helm",
    "charts",
    "terraform",
    "infra",
    "ansible",
];

/// File names, at any depth and in any case, that configure CI, deploys or releases, or are
/// manifests: what a build or a pipeline reads to decide what runs.
const GUARDED_FILES: &[(&str, &str)] = &[
    (".gitlab-ci.yml", "CI configuration"),
    (".travis.yml", "CI configuration"),
    ("azure-pipelines.yml", "CI configuration"),
    ("jenkinsfile", "CI configuration"),
    (".drone.yml", "CI configuration"),
    ("bitbucket-pipelines.yml", "CI configuration"),
    ("appveyor.yml", "CI configuration"),
    (".appveyor.yml", "CI configuration"),
    ("cloudbuild.yaml", "CI configuration"),
    ("cloudbuild.yml", "CI configuration"),
    ("buildspec.yml", "CI configuration"),
    (".woodpecker.yml", "CI configuration"),
    (".pre-commit-config.yaml", "CI configuration"),
    ("justfile", "CI configuration"),
    (".justfile", "CI configuration"),
    ("makefile", "CI configuration"),
    ("gnumakefile", "CI configuration"),
    ("docker-compose.yml", "deploy configuration"),
    ("docker-compose.yaml", "deploy configuration"),
    ("compose.yml", "deploy configuration"),
    ("compose.yaml", "deploy configuration"),
    ("fly.toml", "deploy configuration"),
    ("vercel.json", "deploy configuration"),
    ("netlify.toml", "deploy configuration"),
    ("procfile", "deploy configuration"),
    ("app.yaml", "deploy configuration"),
    ("render.yaml", "deploy configuration"),
    ("skaffold.yaml", "deploy configuration"),
    ("kustomization.yaml", "deploy configuration"),
    ("chart.yaml", "deploy configuration"),
    ("wrangler.toml", "deploy configuration"),
    ("serverless.yml", "deploy configuration"),
    ("release.toml", "release configuration"),
    ("release-plz.toml", "release configuration"),
    (".goreleaser.yml", "release configuration"),
    (".goreleaser.yaml", "release configuration"),
    ("dist-workspace.toml", "release configuration"),
    ("cliff.toml", "release configuration"),
    ("release-please-config.json", "release configuration"),
    (".release-please-manifest.json", "release configuration"),
    ("cargo.toml", "a manifest"),
    ("cargo.lock", "a manifest"),
    ("build.rs", "a manifest"),
    ("rust-toolchain", "a manifest"),
    ("rust-toolchain.toml", "a manifest"),
    ("package.json", "a manifest"),
    ("package-lock.json", "a manifest"),
    ("npm-shrinkwrap.json", "a manifest"),
    ("pnpm-lock.yaml", "a manifest"),
    ("pnpm-workspace.yaml", "a manifest"),
    ("yarn.lock", "a manifest"),
    (".npmrc", "a manifest"),
    (".yarnrc", "a manifest"),
    (".yarnrc.yml", "a manifest"),
    ("bun.lock", "a manifest"),
    ("bun.lockb", "a manifest"),
    ("deno.json", "a manifest"),
    ("go.mod", "a manifest"),
    ("go.sum", "a manifest"),
    ("go.work", "a manifest"),
    ("pyproject.toml", "a manifest"),
    ("setup.py", "a manifest"),
    ("setup.cfg", "a manifest"),
    ("pipfile", "a manifest"),
    ("pipfile.lock", "a manifest"),
    ("poetry.lock", "a manifest"),
    ("uv.lock", "a manifest"),
    ("gemfile", "a manifest"),
    ("gemfile.lock", "a manifest"),
    ("pom.xml", "a manifest"),
    ("composer.json", "a manifest"),
    ("composer.lock", "a manifest"),
    ("mix.exs", "a manifest"),
    ("flake.nix", "a manifest"),
    ("flake.lock", "a manifest"),
    ("default.nix", "a manifest"),
    ("directory.build.props", "a manifest"),
    (".gitmodules", "a manifest"),
    (".gitattributes", "a manifest"),
    (".tool-versions", "a manifest"),
];

/// What `path`, a path a push or pull request changes, configures — when it is CI, deploy or
/// release configuration, a manifest, or the world's tracked Mnemorph configuration — else
/// `None`. A path that does not read plainly is guarded too.
#[must_use]
// The name is lower-cased before it is compared.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn guarded(path: &str) -> Option<&'static str> {
    if path.chars().any(char::is_control) || path.starts_with('"') {
        return Some("a path that does not read plainly");
    }
    let lower = path.to_ascii_lowercase();
    let parts: Vec<&str> = lower.split('/').filter(|p| !p.is_empty()).collect();
    let (dirs, name) = match parts.split_last() {
        Some((name, dirs)) => (dirs, *name),
        None => return Some("a path that does not read plainly"),
    };
    if lower == ".mnemorph/config.toml" {
        return Some("the world's tracked Mnemorph configuration");
    }
    if let Some((_, what)) = GUARDED_DIRS.iter().find(|(d, _)| dirs.contains(d)) {
        return Some(what);
    }
    if dirs.first().is_some_and(|d| DEPLOY_DIRS.contains(d)) {
        return Some("deploy configuration");
    }
    if let Some((_, what)) = GUARDED_FILES.iter().find(|(f, _)| *f == name) {
        return Some(what);
    }
    let prefixed = |p: &str| name.starts_with(p);
    if prefixed("dockerfile")
        || name.ends_with(".dockerfile")
        || name.ends_with(".tf")
        || name.ends_with(".tfvars")
    {
        return Some("deploy configuration");
    }
    if prefixed(".releaserc") || prefixed("release.config.") {
        return Some("release configuration");
    }
    if (prefixed("requirements") && name.ends_with(".txt"))
        || name.ends_with(".gemspec")
        || name.ends_with(".csproj")
        || prefixed("build.gradle")
        || prefixed("settings.gradle")
    {
        return Some("a manifest");
    }
    None
}

/// Prefixes of tokens a service issues: a word starting with one, and long enough, is a secret.
const TOKEN_PREFIXES: &[&str] = &[
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "xoxa-",
    "xoxb-",
    "xoxp-",
    "xoxr-",
    "xoxs-",
    "sk-",
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "rk_test_",
    "npm_",
    "pypi-",
    "AIza",
    "hf_",
    "shpat_",
    "shpss_",
    "sq0atp-",
    "dop_v1_",
    "glc_",
];

/// What `text` holds that looks like a secret, if anything: credentials in a URL, a private key,
/// a token a service issues, a JSON web token, a long random-looking string, or the value of an
/// environment variable the harness keeps from commands as a secret.
#[must_use]
pub fn secret_in(text: &str) -> Option<String> {
    if redact(text) != text {
        return Some("credentials in a URL".into());
    }
    if text.contains("-----BEGIN") && text.contains("PRIVATE KEY") {
        return Some("a private key".into());
    }
    // A JSON web token keeps its dots; every other shape is read in pieces split at `=`, `:`,
    // `/`, `.`, quotes and anything else no token holds, so `token=ghp_…` or `…/glpat-…` is seen.
    let jwt_char = |c: char| c.is_ascii_alphanumeric() || "_-.".contains(c);
    if text
        .split(|c: char| !jwt_char(c))
        .any(|w| w.starts_with("eyJ") && w.matches('.').count() == 2 && w.len() >= 30)
    {
        return Some("a JSON web token".into());
    }
    let word_char = |c: char| c.is_ascii_alphanumeric() || "_-+".contains(c);
    for word in text.split(|c: char| !word_char(c)) {
        if let Some(p) = TOKEN_PREFIXES
            .iter()
            .find(|p| word.starts_with(**p) && word.len() >= p.len() + 16)
        {
            return Some(format!("a token shaped like `{p}…`"));
        }
        if (word.starts_with("AKIA") || word.starts_with("ASIA"))
            && word.len() == 20
            && word
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Some("an AWS access key id".into());
        }
        if random_looking(word) {
            return Some("a long random-looking string".into());
        }
    }
    let kept: BTreeSet<String> = mnemorph_tools::shell::scrubbed_env()
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    std::env::vars()
        .filter(|(k, v)| !kept.contains(k) && v.len() >= 8)
        .find(|(_, v)| text.contains(v.as_str()))
        .map(|(k, _)| format!("the value of ${k}"))
}

/// At least 32 characters of a token alphabet, mixing upper case, lower case and digits — a
/// commit's hex name, all one case, is not one.
fn random_looking(piece: &str) -> bool {
    piece.len() >= 32
        && piece
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'+' | b'='))
        && piece.bytes().any(|b| b.is_ascii_uppercase())
        && piece.bytes().any(|b| b.is_ascii_lowercase())
        && piece.bytes().any(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{guarded, secret_in};

    #[test]
    fn ci_deploy_release_and_manifest_paths_are_guarded() {
        for path in [
            ".github/workflows/ci.yml",
            ".github/CODEOWNERS",
            "crates/x/Cargo.toml",
            "Cargo.lock",
            "packages/web/package.json",
            "Dockerfile.prod",
            "deploy/prod.yaml",
            "infra/main.tf",
            "release-plz.toml",
            ".changeset/new.md",
            ".mnemorph/config.toml",
            "justfile",
            "crates/x/build.rs",
            "JENKINSFILE",
            "\"odd\\npath\"",
        ] {
            assert!(guarded(path).is_some(), "{path}");
        }
        for path in [
            "src/lib.rs",
            "docs/02-races.md",
            "src/charts/line.ts",
            ".mnemorph/log.md",
            "README.md",
        ] {
            assert_eq!(guarded(path), None, "{path}");
        }
    }

    #[test]
    fn secret_shaped_text_is_found() {
        for text in [
            "token ghp_aB3dE5gH7jK9mN1pQ3sT5vW7yZ9bC1dE3fG5",
            "see https://me:hunter2@example.com/x",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "AKIAIOSFODNN7EXAMPLE",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJlMTIz",
            "key: Zx9Qw3Er5Ty7Ui9Op1As3Df5Gh7Jk9Lz2Xc4",
        ] {
            assert!(secret_in(text).is_some(), "{text}");
        }
        // Each shape glued to what may sit around it. The tokens are put together here, never
        // written whole, so no scanner reads a fixture as a leak.
        let tail = "aB3dE5gH7jK9mN1pQ3sT5vW7yZ9bC1dE3fG5";
        for (before, prefix, after) in [
            ("token=", "ghp", "_"),
            ("https://gitlab.example.com/", "glpat", "-"),
            ("\"", "ghp", "_"),
            ("auth:", "xoxb", "-"),
            ("file.", "npm", "_"),
            ("'", "sk_live", "_"),
        ] {
            let text = format!("{before}{prefix}{after}{tail}'\"");
            assert!(secret_in(&text).is_some(), "{text}");
        }
        for text in [
            format!("id={}{};", "AKIA", "IOSFODNN7EXAMPLE"),
            format!("a/{tail}/b"),
        ] {
            assert!(secret_in(&text).is_some(), "{text}");
        }
        for text in [
            "Fixed in 3f2a9c1d4e5b6a7980c1d2e3f4a5b6c7d8e9f0a1.",
            "LGTM — the label waits for review.",
            "See https://github.com/me/proj/pull/12",
            "sk-learn is a library",
        ] {
            assert_eq!(secret_in(text), None, "{text}");
        }
    }
}
