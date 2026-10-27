//! The wire's promises, for any report: parsing what was emitted gives the same report, `@E`
//! is exact, a cap never cuts what must not be cut, and no field can forge a line.

use mnemorph_wire::{
    Commission, Courier, Enqueue, Finding, Forge, ForgeKind, Hole, Report, Unsaid, UnsaidKind,
};
use proptest::prelude::*;

/// A plain word.
fn plain() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9]{0,7}"
}

/// A word a claim may hold, the ones that look like other parts included.
fn word() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => plain(),
        1 => Just("12:30".to_owned()),
        1 => Just("#1".to_owned()),
        1 => Just("witness:foo".to_owned()),
        1 => Just("rec:foo".to_owned()),
        1 => Just("—".to_owned()),
        1 => Just("http://x:80".to_owned()),
        1 => Just("naïve".to_owned()),
        1 => Just("a|b".to_owned()),
        1 => Just("a.rs:3-9".to_owned()),
    ]
}

fn words() -> impl Strategy<Value = String> {
    prop::collection::vec(word(), 1..6).prop_map(|w| w.join(" "))
}

fn plain_words() -> impl Strategy<Value = String> {
    prop::collection::vec(plain(), 1..6).prop_map(|w| w.join(" "))
}

fn path() -> impl Strategy<Value = String> {
    "[a-z]{1,6}(/[a-z]{1,6}){0,2}\\.(rs|ts|md)"
}

fn finding() -> impl Strategy<Value = Finding> {
    (
        prop::option::of("[a-z0-9]{1,6}"),
        prop::option::of((path(), 1u32..5000).prop_map(|(p, l)| format!("{p}:{l}"))),
        words(),
        prop::option::of(path()),
        prop::option::of(path()),
    )
        .prop_map(|(id, at, claim, witness, rec)| Finding {
            id,
            at,
            claim,
            witness,
            rec,
        })
}

fn unsaid() -> impl Strategy<Value = Unsaid> {
    (
        prop::option::of(prop_oneof![
            Just(UnsaidKind::Law),
            Just(UnsaidKind::Colony),
            Just(UnsaidKind::Territory)
        ]),
        words(),
    )
        // An untyped line whose first word is a kind reads back typed: that is the grammar.
        .prop_filter("untyped text must not open with a kind", |(kind, text)| {
            kind.is_some()
                || !["law", "colony", "territory"].contains(&text.split(' ').next().unwrap_or(""))
        })
        .prop_map(|(kind, text)| Unsaid { kind, text })
}

fn hole() -> impl Strategy<Value = Hole> {
    prop_oneof![
        4 => plain_words().prop_map(Hole::Other),
        1 => (plain(), plain_words()).prop_map(|(finding, reason)| Hole::Contest { finding, reason }),
        1 => (0u64..1_000_000, 0u64..1_000_000).prop_map(|(need, have)| Hole::Budget { need, have }),
        1 => ("[0-9a-f]{10}", plain_words(), words())
            .prop_map(|(fingerprint, blocker, plan)| Hole::Nv { fingerprint, blocker, plan }),
    ]
}

fn forge() -> impl Strategy<Value = Forge> {
    (
        prop_oneof![Just(ForgeKind::Tool), Just(ForgeKind::Skill)],
        plain(),
        prop::option::of(words()),
    )
        .prop_map(|(kind, name, text)| Forge {
            kind,
            name,
            text: text.unwrap_or_default(),
        })
}

prop_compose! {
    fn report()(
        status in prop_oneof![Just("DONE"), Just("PASS"), Just("FAIL")],
        tail in prop::option::of(words()),
        holes in prop::collection::vec(hole(), 0..4),
        unsaid in prop::collection::vec(unsaid(), 0..4),
        courier in prop::option::of((path(), "[0-9a-f]{16}")),
        enqueues in prop::collection::vec((plain(), words()), 0..3),
        verdicts in prop::collection::vec(words(), 0..6),
        findings in prop::collection::vec(finding(), 0..30),
        written in prop::collection::vec(path(), 0..6),
        tools in prop::collection::vec(words(), 0..4),
        forges in prop::collection::vec(forge(), 0..3),
        other in prop::collection::vec(plain_words(), 0..4),
    ) -> Report {
        Report {
            status: tail.map_or_else(|| status.to_owned(), |t| format!("{status} {t}")),
            holes,
            unsaid,
            courier: courier.map(|(path, h)| Courier { path, hash: format!("fnv64:{h}") }),
            enqueues: enqueues.into_iter().map(|(body, ask)| Enqueue { body, ask }).collect(),
            verdicts,
            findings,
            written,
            tools,
            forges,
            other,
            bytes: None,
        }
    }
}

/// Every character some reader splits lines on; the encoder folds each to a space.
const BREAKS: [char; 10] = [
    '\n', '\r', '\u{0b}', '\u{0c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// A field value as the wire gives it back: breaks folded to spaces, then trimmed.
fn norm(text: &str) -> String {
    text.replace("\r\n", " ")
        .replace(BREAKS, " ")
        .trim()
        .to_owned()
}

/// A prose line as the wire gives it back: also without the whitespace and invisible characters
/// that led it.
fn norm_prose(text: &str) -> String {
    norm(text)
        .trim_start_matches(|c: char| c.is_whitespace() || mnemorph_wire::invisible(c))
        .to_owned()
}

/// A value built to break the envelope: tags, escapes, pipes and every kind of line break, or
/// any string at all.
fn hostile() -> impl Strategy<Value = String> {
    let piece = prop::sample::select(vec![
        "@",
        "@S",
        "@E 1",
        "@$",
        "@R",
        "@H",
        "@?",
        "@U",
        "@F",
        "@FORGE",
        "@CAP 9",
        "@ROOT /",
        "\u{1c}",
        "\u{1d}",
        "\u{200b}",
        "\u{feff}",
        "\u{202e}",
        "\u{2066}",
        "\u{2067}",
        "\u{2068}",
        "\u{2069}",
        "\u{a0}",
        "\u{3000}",
        "S|",
        "$|",
        "|",
        ":",
        "\\",
        "\\@",
        " ",
        "\t",
        "\n",
        "\r",
        "\r\n",
        "\u{0b}",
        "\u{0c}",
        "\u{1e}",
        "\u{85}",
        "\u{2028}",
        "\u{2029}",
        "x",
        "—",
        "#1",
        "a.rs:3",
        "witness:p",
        "rec:p",
        "rec:",
        "nv #",
        "blocker:",
        "plan:",
        "law",
        "DONE",
    ]);
    prop_oneof![
        3 => prop::collection::vec(piece, 0..8).prop_map(|p| p.concat()),
        1 => any::<String>(),
    ]
}

/// A hostile value that stays one word, as a path field must: no whitespace, no break.
fn hostile_word() -> impl Strategy<Value = Option<String>> {
    hostile().prop_map(|h| {
        let w: String = h
            .chars()
            .filter(|c| !c.is_whitespace() && !BREAKS.contains(c))
            .collect();
        (!w.is_empty()).then_some(w)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// A finding's tail keeps its fixed order, `witness:` then `rec:`, and no claim lends it a
    /// witness or a record: an `@F`, a `witness:` or a `rec:` inside a claim stays a claim.
    #[test]
    fn a_claim_never_lends_a_tail(
        id in prop::option::of("[0-9a-f]{10}"), claim in hostile(),
        witness in hostile_word(), rec in hostile_word(),
    ) {
        let f = Finding { id: id.clone(), at: None, claim: claim.clone(), witness: witness.clone(), rec: rec.clone() };
        let r = Report { status: "DONE".into(), findings: vec![f], ..Report::default() };
        let text = r.to_string();
        prop_assert_eq!(text.split(BREAKS).count(), 3, "@S, one @F, @E");
        let back = Report::parse(&text).expect("on the wire");
        let words = norm(&claim).split_whitespace().collect::<Vec<_>>().join(" ");
        prop_assert_eq!(&back.findings, &vec![Finding { id, at: None, claim: words, witness, rec }]);
        prop_assert!(back.holes.is_empty());
    }

    /// A needs-validation hole keeps its own line and its fields: the blocker up to `plan:`, the
    /// plan to the line's end. It is never a finding.
    #[test]
    fn a_needs_validation_keeps_its_fields(
        fingerprint in "[0-9a-f]{10}", blocker in hostile(), plan in hostile(), claim in hostile(),
    ) {
        let nv = Hole::Nv { fingerprint: fingerprint.clone(), blocker: blocker.clone(), plan: plan.clone() };
        let r = Report { status: "DONE".into(), holes: vec![nv.clone()], ..Report::default() };
        let text = r.to_string();
        prop_assert_eq!(text.split(BREAKS).count(), 3, "@S, one @?, @E");
        let back = Report::parse(&text).expect("on the wire").dark_elf();
        prop_assert!(back.findings.is_empty());
        prop_assert_eq!(&back.holes, &vec![Hole::parse(&norm(&nv.to_string()))]);
        let (b, p) = (norm(&blocker), norm(&plan));
        let clean = !b.is_empty() && !p.is_empty()
            && !b.split_whitespace().any(|w| w.starts_with("plan:") || w.to_ascii_lowercase().starts_with("severity:"));
        if clean {
            prop_assert_eq!(&back.holes, &vec![Hole::Nv { fingerprint, blocker: b, plan: p }]);
        }
        // An `@? nv` riding inside a claim stays a claim.
        let f = Finding { claim: format!("{claim} @? {nv}"), ..Finding::default() };
        let back = Report::parse(&Report { status: "DONE".into(), findings: vec![f], ..Report::default() }.to_string())
            .expect("on the wire");
        prop_assert!(back.holes.is_empty());
        prop_assert_eq!(back.findings.len(), 1);
    }

    #[test]
    fn no_field_forges_a_line(status in any::<String>(), claim in any::<String>(), hole in any::<String>(), other in any::<String>()) {
        let r = Report {
            status: format!("FAIL {status}"),
            findings: vec![Finding { claim, ..Finding::default() }],
            holes: vec![Hole::Other(hole)],
            other: vec![other],
            ..Report::default()
        };
        let text = r.to_string();
        prop_assert!(Report::is_whole(&text));
        let back = Report::parse(&text).expect("on the wire");
        prop_assert_eq!(back.word(), mnemorph_wire::Word::Fail);
        prop_assert_eq!(text.lines().filter(|l| l.trim_start().starts_with("@S")).count(), 1);
        prop_assert!(back.enqueues.is_empty() && back.courier.is_none());
    }

    /// Any value in any field comes back as written, up to folding and trimming, on a line of
    /// its own: one line per field, whatever the reader splits lines on.
    #[test]
    fn every_field_keeps_its_own_line(
        status in hostile(), hole in hostile(), unsaid in hostile(), path in hostile(),
        hash in hostile(), ask in hostile(), verdict in hostile(), claim in hostile(),
        written in hostile(), tool in hostile(), forge in hostile(), other in hostile(),
    ) {
        let r = Report {
            status: format!("FAIL {status}"),
            holes: vec![Hole::Other(hole.clone())],
            unsaid: vec![Unsaid { kind: Some(UnsaidKind::Law), text: unsaid.clone() }],
            courier: Some(Courier { path: path.clone(), hash: hash.clone() }),
            enqueues: vec![Enqueue { body: "orc".into(), ask: ask.clone() }],
            verdicts: vec![verdict.clone()],
            findings: vec![Finding { claim: claim.clone(), ..Finding::default() }],
            written: vec![written.clone()],
            tools: vec![tool.clone()],
            forges: vec![Forge { kind: ForgeKind::Tool, name: "x".into(), text: forge.clone() }],
            other: vec![other.clone()],
            bytes: None,
        };
        let text = r.to_string();
        prop_assert_eq!(text.split(BREAKS).count(), 13, "one line per field, and @E");
        for back in [Report::parse(&text), Report::parse_raw(&text)] {
            let back = back.expect("on the wire");
            prop_assert_eq!(back.bytes, Some(text.len()));
            prop_assert_eq!(back.word(), mnemorph_wire::Word::Fail);
            prop_assert_eq!(&back.status, &norm(&format!("FAIL {status}")));
            prop_assert_eq!(&back.holes, &vec![Hole::parse(&norm(&hole))]);
            prop_assert_eq!(&back.unsaid, &vec![Unsaid { kind: Some(UnsaidKind::Law), text: norm(&unsaid) }]);
            prop_assert_eq!(&back.courier, &Some(Courier { path: norm(&path), hash: norm(&hash) }));
            prop_assert_eq!(&back.enqueues, &vec![Enqueue { body: "orc".into(), ask: norm(&ask) }]);
            prop_assert_eq!(&back.verdicts, &vec![norm(&verdict)]);
            let words = norm(&claim).split_whitespace().collect::<Vec<_>>().join(" ");
            prop_assert_eq!(&back.findings, &vec![Finding { claim: words, ..Finding::default() }]);
            prop_assert_eq!(&back.written, &vec![norm(&written)]);
            prop_assert_eq!(&back.tools, &vec![norm(&tool)]);
            prop_assert_eq!(&back.forges, &vec![Forge { kind: ForgeKind::Tool, name: "x".into(), text: norm(&forge) }]);
            let prose: Vec<String> = Some(norm_prose(&other)).into_iter().filter(|o| !o.is_empty()).collect();
            prop_assert_eq!(&back.other, &prose);
        }
    }

    /// Prose in a commission's body stays prose: it comes back as written and sets no tag.
    #[test]
    fn a_commission_body_stays_prose(body in prop::collection::vec(hostile(), 0..4)) {
        let c = Commission { body: body.clone(), ..Commission::default() };
        let back = Commission::parse(&c.to_string());
        let prose: Vec<String> = body.iter().map(|b| norm_prose(b)).filter(|b| !b.is_empty()).collect();
        prop_assert_eq!(back, Commission { body: prose, ..Commission::default() });
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn round_trip(r in report()) {
        let text = r.to_string();
        let mut back = Report::parse(&text).expect("an emitted report is on the wire");
        prop_assert_eq!(back.bytes, Some(text.len()), "@E is exact");
        back.bytes = None;
        prop_assert_eq!(back, r);
    }
}

proptest! {
    #[test]
    fn a_cut_keeps_what_must_stay(r in report(), cap in 0usize..3000) {
        let out = r.emit(cap, Some(".mnemorph/tmp/full.md"));
        let back = Report::parse(&out).expect("a cut report is on the wire");
        prop_assert_eq!(back.bytes, Some(out.len()));
        prop_assert_eq!(&back.status, &r.status);
        prop_assert_eq!(&back.unsaid, &r.unsaid);
        prop_assert_eq!(&back.courier, &r.courier);
        prop_assert_eq!(&back.enqueues, &r.enqueues);
        prop_assert!(r.holes.iter().all(|h| back.holes.contains(h)));
        let whole = r.to_string();
        if cap == 0 || whole.len() <= cap {
            prop_assert_eq!(out, whole);
        } else {
            let floor = Report {
                status: r.status.clone(),
                holes: r.holes.clone(),
                unsaid: r.unsaid.clone(),
                courier: r.courier.clone(),
                enqueues: r.enqueues.clone(),
                ..Report::default()
            };
            if whole.len() > floor.to_string().len() {
                let cuts = back.holes.iter().filter(|h| matches!(h, Hole::Cut { .. })).count();
                prop_assert_eq!(cuts, 1);
            }
            // Only the uncuttable floor, plus the cut hole, may exceed the cap.
            let mut floor_with_hole = floor;
            floor_with_hole.holes.push(Hole::Cut { cap, dropped: 999, dump: Some(".mnemorph/tmp/full.md".into()) });
            prop_assert!(out.len() <= cap.max(floor_with_hole.to_string().len()));
        }
    }

    #[test]
    fn envelope_carries_the_essentials(r in report()) {
        let filed = r.to_string();
        let env = r.envelope(".mnemorph/instruments/loop/run.md", &filed);
        let back = Report::parse(&env.to_string()).expect("on the wire");
        prop_assert_eq!(&back.status, &r.status);
        prop_assert_eq!(&back.holes, &r.holes);
        prop_assert_eq!(&back.enqueues, &r.enqueues);
        prop_assert_eq!(&back.written, &r.written);
        prop_assert!(back.verdicts.len() + back.findings.len() <= 8);
        let c = back.courier.expect("an envelope carries its courier");
        prop_assert!(c.matches(&filed));
    }

    #[test]
    fn commission_round_trip(
        root in prop::option::of(path()),
        scope in prop::option::of(plain_words()),
        unsaid in any::<bool>(),
        raw in any::<bool>(),
        cap in prop::option::of(1usize..100_000),
        budget in prop::option::of(0u64..10_000_000),
        body in prop::collection::vec(plain_words(), 0..4),
    ) {
        let c = Commission { root, scope, unsaid, raw, cap, budget, body, ..Commission::default() };
        prop_assert_eq!(Commission::parse(&c.to_string()), c);
    }
}
