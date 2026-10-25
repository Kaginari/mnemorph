//! Metis against scripted deciders: a commission routed within its office's band, only on a sure
//! answer, and never past a model the human chose.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mnemorph_config::{Config, Paths};
use mnemorph_ledger::{DecisionRow, Journal, Usage, UsageRow};
use mnemorph_loop::{
    Decisions, Interrupt, Meter, Reason, Stop, Work, band, route, route_ask, tier_of,
};
use mnemorph_provider::{
    ANSWER_TOOL, Block, Caller, Complete, Completion, Cutoffs, Decider, Error, Fallback, Primary,
    Reply, Request, Response, Scripted, StopReason, SystemOne, anthropic_prices,
};
use mnemorph_races::{ModelTier, Race};
use mnemorph_wire::Office;
use serde_json::json;

struct Ground {
    root: PathBuf,
}

impl Ground {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-route-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world")).unwrap();
        Self { root }
    }

    fn config(&self, text: &str) -> Config {
        let global = self.root.join("global.toml");
        fs::write(&global, text).unwrap();
        Config::load(&Paths::with_global(global, &self.root.join("world"))).unwrap()
    }

    fn journal(&self) -> Journal<DecisionRow> {
        Journal::new(self.root.join("decisions.jsonl"))
    }

    fn usage(&self) -> Journal<UsageRow> {
        Journal::new(self.root.join("usage.jsonl"))
    }
}

impl Drop for Ground {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const TIERS: &str = "[models.tier]\nfrontier = \"big\"\nstrong = \"mid\"\nfast = \"small\"\n";

fn caller(body: &str) -> Caller {
    Caller {
        session: "s".into(),
        run: "r".into(),
        body: body.into(),
        race: "orc".into(),
        commission: Some("c1".into()),
        prefix: 0,
    }
}

/// A System One answer: `demand` as a distribution over the three levels, `irreversible` a
/// probability.
fn measured(demand: [f64; 3], irreversible: f64) -> Reply {
    Reply::ok(
        &json!({"model": "jev-1.14.0", "answers": {
            "demand": {"type": "score", "probabilities": {"0": demand[0], "1": demand[1], "2": demand[2]}},
            "irreversible": {"type": "noul", "noul": irreversible},
        }})
        .to_string(),
    )
}

#[derive(Default)]
struct Llm {
    replies: Mutex<Vec<Result<Response, Error>>>,
}

impl Complete for Llm {
    fn complete<'a>(&'a self, _: &'a Request) -> Completion<'a> {
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| Err(Error::Invalid("no fallback expected".into())));
        Box::pin(async move { reply })
    }
}

fn deciders(
    ground: &Ground,
    replies: Vec<Reply>,
    llm: Llm,
    on: bool,
) -> (Arc<Scripted>, Decisions) {
    let wire = Arc::new(Scripted::new(replies));
    let primary = Some(Primary {
        client: SystemOne::new(
            wire.clone(),
            "https://api.typesafe.ai",
            None,
            SystemOne::policy(),
        ),
        provider: "jev".into(),
        model: "jev-latest".into(),
    });
    let dec = Decisions {
        decider: Arc::new(Decider {
            primary,
            fallback: Fallback {
                client: Arc::new(llm),
                provider: "anthropic".into(),
                model: "claude-haiku-5-5".into(),
            },
        }),
        cutoffs: Cutoffs::default(),
        journal: ground.journal(),
        bench: false,
        route: on,
    };
    (wire, dec)
}

fn work(race: Race, office: Option<Office>) -> Work<'static> {
    Work {
        race,
        office,
        commission: "the commission",
        ceiling: None,
    }
}

/// Routes with no stop, the usage ledger of `ground` as the meter.
async fn routed(
    config: &Config,
    dec: Option<&Decisions>,
    ground: &Ground,
    w: Work<'_>,
) -> mnemorph_loop::Routed {
    let prices = anthropic_prices().unwrap();
    let ledger = ground.usage();
    route(
        config,
        dec,
        &caller("body"),
        w,
        None,
        Meter {
            prices: &prices,
            ledger: Some(&ledger),
        },
    )
    .await
}

#[test]
fn each_office_has_its_band() {
    use ModelTier::{Fast, Frontier, Strong};
    let cases = [
        (
            Race::Slime,
            Some(Office::Findings),
            (Fast, Strong, Frontier),
            "Mneme may run fast",
        ),
        (
            Race::Orc,
            Some(Office::Verdict),
            (Strong, Strong, Frontier),
            "Melete never below strong",
        ),
        (
            Race::Slime,
            Some(Office::Draft),
            (Strong, Strong, Frontier),
            "Aoede never below strong",
        ),
        (
            Race::Elf,
            None,
            (Frontier, Frontier, Frontier),
            "no office: the race's own tier",
        ),
        (
            Race::DarkElf,
            Some(Office::Findings),
            (Frontier, Frontier, Frontier),
            "the adversary",
        ),
        (
            Race::ArchDarkElf,
            Some(Office::Findings),
            (Frontier, Frontier, Frontier),
            "ascended too",
        ),
        (
            Race::Rimuru,
            Some(Office::Findings),
            (Frontier, Frontier, Frontier),
            "the throne",
        ),
        (
            Race::HighOrc,
            Some(Office::Verdict),
            (Strong, Strong, Frontier),
            "a High Orc is an Orc",
        ),
        (
            Race::Oni,
            Some(Office::Findings),
            (Fast, Strong, Frontier),
            "an Oni is a Kijin",
        ),
    ];
    for (race, office, want, why) in cases {
        assert_eq!(band(race, office), want, "{why}");
    }
}

#[tokio::test]
async fn a_model_the_human_chose_is_never_routed() {
    let ground = Ground::new("pinned");
    let config = ground.config(&format!(
        "{TIERS}[models.office.orc]\nmelete = \"chosen\"\n[decide]\nroute = true\n"
    ));
    let (wire, dec) = deciders(&ground, vec![], Llm::default(), true);
    let r = routed(
        &config,
        Some(&dec),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!((r.model.as_str(), r.reason), ("chosen", Reason::Pinned));
    assert_eq!(wire.sent(), Vec::new(), "nothing is asked");

    let config = ground.config(&format!(
        "{TIERS}[models.office.orc]\njudge = \"by-function\"\n"
    ));
    let r = routed(
        &config,
        Some(&dec),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!(
        r.model, "by-function",
        "the function word names the same office"
    );
}

#[tokio::test]
async fn with_routing_off_or_no_room_in_the_band_nothing_is_asked() {
    let ground = Ground::new("off");
    let config = ground.config(TIERS);
    let (wire, dec) = deciders(&ground, vec![], Llm::default(), false);
    let r = routed(
        &config,
        Some(&dec),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!((r.model.as_str(), r.reason), ("mid", Reason::Off));
    let r = routed(
        &config,
        None,
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!(r.reason, Reason::Off);
    // The throne and the Dark Elf have nowhere to go: Metis is not asked, nothing is spent.
    let (wire_on, on) = deciders(&ground, vec![], Llm::default(), true);
    for race in [Race::Rimuru, Race::DarkElf] {
        let r = routed(
            &config,
            Some(&on),
            &ground,
            work(race, Some(Office::Findings)),
        )
        .await;
        assert_eq!(
            (r.model.as_str(), r.reason),
            ("big", Reason::Off),
            "{race:?}"
        );
    }
    assert_eq!(wire.sent(), Vec::new());
    assert_eq!(wire_on.sent(), Vec::new());
}

#[tokio::test]
async fn a_sure_reading_moves_the_body_within_its_band_and_no_further() {
    let ground = Ground::new("band");
    let config = ground.config(TIERS);
    let cases = [
        // Hard work for Melete rises to frontier.
        (
            Race::Orc,
            Some(Office::Verdict),
            [0.0, 0.05, 0.95],
            0.1,
            "big",
        ),
        // Routine, surely safe reading for Mneme drops to fast.
        (
            Race::Slime,
            Some(Office::Findings),
            [0.95, 0.05, 0.0],
            0.05,
            "small",
        ),
        // Routine, surely safe judging for Melete stops at its floor, strong.
        (
            Race::Orc,
            Some(Office::Verdict),
            [0.95, 0.05, 0.0],
            0.05,
            "mid",
        ),
        // Routine but hard to undo: the ceiling.
        (
            Race::Slime,
            Some(Office::Draft),
            [0.95, 0.05, 0.0],
            0.95,
            "big",
        ),
        // Surely involved: the default.
        (
            Race::Slime,
            Some(Office::Findings),
            [0.05, 0.95, 0.0],
            0.05,
            "mid",
        ),
    ];
    for (race, office, demand, irreversible, want) in cases {
        let (wire, dec) = deciders(
            &ground,
            vec![measured(demand, irreversible)],
            Llm::default(),
            true,
        );
        let r = routed(&config, Some(&dec), &ground, work(race, office)).await;
        assert_eq!(
            r.model, want,
            "{race:?} {office:?} {demand:?} {irreversible}"
        );
        assert_eq!(r.reason, Reason::Decided);
        assert_eq!(wire.sent().len(), 1);
    }
    let rows = ground.journal().read().unwrap().rows;
    assert_eq!(rows.len(), 5);
    assert!(
        rows.iter()
            .all(|r| r.point == "model.route" && r.basis == "measured")
    );
    let spent = ground.usage().read().unwrap().rows;
    assert_eq!(
        spent.iter().filter(|r| r.provider == "jev").count(),
        5,
        "each route is priced"
    );
}

/// Lesson: a sure "routine" beside an unresolved risk lowered a body. Lowering needs both: surely
/// routine, and surely not hard to undo.
#[tokio::test]
async fn an_unresolved_risk_never_lowers_a_body() {
    let ground = Ground::new("risk");
    let config = ground.config(TIERS);
    // 74% routine is sure (confidence 0.61); 79% hard-to-undo is not (0.58).
    let (_, dec) = deciders(
        &ground,
        vec![measured([0.74, 0.0, 0.26], 0.79)],
        Llm::default(),
        true,
    );
    let r = routed(
        &config,
        Some(&dec),
        &ground,
        work(Race::Slime, Some(Office::Findings)),
    )
    .await;
    assert_eq!((r.model.as_str(), r.reason), ("mid", Reason::Unsure));
    // Measured, and unsure on both: the default.
    let (_, dec) = deciders(
        &ground,
        vec![measured([0.4, 0.3, 0.3], 0.5)],
        Llm::default(),
        true,
    );
    let r = routed(
        &config,
        Some(&dec),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!((r.model.as_str(), r.reason), ("mid", Reason::Unsure));
}

#[tokio::test]
async fn a_tier_with_no_model_is_never_ridden() {
    let ground = Ground::new("unset");
    let config = ground.config("[models.tier]\nfrontier = \"\"\nstrong = \"mid\"\n");
    let (_, dec) = deciders(
        &ground,
        vec![measured([0.0, 0.05, 0.95], 0.1)],
        Llm::default(),
        true,
    );
    let r = routed(
        &config,
        Some(&dec),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!(r.model, "mid");
    assert_eq!(r.reason, Reason::Unconfigured(ModelTier::Frontier));
}

#[tokio::test]
async fn an_unsure_stated_or_failed_route_keeps_the_default_tier() {
    let ground = Ground::new("unsure");
    let config = ground.config(TIERS);
    // An LLM's stated 80% "hard" is under its cut-off of 0.9 confidence.
    let stated = Ok(Response {
        id: "d".into(),
        model: "claude-haiku-5-5".into(),
        content: vec![Block::ToolUse {
            id: "a".into(),
            name: ANSWER_TOOL.into(),
            input: json!({"answers": {"demand": {"0": 0.05, "1": 0.15, "2": 0.8}, "irreversible": 0.2}}),
        }],
        stop: StopReason::ToolUse,
        stop_details: None,
        usage: Usage {
            input: 200,
            output: 30,
            cache_read: 0,
            cache_write: 0,
        },
        warnings: vec![],
    });
    let (_, dec) = deciders(
        &ground,
        vec![],
        Llm {
            replies: Mutex::new(vec![stated]),
        },
        true,
    );
    let dec = Decisions {
        decider: Arc::new(Decider {
            primary: None,
            fallback: Fallback {
                client: Arc::clone(&dec.decider.fallback.client),
                provider: "anthropic".into(),
                model: "claude-haiku-5-5".into(),
            },
        }),
        ..dec
    };
    let r = routed(
        &config,
        Some(&dec),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!(
        (r.model.as_str(), r.reason.clone()),
        ("mid", Reason::Unsure)
    );
    assert_eq!(
        r.decision.unwrap().source.basis,
        mnemorph_provider::Basis::Stated
    );

    // The System One fails and so does the fallback: the default stands, and says why.
    let (_, dec) = deciders(
        &ground,
        vec![Reply::status(401, "{}", &[])],
        Llm::default(),
        true,
    );
    let r = routed(
        &config,
        Some(&dec),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!(r.model, "mid");
    assert!(
        matches!(r.reason, Reason::Failed(Error::Invalid(_))),
        "{:?}",
        r.reason
    );
}

#[tokio::test]
async fn an_esc_stops_a_route_and_the_default_stands() {
    let ground = Ground::new("esc");
    let config = ground.config(TIERS);
    let (_, dec) = deciders(
        &ground,
        vec![Reply::Silent(std::time::Duration::from_secs(20))],
        Llm::default(),
        true,
    );
    let interrupt = Interrupt::default();
    interrupt.fire();
    let prices = anthropic_prices().unwrap();
    let started = std::time::Instant::now();
    let r = route(
        &config,
        Some(&dec),
        &caller("body"),
        work(Race::Orc, Some(Office::Verdict)),
        Some(Stop {
            interrupt: &interrupt,
            left: std::time::Duration::from_secs(60),
        }),
        Meter {
            prices: &prices,
            ledger: None,
        },
    )
    .await;
    assert_eq!((r.model.as_str(), r.reason), ("mid", Reason::Interrupted));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn a_route_reads_the_commissions_head_and_never_splits_a_character() {
    let long = format!("{}{}", "é".repeat(4_001), "tail");
    let ask = route_ask(Race::Orc, Some(Office::Verdict), &long);
    assert!(ask.state.starts_with("For melete (judge), a orc body:\n"));
    assert!(!ask.state.ends_with("tail"));
    assert!(ask.state.len() < 8_100);
}

/// One model for everything leaves no room in any band: nothing is asked, nothing is spent. A
/// race's own model is its default, and a sure reading still moves the body off it.
#[tokio::test]
async fn one_model_leaves_no_room_and_a_race_model_is_a_default() {
    let ground = Ground::new("one-model");
    let config = ground.config("[models]\ndefault = \"only\"\n");
    let (wire, on) = deciders(&ground, vec![], Llm::default(), true);
    let r = routed(
        &config,
        Some(&on),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!((r.model.as_str(), r.reason), ("only", Reason::Off));
    assert_eq!(wire.sent(), Vec::new(), "no room, no question");

    let config = ground.config(&format!("{TIERS}[models.race]\norc = \"orc-model\"\n"));
    let (wire, on) = deciders(
        &ground,
        vec![measured([0.02, 0.03, 0.95], 0.9)],
        Llm::default(),
        true,
    );
    let r = routed(
        &config,
        Some(&on),
        &ground,
        work(Race::Orc, Some(Office::Verdict)),
    )
    .await;
    assert_eq!(wire.sent().len(), 1, "a race's model is routed, not pinned");
    assert_eq!(
        r.model, "big",
        "a hard, irreversible commission rides the band's ceiling"
    );
}

/// A body is never routed above the body that commissioned it: hard work that would rise to
/// frontier stops at the parent's tier, and a parent below the child's default holds the default
/// down too.
#[tokio::test]
async fn a_body_is_never_routed_above_its_parent() {
    let ground = Ground::new("ceiling");
    let config = ground.config(TIERS);
    for (ceiling, want) in [(ModelTier::Strong, "mid"), (ModelTier::Fast, "small")] {
        let (_, dec) = deciders(
            &ground,
            vec![measured([0.0, 0.05, 0.95], 0.1)],
            Llm::default(),
            true,
        );
        let w = Work {
            ceiling: Some(ceiling),
            ..work(Race::Orc, Some(Office::Verdict))
        };
        let r = routed(&config, Some(&dec), &ground, w).await;
        assert_eq!(r.model, want, "{ceiling:?}");
    }
    assert_eq!(tier_of(&config, Race::Orc, "mid"), Some(ModelTier::Strong));
    // A model outside the tier table — a human's pin — caps nothing.
    assert_eq!(tier_of(&config, Race::Orc, "pinned-by-hand"), None);
    let (_, dec) = deciders(
        &ground,
        vec![measured([0.0, 0.05, 0.95], 0.1)],
        Llm::default(),
        true,
    );
    let free = Work {
        ceiling: tier_of(&config, Race::Elf, "pinned-by-hand"),
        ..work(Race::Orc, Some(Office::Verdict))
    };
    assert_eq!(
        routed(&config, Some(&dec), &ground, free).await.model,
        "big"
    );
}
