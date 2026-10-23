//! The race law, table by table: every cell of the dispatch decision table, every bar at, just
//! under and just over its threshold.

use mnemorph_races::{
    Band, DispatchContext, FallReading, RECOVER_REPORTS, Race, Recent, Refusal, Route, Signals,
    falls, may_dispatch, meter, recovers, rises, score,
};

const ALL_TRUE: DispatchContext = DispatchContext {
    lineage: true,
    plan_names_target: true,
    verify_coded: true,
    other_lane: false,
    budget_fits: true,
};

#[test]
fn dispatch_follows_the_tree() {
    for from in Race::ALL {
        for to in Race::ALL {
            let got = may_dispatch(from, to, &ALL_TRUE);
            let commanded = from.commands().contains(&to.base());
            let sideways = from.base() == Race::Orc && to.base() == Race::Orc;
            if commanded || sideways {
                assert_eq!(got, Ok(Route::Direct), "{from} → {to}");
            } else {
                assert_eq!(got, Err(Refusal::NotOneHop), "{from} → {to}");
            }
        }
    }
}

#[test]
fn dispatch_rules_in_order() {
    let ctx = |f: fn(&mut DispatchContext)| {
        let mut c = ALL_TRUE;
        f(&mut c);
        c
    };
    // Lineage binds commanders, not sideways Orcs.
    assert_eq!(
        may_dispatch(Race::Elf, Race::Orc, &ctx(|c| c.lineage = false)),
        Err(Refusal::NotYourLine)
    );
    assert_eq!(
        may_dispatch(Race::Orc, Race::HighOrc, &ctx(|c| c.lineage = false)),
        Ok(Route::Direct)
    );
    // Elves need the plan item; nobody else does.
    assert_eq!(
        may_dispatch(
            Race::HighElf,
            Race::Orc,
            &ctx(|c| c.plan_names_target = false)
        ),
        Err(Refusal::NoPlanItem)
    );
    assert_eq!(
        may_dispatch(
            Race::Rimuru,
            Race::Elf,
            &ctx(|c| c.plan_names_target = false)
        ),
        Ok(Route::Direct)
    );
    // Orcs need the verify line before a Slime; not before another Orc.
    assert_eq!(
        may_dispatch(
            Race::Orc,
            Race::NamedSlime,
            &ctx(|c| c.verify_coded = false)
        ),
        Err(Refusal::VerifyFirst)
    );
    assert_eq!(
        may_dispatch(Race::Orc, Race::Orc, &ctx(|c| c.verify_coded = false)),
        Ok(Route::Direct)
    );
    // Budget binds every dispatch, sideways included.
    assert_eq!(
        may_dispatch(Race::Rimuru, Race::Kijin, &ctx(|c| c.budget_fits = false)),
        Err(Refusal::Budget)
    );
    assert_eq!(
        may_dispatch(Race::Orc, Race::Orc, &ctx(|c| c.budget_fits = false)),
        Err(Refusal::Budget)
    );
    // The order: lineage is reported before the plan, the plan before the budget.
    let none = DispatchContext::default();
    assert_eq!(
        may_dispatch(Race::Elf, Race::Orc, &none),
        Err(Refusal::NotYourLine)
    );
    assert_eq!(
        may_dispatch(
            Race::Elf,
            Race::Orc,
            &DispatchContext {
                lineage: true,
                ..none
            }
        ),
        Err(Refusal::NoPlanItem)
    );
    assert_eq!(
        may_dispatch(
            Race::Orc,
            Race::Slime,
            &DispatchContext {
                lineage: true,
                ..none
            }
        ),
        Err(Refusal::VerifyFirst)
    );
}

#[test]
fn a_high_elf_reaches_another_lane_through_its_elf() {
    let other = DispatchContext {
        other_lane: true,
        lineage: false,
        ..ALL_TRUE
    };
    assert_eq!(
        may_dispatch(Race::HighElf, Race::Orc, &other),
        Ok(Route::ViaItsElf)
    );
    assert_eq!(
        may_dispatch(Race::HighElf, Race::HighOrc, &other),
        Ok(Route::ViaItsElf)
    );
    assert_eq!(
        may_dispatch(
            Race::HighElf,
            Race::Orc,
            &DispatchContext {
                budget_fits: false,
                ..other
            }
        ),
        Err(Refusal::Budget)
    );
    // A plain Elf has no such power; a High Elf never reaches a Slime that way.
    assert_eq!(
        may_dispatch(Race::Elf, Race::Orc, &other),
        Err(Refusal::NotYourLine)
    );
    assert_eq!(
        may_dispatch(Race::HighElf, Race::Slime, &other),
        Err(Refusal::NotOneHop)
    );
}

#[test]
fn every_refusal_names_a_next_act() {
    for r in [
        Refusal::NotOneHop,
        Refusal::NotYourLine,
        Refusal::NoPlanItem,
        Refusal::VerifyFirst,
        Refusal::Budget,
    ] {
        let text = r.to_string();
        assert!(
            text.starts_with("refused: ") && text.len() > "refused: ".len() + 10,
            "{text}"
        );
    }
}

#[test]
fn scores_follow_the_formulas_and_clamp() {
    let s = Signals {
        ask_done_rate: 90.0,
        plan_land_rate: 80.0,
        reask_rate: 10.0,
        land_rate: 70.0,
        gate_right: 95.0,
        slime_health: 60.0,
        gate_rate: 50.0,
        hole_rate: 20.0,
        outward_clean_rate: 100.0,
        verify_green_rate: 80.0,
        upheld_rate: 60.0,
        precision: 90.0,
        ..Signals::default()
    };
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(close(score(Race::Rimuru, &s), 90.0));
    assert!(close(
        score(Race::Elf, &s),
        0.6 * 80.0 - 0.2 * 10.0 + 0.2 * 70.0
    ));
    assert!(close(
        score(Race::HighOrc, &s),
        0.5 * 95.0 + 0.3 * 60.0 + 0.2 * 70.0
    ));
    assert!(close(
        score(Race::Slime, &s),
        0.6 * 70.0 + 0.3 * 50.0 - 0.1 * 20.0
    ));
    assert!(close(
        score(Race::Oni, &s),
        0.5 * 100.0 + 0.3 * 80.0 + 0.2 * 70.0
    ));
    assert!(close(score(Race::ArchDarkElf, &s), 0.7 * 60.0 + 0.3 * 90.0));
    let bad = Signals {
        hole_rate: 100.0,
        reask_rate: 100.0,
        ..Signals::default()
    };
    assert!(close(score(Race::Slime, &bad), 0.0) && close(score(Race::Elf, &bad), 0.0));
    let high = Signals {
        ask_done_rate: 150.0,
        ..Signals::default()
    };
    assert!(close(score(Race::Rimuru, &high), 100.0));
}

#[test]
fn meters_band_their_reading() {
    let at = |race: Race, value: f64| {
        let s = Signals {
            reports: 5,
            ask_done_rate: value,
            plan_land_rate: value,
            land_rate: value,
            gate_right: value,
            outward_clean_rate: value,
            upheld_rate: value,
            ..Signals::default()
        };
        meter(race, &s).band
    };
    for (race, ok, warn) in [
        (Race::Rimuru, 80.0, 50.0),
        (Race::Elf, 80.0, 50.0),
        (Race::Orc, 90.0, 70.0),
        (Race::Slime, 80.0, 50.0),
        (Race::Kijin, 90.0, 70.0),
        (Race::DarkElf, 80.0, 50.0),
    ] {
        assert_eq!(at(race, ok), Band::Ok, "{race} at ok");
        assert_eq!(at(race, ok - 0.1), Band::Warn, "{race} under ok");
        assert_eq!(at(race, warn), Band::Warn, "{race} at warn");
        assert_eq!(at(race, warn - 0.1), Band::Low, "{race} under warn");
        assert_ne!(meter(race, &Signals::default()).label, "");
    }
    let few = Signals {
        reports: 4,
        land_rate: 100.0,
        ..Signals::default()
    };
    assert_eq!(meter(Race::Slime, &few).band, Band::Unranked);
}

/// For each ascended form: a record exactly at its bars, and one bar at a time nudged just past.
#[allow(clippy::too_many_lines)] // a table, one row per bar
fn rise_cases() -> Vec<(Race, Signals, Vec<Signals>)> {
    let high_elf = Signals {
        reports: 20,
        plan_land_rate: 85.0,
        reask_rate: 10.0,
        ..Signals::default()
    };
    let high_orc = Signals {
        reports: 20,
        gate_right: 90.0,
        hole_rate: 10.0,
        ..Signals::default()
    };
    let named = Signals {
        reports: 20,
        land_rate: 90.0,
        gate_rate: 90.0,
        hole_rate: 5.0,
        ..Signals::default()
    };
    let oni = Signals {
        outward_acts: 30,
        outward_clean_rate: 95.0,
        ..Signals::default()
    };
    let arch = Signals {
        findings: 20,
        upheld_rate: 80.0,
        precision: 85.0,
        ..Signals::default()
    };
    vec![
        (
            Race::HighElf,
            high_elf,
            vec![
                Signals {
                    reports: 19,
                    ..high_elf
                },
                Signals {
                    plan_land_rate: 84.9,
                    ..high_elf
                },
                Signals {
                    reask_rate: 10.1,
                    ..high_elf
                },
            ],
        ),
        (
            Race::HighOrc,
            high_orc,
            vec![
                Signals {
                    reports: 19,
                    ..high_orc
                },
                Signals {
                    gate_right: 89.9,
                    ..high_orc
                },
                Signals {
                    hole_rate: 10.1,
                    ..high_orc
                },
            ],
        ),
        (
            Race::NamedSlime,
            named,
            vec![
                Signals {
                    reports: 19,
                    ..named
                },
                Signals {
                    land_rate: 89.9,
                    ..named
                },
                Signals {
                    gate_rate: 89.9,
                    ..named
                },
                Signals {
                    hole_rate: 5.1,
                    ..named
                },
            ],
        ),
        (
            Race::Oni,
            oni,
            vec![
                Signals {
                    outward_acts: 29,
                    ..oni
                },
                Signals {
                    outward_clean_rate: 94.9,
                    ..oni
                },
                Signals {
                    rollbacks: 1,
                    ..oni
                },
            ],
        ),
        (
            Race::ArchDarkElf,
            arch,
            vec![
                Signals {
                    findings: 19,
                    ..arch
                },
                Signals {
                    upheld_rate: 79.9,
                    ..arch
                },
                Signals {
                    precision: 84.9,
                    ..arch
                },
            ],
        ),
    ]
}

#[test]
fn rise_bars_at_under_and_over() {
    for (risen, at_bar, misses) in rise_cases() {
        let base = risen.base();
        assert!(rises(risen, &at_bar), "{risen} at its bars");
        for miss in misses {
            assert!(!rises(risen, &miss), "{risen} just past a bar: {miss:?}");
        }
        assert!(!rises(base, &at_bar), "{base} is not an ascended form");
        // Rising at the bar never reads as a fall: the bars sit apart.
        let recent = Signals {
            reports: 10,
            findings: 10,
            ..at_bar
        };
        assert_eq!(
            falls(risen, &Recent(recent)),
            FallReading::Holds,
            "{risen} flaps"
        );
    }
    for race in Race::ALL {
        assert!(!rises(race, &Signals::default()), "{race} rises on nothing");
    }
}

#[test]
fn falls_by_rate_need_a_full_recent_record() {
    for (risen, low) in [
        (
            Race::HighElf,
            Signals {
                plan_land_rate: 69.9,
                ..Signals::default()
            },
        ),
        (
            Race::HighOrc,
            Signals {
                gate_right: 74.9,
                ..Signals::default()
            },
        ),
        (
            Race::NamedSlime,
            Signals {
                land_rate: 74.9,
                ..Signals::default()
            },
        ),
        (
            Race::ArchDarkElf,
            Signals {
                precision: 69.9,
                ..Signals::default()
            },
        ),
    ] {
        let full = Signals {
            reports: 10,
            findings: 10,
            ..low
        };
        let short = Signals {
            reports: 9,
            findings: 9,
            ..low
        };
        assert_eq!(
            falls(risen, &Recent(full)),
            FallReading::Probation,
            "{risen}"
        );
        assert_eq!(
            falls(risen, &Recent(short)),
            FallReading::Holds,
            "{risen} on a short record"
        );
    }
}

#[test]
fn falls_by_event_are_immediate() {
    let healthy = Signals {
        reports: 10,
        gate_right: 100.0,
        land_rate: 100.0,
        outward_clean_rate: 100.0,
        ..Signals::default()
    };
    for (risen, event) in [
        (
            Race::HighOrc,
            Signals {
                overturned_on_sealed: 1,
                ..healthy
            },
        ),
        (
            Race::NamedSlime,
            Signals {
                tests_intact_failures: 1,
                ..healthy
            },
        ),
        (
            Race::Oni,
            Signals {
                rollbacks: 1,
                ..healthy
            },
        ),
        (
            Race::Oni,
            Signals {
                envelope_corrections: 1,
                ..healthy
            },
        ),
    ] {
        assert_eq!(
            falls(risen, &Recent(healthy)),
            FallReading::Holds,
            "{risen} healthy"
        );
        assert_eq!(
            falls(risen, &Recent(event)),
            FallReading::Immediate,
            "{risen}"
        );
    }
    // An event that is not its own does not fell a body; a base race never falls.
    assert_eq!(
        falls(
            Race::HighElf,
            &Recent(Signals {
                rollbacks: 3,
                reports: 10,
                plan_land_rate: 100.0,
                ..Signals::default()
            })
        ),
        FallReading::Holds
    );
    for base in [
        Race::Rimuru,
        Race::Elf,
        Race::Orc,
        Race::Slime,
        Race::Kijin,
        Race::DarkElf,
    ] {
        assert_eq!(
            falls(
                base,
                &Recent(Signals {
                    reports: 50,
                    rollbacks: 5,
                    ..Signals::default()
                })
            ),
            FallReading::Holds
        );
    }
}

#[test]
fn the_dispatch_table_as_the_book_draws_it() {
    use Race::{
        ArchDarkElf as Ad, DarkElf as De, Elf as E, HighElf as He, HighOrc as Ho, Kijin as K,
        NamedSlime as Ns, Oni as On, Orc as O, Rimuru as R, Slime as S,
    };
    // Rows dispatch, columns are dispatched, in Race::ALL order:
    //            R  E  O  S  K  De He Ho Ns On Ad
    let table: [(Race, [u8; 11]); 11] = [
        (R, [0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1]),
        (E, [0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0]),
        (O, [0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0]),
        (S, [0; 11]),
        (K, [0; 11]),
        (De, [0; 11]),
        (He, [0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0]),
        (Ho, [0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0]),
        (Ns, [0; 11]),
        (On, [0; 11]),
        (Ad, [0; 11]),
    ];
    for (from, row) in table {
        for (to, &may) in Race::ALL.iter().zip(row.iter()) {
            let got = may_dispatch(from, *to, &ALL_TRUE).is_ok();
            assert_eq!(got, may == 1, "{from} → {to}");
        }
    }
}

#[test]
fn the_lane_route_still_needs_a_plan_item() {
    let other = DispatchContext {
        other_lane: true,
        plan_names_target: false,
        ..ALL_TRUE
    };
    assert_eq!(
        may_dispatch(Race::HighElf, Race::Orc, &other),
        Err(Refusal::NoPlanItem)
    );
}

#[test]
fn probation_recovers_or_not() {
    assert_eq!(RECOVER_REPORTS, 5);
    let back = |s: Signals| {
        Recent(Signals {
            reports: 5,
            findings: 5,
            ..s
        })
    };
    for (risen, ok, low) in [
        (
            Race::HighElf,
            Signals {
                plan_land_rate: 70.0,
                ..Signals::default()
            },
            Signals {
                plan_land_rate: 69.9,
                ..Signals::default()
            },
        ),
        (
            Race::HighOrc,
            Signals {
                gate_right: 75.0,
                ..Signals::default()
            },
            Signals {
                gate_right: 74.9,
                ..Signals::default()
            },
        ),
        (
            Race::NamedSlime,
            Signals {
                land_rate: 75.0,
                ..Signals::default()
            },
            Signals {
                land_rate: 74.9,
                ..Signals::default()
            },
        ),
        (
            Race::ArchDarkElf,
            Signals {
                precision: 70.0,
                ..Signals::default()
            },
            Signals {
                precision: 69.9,
                ..Signals::default()
            },
        ),
    ] {
        assert!(recovers(risen, &back(ok)), "{risen} back at its bar");
        assert!(!recovers(risen, &back(low)), "{risen} still under");
        assert!(
            !recovers(
                risen,
                &Recent(Signals {
                    reports: 4,
                    findings: 4,
                    ..ok
                })
            ),
            "{risen} too soon"
        );
    }
    let event = Recent(Signals {
        reports: 5,
        gate_right: 100.0,
        overturned_on_sealed: 1,
        ..Signals::default()
    });
    assert!(
        !recovers(Race::HighOrc, &event),
        "an event fells, it does not recover"
    );
    for race in [Race::Oni, Race::Rimuru, Race::Elf, Race::Slime] {
        assert!(
            !recovers(
                race,
                &back(Signals {
                    outward_clean_rate: 100.0,
                    ..Signals::default()
                })
            ),
            "{race} has no probation"
        );
    }
}
