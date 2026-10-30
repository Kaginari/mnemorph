//! The ascension lifecycle, explored whole: every state a body of each ascended form can reach
//! from its base race, breadth-first to a fixpoint over a small alphabet of readings and the
//! human's word, with the shortest path to each.
//!
//! The guards are the law's own functions — [`rises`], [`falls`], [`recovers`] — read on every
//! report; the moves between them are docs/02's diagram: a candidate is proposed and the human
//! confirms or declines it; a slipping body is proposed probation; a body on probation is proposed
//! its recovery or its fall. Nothing here applies a change of form without the human.
//!
//! The space is finite (probation counts to [`RECOVER_REPORTS`], a cooldown to
//! [`DECLINE_COOLDOWN_RUNS`]), so the walk runs to its fixpoint: no depth bound cuts it short.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mnemorph_races::{
    DECLINE_COOLDOWN_RUNS, FallReading, RECOVER_REPORTS, Race, Recent, Signals, falls, recovers,
    rises,
};

/// The forms a body moves through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Form {
    Base,
    Ascended,
    Probation,
}

/// What waits on the human.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Proposed {
    Nothing,
    Rise,
    Probation,
    Recover,
    Fall,
}

/// One body's standing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Body {
    form: Form,
    proposed: Proposed,
    /// Reports since probation began, counted to [`RECOVER_REPORTS`].
    since: u32,
    /// Runs a declined candidate still waits.
    cooldown: u32,
    /// It recovered from probation once.
    recovered: bool,
    /// It fell from an ascended form once.
    fallen: bool,
    /// The human declined it once.
    declined: bool,
}

const START: Body = Body {
    form: Form::Base,
    proposed: Proposed::Nothing,
    since: 0,
    cooldown: 0,
    recovered: false,
    fallen: false,
    declined: false,
};

/// A report's readings, as the fold would give them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reading {
    /// Every rate at its best over a full record: the rise bars.
    Strong,
    /// Rates between the fall bars and the rise bars.
    Fair,
    /// Rates under every fall bar, on a full record.
    Poor,
    /// Poor rates on too short a record to judge.
    Thin,
    /// Strong rates, and one event that ends an ascension at once.
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Report(Reading),
    /// The human confirms what is proposed.
    Confirm,
    /// The human declines what is proposed.
    Decline,
    /// A run passes.
    Run,
}

const ALPHABET: [Event; 8] = [
    Event::Report(Reading::Strong),
    Event::Report(Reading::Fair),
    Event::Report(Reading::Poor),
    Event::Report(Reading::Thin),
    Event::Report(Reading::Event),
    Event::Confirm,
    Event::Decline,
    Event::Run,
];

fn signals(r: Reading) -> Signals {
    let (rate, record) = match r {
        Reading::Strong | Reading::Event => (100.0, 30),
        Reading::Fair => (80.0, 30),
        Reading::Poor => (60.0, 30),
        Reading::Thin => (60.0, 3),
    };
    let event = u32::from(r == Reading::Event);
    Signals {
        reports: record,
        findings: record,
        outward_acts: record,
        rollbacks: event,
        envelope_corrections: event,
        overturned_on_sealed: event,
        tests_intact_failures: event,
        ask_done_rate: rate,
        plan_land_rate: rate,
        reask_rate: 0.0,
        land_rate: rate,
        gate_right: rate,
        slime_health: rate,
        gate_rate: rate,
        hole_rate: 0.0,
        outward_clean_rate: rate,
        verify_green_rate: rate,
        upheld_rate: rate,
        precision: rate,
    }
}

/// One move of `risen`'s lifecycle, its guards the law's; every invariant asserted on the way.
fn step(risen: Race, was: Body, e: Event) -> Body {
    let mut now = was;
    match e {
        Event::Report(r) => {
            let s = signals(r);
            // Hysteresis: a reading at the rise bars never reads as falling.
            if rises(risen, &s) {
                assert_eq!(
                    falls(risen, &Recent(s)),
                    FallReading::Holds,
                    "{risen}: {r:?} rises and falls at once"
                );
            }
            match was.form {
                Form::Base => {
                    if was.proposed == Proposed::Nothing && was.cooldown == 0 && rises(risen, &s) {
                        now.proposed = Proposed::Rise;
                    }
                }
                Form::Ascended => match falls(risen, &Recent(s)) {
                    FallReading::Immediate => now.proposed = Proposed::Fall,
                    FallReading::Probation if was.proposed != Proposed::Fall => {
                        now.proposed = Proposed::Probation;
                    }
                    _ => {}
                },
                Form::Probation => {
                    now.since = (was.since + 1).min(RECOVER_REPORTS);
                    // The record since probation began is this many reports.
                    let since = Recent(Signals {
                        reports: now.since,
                        findings: now.since,
                        ..s
                    });
                    if falls(risen, &Recent(s)) == FallReading::Immediate {
                        now.proposed = Proposed::Fall;
                    } else if recovers(risen, &since) {
                        assert!(
                            now.since >= RECOVER_REPORTS,
                            "{risen}: recovered on {} reports",
                            now.since
                        );
                        now.proposed = Proposed::Recover;
                    } else if now.since >= RECOVER_REPORTS {
                        now.proposed = Proposed::Fall;
                    }
                }
            }
        }
        Event::Confirm => {
            now.proposed = Proposed::Nothing;
            match (was.form, was.proposed) {
                (Form::Base, Proposed::Rise) => now.form = Form::Ascended,
                (Form::Ascended, Proposed::Probation) => {
                    now.form = Form::Probation;
                    now.since = 0;
                }
                (Form::Probation, Proposed::Recover) => {
                    now.form = Form::Ascended;
                    now.since = 0;
                    now.recovered = true;
                }
                (Form::Ascended | Form::Probation, Proposed::Fall) => {
                    now.form = Form::Base;
                    now.since = 0;
                    now.fallen = true;
                }
                (_, Proposed::Nothing) => {}
                (form, p) => panic!("{risen}: {p:?} proposed to a body in {form:?}"),
            }
        }
        Event::Decline => {
            if was.proposed == Proposed::Rise {
                now.cooldown = DECLINE_COOLDOWN_RUNS;
                now.declined = true;
            }
            now.proposed = Proposed::Nothing;
        }
        Event::Run => now.cooldown = was.cooldown.saturating_sub(1),
    }
    // The form changes on the human's word alone.
    if e != Event::Confirm {
        assert_eq!(
            now.form, was.form,
            "{risen}: {e:?} changed the form of {was:?}"
        );
    }
    // No rise is proposed while a declined candidate waits.
    if now.proposed == Proposed::Rise {
        assert_eq!(now.cooldown, 0, "{risen}: proposed during its cooldown");
    }
    now
}

/// Breadth-first from [`START`] to a fixpoint: each state with the shortest path to it.
fn explore(risen: Race) -> BTreeMap<Body, Vec<Event>> {
    let mut seen: BTreeMap<Body, Vec<Event>> = BTreeMap::from([(START, Vec::new())]);
    let mut queue = VecDeque::from([START]);
    while let Some(b) = queue.pop_front() {
        for e in ALPHABET {
            let n = step(risen, b, e);
            if !seen.contains_key(&n) {
                let mut path = seen[&b].clone();
                path.push(e);
                seen.insert(n, path);
                queue.push_back(n);
            }
        }
    }
    seen
}

const ASCENDED: [Race; 5] = [
    Race::HighElf,
    Race::HighOrc,
    Race::NamedSlime,
    Race::Oni,
    Race::ArchDarkElf,
];

#[test]
fn every_ascended_form_reaches_what_it_should_and_nothing_else() {
    use Form as F;
    use Proposed as P;
    for risen in ASCENDED {
        assert!(risen.is_ascended());
        let base = risen.base();
        assert_eq!(base.ascends_to(), Some(risen));
        let states = explore(risen);
        // Rate falls take probation; events fall at once. An Oni falls only by an event, an Elf
        // and a Dark Elf only by their rates.
        let rates = risen != Race::Oni;
        let events = matches!(risen, Race::HighOrc | Race::NamedSlime | Race::Oni);
        let mut intended: BTreeSet<(Form, Proposed)> = [
            (F::Base, P::Nothing),
            (F::Base, P::Rise),
            (F::Ascended, P::Nothing),
        ]
        .into();
        if rates {
            intended.extend([
                (F::Ascended, P::Probation),
                (F::Probation, P::Nothing),
                (F::Probation, P::Recover),
                (F::Probation, P::Fall),
            ]);
        }
        if events {
            intended.insert((F::Ascended, P::Fall));
            if rates {
                // An event on a body on probation.
                intended.insert((F::Probation, P::Fall));
            }
        }
        let reached: BTreeSet<(Form, Proposed)> =
            states.keys().map(|b| (b.form, b.proposed)).collect();
        for want in &intended {
            let witness = states
                .iter()
                .find(|(b, _)| (b.form, b.proposed) == *want)
                .map(|(_, p)| p);
            assert!(witness.is_some(), "{risen}: {want:?} is unreachable");
        }
        let unintended: Vec<_> = reached.difference(&intended).collect();
        assert!(
            unintended.is_empty(),
            "{risen}: unintended states reached: {:?}",
            unintended
                .iter()
                .map(|u| (
                    u,
                    &states[states.keys().find(|b| (b.form, b.proposed) == **u).unwrap()]
                ))
                .collect::<Vec<_>>()
        );
        // Witnesses that the walk is not vacuous: a fall can happen and leave the record, a fall on
        // rates can recover, a declined candidate is proposed again once its cooldown has passed.
        let fell = states.iter().find(|(b, _)| b.fallen && b.form == F::Base);
        assert!(fell.is_some(), "{risen}: no fall reachable");
        let recovered = states.keys().any(|b| b.recovered && b.form == F::Ascended);
        assert_eq!(recovered, rates, "{risen}: recovery");
        let again = states
            .iter()
            .find(|(b, _)| b.proposed == P::Rise && b.declined && !b.fallen);
        let (_, path) = again.unwrap_or_else(|| panic!("{risen}: never proposed again"));
        let runs = path.iter().filter(|e| **e == Event::Run).count();
        assert!(
            runs >= DECLINE_COOLDOWN_RUNS as usize,
            "{risen}: proposed again after {runs} runs"
        );
    }
}

#[test]
fn the_shortest_paths_read_as_the_lifecycle_does() {
    use Event::{Confirm, Report};
    let states = explore(Race::HighOrc);
    let path = |form, proposed| {
        states
            .iter()
            .find(|(b, _)| b.form == form && b.proposed == proposed && !b.fallen && !b.declined)
            .map(|(_, p)| p.clone())
            .unwrap()
    };
    // A rise is one strong report and the human's confirmation.
    assert_eq!(
        path(Form::Ascended, Proposed::Nothing),
        [Report(Reading::Strong), Confirm]
    );
    // Probation is proposed on a poor full record, and applied only on the human's word.
    assert_eq!(
        path(Form::Probation, Proposed::Nothing),
        [
            Report(Reading::Strong),
            Confirm,
            Report(Reading::Poor),
            Confirm
        ]
    );
    // Recovery takes the full five reports since probation began.
    let recover = path(Form::Probation, Proposed::Recover);
    assert_eq!(recover.len(), 4 + RECOVER_REPORTS as usize, "{recover:?}");
    // Every state of every form, counted: the walk is bounded and whole.
    for risen in ASCENDED {
        let n = explore(risen).len();
        println!("{risen}: {n} states");
        assert!((10..2000).contains(&n), "{risen}: {n} states");
    }
}
