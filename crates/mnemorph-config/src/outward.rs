//! The Kijin's plan step: an outward act is reversible only when the harness can derive its undo.
//!
//! The act is typed. For a small closed set of kinds the harness renders both the command and
//! its undo from the act's own fields — the repository pinned in the act, the branch a plain
//! ref name — so an undo is never a string a model writes, and never needs verifying. Every
//! other outward act is an [`Act::Command`], and a command has no undo: it is `destructive`, and
//! no Oni envelope covers it; it goes to the human, whatever the envelope says. The classifier
//! still reads every rendered command first, so a typed act it calls destructive stays so.
//!
//! A standing effect (a deploy, a feature flag, a scheduled job) is not a typed kind: it is a
//! command, so destructive, until the gate honours envelopes and an expiry runner ends what it
//! starts (M11).

use std::fmt;
use std::path::PathBuf;

use mnemorph_ledger::Day;
use mnemorph_races::{OutwardKind, PostRefusal, Race, ToolClass, check_post};
use mnemorph_tools::{Ask, Place, Verdict, classify_acts};

use crate::{Envelope, Lapse};

/// What the approval card says of an outward act the harness cannot undo.
pub const NO_UNDO: &str = "no undo — destructive";

/// The longest outbound text an envelope covers, in bytes.
pub const COVERED_TEXT: usize = 4096;

/// One outward act of a Kijin's plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Create a branch on a remote; it must not exist yet. Undone by deleting it, leased on the
    /// commit that was pushed.
    PushNewBranch {
        /// The remote, by its configured name (`origin`).
        remote: String,
        /// The branch, a plain ref name (`feat/undo`), without `refs/heads/`.
        name: String,
    },
    /// Add a label to a pull request. Undone by removing the same label from it.
    AddLabel {
        /// The repository, `owner/name`.
        repo: String,
        /// The pull request's number.
        pr: u64,
        /// The label.
        label: String,
    },
    /// Comment on a pull request. Undone by deleting the comment, by the id it was given.
    Comment {
        /// The repository, `owner/name`.
        repo: String,
        /// The pull request's number.
        pr: u64,
        /// The comment.
        body: String,
    },
    /// Any other command: the harness derives no undo for it, so an outward one is destructive.
    Command(String),
}

/// Why a plan's act is refused before it is ever asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Malformed {
    /// The command is empty.
    Empty,
    /// A field is not what its kind takes: which, and why.
    Field(&'static str, &'static str),
    /// The guard refuses the rendered command.
    Refused(String),
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Malformed::Empty => f.write_str("the act names no command"),
            Malformed::Field(field, why) => write!(f, "`{field}` {why}"),
            Malformed::Refused(why) => write!(f, "the guard refuses the act: {why}"),
        }
    }
}

impl std::error::Error for Malformed {}

/// What the act left behind, as the harness recorded it when the act ran: what the undo needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    /// A branch was pushed at this commit (its full hex object name).
    Pushed {
        /// The commit pushed.
        sha: String,
    },
    /// The label was added.
    Labelled,
    /// The comment was made, with this id.
    Commented {
        /// The comment's id.
        id: u64,
    },
}

/// Why an undo cannot be rendered from what the act left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoUndo {
    /// The act is a command: nothing undoes it.
    Command,
    /// The record is of another kind of act, or names no full commit.
    Mismatch,
}

impl fmt::Display for NoUndo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NoUndo::Command => "a command has no undo",
            NoUndo::Mismatch => "the record is not of this act",
        })
    }
}

/// An act the plan step has classed. It can only be made by [`plan`], so whatever reads its
/// class, command or undo reads the plan's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    act: Act,
    argv: Vec<String>,
    command: String,
    class: ToolClass,
    why: Vec<String>,
    outside: Vec<PathBuf>,
    /// The kinds of outward act the classifier read in the command.
    acts: Vec<OutwardKind>,
}

/// A plain configured remote name: `origin`, `upstream`.
fn remote_ok(remote: &str) -> bool {
    mnemorph_tools::remote_name(remote)
}

/// A plain branch name: ASCII letters, digits, `-`, `_`, `.` and `/` between components; no
/// `:`, `+`, `..`, `@{`, glob or space; no component starting with `.` or `-`, none ending in
/// `.` or `.lock`; not `HEAD`, and not under `refs/`.
fn branch_ok(name: &str) -> bool {
    name.len() <= 200
        && name != "HEAD"
        && !name.starts_with("refs/")
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/'))
        && name.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with(['.', '-'])
                && !part.ends_with('.')
                && !part
                    .rsplit_once('.')
                    .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("lock"))
        })
}

/// `owner/name`, as GitHub spells them.
fn repo_ok(repo: &str) -> bool {
    let part = |p: &str| {
        !p.is_empty()
            && p.len() <= 100
            && !p.starts_with(['.', '-'])
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    repo.split_once('/')
        .is_some_and(|(owner, name)| part(owner) && part(name))
}

/// The longest comment an act carries, in bytes.
pub const BODY_BYTES: usize = 65_536;

/// Characters that make text read otherwise than it is: bidirectional overrides and isolates,
/// zero-width characters and marks, and the Unicode line and paragraph separators.
fn deceptive(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Text a model writes into an act: not blank, no control character but line breaks and tabs,
/// nothing that reads otherwise than it is.
fn text_ok(text: &str) -> bool {
    !text.trim().is_empty()
        && !text
            .chars()
            .any(|c| (c.is_control() && !matches!(c, '\n' | '\t')) || deceptive(c))
}

/// A word as the shell reads it back unchanged: bare when it is plain, else single-quoted.
fn quote(word: &str) -> String {
    let plain = !word.is_empty()
        && word.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/' | b':' | b'=' | b'@')
        });
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

fn line(argv: &[String]) -> String {
    argv.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ")
}

fn owned(words: &[&str]) -> Vec<String> {
    words.iter().map(|&w| w.to_owned()).collect()
}

fn pr_word(pr: u64) -> Result<String, Malformed> {
    if pr == 0 {
        Err(Malformed::Field("pr", "is a pull request number, from 1"))
    } else {
        Ok(pr.to_string())
    }
}

fn repo_word(repo: &str) -> Result<&str, Malformed> {
    if repo_ok(repo) {
        Ok(repo)
    } else {
        Err(Malformed::Field("repo", "is `owner/name`, plainly spelled"))
    }
}

/// The argv the harness renders for a typed act, its fields checked; `None` for a command.
fn render(act: &Act) -> Result<Option<Vec<String>>, Malformed> {
    Ok(Some(match act {
        Act::PushNewBranch { remote, name } => {
            if !remote_ok(remote) {
                return Err(Malformed::Field(
                    "remote",
                    "is a configured remote's name, not a URL or an option",
                ));
            }
            if !branch_ok(name) {
                return Err(Malformed::Field(
                    "name",
                    "is a plain branch name: no `:`, `+`, `..`, space or glob, not under `refs/`",
                ));
            }
            owned(&[
                "git",
                "push",
                remote,
                &format!("HEAD:refs/heads/{name}"),
                &format!("--force-with-lease=refs/heads/{name}:"),
            ])
        }
        Act::AddLabel { repo, pr, label } => {
            let repo = repo_word(repo)?;
            // `gh pr edit --add-label` splits on commas: one approved label must add one.
            if !text_ok(label) || label.contains(['\n', ',', '"']) || label.chars().count() > 50 {
                return Err(Malformed::Field(
                    "label",
                    "is one line of at most 50 characters, without `,` or `\"`",
                ));
            }
            owned(&[
                "gh",
                "pr",
                "edit",
                &pr_word(*pr)?,
                "--repo",
                repo,
                "--add-label",
                label,
            ])
        }
        Act::Comment { repo, pr, body } => {
            let repo = repo_word(repo)?;
            if !text_ok(body) || body.len() > BODY_BYTES {
                return Err(Malformed::Field(
                    "body",
                    "is text of at most 65536 bytes, with no control character but line breaks and tabs, and no bidirectional or zero-width character",
                ));
            }
            owned(&[
                "gh",
                "pr",
                "comment",
                &pr_word(*pr)?,
                "--repo",
                repo,
                "--body",
                body,
            ])
        }
        Act::Command(_) => return Ok(None),
    }))
}

/// Class an act of a Kijin's plan at `place`.
///
/// A typed act is rendered by the harness, read by the classifier, and stays `outward` unless
/// the classifier reads its command as destructive. An [`Act::Command`] the classifier reads as
/// outward or destructive is `destructive` ([`NO_UNDO`]): the harness derives no undo for it. A
/// command that does not leave the machine keeps its class: it has nothing outside to undo.
///
/// # Errors
///
/// [`Malformed`] when a field is not what its kind takes, a command is empty, or the guard
/// refuses what would run.
pub fn plan(act: &Act, place: &Place) -> Result<Planned, Malformed> {
    let argv = render(act)?;
    let command = match (&argv, act) {
        (Some(argv), _) => line(argv),
        (None, Act::Command(c)) if !c.trim().is_empty() => c.clone(),
        (None, _) => return Err(Malformed::Empty),
    };
    let (verdict, acts) = classify_acts(&command, place);
    let (read, mut why, outside) = match verdict {
        Verdict::Run {
            class,
            why,
            outside,
        } => (class, why, outside),
        Verdict::Refuse { reason, .. } => return Err(Malformed::Refused(reason)),
    };
    let class = match (argv.is_some(), read) {
        (_, ToolClass::Destructive) | (false, ToolClass::Outward) => {
            if argv.is_none() {
                why.insert(0, NO_UNDO.to_owned());
            }
            ToolClass::Destructive
        }
        (true, _) => ToolClass::Outward,
        (false, other) => other,
    };
    Ok(Planned {
        act: act.clone(),
        argv: argv.unwrap_or_default(),
        command,
        class,
        why,
        outside,
        acts,
    })
}

/// A full hex object name: 40 digits (SHA-1) or 64 (SHA-256), lower case.
pub(crate) fn sha_ok(sha: &str) -> bool {
    matches!(sha.len(), 40 | 64) && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl Planned {
    /// The act, as the plan took it.
    #[must_use]
    pub fn act(&self) -> &Act {
        &self.act
    }

    /// The command, exactly as the gate runs it.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Whether the Kijin named `body` may do this act: every kind of outward act in it must be
    /// its post's, as for a command it runs itself.
    ///
    /// # Errors
    ///
    /// The [`PostRefusal`] naming the first kind its post does not hold.
    pub fn check_post(&self, body: &str) -> Result<(), PostRefusal> {
        check_post(body, &self.acts)
    }

    /// Its class: the classifier's, raised to `destructive` for a command the harness cannot
    /// undo.
    #[must_use]
    pub fn class(&self) -> ToolClass {
        self.class
    }

    /// Why it has its class; [`NO_UNDO`] first when that is why.
    #[must_use]
    pub fn why(&self) -> &[String] {
        &self.why
    }

    /// How the act will be put back, as the card says it before the act runs.
    #[must_use]
    pub fn undo_plan(&self) -> Option<String> {
        match &self.act {
            Act::PushNewBranch { remote, name } => Some(format!(
                "delete branch {name} on {remote}, only while it still holds the pushed commit"
            )),
            Act::AddLabel { repo, pr, label } => {
                Some(format!("remove label {label:?} from {repo}#{pr}"))
            }
            Act::Comment { repo, pr, .. } => Some(format!("delete this comment on {repo}#{pr}")),
            Act::Command(_) => None,
        }
    }

    /// The undo's command, rendered from the act's own fields and what it left behind. The undo
    /// is an act of its own: it goes through the gate like any other.
    ///
    /// # Errors
    ///
    /// [`NoUndo`] for a command, or a record of another kind of act or without a full commit.
    pub fn undo(&self, done: &Done) -> Result<String, NoUndo> {
        let argv = match (&self.act, done) {
            (Act::PushNewBranch { remote, name }, Done::Pushed { sha }) if sha_ok(sha) => owned(&[
                "git",
                "push",
                remote,
                "--delete",
                &format!("refs/heads/{name}"),
                &format!("--force-with-lease=refs/heads/{name}:{sha}"),
            ]),
            (Act::AddLabel { repo, pr, label }, Done::Labelled) => owned(&[
                "gh",
                "pr",
                "edit",
                &pr.to_string(),
                "--repo",
                repo,
                "--remove-label",
                label,
            ]),
            (Act::Comment { repo, .. }, Done::Commented { id }) => owned(&[
                "gh",
                "api",
                "-X",
                "DELETE",
                &format!("repos/{repo}/issues/comments/{id}"),
            ]),
            (Act::Command(_), _) => return Err(NoUndo::Command),
            _ => return Err(NoUndo::Mismatch),
        };
        Ok(line(&argv))
    }

    /// What a comment act left behind, read from what `gh pr comment` printed: the comment's URL
    /// on the host it was pinned to (`github.com`, or the `GH_HOST` of an Enterprise server), this
    /// act's repository and pull request, and nothing else.
    #[must_use]
    pub fn commented(&self, printed: &str, host: &str) -> Option<Done> {
        let Act::Comment { repo, pr, .. } = &self.act else {
            return None;
        };
        let head = format!("https://{host}/{repo}/pull/{pr}#issuecomment-");
        let id = printed.lines().find_map(|l| l.trim().strip_prefix(&head))?;
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        id.parse().ok().map(|id| Done::Commented { id })
    }

    /// The question the gate puts to the human: [`NO_UNDO`] leads the card of an act the
    /// harness cannot undo; a typed act's card says how it will be put back.
    #[must_use]
    pub fn ask(&self, body: &str, race: Race) -> Ask {
        let mut why = self.why.clone();
        if let Some(undo) = self.undo_plan() {
            why.push(format!("undo: {undo}"));
        }
        Ask {
            body: body.to_owned(),
            race,
            class: self.class,
            act: self.command.clone(),
            why,
            outside: self.outside.clone(),
            // The plan binds where a command is spawned: the bash tool takes and rechecks it.
            fingerprint: None,
        }
    }

    /// The words an envelope pattern is read against: the act as a human would write it — for a
    /// branch `git push <remote> <name>`, for the others the rendered argv, the repository in
    /// it. Never the shell's reading of a string.
    fn face(&self) -> Vec<&str> {
        match &self.act {
            Act::PushNewBranch { remote, name } => vec!["git", "push", remote, name],
            _ => self.argv.iter().map(String::as_str).collect(),
        }
    }
}

/// Why an envelope does not cover a planned act.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uncovered {
    /// The act is destructive: it always goes to the human.
    Destructive,
    /// The act does not leave the machine; an envelope covers only outward acts.
    NotOutward(ToolClass),
    /// The envelope is not in force.
    Lapsed(Lapse),
    /// A word of it is above [`COVERED_TEXT`] bytes.
    TooLong,
    /// No pattern of the envelope matches the act.
    NoPattern,
}

impl fmt::Display for Uncovered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Uncovered::Destructive => {
                f.write_str("destructive acts always ask, whatever the envelope says")
            }
            Uncovered::NotOutward(class) => {
                write!(f, "a {class} act is not an envelope's to cover")
            }
            Uncovered::Lapsed(lapse) => write!(f, "the envelope is {lapse}"),
            Uncovered::TooLong => {
                write!(f, "outbound text above {COVERED_TEXT} bytes always asks")
            }
            Uncovered::NoPattern => f.write_str("no pattern of the envelope matches the act"),
        }
    }
}

impl Envelope {
    /// Whether the envelope covers a planned act on `today`, after `used` acts it already
    /// covered. A destructive act is refused first, before the envelope's own time, count or
    /// patterns are read; only a typed act can be outward, so only a typed act is ever covered.
    ///
    /// # Errors
    ///
    /// The [`Uncovered`] reason.
    pub fn covers(&self, act: &Planned, today: Day, used: u64) -> Result<(), Uncovered> {
        match act.class() {
            ToolClass::Destructive => return Err(Uncovered::Destructive),
            ToolClass::Outward => {}
            other => return Err(Uncovered::NotOutward(other)),
        }
        self.admits(today, used).map_err(Uncovered::Lapsed)?;
        let face = act.face();
        if face.iter().any(|w| w.len() > COVERED_TEXT) {
            return Err(Uncovered::TooLong);
        }
        if self.allow.iter().any(|p| matches(p, &face)) {
            Ok(())
        } else {
            Err(Uncovered::NoPattern)
        }
    }
}

/// Characters a matched word may not hold: what lets a shell run, redirect, expand, glob or
/// quote, or a refspec name another ref. A word holding one, or anything but printable ASCII and
/// the space, is never matched, and its act goes to the human.
const UNMATCHED: &[char] = &[
    ';', '&', '|', '<', '>', '$', '`', '(', ')', '{', '}', '\\', '*', '?', '[', ']', '~', '!', '#',
    ':', '\'', '"',
];

fn word_ok(word: &str) -> bool {
    word.bytes().all(|b| b == b' ' || b.is_ascii_graphic()) && !word.contains(UNMATCHED)
}

/// An envelope pattern against an act's words. A word that is a bare `*` and the pattern's last
/// matches the rest (at least one word); a bare `*` elsewhere matches one word; a word ending in
/// `*` (`feat/*`) matches one longer word beginning with what comes before it; every other word
/// matches itself. Every word of the act must pass [`UNMATCHED`], whichever pattern word it
/// falls under.
fn matches(pattern: &str, face: &[&str]) -> bool {
    if !face.iter().all(|w| word_ok(w)) {
        return false;
    }
    let pattern: Vec<&str> = pattern.split_whitespace().collect();
    for (i, p) in pattern.iter().enumerate() {
        if *p == "*" && i + 1 == pattern.len() {
            return face.len() > i;
        }
        let Some(word) = face.get(i) else {
            return false;
        };
        let fits = match p.strip_suffix('*') {
            Some("") => true,
            Some(prefix) => word.len() > prefix.len() && word.starts_with(prefix),
            None => p == word,
        };
        if !fits {
            return false;
        }
    }
    face.len() == pattern.len()
}
