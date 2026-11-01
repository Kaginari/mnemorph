//! The end gate runs only pinned verify lines: a creature's card lives in the world, which a body
//! writes and a cloned repository brings from a stranger, so what a card says is never trusted
//! because it says it. The pins are kept under the human's state directory (a temporary
//! `XDG_STATE_HOME` in CI and here), chained.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_loop::open::{Bound, open_with};
use mnemorph_loop::{Config, Ending, Paths, ScriptedModel};
use mnemorph_world::pins;

const CARD: &str = ".mnemorph/rimuru/rimuru/README.md";
const CHECK: &str = "grep -q fixed out.txt";

struct World(PathBuf);

impl World {
    /// A world whose session card's verify line is `line`; nothing pinned.
    fn new(name: &str, line: &str) -> Self {
        let w = std::env::temp_dir().join(format!("mnemorph-pin-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&w);
        fs::create_dir_all(w.join(".mnemorph/rimuru/rimuru")).unwrap();
        fs::write(
            w.join(CARD),
            format!("# rimuru\n\n- **Race:** rimuru\n- **Verify:** `{line}`\n"),
        )
        .unwrap();
        // No kernel guard in a test run: the verify line runs unguarded, as the human allows here.
        fs::write(
            w.with_extension("global.toml"),
            "[sandbox]\nallow_unguarded = true\n",
        )
        .unwrap();
        Self(w.canonicalize().unwrap())
    }

    fn pin(&self, lines: &[&str]) {
        let lines: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
        pins::record(
            &self.0,
            pins::row(pins::Act::Pin, "rimuru", &lines, pins::HUMAN),
        )
        .unwrap();
    }

    /// One session's ask, `model` the body.
    async fn ask(&self, model: &Arc<ScriptedModel>) -> Ending {
        let config = Config::load(&Paths::with_global(
            self.0.with_extension("global.toml"),
            &self.0,
        ))
        .unwrap();
        let attic = self.0.with_extension("attic");
        let mut opened = open_with(
            &self.0,
            &config,
            Bound {
                model: model.clone(),
                model_id: "scripted-model",
                attic: Some(&attic),
            },
            Arc::new(mnemorph_tools::NoHuman),
            Arc::new(|_: &mnemorph_loop::Seen| {}),
            |_| None,
        )
        .unwrap();
        opened
            .session
            .run()
            .ask("make out.txt say fixed", &mut |_| {})
            .await
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
        let _ = fs::remove_dir_all(self.0.with_extension("attic"));
        let _ = fs::remove_file(self.0.with_extension("global.toml"));
        if let Some(p) = pins::path_of(&self.0) {
            let _ = fs::remove_file(p);
        }
    }
}

fn write(
    id: &str,
    path: &str,
    content: &str,
) -> Result<mnemorph_provider::Response, mnemorph_provider::Error> {
    ScriptedModel::tool(
        id,
        "write",
        serde_json::json!({"path": path, "content": content}),
    )
}

fn failed(ending: &Ending) -> &str {
    match ending {
        Ending::Failed(why) => why,
        other => panic!("the landing passed: {other:?}"),
    }
}

/// A body that turns its card's verify line into `true` — by any route that reached the file —
/// gets nothing run: the card differs from the pin, so the landing fails closed at once, with the
/// human's way on.
#[tokio::test]
async fn a_card_turned_to_true_runs_nothing_and_fails_closed() {
    let w = World::new("true", CHECK);
    w.pin(&[CHECK]);
    // Whatever route did reach it — a commit checked out, a merge, a clone — the card now says
    // `true`, and the body lands work the pinned line would fail.
    fs::write(
        w.0.join(CARD),
        "# rimuru\n\n- **Race:** rimuru\n- **Verify:** `true`\n",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        write("w1", "out.txt", "broken\n"),
        ScriptedModel::text("Done."),
    ]));
    let ending = w.ask(&model).await;
    let why = failed(&ending);
    assert!(
        why.contains(
            "the verify line in .mnemorph/rimuru/rimuru/README.md differs from the pinned one"
        ) && why.contains("run `mnemorphd verify pin rimuru` to accept it"),
        "{why}"
    );
    assert_eq!(
        model.requests().len(),
        2,
        "no retry: a body cannot pin its way out"
    );
}

/// The write tool on the card is refused, and the body is told so.
#[tokio::test]
async fn the_write_tool_never_reaches_a_card() {
    let w = World::new("tool", CHECK);
    w.pin(&[CHECK]);
    let model = Arc::new(ScriptedModel::new([
        write(
            "w0",
            CARD,
            "# rimuru\n\n- **Race:** rimuru\n- **Verify:** `true`\n",
        ),
        ScriptedModel::text("Nothing changed."),
    ]));
    let ending = w.ask(&model).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    let seen = format!("{:?}", model.requests()[1].messages);
    assert!(seen.contains("is protected"), "{seen}");
    assert!(fs::read_to_string(w.0.join(CARD)).unwrap().contains(CHECK));
}

/// The pinned line runs: it fails broken work, the body is handed it back, and fixed work lands.
#[tokio::test]
async fn a_pinned_line_runs() {
    let w = World::new("runs", CHECK);
    w.pin(&[CHECK]);
    let model = Arc::new(ScriptedModel::new([
        write("w1", "out.txt", "broken\n"),
        ScriptedModel::text("Done."),
        write("w2", "out.txt", "fixed\n"),
        ScriptedModel::text("Fixed."),
    ]));
    let ending = w.ask(&model).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    assert_eq!(
        model.requests().len(),
        4,
        "the pinned line failed the first"
    );
}

/// A cloned world brings its cards from a stranger: with no pin, its verify line never runs and
/// every landing fails closed until the human pins it — then it runs.
#[tokio::test]
async fn a_cloned_world_refuses_until_pinned() {
    let w = World::new("clone", "true");
    let model = Arc::new(ScriptedModel::new([
        write("w1", "out.txt", "broken\n"),
        ScriptedModel::text("Done."),
    ]));
    let ending = w.ask(&model).await;
    let why = failed(&ending);
    assert!(
        why.contains("is not pinned") && why.contains("mnemorphd verify pin rimuru"),
        "{why}"
    );
    // The human reads the stranger's line, replaces it, and pins it.
    fs::write(
        w.0.join(CARD),
        format!("# rimuru\n\n- **Race:** rimuru\n- **Verify:** `{CHECK}`\n"),
    )
    .unwrap();
    w.pin(&[CHECK]);
    let model = Arc::new(ScriptedModel::new([
        write("w2", "out.txt", "fixed\n"),
        ScriptedModel::text("Fixed."),
    ]));
    let ending = w.ask(&model).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
}

/// A witness whose chain breaks — a pin edited in place — trusts no pin: the gate fails closed.
#[tokio::test]
async fn a_tampered_witness_fails_closed() {
    let w = World::new("tamper", CHECK);
    w.pin(&[CHECK]);
    let at = pins::path_of(&w.0).unwrap();
    let text = fs::read_to_string(&at).unwrap();
    assert!(text.contains(CHECK), "{text}");
    fs::write(&at, text.replace(CHECK, "true")).unwrap();
    fs::write(
        w.0.join(CARD),
        "# rimuru\n\n- **Race:** rimuru\n- **Verify:** `true`\n",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        write("w1", "out.txt", "broken\n"),
        ScriptedModel::text("Done."),
    ]));
    let ending = w.ask(&model).await;
    let why = failed(&ending);
    assert!(why.contains("the verify pins do not verify"), "{why}");
}

/// A pin is refused inside a body's command, and the witness is out of the world.
#[test]
fn a_body_s_command_never_pins() {
    let w = World::new("body", CHECK);
    let at = pins::path_of(&w.0).unwrap();
    assert!(!at.starts_with(&w.0));
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "pin_as_a_body", "--nocapture", "--ignored"])
        .env("MNEMORPH_BODY", "slime-x")
        .env("PIN_WORLD", &w.0)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(!at.exists(), "nothing was recorded");
}

/// Run by [`a_body_s_command_never_pins`] with `MNEMORPH_BODY` set.
#[test]
#[ignore = "run as a body by a_body_s_command_never_pins"]
fn pin_as_a_body() {
    let Some(w) = std::env::var_os("PIN_WORLD") else {
        return;
    };
    let e = pins::record(
        Path::new(&w),
        pins::row(pins::Act::Pin, "rimuru", &["true".into()], pins::HUMAN),
    )
    .unwrap_err();
    assert!(e.contains("slime-x runs this"), "{e}");
}

/// The guts classes' bench is the pinned line: run by the body, it is `positive` while the card
/// says the pin, and a plain command once the card differs.
#[tokio::test]
async fn the_pinned_line_is_the_only_bench() {
    let w = World::new("bench", CHECK);
    w.pin(&[CHECK]);
    let classes = |w: &World| -> String {
        let dir = w.0.join(".mnemorph/instruments/loop");
        fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| fs::read_to_string(e.path()).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    };
    let bench = || {
        Arc::new(ScriptedModel::new([
            write("w1", "out.txt", "fixed\n"),
            ScriptedModel::tool("b1", "bash", serde_json::json!({ "command": CHECK })),
            ScriptedModel::text("Fixed."),
        ]))
    };
    let ending = w.ask(&bench()).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    assert!(classes(&w).contains("\"positive\""), "{}", classes(&w));
    // The card now differs: the same command is no bench.
    let _ = fs::remove_dir_all(w.0.join(".mnemorph/instruments"));
    fs::write(
        w.0.join(CARD),
        format!("# rimuru\n\n- **Race:** rimuru\n- **Verify:** `{CHECK}`\n- **Verify:** `true`\n"),
    )
    .unwrap();
    let _ = w.ask(&bench()).await;
    assert!(!classes(&w).contains("\"positive\""), "{}", classes(&w));
}

/// The verdicts kept for `w`, as the harness wrote them.
fn verdicts(w: &World) -> Vec<mnemorph_world::verdicts::VerdictRow> {
    mnemorph_world::verdicts::read(&w.0).unwrap()
}

/// A line an Orc coded is checked, but it is the Orc's word, not the human's: run by the body it
/// is no bench, and a pass on it alone is recorded as that Orc's check.
#[tokio::test]
async fn an_orc_s_line_is_no_bench_and_its_pass_is_the_orc_s() {
    let w = World::new("orc", CHECK);
    pins::record(
        &w.0,
        pins::row(pins::Act::Code, "rimuru", &[CHECK.to_owned()], "orc-o"),
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([
        write("w1", "out.txt", "fixed\n"),
        ScriptedModel::tool("b1", "bash", serde_json::json!({ "command": CHECK })),
        ScriptedModel::text("Fixed."),
    ]));
    let ending = w.ask(&model).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    let loops = fs::read_dir(w.0.join(".mnemorph/instruments/loop"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| fs::read_to_string(e.path()).unwrap())
        .collect::<String>();
    assert!(!loops.contains("\"positive\""), "{loops}");
    let v = verdicts(&w);
    let last = v.last().unwrap();
    assert_eq!(last.verdict, "PASS");
    assert_eq!(last.verified_by, ["orc-o's pin"]);
    let log = fs::read_to_string(w.0.join(".mnemorph/log.md")).unwrap();
    assert!(log.contains("**Verified by:** orc-o's pin"), "{log}");
    // The human's pin, for contrast.
    w.pin(&[CHECK]);
    let model = Arc::new(ScriptedModel::new([
        write("w2", "out.txt", "fixed\nagain\n"),
        ScriptedModel::text("Fixed."),
    ]));
    let _ = w.ask(&model).await;
    assert_eq!(
        verdicts(&w).last().unwrap().verified_by,
        ["the human's pin"]
    );
}

/// A card with no verify line and no pin — a clone with its lines stripped — lands as the baseline,
/// but the verdict says no check ran: `PASS, unchecked`, never a plain pass.
#[tokio::test]
async fn a_landing_no_line_checked_is_unchecked() {
    let w = World::new("unchecked", CHECK);
    fs::write(w.0.join(CARD), "# rimuru\n\n- **Race:** rimuru\n").unwrap();
    let model = Arc::new(ScriptedModel::new([
        write("w1", "out.txt", "anything\n"),
        ScriptedModel::text("Done."),
    ]));
    let ending = w.ask(&model).await;
    assert!(matches!(ending, Ending::Answered(_)), "{ending:?}");
    let last = verdicts(&w).pop().unwrap();
    assert_eq!(last.verdict, "PASS, unchecked");
    assert!(last.unchecked && last.verified_by.is_empty(), "{last:?}");
    let log = fs::read_to_string(w.0.join(".mnemorph/log.md")).unwrap();
    assert!(
        log.contains("gate: PASS, unchecked") && log.contains("**Unchecked:** no verify line ran"),
        "{log}"
    );
}
