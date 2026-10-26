//! Consolidation and forgetting: agreeing notes distilled into one schema note — dissent exempt,
//! so a summary never absorbs the exception it disagrees with — and three ways to forget, each
//! keeping its evidence but the last, which deletes text only where it can truly be deleted.

use std::path::Path;

use crate::note::{By, Change, Link, Note, Polarity, Status, fold};
use crate::store::{Tier, append, event_id, now_ms, read};

/// How a note is forgotten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forget {
    /// Out of recall; the note and its evidence kept.
    Dormant,
    /// Replaced by a schema note at lower salience and confidence; the evidence kept.
    Abstract,
    /// Its text removed, its event kept: the machine-local short tier only.
    Tombstone,
}

/// Distil `ids` from `tier` into one schema note saying `text`, `owner`'s or the world's: salience
/// 0.8, confidence the mean of the four most salient merged notes' (capped at 0.95), linked to
/// every note it merged; the two most salient originals stay active beside it, the rest go dormant.
/// Only grounded agreeing notes merge — a counter or revise note is never merged nor put to sleep —
/// and the schema's standing rests on them: its proof is built here, naming every note it merged,
/// never taken from the caller. Returns the schema note's id, or `None` when fewer than two agreeing
/// notes were given.
///
/// # Errors
///
/// The tier cannot be read or written.
pub fn distill(
    world: &Path,
    tier: &Tier,
    ids: &[String],
    text: &str,
    owner: Option<&str>,
) -> std::io::Result<Option<String>> {
    let notes = fold(&read(world, tier)?.rows);
    let mut merged: Vec<&Note> = ids
        .iter()
        .filter_map(|id| notes.get(id))
        .filter(|n| n.polarity == Polarity::Support && n.status == Status::Grounded)
        .collect();
    if merged.len() < 2 {
        return Ok(None);
    }
    merged.sort_by(|a, b| {
        b.salience
            .total_cmp(&a.salience)
            .then_with(|| a.id.cmp(&b.id))
    });
    let top: Vec<f64> = merged.iter().take(4).map(|n| n.confidence).collect();
    #[allow(clippy::cast_precision_loss, reason = "at most four")]
    let confidence = (top.iter().sum::<f64>() / top.len() as f64).min(0.95);
    let schema = format!("s{}", event_id(now_ms()));
    let merged_ids: Vec<&str> = merged.iter().map(|n| n.id.as_str()).collect();
    let proof = format!("distilled: {}", merged_ids.join(", "));
    let owner = owner.map(str::to_owned);
    append(
        world,
        tier,
        &schema,
        By::Record,
        Some(proof.clone()),
        Change::Create {
            text: text.to_owned(),
            tags: vec!["schema".into()],
            owner,
            polarity: Polarity::Support,
            salience: 0.8,
            confidence,
            source: None,
            links: merged
                .iter()
                .map(|n| Link {
                    note: n.id.clone(),
                    weight: 1.0,
                })
                .collect(),
        },
    )?;
    append(
        world,
        tier,
        &schema,
        By::Record,
        Some(proof.clone()),
        Change::Status {
            status: Status::Grounded,
        },
    )?;
    for n in merged.iter().skip(2) {
        append(
            world,
            tier,
            &n.id,
            By::Record,
            Some(proof.clone()),
            Change::Status {
                status: Status::Dormant,
            },
        )?;
    }
    Ok(Some(schema))
}

/// Forget the note `id` of `tier` by `mode`. `Abstract` writes `abstraction` as a schema note at
/// `max(0.4, s − 0.1)` and `max(0.3, κ − 0.1)` and puts the original to sleep; `Tombstone` empties
/// the note's text in its file and marks it tombstoned — on the short tier alone, since a tracked
/// tier's text has already travelled by git. Returns the schema note's id for `Abstract`.
///
/// # Errors
///
/// No such note; a tombstone outside the short tier; the tier cannot be read or written.
pub fn forget(
    world: &Path,
    tier: &Tier,
    id: &str,
    mode: Forget,
    abstraction: &str,
) -> std::io::Result<Option<String>> {
    let notes = fold(&read(world, tier)?.rows);
    let Some(n) = notes.get(id) else {
        return Err(std::io::Error::other(format!(
            "no note `{id}` in that tier"
        )));
    };
    match mode {
        Forget::Dormant => {
            append(
                world,
                tier,
                id,
                By::Record,
                None,
                Change::Status {
                    status: Status::Dormant,
                },
            )?;
            Ok(None)
        }
        Forget::Abstract => {
            let schema = format!("s{}", event_id(now_ms()));
            append(
                world,
                tier,
                &schema,
                By::Record,
                None,
                Change::Create {
                    text: abstraction.to_owned(),
                    tags: vec!["schema".into()],
                    owner: n.owner.clone(),
                    polarity: n.polarity,
                    salience: (n.salience - 0.1).max(0.4),
                    confidence: (n.confidence - 0.1).max(0.3),
                    source: n.source.clone(),
                    links: vec![Link {
                        note: id.to_owned(),
                        weight: 1.0,
                    }],
                },
            )?;
            // Its standing rests on the original's, named: never on a proof given for other text.
            if n.status == Status::Grounded {
                append(
                    world,
                    tier,
                    &schema,
                    By::Record,
                    Some(format!("abstracted: {id}")),
                    Change::Status {
                        status: Status::Grounded,
                    },
                )?;
            }
            append(
                world,
                tier,
                id,
                By::Record,
                None,
                Change::Status {
                    status: Status::Dormant,
                },
            )?;
            Ok(Some(schema))
        }
        Forget::Tombstone => {
            tombstone(world, tier, id)?;
            Ok(None)
        }
    }
}

/// Empty the note `id`'s text, tags and source in a short-tier file, keeping its events, and mark it
/// tombstoned: a tracked tier's text has travelled by git, so only the short tier is tombstoned.
fn tombstone(world: &Path, tier: &Tier, id: &str) -> std::io::Result<()> {
    if !matches!(tier, Tier::Short(_)) {
        return Err(std::io::Error::other(
            "a tracked note's text has travelled by git: only a short-tier note is tombstoned",
        ));
    }
    // The short tier is machine-local: its file is rewritten with the text gone.
    let path = world.join(tier.file());
    let back = read(world, tier)?;
    let mut text = String::new();
    for mut e in back.rows {
        if e.note == id
            && let Change::Create {
                text: t,
                tags,
                source,
                ..
            } = &mut e.change
        {
            t.clear();
            tags.clear();
            *source = None;
        }
        text.push_str(&serde_json_line(&e)?);
    }
    // Written new beside it, then renamed over it: a crash leaves the old file whole.
    let temp = path.with_extension("jsonl.tombstone");
    let _ = std::fs::remove_file(&temp);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()))
        .and_then(|()| std::fs::rename(&temp, &path))?;
    append(
        world,
        tier,
        id,
        By::Record,
        None,
        Change::Status {
            status: Status::Tombstoned,
        },
    )?;
    Ok(())
}

fn serde_json_line(e: &crate::note::Event) -> std::io::Result<String> {
    let mut line = serde_json::to_string(e).map_err(std::io::Error::other)?;
    line.push('\n');
    Ok(line)
}
