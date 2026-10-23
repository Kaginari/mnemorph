//! M2's exit: a tree of commissions, sequential and parallel, never exceeds any envelope, and
//! the folds match numbers computed by hand.

use proptest::prelude::*;
use tempest_ledger::{Envelope, Item, Key, Timestamp, Usage, UsageRow, fold, wt};
use tempest_races::Race;

/// One step of a random commission tree.
#[derive(Debug, Clone)]
enum Step {
    /// The body spends a call's worth, through pre-flight.
    Spend(u64),
    /// The body carves a plan of items for children of these races and weights, then runs each
    /// child's own steps (in parallel: all slices are carved before any child spends).
    Plan(Vec<(Race, u32, Vec<Step>)>),
    /// The session draws an automatic challenge and spends in it.
    Challenge(u64),
}

fn child_race(parent: Race) -> Race {
    match parent {
        Race::Rimuru => Race::Elf,
        Race::Elf => Race::Orc,
        _ => Race::Slime,
    }
}

fn steps(depth: u32) -> BoxedStrategy<Vec<Step>> {
    let leaf = prop_oneof![
        (1u64..20_000).prop_map(Step::Spend),
        (1u64..5_000).prop_map(Step::Challenge),
    ];
    if depth == 0 {
        return prop::collection::vec(leaf, 0..4).boxed();
    }
    let plan = prop::collection::vec((1u32..5, steps(depth - 1)), 1..4).prop_map(|items| {
        Step::Plan(
            items
                .into_iter()
                .map(|(w, s)| (Race::Slime, w, s))
                .collect(),
        )
    });
    prop::collection::vec(prop_oneof![3 => leaf, 1 => plan], 0..5).boxed()
}

/// Runs a body's steps; returns how many commissions ran under it. Asserts on the way.
fn run(
    env: &mut Envelope,
    race: Race,
    steps: &[Step],
    session: &mut Option<&mut Envelope>,
    count: &mut u32,
) {
    for step in steps {
        match step {
            Step::Spend(n) => {
                if env.preflight(*n).is_ok() {
                    env.record(*n);
                }
            }
            Step::Challenge(n) => {
                if let Some(session) = session.as_deref_mut()
                    && let Some(slice) = session.challenge()
                {
                    let mut child = Envelope::child(slice);
                    if child.preflight(*n).is_ok() {
                        child.record(*n);
                    }
                    assert!(!child.overspent());
                    session.settle(slice, child.spent());
                }
            }
            Step::Plan(items) => {
                if *count >= 50 {
                    continue;
                }
                let kid = child_race(race);
                let left_before = env.left();
                let plan: Vec<Item> = items
                    .iter()
                    .map(|(_, w, _)| Item {
                        race: kid,
                        weight: *w,
                    })
                    .collect();
                let slices = env.carve(&plan);
                let carved: u64 = slices.iter().map(|s| s.of).sum();
                assert!(carved <= left_before, "carved {carved} of {left_before}");
                if left_before >= 1_000 {
                    assert!(
                        slices.iter().all(|s| s.of > 0),
                        "a weighted item starved: {slices:?}"
                    );
                }
                // Parallel: every slice is out before any child spends.
                let mut children: Vec<Envelope> =
                    slices.iter().map(|s| Envelope::child(*s)).collect();
                for (child, (_, _, sub)) in children.iter_mut().zip(items) {
                    *count += 1;
                    run(child, kid, sub, &mut None, count);
                    assert!(!child.overspent(), "a child spent past its slice");
                }
                for (slice, child) in slices.into_iter().zip(children) {
                    env.settle(slice, child.spent());
                }
                assert!(!env.overspent());
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn no_envelope_is_ever_exceeded(of in 0u64..2_000_000, tree in steps(3)) {
        let mut session = Envelope::session(of);
        let mut count = 0;
        let mut work = session.clone();
        // The session's own steps run on the session envelope; challenges draw on its reserve.
        run(&mut work, Race::Rimuru, &tree, &mut Some(&mut session), &mut count);
        prop_assert!(!work.overspent());
        prop_assert!(work.spent() <= of);
        prop_assert!(!session.overspent());
        prop_assert!(count <= 52, "{count} commissions");
    }
}

fn row(
    day: i64,
    body: &str,
    race: &str,
    model: &str,
    usage: Usage,
    usd: Option<f64>,
    prefix: u64,
) -> UsageRow {
    UsageRow {
        ts: Timestamp(1_792_800_000 + day * 86_400),
        session: "s".into(),
        run: "r".into(),
        body: body.into(),
        race: race.into(),
        commission: None,
        model: model.into(),
        provider: "p".into(),
        usage,
        prefix,
        wt: wt(usage, 5.0),
        usd,
        envelope: None,
    }
}

#[test]
fn folds_match_the_hand_count() {
    // An Anthropic-shaped call (cache classes reported) and an OpenAI-shaped one (cache reads
    // only): both kinds of cache tokens must survive the fold.
    let anthropic = Usage {
        input: 200,
        output: 100,
        cache_read: 5_000,
        cache_write: 300,
    };
    let openai = Usage {
        input: 1_000,
        output: 50,
        cache_read: 4_000,
        cache_write: 0,
    };
    let rows = [
        row(0, "slime-wire", "slime", "a", anthropic, Some(0.01), 4_000),
        row(0, "slime-wire", "slime", "b", openai, None, 4_000),
        row(1, "orc-law", "orc", "a", anthropic, Some(0.01), 4_500),
    ];
    let by_body = fold(&rows, Key::Body);
    let wire = by_body["slime-wire"];
    assert_eq!(wire.calls, 2);
    assert_eq!(
        wire.usage,
        Usage {
            input: 1_200,
            output: 150,
            cache_read: 9_000,
            cache_write: 300
        }
    );
    assert_eq!(wire.wt, wt(anthropic, 5.0) + wt(openai, 5.0));
    assert!((wire.usd - 0.01).abs() < 1e-12);
    assert_eq!(
        wire.unpriced, 1,
        "the unpriced call is counted, not priced at zero"
    );
    assert!((wire.cache_hit_rate() - 9_000.0 / 10_500.0).abs() < 1e-12);
    assert!((wire.prefix_share() - 8_000.0 / 10_500.0).abs() < 1e-12);

    let by_race = fold(&rows, Key::Race);
    assert_eq!(by_race.keys().collect::<Vec<_>>(), ["orc", "slime"]);
    let by_model = fold(&rows, Key::Model);
    assert_eq!(by_model["a"].calls, 2);
    let by_day = fold(&rows, Key::Day);
    assert_eq!(by_day.len(), 2);
    assert_eq!(by_day.values().map(|t| t.calls).sum::<u64>(), 3);
    assert!((tempest_ledger::Totals::default().cache_hit_rate()).abs() < 1e-12);
    assert!((tempest_ledger::Totals::default().prefix_share()).abs() < 1e-12);
}
