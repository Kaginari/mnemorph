//! A session's numbers across its bodies, read from its run journals and its usage ledger alone —
//! `mnemorphd runs --session`: unexplained cache-miss cost, rent on carried context per order,
//! member re-read bytes by cause, and orders by outcome. Read-only; never fed to a model.
//!
//! A journal's model beats and its run's ledger rows are paired in order: the k-th turn with the
//! k-th row of that run that names no `purpose` (a rerank or another call that is no body's
//! turn). Turns or rows left without a partner are counted as `unpaired`, never guessed at.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use mnemorph_ledger::{Journal, UsageRow};
use serde::Serialize;
use serde_json::Value;

use crate::beats::Beat;

/// A call is judged for a cache miss only when the call before it carried at least this many
/// tokens: below it a provider may not cache at all.
pub const MISS_FLOOR: u64 = 8000;

/// A call missed the cache when it read back less than this share of what the call before it
/// left cached (the lesser of the two calls' contexts).
pub const MISS_SHARE: f64 = 0.5;

/// Two reads meet when they read the same file from the same 100-line neighbourhood, by first line.
pub const BUCKET: usize = 100;

/// Dollars per token: input, output, cache read.
pub type Prices = (f64, f64, f64);

/// Member read bytes, by why the read was made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ReadCauses {
    /// The first read of these lines in the session by any member.
    pub new: u64,
    /// Read again within the same order.
    pub same_order: u64,
    /// Read by the same body in an earlier order: what continuity is meant to save.
    pub own_earlier_order: u64,
    /// First read by another member: what a shared map could save.
    pub after_another_body: u64,
}

impl ReadCauses {
    /// All member read bytes.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.new + self.same_order + self.own_earlier_order + self.after_another_body
    }
}

/// One order a member took: one ask in a member's journal.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Order {
    /// The member.
    pub body: String,
    /// Model calls it made.
    pub calls: u64,
    /// Lines it added to files outside `.mnemorph`.
    pub lines: u64,
    /// What it cost.
    pub usd: f64,
    /// How it ended, in words; empty when the journal shows no end.
    pub ending: String,
}

impl Order {
    /// The order's outcome: `landed`, `nothing`, `step cap`, `escalated` or `refused`.
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        if self.ending.starts_with("checkpoint: steps") {
            "step cap"
        } else if self.ending.starts_with("escalate") {
            "escalated"
        } else if self.ending.starts_with("refused") {
            "refused"
        } else if self.lines > 0 {
            "landed"
        } else {
            "nothing"
        }
    }
}

/// A session's numbers.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SessionMetrics {
    /// Model calls paired with a ledger row.
    pub calls: u64,
    /// Their weighted tokens.
    pub wt: u64,
    /// Their dollars: each row's reported cost, else its own price, else priced at `prices`.
    pub usd: f64,
    /// The prices misses and rent are counted at; `None` when the ledger allows none.
    pub prices: Option<Prices>,
    /// Calls that read back under [`MISS_SHARE`] of what the call before left cached.
    pub misses: u64,
    /// Of those, the ones no prefix move, fold or compaction explains.
    pub unexplained: u64,
    /// Weighted tokens the unexplained misses sent again uncached, at wt's 0.9 difference.
    pub lost_wt: u64,
    /// Their dollars: the tokens times input less cache read.
    pub lost_usd: f64,
    /// Rent on carried context: for a member's second order on, the context it inherited, paid
    /// again on each of the order's calls, priced cached first.
    pub rent_usd: f64,
    /// What those later orders cost in all.
    pub resumed_usd: f64,
    /// Member read bytes by cause.
    pub reads: ReadCauses,
    /// The top's own dollars.
    pub top_usd: f64,
    /// The members' orders, in journal order.
    pub orders: Vec<Order>,
    /// Model turns and ledger rows left without a partner (a journal cut short, a lost row).
    pub unpaired: u64,
}

/// The body a run id names: `slime-expr-<16 hex>-<n>` is `slime-expr`; a session's own run is
/// the top's.
fn body_of(run: &str) -> Option<&str> {
    let (rest, n) = run.rsplit_once('-')?;
    let (body, hex) = rest.rsplit_once('-')?;
    (n.bytes().all(|b| b.is_ascii_digit())
        && hex.len() == 16
        && hex.bytes().all(|b| b.is_ascii_hexdigit()))
    .then_some(body)
}

/// When a member's run started, from its id; 0 for the top's, which starts first.
fn started(run: &str) -> u64 {
    run.rsplit_once('-')
        .and_then(|(rest, _)| rest.rsplit_once('-'))
        .and_then(|(_, hex)| u64::from_str_radix(hex, 16).ok())
        .unwrap_or(0)
}

/// The tokens a call carried: everything it sent, cached or not.
fn occupancy(r: &UsageRow) -> u64 {
    r.usage.input + r.usage.cache_read + r.usage.cache_write
}

/// Prices fitted by least squares to the dollars the rows state (reported, else their own),
/// cache writes counted as input; `None` with fewer than three priced rows or a singular fit.
#[must_use]
pub fn fitted(rows: &[UsageRow]) -> Option<Prices> {
    #[allow(clippy::cast_precision_loss)]
    let data: Vec<[f64; 4]> = rows
        .iter()
        .filter_map(|r| {
            let usd = r.reported_cost.or(r.usd)?;
            Some([
                (r.usage.input + r.usage.cache_write) as f64,
                r.usage.output as f64,
                r.usage.cache_read as f64,
                usd,
            ])
        })
        .collect();
    if data.len() < 3 {
        return None;
    }
    let mut m = [[0.0f64; 4]; 3];
    for x in &data {
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += x[i] * x[j];
            }
            m[i][3] += x[i] * x[3];
        }
    }
    for i in 0..3 {
        let p = (i..3).max_by(|a, b| m[*a][i].abs().total_cmp(&m[*b][i].abs()))?;
        m.swap(i, p);
        if m[i][i].abs() < 1e-18 {
            return None;
        }
        for r in 0..3 {
            if r != i {
                let f = m[r][i] / m[i][i];
                let pivot = m[i];
                for (c, v) in m[r].iter_mut().enumerate() {
                    *v -= f * pivot[c];
                }
            }
        }
    }
    Some((m[0][3] / m[0][0], m[1][3] / m[1][1], m[2][3] / m[2][2]))
}

/// One call's dollars: as reported, else as priced by Mnemorph, else at `prices`.
#[allow(clippy::cast_precision_loss)]
fn dollars(r: &UsageRow, prices: Option<Prices>) -> f64 {
    r.reported_cost.or(r.usd).unwrap_or_else(|| {
        prices.map_or(0.0, |(i, o, c)| {
            (r.usage.input + r.usage.cache_write) as f64 * i
                + r.usage.output as f64 * o
                + r.usage.cache_read as f64 * c
        })
    })
}

/// What a session's journals and ledger say: `journals` as (run id, beats), any order.
#[must_use]
#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
pub fn session(mut journals: Vec<(String, Vec<Beat>)>, ledger: &[UsageRow]) -> SessionMetrics {
    let mut m = SessionMetrics {
        prices: fitted(ledger),
        ..SessionMetrics::default()
    };
    let (p_in, _, p_cache) = m.prices.unwrap_or_default();
    let mut rows: HashMap<&str, Vec<&UsageRow>> = HashMap::new();
    for r in ledger.iter().filter(|r| r.purpose.is_none()) {
        rows.entry(r.run.as_str()).or_default().push(r);
    }
    journals.sort_by_key(|(run, _)| started(run));
    // (file, bucket) -> the body that first read it, and per body the order it first read it in.
    let mut first_reader: HashMap<(String, usize), String> = HashMap::new();
    let mut own: HashMap<(String, String, usize), usize> = HashMap::new();
    let mut orders_by_body: HashMap<String, usize> = HashMap::new();
    let mut paired_runs = std::collections::BTreeSet::new();
    // Rounded once, at the end: wt weighs a cache read at 0.1 of an input token.
    let mut lost_tokens = 0u64;
    for (run, beats) in &journals {
        let member = body_of(run).map(str::to_owned);
        let mine = rows.get(run.as_str()).map_or(&[][..], Vec::as_slice);
        paired_runs.insert(run.as_str());
        let mut k = 0usize;
        let mut prev: Option<u64> = None;
        let mut moved = false;
        let mut names: HashMap<String, Value> = HashMap::new();
        let mut order: Option<Order> = None;
        let mut order_no = 0usize;
        let mut in_journal = 0usize;
        let mut inherited = 0u64;
        let mut first_call = false;
        let close = |m: &mut SessionMetrics, o: &mut Option<Order>| {
            if let Some(o) = o.take() {
                m.orders.push(o);
            }
        };
        for beat in beats {
            match beat {
                Beat::Ask { .. } => {
                    close(&mut m, &mut order);
                    in_journal += 1;
                    if let Some(body) = &member {
                        let n = orders_by_body.entry(body.clone()).or_default();
                        *n += 1;
                        order_no = *n;
                        order = Some(Order {
                            body: body.clone(),
                            ..Order::default()
                        });
                        first_call = true;
                    }
                }
                Beat::Prefix { .. } | Beat::Compacted { .. } => moved = true,
                Beat::Warning { text } if text.contains("sent folded") => moved = true,
                Beat::Model { content, .. } => {
                    for b in content {
                        if b["type"] == "tool_use"
                            && let Some(id) = b["id"].as_str()
                        {
                            names.insert(id.to_owned(), b.clone());
                        }
                    }
                    let Some(r) = mine.get(k) else {
                        m.unpaired += 1;
                        moved = false;
                        continue;
                    };
                    k += 1;
                    let occ = occupancy(r);
                    let usd = dollars(r, m.prices);
                    m.calls += 1;
                    m.wt += r.wt;
                    m.usd += usd;
                    match order.as_mut() {
                        Some(o) => {
                            o.calls += 1;
                            o.usd += usd;
                            // A body's second order on: inside one journal, the context the
                            // order began with was carried over from the orders before it.
                            if first_call {
                                first_call = false;
                                inherited = if in_journal > 1 { occ } else { 0 };
                            }
                            if in_journal > 1 {
                                m.resumed_usd += usd;
                                let held = inherited.min(occ);
                                m.rent_usd += held.min(r.usage.cache_read) as f64 * p_cache
                                    + held.saturating_sub(r.usage.cache_read) as f64 * p_in;
                            }
                        }
                        None => m.top_usd += usd,
                    }
                    if let Some(prev) = prev.filter(|p| *p > MISS_FLOOR) {
                        let could = prev.min(occ);
                        if (r.usage.cache_read as f64) < MISS_SHARE * could as f64 {
                            m.misses += 1;
                            if !moved {
                                m.unexplained += 1;
                                let gone = could.saturating_sub(r.usage.cache_read);
                                lost_tokens += gone;
                                m.lost_usd += gone as f64 * (p_in - p_cache);
                            }
                        }
                    }
                    prev = Some(occ);
                    moved = false;
                }
                Beat::Result {
                    tool_use_id,
                    content,
                    facts,
                    ..
                } => {
                    let Some(body) = &member else { continue };
                    let Some(call) = names.get(tool_use_id) else {
                        continue;
                    };
                    if call["name"] != "read" {
                        continue;
                    }
                    let args = &call["input"];
                    let path = facts.read.as_ref().map_or_else(
                        || args["path"].as_str().unwrap_or_default().to_owned(),
                        |s| s.path.clone(),
                    );
                    let path = path.split('#').next().unwrap_or_default().to_owned();
                    if path.is_empty() || path.contains(".mnemorph") {
                        continue;
                    }
                    let first = facts
                        .read
                        .as_ref()
                        .and_then(|s| s.lines.map(|l| l.0))
                        .or_else(|| {
                            args["offset"]
                                .as_u64()
                                .and_then(|o| usize::try_from(o).ok())
                        })
                        .unwrap_or(1);
                    let key = (path, first / BUCKET);
                    let size = facts.bytes.unwrap_or(content.len() as u64);
                    let mine_key = (body.clone(), key.0.clone(), key.1);
                    match own.get(&mine_key) {
                        Some(n) if *n == order_no => m.reads.same_order += size,
                        Some(_) => m.reads.own_earlier_order += size,
                        None if first_reader.contains_key(&key) => {
                            m.reads.after_another_body += size;
                        }
                        None => m.reads.new += size,
                    }
                    own.entry(mine_key).or_insert(order_no);
                    first_reader.entry(key).or_insert_with(|| body.clone());
                }
                Beat::Guts {
                    tool_use_id,
                    wrote,
                    lines_added,
                    ..
                } if *wrote > 0 => {
                    let Some(o) = order.as_mut() else { continue };
                    let path = names
                        .get(tool_use_id)
                        .and_then(|c| c["input"]["path"].as_str())
                        .unwrap_or_default();
                    if !path.contains(".mnemorph") {
                        o.lines += u64::from(*lines_added);
                    }
                }
                Beat::End { ending } => {
                    if let Some(o) = order.as_mut() {
                        ending.clone_into(&mut o.ending);
                    }
                }
                _ => {}
            }
        }
        close(&mut m, &mut order);
        m.unpaired += (mine.len() - k.min(mine.len())) as u64;
    }
    m.lost_wt = lost_tokens * 9 / 10;
    m.unpaired += rows
        .iter()
        .filter(|(run, _)| !paired_runs.contains(*run))
        .map(|(_, r)| r.len() as u64)
        .sum::<u64>();
    m
}

/// The numbers of the session whose instruments are at `dir` — a world, or its
/// `.mnemorph/instruments`.
///
/// # Errors
///
/// No ledger there, or a journal or the ledger cannot be read.
pub fn of_instruments(dir: &Path) -> Result<SessionMetrics, String> {
    let dir = if dir.join("usage.jsonl").is_file() {
        dir.to_path_buf()
    } else {
        dir.join(".mnemorph/instruments")
    };
    let ledger_path = dir.join("usage.jsonl");
    if !ledger_path.is_file() {
        return Err(format!("no usage ledger at {}", ledger_path.display()));
    }
    let ledger = Journal::<UsageRow>::new(ledger_path)
        .read()
        .map_err(|e| format!("the usage ledger cannot be read: {e}"))?
        .rows;
    let mut journals = Vec::new();
    let loop_dir = dir.join("loop");
    for e in std::fs::read_dir(&loop_dir)
        .map_err(|e| format!("no run journals in {}: {e}", loop_dir.display()))?
        .filter_map(Result::ok)
    {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        let run = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let beats = Journal::<Beat>::new(p)
            .read()
            .map_err(|e| format!("run `{run}`: its journal cannot be read: {e}"))?
            .rows;
        journals.push((run, beats));
    }
    Ok(session(journals, &ledger))
}

#[allow(clippy::cast_precision_loss)]
fn pct(part: f64, whole: f64) -> String {
    if whole > 0.0 {
        format!("{:.0}%", 100.0 * part / whole)
    } else {
        "—".to_owned()
    }
}

/// The numbers as the human reads them.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn render(m: &SessionMetrics) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "session: {} calls, {} wt, ${:.3}", m.calls, m.wt, m.usd);
    match m.prices {
        Some((input, output, cached)) => {
            let _ = writeln!(
                s,
                "prices fitted from the ledger, $ per million: input {:.3}, output {:.3}, cache read {:.3}",
                input * 1e6,
                output * 1e6,
                cached * 1e6
            );
        }
        None => {
            let _ = writeln!(
                s,
                "no prices: the ledger states too few dollars to fit them"
            );
        }
    }
    let _ = writeln!(
        s,
        "cache misses: {} calls ({}), {} with no prefix move before them",
        m.misses,
        pct(m.misses as f64, m.calls as f64),
        m.unexplained
    );
    let _ = writeln!(
        s,
        "  lost to the unexplained ones: {} wt ({} of the session), ${:.3} ({})",
        m.lost_wt,
        pct(m.lost_wt as f64, m.wt as f64),
        m.lost_usd,
        pct(m.lost_usd, m.usd)
    );
    let _ = writeln!(
        s,
        "rent on carried context: ${:.3} = {} of what orders after a body's first cost (${:.3}), {} of the session",
        m.rent_usd,
        pct(m.rent_usd, m.resumed_usd),
        m.resumed_usd,
        pct(m.rent_usd, m.usd)
    );
    let reads = m.reads;
    let t = reads.total() as f64;
    let _ = writeln!(
        s,
        "member read bytes {}: new {}, same order {}, own earlier order {}, after another body {}",
        reads.total(),
        pct(reads.new as f64, t),
        pct(reads.same_order as f64, t),
        pct(reads.own_earlier_order as f64, t),
        pct(reads.after_another_body as f64, t)
    );
    let mut by: BTreeMap<&str, (u64, f64)> = BTreeMap::new();
    for o in &m.orders {
        let e = by.entry(o.outcome()).or_default();
        e.0 += 1;
        e.1 += o.usd;
    }
    let _ = write!(
        s,
        "orders below the top: {}; the top's own turns ${:.3}",
        m.orders.len(),
        m.top_usd
    );
    for (outcome, (n, usd)) in &by {
        let _ = write!(s, "; {outcome} {n} (${usd:.3})");
    }
    s.push('\n');
    if m.unpaired > 0 {
        let _ = writeln!(
            s,
            "{} model turns or ledger rows had no partner and are not counted",
            m.unpaired
        );
    }
    s
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss)]
mod tests {
    use super::*;

    fn row(run: &str, input: u64, cache_read: u64, cost: f64) -> UsageRow {
        serde_json::from_value(serde_json::json!({
            "ts": "2026-11-11T08:00:00Z", "session": "s", "run": run, "body": "b", "race": "slime",
            "model": "m", "provider": "openrouter", "input": input, "output": 10,
            "cacheRead": cache_read, "cacheWrite": 0, "prefix": 0, "wt": input + 10,
            "usd": null, "reportedCost": cost
        }))
        .unwrap()
    }

    fn turn() -> Beat {
        Beat::Model {
            content: Vec::new(),
            stop: "end_turn".into(),
            at: None,
            ms: None,
        }
    }

    fn read(id: &str, path: &str, offset: u64, content: &str) -> [Beat; 2] {
        [
            Beat::Model {
                content: vec![
                    serde_json::json!({"type": "tool_use", "id": id, "name": "read",
                    "input": {"path": path, "offset": offset}}),
                ],
                stop: "tool_use".into(),
                at: None,
                ms: None,
            },
            Beat::Result {
                tool_use_id: id.into(),
                content: content.into(),
                is_error: false,
                references: Vec::new(),
                refused: None,
                facts: crate::beats::Facts::default(),
            },
        ]
    }

    #[test]
    fn a_miss_no_prefix_move_explains_is_priced() {
        let beats = vec![
            Beat::Ask { text: "go".into() },
            turn(),
            turn(),
            Beat::Prefix {
                at: 3,
                why: "fold".into(),
                stripped: 0,
            },
            turn(),
            turn(),
        ];
        // Prices: input 1e-7, output 1e-6, cache read 1e-8 per token, stated by each row.
        let r = |run: &str, i: u64, c: u64| {
            row(run, i, c, i as f64 * 1e-7 + 10.0 * 1e-6 + c as f64 * 1e-8)
        };
        let ledger = vec![
            r("r1-1", 20_000, 0),
            r("r1-1", 20_000, 0),
            r("r1-1", 21_000, 0),
            r("r1-1", 1_000, 20_000),
            r("r1-1", 6_000, 0),
        ];
        let m = session(vec![("r1-1".into(), beats)], &ledger);
        // Call 2 missed with nothing said: 20 000 tokens sent again. Call 3 missed after a fold.
        assert_eq!((m.calls, m.misses, m.unexplained), (4, 2, 1));
        assert_eq!(m.lost_wt, 18_000);
        assert!(
            (m.lost_usd - 20_000.0 * 9e-8).abs() < 1e-9,
            "{}",
            m.lost_usd
        );
        // The fifth row has no turn: counted, never paired.
        assert_eq!(m.unpaired, 1);
    }

    #[test]
    fn a_kept_member_pays_rent_on_what_it_carried() {
        let run = "slime-a-0000000000000001-0";
        let beats = vec![
            Beat::Ask { text: "one".into() },
            turn(),
            Beat::Ask { text: "two".into() },
            turn(),
            turn(),
        ];
        let ledger = vec![
            row(run, 9_000, 0, 0.01),
            row(run, 1_000, 9_000, 0.02),
            row(run, 2_000, 9_000, 0.03),
        ];
        let m = session(vec![(run.into(), beats)], &ledger);
        assert_eq!(m.orders.len(), 2);
        assert!((m.resumed_usd - 0.05).abs() < 1e-12);
        // Order two began at 10 000 tokens; both its calls carry them, 9 000 read cached.
        let (i, _, c) = m.prices.unwrap();
        let want = 2.0 * (9_000.0 * c + 1_000.0 * i);
        assert!(
            (m.rent_usd - want).abs() < 1e-12,
            "{} vs {want}",
            m.rent_usd
        );
    }

    #[test]
    fn re_reads_are_split_by_why_across_orders_and_bodies() {
        let a = "slime-a-0000000000000001-0";
        let a2 = "slime-a-0000000000000003-0";
        let b = "slime-b-0000000000000002-0";
        let mut one = vec![Beat::Ask { text: "x".into() }];
        one.extend(read("1", "src/x.rs", 1, "aaaa"));
        one.extend(read("2", "src/x.rs", 50, "bb"));
        let mut two = vec![Beat::Ask { text: "y".into() }];
        two.extend(read("3", "src/x.rs", 1, "ccc"));
        let mut other = vec![Beat::Ask { text: "z".into() }];
        other.extend(read("4", "src/x.rs", 1, "d"));
        let m = session(
            vec![(a.into(), one), (a2.into(), two), (b.into(), other)],
            &[],
        );
        // A cold body's own earlier order is its own, though its journal is new.
        assert_eq!(
            m.reads,
            ReadCauses {
                new: 4,
                same_order: 2,
                own_earlier_order: 3,
                after_another_body: 1
            }
        );
    }

    #[test]
    fn a_run_id_names_its_body() {
        assert_eq!(body_of("slime-expr-18e6f033f4b0656c-0"), Some("slime-expr"));
        assert_eq!(body_of("r1794385558802-3291591"), None);
        assert_eq!(started("slime-expr-0000000000000010-0"), 16);
        assert_eq!(started("r1794385558802-3291591"), 0);
    }

    #[test]
    fn an_outcome_reads_the_ending_before_the_lines() {
        let o = |lines, ending: &str| Order {
            lines,
            ending: ending.into(),
            ..Order::default()
        };
        assert_eq!(o(3, "answered").outcome(), "landed");
        assert_eq!(o(0, "answered").outcome(), "nothing");
        assert_eq!(o(5, "checkpoint: steps 60 of 60").outcome(), "step cap");
        assert_eq!(o(0, "escalated to the Orc").outcome(), "escalated");
    }
}
