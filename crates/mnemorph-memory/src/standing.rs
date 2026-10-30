//! How much of a world's memory stands: the notes grounded in shared memory, those kept personal,
//! and the claim grounded last — what `memory status` prints first and the terminal UI's sidebar
//! shows.

use std::path::Path;

use crate::note::{Change, Status, fold};
use crate::store::{Tier, read};

/// A world's memory, counted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standing {
    /// Notes grounded in shared memory: the only ones other bodies recall.
    pub shared: usize,
    /// Personal notes proposed, not yet proved.
    pub proposed: usize,
    /// Personal notes grounded.
    pub grounded: usize,
    /// The text of the note grounded last, in any tier, while it still stands grounded.
    pub latest: Option<String>,
}

/// Read `world`'s standing: the shared tier and every creature's personal tier. A tier that
/// cannot be read counts nothing.
#[must_use]
pub fn standing(world: &Path) -> Standing {
    let mut s = Standing::default();
    let mut latest: Option<(u64, String)> = None;
    let mut tier = |tier: &Tier, personal: bool| {
        let Ok(r) = read(world, tier) else {
            return;
        };
        let notes = fold(&r.rows);
        for n in notes.values() {
            match (n.status, personal) {
                (Status::Grounded, false) => s.shared += 1,
                (Status::Grounded, true) => s.grounded += 1,
                (Status::Proposed, true) => s.proposed += 1,
                _ => {}
            }
        }
        for e in &r.rows {
            let grounded = matches!(
                e.change,
                Change::Status {
                    status: Status::Grounded
                }
            ) && notes
                .get(&e.note)
                .is_some_and(|n| n.status == Status::Grounded);
            if grounded
                && latest.as_ref().is_none_or(|(t, _)| e.time >= *t)
                && let Some(n) = notes.get(&e.note)
            {
                latest = Some((e.time, n.text.clone()));
            }
        }
    };
    tier(&Tier::Shared, false);
    let creatures = world.join(".mnemorph/memory/creature");
    if let Ok(rd) = std::fs::read_dir(creatures) {
        let mut names: Vec<String> = rd
            .filter_map(Result::ok)
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .collect();
        names.sort();
        for name in names {
            tier(&Tier::Personal(name), true);
        }
    }
    s.latest = latest.map(|(_, text)| text);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::{By, Event, Polarity};

    fn create(note: &str, time: u64, text: &str) -> Event {
        Event {
            id: format!("c{time}"),
            time,
            note: note.into(),
            by: By::Record,
            proof: None,
            change: Change::Create {
                text: text.into(),
                tags: vec![],
                owner: None,
                polarity: Polarity::Support,
                salience: 0.5,
                confidence: 0.5,
                source: None,
                links: vec![],
                evidence: vec![],
                depends_on: vec![],
            },
        }
    }

    fn status(note: &str, time: u64, status: Status) -> Event {
        Event {
            id: format!("s{time}"),
            time,
            note: note.into(),
            by: By::Route,
            proof: Some("log.md".into()),
            change: Change::Status { status },
        }
    }

    fn write(world: &Path, tier: &Tier, events: &[Event]) {
        let path = world.join(tier.file());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let lines: Vec<String> = events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect();
        std::fs::write(path, lines.join("\n") + "\n").unwrap();
    }

    #[test]
    fn it_counts_each_tier_and_names_the_claim_grounded_last() {
        let world = std::env::temp_dir().join(format!("mnemorph-standing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&world);
        write(
            &world,
            &Tier::Shared,
            &[
                create("a", 1, "the parser drops collations"),
                status("a", 2, Status::Grounded),
            ],
        );
        write(
            &world,
            &Tier::Personal("slime-x".into()),
            &[
                create("b", 3, "tests live in tests/"),
                status("b", 9, Status::Grounded),
                create("c", 4, "maybe"),
                create("d", 5, "grounded then dormant"),
                status("d", 10, Status::Grounded),
                status("d", 11, Status::Dormant),
            ],
        );
        let s = standing(&world);
        assert_eq!((s.shared, s.proposed, s.grounded), (1, 1, 1));
        assert_eq!(s.latest.as_deref(), Some("tests live in tests/"));
        assert_eq!(standing(&world.join("none")), Standing::default());
        let _ = std::fs::remove_dir_all(&world);
    }
}
