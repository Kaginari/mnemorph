//! The unproven write: a note event whose author or proof does not resolve — a hand edit, a bad
//! merge, an import that skipped its checks. Recall ignores it, the creature's memory writes freeze
//! and `memory status` tells the human.

use std::collections::BTreeSet;
use std::path::Path;

use crate::note::{By, Change, Event, Status};
use crate::store::{Tier, read};

/// The routes a `Route` event's proof may name, as `<route>: <what>`.
const ROUTES: [&str; 4] = ["checked", "accepted", "corroborated", "confirmed"];

/// Why an event of one tier does not resolve, or `None` when it does. A standing — grounded, or a
/// confidence — is claimed only by a route citing its proof, the gate citing its verdict, the human,
/// or a distillation or abstraction of notes this tier grounded; an import always cites its source.
#[must_use]
pub fn why_unproven(e: &Event, grounded: &BTreeSet<&str>) -> Option<String> {
    let proof = e.proof.as_deref().map(str::trim).filter(|p| !p.is_empty());
    let claims = matches!(
        e.change,
        Change::Status {
            status: Status::Grounded
        } | Change::Confidence { .. }
    );
    match e.by {
        By::Route => match proof.and_then(|p| p.split_once(": ")) {
            Some((route, what)) if ROUTES.contains(&route) && !what.trim().is_empty() => None,
            _ => Some("a route that names no proof".into()),
        },
        By::Gate if proof.is_none() => Some("a gate event citing no verdict".into()),
        By::Import if proof.is_none() => Some("an import citing no source".into()),
        By::Record if claims => {
            let rested = proof.and_then(|p| {
                p.strip_prefix("distilled: ")
                    .or_else(|| p.strip_prefix("abstracted: "))
            });
            match rested {
                Some(ids) if ids.split(", ").all(|id| grounded.contains(id.trim())) => None,
                Some(_) => Some("a schema resting on a note this tier never grounded".into()),
                None => Some("a standing claimed by a plain record".into()),
            }
        }
        By::Human | By::Gate | By::Import | By::Record => None,
    }
}

/// The events of one tier that do not resolve, each with why.
#[must_use]
pub fn unproven(events: &[Event]) -> Vec<(&Event, String)> {
    // A note ever grounded by an event that itself resolves — a schema rests on those.
    let none = BTreeSet::new();
    let grounded: BTreeSet<&str> = events
        .iter()
        .filter(|e| {
            matches!(
                e.change,
                Change::Status {
                    status: Status::Grounded
                }
            ) && !matches!(e.by, By::Record)
                && why_unproven(e, &none).is_none()
        })
        .map(|e| e.note.as_str())
        .collect();
    events
        .iter()
        .filter_map(|e| why_unproven(e, &grounded).map(|why| (e, why)))
        .collect()
}

/// The events of one tier, those that do not resolve left out: what recall folds.
#[must_use]
pub fn resolved(events: Vec<Event>) -> (Vec<Event>, usize) {
    let bad: BTreeSet<String> = unproven(&events)
        .into_iter()
        .map(|(e, _)| e.id.clone())
        .collect();
    let n = bad.len();
    (
        events
            .into_iter()
            .filter(|e| !bad.contains(&e.id))
            .collect(),
        n,
    )
}

/// A finding: one unresolved event, where it lies.
#[derive(Debug, Clone, PartialEq)]
pub struct Unproven {
    /// The tier's file, relative to the world.
    pub file: String,
    /// The event's id.
    pub event: String,
    /// The note it is about.
    pub note: String,
    /// Why it does not resolve.
    pub why: String,
}

/// The unresolved events touching `owner`: in its personal and short tiers, and in shared memory
/// on a note it owns. Any one freezes its memory writes.
///
/// # Errors
///
/// A note file cannot be read.
pub fn findings(world: &Path, owner: &str) -> std::io::Result<Vec<Unproven>> {
    let mut out = Vec::new();
    for tier in [
        Tier::Personal(owner.to_owned()),
        Tier::Short(owner.to_owned()),
    ] {
        let rows = read(world, &tier)?.rows;
        push(&mut out, &tier, &unproven(&rows));
    }
    let shared = read(world, &Tier::Shared)?.rows;
    let owns: BTreeSet<&str> = shared
        .iter()
        .filter(|e| matches!(&e.change, Change::Create { owner: Some(o), .. } if o == owner))
        .map(|e| e.note.as_str())
        .collect();
    let mine: Vec<(&Event, String)> = unproven(&shared)
        .into_iter()
        .filter(|(e, _)| owns.contains(e.note.as_str()))
        .collect();
    push(&mut out, &Tier::Shared, &mine);
    Ok(out)
}

fn push(out: &mut Vec<Unproven>, tier: &Tier, bad: &[(&Event, String)]) {
    out.extend(bad.iter().map(|(e, why)| Unproven {
        file: tier.file().display().to_string(),
        event: e.id.clone(),
        note: e.note.clone(),
        why: why.clone(),
    }));
}

/// Refuse a write for `owner` while an unresolved event touches it: its memory is frozen until the
/// human removes or re-proves the event.
///
/// # Errors
///
/// The memory is frozen, or a note file cannot be read.
pub fn unfrozen(world: &Path, owner: &str) -> std::io::Result<()> {
    let found = findings(world, owner)?;
    match found.first() {
        None => Ok(()),
        Some(f) => Err(std::io::Error::other(format!(
            "{owner}'s memory writes are frozen: {} event(s) do not resolve, the first `{}` in {} \
             ({}); run `mnemorphd memory status`",
            found.len(),
            f.event,
            f.file,
            f.why
        ))),
    }
}

/// Every creature with memory in `world` — a personal or short tier, or a shared note it owns —
/// and the unresolved events touching it, by name.
///
/// # Errors
///
/// A note file or the memory directory cannot be read.
pub fn status(world: &Path) -> std::io::Result<Vec<(String, Vec<Unproven>)>> {
    let mut names = BTreeSet::new();
    let base = world.join(".mnemorph/memory");
    for dir in ["creature", "short"] {
        let Ok(rd) = std::fs::read_dir(base.join(dir)) else {
            continue;
        };
        for entry in rd {
            let path = entry?.path();
            if path.extension().is_some_and(|x| x == "jsonl")
                && let Some(stem) = path.file_stem()
            {
                names.insert(stem.to_string_lossy().into_owned());
            }
        }
    }
    for e in read(world, &Tier::Shared)?.rows {
        if let Change::Create { owner: Some(o), .. } = e.change {
            names.insert(o);
        }
    }
    names
        .into_iter()
        .map(|n| findings(world, &n).map(|f| (n, f)))
        .collect()
}
