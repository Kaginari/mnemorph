//! Explainable decisions: every rise, probation and fall is one typed line per bar, and the line
//! recomputes the decision it explains.

#![allow(
    clippy::float_cmp,
    reason = "the law reads exact readings; so do its tests"
)]

use mnemorph_races::{
    Explanation, FallReading, Kind, Op, Race, Recent, Scale, Signals, explain_fall, explain_rise,
    falls, recovers, rerise, rises,
};
use proptest::prelude::*;

fn signals() -> impl Strategy<Value = Signals> {
    let rate = || prop_oneof![Just(0.0), Just(100.0), 0.0..=100.0f64];
    (
        (
            0u32..40,
            0u32..40,
            0u32..40,
            0u32..3,
            0u32..3,
            0u32..3,
            0u32..3,
        ),
        (rate(), rate(), rate(), rate(), rate()),
        (rate(), rate(), rate(), rate(), rate()),
    )
        .prop_map(
            |((r, f, o, rb, ec, ov, ti), (a, b, c, d, e), (g, h, i, j, k))| Signals {
                reports: r,
                findings: f,
                outward_acts: o,
                rollbacks: rb,
                envelope_corrections: ec,
                overturned_on_sealed: ov,
                tests_intact_failures: ti,
                plan_land_rate: a,
                reask_rate: b,
                land_rate: c,
                gate_right: d,
                gate_rate: e,
                hole_rate: g,
                outward_clean_rate: h,
                upheld_rate: i,
                precision: j,
                verify_green_rate: k,
                ..Signals::default()
            },
        )
}

proptest! {
    /// The lines are the decision: a rise is every rise line holding; a fall is its lines, an
    /// event first; each line, recomputed from its reading, holds.
    #[test]
    fn the_lines_recompute_the_decision(s in signals()) {
        for race in Race::ALL {
            let up = explain_rise(race, &s);
            prop_assert_eq!(rises(race, &s), !up.is_empty() && up.iter().all(Explanation::holds));
            prop_assert!(up.iter().all(|e| e.kind == Kind::Rise));
            let down = explain_fall(race, &Recent(s));
            prop_assert!(down.iter().all(Explanation::holds), "{:?}", down);
            let reading = match down.first().map(|e| e.kind) {
                None => FallReading::Holds,
                Some(Kind::Fall) => FallReading::Immediate,
                Some(_) => FallReading::Probation,
            };
            prop_assert_eq!(falls(race, &Recent(s)), reading);
            for e in &down {
                prop_assert!(e.rerise.is_some(), "a fallen form has a bar to rise again");
                prop_assert_eq!(e.recover.is_some(), e.kind == Kind::Probation);
                // Recovering is the fall bar's own reading, turned round.
                if let Some(r) = e.recover {
                    prop_assert_eq!(r.threshold, e.threshold);
                    prop_assert_eq!(r.op, Op::Ge);
                }
            }
        }
    }
}

#[test]
fn a_probation_line_names_its_bar_its_recovery_and_its_rise() {
    let recent = Recent(Signals {
        reports: 10,
        land_rate: 71.0,
        ..Signals::default()
    });
    let lines = explain_fall(Race::NamedSlime, &recent);
    assert_eq!(lines.len(), 1);
    let e = &lines[0];
    assert_eq!(
        (e.kind, e.meter, e.scale, e.op, e.threshold, e.window),
        (Kind::Probation, "landRate", Scale::Rate, Op::Lt, 75.0, 10)
    );
    assert_eq!(
        e.to_string(),
        "probation: landRate 0.71 < 0.75 (last 10 reports); recover: >= 0.75 within 5 reports; \
         re-rise: landRate >= 0.90 over 20 reports"
    );
    // With the ledger it was read from: its head and the window's first and last row.
    let on = e.clone().on_ledger(Some("3f9a"), Some((12, 48)));
    assert_eq!((on.from, on.to), (Some(12), Some(48)));
    assert!(
        on.to_string().ends_with("; ledger head 3f9a, rows 12–48"),
        "{on}"
    );
    let unchained = e.clone().on_ledger(None, Some((1, 9)));
    assert!(
        unchained
            .to_string()
            .ends_with("; ledger unchained, rows 1–9")
    );
    let headed = e.clone().on_ledger(Some("3f9a"), None);
    assert!(headed.to_string().ends_with("; ledger head 3f9a"));
    // A reading just under the bar shows the digits that keep it under.
    let close = Recent(Signals {
        reports: 10,
        land_rate: 74.95,
        ..Signals::default()
    });
    assert!(
        explain_fall(Race::NamedSlime, &close)[0]
            .to_string()
            .starts_with("probation: landRate 0.7495 < 0.75")
    );
    // Recovered at the bar it fell at.
    assert!(recovers(
        Race::NamedSlime,
        &Recent(Signals {
            reports: 5,
            land_rate: 75.0,
            ..Signals::default()
        })
    ));
}

#[test]
fn an_event_line_falls_at_once_and_names_the_rise_again() {
    let recent = Recent(Signals {
        outward_acts: 12,
        rollbacks: 1,
        envelope_corrections: 2,
        outward_clean_rate: 100.0,
        ..Signals::default()
    });
    let lines = explain_fall(Race::Oni, &recent);
    let shown: Vec<String> = lines.iter().map(ToString::to_string).collect();
    assert_eq!(
        shown,
        [
            "fall: rollbacks 1 > 0 (last 12 outward acts); recover: none, it falls at once; \
             re-rise: outwardCleanRate >= 0.95 over 30 outward acts",
            "fall: envelopeCorrections 2 > 0 (last 12 outward acts); recover: none, it falls at \
             once; re-rise: outwardCleanRate >= 0.95 over 30 outward acts",
        ]
    );
    // An event outranks a rate under its bar.
    let both = Recent(Signals {
        reports: 10,
        gate_right: 10.0,
        overturned_on_sealed: 1,
        ..Signals::default()
    });
    let lines = explain_fall(Race::HighOrc, &both);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        (lines[0].kind, lines[0].meter),
        (Kind::Fall, "overturnedOnSealed")
    );
    assert_eq!(lines[0].rerise.map(|r| r.meter), Some("gateRight"));
}

#[test]
fn a_rise_names_every_bar() {
    let s = Signals {
        findings: 20,
        upheld_rate: 85.0,
        precision: 90.0,
        ..Signals::default()
    };
    let shown: Vec<String> = explain_rise(Race::ArchDarkElf, &s)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        shown,
        [
            "rise: findings 20 >= 20 (over 20 findings)",
            "rise: upheldRate 0.85 >= 0.80 (over 20 findings)",
            "rise: precision 0.90 >= 0.85 (over 20 findings)",
        ]
    );
    let held = explain_rise(Race::HighElf, &Signals::default());
    assert_eq!(
        held[2].to_string(),
        "rise: reaskRate 0.00 <= 0.10 (over 0 reports)"
    );
    assert!(
        explain_rise(Race::Elf, &s).is_empty(),
        "a base race has no rise"
    );
    // A fall on a meter with no rise bar of its own re-rises on the form's first rate.
    assert_eq!(
        rerise(Race::ArchDarkElf, "cannotVerifyRate").map(|r| (r.meter, r.threshold, r.over)),
        Some(("upheldRate", 80.0, 20))
    );
    assert_eq!(
        rerise(Race::ArchDarkElf, "precision").map(|r| r.meter),
        Some("precision")
    );
    assert_eq!(rerise(Race::Slime, "landRate"), None);
}

#[test]
fn every_operator_reads_as_the_law_holds_it() {
    for (op, symbol, under, at, over) in [
        (Op::Lt, "<", true, false, false),
        (Op::Le, "<=", true, true, false),
        (Op::Ge, ">=", false, true, true),
        (Op::Gt, ">", false, false, true),
    ] {
        assert_eq!(op.symbol(), symbol);
        assert_eq!(
            (op.holds(1.0, 2.0), op.holds(2.0, 2.0), op.holds(3.0, 2.0)),
            (under, at, over),
            "{symbol}"
        );
    }
    assert_eq!(
        [Kind::Rise, Kind::Probation, Kind::Fall].map(Kind::word),
        ["rise", "probation", "fall"]
    );
}

#[test]
fn an_evolution_line_reads_over_its_window_against_doing_nothing() {
    let line = |kind, op, reading| Explanation {
        kind,
        meter: "landRate",
        scale: Scale::Rate,
        reading,
        op,
        threshold: 70.0,
        window: 10,
        unit: "runs beside do-nothing",
        recover: None,
        rerise: None,
        head: Some("3f9a".into()),
        from: Some(4),
        to: Some(23),
    };
    let keep = line(Kind::Keep, Op::Gt, 90.0);
    assert!(keep.holds());
    assert_eq!(
        keep.to_string(),
        "keep: landRate 0.90 > 0.70 (over 10 runs beside do-nothing); ledger head 3f9a, rows 4–23"
    );
    let rollback = line(Kind::Rollback, Op::Le, 70.0);
    assert!(rollback.holds());
    assert_eq!(
        rollback.to_string(),
        "rollback: landRate 0.70 <= 0.70 (over 10 runs beside do-nothing); ledger head 3f9a, rows \
         4–23"
    );
}
