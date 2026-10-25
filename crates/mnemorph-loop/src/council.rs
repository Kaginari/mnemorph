//! The council: strain read per body across its runs — its failed acts, and one for every run its
//! report did not answer `DONE` — cleared by a clean landing. At `escalate_at` its parent must
//! answer as its manager; after `max_consults` such warrants its report is forced to escalate;
//! at `relieve_at` relief splits or retires it, and its strain starts again.

use std::path::Path;

use mnemorph_ledger::Journal;
use serde::{Deserialize, Serialize};

/// Where the council keeps its tally, under the world.
pub const COUNCIL: &str = ".mnemorph/instruments/council.jsonl";

/// The council's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    /// Strain at which the parent must answer as manager.
    pub escalate_at: u64,
    /// Strain at which relief acts.
    pub relieve_at: u64,
    /// Warrants after which the report is forced to escalate.
    pub max_consults: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            escalate_at: 5,
            relieve_at: 7,
            max_consults: 3,
        }
    }
}

/// One line of the tally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Row {
    /// Strain added by a run.
    Strain { body: String, by: u64 },
    /// A warrant served to the parent.
    Warrant { body: String },
    /// Cleared: a clean landing, or relief.
    Clear { body: String },
}

/// What the council asks after a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sitting {
    /// Nothing.
    Quiet,
    /// The parent must answer as manager: a hole in the report.
    Warrant(String),
    /// Too many warrants: the report escalates.
    Escalate(String),
    /// Relief acted.
    Relieved(String),
}

fn journal(world: &Path) -> Journal<Row> {
    Journal::new(world.join(COUNCIL))
}

/// `body`'s strain and warrants since it was last cleared.
#[must_use]
pub fn tally(world: &Path, body: &str) -> (u64, u64) {
    let rows = journal(world).read().map(|r| r.rows).unwrap_or_default();
    let mut strain = 0;
    let mut warrants = 0;
    for row in rows {
        match row {
            Row::Strain { body: b, by } if b == body => strain += by,
            Row::Warrant { body: b } if b == body => warrants += 1,
            Row::Clear { body: b } if b == body => (strain, warrants) = (0, 0),
            _ => {}
        }
    }
    (strain, warrants)
}

/// Sit after one of `body`'s runs: `failed` acts, `done` when its report answered `DONE` and its
/// landing held, `counted` false for an interrupted or dry run, which never adds.
#[must_use]
pub fn sit(
    world: &Path,
    keys: Thresholds,
    body: &str,
    failed: u64,
    done: bool,
    counted: bool,
) -> Sitting {
    let j = journal(world);
    let record = |row: &Row| {
        if let Err(e) = j.append(row) {
            eprintln!("mnemorph: the council could not record {body}: {e}");
        }
    };
    if done && failed == 0 {
        if tally(world, body) != (0, 0) {
            record(&Row::Clear {
                body: body.to_owned(),
            });
        }
        return Sitting::Quiet;
    }
    if !counted {
        return Sitting::Quiet;
    }
    let by = failed + u64::from(!done);
    if by > 0 {
        record(&Row::Strain {
            body: body.to_owned(),
            by,
        });
    }
    let (strain, warrants) = tally(world, body);
    if strain >= keys.relieve_at {
        record(&Row::Clear {
            body: body.to_owned(),
        });
        return match crate::world::relief::relieve(world, body, true) {
            Ok(r) => Sitting::Relieved(r.line()),
            Err(e) => Sitting::Escalate(format!(
                "council: {body} strain {strain} reached relief, and relief could not act: {e}"
            )),
        };
    }
    if strain >= keys.escalate_at {
        if warrants + 1 >= keys.max_consults {
            record(&Row::Warrant {
                body: body.to_owned(),
            });
            return Sitting::Escalate(format!(
                "council: {body} strain {strain} after {} warrants — the human decides",
                warrants + 1
            ));
        }
        record(&Row::Warrant {
            body: body.to_owned(),
        });
        return Sitting::Warrant(format!(
            "council: {body} strain {strain} — answer as its manager: @INSTRUCT {body}: <how>, @SPLIT {body}, or @ESCALATE"
        ));
    }
    Sitting::Quiet
}
