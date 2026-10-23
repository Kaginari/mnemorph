//! The wire's promises, for any report: parsing what was emitted gives the same report, `@E`
//! is exact, a cap never cuts what must not be cut, and no field can forge a line.

use proptest::prelude::*;
use tempest_wire::{
    Commission, Courier, Enqueue, Finding, Forge, ForgeKind, Hole, Report, Unsaid, UnsaidKind,
};

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
    )
        .prop_map(|(id, at, claim, witness)| Finding {
            id,
            at,
            claim,
            witness,
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

proptest! {
    #[test]
    fn round_trip(r in report()) {
        let text = r.to_string();
        let mut back = Report::parse(&text).expect("an emitted report is on the wire");
        prop_assert_eq!(back.bytes, Some(text.len()), "@E is exact");
        back.bytes = None;
        prop_assert_eq!(back, r);
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
        prop_assert_eq!(back.word(), tempest_wire::Word::Fail);
        prop_assert_eq!(text.lines().filter(|l| l.trim_start().starts_with("@S")).count(), 1);
        prop_assert!(back.enqueues.is_empty() && back.courier.is_none());
    }

    #[test]
    fn a_cut_keeps_what_must_stay(r in report(), cap in 0usize..3000) {
        let out = r.emit(cap, Some(".tempest/tmp/full.md"));
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
            floor_with_hole.holes.push(Hole::Cut { cap, dropped: 999, dump: Some(".tempest/tmp/full.md".into()) });
            prop_assert!(out.len() <= cap.max(floor_with_hole.to_string().len()));
        }
    }

    #[test]
    fn envelope_carries_the_essentials(r in report()) {
        let filed = r.to_string();
        let env = r.envelope(".tempest/instruments/loop/run.md", &filed);
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
