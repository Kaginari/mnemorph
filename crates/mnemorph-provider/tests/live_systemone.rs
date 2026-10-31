//! One real decision, only when asked for: `MNEMORPH_LIVE=1 TYPESAFE_API_KEY=… cargo nextest run
//! -p mnemorph-provider --run-ignored all live_systemone`. Point `MNEMORPH_LIVE_SYSTEMONE_URL` at a
//! jeff server (and `MNEMORPH_LIVE_SYSTEMONE_MODEL=jeff-latest`) to ask it instead of TypeSafe.

use std::sync::Arc;

use mnemorph_provider::{
    Ask, Basis, Complete, Completion, Decider, Error, Fallback, HttpTransport, Primary, Question,
    Request, SystemOne,
};

/// A fallback that must never be reached: the live test proves the System One provider answered.
struct Unreachable;

impl Complete for Unreachable {
    fn complete<'a>(&'a self, _: &'a Request) -> Completion<'a> {
        Box::pin(async {
            Err(Error::Invalid(
                "the System One provider did not answer".into(),
            ))
        })
    }
}

#[tokio::test]
#[ignore = "spends a decision: run with MNEMORPH_LIVE=1 and TYPESAFE_API_KEY"]
async fn one_real_decision() {
    if std::env::var("MNEMORPH_LIVE").as_deref() != Ok("1") {
        return;
    }
    let url = std::env::var("MNEMORPH_LIVE_SYSTEMONE_URL")
        .unwrap_or_else(|_| "https://api.typesafe.ai".into());
    let model =
        std::env::var("MNEMORPH_LIVE_SYSTEMONE_MODEL").unwrap_or_else(|_| "jev-latest".into());
    let key = std::env::var("TYPESAFE_API_KEY").ok();
    let policy = SystemOne::policy();
    let transport = Arc::new(HttpTransport::new(policy.connect).expect("tls"));
    let decider = Decider {
        primary: Some(Primary {
            client: SystemOne::new(transport, &url, key, policy),
            provider: "live".into(),
            model,
        }),
        fallback: Fallback {
            client: Arc::new(Unreachable),
            provider: "none".into(),
            model: "none".into(),
        },
        unavailable: None,
    };
    let ask = Ask {
        state:
            "cargo test -p mnemorph-config\n\nrunning 25 tests\ntest result: FAILED. 23 passed; \
                2 failed; 0 ignored\n\nfailures:\n    decide_via_names_a_system_one_provider"
                .into(),
        questions: vec![
            (
                "passed".into(),
                Question::Noul {
                    instructions: "Every test in this run passed.".into(),
                },
            ),
            (
                "outcome".into(),
                Question::Choice {
                    instructions: "How did this test run end?".into(),
                    options: vec![
                        ("green".into(), "Every test passed.".into()),
                        ("red".into(), "At least one test failed.".into()),
                        ("unknown".into(), "The output does not say.".into()),
                    ],
                },
            ),
        ],
    };
    let d = decider.decide(&ask).await.expect("a decision");
    eprintln!(
        "{} · {} ms · {:?}\n{:#?}",
        d.source.model,
        d.elapsed.as_millis(),
        d.usage,
        d.answers
    );
    assert_eq!(d.source.basis, Basis::Measured);
    assert_eq!(d.fell_back, None);
    assert_ne!(d.id, "", "a decision has an id");
    let outcome = d.answer("outcome").unwrap();
    assert_eq!(outcome.choice(), "red", "{outcome:?}");
    assert!(d.answer("passed").unwrap().probability("true") < 0.5);
}

/// Metis's two questions, asked of the real service: a rename is routine, a payments migration is
/// hard and hard to undo.
#[tokio::test]
#[ignore = "spends two decisions: run with MNEMORPH_LIVE=1 and TYPESAFE_API_KEY"]
async fn two_real_routes() {
    if std::env::var("MNEMORPH_LIVE").as_deref() != Ok("1") {
        return;
    }
    let key = std::env::var("TYPESAFE_API_KEY").ok();
    let policy = SystemOne::policy();
    let transport = Arc::new(HttpTransport::new(policy.connect).expect("tls"));
    let jev = SystemOne::new(transport, "https://api.typesafe.ai", key, policy);
    let ask = |work: &str| {
        Ask {
        state: work.into(),
        questions: vec![
            (
                "demand".into(),
                Question::Score {
                    instructions: "How demanding is this work for the model that will do it?".into(),
                    levels: vec![
                        "Routine: a lookup, a rename, a small change with an obvious shape.".into(),
                        "Involved: several files or steps, some judgement, a known pattern.".into(),
                        "Hard: subtle reasoning, an unclear design, wide or tricky change.".into(),
                    ],
                },
            ),
            (
                "irreversible".into(),
                Question::Noul {
                    instructions: "A mistake in this work would be costly or hard to undo: security, money, migrations, data loss, or an outward act.".into(),
                },
            ),
        ],
    }
    };
    let easy = jev
        .decide(
            "jev-latest",
            &ask("Rename the local variable `cnt` to `count` in src/report.rs."),
        )
        .await
        .expect("a decision");
    let hard = jev
        .decide(
            "jev-latest",
            &ask(
                "Migrate the payments ledger to the new schema in production: split the \
                  `amount` column into currency and minor units, backfill eleven million rows, \
                  and keep refunds consistent while both writers run.",
            ),
        )
        .await
        .expect("a decision");
    eprintln!("easy {:?}\nhard {:?}", easy.answers, hard.answers);
    assert_eq!(easy.answers[0].1.choice(), "0", "a rename is routine");
    assert_eq!(hard.answers[0].1.choice(), "2", "a live migration is hard");
    assert_eq!(hard.answers[1].1.choice(), "true", "and hard to undo");
}
