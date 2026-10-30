//! Territory facts: what creatures learned about a path, each with who said it and on what
//! evidence, kept as append-only events and folded into a status no one has to vote on.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemorph_ledger::Journal;
use mnemorph_memory::text::{bm25, words};
use mnemorph_wire::fnv64a;
use serde::{Deserialize, Serialize};

use crate::own::normal;

/// Where the events live: harness soil.
pub const FACTS: &str = ".mnemorph/ontology/facts.jsonl";

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
        let text = std::fs::read_to_string(world.join(file)).ok()?;
        let cited: Vec<&str> = text
            .lines()
            .skip(from.saturating_sub(1))
            .take(to + 1 - from.min(to))
            .collect();
        return Some(hash(&cited.join("\n")));
    }
    // A file is anchored by its whole text: what it says now is what the fact was said about.
    if world.join(at).is_file() {
        return std::fs::read_to_string(world.join(at))
            .ok()
            .map(|t| hash(&t));
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
}

impl Facts {
    /// Read the world's facts journal and fold it.
    #[must_use]
    pub fn load(world: &Path) -> Facts {
        let events = Journal::<Event>::new(world.join(FACTS))
            .read()
            .map(|r| r.rows)
            .unwrap_or_default();
        Facts::fold(&events)
    }

    /// Append one event to the world's facts journal. A fact it leaves contested that was not
    /// before marks for review every claim in memory resting on it, and what rests on those.
    ///
    /// # Errors
    ///
    /// The journal cannot be written. Memory that cannot be marked is said on stderr: the fact
    /// is filed either way.
    pub fn record(world: &Path, event: &Event) -> std::io::Result<()> {
        let contested = |f: &Facts| -> BTreeSet<String> {
            f.facts
                .values()
                .filter(|x| x.status() == Status::Contested)
                .map(|x| x.id.clone())
                .collect()
        };
        let before = contested(&Facts::load(world));
        Journal::<Event>::new(world.join(FACTS)).append(event)?;
        for id in contested(&Facts::load(world)).difference(&before) {
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

    /// Fold events, in the order written, into facts; then the collisions among keyed facts
    /// counter each other, and the parent's labels merge, counter or mark.
    #[must_use]
    pub fn fold(events: &[Event]) -> Facts {
        let mut facts: BTreeMap<String, Fact> = BTreeMap::new();
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
                Event::Settle { id, .. } => {
                    let into = resolve(&facts, id);
                    if let Some(f) = facts.get_mut(&into) {
                        f.settled = true;
                        f.settled_at = Some(seq);
                    }
                }
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
        Facts { facts }
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
