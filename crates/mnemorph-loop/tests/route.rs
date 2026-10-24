//! Metis against scripted deciders: a commission routed within its office's band, only on a sure
//! answer, and never past a model the human chose.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mnemorph_config::{Config, Paths};
use mnemorph_ledger::{DecisionRow, Journal, Usage};
use mnemorph_loop::{Decisions, Reason, band, route};
use mnemorph_provider::{
    ANSWER_TOOL, Block, Caller, Complete, Completion, Cutoffs, Decider, Error, Fallback, Primary,
    Reply, Request, Response, Scripted, StopReason, SystemOne,
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

#[test]
fn each_office_has_its_band() {
    use ModelTier::{Fast, Frontier, Strong};
    assert_eq!(
        band(Race::Slime, Some(Office::Findings)),
        (Fast, Strong, Frontier),
        "Mneme may run fast"
    );
    assert_eq!(
        band(Race::Orc, Some(Office::Verdict)),
        (Strong, Strong, Frontier),
        "Melete never below strong"
    );
    assert_eq!(
        band(Race::Slime, Some(Office::Draft)),
        (Strong, Strong, Frontier),
        "Aoede never below strong"
    );
    assert_eq!(
        band(Race::Elf, None),
        (Frontier, Frontier, Frontier),
        "no office: the race's own tier"
    );
    assert_eq!(
        band(Race::DarkElf, Some(Office::Findings)),
        (Frontier, Frontier, Frontier),
        "the adversary thinks hardest"
    );
}

#[tokio::test]
async fn a_model_the_human_chose_is_never_routed() {
    let ground = Ground::new("pinned");
    let config = ground.config(&format!(
        "{TIERS}[models.office.orc]\nmelete = \"chosen\"\n[decide]\nroute = true\n"
    ));
    let (wire, dec) = deciders(&ground, vec![], Llm::default(), true);
    let routed = route(
        &config,
        Some(&dec),
        &caller("orc-gate"),
        Race::Orc,
        Some(Office::Verdict),
        "x",
    )
    .await;
    assert_eq!(
        (routed.model.as_str(), routed.reason),
        ("chosen", Reason::Pinned)
    );
    assert_eq!(wire.sent(), Vec::new(), "nothing is asked");

    // The function word names the same office.
    let config = ground.config(&format!(
        "{TIERS}[models.office.orc]\njudge = \"by-function\"\n"
    ));
    let routed = route(
        &config,
        Some(&dec),
        &caller("orc-gate"),
        Race::Orc,
        Some(Office::Verdict),
        "x",
    )
    .await;
    assert_eq!(routed.model, "by-function");
}

#[tokio::test]
async fn with_routing_off_a_body_rides_its_default_tier() {
    let ground = Ground::new("off");
    let config = ground.config(TIERS);
    let (wire, dec) = deciders(&ground, vec![], Llm::default(), false);
    let routed = route(
        &config,
        Some(&dec),
        &caller("orc-gate"),
        Race::Orc,
        Some(Office::Verdict),
        "x",
    )
    .await;
    assert_eq!((routed.model.as_str(), routed.reason), ("mid", Reason::Off));
    assert_eq!(wire.sent(), Vec::new());
    let routed = route(
        &config,
        None,
        &caller("orc-gate"),
        Race::Orc,
        Some(Office::Verdict),
        "x",
    )
    .await;
    assert_eq!(routed.reason, Reason::Off);
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
        // Routine reading for Mneme drops to fast.
        (
            Race::Slime,
            Some(Office::Findings),
            [0.95, 0.05, 0.0],
            0.1,
            "small",
        ),
        // Routine judging for Melete stops at its floor, strong.
        (
            Race::Orc,
            Some(Office::Verdict),
            [0.95, 0.05, 0.0],
            0.1,
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
        // The Dark Elf's routine findings stay at frontier.
        (
            Race::DarkElf,
            Some(Office::Findings),
            [0.95, 0.05, 0.0],
            0.1,
            "big",
        ),
    ];
    for (race, office, demand, irreversible, want) in cases {
        let (wire, dec) = deciders(
            &ground,
            vec![measured(demand, irreversible)],
            Llm::default(),
            true,
        );
        let routed = route(
            &config,
            Some(&dec),
            &caller("body"),
            race,
            office,
            "the commission",
        )
        .await;
        assert_eq!(
            routed.model, want,
            "{race:?} {office:?} {demand:?} {irreversible}"
        );
        assert_eq!(routed.reason, Reason::Decided);
        assert_eq!(wire.sent().len(), 1);
    }
    let rows = ground.journal().read().unwrap().rows;
    assert_eq!(rows.len(), 5);
    assert!(
        rows.iter()
            .all(|routed| routed.point == "model.route" && routed.basis == "measured")
    );
}

#[tokio::test]
async fn an_unsure_or_failed_route_keeps_the_default_tier() {
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
    let routed = route(
        &config,
        Some(&dec),
        &caller("orc-gate"),
        Race::Orc,
        Some(Office::Verdict),
        "x",
    )
    .await;
    assert_eq!(
        (routed.model.as_str(), routed.reason.clone()),
        ("mid", Reason::Unsure)
    );
    assert_eq!(
        routed.decision.unwrap().source.basis,
        mnemorph_provider::Basis::Stated
    );

    // The System One fails and so does the fallback: the default stands, and says why.
    let (_, dec) = deciders(
        &ground,
        vec![Reply::status(401, "{}", &[])],
        Llm::default(),
        true,
    );
    let routed = route(
        &config,
        Some(&dec),
        &caller("orc-gate"),
        Race::Orc,
        Some(Office::Verdict),
        "x",
    )
    .await;
    assert_eq!(routed.model, "mid");
    assert!(
        matches!(routed.reason, Reason::Failed(_)),
        "{:?}",
        routed.reason
    );
}
