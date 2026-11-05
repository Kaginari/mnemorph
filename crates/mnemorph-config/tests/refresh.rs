//! The standing reads on a schedule: each runs only when due, a failure is said and not retried
//! every tick, and a body's future date can never put a read off.

use std::cell::Cell;
use std::fs;

use mnemorph_config::refresh::{Every, Jobs, STAMPS, tick};

const HOUR: i64 = 3600;
const EVERY: Every = Every {
    ci_minutes: 15,
    scout_hours: 24,
};

struct Counts {
    ci: Cell<u32>,
    scout: Cell<u32>,
    audit: Cell<u32>,
}

fn counts() -> Counts {
    Counts {
        ci: Cell::new(0),
        scout: Cell::new(0),
        audit: Cell::new(0),
    }
}

fn run(w: &std::path::Path, every: Every, now: i64, c: &Counts, scout_ok: bool) -> Vec<String> {
    let bump = |n: &Cell<u32>| n.set(n.get() + 1);
    tick(
        w,
        every,
        now,
        &Jobs {
            ci: &|| {
                bump(&c.ci);
                Ok("read".into())
            },
            scout: &|| {
                bump(&c.scout);
                if scout_ok {
                    Ok("mirrored".into())
                } else {
                    Err("no network".into())
                }
            },
            audit: &|| {
                bump(&c.audit);
                Ok("audited".into())
            },
        },
    )
}

#[test]
fn each_read_runs_only_when_it_is_due() {
    let dir = tempfile::tempdir().unwrap();
    let (w, c) = (dir.path(), counts());
    let t = 1_000_000 * HOUR;
    let said = run(w, EVERY, t, &c, true);
    assert_eq!(said, vec!["ci: read", "scout: mirrored", "audit: audited"]);
    // A tick soon after does nothing: the 220 MB mirror is never fetched on every tick.
    assert_eq!(run(w, EVERY, t + 60, &c, true), Vec::<String>::new());
    // CI is due again first, the mirror a day on.
    assert_eq!(run(w, EVERY, t + 15 * 60, &c, true), vec!["ci: read"]);
    assert_eq!((c.ci.get(), c.scout.get()), (2, 1));
    run(w, EVERY, t + 24 * HOUR, &c, true);
    assert_eq!((c.scout.get(), c.audit.get()), (2, 2));
}

#[test]
fn zero_turns_a_read_off() {
    let dir = tempfile::tempdir().unwrap();
    let c = counts();
    let off = Every {
        ci_minutes: 0,
        scout_hours: 0,
    };
    assert_eq!(
        run(dir.path(), off, 1_000_000, &c, true),
        Vec::<String>::new()
    );
    assert_eq!(c.ci.get() + c.scout.get(), 0);
}

#[test]
fn a_failed_mirror_says_why_audits_nothing_and_waits_its_turn() {
    let dir = tempfile::tempdir().unwrap();
    let (w, c) = (dir.path(), counts());
    let said = run(w, EVERY, 1_000_000, &c, false);
    assert!(
        said.contains(&"scout: not refreshed — no network".to_owned()),
        "{said:?}"
    );
    assert_eq!(
        c.audit.get(),
        0,
        "an audit over a mirror that failed says nothing new"
    );
    run(w, EVERY, 1_000_060, &c, false);
    assert_eq!(c.scout.get(), 1, "not retried every tick");
}

#[test]
fn a_date_in_the_future_never_puts_a_read_off() {
    let dir = tempfile::tempdir().unwrap();
    let (w, c) = (dir.path(), counts());
    fs::create_dir_all(w.join(".mnemorph/instruments")).unwrap();
    fs::write(w.join(STAMPS), r#"{"ci":99999999999,"scout":99999999999}"#).unwrap();
    run(w, EVERY, 1_000_000, &c, true);
    assert_eq!((c.ci.get(), c.scout.get()), (1, 1));
    // A file that does not read is no stamp at all.
    fs::write(w.join(STAMPS), "not json").unwrap();
    run(w, EVERY, 1_000_001, &c, true);
    assert_eq!(c.ci.get(), 2);
}
