//! M2's exit: a tree of commissions, sequential and parallel, never exceeds any envelope, and
//! the folds match numbers computed by hand.

use mnemorph_ledger::{Envelope, Item, Key, Slice, Timestamp, Usage, UsageRow, fold, wt};
use mnemorph_races::Race;
use proptest::prelude::*;

/// One step of a random commission tree.
#[derive(Debug, Clone)]
enum Step {
    /// The body spends a call's worth, through pre-flight.
    Spend(u64),
    /// A call that cost more than its estimate: recorded without pre-flight.
    Overspend(u64),
    /// The body carves a plan, then runs each child's own steps — in parallel: every slice is
    /// carved before any child spends. Each item: (which commanded race, weight, child steps).
    Plan(Vec<(usize, u32, Vec<Step>)>),
    /// An automatic challenge (only the session holds a reserve; elsewhere it draws nothing).
    Challenge(u64),
}

fn steps(depth: u32) -> BoxedStrategy<Vec<Step>> {
    let leaf = prop_oneof![
        6 => (1u64..20_000).prop_map(Step::Spend),
        1 => (1u64..50_000).prop_map(Step::Overspend),
        2 => (1u64..5_000).prop_map(Step::Challenge),
    ];
    if depth == 0 {
        return prop::collection::vec(leaf, 0..4).boxed();
    }
    let plan =
        prop::collection::vec((0usize..6, 0u32..5, steps(depth - 1)), 1..4).prop_map(Step::Plan);
    prop::collection::vec(prop_oneof![3 => leaf, 1 => plan], 0..5).boxed()
}

/// The books kept beside the envelopes, to check them.
#[derive(Default)]
struct Witness {
    recorded: u64,
    overspends: u32,
    commissions: u32,
    races: std::collections::BTreeSet<Race>,
}

fn run(env: &mut Envelope, race: Race, steps: &[Step], w: &mut Witness) {
    for step in steps {
        match step {
            Step::Spend(n) => {
                if env.preflight(*n).is_ok() {
                    env.record(*n);
                    w.recorded += n;
                }
            }
            Step::Overspend(n) => {
                env.record(*n);
                w.recorded += n;
                w.overspends += 1;
            }
            Step::Challenge(n) => {
                if let Some(slice) = env.challenge() {
                    let mut child = Envelope::child(&slice);
                    if child.preflight(*n).is_ok() {
                        child.record(*n);
                        w.recorded += n;
                    }
                    env.settle(slice, child.spent())
                        .expect("its own open slice");
                }
            }
            Step::Plan(items) => {
                let kids = race.commands();
                if kids.is_empty() || w.commissions >= 50 {
                    continue;
                }
                let left_before = env.left();
                let plan: Vec<Item> = items
                    .iter()
                    .map(|(k, weight, _)| Item {
                        race: kids[k % kids.len()],
                        weight: *weight,
                    })
                    .collect();
                let slices = env.carve(&plan);
                let carved: u64 = slices.iter().map(Slice::of).sum();
                assert!(
                    carved * 100 <= left_before * 90 + 100,
                    "carved {carved} of {left_before}"
                );
                for (slice, item) in slices.iter().zip(&plan) {
                    assert!(
                        slice.of() * 100 <= left_before * u64::from(item.race.max_share()),
                        "{item:?} over its ceiling"
                    );
                    if item.weight == 0 {
                        assert_eq!(slice.of(), 0, "weight 0 gets nothing");
                    } else if left_before >= 10_000 {
                        assert!(slice.of() > 0, "a weighted item starved: {item:?}");
                    }
                    w.races.insert(item.race);
                }
                let mut children: Vec<Envelope> = slices.iter().map(Envelope::child).collect();
                for (child, ((_, _, sub), item)) in children.iter_mut().zip(items.iter().zip(&plan))
                {
                    w.commissions += 1;
                    let before = child.left();
                    run(child, item.race, sub, w);
                    assert!(
                        child.challenge().is_none() || before == 0,
                        "a child holds no reserve"
                    );
                }
                for (slice, child) in slices.into_iter().zip(children) {
                    env.settle(slice, child.spent())
                        .expect("its own open slice");
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn the_books_balance_and_hold(of in 0u64..2_000_000, tree in steps(3)) {
        let mut session = Envelope::session(of);
        let mut w = Witness::default();
        run(&mut session, Race::Rimuru, &tree, &mut w);
        // Every weighted token recorded anywhere reaches the session, once.
        prop_assert_eq!(session.spent(), w.recorded);
        if w.overspends == 0 {
            prop_assert!(session.spent() <= of, "{} of {}", session.spent(), of);
            prop_assert!(!session.overspent());
        }
        prop_assert_eq!(session.left() + session.spent() <= of || w.overspends > 0, true);
        prop_assert!(w.commissions <= 52, "{} commissions", w.commissions);
    }
}

#[test]
fn the_generator_reaches_every_ceiling() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..200 {
        let tree = steps(3).new_tree(&mut runner).unwrap().current();
        let mut w = Witness::default();
        run(
            &mut Envelope::session(2_000_000),
            Race::Rimuru,
            &tree,
            &mut w,
        );
        seen.extend(w.races);
    }
    for race in [
        Race::Elf,
        Race::Kijin,
        Race::DarkElf,
        Race::Orc,
        Race::Slime,
    ] {
        assert!(seen.contains(&race), "{race} is never carved for: {seen:?}");
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
        reported_cost: None,
        envelope: None,
        purpose: None,
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
    assert!((mnemorph_ledger::Totals::default().cache_hit_rate()).abs() < 1e-12);
    assert!((mnemorph_ledger::Totals::default().prefix_share()).abs() < 1e-12);
}
