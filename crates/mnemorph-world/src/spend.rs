//! The usage ledger read back for the human: what a window of it spent, in total and by session,
//! day, body, race and model; its cache hit rate; its burn per five minutes; its unpriced calls —
//! as `mnemorphd usage` prints it and the board's metrics draw it, from this one fold.
//!
//! Dollars cover the priced calls only: a window with no priced call reads no dollars at all,
//! never a zero, and every total says how many calls its dollars do not cover. The ledger read is
//! its witness under the human's state directory (`$XDG_STATE_HOME/mnemorph/usage/<world>-<fnv64a>.jsonl`,
//! every byte the harness appended, written there first), its chain walked: trusted while the
//! chain holds. A world with no witness yet is read from its own copy inside the world, where a
//! body can write it, and marked untrusted. Either is read as [`crate::regular`] reads a file, its
//! last [`READ_CAP`] bytes at most.

use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};

use mnemorph_ledger::{Key, Timestamp, Totals, UsageRow, fold as fold_by, parse_rows};

use crate::regular;
use crate::worth::USAGE;

/// The most bytes of the ledger read: its newest part, when it is longer.
pub const READ_CAP: u64 = 64 << 20;

/// The burn's slot: five minutes.
pub const BURN_SLOT: i64 = 300;

/// How many slots the burn holds: the last twelve hours.
pub const BURN_SLOTS: usize = 144;

/// The span of the ledger a reading folds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// The last seven days.
    Week,
    /// The last twenty-eight days.
    Month,
    /// Everything the ledger holds.
    All,
}

impl Window {
    /// Every window, shortest first.
    pub const ALL: [Window; 3] = [Window::Week, Window::Month, Window::All];

    /// The window `word` names: `7d`, `28d` or `all`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Window> {
        match word {
            "7d" => Some(Window::Week),
            "28d" => Some(Window::Month),
            "all" => Some(Window::All),
            _ => None,
        }
    }

    /// Its word: `7d`, `28d` or `all`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Window::Week => "7d",
            Window::Month => "28d",
            Window::All => "all",
        }
    }

    /// The first moment it holds, at `now`; `None` for everything.
    #[must_use]
    pub fn since(self, now: Timestamp) -> Option<Timestamp> {
        let days = match self {
            Window::Week => 7,
            Window::Month => 28,
            Window::All => return None,
        };
        Some(Timestamp(now.0.saturating_sub(days * 86_400)))
    }
}

/// The dollars of `t`: `None` when none of its calls was priced — never a made-up zero.
#[must_use]
pub fn usd(t: &Totals) -> Option<f64> {
    (t.calls > t.unpriced).then_some(t.usd)
}

/// The share of `t`'s input served from the cache; `None` with no input.
#[must_use]
pub fn cache_hit(t: &Totals) -> Option<f64> {
    let all = t.usage.input + t.usage.cache_read + t.usage.cache_write;
    (all > 0).then(|| t.cache_hit_rate())
}

/// One session's spend in a window.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// The session.
    pub id: String,
    /// Its first call in the window.
    pub first: Timestamp,
    /// Its last.
    pub last: Timestamp,
    /// Its totals.
    pub totals: Totals,
}

/// What a window of the ledger spent.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// The window.
    pub window: Window,
    /// When it was read: the window ends here.
    pub now: Timestamp,
    /// Everything in it.
    pub total: Totals,
    /// By session, the latest first.
    pub sessions: Vec<Session>,
    /// By UTC day.
    pub days: BTreeMap<String, Totals>,
    /// By UTC day, then race.
    pub day_races: BTreeMap<String, BTreeMap<String, Totals>>,
    /// By body.
    pub bodies: BTreeMap<String, Totals>,
    /// By race.
    pub races: BTreeMap<String, Totals>,
    /// By model.
    pub models: BTreeMap<String, Totals>,
    /// Weighted tokens per [`BURN_SLOT`], oldest first, the last slot ending at `now`: the last
    /// [`BURN_SLOTS`] whatever the window.
    pub burn: Vec<u64>,
    /// Each body's first call the ledger holds, whatever the window: what a silent meter is
    /// silent since.
    pub first_call: BTreeMap<String, Timestamp>,
}

impl Reading {
    /// Weighted tokens per minute over the last [`BURN_SLOT`].
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn burn_per_minute(&self) -> f64 {
        self.burn
            .last()
            .map_or(0.0, |&wt| wt as f64 / (BURN_SLOT as f64 / 60.0))
    }
}

/// `rows` folded over `window` at `now`.
#[must_use]
pub fn fold(rows: &[UsageRow], window: Window, now: Timestamp) -> Reading {
    let since = window.since(now);
    let held: Vec<UsageRow> = rows
        .iter()
        .filter(|r| since.is_none_or(|s| r.ts >= s) && r.ts <= now)
        .cloned()
        .collect();
    let total = held.iter().fold(Totals::default(), |mut t, r| {
        add(&mut t, r);
        t
    });
    let mut spans: BTreeMap<&str, (Timestamp, Timestamp)> = BTreeMap::new();
    for r in &held {
        spans
            .entry(r.session.as_str())
            .and_modify(|(a, b)| (*a, *b) = ((*a).min(r.ts), (*b).max(r.ts)))
            .or_insert((r.ts, r.ts));
    }
    let mut sessions: Vec<Session> = fold_by(&held, Key::Session)
        .into_iter()
        .map(|(id, totals)| {
            let (first, last) = spans
                .get(id.as_str())
                .copied()
                .unwrap_or((Timestamp(0), Timestamp(0)));
            Session {
                id,
                first,
                last,
                totals,
            }
        })
        .collect();
    sessions.sort_by(|a, b| b.last.cmp(&a.last).then(a.id.cmp(&b.id)));
    let mut day_races: BTreeMap<String, BTreeMap<String, Totals>> = BTreeMap::new();
    for (day, of) in group(&held, |r| r.ts.day().to_string()) {
        day_races.insert(day, fold_by(&of, Key::Race));
    }
    let mut burn = vec![0u64; BURN_SLOTS];
    let span = BURN_SLOT * i64::try_from(BURN_SLOTS).unwrap_or(i64::MAX);
    for r in rows {
        // Slot 0 is the oldest; the last ends at `now`, inclusive.
        let back = now.0 - r.ts.0;
        if (0..span).contains(&back) {
            let slot = usize::try_from(back / BURN_SLOT).unwrap_or(BURN_SLOTS);
            if let Some(b) = burn.get_mut(BURN_SLOTS - 1 - slot) {
                *b += r.wt;
            }
        }
    }
    let mut first_call: BTreeMap<String, Timestamp> = BTreeMap::new();
    for r in rows {
        first_call
            .entry(r.body.clone())
            .and_modify(|t| *t = (*t).min(r.ts))
            .or_insert(r.ts);
    }
    Reading {
        window,
        now,
        total,
        sessions,
        days: fold_by(&held, Key::Day),
        day_races,
        bodies: fold_by(&held, Key::Body),
        races: fold_by(&held, Key::Race),
        models: fold_by(&held, Key::Model),
        burn,
        first_call,
    }
}

/// `t` with the call `r` added, as the ledger's fold adds one.
fn add(t: &mut Totals, r: &UsageRow) {
    t.calls += 1;
    t.usage = t.usage + r.usage;
    t.wt += r.wt;
    t.prefix += r.prefix;
    match r.usd {
        Some(usd) => t.usd += usd,
        None => t.unpriced += 1,
    }
}

/// `rows` grouped by `key`, in key order.
fn group(rows: &[UsageRow], key: impl Fn(&UsageRow) -> String) -> BTreeMap<String, Vec<UsageRow>> {
    let mut out: BTreeMap<String, Vec<UsageRow>> = BTreeMap::new();
    for r in rows {
        out.entry(key(r)).or_default().push(r.clone());
    }
    out
}

/// The ledger's rows as read: what parsed, the lines that did not, and whether an older part was
/// left unread past [`READ_CAP`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ledger {
    /// The rows that parsed, in order.
    pub rows: Vec<UsageRow>,
    /// How many lines did not parse: not counted.
    pub skipped: usize,
    /// Whether the ledger is longer than [`READ_CAP`] and its oldest part was left unread.
    pub cut: bool,
    /// The file read.
    pub path: PathBuf,
    /// Whether it is the harness's witness under the state directory, its chain holding.
    pub trusted: bool,
    /// Why it is not trusted: no witness (the world's own copy was read), or the witness's chain
    /// does not hold.
    pub why: Option<String>,
}

/// Where `world`'s usage witness lives, when there is a state directory: the file the harness
/// writes every ledger byte to first.
#[must_use]
pub fn witness_of(world: &Path) -> Option<PathBuf> {
    crate::queue::state_dir().map(|s| crate::queue::state_file_in(&s, world, "usage"))
}

/// `world`'s usage ledger, its last [`READ_CAP`] bytes at most; an empty one when it is absent.
///
/// # Errors
///
/// It is not a regular file (a link, a pipe, a device), or it cannot be read.
pub fn read(world: &Path) -> Result<Ledger, String> {
    read_from(world, witness_of(world).as_deref())
}

/// [`read`], the witness at `witness`: it, its chain walked, when it is there; else the world's
/// own copy, untrusted.
///
/// # Errors
///
/// As [`read`].
pub fn read_from(world: &Path, witness: Option<&Path>) -> Result<Ledger, String> {
    if let Some(w) = witness.filter(|w| std::fs::symlink_metadata(w).is_ok()) {
        let mut l = read_at(w, READ_CAP)?;
        match mnemorph_ledger::chain::verify(w) {
            Ok(r) if r.holds() => l.trusted = true,
            Ok(r) => l.why = Some(format!("the witness's chain does not hold: {r}")),
            Err(e) => l.why = Some(format!("the witness's chain cannot be walked: {e}")),
        }
        return Ok(l);
    }
    let mut l = read_at(&world.join(USAGE), READ_CAP)?;
    l.why = Some(
        "no witness under the state directory: read from the world's own copy, which a body can \
         write"
            .to_owned(),
    );
    Ok(l)
}

/// [`read`] of the file at `path`, its last `cap` bytes at most.
///
/// # Errors
///
/// As [`read`].
pub fn read_at(path: &Path, cap: u64) -> Result<Ledger, String> {
    let Some(mut file) = regular::open(path).map_err(|e| e.to_string())? else {
        return Ok(Ledger {
            path: path.to_path_buf(),
            ..Ledger::default()
        });
    };
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    let from = len.saturating_sub(cap);
    file.seek(SeekFrom::Start(from))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(cap)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    // Cut mid-line: the first, partial line is not a row.
    let start = if from > 0 {
        bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |i| i + 1)
    } else {
        0
    };
    let back = parse_rows(&bytes[start..]).map_err(|e| e.to_string())?;
    Ok(Ledger {
        rows: back.rows,
        skipped: back.skipped.len(),
        cut: from > 0,
        path: path.to_path_buf(),
        trusted: false,
        why: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemorph_ledger::Usage;

    fn row(ts: i64, session: &str, race: &str, wt: u64, usd: Option<f64>) -> UsageRow {
        UsageRow {
            ts: Timestamp(ts),
            session: session.into(),
            run: "r".into(),
            body: format!("{race}-a"),
            race: race.into(),
            commission: None,
            model: "m".into(),
            provider: "p".into(),
            usage: Usage {
                input: 100,
                output: 10,
                cache_read: 300,
                cache_write: 0,
            },
            prefix: 0,
            wt,
            usd,
            envelope: None,
            purpose: None,
        }
    }

    const NOW: i64 = 1_792_782_420;

    #[test]
    fn a_window_holds_its_days_and_unpriced_is_never_zero() {
        let rows = [
            row(NOW - 30 * 86_400, "old", "slime", 1000, Some(1.0)),
            row(NOW - 86_400, "s1", "slime", 100, Some(0.10)),
            row(NOW - 60, "s2", "orc", 200, None),
            row(NOW - 30, "s2", "orc", 50, None),
        ];
        let week = fold(&rows, Window::Week, Timestamp(NOW));
        assert_eq!(week.total.calls, 3);
        assert_eq!(week.total.wt, 350);
        assert_eq!(week.total.unpriced, 2);
        assert_eq!(usd(&week.total), Some(0.10));
        assert_eq!(usd(&week.races["orc"]), None, "unpriced reads no dollars");
        assert_eq!(week.sessions[0].id, "s2", "the latest first");
        assert_eq!(week.sessions[0].totals.calls, 2);
        assert_eq!(cache_hit(&week.total), Some(0.75));
        let all = fold(&rows, Window::All, Timestamp(NOW));
        assert_eq!(all.total.calls, 4);
        assert_eq!(all.first_call["slime-a"], Timestamp(NOW - 30 * 86_400));
        assert_eq!(week.first_call, all.first_call, "whatever the window");
        assert_eq!(week.day_races.values().map(BTreeMap::len).sum::<usize>(), 2);
    }

    #[test]
    fn the_burn_ends_now_in_five_minute_slots() {
        let rows = [
            row(NOW, "s", "orc", 600, None),
            row(NOW - 299, "s", "orc", 400, None),
            row(NOW - 300, "s", "orc", 7, None),
            row(NOW - 12 * 3600, "s", "orc", 9, None),
            row(NOW + 10, "s", "orc", 5, None),
        ];
        let r = fold(&rows, Window::All, Timestamp(NOW));
        assert_eq!(r.burn.len(), BURN_SLOTS);
        assert_eq!(r.burn[BURN_SLOTS - 1], 1000);
        assert_eq!(r.burn[BURN_SLOTS - 2], 7);
        assert_eq!(
            r.burn.iter().sum::<u64>(),
            1007,
            "outside twelve hours, or ahead"
        );
        assert!((r.burn_per_minute() - 200.0).abs() < 1e-9);
        assert_eq!(r.total.calls, 4, "a row ahead of now is not in the window");
    }

    #[test]
    fn nothing_spent_reads_no_dollars_and_no_hit_rate() {
        let r = fold(&[], Window::Week, Timestamp(NOW));
        assert_eq!(usd(&r.total), None);
        assert_eq!(cache_hit(&r.total), None);
        assert_eq!(r.sessions, Vec::new());
    }

    #[test]
    fn the_windows_read_their_words() {
        for w in Window::ALL {
            assert_eq!(Window::parse(w.word()), Some(w));
        }
        assert_eq!(Window::parse("1y"), None);
    }

    #[test]
    fn a_long_ledger_is_read_from_its_newest_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.jsonl");
        let mut text = String::new();
        for i in 0..20 {
            text.push_str(&serde_json::to_string(&row(NOW + i, "s", "orc", 1, None)).unwrap());
            text.push('\n');
        }
        std::fs::write(&path, &text).unwrap();
        let whole = read_at(&path, 1 << 20).unwrap();
        assert_eq!((whole.rows.len(), whole.cut), (20, false));
        let tail = read_at(&path, 1000).unwrap();
        assert!(tail.cut);
        assert!(tail.rows.len() < 20 && !tail.rows.is_empty());
        assert_eq!(tail.skipped, 0, "the partial first line is not a row");
        let none = read_at(&dir.path().join("none"), 10).unwrap();
        assert!(none.rows.is_empty() && !none.cut && !none.trusted);
    }

    #[test]
    fn the_witness_is_read_and_trusted_while_its_chain_holds() {
        use mnemorph_ledger::Journal;
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w");
        std::fs::create_dir_all(world.join(".mnemorph/instruments")).unwrap();
        let witness = dir.path().join("state/usage.jsonl");
        std::fs::create_dir_all(witness.parent().unwrap()).unwrap();
        let journal = Journal::<UsageRow>::chained(world.join(USAGE)).witnessed_at(&witness);
        for wt in [10, 20] {
            journal.append(&row(NOW, "s", "orc", wt, None)).unwrap();
        }
        // A body inflates the world's copy: the witness is what is read.
        let mut copy = std::fs::read_to_string(world.join(USAGE)).unwrap();
        copy.push_str(&serde_json::to_string(&row(NOW, "s", "orc", 9_999, None)).unwrap());
        copy.push('\n');
        std::fs::write(world.join(USAGE), copy).unwrap();
        let l = read_from(&world, Some(&witness)).unwrap();
        assert!(l.trusted, "{:?}", l.why);
        assert_eq!(l.path, witness);
        assert_eq!(l.rows.iter().map(|r| r.wt).sum::<u64>(), 30);

        // An edited witness breaks its chain: read, but not trusted.
        let text = std::fs::read_to_string(&witness)
            .unwrap()
            .replace("\"wt\":10", "\"wt\":11");
        std::fs::write(&witness, text).unwrap();
        let broken = read_from(&world, Some(&witness)).unwrap();
        assert!(!broken.trusted);
        assert!(broken.why.unwrap().contains("does not hold"));

        // No witness: the world's copy, untrusted, and why.
        let old = read_from(&world, Some(&dir.path().join("absent.jsonl"))).unwrap();
        assert!(!old.trusted);
        assert_eq!(old.path, world.join(USAGE));
        // The body's unchained row after the genesis is skipped there too, and said.
        assert_eq!((old.rows.len(), old.skipped), (2, 1));
        assert!(old.why.unwrap().contains("no witness"));
    }
}
