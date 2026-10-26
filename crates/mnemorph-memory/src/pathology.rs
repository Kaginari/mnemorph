//! Pathologies as instruments, read by code at each recall: the replay loop — the same recall
//! again and again with nothing landing and no dissent shown — and lock-in — a revision newer than
//! every agreeing note recalled, while no dissent was selected. Each has its lawful act.

use std::path::{Path, PathBuf};

use mnemorph_ledger::Journal;
use serde::{Deserialize, Serialize};

use crate::note::{Note, Polarity, Status};
use crate::recall::{Hit, Recalled, Relation};
use crate::text::{bm25, content_words};

/// How many recalls the replay loop looks back over, the current one among them.
pub const WINDOW: usize = 5;
/// How many of them, the current one among them, make a replay loop.
pub const REPLAYS: usize = 3;

/// A recall's fingerprint: SHA-256 over the query, the selected anchors' ids and the office.
#[must_use]
pub fn fingerprint(query: &str, anchors: &[String], office: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for part in [query, &anchors.join("\u{1f}"), office] {
        h.update(part.as_bytes());
        h.update([0x1e]);
    }
    h.finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, b| {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
            hex
        })
}

/// One ended ask's recall, kept machine-local for the replay loop and `memory status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    /// When, in milliseconds since the epoch.
    pub time: u64,
    /// The recall's fingerprint.
    pub fingerprint: String,
    /// Whether a dissenting note sat among its anchors.
    pub dissent: bool,
    /// What the ask landed — the gate's journal heading — or `None` when nothing landed.
    pub outcome: Option<String>,
    /// The act a pathology forced into it, if any: `replay`, `replay-escalated`, `lock-in`.
    #[serde(default)]
    pub forced: Option<String>,
}

/// A creature's recall traces in `world`: in the machine-local short tier, never tracked.
#[must_use]
pub fn traces_file(world: &Path, name: &str) -> PathBuf {
    world
        .join(".mnemorph/memory/short/recalls")
        .join(format!("{name}.jsonl"))
}

/// Append an ended ask's trace.
///
/// # Errors
///
/// The traces file cannot be written.
pub fn trace(world: &Path, name: &str, t: &Trace) -> std::io::Result<()> {
    Journal::<Trace>::new(traces_file(world, name)).append(t)
}

/// A creature's traces, oldest first; unreadable lines are left out.
///
/// # Errors
///
/// The traces file cannot be read.
pub fn traces(world: &Path, name: &str) -> std::io::Result<Vec<Trace>> {
    Ok(Journal::<Trace>::new(traces_file(world, name)).read()?.rows)
}

/// Whether a recall with `fingerprint` and no dissent, after `past` (oldest first), is a replay
/// loop: the same fingerprint at least [`REPLAYS`] times in the last [`WINDOW`] recalls, the
/// current one among them, every one of them with nothing landed and no dissent shown.
#[must_use]
pub fn replays(past: &[Trace], fingerprint: &str, dissent: bool) -> bool {
    if dissent {
        return false;
    }
    let window = &past[past.len().saturating_sub(WINDOW - 1)..];
    let same: Vec<&Trace> = window
        .iter()
        .filter(|t| t.fingerprint == fingerprint)
        .collect();
    same.len() + 1 >= REPLAYS && same.iter().all(|t| t.outcome.is_none() && !t.dissent)
}

/// Whether `r` is locked in: no dissent was selected, and a live revision linked to one of its
/// agreeing anchors is newer than every one of them. Returns the newest such revision.
#[must_use]
pub fn locked_in<'a>(r: &Recalled, notes: &'a [(Note, Relation)]) -> Option<&'a Note> {
    if r.anchors.is_empty() || r.anchors.iter().any(|h| h.note.polarity.dissents()) {
        return None;
    }
    let newest_agreeing = r.anchors.iter().map(|h| h.note.born).max()?;
    notes
        .iter()
        .map(|(n, _)| n)
        .filter(|n| {
            n.polarity == Polarity::Revise
                && n.status == Status::Grounded
                && n.born > newest_agreeing
                && n.links
                    .iter()
                    .any(|l| r.anchors.iter().any(|h| h.note.id == l.note))
        })
        .max_by(|a, b| a.born.cmp(&b.born).then_with(|| a.id.cmp(&b.id)))
}

/// The best dissenting note for `query`, the relevance floor set aside: the replay loop's act
/// when recall selected none. Grounded notes only, or the asker's own proposed ones; by the query's
/// words, then confidence, then the newest.
#[must_use]
pub fn best_dissent<'a>(query: &str, notes: &'a [(Note, Relation)]) -> Option<&'a Note> {
    let terms = content_words(query);
    let live: Vec<&Note> = notes
        .iter()
        .filter(|(n, rel)| {
            n.polarity.dissents()
                && (n.status == Status::Grounded
                    || (n.status == Status::Proposed && *rel == Relation::Own))
        })
        .map(|(n, _)| n)
        .collect();
    let docs: Vec<Vec<String>> = live
        .iter()
        .map(|n| content_words(&format!("{} {}", n.text, n.tags.join(" "))))
        .collect();
    live.iter()
        .zip(&docs)
        .map(|(n, doc)| (*n, bm25(&terms, doc, &docs)))
        .max_by(|(a, x), (b, y)| {
            x.total_cmp(y)
                .then_with(|| a.confidence.total_cmp(&b.confidence))
                .then_with(|| a.born.cmp(&b.born))
                .then_with(|| b.id.cmp(&a.id))
        })
        .map(|(n, _)| n)
}

/// Seat `note` among `r`'s anchors: in a free seat while fewer than `k` sit, else in the place of
/// the lowest-ranked agreeing anchor, which is counted cut.
pub fn force(r: &mut Recalled, note: &Note, k: usize) {
    if r.anchors.iter().any(|h| h.note.id == note.id) {
        return;
    }
    if r.anchors.len() >= k.max(2)
        && let Some(i) = r.anchors.iter().rposition(|h| !h.note.polarity.dissents())
    {
        let out = r.anchors.remove(i);
        r.cut += 1;
        r.cut_ids.insert(0, out.note.id);
        r.cut_ids.truncate(3);
    }
    r.anchors.push(Hit {
        note: note.clone(),
        score: 0.0,
        meaning: 0.0,
    });
}

/// What a creature's last [`WINDOW`] traces show, each a line for `memory status`: a replay loop
/// among them, and each act a pathology forced. Empty when they show nothing.
#[must_use]
pub fn findings(past: &[Trace]) -> Vec<String> {
    let window = &past[past.len().saturating_sub(WINDOW)..];
    let mut out = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for t in window {
        if seen.contains(&t.fingerprint.as_str()) {
            continue;
        }
        seen.push(&t.fingerprint);
        let same: Vec<&Trace> = window
            .iter()
            .filter(|u| u.fingerprint == t.fingerprint)
            .collect();
        if same.len() >= REPLAYS && same.iter().all(|u| u.outcome.is_none() && !u.dissent) {
            out.push(format!(
                "replay loop: the same recall {} times in the last {}, nothing landed, no correction \
                 shown",
                same.len(),
                window.len()
            ));
        }
    }
    for act in ["lock-in", "replay", "replay-escalated"] {
        let n = window
            .iter()
            .filter(|t| t.forced.as_deref() == Some(act))
            .count();
        if n > 0 {
            out.push(format!(
                "{act} acted on {n} of the last {} recalls",
                window.len()
            ));
        }
    }
    out
}

/// The creatures with recall traces in `world`, by name.
#[must_use]
pub fn traced(world: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(world.join(".mnemorph/memory/short/recalls")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}
