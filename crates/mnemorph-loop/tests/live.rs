//! M5's exit: one real task, solo, against a live model — and the spend the ledger recorded is
//! exactly what the provider reported.
//!
//! `MNEMORPH_LIVE=1 OPENROUTER_API_KEY=… cargo nextest run -p mnemorph-loop --run-ignored only
//! live_solo`. The default model is a free one with tools; `MNEMORPH_LIVE_OR_MODEL` picks another.
//! The key travels only through the environment.

use std::fs;
use std::sync::{Arc, Mutex};

use mnemorph_ledger::{Journal, Usage, UsageRow, spend};
use mnemorph_loop::model::Completion;
use mnemorph_loop::{Beat, EndGate, Ending, Limits, Model, OpenAiModel, Run, Setup, solo};
use mnemorph_provider::{
    Caller, Event, HttpTransport, OpenAiCompatible, Policy, Request, anthropic_prices,
};
use mnemorph_races::Race;
use mnemorph_tools::{Sandbox, registry};

/// A model that keeps the usage of every response the provider returned.
struct Counted {
    inner: OpenAiModel,
    usage: Mutex<Vec<Usage>>,
}

impl Model for Counted {
    fn provider(&self) -> &str {
        self.inner.provider()
    }

    fn complete<'a>(
        &'a self,
        request: &'a Request,
        on: &'a mut (dyn FnMut(Event) + Send),
    ) -> Completion<'a> {
        Box::pin(async move {
            let r = self.inner.complete(request, on).await?;
            self.usage.lock().expect("usage lock").push(r.usage);
            Ok(r)
        })
    }
}

#[tokio::test]
#[ignore = "calls OpenRouter: run with MNEMORPH_LIVE=1 and OPENROUTER_API_KEY"]
async fn live_solo_fixes_a_typo() {
    if std::env::var("MNEMORPH_LIVE").as_deref() != Ok("1") {
        return;
    }
    let key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY");
    let model_name = std::env::var("MNEMORPH_LIVE_OR_MODEL")
        .unwrap_or_else(|_| "nvidia/nemotron-3.5-lightning:free".into());

    let root = std::env::temp_dir().join(format!("mnemorph-live-solo-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("world")).unwrap();
    fs::write(
        root.join("world/notes.md"),
        "# Notes\n\nMnemorph is a harnes for agents.\n",
    )
    .unwrap();

    let policy = Policy::default();
    let transport = Arc::new(HttpTransport::new(policy.connect).expect("tls"));
    let client = OpenAiCompatible::new(transport, "https://openrouter.ai/api", Some(key), policy);
    let model = Arc::new(Counted {
        inner: OpenAiModel {
            client,
            name: "openrouter".into(),
        },
        usage: Mutex::new(Vec::new()),
    });

    let (body, cx) = solo(&root.join("world"), &model_name);
    // The guard helper is not beside the test binary; this test proves the loop, not the guard.
    let cx = cx
        .with_sandbox(Sandbox::None("the live loop test runs unguarded".into()))
        .with_unguarded(true)
        .with_attic(
            mnemorph_tools::Attic::at(&root.join("attic"), &root.join("world")).expect("git"),
        );
    let ledger: Journal<UsageRow> = Journal::new(root.join("usage.jsonl"));
    let setup = Setup {
        model: model.clone(),
        tools: registry(),
        cx,
        journal: Journal::new(root.join("run.jsonl")),
        ledger: Some(Journal::new(root.join("usage.jsonl"))),
        prices: anthropic_prices().unwrap(),
        caller: Caller {
            session: "live".into(),
            run: "live".into(),
            body: "rimuru".into(),
            race: "rimuru".into(),
            commission: None,
            prefix: 0,
        },
    };
    let gate = EndGate {
        verify: vec!["grep -q 'harness for agents' notes.md".into()],
        author: None,
    };
    let mut run = Run::new(
        body,
        Limits {
            steps: 12,
            ..Limits::for_race(Race::Rimuru)
        },
        setup,
    )
    .with_gate(gate);
    let mut shown = String::new();
    let mut on = |e: Event| {
        if let Event::Text(t) = e {
            shown.push_str(&t);
        }
    };
    let ending = run
        .ask(
            "notes.md has a spelling mistake. Fix it, then tell me what you changed.",
            &mut on,
        )
        .await;

    let beats = Journal::<Beat>::new(root.join("run.jsonl"))
        .read()
        .unwrap()
        .rows;
    eprintln!(
        "ending: {ending:?}\nsteps: {}\nbeats: {}",
        run.steps(),
        beats.len()
    );
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    assert_eq!(
        fs::read_to_string(root.join("world/notes.md")).unwrap(),
        "# Notes\n\nMnemorph is a harness for agents.\n"
    );
    assert!(run.steps() >= 2, "it read or edited before answering");

    // The spend the ledger recorded is exactly what the provider reported, call for call.
    let rows = ledger.read().unwrap().rows;
    let reported = model.usage.lock().unwrap().clone();
    assert_eq!(rows.len(), reported.len());
    assert_eq!(rows.iter().map(|r| r.usage).collect::<Vec<_>>(), reported);
    let total = spend(&reported);
    eprintln!("spend: {total:?} over {} calls", rows.len());
    assert!(total.input > 0 && total.output > 0);
    let _ = fs::remove_dir_all(&root);
}
