//! A session's run, explored breadth-first over the real opening, resume and ask: every state its
//! journal can reach from a fresh world within [`DEPTH`] moves — answered, failed, a checkpoint,
//! interrupted, cut mid-turn — with the shortest event path to each, and the invariants asserted
//! in every one. Each state is a copy of the world on disk; each move opens a session on a copy
//! of its parent, fresh or resumed through the same door, and asks once.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use mnemorph_loop::open::{Bound, Opened, open_with, resume_with};
use mnemorph_loop::resume::{hold, stopped};
use mnemorph_loop::{Beat, Config, Ending, Paths, ScriptedModel};
use mnemorph_provider::{Event, Response};

/// How deep the walk goes: a run checkpointed, held, released and resumed to its answer is five.
const DEPTH: usize = 4;

/// What happens to the run next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Ev {
    /// The body fixes `out.txt` and answers: the gate passes.
    Answer,
    /// The body answers over a broken `out.txt`; the gate sends it back, and it fixes it.
    AnswerBroken,
    /// The step meter stops the turn: a checkpoint.
    Checkpoint,
    /// The provider fails.
    Fail,
    /// The human presses Esc during the turn.
    Interrupt,
    /// The process dies mid-turn: an ask, and nothing after it.
    Die,
    /// Another live session takes the run.
    Hold,
    /// That session ends.
    Release,
}

const EVENTS: [Ev; 8] = [
    Ev::Answer,
    Ev::AnswerBroken,
    Ev::Checkpoint,
    Ev::Fail,
    Ev::Interrupt,
    Ev::Die,
    Ev::Hold,
    Ev::Release,
];

/// A run's state as the walk tells them apart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Abs {
    /// How its journal ends: `none`, `answered`, `failed`, `checkpoint`, `interrupted` or `cut`.
    stop: &'static str,
    /// Another live session holds it.
    held: bool,
    /// Where it was last resumed from: `checkpoint`, `interrupted` or `cut`.
    resumed: Option<&'static str>,
}

/// A private directory of its own, gone when dropped.
struct Temp(PathBuf);

impl Temp {
    fn new() -> Temp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mnemorph-lifecycle-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// One state, on disk: the world and the attic; whether another session holds the run.
struct Node {
    tmp: Temp,
    run: Option<String>,
    held: bool,
    resumed: Option<&'static str>,
}

/// Content no earlier write left: every answer changes the tree, so the gate always has a diff.
fn fresh(word: &str) -> String {
    static N: AtomicUsize = AtomicUsize::new(0);
    format!("{word} {}\n", N.fetch_add(1, Ordering::SeqCst))
}

fn write(id: &str, content: &str) -> Result<Response, mnemorph_provider::Error> {
    ScriptedModel::tool(
        id,
        "write",
        serde_json::json!({"path": "out.txt", "content": content}),
    )
}

impl Node {
    fn root(&self) -> &Path {
        self.tmp.path()
    }

    fn world(&self) -> PathBuf {
        self.root().join("w")
    }

    fn start() -> Node {
        let tmp = Temp::new();
        let rimuru = tmp.path().join("w/.mnemorph/rimuru/rimuru");
        fs::create_dir_all(&rimuru).unwrap();
        fs::write(
            rimuru.join("README.md"),
            "# rimuru\n\n- **Race:** rimuru\n- **Verify:** `grep -q fixed out.txt`\n",
        )
        .unwrap();
        Node {
            tmp,
            run: None,
            held: false,
            resumed: None,
        }
    }

    fn fork(&self) -> Node {
        let tmp = Temp::new();
        copy_tree(self.root(), tmp.path());
        Node {
            tmp,
            run: self.run.clone(),
            held: self.held,
            resumed: self.resumed,
        }
    }

    fn config(&self, steps: Option<u32>) -> Config {
        let steps = steps.map_or_else(String::new, |n| format!("\n[steps]\nsolo = {n}\n"));
        fs::write(
            self.root().join("no-global.toml"),
            format!("[sandbox]\nallow_unguarded = true\n{steps}"),
        )
        .unwrap();
        Config::load(&Paths::with_global(
            self.root().join("no-global.toml"),
            &self.world(),
        ))
        .unwrap()
    }

    fn journal(&self) -> Option<PathBuf> {
        let run = self.run.as_ref()?;
        Some(
            self.world()
                .join(format!(".mnemorph/instruments/loop/{run}.jsonl")),
        )
    }

    fn beats(&self) -> Vec<Beat> {
        self.journal().map_or_else(Vec::new, |j| {
            let back = mnemorph_ledger::Journal::<Beat>::new(j).read().unwrap();
            assert!(back.skipped.is_empty(), "a journal line did not read");
            back.rows
        })
    }

    /// Open a session on the run — fresh when there is none yet, resumed otherwise — under
    /// `steps`, answering from `script`. `Err` is the resume's refusal.
    fn open(
        &self,
        steps: Option<u32>,
        script: Vec<Result<Response, mnemorph_provider::Error>>,
    ) -> Result<Opened, String> {
        let config = self.config(steps);
        let model = Arc::new(ScriptedModel::new(script));
        let attic = self.root().join("attic");
        let bound = Bound {
            model,
            model_id: "scripted-model",
            attic: Some(&attic),
        };
        let gate = Arc::new(mnemorph_tools::NoHuman);
        let seen = Arc::new(|_: &mnemorph_loop::Seen| {});
        match &self.run {
            None => open_with(&self.world(), &config, bound, gate, seen, |_| None),
            Some(run) => resume_with(&self.world(), &config, bound, run, gate, seen, |_| None),
        }
    }

    /// Run `e`: whether a resume was refused.
    fn apply(&mut self, e: Ev, rt: &tokio::runtime::Runtime) -> bool {
        match e {
            Ev::Hold => {
                self.held = self.run.is_some();
                return false;
            }
            Ev::Release => {
                self.held = false;
                return false;
            }
            Ev::Die => {
                if let Some(j) = self.journal() {
                    let mut f = fs::OpenOptions::new().append(true).open(j).unwrap();
                    writeln!(f, "{{\"beat\":\"ask\",\"text\":\"go\"}}").unwrap();
                }
                return false;
            }
            _ => {}
        }
        // Another live session's hold, for the length of the move.
        let _other = match (&self.run, self.held) {
            (Some(run), true) => Some(hold(&self.world(), run).unwrap()),
            _ => None,
        };
        let (steps, script) = match e {
            Ev::Answer => (
                None,
                vec![write("w1", &fresh("fixed")), ScriptedModel::text("Fixed.")],
            ),
            Ev::AnswerBroken => (
                None,
                vec![
                    write("w1", &fresh("broken")),
                    ScriptedModel::text("Done."),
                    write("w2", &fresh("fixed")),
                    ScriptedModel::text("Fixed."),
                ],
            ),
            Ev::Checkpoint => (Some(1), vec![write("w1", &fresh("half"))]),
            Ev::Interrupt => (None, vec![write("w1", &fresh("never"))]),
            // The script is empty: the provider fails.
            _ => (None, Vec::new()),
        };
        let resuming = self.run.is_some();
        let from = self.stop();
        let mut opened = match self.open(steps, script) {
            Ok(o) => o,
            Err(why) => {
                assert!(resuming, "a fresh session was refused: {why}");
                return true;
            }
        };
        if let Some(run) = &self.run {
            assert_eq!(&opened.run_id, run, "a resume picked up another run");
            self.resumed = Some(from);
        }
        self.run = Some(opened.run_id.clone());
        let esc = opened.session.interrupt();
        let mut on = |ev: Event| {
            if e == Ev::Interrupt && matches!(ev, Event::ToolStart { .. }) {
                esc.fire();
            }
        };
        let ending = rt.block_on(opened.session.run().ask("make out.txt say fixed", &mut on));
        let want = match e {
            Ev::Answer | Ev::AnswerBroken => Ending::Answered("Fixed.".into()),
            Ev::Interrupt => Ending::Interrupted,
            _ => ending.clone(),
        };
        assert_eq!(ending, want, "{e:?}");
        if e == Ev::AnswerBroken {
            // The gate sent the broken answer back, and passed the fixed one.
            let beats = self.beats();
            let from = beats
                .iter()
                .rposition(|b| matches!(b, Beat::Ask { .. }))
                .unwrap();
            let gates: Vec<&str> = beats[from..]
                .iter()
                .filter_map(|b| match b {
                    Beat::Gate { verdict, .. } => Some(verdict.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(gates, ["FAIL", "PASS"]);
        }
        match e {
            Ev::Checkpoint => assert!(matches!(ending, Ending::Checkpoint { .. }), "{ending:?}"),
            Ev::Fail => assert!(matches!(ending, Ending::Failed(_)), "{ending:?}"),
            _ => {}
        }
        false
    }

    /// How the journal ends.
    fn stop(&self) -> &'static str {
        let beats = self.beats();
        if beats.is_empty() {
            return "none";
        }
        let last_ask = beats.iter().rposition(|b| matches!(b, Beat::Ask { .. }));
        let last_end = beats.iter().rposition(|b| matches!(b, Beat::End { .. }));
        match (last_ask, last_end) {
            (Some(a), Some(e)) if e > a => match &beats[e] {
                Beat::End { ending } if ending == "answered" => "answered",
                Beat::End { ending } if ending == "interrupted" => "interrupted",
                Beat::End { ending } if ending.starts_with("checkpoint") => "checkpoint",
                Beat::End { ending } if ending.starts_with("failed") => "failed",
                Beat::End { ending } => panic!("an ending the lifecycle does not know: {ending}"),
                _ => unreachable!(),
            },
            (Some(_), _) => "cut",
            (None, _) => "none",
        }
    }

    fn abs(&self) -> Abs {
        Abs {
            stop: self.stop(),
            held: self.held,
            resumed: self.resumed,
        }
    }

    /// The invariants every state keeps, read from its journal.
    fn check(&self, path: &[Ev]) {
        let beats = self.beats();
        // Every answer passed the end gate: the last gate of its ask said PASS.
        let mut gate: Option<&str> = None;
        for b in &beats {
            match b {
                Beat::Ask { .. } => gate = None,
                Beat::Gate { verdict, .. } => gate = Some(verdict),
                Beat::End { ending } if ending == "answered" => {
                    assert_eq!(gate, Some("PASS"), "{path:?}: answered past the gate");
                }
                _ => {}
            }
        }
        // A run is listed to resume exactly when it stopped and no live session holds it.
        let _other = match (&self.run, self.held) {
            (Some(run), true) => Some(hold(&self.world(), run).unwrap()),
            _ => None,
        };
        let resumable = matches!(self.stop(), "checkpoint" | "interrupted" | "cut");
        let listed = || {
            self.run
                .as_ref()
                .is_some_and(|run| stopped(&self.world()).iter().any(|s| &s.run == run))
        };
        let want = resumable && !self.held;
        // A command another thread spawns may carry a copy of a session's lock for an instant
        // before it execs: a run that should be listed is read again for up to a second.
        let mut got = listed();
        for _ in 0..100 {
            if got == want || !want {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            got = listed();
        }
        assert_eq!(got, want, "{path:?}");
    }
}

/// Copy `from` into `to`.
fn copy_tree(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir_all(&dst).unwrap();
            copy_tree(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

/// One move from `node`, on a copy of it, every invariant checked.
fn advance(node: &Node, abs: &Abs, path: &[Ev]) -> (Node, Abs) {
    let e = *path.last().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut n = node.fork();
    let refused = n.apply(e, &rt);
    n.check(path);
    let after = n.abs();
    // A resume is refused exactly when the run answered, failed, or another session holds it.
    let asks = !matches!(e, Ev::Hold | Ev::Release | Ev::Die);
    if asks && abs.stop != "none" {
        let terminal = matches!(abs.stop, "answered" | "failed");
        assert_eq!(refused, terminal || abs.held, "{path:?}");
    }
    // Answered and failed are final.
    if matches!(abs.stop, "answered" | "failed") && e != Ev::Die {
        assert_eq!(after.stop, abs.stop, "{path:?}");
    }
    (n, after)
}

/// Breadth-first from a fresh world to [`DEPTH`]: each state with the shortest path to it. A
/// layer's moves run on every core, each with its own runtime and copy; merged in order.
fn explore() -> BTreeMap<Abs, Vec<Ev>> {
    let start = Node::start();
    start.check(&[]);
    let mut seen = BTreeMap::from([(start.abs(), Vec::new())]);
    let mut frontier = vec![(start.abs(), start)];
    let threads = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
    for _ in 0..DEPTH {
        let jobs: Vec<(usize, Ev)> = (0..frontier.len())
            .flat_map(|i| EVENTS.map(|e| (i, e)))
            .collect();
        let taken = AtomicUsize::new(0);
        let done: Mutex<Vec<(usize, Node, Abs)>> = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        let k = taken.fetch_add(1, Ordering::SeqCst);
                        let Some(&(i, e)) = jobs.get(k) else { break };
                        let (abs, node) = &frontier[i];
                        let mut path = seen[abs].clone();
                        path.push(e);
                        let (n, after) = advance(node, abs, &path);
                        done.lock().unwrap().push((k, n, after));
                    }
                });
            }
        });
        let mut done = done.into_inner().unwrap();
        done.sort_by_key(|(k, _, _)| *k);
        let mut next = Vec::new();
        for (k, n, after) in done {
            let (i, e) = jobs[k];
            let mut path = seen[&frontier[i].0].clone();
            path.push(e);
            if let std::collections::btree_map::Entry::Vacant(v) = seen.entry(after.clone()) {
                v.insert(path);
                next.push((after, n));
            }
        }
        frontier = next;
    }
    println!("run: {} states to depth {DEPTH}", seen.len());
    seen
}

#[test]
fn a_run_reaches_its_stops_and_every_answer_passes_the_end_gate() {
    let states = explore();
    let find = |what: &str, f: &dyn Fn(&Abs) -> bool| {
        states
            .iter()
            .filter(|(a, _)| f(a))
            .map(|(_, p)| p.clone())
            .min_by_key(Vec::len)
            .unwrap_or_else(|| panic!("unreachable: {what}"))
    };
    let stops: BTreeSet<&str> = states.keys().map(|a| a.stop).collect();
    assert_eq!(
        stops,
        [
            "answered",
            "checkpoint",
            "cut",
            "failed",
            "interrupted",
            "none"
        ]
        .into()
    );
    assert_eq!(find("answered", &|a| a.stop == "answered"), [Ev::Answer]);
    // Each stop resumes, through the same door, to an answer the end gate passed.
    for (stop, ev) in [
        ("checkpoint", Ev::Checkpoint),
        ("interrupted", Ev::Interrupt),
        ("cut", Ev::Die),
    ] {
        let path = find(stop, &|a| a.resumed == Some(stop) && a.stop == "answered");
        assert!(
            path.contains(&ev) && path.last() == Some(&Ev::Answer),
            "{path:?}"
        );
    }
    find("checkpointed, held by another session", &|a| {
        a.stop == "checkpoint" && a.held
    });
    find("resumed, then checkpointed again", &|a| {
        a.resumed.is_some() && a.stop == "checkpoint"
    });
    find("resumed, then failed", &|a| {
        a.resumed.is_some() && a.stop == "failed"
    });
    // A run that never stopped resumable is never resumed.
    for (a, path) in &states {
        if a.resumed.is_some() {
            let first_stop = path
                .iter()
                .position(|e| matches!(e, Ev::Checkpoint | Ev::Interrupt | Ev::Die));
            assert!(first_stop.is_some(), "{path:?}: resumed with no stop");
        }
    }
}
