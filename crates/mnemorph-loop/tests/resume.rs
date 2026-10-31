//! Resuming a session's run: it opens through the same door as a fresh session — the end gate
//! included — refuses what has nothing to resume, picks the newest stop for `last`, and keeps the
//! prefix append-only when the configuration names another model.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use mnemorph_loop::open::{Bound, Opened, open_with, resume_with};
use mnemorph_loop::{Beat, Config, Ending, Paths, ScriptedModel};
use mnemorph_provider::{Block, Message, Response};

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-resume-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("w/.mnemorph/rimuru/rimuru")).unwrap();
        fs::write(
            root.join("w/.mnemorph/rimuru/rimuru/README.md"),
            "# rimuru\n\n- **Race:** rimuru\n- **Verify:** `grep -q fixed out.txt`\n",
        )
        .unwrap();
        let w = Self { root };
        w.config(None);
        w
    }

    fn world(&self) -> PathBuf {
        self.root.join("w")
    }

    /// The world's configuration: unguarded for the test's verify line, and `steps.solo` when a
    /// session should stop at a checkpoint after that many steps.
    fn config(&self, solo_steps: Option<u32>) {
        let steps = solo_steps.map_or_else(String::new, |n| format!("\n[steps]\nsolo = {n}\n"));
        fs::write(
            self.root.join("no-global.toml"),
            format!("[sandbox]\nallow_unguarded = true\n{steps}"),
        )
        .unwrap();
    }

    fn loaded(&self) -> Config {
        Config::load(&Paths::with_global(
            self.root.join("no-global.toml"),
            &self.world(),
        ))
        .unwrap()
    }

    fn open(&self, model: &Arc<ScriptedModel>, model_id: &str) -> Opened {
        let attic = self.root.join("attic");
        open_with(
            &self.world(),
            &self.loaded(),
            Bound {
                model: model.clone(),
                model_id,
                attic: Some(&attic),
            },
            Arc::new(mnemorph_tools::NoHuman),
            Arc::new(|_: &mnemorph_loop::Seen| {}),
            |_| None,
        )
        .unwrap()
    }

    fn resume(
        &self,
        model: &Arc<ScriptedModel>,
        model_id: &str,
        run: &str,
    ) -> Result<Opened, String> {
        let attic = self.root.join("attic");
        resume_with(
            &self.world(),
            &self.loaded(),
            Bound {
                model: model.clone(),
                model_id,
                attic: Some(&attic),
            },
            run,
            Arc::new(mnemorph_tools::NoHuman),
            Arc::new(|_: &mnemorph_loop::Seen| {}),
            |_| None,
        )
    }

    fn journal(&self, run: &str) -> PathBuf {
        self.world()
            .join(format!(".mnemorph/instruments/loop/{run}.jsonl"))
    }

    fn beats(&self, run: &str) -> Vec<Beat> {
        mnemorph_ledger::Journal::<Beat>::new(self.journal(run))
            .read()
            .unwrap()
            .rows
    }

    /// A session that stops at a checkpoint after one step: its run's id.
    async fn checkpointed(
        &self,
        model_id: &str,
        first: Result<Response, mnemorph_provider::Error>,
    ) -> String {
        self.config(Some(1));
        let model = Arc::new(ScriptedModel::new([first]));
        let mut opened = self.open(&model, model_id);
        let ending = opened
            .session
            .run()
            .ask("make out.txt say fixed", &mut |_| {})
            .await;
        assert!(matches!(ending, Ending::Checkpoint { .. }), "{ending:?}");
        opened.run_id
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write(id: &str, content: &str) -> Result<Response, mnemorph_provider::Error> {
    ScriptedModel::tool(
        id,
        "write",
        serde_json::json!({"path": "out.txt", "content": content}),
    )
}

/// A resumed run answers through the end gate exactly as a fresh session does: a failing verify
/// line sends its answer back.
#[tokio::test]
async fn a_resumed_run_lands_through_the_end_gate() {
    let w = World::new("gate");
    let run = w
        .checkpointed("scripted-model", write("w0", "broken\n"))
        .await;
    let warned = w.beats(&run).iter().any(|b| {
        matches!(b, Beat::Warning { text } if text.contains(&format!("`mnemorph resume {run}`")))
    });
    assert!(warned, "the checkpoint names the way back");
    w.config(None);
    let model = Arc::new(ScriptedModel::new([
        write("w1", "still broken\n"),
        ScriptedModel::text("Done."),
        write("w2", "fixed\n"),
        ScriptedModel::text("Fixed."),
    ]));
    let mut opened = w.resume(&model, "scripted-model", &run).unwrap();
    assert_eq!(opened.run_id, run, "the same run, picked up");
    let ending = opened.session.run().ask("go on", &mut |_| {}).await;
    assert_eq!(ending, Ending::Answered("Fixed.".into()));
    assert_eq!(
        model.requests().len(),
        4,
        "the gate sent the first answer back"
    );
    let verdicts: Vec<String> = w
        .beats(&run)
        .into_iter()
        .filter_map(|b| match b {
            Beat::Gate { verdict, .. } => Some(verdict),
            _ => None,
        })
        .collect();
    assert_eq!(verdicts, ["FAIL", "PASS"]);
}

#[tokio::test]
async fn nothing_to_resume_is_refused() {
    let w = World::new("refuse");
    let model = Arc::new(ScriptedModel::new([ScriptedModel::text("hello")]));
    let answered = {
        let mut opened = w.open(&model, "scripted-model");
        opened.session.run().ask("hi", &mut |_| {}).await;
        opened.run_id
    };
    let none = Arc::new(ScriptedModel::new([]));
    let refused = |run: &str| w.resume(&none, "scripted-model", run).err().unwrap();
    assert!(
        refused(&answered).contains("ended answered: there is nothing to resume"),
        "{}",
        refused(&answered)
    );
    assert!(
        refused("r1-1").contains("does not exist"),
        "{}",
        refused("r1-1")
    );
    assert!(refused("rimuru-1").contains("names no session's run"));
    fs::write(
        w.journal("r2-2"),
        "{\"beat\":\"ask\",\"text\":\"go\"}\nnot json\n",
    )
    .unwrap();
    assert!(refused("r2-2").contains("corrupt"), "{}", refused("r2-2"));
    fs::write(
        w.journal("r3-3"),
        "{\"beat\":\"ask\",\"text\":\"go\"}\n{\"beat\":\"end\",\"ending\":\"checkpoint: steps: 1 taken\"}\n",
    )
    .unwrap();
    let held = mnemorph_loop::resume::hold(&w.world(), "r3-3").unwrap();
    assert!(
        refused("r3-3").contains("held by another live session"),
        "{}",
        refused("r3-3")
    );
    drop(held);
    assert!(
        w.resume(&none, "scripted-model", "r3-3").is_ok(),
        "free once its session ends"
    );
    assert_eq!(none.requests().len(), 0, "a refusal calls no model");
}

/// `last` is the newest run that stopped; an answered one, however new, is not among them.
#[test]
fn last_is_the_newest_stop() {
    let w = World::new("last");
    let stop = "{\"beat\":\"ask\",\"text\":\"the ask\"}\n{\"beat\":\"end\",\"ending\":\"checkpoint: budget: 9 of 8 spent\"}\n";
    let done = "{\"beat\":\"ask\",\"text\":\"hi\"}\n{\"beat\":\"end\",\"ending\":\"answered\"}\n";
    let now = SystemTime::now();
    fs::create_dir_all(w.journal("r").parent().unwrap()).unwrap();
    for (run, text, ago) in [
        ("r100-1", stop, 300),
        ("r200-1", stop, 200),
        ("r300-1", done, 100),
    ] {
        fs::write(w.journal(run), text).unwrap();
        fs::File::options()
            .write(true)
            .open(w.journal(run))
            .unwrap()
            .set_modified(now - Duration::from_secs(ago))
            .unwrap();
    }
    let stopped = mnemorph_loop::stopped(&w.world());
    let runs: Vec<&str> = stopped.iter().map(|s| s.run.as_str()).collect();
    assert_eq!(runs, ["r200-1", "r100-1"]);
    assert_eq!(stopped[0].why, "budget: 9 of 8 spent");
    assert_eq!(stopped[0].ask, "the ask");
    let none = Arc::new(ScriptedModel::new([]));
    let opened = w.resume(&none, "scripted-model", "last").unwrap();
    assert_eq!(opened.run_id, "r200-1");
    assert!(
        mnemorph_loop::stopped(&w.world())
            .iter()
            .all(|s| s.run != "r200-1"),
        "a held run is not offered again"
    );
}

fn plain(m: &Message) -> Vec<Block> {
    m.content
        .iter()
        .map(|b| match b {
            Block::Text { text, .. } => Block::Text {
                text: text.clone(),
                cache: false,
            },
            Block::ToolResult {
                tool_use_id,
                content,
                is_error,
                ..
            } => Block::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: content.clone(),
                is_error: *is_error,
                cache: false,
                references: Vec::new(),
            },
            other => other.clone(),
        })
        .collect()
}

/// What `before` sent is the head of what `after` sends: only appended to.
fn appends_to(before: &[Message], after: &[Message]) -> bool {
    before.len() <= after.len()
        && before.iter().zip(after).enumerate().all(|(i, (b, a))| {
            let (b, a) = (plain(b), plain(a));
            if i + 1 == before.len() {
                a.starts_with(&b)
            } else {
                a == b
            }
        })
}

fn said(m: &Message) -> String {
    m.content
        .iter()
        .filter_map(|b| match b {
            Block::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// The run keeps its model; when the configuration names another, the switch is said as a context
/// update appended after what was sent, and no request rewrites the one before.
#[tokio::test]
async fn a_changed_model_is_a_context_update() {
    let w = World::new("model");
    fs::write(w.world().join("a.txt"), "alpha\n").unwrap();
    let read = || ScriptedModel::tool("t", "read", serde_json::json!({"path": "a.txt"}));
    let run = w.checkpointed("model-a", read()).await;

    // The same model: kept, and nothing said.
    let same = Arc::new(ScriptedModel::new([read()]));
    {
        let mut opened = w.resume(&same, "model-a", &run).unwrap();
        opened.session.run().ask("go on", &mut |_| {}).await;
    }
    let first = &same.requests()[0];
    assert_eq!(first.model, "model-a");
    assert!(
        first
            .messages
            .iter()
            .all(|m| !said(m).contains("the model is now"))
    );
    let sent_before = same.requests().last().unwrap().messages.clone();

    // Another model: the run rides it, told so after what it was sent.
    w.config(None);
    let other = Arc::new(ScriptedModel::new([read(), ScriptedModel::text("alpha")]));
    let mut opened = w.resume(&other, "model-b", &run).unwrap();
    let ending = opened.session.run().ask("and now", &mut |_| {}).await;
    assert_eq!(ending, Ending::Answered("alpha".into()));
    let requests = other.requests();
    assert_eq!(requests[0].model, "model-b");
    let last = said(requests[0].messages.last().unwrap());
    assert!(
        last.contains("the model is now `model-b` (was `model-a`)"),
        "{last}"
    );
    assert!(
        appends_to(&sent_before, &requests[0].messages),
        "the resumed prefix is the sent one"
    );
    assert!(appends_to(&requests[0].messages, &requests[1].messages));
    let beats = w.beats(&run);
    assert!(
        !beats
            .iter()
            .any(|b| matches!(b, Beat::Prefix { why, .. } if why == "unexplained")),
        "the guard saw no unexplained move"
    );
    assert!(
        matches!(beats.iter().rev().find(|b| matches!(b, Beat::Rides { .. })), Some(Beat::Rides { model }) if model == "model-b")
    );
}

/// A journal from a cloned world is printed by `runs`: its control, bidi and zero-width
/// characters are shown escaped, never sent to the terminal.
#[test]
fn a_hostile_journal_is_shown_escaped() {
    let w = World::new("hostile");
    fs::create_dir_all(w.journal("r").parent().unwrap()).unwrap();
    fs::write(
        w.journal("r7-7"),
        "{\"beat\":\"ask\",\"text\":\"look \\u001b]0;pwned\\u0007 \\u202eevil\\u200b\"}\n{\"beat\":\"end\",\"ending\":\"checkpoint: x\\u001b[2J\"}\n",
    )
    .unwrap();
    let s = &mnemorph_loop::stopped(&w.world())[0];
    for text in [&s.ask, &s.why] {
        assert!(
            !text.chars().any(mnemorph_loop::hidden),
            "{text:?} carries a hidden character"
        );
    }
    assert!(s.ask.contains("\\u{1b}]0;pwned\\u{7}"), "{}", s.ask);
    assert!(s.ask.contains("\\u{202e}evil\\u{200b}"), "{}", s.ask);
    assert_eq!(s.why, "x\\u{1b}[2J");
}

/// A fresh session whose run no lock can hold does not open: it would be resumable while live.
#[test]
fn a_run_that_cannot_be_held_opens_no_session() {
    let w = World::new("unheld");
    fs::create_dir_all(w.world().join(".mnemorph/instruments")).unwrap();
    fs::write(
        w.world().join(".mnemorph/instruments/loop"),
        "not a directory",
    )
    .unwrap();
    let model = Arc::new(ScriptedModel::new([]));
    let attic = w.root.join("attic");
    let refused = open_with(
        &w.world(),
        &w.loaded(),
        Bound {
            model,
            model_id: "scripted-model",
            attic: Some(&attic),
        },
        Arc::new(mnemorph_tools::NoHuman),
        Arc::new(|_: &mnemorph_loop::Seen| {}),
        |_| None,
    )
    .err()
    .unwrap();
    assert!(refused.contains("cannot be held"), "{refused}");
}

/// A listing's momentary probe never makes a resume fail; a run's lock file goes when its session
/// ends with nothing left to resume, and stays while the run may still be resumed.
#[tokio::test]
async fn a_probe_never_refuses_and_finished_locks_go() {
    let w = World::new("locks");
    let run = w
        .checkpointed("scripted-model", write("w0", "broken\n"))
        .await;
    let lock = w.journal(&run).with_extension("lock");
    assert!(lock.exists(), "a stopped run keeps its lock file");
    let world = w.world();
    let probing = std::thread::spawn(move || {
        for _ in 0..200 {
            let _ = mnemorph_loop::stopped(&world);
        }
    });
    w.config(None);
    let none = Arc::new(ScriptedModel::new([ScriptedModel::text("ok")]));
    for _ in 0..20 {
        let opened = w.resume(&none, "scripted-model", &run);
        assert!(opened.is_ok(), "{:?}", opened.err());
    }
    probing.join().unwrap();
    let model = Arc::new(ScriptedModel::new([ScriptedModel::text("done")]));
    {
        let mut opened = w.resume(&model, "scripted-model", &run).unwrap();
        opened.session.run().ask("go on", &mut |_| {}).await;
        assert!(lock.exists(), "held while the session lives");
    }
    assert!(!lock.exists(), "answered: the lock file is gone");
    let fresh = w.open(&model, "scripted-model").run_id;
    assert!(
        !w.journal(&fresh).with_extension("lock").exists(),
        "a session that asked nothing leaves no lock"
    );
}

/// A resume always holds the law: a `--no-law` run picked up later journals a second law beat,
/// on and marked resumed, so a report sees that the run mixed arms.
#[tokio::test]
async fn a_resumed_law_off_run_says_it_mixed_arms() {
    let w = World::new("law-mixed");
    w.config(Some(1));
    let attic = w.root.join("attic");
    let model = Arc::new(ScriptedModel::new([write("w0", "broken\n")]));
    let mut opened = mnemorph_loop::open::open_with_law(
        &w.world(),
        &w.loaded(),
        Bound {
            model: model.clone(),
            model_id: "scripted-model",
            attic: Some(&attic),
        },
        mnemorph_loop::Law::Off,
        Arc::new(mnemorph_tools::NoHuman),
        Arc::new(|_: &mnemorph_loop::Seen| {}),
        |_| None,
    )
    .unwrap();
    let ending = opened
        .session
        .run()
        .ask("make out.txt say fixed", &mut |_| {})
        .await;
    assert!(matches!(ending, Ending::Checkpoint { .. }), "{ending:?}");
    let run = opened.run_id.clone();
    drop(opened);
    w.config(None);
    let model = Arc::new(ScriptedModel::new([
        write("w1", "fixed\n"),
        ScriptedModel::text("Fixed."),
    ]));
    let mut opened = w.resume(&model, "scripted-model", &run).unwrap();
    assert_eq!(opened.law, mnemorph_loop::Law::On);
    opened.session.run().ask("go on", &mut |_| {}).await;
    let said: Vec<(bool, bool)> = w
        .beats(&run)
        .into_iter()
        .filter_map(|b| match b {
            Beat::Law { on, resumed } => Some((on, resumed)),
            _ => None,
        })
        .collect();
    assert_eq!(
        said,
        [(false, false), (true, true)],
        "the last one holds from the resume"
    );
}
