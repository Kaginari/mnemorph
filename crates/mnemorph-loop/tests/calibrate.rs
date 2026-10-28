//! The reviewers' calibration against live models: each case of the gold set
//! (`crates/mnemorph-proving/fixtures/`) put to a reviewer — a Dark Elf speciality, or the Fable
//! review — and its judgement scored. Ignored, never run in CI: `just calibrate`.
//!
//! The scorer and the set are held with no model by `mnemorph-proving`'s `calibrate` tests.

#[path = "../../mnemorph-proving/tests/support/seeded.rs"]
mod seeded;

#[path = "../../mnemorph-proving/tests/support/calibrate.rs"]
mod calibrate;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_loop::{Model, OpenAiModel};
use mnemorph_provider::{Block, HttpTransport, Message, OpenAiCompatible, Policy, Request, Role};

use calibrate::{Answer, Judgement, Reviewer, gold, judgement, report, score};

/// Where the report goes: `MNEMORPH_CALIBRATE_OUT`, else
/// `$XDG_STATE_HOME/mnemorph/calibration/<reviewer>-<unix seconds>.md` (`~/.local/state` without
/// it) — never inside the repository, whose tree the reviewers' cases come from.
fn out_path(reviewer: Reviewer) -> PathBuf {
    let path = std::env::var_os("MNEMORPH_CALIBRATE_OUT").map_or_else(
        || {
            let state = std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| std::env::home_dir().map(|h| h.join(".local/state")))
                .expect("a state directory");
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            state
                .join("mnemorph/calibration")
                .join(format!("{}-{secs}.md", reviewer.name()))
        },
        PathBuf::from,
    );
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository");
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).expect("the report's folder");
    let parent = parent.canonicalize().expect("the report's folder");
    assert!(
        !parent.starts_with(&repo),
        "the report goes outside the repository: {}",
        path.display()
    );
    path
}

/// The live run. `MNEMORPH_LIVE=1 OPENROUTER_API_KEY=… cargo nextest run -p mnemorph-loop --test
/// calibrate --run-ignored only --no-capture calibrate_live` (or `just calibrate`).
///
/// `MNEMORPH_CALIBRATE_REVIEWER` is `dark-elf` (each case by its own speciality, the default),
/// `dark-elf-<speciality>` (that speciality's cases only) or `fable`; `MNEMORPH_CALIBRATE_MODELS`
/// is a comma-separated panel of OpenRouter models, a free one by default;
/// `MNEMORPH_CALIBRATE_PAUSE_MS` spaces the calls (free models are rate-limited). The key travels
/// only through the environment and is never printed.
#[tokio::test]
#[ignore = "calls OpenRouter: run with MNEMORPH_LIVE=1 and OPENROUTER_API_KEY"]
async fn calibrate_live() {
    if std::env::var("MNEMORPH_LIVE").as_deref() != Ok("1") {
        return;
    }
    let api_key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY");
    let reviewer_name =
        std::env::var("MNEMORPH_CALIBRATE_REVIEWER").unwrap_or_else(|_| "dark-elf".into());
    let reviewer = Reviewer::of(&reviewer_name).unwrap_or_else(|| {
        panic!("no reviewer {reviewer_name}: dark-elf, dark-elf-<speciality> or fable")
    });
    let models: Vec<String> = std::env::var("MNEMORPH_CALIBRATE_MODELS")
        .unwrap_or_else(|_| "nvidia/nemotron-3.5-lightning:free".into())
        .split(',')
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
        .collect();
    let pause = std::time::Duration::from_millis(
        std::env::var("MNEMORPH_CALIBRATE_PAUSE_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3000),
    );
    let out = out_path(reviewer);

    let policy = Policy::default();
    let transport = Arc::new(HttpTransport::new(policy.connect).expect("tls"));
    let client = OpenAiCompatible::new(
        transport,
        "https://openrouter.ai/api",
        Some(api_key),
        policy,
    );
    let model = OpenAiModel {
        client,
        name: "openrouter".into(),
    };

    let cases: Vec<_> = gold().into_iter().filter(|c| reviewer.judges(c)).collect();
    let mut answers = Vec::new();
    for name in &models {
        for case in &cases {
            let (system, ask) = reviewer.prompt(case);
            let request = Request {
                model: name.clone(),
                system: vec![Block::Text {
                    text: system,
                    cache: false,
                }],
                tools: Vec::new(),
                messages: vec![Message {
                    role: Role::User,
                    content: vec![Block::Text {
                        text: ask,
                        cache: false,
                    }],
                }],
                max_tokens: 2048,
                effort: None,
            };
            let judged = match model.complete(&request, &mut |_| {}).await {
                Ok(r) => judgement(
                    &r.content
                        .iter()
                        .filter_map(|b| match b {
                            Block::Text { text, .. } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                Err(e) => {
                    eprintln!("  {name} on {}: {e}", case.id);
                    Judgement::Failed
                }
            };
            eprintln!("  {name} {}: {judged:?}", case.id);
            answers.push(Answer {
                case: case.id.clone(),
                model: name.clone(),
                judgement: judged,
            });
            tokio::time::sleep(pause).await;
        }
    }
    let text = report(reviewer, &models, &cases, &answers);
    std::fs::write(&out, &text).expect("the report");
    let all = score(&cases, &answers).all;
    eprintln!(
        "{}: {} cases, recall {:?}, precision {:?}; report at {}",
        reviewer.name(),
        cases.len(),
        all.recall(),
        all.precision(),
        out.display()
    );
}
