//! An approval is bound to the plan the human saw: a world that changed between the answer and
//! the spawn runs nothing, and the gate asks again.

use std::collections::VecDeque;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, PoisonError};

use mnemorph_races::Race;
use mnemorph_sandbox::Sandbox;
use mnemorph_tools::gate::Gate;
use mnemorph_tools::{Answer, Ask, Context, Fingerprint, Outcome, call, registry};

/// A world that is a repository on branch `feature`, with a bare `origin` outside it.
struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-plan-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world/src")).unwrap();
        let w = Self { root };
        git(&w.root, &["init", "-q", "--bare", "origin.git"]);
        git(&w.root, &["init", "-q", "--bare", "elsewhere.git"]);
        git(&w.world(), &["init", "-q"]);
        git(&w.world(), &["symbolic-ref", "HEAD", "refs/heads/feature"]);
        fs::write(w.world().join("src/a.txt"), "one\n").unwrap();
        git(&w.world(), &["add", "-A"]);
        git(&w.world(), &["commit", "-q", "-m", "one"]);
        let origin = w.root.join("origin.git");
        git(
            &w.world(),
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        // Diff A: the change the human is shown.
        fs::write(w.world().join("src/a.txt"), "A\n").unwrap();
        w
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn cx(&self, gate: Arc<dyn Gate>) -> Context {
        Context::new(Race::Kijin, self.world(), &["src"])
            .with_gate(gate)
            .with_home("/h")
            .with_sandbox(Sandbox::None("these tests check the gate's binding".into()))
            .with_unguarded(true)
    }

    /// Whether `remote` received the branch.
    fn pushed_to(&self, remote: &str) -> bool {
        Command::new("git")
            .args([
                "--git-dir",
                remote,
                "rev-parse",
                "-q",
                "--verify",
                "refs/heads/feature",
            ])
            .current_dir(&self.root)
            .output()
            .unwrap()
            .status
            .success()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

type Meanwhile = Box<dyn FnOnce() + Send>;

/// A gate whose human answers late: before each answer, `meanwhile` changes the world, as a
/// concurrent court would while the card waits.
#[derive(Default)]
struct Late {
    script: Mutex<VecDeque<(Option<Meanwhile>, Answer)>>,
    asked: Mutex<Vec<Ask>>,
}

impl Late {
    fn new(script: Vec<(Option<Meanwhile>, Answer)>) -> Arc<Self> {
        Arc::new(Self {
            script: Mutex::new(script.into()),
            asked: Mutex::default(),
        })
    }

    fn asked(&self) -> Vec<Ask> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Gate for Late {
    fn ask(&self, ask: &Ask) -> Answer {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(ask.clone());
        let next = self
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front();
        match next {
            Some((meanwhile, answer)) => {
                if let Some(change) = meanwhile {
                    change();
                }
                answer
            }
            None => Answer::Deny("the script ran out of answers".into()),
        }
    }
}

fn push(cx: &mut Context) -> Outcome {
    let args = serde_json::json!({ "command": "git push -q origin feature" }).to_string();
    call(&registry(), "bash", &args, cx)
}

fn plan(ask: &Ask) -> &Fingerprint {
    ask.fingerprint
        .as_ref()
        .expect("an outward ask carries its plan")
}

#[test]
fn a_push_approved_against_one_diff_never_runs_on_another() {
    let w = World::new("diff");
    let tree = w.world().join("src/a.txt");
    let gate = Late::new(vec![
        // Diff B lands while the human reads diff A's card.
        (
            Some(Box::new(move || fs::write(tree, "B\n").unwrap())),
            Answer::Approve,
        ),
        (None, Answer::Deny("not this one".into())),
    ]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(o.is_error && o.meta.denied, "{o:?}");
    assert!(
        !w.pushed_to("origin.git"),
        "the push ran on a world nobody approved"
    );
    let asked = gate.asked();
    assert_eq!(asked.len(), 2, "asked again");
    assert!(
        asked[1].why[0].starts_with("the world changed since approval")
            && asked[1].why[0].contains("the working tree changed"),
        "{:?}",
        asked[1].why
    );
    assert_eq!(plan(&asked[0]).head, plan(&asked[1]).head);
    assert_ne!(plan(&asked[0]).diff, plan(&asked[1]).diff);
    assert!(
        o.content.contains("did not run and the gate asked again")
            && o.content.contains(&plan(&asked[0]).id())
            && o.content.contains(&plan(&asked[1]).short()),
        "the journal keeps both plans: {o:?}"
    );
}

#[test]
fn a_second_approval_binds_to_the_world_as_it_is_now() {
    let w = World::new("again");
    let tree = w.world().join("src/a.txt");
    let gate = Late::new(vec![
        (
            Some(Box::new(move || fs::write(tree, "B\n").unwrap())),
            Answer::Approve,
        ),
        (None, Answer::Approve),
    ]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(!o.is_error, "{o:?}");
    assert!(w.pushed_to("origin.git"));
    let asked = gate.asked();
    assert_eq!(asked.len(), 2);
    assert!(
        o.content.contains(&format!(
            "approved by the human against {}, unchanged at the spawn",
            plan(&asked[1]).short()
        )),
        "{o:?}"
    );
}

#[test]
fn head_moved_is_asked_again() {
    let w = World::new("head");
    let world = w.world();
    let gate = Late::new(vec![
        (
            Some(Box::new(move || {
                git(
                    &world,
                    &["commit", "-q", "--allow-empty", "-m", "meanwhile"],
                );
            })),
            Answer::Approve,
        ),
        (None, Answer::Deny("no".into())),
    ]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(o.is_error, "{o:?}");
    assert!(!w.pushed_to("origin.git"));
    let asked = gate.asked();
    assert_eq!(asked.len(), 2);
    assert!(asked[1].why[0].contains("head moved"), "{:?}", asked[1].why);
}

#[test]
fn a_changed_target_is_asked_again() {
    let w = World::new("target");
    let world = w.world();
    let elsewhere = w.root.join("elsewhere.git");
    let gate = Late::new(vec![
        (
            Some(Box::new(move || {
                git(
                    &world,
                    &["remote", "set-url", "origin", elsewhere.to_str().unwrap()],
                );
            })),
            Answer::Approve,
        ),
        (None, Answer::Deny("no".into())),
    ]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(o.is_error, "{o:?}");
    assert!(!w.pushed_to("origin.git") && !w.pushed_to("elsewhere.git"));
    let asked = gate.asked();
    assert_eq!(asked.len(), 2);
    assert!(
        asked[1].why[0].contains("the target changed")
            && !asked[1].why[0].contains("head moved")
            && !asked[1].why[0].contains("working tree"),
        "{:?}",
        asked[1].why
    );
}

#[test]
fn an_unchanged_world_runs_once() {
    let w = World::new("same");
    let gate = Late::new(vec![(None, Answer::Approve)]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(!o.is_error, "{o:?}");
    assert!(w.pushed_to("origin.git"));
    let asked = gate.asked();
    assert_eq!(asked.len(), 1, "asked once");
    assert!(o.content.contains(&plan(&asked[0]).short()), "{o:?}");
}

#[test]
fn a_world_that_keeps_changing_never_runs() {
    let w = World::new("churn");
    let script = (0..10)
        .map(|i| {
            let tree = w.world().join("src/a.txt");
            let change: Meanwhile = Box::new(move || fs::write(tree, format!("{i}\n")).unwrap());
            (Some(change), Answer::Approve)
        })
        .collect();
    let gate = Late::new(script);
    let o = push(&mut w.cx(gate.clone()));
    assert!(
        o.is_error && o.content.contains("the command did not run"),
        "{o:?}"
    );
    assert!(!w.pushed_to("origin.git"));
    assert_eq!(
        gate.asked().len(),
        4,
        "asked again a bounded number of times"
    );
}

#[test]
fn the_fingerprint_sees_staged_unstaged_and_untracked_changes() {
    let w = World::new("parts");
    let world = w.world();
    let act = "git push origin feature";
    let base = Fingerprint::of(act, &world);
    assert_eq!(base, Fingerprint::of(act, &world), "deterministic");
    assert!(base.head.is_some());

    // Untracked, not ignored.
    fs::write(world.join("new.txt"), "x").unwrap();
    let untracked = Fingerprint::of(act, &world);
    assert_ne!(base.diff, untracked.diff);
    // Ignored files are build output, not the plan.
    fs::write(world.join(".gitignore"), "out/\n").unwrap();
    let ignoring = Fingerprint::of(act, &world);
    fs::create_dir_all(world.join("out")).unwrap();
    fs::write(world.join("out/build.bin"), "x").unwrap();
    assert_eq!(ignoring, Fingerprint::of(act, &world));
    // Staging the same bytes changes the index, so the plan.
    git(&world, &["add", "src/a.txt"]);
    let staged = Fingerprint::of(act, &world);
    assert_ne!(ignoring.diff, staged.diff);
    // The executable bit is part of a file.
    fs::set_permissions(world.join("new.txt"), fs::Permissions::from_mode(0o755)).unwrap();
    assert_ne!(staged.diff, Fingerprint::of(act, &world).diff);
    // A different act is a different plan.
    assert_ne!(
        Fingerprint::of(act, &world).id(),
        Fingerprint::of("git push origin main", &world).id()
    );
}

#[test]
fn a_world_with_no_repository_is_bound_to_its_files() {
    let root = std::env::temp_dir().join(format!("mnemorph-plan-bare-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "A").unwrap();
    let before = Fingerprint::of("deploy", &root);
    assert_eq!(before.head, None);
    fs::write(root.join("a.txt"), "B").unwrap();
    let after = Fingerprint::of("deploy", &root);
    assert_eq!(after.changes(&after), Vec::<String>::new());
    assert_ne!(before.diff, after.diff);
    let _ = fs::remove_dir_all(&root);
}
