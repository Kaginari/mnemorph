use super::*;
use mnemorph_ledger::reports::{Gate, RunReport};

use crate::creatures::{Ground, row};

/// No proposal.
const NONE: [Proposal; 0] = [];

const HOUR: i64 = 3600;
const DAY: i64 = 86_400;
/// A time well past the epoch, so a window never reaches below it.
const T0: i64 = 1_800_000_000;

/// One run of `creature` as `race`, begun at `ts`: passed on the first try, or failed; `holes`
/// *tests intact* failures.
fn run(creature: &str, race: &str, ts: i64, pass: bool, holes: u32) -> ReportRow {
    ReportRow::Run(RunReport {
        creature: creature.into(),
        race: race.into(),
        run: format!("{creature}-{ts}"),
        ts: Timestamp(ts),
        build: Build::default(),
        gate: Some(if pass { Gate::Pass } else { Gate::Fail }),
        first_try: pass,
        holes,
        verify_green: Some(pass),
        dispatches: 0,
        landed: 0,
        reasks: 0,
        passes: 0,
        overturned: 0,
    })
}

/// `n` runs of `creature` from `from`, an hour apart, the `k`th passed when `pass(k)`.
fn runs(
    creature: &str,
    race: &str,
    from: i64,
    n: i64,
    pass: impl Fn(i64) -> bool,
) -> Vec<ReportRow> {
    (0..n)
        .map(|k| run(creature, race, from + k * HOUR, pass(k), 0))
        .collect()
}

/// The witness's adoption of `name` as `race`.
fn adopted(name: &str, race: Race) -> Row {
    let mut r = row(
        Act::Adopt,
        name,
        race,
        Some("rimuru"),
        Ground {
            zone: vec!["src/".into()],
            ..Ground::default()
        },
        false,
        HUMAN,
    );
    r.ts = Timestamp(T0 - DAY);
    r
}

/// The human's apply of `p`, written after it was read.
fn applied(p: &Proposal, roster: &[Row]) -> Row {
    let mut r = row_for(p, roster).unwrap();
    r.ts = Timestamp(p.as_of.0 + 60);
    r
}

#[test]
fn twenty_settled_passes_in_the_window_propose_a_rise_and_the_newest_waits() {
    let roster = vec![adopted("sly", Race::Slime)];
    // Twenty runs: the newest is not settled yet, so nineteen are read — no rise.
    let rows = runs("sly", "slime", T0, 20, |_| true);
    assert_eq!(propose(&rows, &roster).proposals, NONE);
    let rows = runs("sly", "slime", T0, 21, |_| true);
    let s = propose(&rows, &roster);
    let p = s.of("sly").expect("a rise");
    assert_eq!(
        (p.act, p.from, p.to, p.n),
        (Move::Rise, Race::Slime, Race::NamedSlime, 20)
    );
    assert_eq!(p.as_of, Timestamp(T0 + 20 * HOUR));
    assert_eq!(p.id, p.inputs_sha256[..12]);
    assert_eq!(p.meters[0], ("landRate", Rate::of(20, 20)));
    assert!(p.why.iter().all(Explanation::holds), "{:?}", p.why);
    let shown = p.lines().join("\n");
    assert!(
        shown.contains("landRate 1.00 (20/20, 95% 0.84–1.00)")
            && shown.contains("read over 20 reports")
            && shown.contains("rise: landRate 1.00 >= 0.90"),
        "{shown}"
    );
    assert_eq!(
        p.command(Path::new("/w/my world")),
        "mnemorphd roster rise sly --world '/w/my world'"
    );
    let lines = lines_of(&s, Path::new("/w"));
    assert!(
        lines[0].starts_with("sly: rise to named-slime proposed (landRate 1.00")
            && lines[0]
                .ends_with("the human applies it with: mnemorphd roster rise sly --world /w"),
        "{lines:?}"
    );
}

#[test]
fn the_window_is_twenty_eight_days_of_trusted_time() {
    let roster = vec![adopted("sly", Race::Slime)];
    let mut rows = runs("sly", "slime", T0, 21, |_| true);
    // A month on, one more run and the one that settles it: the old record is out of the window.
    rows.push(run("sly", "slime", T0 + 29 * DAY, true, 0));
    rows.push(run("sly", "slime", T0 + 29 * DAY + HOUR, true, 0));
    assert_eq!(propose(&rows, &roster).proposals, NONE);
    // Read at a time inside the window, it rises still: the window is the clock's, not the wall's.
    let p = propose_at(&rows, &roster, Timestamp(T0 + 20 * HOUR));
    assert_eq!(p.proposals.len(), 1);
    // Twenty settled runs, the first exactly 28 days before the clock: outside, so nineteen.
    let twenty = runs("sly", "slime", T0, 20, |_| true);
    let mut out = twenty.clone();
    out.push(run("sly", "slime", T0 + 28 * DAY, true, 0));
    assert_eq!(propose(&out, &roster).proposals, NONE);
    // A second later than 28 days, inside: twenty.
    let mut inside = twenty;
    inside.push(run("sly", "slime", T0 + 28 * DAY - 1, true, 0));
    assert_eq!(propose(&inside, &roster).proposals.len(), 1);
}

#[test]
fn the_race_keying_reads_the_witness_never_the_card_or_a_stranger() {
    // No row: a stranger proposes nothing, whatever its record.
    let rows = runs("sly", "slime", T0, 25, |_| true);
    assert_eq!(propose(&rows, &[]).proposals, NONE);
    // A row holding it an Elf: its runs as a Slime claim no record of an Elf.
    let s = propose(&rows, &[adopted("sly", Race::Elf)]);
    assert!(s.proposals.is_empty(), "{s:?}");
    // Runs recorded as an ascended form count for the base race the row holds.
    let rows = runs("sly", "named-slime", T0, 21, |_| true);
    assert_eq!(
        propose(&rows, &[adopted("sly", Race::Slime)])
            .proposals
            .len(),
        1
    );
    // A retired row proposes nothing.
    let mut gone = adopted("sly", Race::Slime);
    gone.retired = true;
    assert_eq!(propose(&rows, &[gone]).proposals, NONE);
}

#[test]
fn an_input_no_journal_carries_proposes_nothing() {
    let rows = runs("gor", "orc", T0, 25, |_| true);
    let s = propose(&rows, &[adopted("gor", Race::Orc)]);
    assert_eq!(s.proposals, NONE);
    assert_eq!(s.unknown, vec![("gor".to_owned(), "slimeHealth")]);
    assert_eq!(
        propose(&[], &[adopted("gor", Race::Orc)]),
        Survey::default()
    );
}

/// A Named Slime risen at `T0 + 20h`, and its rows to then.
fn risen() -> (Vec<Row>, Vec<ReportRow>) {
    let mut roster = vec![adopted("sly", Race::Slime)];
    let rows = runs("sly", "slime", T0, 21, |_| true);
    let p = propose(&rows, &roster).of("sly").cloned().unwrap();
    let r = applied(&p, &roster);
    roster.push(r);
    (roster, rows)
}

#[test]
fn under_the_fall_bar_is_probation_and_unrecovered_after_five_is_a_fall() {
    let (roster, mut rows) = risen();
    // Risen: it holds, and is proposed nothing more.
    assert_eq!(propose(&rows, &roster), Survey::default());
    let after = T0 + 30 * HOUR;
    // Ten runs since the rise, four failed: landRate 0.60 < 0.75 — probation; one more settles.
    rows.extend(runs("sly", "slime", after, 11, |k| k >= 4));
    let s = propose(&rows, &roster);
    assert!(s.proposals.is_empty(), "{:?}", s.proposals);
    let pr = s.probation_of("sly").expect("on probation");
    assert_eq!((pr.race, pr.reports), (Race::NamedSlime, 0));
    assert_eq!(pr.began, Timestamp(after + 9 * HOUR));
    assert!(pr.to_string().contains("0 of 5 reports"), "{pr}");
    // Four on probation, three failed: still on it.
    rows.extend(runs("sly", "slime", after + 11 * HOUR, 4, |k| k >= 3));
    let s = propose(&rows, &roster);
    assert_eq!(s.probation_of("sly").map(|p| p.reports), Some(4));
    assert!(lines_of(&s, Path::new("/w"))[0].contains("on probation"));
    // The fifth settled: 0.40 < 0.75, unrecovered — the fall is proposed.
    rows.push(run("sly", "slime", after + 15 * HOUR, true, 0));
    let s = propose(&rows, &roster);
    let p = s.of("sly").expect("a fall").clone();
    assert_eq!(
        (p.act, p.from, p.to),
        (Move::Fall, Race::NamedSlime, Race::Slime)
    );
    assert!(p.ended_probation && p.what().contains("probation ended"));
    assert_eq!(
        p.as_of,
        Timestamp(after + 15 * HOUR),
        "read when the fifth was settled"
    );
    let line = p.why[0].to_string();
    assert!(
        line.starts_with(
            "probation: landRate 0.40 < 0.75 (last 5 reports); recover: >= 0.75 within 5 reports"
        ),
        "{line}"
    );
    // The fall stays the same proposal as more reports arrive.
    rows.extend(runs("sly", "slime", after + 16 * HOUR, 3, |_| true));
    assert_eq!(propose(&rows, &roster).of("sly").unwrap().id, p.id);
}

#[test]
fn a_probation_recovered_at_the_fall_bar_holds_the_form() {
    let (roster, mut rows) = risen();
    let after = T0 + 30 * HOUR;
    rows.extend(runs("sly", "slime", after, 11, |k| k >= 4));
    // Five on probation, four passed: 0.80 >= 0.75 — it recovers.
    rows.extend(runs("sly", "slime", after + 11 * HOUR, 6, |k| k != 0));
    let s = propose(&rows, &roster);
    assert!(s.proposals.is_empty() && s.probation.is_empty(), "{s:?}");
}

#[test]
fn an_event_falls_the_form_at_once() {
    let (roster, mut rows) = risen();
    let after = T0 + 30 * HOUR;
    rows.push(run("sly", "slime", after, true, 1));
    rows.push(run("sly", "slime", after + HOUR, true, 0));
    let p = propose(&rows, &roster).of("sly").cloned().unwrap();
    assert!(!p.ended_probation);
    assert!(
        p.why[0]
            .to_string()
            .starts_with("fall: testsIntactFailures 1 > 0"),
        "{}",
        p.why[0]
    );
}

#[test]
fn a_proposal_is_read_from_the_rows_alone_and_a_replay_reproduces_its_row() {
    let (roster, rows) = risen();
    let again = replay(&rows, &roster).unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(
        (again[0].line, again[0].act, again[0].to.as_str()),
        (2, Act::Rise, "named-slime")
    );
    let p = propose_at(&rows, &roster[..1], roster[1].as_of.unwrap());
    assert_eq!(p.of("sly").unwrap().id, roster[1].proposal.clone().unwrap());
    // A report rewritten after the apply: the row no longer reproduces, and says where.
    let mut moved = rows.clone();
    moved[3] = run("sly", "slime", T0 + 3 * HOUR, false, 0);
    let e = replay(&moved, &roster).unwrap_err();
    assert!(e.starts_with("row 2:"), "{e}");
    // Reports gone: the law proposes no rise there.
    let e = replay(&rows[..5], &roster).unwrap_err();
    assert!(e.contains("proposes no rise"), "{e}");
    // A row whose digest was edited does not either.
    let mut forged = roster.clone();
    forged[1].inputs_sha256 = Some("0".repeat(64));
    assert!(replay(&rows, &forged).is_err());
    // A rise with no time it was read at is no replayable row.
    let mut bare = roster;
    bare[1].as_of = None;
    assert!(replay(&rows, &bare).unwrap_err().contains("names no time"));
}

#[test]
fn a_word_is_quoted_for_a_shell_only_when_it_needs_it() {
    assert_eq!(quoted("slime-s"), "slime-s");
    assert_eq!(quoted("a b"), "'a b'");
    assert_eq!(quoted("it's"), r"'it'\''s'");
    assert_eq!(quoted("$(x)"), "'$(x)'");
    assert_eq!(quoted(""), "''");
}
