//! Territory facts: what creatures learned about a path, each with who said it and on what
//! evidence, kept as append-only events and folded into a status no one has to vote on.
//!
//! The events that count are kept by the harness outside the world, at
//! `$XDG_STATE_HOME/mnemorph/facts/<world>-<fnv64a>.jsonl` (the world named by its real path),
//! chained by hash ([`mnemorph_ledger::chain`]) and anchored as `facts` with the world's other
//! chained journals ([`crate::chains`]). Their writers are the harness — a body's `@U territory`
//! facts, a parent's labels, an Orc's `@V settle` — and the human's settle; never a body's
//! command. The world's `.mnemorph/ontology/facts.jsonl` is a copy written after each event for
//! readers, and is never read for trust: a commit or a clone brings whatever a stranger wrote
//! there, so a settle, a status or a voice it holds that the trusted journal does not is ignored
//! and flagged (`untrusted-fact`). A trusted journal that does not walk whole leaves the facts
//! unavailable, never settled. The first read of a world with no trusted journal imports its
//! in-world facts once, each as one unproven voice, unsettled.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mnemorph_ledger::Journal;
use mnemorph_ledger::chain::{self, Head};
use mnemorph_memory::text::{bm25, words};
use mnemorph_wire::fnv64a;
use serde::{Deserialize, Serialize};

use crate::own::normal;

/// The in-world copy of the events, for readers: harness soil, never read for trust.
pub const FACTS: &str = ".mnemorph/ontology/facts.jsonl";

/// The trusted journal's kind under the state directory, and its name among the world's chains.
pub const KIND: &str = "facts";

/// The voice an imported fact is said in: the in-world file's, whose authors cannot be proven.
pub const IMPORTED: &str = "imported";

/// The largest in-world copy read back to compare with the trusted journal.
const MIRROR_CAP: u64 = 16 << 20;

/// The tolerance on the losing side: one dissenter in three.
const TOLERANCE: f64 = 0.35;

/// The compiled vocabulary of keyed facts, and whether paths below inherit each.
pub const KEYS: &[(&str, bool)] = &[
    ("entry", false),
    ("build", false),
    ("test", false),
    ("generated", true),
    ("deprecated", true),
];

/// What a fact rests on: a `file:line`, a directory, a commit, a journal entry — and a digest of
/// what it pointed at, so a fact whose evidence moved reads stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    /// `src/a.rs:12`, `src/`, `commit:<sha>`, `journal:<id>`.
    pub at: String,
    /// The digest of what it pointed at when said; `None` for what never drifts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

/// How a parent labels a new free fact against an older one it resembled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Label {
    /// The same fact: merged as a support.
    Support,
    /// The opposite: a counter.
    Counter,
    /// Narrower: both stand, linked.
    Refines,
    /// Unrelated to all shown.
    Distinct,
}

/// One event in the facts journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event {
    /// A creature asserts a fact.
    Support {
        /// The fact's id.
        id: String,
        /// The path it is about.
        about: String,
        /// Its text: `key=value` for a keyed fact.
        text: String,
        /// Who said it.
        by: String,
        /// Its evidence.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Anchor>,
    },
    /// A creature counters a fact by naming it.
    Counter {
        /// The fact countered.
        id: String,
        /// Who.
        by: String,
        /// Why.
        reason: String,
    },
    /// A proof settles a fact: a landing by the path's owner that passed its gate, the Orc's
    /// verdict, or the human.
    Settle {
        /// The fact.
        id: String,
        /// Who.
        by: String,
        /// The proof.
        proof: String,
    },
    /// The in-world facts imported once into a trusted journal that had none: each fact as one
    /// unproven voice, unsettled — their authors and settles cannot be proven.
    Import {
        /// The facts the in-world file read settled, whose settle was dropped.
        settles: Vec<String>,
        /// How many in-world events were read.
        read: usize,
    },
    /// The parent's triage of a new free fact.
    Label {
        /// The new fact.
        new: String,
        /// The label.
        label: Label,
        /// The older fact; `None` for `distinct`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of: Option<String>,
        /// Who labelled it.
        by: String,
    },
}

/// A fact's status, read by the table: no one votes on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// A settle event with its proof, final until a newer counter.
    Settled,
    /// One voice.
    Single,
    /// Named twice, one dissenter in three tolerated.
    Agreed,
    /// The counters carry it.
    Countered,
    /// Any other case with a counter: a `contested-fact` finding.
    Contested,
}

impl Status {
    /// The status, in words.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Status::Settled => "settled",
            Status::Single => "single",
            Status::Agreed => "agreed",
            Status::Countered => "countered",
            Status::Contested => "contested",
        }
    }
}

/// One fact, folded from its events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fact {
    /// Its id.
    pub id: String,
    /// The path it is about.
    pub about: String,
    /// Its text.
    pub text: String,
    /// `(key, normalised value)` for a keyed fact.
    pub keyed: Option<(String, String)>,
    /// Who supports it.
    pub supporters: BTreeSet<String>,
    /// Who counters it, by name or by collision.
    pub counters: BTreeSet<String>,
    /// Settled, and no counter since.
    pub settled: bool,
    /// Its latest evidence.
    pub source: Option<Anchor>,
    /// The order it was first said in.
    pub seq: usize,
    /// Merged into another fact by the parent's `support` label.
    pub merged_into: Option<String>,
    /// The parent labelled it against the facts it resembled.
    pub labelled: bool,
    /// Why each counter countered, by creature.
    pub reasons: BTreeMap<String, String>,
    /// The order of its last settle, if settled.
    pub settled_at: Option<usize>,
    /// Who settled it last, and on what proof.
    pub settled_by: Option<(String, String)>,
    /// The order of its last support.
    pub last_support: usize,
}

impl Fact {
    /// Its status by the table.
    #[must_use]
    pub fn status(&self) -> Status {
        if self.settled {
            return Status::Settled;
        }
        let p = self.supporters.len();
        let q = self.counters.len();
        #[allow(clippy::cast_precision_loss, reason = "counts of creatures are small")]
        let share = |part: usize| part as f64 / (p + q) as f64;
        match (p, q) {
            // Countered by every voice it had, its own included: withdrawn, not contested.
            (0, _) if q > 0 => Status::Countered,
            (_, 0) if p <= 1 => Status::Single,
            (_, 0) => Status::Agreed,
            _ if p >= 2 && share(q) <= TOLERANCE => Status::Agreed,
            _ if q >= 2 && share(p) <= TOLERANCE => Status::Countered,
            _ => Status::Contested,
        }
    }

    /// Whether its evidence has moved since it was said: computed, never stored.
    #[must_use]
    pub fn stale(&self, world: &Path) -> bool {
        self.source
            .as_ref()
            .and_then(|a| a.digest.as_ref().map(|d| (a, d)))
            .is_some_and(|(a, d)| digest(world, &a.at).as_ref() != Some(d))
    }
}

/// The parent's triage of a new free fact, read from one of its `@V` verdicts:
/// `#<new> support|counter|refines #<id>`, or `#<new> distinct`. Anything else is no label.
#[must_use]
pub fn label_of(verdict: &str, by: &str) -> Option<Event> {
    let mut words = verdict.split_whitespace();
    let new = words.next()?.strip_prefix('#')?.to_owned();
    let label = match words.next()? {
        "support" => Label::Support,
        "counter" => Label::Counter,
        "refines" => Label::Refines,
        "distinct" => Label::Distinct,
        _ => return None,
    };
    let of = match (label, words.next()) {
        (Label::Distinct, None) => None,
        (Label::Distinct, Some(_)) | (_, None) => return None,
        (_, Some(id)) => Some(id.strip_prefix('#')?.to_owned()),
    };
    if words.next().is_some() || new.is_empty() {
        return None;
    }
    Some(Event::Label {
        new,
        label,
        of,
        by: by.to_owned(),
    })
}

/// The floor a triage candidate's BM25 score must clear to be shown to the parent.
pub const THETA_T: f64 = 0.1;

/// A path as facts name it: normalised, inside the world, with no trailing `/` — `src` and
/// `src/` are one path. `None` outside the world.
#[must_use]
pub fn fact_path(about: &str) -> Option<String> {
    normal(about).map(|p| p.trim_end_matches('/').to_owned())
}

/// A fact's text as its id reads it: whitespace collapsed; a keyed fact as `key=value` with its
/// value normalised, so every way of writing it is one fact.
#[must_use]
pub fn fact_text(text: &str) -> String {
    keyed(text).map_or_else(
        || text.split_whitespace().collect::<Vec<_>>().join(" "),
        |(k, v)| format!("{k}={v}"),
    )
}

/// A fact's id: the hash of its path and its normalised text. A path outside the world hashes
/// as written, never folded into another's.
#[must_use]
pub fn fact_id(about: &str, text: &str) -> String {
    let about = fact_path(about).unwrap_or_else(|| format!("\u{1}{about}"));
    let text = fact_text(text);
    format!("{:016x}", fnv64a(format!("{about}\0{text}").as_bytes()))[..10].to_owned()
}

/// The digest of what an anchor points at now: the cited lines of a `file:line[-line]`, the
/// listing of a directory; `None` for a commit or a journal entry, which never drift, or for
/// what cannot be read.
#[must_use]
pub fn digest(world: &Path, at: &str) -> Option<String> {
    if at.starts_with("commit:") || at.starts_with("journal:") {
        return None;
    }
    let hash = |s: &str| format!("{:016x}", fnv64a(s.as_bytes()));
    if let Some((file, lines)) = at.rsplit_once(':') {
        let (from, to) = lines.split_once('-').unwrap_or((lines, lines));
        let (from, to) = (from.parse::<usize>().ok()?, to.parse::<usize>().ok()?);
        let text = crate::regular::text(&world.join(file))?;
        let cited: Vec<&str> = text
            .lines()
            .skip(from.saturating_sub(1))
            .take(to + 1 - from.min(to))
            .collect();
        return Some(hash(&cited.join("\n")));
    }
    // A file is anchored by its whole text: what it says now is what the fact was said about.
    if world.join(at).is_file() {
        return crate::regular::text(&world.join(at)).map(|t| hash(&t));
    }
    let mut names: Vec<String> = std::fs::read_dir(world.join(at))
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Some(hash(&names.join("\n")))
}

/// A fact asserted with no anchor of its own is anchored where it is about, with the digest of
/// what is there now — the file's text, the directory's listing — so that when the code moves
/// under it, the fact reads stale instead of standing as true.
#[must_use]
pub fn anchored(world: &Path, event: Event) -> Event {
    match event {
        Event::Support {
            id,
            about,
            text,
            by,
            source: None,
        } if !(keyed(&text).is_some() && world.join(about.trim_end_matches('/')).is_dir()) => {
            // A keyed fact about a directory (`src/ build=…`) is not anchored: its listing moves
            // with every file added, and the key's own collisions already counter it.
            let at = about.trim_end_matches('/').to_owned();
            let at = if world.join(&at).is_dir() {
                format!("{at}/")
            } else {
                at
            };
            Event::Support {
                source: Some(Anchor {
                    digest: digest(world, &at),
                    at,
                }),
                id,
                about,
                text,
                by,
            }
        }
        other => other,
    }
}

/// Read one `@U territory …` payload, said by `by`, into its event: `<about> <key>=<value>`,
/// `<about> !#<id> <reason>`, or `<about> <fact>`. `<about>` is a path — it holds a `/` or
/// starts with `.`.
#[must_use]
pub fn from_unsaid(payload: &str, by: &str, source: Option<Anchor>) -> Option<Event> {
    let (about, rest) = payload.trim().split_once(char::is_whitespace)?;
    if !(about.contains('/') || about.starts_with('.')) {
        return None;
    }
    let path = fact_path(about)?;
    let rest = rest.trim();
    if let Some(countered) = rest.strip_prefix("!#") {
        let (id, reason) = countered
            .split_once(char::is_whitespace)
            .unwrap_or((countered, ""));
        return Some(Event::Counter {
            id: id.to_owned(),
            by: by.to_owned(),
            reason: reason.trim().to_owned(),
        });
    }
    let text = fact_text(rest);
    if text.is_empty() {
        return None;
    }
    Some(Event::Support {
        id: fact_id(&path, &text),
        about: path,
        text,
        by: by.to_owned(),
        source,
    })
}

/// A keyed fact's `(key, normalised value)`, when its text is `key=value` for a known key.
fn keyed(text: &str) -> Option<(String, String)> {
    let (k, v) = text.split_once('=')?;
    let k = k.trim();
    KEYS.iter().any(|(key, _)| *key == k).then(|| {
        // Commands with whitespace collapsed, each path in them normalised.
        let v = v
            .split_whitespace()
            .map(|w| {
                if w.contains('/') {
                    normal(w).unwrap_or_else(|| w.to_owned())
                } else {
                    w.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        (k.to_owned(), v)
    })
}

/// Whether `a` lies at or under `b` (paths as facts name them).
fn under(a: &str, b: &str) -> bool {
    let b = b.trim_end_matches('/');
    a == b || a.starts_with(&format!("{b}/"))
}

/// All the facts of a world, folded.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// Every fact, by id.
    pub facts: BTreeMap<String, Fact>,
    /// The trusted events they were folded from, in order.
    pub events: Vec<Event>,
    /// Why the facts cannot be read, when they cannot: the trusted journal does not walk whole,
    /// or there is no home to keep it under. No fact then stands, settled or not.
    pub unavailable: Option<String>,
    /// What the in-world copy says that the trusted journal does not: ignored, and flagged.
    pub untrusted: Vec<Untrusted>,
    /// The facts the in-world file read settled when it was imported: their settles were dropped,
    /// unproven, and the human may settle them again.
    pub unproven: BTreeSet<String>,
}

/// A fact the in-world copy reads with more standing than the trusted journal gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untrusted {
    /// Its id.
    pub id: String,
    /// The path it is about.
    pub about: String,
    /// Its text.
    pub text: String,
    /// The status the in-world copy reads.
    pub said: Status,
    /// Who the in-world copy says settled it.
    pub settled_by: Option<String>,
    /// Its status in the trusted journal; `None` when it holds no such fact.
    pub trusted: Option<Status>,
}

impl Untrusted {
    /// The mismatch, in words.
    #[must_use]
    pub fn message(&self) -> String {
        let by = self
            .settled_by
            .as_ref()
            .map(|b| format!(" by `{}`", b.escape_debug()))
            .unwrap_or_default();
        let trusted = self.trusted.map_or_else(
            || "holds no such fact".to_owned(),
            |t| format!("reads it {}", t.word()),
        );
        format!(
            "#{} `{}`: the in-world {FACTS} reads it {}{by}, the trusted journal {trusted} — the \
             in-world file is a copy, never trusted",
            self.id,
            self.text.escape_debug(),
            self.said.word(),
        )
    }
}

/// Where `world`'s trusted facts are kept: `$XDG_STATE_HOME/mnemorph/facts/<world>-<fnv64a>.jsonl`,
/// else under `~/.local/state`; `None` with no home.
#[must_use]
pub fn trusted_of(world: &Path) -> Option<PathBuf> {
    Some(trusted_in(&crate::queue::state_dir()?, world))
}

/// [`trusted_of`], the human's state directory at `state`. The world is named by its real path,
/// so every spelling of it finds one journal.
#[must_use]
pub fn trusted_in(state: &Path, world: &Path) -> PathBuf {
    let real = world.canonicalize().unwrap_or_else(|_| world.to_path_buf());
    crate::queue::state_file_in(state, &real, KIND)
}

/// The trusted events at `path`, its chain walked first and held to `anchor`: a journal that does
/// not walk whole is an error, never read as fewer facts.
fn trusted_events(path: &Path, anchor: Option<&Head>) -> Result<Vec<Event>, String> {
    let report = chain::verify_against(path, anchor)
        .map_err(|e| format!("the trusted facts cannot be read: {e}"))?;
    if !report.holds() {
        return Err(format!("the trusted facts do not verify: {report}"));
    }
    let back = Journal::<Event>::chained(path)
        .read()
        .map_err(|e| format!("the trusted facts cannot be read: {e}"))?;
    if let Some(line) = back.skipped.first() {
        return Err(format!(
            "the trusted facts do not verify: line {line} does not read"
        ));
    }
    Ok(back.rows)
}

/// The events the in-world copy holds, read through no link and under a cap; none when it cannot
/// be read. Every line that reads as an event counts, chained or not: whatever it says is a
/// stranger's until the trusted journal says it too, and nothing a stranger wrote is let off for
/// breaking a chain.
fn mirrored(world: &Path) -> Vec<Event> {
    let Ok(Some(bytes)) = crate::beneath::read(world, FACTS, MIRROR_CAP) else {
        return Vec::new();
    };
    bytes
        .split(|b| *b == b'\n')
        .filter_map(|line| serde_json::from_slice(&chain::body(line)).ok())
        .collect()
}

/// The in-world `events` as they are imported: each fact said once, by [`IMPORTED`], with its
/// anchor, under the id its path and text hash to; no counter, label or settle — none can be
/// proven — and an [`Event::Import`] naming the facts that read settled.
fn imported(events: &[Event]) -> Vec<Event> {
    let said = Facts::fold(events);
    let mut out: Vec<Event> = Vec::new();
    let mut seen = BTreeSet::new();
    for e in events {
        if let Event::Support {
            about,
            text,
            source,
            ..
        } = e
        {
            let Some(path) = fact_path(about) else {
                continue;
            };
            let text = fact_text(text);
            let id = fact_id(&path, &text);
            if text.is_empty() || !seen.insert(id.clone()) {
                continue;
            }
            out.push(Event::Support {
                id,
                about: path,
                text,
                by: IMPORTED.to_owned(),
                source: source.clone(),
            });
        }
    }
    out.push(Event::Import {
        settles: said
            .facts
            .values()
            .filter(|f| f.settled)
            .map(|f| f.id.clone())
            .collect(),
        read: events.len(),
    });
    out
}

/// What the in-world copy `said` reads with more standing than `trusted` gives: a fact it reads
/// settled that the trusted journal does not, one it reads agreed that is a single voice there,
/// or one the trusted journal does not hold at all.
fn untrusted(trusted: &Facts, said: &Facts) -> Vec<Untrusted> {
    said.live()
        .filter_map(|f| {
            let s = f.status();
            let t = trusted
                .facts
                .get(&f.id)
                .filter(|t| t.merged_into.is_none())
                .map(Fact::status);
            let more = match t {
                None => true,
                Some(t) => {
                    (s == Status::Settled && t != Status::Settled)
                        || (s == Status::Agreed && t == Status::Single)
                }
            };
            more.then(|| Untrusted {
                id: f.id.clone(),
                about: f.about.clone(),
                text: f.text.clone(),
                said: s,
                settled_by: f
                    .settled_by
                    .as_ref()
                    .filter(|_| s == Status::Settled)
                    .map(|(b, _)| b.clone()),
                trusted: t,
            })
        })
        .collect()
}

/// Every write to the trusted facts is refused inside a body's command (`MNEMORPH_BODY` is set
/// there), and the sandbox keeps the state directory out of every command's reach besides.
fn refuse_a_body() -> std::io::Result<()> {
    match std::env::var_os("MNEMORPH_BODY") {
        Some(body) => Err(std::io::Error::other(format!(
            "{} runs this: facts are filed by the harness and settled by the human or an Orc's \
             verdict, never by a body's command",
            body.to_string_lossy()
        ))),
        None => Ok(()),
    }
}

/// The in-world copy written again from the trusted journal at `path`, whole, through no link and
/// under the journal's lock, so it reads as the trusted journal does. A copy that cannot be
/// written is said on stderr: it grants nothing either way.
fn mirror(world: &Path, path: &Path) {
    let copied = (|| -> std::io::Result<()> {
        let file = std::fs::File::open(path)?;
        file.lock()?;
        let bytes = std::fs::read(path)?;
        crate::beneath::write(world, FACTS, &bytes, false)
    })();
    if let Err(e) = copied {
        eprintln!("mnemorph: the in-world copy of the facts could not be written: {e}");
    }
}

/// The trusted journal at `path` begun from the in-world `events`, once: written whole beside it
/// and linked into place only where nothing stands, so two first readers import once.
fn import(world: &Path, path: &Path, events: &[Event]) -> std::io::Result<Vec<Event>> {
    refuse_a_body()?;
    let rows = imported(events);
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    mnemorph_ledger::private_dir(dir)?;
    let fresh = dir.join(format!(
        ".{}.import-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&fresh);
    let j = Journal::<Event>::chained(&fresh);
    let linked = rows
        .iter()
        .try_for_each(|r| j.append(r))
        .and_then(|()| std::fs::hard_link(&fresh, path));
    let _ = std::fs::remove_file(&fresh);
    match linked {
        Ok(()) => {}
        // Another reader imported first: its journal stands.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return trusted_events(path, None).map_err(std::io::Error::other);
        }
        Err(e) => return Err(e),
    }
    mirror(world, path);
    let settles = match rows.last() {
        Some(Event::Import { settles, .. }) => settles.clone(),
        _ => Vec::new(),
    };
    let mut lines = vec![(
        "Imported".to_owned(),
        format!(
            "{} in-world events of {FACTS} into the trusted journal under the human's state \
             directory, each fact as one unproven voice, unsettled",
            events.len()
        ),
    )];
    if !settles.is_empty() {
        lines.push((
            "Re-settle".to_owned(),
            format!(
                "the in-world file read {} settled, which cannot be proven: settle what matters \
                 again with `mnemorphd onto settle <id>`",
                settles
                    .iter()
                    .map(|id| format!("#{id}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    if let Err(e) = crate::log::append(
        world,
        &crate::log::Entry {
            body: "harness".to_owned(),
            title: "facts: the in-world journal imported, unsettled".to_owned(),
            lines,
        },
    ) {
        eprintln!("mnemorph: the facts' import could not be journaled: {e}");
    }
    Ok(rows)
}

impl Facts {
    /// The facts of a world that cannot be read, and why.
    fn unavailable(why: String) -> Facts {
        Facts {
            unavailable: Some(why),
            ..Facts::default()
        }
    }

    /// Read the world's trusted facts and fold them, beside what its in-world copy says beyond
    /// them.
    #[must_use]
    pub fn load(world: &Path) -> Facts {
        Facts::load_in(crate::queue::state_dir().as_deref(), world)
    }

    /// [`Facts::load`], the human's state directory at `state` (`None`: no home). A world with no
    /// trusted journal, none ever anchored, imports its in-world facts once; one whose journal
    /// does not walk whole reads unavailable.
    #[must_use]
    pub fn load_in(state: Option<&Path>, world: &Path) -> Facts {
        let Some(state) = state else {
            return Facts::unavailable(
                "no home: the trusted facts are kept under the human's state directory".into(),
            );
        };
        let mut said = mirrored(world);
        let path = trusted_in(state, world);
        let anchor = crate::chains::anchor_under(state, world, KIND);
        let events = if !path.exists() && anchor.is_none() && !said.is_empty() {
            // Where the import cannot be written — a sandboxed reader — it is read as written.
            match import(world, &path, &said) {
                Ok(events) => {
                    said = mirrored(world);
                    events
                }
                Err(_) => imported(&said),
            }
        } else {
            match trusted_events(&path, anchor.as_ref()) {
                Ok(events) => events,
                Err(why) => return Facts::unavailable(why),
            }
        };
        let mut facts = Facts::fold(&events);
        facts.untrusted = untrusted(&facts, &Facts::fold(&said));
        facts
    }

    /// Append one event to the world's trusted facts, then write its in-world copy again. A fact
    /// it leaves contested that was not before marks for review every claim in memory resting on
    /// it, and what rests on those.
    ///
    /// # Errors
    ///
    /// A body asks, there is no home, the facts are unavailable, or the journal cannot be written.
    /// Memory that cannot be marked is said on stderr: the fact is filed either way.
    pub fn record(world: &Path, event: &Event) -> std::io::Result<()> {
        Facts::record_in(crate::queue::state_dir().as_deref(), world, event)
    }

    /// [`Facts::record`], the human's state directory at `state`.
    ///
    /// # Errors
    ///
    /// As [`Facts::record`].
    pub fn record_in(state: Option<&Path>, world: &Path, event: &Event) -> std::io::Result<()> {
        refuse_a_body()?;
        let Some(state) = state else {
            return Err(std::io::Error::other(
                "no home: the trusted facts are kept under the human's state directory",
            ));
        };
        let contested = |f: &Facts| -> BTreeSet<String> {
            f.facts
                .values()
                .filter(|x| x.status() == Status::Contested)
                .map(|x| x.id.clone())
                .collect()
        };
        let loaded = Facts::load_in(Some(state), world);
        if let Some(why) = loaded.unavailable {
            return Err(std::io::Error::other(why));
        }
        let before = contested(&loaded);
        let path = trusted_in(state, world);
        Journal::<Event>::chained(&path).append(event)?;
        mirror(world, &path);
        for id in contested(&Facts::load_in(Some(state), world)).difference(&before) {
            if let Err(e) = mnemorph_memory::invalidate(
                world,
                mnemorph_memory::Moved::Fact(id),
                &format!("fact #{id} it rests on is contested"),
            ) {
                eprintln!("mnemorph: the claims resting on fact #{id} could not be marked: {e}");
            }
        }
        Ok(())
    }

    /// Settle the fact `id` — `#<id>` or `<id>` — by `by`, citing `proof`: the human's act
    /// (`mnemorphd onto settle`, the board's settle), the one besides an Orc's verdict that
    /// settles; recency never does. Refused when the trusted facts hold no such fact, or when it
    /// no longer says what the human read: `seen_as` is its `mnemorph_memory::seen` digest,
    /// checked against the fact read afresh right before the write. Returns the id settled.
    ///
    /// # Errors
    ///
    /// No fact has the id, it changed since it was read, or the journal cannot be written.
    pub fn settle(
        world: &Path,
        id: &str,
        by: &str,
        proof: &str,
        seen_as: &str,
    ) -> Result<String, String> {
        Facts::settle_in(
            crate::queue::state_dir().as_deref(),
            world,
            id,
            by,
            proof,
            seen_as,
        )
    }

    /// [`Facts::settle`], the human's state directory at `state`.
    ///
    /// # Errors
    ///
    /// As [`Facts::settle`].
    pub fn settle_in(
        state: Option<&Path>,
        world: &Path,
        id: &str,
        by: &str,
        proof: &str,
        seen_as: &str,
    ) -> Result<String, String> {
        let id = id.trim_start_matches('#');
        let mut facts = Facts::load_in(state, world);
        if let Some(why) = facts.unavailable {
            return Err(why);
        }
        let Some(fact) = facts.facts.remove(id) else {
            return Err(format!("no fact `#{id}` in this world's ontology"));
        };
        // Read afresh right before the write: a fact rewritten since the human read it is refused.
        if mnemorph_memory::seen(&fact.id, &fact.text) != seen_as {
            return Err(mnemorph_memory::changed(&format!("#{id}")));
        }
        let event = Event::Settle {
            id: id.to_owned(),
            by: by.to_owned(),
            proof: proof.to_owned(),
        };
        Facts::record_in(state, world, &event)
            .map(|()| id.to_owned())
            .map_err(|e| format!("the settle could not be filed: {e}"))
    }

    /// Fold events, in the order written, into facts; then the collisions among keyed facts
    /// counter each other, and the parent's labels merge, counter or mark.
    #[must_use]
    pub fn fold(events: &[Event]) -> Facts {
        let mut facts: BTreeMap<String, Fact> = BTreeMap::new();
        let mut unproven = BTreeSet::new();
        // A fact the parent merged into another answers for it from then on.
        let resolve = |facts: &BTreeMap<String, Fact>, id: &str| {
            let mut id = id.to_owned();
            for _ in 0..16 {
                match facts.get(&id).and_then(|f| f.merged_into.clone()) {
                    Some(next) => id = next,
                    None => break,
                }
            }
            id
        };
        for (seq, e) in events.iter().enumerate() {
            match e {
                Event::Support {
                    id,
                    about,
                    text,
                    by,
                    source,
                } => {
                    let into = resolve(&facts, id);
                    let f = facts.entry(into).or_insert_with(|| Fact {
                        id: id.clone(),
                        about: fact_path(about).unwrap_or_else(|| about.clone()),
                        text: text.clone(),
                        keyed: keyed(text),
                        seq,
                        ..Fact::default()
                    });
                    f.supporters.insert(by.clone());
                    f.counters.remove(by);
                    f.reasons.remove(by);
                    f.last_support = seq;
                    if source.is_some() {
                        f.source.clone_from(source);
                    }
                }
                Event::Counter { id, by, reason } => {
                    let into = resolve(&facts, id);
                    if let Some(f) = facts.get_mut(&into) {
                        f.counters.insert(by.clone());
                        f.supporters.remove(by);
                        f.reasons.insert(by.clone(), reason.clone());
                        f.settled = false;
                    }
                }
                Event::Settle { id, by, proof } => {
                    let into = resolve(&facts, id);
                    if let Some(f) = facts.get_mut(&into) {
                        f.settled = true;
                        f.settled_at = Some(seq);
                        f.settled_by = Some((by.clone(), proof.clone()));
                    }
                }
                Event::Import { settles, .. } => unproven.extend(settles.iter().cloned()),
                Event::Label { new, label, of, .. } => {
                    if of.as_deref() == Some(new.as_str()) {
                        continue;
                    }
                    let supporters = facts
                        .get(new)
                        .map(|f| f.supporters.clone())
                        .unwrap_or_default();
                    if let Some(f) = facts.get_mut(new) {
                        f.labelled = true;
                    }
                    let of = of.as_ref().map(|o| resolve(&facts, o));
                    if let Some(target) = of.as_ref().and_then(|o| facts.get_mut(o)) {
                        // A creature stands on one side of a fact, never both.
                        match label {
                            Label::Support => {
                                for s in &supporters {
                                    target.counters.remove(s);
                                }
                                target.supporters.extend(supporters);
                            }
                            Label::Counter => {
                                for s in &supporters {
                                    target.supporters.remove(s);
                                }
                                target.counters.extend(supporters);
                            }
                            Label::Refines | Label::Distinct => {}
                        }
                    }
                    if *label == Label::Support
                        && let Some(f) = facts.get_mut(new)
                    {
                        f.merged_into = of;
                    }
                }
            }
        }
        collide(&mut facts);
        Facts {
            facts,
            events: events.to_vec(),
            unproven,
            ..Facts::default()
        }
    }

    /// The live facts: not merged into another.
    pub fn live(&self) -> impl Iterator<Item = &Fact> {
        self.facts.values().filter(|f| f.merged_into.is_none())
    }

    /// Free facts that collided with `fact` when it landed: older live free facts on its path, a
    /// path above it or below it.
    #[must_use]
    pub fn collisions(&self, fact: &Fact) -> Vec<&Fact> {
        self.live()
            .filter(|o| {
                o.keyed.is_none()
                    && o.id != fact.id
                    && o.seq < fact.seq
                    && (under(&fact.about, &o.about) || under(&o.about, &fact.about))
            })
            .collect()
    }

    /// The facts a parent is shown to label a new free fact against: the three that rank highest
    /// by BM25 against its text, above `floor`. Similarity only chooses what to compare.
    #[must_use]
    pub fn candidates(&self, fact: &Fact, floor: f64) -> Vec<&Fact> {
        let pool = self.collisions(fact);
        let docs: Vec<Vec<String>> = pool.iter().map(|f| words(&f.text)).collect();
        let query = words(&fact.text);
        let mut ranked: Vec<(f64, &Fact)> = pool
            .iter()
            .zip(&docs)
            .map(|(f, d)| (bm25(&query, d, &docs), *f))
            .filter(|(s, _)| *s > floor)
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        ranked.into_iter().take(3).map(|(_, f)| f).collect()
    }

    /// A body's projection of the paths it touches: each path's owner and at most `k` of its
    /// facts — contested first, both sides shown with the counters' reasons, then countered,
    /// then the rest by recency — a stale fact marked so, and `@ONTO -<n>` counting the distinct
    /// facts cut.
    #[must_use]
    pub fn projection(
        &self,
        world: &Path,
        owner_of: impl Fn(&str) -> String,
        paths: &[&str],
        k: usize,
    ) -> Vec<String> {
        let mut out = Vec::new();
        let mut shown_ids: BTreeSet<&str> = BTreeSet::new();
        let mut seen_ids: BTreeSet<&str> = BTreeSet::new();
        for path in paths {
            let here_path = fact_path(path).unwrap_or_else(|| (*path).to_owned());
            let mut here: Vec<&Fact> = self
                .live()
                .filter(|f| under(&here_path, &f.about) || under(&f.about, &here_path))
                .collect();
            here.sort_by_key(|f| {
                let rank = match f.status() {
                    Status::Contested => 0,
                    Status::Countered => 1,
                    _ => 2,
                };
                (rank, std::cmp::Reverse(f.seq))
            });
            seen_ids.extend(here.iter().map(|f| f.id.as_str()));
            let mut shown: Vec<String> = Vec::new();
            for f in here.iter().take(k) {
                shown_ids.insert(f.id.as_str());
                let against: Vec<String> = f
                    .counters
                    .iter()
                    .map(|c| match f.reasons.get(c).filter(|r| !r.is_empty()) {
                        Some(r) => format!("{c}: {r}"),
                        None => c.clone(),
                    })
                    .collect();
                let against = if against.is_empty() {
                    String::new()
                } else {
                    format!(" (countered by {})", against.join("; "))
                };
                let stale = if f.stale(world) { " [stale]" } else { "" };
                // A fact's text is a creature's words: one short line, no bracket of its own, so it
                // never passes for a status or a stale mark.
                let said: String = f
                    .text
                    .chars()
                    .map(|c| {
                        if c.is_control() || "[]()".contains(c) {
                            ' '
                        } else {
                            c
                        }
                    })
                    .take(200)
                    .collect();
                shown.push(format!(
                    "#{} {} {}{against}{stale}",
                    f.id,
                    f.status().word(),
                    said
                ));
            }
            let owner = owner_of(path);
            out.push(if shown.is_empty() {
                format!("@ONTO {path}: {owner}")
            } else {
                format!("@ONTO {path}: {owner} · {}", shown.join(" · "))
            });
        }
        let cut = seen_ids.difference(&shown_ids).count();
        if cut > 0 {
            out.push(format!("@ONTO -{cut}"));
        }
        out
    }
}

/// Keyed facts, compared by code: one value per key per path, inherited where the key is. A
/// colliding assertion counters, and reopens a settle it came after.
fn collide(facts: &mut BTreeMap<String, Fact>) {
    let keyed: Vec<(String, String, String, String, BTreeSet<String>, usize)> = facts
        .values()
        .filter(|f| f.merged_into.is_none())
        .filter_map(|f| {
            f.keyed.as_ref().map(|(k, v)| {
                (
                    f.id.clone(),
                    f.about.clone(),
                    k.clone(),
                    v.clone(),
                    f.supporters.clone(),
                    f.last_support,
                )
            })
        })
        .collect();
    for (id, about, k, v, _, _) in &keyed {
        let inherited = KEYS.iter().any(|(key, inh)| key == k && *inh);
        for (other, oabout, ok, ov, osupport, olast) in &keyed {
            let collide = ok == k
                && ov != v
                && (oabout == about
                    || (inherited && (under(about, oabout) || under(oabout, about))));
            if collide
                && other != id
                && let Some(f) = facts.get_mut(id)
            {
                f.counters.extend(
                    osupport
                        .iter()
                        .filter(|s| !f.supporters.contains(*s))
                        .cloned(),
                );
                if f.settled_at.is_some_and(|at| *olast > at) {
                    f.settled = false;
                }
            }
        }
    }
}
