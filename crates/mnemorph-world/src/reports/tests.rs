use super::*;
use mnemorph_ledger::{Journal, Usage};
use mnemorph_races::Rate;
use serde_json::json;

/// How the gate names a line the human pinned.
const HUMAN_PIN: &str = "the human's pin";

/// The build the fixture's verdicts are stamped with.
fn stamp() -> Build {
    Build {
        version: "9.9.9".into(),
        commit: "gate000001".into(),
    }
}

/// A world and a state directory beside it, never the human's.
struct Fixture {
    _dir: tempfile::TempDir,
    world: PathBuf,
    state: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("world");
        let state = dir.path().join("state");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        let world = world.canonicalize().unwrap();
        Fixture {
            _dir: dir,
            world,
            state,
        }
    }

    fn at(&self, kind: &str) -> PathBuf {
        state_file_in(&self.state, &self.world, kind)
    }

    /// One call of `run` by `body`, a `race`, commissioned by `parent`, at `ts`.
    fn call(&self, run: &str, body: &str, race: &str, parent: Option<&str>, ts: i64) {
        Journal::<UsageRow>::chained(self.at("usage"))
            .append(&UsageRow {
                ts: Timestamp(ts),
                session: "s".into(),
                run: run.into(),
                body: body.into(),
                race: race.into(),
                commission: parent.map(Into::into),
                model: "m".into(),
                provider: "p".into(),
                usage: Usage::default(),
                prefix: 0,
                wt: 0,
                usd: None,
                reported_cost: None,
                estimated: false,
                envelope: None,
                purpose: None,
            })
            .unwrap();
    }

    /// A verdict on `body`'s landing in `run` (none: a row from before runs were named).
    fn verdict(&self, run: Option<&str>, body: &str, word: &str, ts: i64, changed: &[&str]) {
        self.verdict_with(run, body, word, ts, changed, &[], &[]);
    }

    #[allow(clippy::too_many_arguments)]
    fn verdict_with(
        &self,
        run: Option<&str>,
        body: &str,
        word: &str,
        ts: i64,
        changed: &[&str],
        failures: &[&str],
        unverified: &[&str],
    ) {
        // A verdict whose lines ran on the human's pins, as the gate writes it.
        let by: &[&str] = if word.contains(", unchecked") {
            &[]
        } else {
            &[HUMAN_PIN]
        };
        self.verdict_by(run, body, word, ts, changed, failures, unverified, by);
    }

    /// A verdict whose lines ran on the pins `by` (`the human's pin`, `<orc>'s pin`).
    #[allow(clippy::too_many_arguments)]
    fn verdict_by(
        &self,
        run: Option<&str>,
        body: &str,
        word: &str,
        ts: i64,
        changed: &[&str],
        failures: &[&str],
        unverified: &[&str],
        by: &[&str],
    ) {
        let own = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect();
        crate::verdicts::journal_at(self.at("gate"))
            .append(&VerdictRow {
                ts: Timestamp(ts),
                body: body.into(),
                verdict: word.into(),
                changed: own(changed),
                failures: own(failures),
                unverified: own(unverified),
                unchecked: word.contains(", unchecked"),
                verified_by: own(by),
                entry: None,
                build: stamp(),
                run: run.map(Into::into),
            })
            .unwrap();
    }

    fn act(&self, post: Post, act: &str, state: &str, undoes: Option<&str>) {
        let mut row = json!({"act": act, "state": state, "at": "2026-11-01T10:00:00Z",
            "build": {"version": "9.9.9", "commit": "post000001"}});
        if let Some(u) = undoes {
            row["undoes"] = json!(u);
        }
        Journal::<serde_json::Value>::chained(crate::chains::post_witness_in(
            &self.state,
            &self.world,
            post,
        ))
        .append(&row)
        .unwrap();
    }

    fn fold(&self) -> Result<Folded, Unavailable> {
        fold_in(&self.world, &self.state)
    }

    /// The colony: the throne commissions an Elf and the forge's Kijin; the Elf two Orc runs; the
    /// first Orc run three Slime runs, `sly` twice.
    fn colony() -> Self {
        let f = Fixture::new();
        f.call("r0", "rimuru", "rimuru", None, 100);
        f.call("e1", "lana", "elf", Some("rimuru"), 110);
        f.call("o1", "gor", "orc", Some("lana"), 120);
        f.call("s1", "sly", "slime", Some("gor"), 130);
        f.verdict(Some("s1"), "sly", "FAIL, handed back", 131, &["src/a.rs"]);
        f.verdict(Some("s1"), "sly", "PASS", 132, &["src/a.rs"]);
        f.call("s2", "sam", "slime", Some("gor"), 140);
        // A verdict from before runs were named: the body's latest run begun before it.
        f.verdict(None, "sam", "PASS", 145, &["src/b.rs"]);
        f.call("s3", "sly", "slime", Some("gor"), 150);
        f.verdict_with(
            Some("s3"),
            "sly",
            "FAIL",
            151,
            &["src/a.rs"],
            &[
                "tests intact: `t.rs` has 1 tests, it had 2",
                "traits hold: `cargo test` failed:\nboom",
            ],
            &[],
        );
        // A verdict no check could run for is the machine's, never the body's.
        f.verdict(Some("s3"), "sly", "NOT_VERIFIED, unrun", 152, &["src/a.rs"]);
        f.call("o2", "gor", "orc", Some("lana"), 160);
        f.call("k1", "kijin-forge", "kijin", Some("rimuru"), 170);
        f.verdict_with(
            Some("k1"),
            "kijin-forge",
            "NOT_VERIFIED",
            171,
            &[".github/x.yml"],
            &[],
            &["traits unverified: `act` gave no result: killed"],
        );
        f.verdict(Some("r0"), "rimuru", "PASS", 200, &["a.rs"]);
        f.verdict(Some("r0"), "rimuru", "PASS", 210, &["b.rs"]);
        f.verdict(Some("r0"), "rimuru", "FAIL", 220, &["c.rs"]);
        f.verdict(Some("r0"), "rimuru", "PASS", 230, &["a.rs"]);
        f.act(Post::Forge, "a1", "committed", None);
        f.act(Post::Forge, "a1", "pending observation", None);
        f.act(Post::Forge, "a1", "observed", None);
        f.act(Post::Forge, "a2", "committed", None);
        f.act(Post::Forge, "a2", "reconcile", None);
        f.act(Post::Forge, "a3", "committed", None);
        f.act(Post::Forge, "u1", "committed", Some("a3"));
        f.act(Post::Forge, "u1", "observed", None);
        f.act(Post::Forge, "a3", "corrected", None);
        f.act(Post::Forge, "a4", "committed", None);
        f
    }
}

fn of(f: &Folded, creature: &str, race: Race, meter: &str) -> Rate {
    f.of(creature, race).rate(meter).unwrap()
}

/// Every race's meters, computed by hand over the colony's journals.
#[test]
fn each_races_meters_are_the_hand_computed_ones() {
    let f = Fixture::colony().fold().unwrap();
    // sly: s1 passed on its second try; s3 failed with a hole and a red verify line.
    assert_eq!(f.of("sly", Race::Slime).reports, 2);
    assert_eq!(of(&f, "sly", Race::Slime, "landRate"), Rate::of(1, 2));
    assert_eq!(of(&f, "sly", Race::Slime, "gateRate"), Rate::of(0, 2));
    assert_eq!(of(&f, "sly", Race::Slime, "holeRate"), Rate::of(1, 2));
    assert_eq!(
        of(&f, "sly", Race::Slime, "landRate").to_string(),
        "0.50 (1/2, 95% 0.09–0.91)"
    );
    // sam: one pass on the first try, from a verdict that names no run.
    assert_eq!(of(&f, "sam", Race::Slime, "gateRate"), Rate::of(1, 1));
    // gor: o1 commissioned s1 (landed), s2 (landed) and s3 (sly again, failed).
    assert_eq!(of(&f, "gor", Race::Orc, "gateRight"), Rate::of(2, 2));
    assert_eq!(of(&f, "gor", Race::Orc, "landRate"), Rate::of(0, 2));
    assert_eq!(of(&f, "gor", Race::Orc, "holeRate"), Rate::of(0, 2));
    assert_eq!(of(&f, "gor", Race::Orc, "planLandRate"), Rate::of(2, 3));
    assert_eq!(of(&f, "gor", Race::Orc, "reaskRate"), Rate::of(1, 3));
    // lana: o1 holds a pass and a fail under it, o2 nothing; gor commissioned twice.
    assert_eq!(of(&f, "lana", Race::Elf, "planLandRate"), Rate::of(0, 2));
    assert_eq!(of(&f, "lana", Race::Elf, "reaskRate"), Rate::of(1, 2));
    assert_eq!(of(&f, "lana", Race::Elf, "landRate"), Rate::of(0, 1));
    // kijin-forge: a1 clean, a2 in reconcile, a3 put back; a4 open and the undo u1 not counted.
    let k = f.of("kijin-forge", Race::Kijin);
    assert_eq!(k.outward_clean, Rate::of(1, 3));
    assert_eq!(k.rollbacks, 1);
    assert_eq!(k.verify_green, Rate::of(0, 1));
    assert_eq!(k.land, Rate::of(0, 1));
    // The throne: a.rs reopened within three landings, c.rs failed.
    assert_eq!(
        of(&f, "rimuru", Race::Rimuru, "askDoneRate"),
        Rate::of(2, 4)
    );
    // Each through the compiled law where every input it reads is known.
    let s = f.of("sly", Race::Slime).signals(Race::Slime).unwrap();
    assert!((s.land_rate - 50.0).abs() < 1e-9 && s.tests_intact_failures == 1);
    let e = f.of("lana", Race::Elf).signals(Race::Elf).unwrap();
    assert!((e.reask_rate - 50.0).abs() < 1e-9);
    assert_eq!(
        f.of("gor", Race::Orc).signals(Race::Orc),
        Err("slimeHealth"),
        "the strain is journaled only inside the world: unknown, never 0"
    );
}

/// A pass a Dark Elf's upheld finding names is overturned: the gate was not right.
#[test]
fn an_upheld_finding_overturns_the_pass_on_its_landing() {
    let f = Fixture::colony();
    let usage: Vec<UsageRow> = Journal::new(f.at("usage")).read().unwrap().rows;
    let verdicts: Vec<VerdictRow> = Journal::new(f.at("gate")).read().unwrap().rows;
    let upheld = BTreeSet::from([landing_run("s2")]);
    let rows = run_reports(&usage, &verdicts, &upheld);
    let o1 = rows
        .iter()
        .find_map(|r| match r {
            ReportRow::Run(r) if r.run == "o1" => Some(r),
            _ => None,
        })
        .unwrap();
    assert_eq!((o1.passes, o1.overturned), (2, 1));
    let inputs = fold_rows(&rows);
    assert_eq!(
        inputs[&("gor".to_owned(), "orc".to_owned())].gate_right,
        Rate::of(1, 2)
    );
}

/// The same journals fold to the same bytes, and the ledger kept is those bytes, chained.
#[test]
fn a_replay_gives_the_ledger_byte_for_byte() {
    let f = Fixture::colony();
    let (first, path) = record_in(&f.world, &f.state).unwrap();
    let kept = std::fs::read(&path).unwrap();
    assert!(path.starts_with(&f.state) && path.to_string_lossy().contains("mnemorph/reports/"));
    let (again, _) = record_in(&f.world, &f.state).unwrap();
    assert_eq!(first, again);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        kept,
        "folded twice: the same bytes"
    );
    assert_eq!(ledger_bytes(&f.fold().unwrap().rows).unwrap(), kept);
    let report = mnemorph_ledger::chain::verify(&path).unwrap();
    assert!(report.holds(), "{report}");
    assert_eq!(report.genesis.map(|g| g.line), Some(1));
    assert_eq!(report.rows, first.rows.len());
    let text = String::from_utf8(kept).unwrap();
    assert!(
        text.lines().all(|l| l.contains("\"build\":{")),
        "every row stamped"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    // A journal that grows folds again from the start: the ledger is the new fold, whole.
    f.call("s4", "sly", "slime", Some("gor"), 300);
    let (grown, _) = record_in(&f.world, &f.state).unwrap();
    assert_eq!(grown.rows.len(), first.rows.len() + 1);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        ledger_bytes(&grown.rows).unwrap()
    );
}

/// An edited row, a torn tail or a dropped tail past the anchored head stops the fold: every
/// meter it feeds reads unavailable, never stale, never zero.
#[test]
fn a_tampered_or_truncated_journal_stops_the_fold() {
    let f = Fixture::colony();
    let gate = f.at("gate");
    let raw = std::fs::read_to_string(&gate).unwrap();
    std::fs::write(&gate, raw.replacen("\"FAIL\"", "\"PASS\"", 1)).unwrap();
    let broken = f.fold().unwrap_err();
    assert!(
        matches!(&broken, Unavailable::ChainBroken { journal, .. } if journal == "gate"),
        "{broken}"
    );
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join(".mnemorph/slime/sly");
    std::fs::create_dir_all(&doc).unwrap();
    std::fs::write(doc.join("README.md"), "# sly\n\n- **Race:** slime\n").unwrap();
    let shown = crate::meters::of(
        &crate::Roster::load(dir.path()),
        &BTreeMap::new(),
        &Err(broken),
    );
    assert_eq!(shown[0].readings[0].rate, None);
    assert!(
        shown[0]
            .to_string()
            .starts_with("sly (slime): unavailable — the gate chain is broken at line"),
        "{}",
        shown[0]
    );

    // A torn tail: rows may be missing.
    std::fs::write(&gate, format!("{raw}{{\"ts\":\"20")).unwrap();
    assert!(matches!(
        f.fold(),
        Err(Unavailable::ChainBroken { journal, .. }) if journal == "gate"
    ));

    // The tail dropped whole, past the head anchored under the state directory.
    std::fs::write(&gate, &raw).unwrap();
    let head = mnemorph_ledger::chain::verify(&gate).unwrap().head.unwrap();
    Journal::<serde_json::Value>::new(f.at("anchors"))
        .append(&json!({"ts": "2026-11-01T10:00:00Z", "journal": "gate",
            "digest": head.digest, "line": head.line}))
        .unwrap();
    assert!(f.fold().is_ok());
    let kept: Vec<&str> = raw.lines().take(3).collect();
    std::fs::write(&gate, kept.join("\n") + "\n").unwrap();
    assert!(matches!(
        f.fold(),
        Err(Unavailable::ChainBroken { journal, .. }) if journal == "gate"
    ));

    // A Kijin post's witness edited stops it too.
    let f = Fixture::colony();
    let post = crate::chains::post_witness_in(&f.state, &f.world, Post::Forge);
    let raw = std::fs::read_to_string(&post).unwrap();
    std::fs::write(&post, raw.replacen("reconcile", "observed", 1)).unwrap();
    assert!(matches!(
        f.fold(),
        Err(Unavailable::ChainBroken { journal, .. }) if journal == "kijin-forge"
    ));

    // And the usage witness.
    let f = Fixture::colony();
    let usage = f.at("usage");
    let raw = std::fs::read_to_string(&usage).unwrap();
    std::fs::write(&usage, raw.replacen("\"slime\"", "\"orc\"", 1)).unwrap();
    assert!(matches!(
        f.fold(),
        Err(Unavailable::ChainBroken { journal, .. }) if journal == "usage-witness"
    ));
}

/// The world's own copies are never read: a body can write them. A state directory inside the
/// world is refused whole.
#[test]
fn an_in_world_journal_copy_is_ignored() {
    let f = Fixture::new();
    let copy = Journal::<UsageRow>::chained(f.world.join(crate::worth::USAGE));
    for run in ["x1", "x2"] {
        copy.append(&UsageRow {
            ts: Timestamp(1),
            session: "s".into(),
            run: run.into(),
            body: "sly".into(),
            race: "slime".into(),
            commission: None,
            model: "m".into(),
            provider: "p".into(),
            usage: Usage::default(),
            prefix: 0,
            wt: 0,
            usd: None,
            reported_cost: None,
            estimated: false,
            envelope: None,
            purpose: None,
        })
        .unwrap();
    }
    crate::verdicts::journal_at(f.world.join(".mnemorph/instruments/gate.jsonl"))
        .append(&VerdictRow {
            ts: Timestamp(2),
            body: "sly".into(),
            verdict: "PASS".into(),
            changed: vec!["a".into()],
            failures: Vec::new(),
            unverified: Vec::new(),
            unchecked: false,
            verified_by: Vec::new(),
            entry: None,
            build: Build::default(),
            run: Some("x1".into()),
        })
        .unwrap();
    let folded = f.fold().unwrap();
    assert!(folded.rows.is_empty(), "{:?}", folded.rows);
    assert_eq!(folded.of("sly", Race::Slime).reports, 0);

    let inside = f.world.join(".state");
    std::fs::create_dir_all(&inside).unwrap();
    assert!(matches!(
        fold_in(&f.world, &inside),
        Err(Unavailable::InWorld { .. })
    ));
}

/// A race with no runs reads `no reports`, its count zero, never a 0%.
#[test]
fn a_race_with_no_runs_reads_no_reports() {
    let f = Fixture::colony();
    let folded = f.fold().unwrap();
    let none = folded.of("nobody", Race::Slime);
    assert_eq!(none.reports, 0);
    assert_eq!(none.land.value(), None);
    assert_eq!(none.land.to_string(), "no reports");
    // A body's doc claiming another race claims no record: the witness names the race it ran as.
    assert_eq!(folded.of("sly", Race::Orc).reports, 0);
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join(".mnemorph/slime/nobody");
    std::fs::create_dir_all(&doc).unwrap();
    std::fs::write(doc.join("README.md"), "# nobody\n\n- **Race:** slime\n").unwrap();
    let shown = crate::meters::of(
        &crate::Roster::load(dir.path()),
        &BTreeMap::new(),
        &Ok(folded),
    );
    assert_eq!(shown[0].readings[0].rate, Some(Rate::of(0, 0)));
    assert_eq!(
        shown[0].to_string(),
        "nobody (slime): no reports — landRate, gateRate, holeRate"
    );
}

/// A journal with no chained row at all binds nothing: it is not trusted.
#[test]
fn an_unchained_journal_is_not_trusted() {
    let f = Fixture::new();
    let gate = f.at("gate");
    std::fs::create_dir_all(gate.parent().unwrap()).unwrap();
    std::fs::write(
        &gate,
        "{\"ts\":\"2026-10-30T12:00:00Z\",\"body\":\"sly\",\"verdict\":\"PASS\"}\n",
    )
    .unwrap();
    assert!(matches!(
        f.fold(),
        Err(Unavailable::ChainBroken { journal, .. }) if journal == "gate"
    ));
}

/// A chained line that is no row the fold can read stops it: a row read past is a report lost.
#[test]
fn a_row_the_fold_cannot_read_stops_it() {
    let f = Fixture::colony();
    Journal::<serde_json::Value>::chained(f.at("gate"))
        .append(&json!({"not": "a verdict"}))
        .unwrap();
    assert!(matches!(
        f.fold(),
        Err(Unavailable::Unreadable { journal, .. }) if journal == "gate"
    ));
}

/// A pass no verify line ran on lands as the baseline, never counted as verified; lines held back
/// for the human to pin are the human's, never the body's record.
#[test]
fn an_unchecked_pass_is_not_verified_and_an_unpinned_one_not_the_bodys() {
    let f = Fixture::new();
    f.call("a", "sly", "slime", None, 1);
    f.verdict(Some("a"), "sly", "PASS, unchecked", 2, &["x"]);
    f.call("b", "sly", "slime", None, 3);
    f.verdict(Some("b"), "sly", "NOT_VERIFIED, unpinned", 4, &["x"]);
    let i = f.fold().unwrap().of("sly", Race::Slime);
    assert_eq!(i.reports, 2);
    assert_eq!(i.land, Rate::of(0, 2));
    assert_eq!(
        i.verify_green,
        Rate::of(0, 0),
        "no line ran: nothing said of its lines"
    );
    assert_eq!(
        i.outcomes[0].gate,
        Some(mnemorph_races::GateWord::NotVerified)
    );
    assert_eq!(
        i.outcomes[1].gate, None,
        "held for the human: no landing of the body's"
    );
}

/// Keeps a head for `name` under the state directory, as `housekeep --verify` does.
fn anchor(f: &Fixture, name: &str, path: &Path) {
    let head = mnemorph_ledger::chain::verify(path).unwrap().head.unwrap();
    Journal::<serde_json::Value>::new(f.at("anchors"))
        .append(&json!({"ts": "2026-11-01T10:00:00Z", "journal": name,
            "digest": head.digest, "line": head.line}))
        .unwrap();
}

/// A pass counts as verified only when a line the human pinned held it: an Orc's pins alone, or
/// a verdict from before verdicts named their pins, is no verified landing and says nothing of
/// the verify lines.
#[test]
fn a_pass_on_an_orcs_pins_alone_is_not_verified() {
    use mnemorph_races::GateWord::{Fail, NotVerified, Pass};
    let f = Fixture::new();
    f.call("a", "sly", "slime", None, 1);
    f.verdict_by(
        Some("a"),
        "sly",
        "PASS",
        2,
        &["x"],
        &[],
        &[],
        &["orc-o's pin"],
    );
    f.call("b", "sly", "slime", None, 3);
    f.verdict_by(Some("b"), "sly", "PASS", 4, &["x"], &[], &[], &[]);
    f.call("c", "sly", "slime", None, 5);
    f.verdict_by(
        Some("c"),
        "sly",
        "PASS",
        6,
        &["x"],
        &[],
        &[],
        &["orc-o's pin", HUMAN_PIN],
    );
    f.call("d", "sly", "slime", None, 7);
    f.verdict_by(
        Some("d"),
        "sly",
        "FAIL",
        8,
        &["x"],
        &["traits hold: `t` failed:\nno"],
        &[],
        &["orc-o's pin"],
    );
    let i = f.fold().unwrap().of("sly", Race::Slime);
    assert_eq!(i.land, Rate::of(1, 4), "only c: the human's pin held it");
    assert_eq!(i.gate, Rate::of(1, 4));
    assert_eq!(
        i.verify_green,
        Rate::of(1, 1),
        "an Orc's lines say nothing of the human's: only c is read"
    );
    let gates: Vec<_> = i.outcomes.iter().map(|o| o.gate).collect();
    assert_eq!(
        gates,
        [Some(NotVerified), Some(NotVerified), Some(Pass), Some(Fail)]
    );
}

/// A journal deleted whole while its head stays anchored is no empty journal: the fold stops.
#[test]
fn a_deleted_anchored_journal_stops_the_fold() {
    let f = Fixture::colony();
    let gate = f.at("gate");
    anchor(&f, "gate", &gate);
    assert!(f.fold().is_ok());
    std::fs::remove_file(&gate).unwrap();
    let broken = f.fold().unwrap_err();
    assert!(
        matches!(&broken, Unavailable::ChainBroken { journal, .. } if journal == "gate"),
        "{broken}"
    );
    assert!(
        broken.to_string().contains("the journal is gone"),
        "{broken}"
    );
    // Never anchored, an absent journal is an empty one.
    let fresh = Fixture::new();
    assert_eq!(fresh.fold().unwrap().rows, Vec::new());
}

/// The usage witness is held to its own anchor: cut or written again, the fold stops.
#[test]
fn the_usage_witness_is_held_to_its_anchor() {
    let f = Fixture::colony();
    let usage = f.at("usage");
    anchor(&f, crate::chains::USAGE_WITNESS, &usage);
    assert!(f.fold().is_ok());
    let raw = std::fs::read_to_string(&usage).unwrap();
    let kept: Vec<&str> = raw.lines().take(2).collect();
    std::fs::write(&usage, kept.join("\n") + "\n").unwrap();
    assert!(matches!(
        f.fold(),
        Err(Unavailable::ChainBroken { journal, .. }) if journal == "usage-witness"
    ));
    // Regenerated from scratch: a chain that holds, but not the one anchored.
    std::fs::remove_file(&usage).unwrap();
    f.call("z", "sly", "slime", None, 1);
    assert!(matches!(
        f.fold(),
        Err(Unavailable::ChainBroken { journal, .. }) if journal == "usage-witness"
    ));
}

/// Challenge records that cannot be read drop no overturn silently: the fold stops.
#[test]
fn unreadable_challenge_records_stop_the_fold() {
    let f = Fixture::colony();
    std::fs::create_dir_all(f.at("challenges")).unwrap();
    assert!(matches!(
        f.fold(),
        Err(Unavailable::Unreadable { journal, .. }) if journal == "value"
    ));
}

/// A journal unchanged since its walk is not read again; one that grew is.
#[test]
fn an_unchanged_journal_is_not_read_again() {
    let f = Fixture::colony();
    std::thread::sleep(TICK * 2);
    let gate = f.at("gate");
    let usage = f.at("usage");
    let first = f.fold().unwrap();
    let (g, u) = (scans(&gate), scans(&usage));
    assert_eq!((g, u), (1, 1));
    let again = f.fold().unwrap();
    assert_eq!(first, again);
    assert_eq!(
        (scans(&gate), scans(&usage)),
        (1, 1),
        "no head moved: no rescan"
    );
    f.verdict(Some("s2"), "sam", "PASS", 146, &["src/b.rs"]);
    std::thread::sleep(TICK * 2);
    let grown = f.fold().unwrap();
    assert_eq!(scans(&gate), 2, "the gate's head moved");
    assert_eq!(scans(&usage), 1, "the usage witness's did not");
    assert_ne!(grown, first);
    // Edited in place to the same length: its change time moved, so it is read and found.
    let raw = std::fs::read_to_string(&gate).unwrap();
    std::fs::write(&gate, raw.replacen("\"FAIL\"", "\"PASS\"", 1)).unwrap();
    assert!(matches!(f.fold(), Err(Unavailable::ChainBroken { .. })));
}

/// Every report row carries the build of the row that settled it, and `unstamped` where that
/// row carries none.
#[test]
fn each_row_carries_its_sources_build() {
    let f = Fixture::colony();
    Journal::<serde_json::Value>::chained(crate::chains::post_witness_in(
        &f.state,
        &f.world,
        Post::Ci,
    ))
    .append(&json!({"act": "old", "state": "observed", "at": "2026-11-01T10:00:00Z"}))
    .unwrap();
    let rows = f.fold().unwrap().rows;
    let build = |want: &dyn Fn(&ReportRow) -> bool| {
        rows.iter()
            .find(|r| want(r))
            .map(|r| match r {
                ReportRow::Run(x) => x.build.clone(),
                ReportRow::Ask(x) => x.build.clone(),
                ReportRow::Act(x) => x.build.clone(),
            })
            .unwrap()
    };
    let unstamped = mnemorph_ledger::reports::unstamped();
    assert_eq!(
        build(&|r| matches!(r, ReportRow::Run(x) if x.run == "s1")),
        stamp()
    );
    assert_eq!(
        build(&|r| matches!(r, ReportRow::Run(x) if x.run == "o2")),
        unstamped,
        "no verdict settled it"
    );
    assert_eq!(build(&|r| matches!(r, ReportRow::Ask(_))), stamp());
    assert_eq!(
        build(&|r| matches!(r, ReportRow::Act(x) if x.act == "a1")).commit,
        "post000001"
    );
    assert_eq!(
        build(&|r| matches!(r, ReportRow::Act(x) if x.act == "old")),
        unstamped
    );
}
