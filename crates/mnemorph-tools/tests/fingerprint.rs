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

fn fp(act: &str, world: &Path) -> Fingerprint {
    Fingerprint::of(act, world).expect("the plan reads")
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

/// A write in the world can reroute an approved push without touching HEAD, the diff or the
/// command: each way is a changed target.
#[test]
fn a_rerouted_push_is_asked_again() {
    let reroutes: [&[&str]; 3] = [
        &["remote", "set-url", "origin", "ELSEWHERE"],
        &["config", "remote.origin.pushurl", "ELSEWHERE"],
        &["config", "url.ELSEWHERE.pushInsteadOf", "ORIGIN"],
    ];
    for (i, reroute) in reroutes.iter().enumerate() {
        let w = World::new(&format!("reroute{i}"));
        let world = w.world();
        let origin = w.root.join("origin.git").to_str().unwrap().to_owned();
        let elsewhere = w.root.join("elsewhere.git").to_str().unwrap().to_owned();
        let args: Vec<String> = reroute
            .iter()
            .map(|a| {
                a.replace("ELSEWHERE", &elsewhere)
                    .replace("ORIGIN", &origin)
            })
            .collect();
        let gate = Late::new(vec![
            (
                Some(Box::new(move || {
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    git(&world, &args);
                })),
                Answer::Approve,
            ),
            (None, Answer::Deny("no".into())),
        ]);
        let o = push(&mut w.cx(gate.clone()));
        assert!(o.is_error, "{reroute:?}: {o:?}");
        assert!(
            !w.pushed_to("origin.git") && !w.pushed_to("elsewhere.git"),
            "{reroute:?}"
        );
        let asked = gate.asked();
        assert_eq!(asked.len(), 2, "{reroute:?}");
        assert!(
            asked[1].why[0].contains("the target changed"),
            "{reroute:?}"
        );
    }
}

/// A change that steers what the push runs or where it goes, made while the card waits.
fn re_asked(name: &str, meanwhile: impl FnOnce(&Path) + Send + 'static, part: &str) {
    let w = World::new(name);
    let world = w.world();
    let gate = Late::new(vec![
        (Some(Box::new(move || meanwhile(&world))), Answer::Approve),
        (None, Answer::Deny("no".into())),
    ]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(o.is_error, "{name}: {o:?}");
    assert!(
        !w.pushed_to("origin.git") && !w.pushed_to("elsewhere.git"),
        "{name}"
    );
    let asked = gate.asked();
    assert_eq!(asked.len(), 2, "{name}");
    assert!(asked[1].why[0].contains(part), "{name}: {:?}", asked[1].why);
}

/// Git's executables are part of the plan: a hook, an ssh command or an alias planted after the
/// approval would run with it.
#[test]
fn git_planted_with_code_after_approval_is_asked_again() {
    re_asked(
        "hook",
        |world| {
            let hook = world.join(".git/hooks/pre-push");
            fs::write(&hook, "#!/bin/sh\necho planted\n").unwrap();
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        },
        "git's configuration, hooks or info changed",
    );
    re_asked(
        "ssh",
        |world| {
            git(
                world,
                &["config", "core.sshCommand", "sh -c 'echo planted'"],
            );
        },
        "git's configuration, hooks or info changed",
    );
    re_asked(
        "alias",
        |world| {
            git(world, &["config", "alias.push", "!echo planted"]);
        },
        "git's configuration, hooks or info changed",
    );
    re_asked(
        "exclude",
        |world| fs::write(world.join(".git/info/exclude"), "src/\n").unwrap(),
        "git's configuration, hooks or info changed",
    );
}

/// The publish files are bound by name, ignored or not.
#[test]
fn an_ignored_npmrc_changed_after_approval_is_asked_again() {
    let w = World::new("npmrc");
    let world = w.world();
    fs::write(world.join(".gitignore"), ".npmrc\n.env*\n").unwrap();
    fs::write(
        world.join(".npmrc"),
        "registry=https://registry.npmjs.org/\n",
    )
    .unwrap();
    fs::write(world.join("src/.env.local"), "A=1\n").unwrap();
    let before = fp("npm publish", &world);
    fs::write(world.join(".npmrc"), "registry=https://evil.example/\n").unwrap();
    let npmrc = fp("npm publish", &world);
    assert_ne!(
        before.diff, npmrc.diff,
        "an ignored .npmrc is part of the plan"
    );
    fs::write(world.join("src/.env.local"), "A=2\n").unwrap();
    assert_ne!(
        npmrc.diff,
        fp("npm publish", &world).diff,
        "an ignored .env.* too"
    );
    // And through the gate.
    let gate = Late::new(vec![
        (
            Some(Box::new(move || {
                fs::write(world.join(".npmrc"), "registry=https://other.example/\n").unwrap();
            })),
            Answer::Approve,
        ),
        (None, Answer::Deny("no".into())),
    ]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(o.is_error && !w.pushed_to("origin.git"), "{o:?}");
    assert_eq!(gate.asked().len(), 2);
}

/// A plan that cannot be read is no plan: the gate is not asked and nothing runs.
#[test]
fn a_corrupt_index_refuses() {
    let w = World::new("corrupt");
    fs::write(w.world().join(".git/index"), "garbage").unwrap();
    let gate = Late::new(vec![(None, Answer::Approve)]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(
        o.is_error && o.content.contains("could not be read") && o.content.contains("ls-files"),
        "{o:?}"
    );
    assert!(gate.asked().is_empty(), "the gate was not asked");
    assert!(!w.pushed_to("origin.git"));
}

/// A submodule's gitlink is in the index, its working tree is not: the plan follows it.
#[test]
fn a_dirty_submodule_after_approval_is_asked_again() {
    let w = World::new("submodule");
    let world = w.world();
    let sub = world.join("vendor");
    fs::create_dir_all(&sub).unwrap();
    git(&sub, &["init", "-q"]);
    fs::write(sub.join("lib.txt"), "one\n").unwrap();
    git(&sub, &["add", "-A"]);
    git(&sub, &["commit", "-q", "-m", "lib"]);
    git(&world, &["add", "vendor"]);
    git(&world, &["commit", "-q", "-m", "vendor"]);
    let before = fp("git push origin feature", &world);
    fs::write(sub.join("lib.txt"), "dirty\n").unwrap();
    assert_ne!(before.diff, fp("git push origin feature", &world).diff);
    fs::write(sub.join("lib.txt"), "one\n").unwrap();
    let file = sub.join("lib.txt");
    let gate = Late::new(vec![
        (
            Some(Box::new(move || fs::write(file, "dirty\n").unwrap())),
            Answer::Approve,
        ),
        (None, Answer::Deny("no".into())),
    ]);
    let o = push(&mut w.cx(gate.clone()));
    assert!(o.is_error && !w.pushed_to("origin.git"), "{o:?}");
    let asked = gate.asked();
    assert_eq!(asked.len(), 2);
    assert!(
        asked[1].why[0].contains("the working tree changed"),
        "{:?}",
        asked[1].why
    );
}

/// A repository inside the world, at `vendor`, with a bare `vendor-origin.git` beside the world.
fn with_inner(w: &World, how: &str) -> PathBuf {
    let inner = w.world().join("vendor");
    fs::create_dir_all(&inner).unwrap();
    git(&inner, &["init", "-q"]);
    git(&inner, &["symbolic-ref", "HEAD", "refs/heads/feature"]);
    fs::write(inner.join("lib.txt"), "one\n").unwrap();
    git(&inner, &["add", "-A"]);
    git(&inner, &["commit", "-q", "-m", "lib"]);
    git(&w.root, &["init", "-q", "--bare", "vendor-origin.git"]);
    let origin = w.root.join("vendor-origin.git");
    git(
        &inner,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    match how {
        "submodule" => {
            git(&w.world(), &["add", "vendor"]);
            git(&w.world(), &["commit", "-q", "-m", "vendor"]);
        }
        "ignored" => fs::write(w.world().join(".gitignore"), "vendor/\n").unwrap(),
        _ => {}
    }
    inner
}

/// Approve `git -C vendor push`, let `meanwhile` change the inner repository, and expect a
/// re-ask naming `part`, with nothing pushed.
fn inner_re_asked(how: &str, meanwhile: fn(&Path), part: &str) {
    let w = World::new(&format!("inner-{how}-{}", part.len()));
    let inner = with_inner(&w, how);
    let gate = Late::new(vec![
        (Some(Box::new(move || meanwhile(&inner))), Answer::Approve),
        (None, Answer::Deny("no".into())),
    ]);
    let args = serde_json::json!({ "command": "git -C vendor push -q origin feature" }).to_string();
    let o = call(&registry(), "bash", &args, &mut w.cx(gate.clone()));
    assert!(o.is_error, "{how}: {o:?}");
    assert!(
        !w.pushed_to("vendor-origin.git") && !w.pushed_to("elsewhere.git"),
        "{how}"
    );
    let asked = gate.asked();
    assert_eq!(asked.len(), 2, "{how}: {o:?}");
    assert!(asked[1].why[0].contains(part), "{how}: {:?}", asked[1].why);
}

fn reroute(inner: &Path) {
    let elsewhere = inner
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("elsewhere.git");
    git(
        inner,
        &["remote", "set-url", "origin", elsewhere.to_str().unwrap()],
    );
}

fn plant_hook(inner: &Path) {
    let hook = inner.join(".git/hooks/pre-push");
    fs::write(&hook, "#!/bin/sh\necho planted\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A repository inside the world is bound whole: its remotes and its hooks, not only its tree.
#[test]
fn a_repository_inside_the_world_is_bound_whole() {
    inner_re_asked("submodule", reroute, "the target changed");
    inner_re_asked(
        "submodule",
        plant_hook,
        "git's configuration, hooks or info changed",
    );
    inner_re_asked("untracked", reroute, "the target changed");
    inner_re_asked(
        "untracked",
        plant_hook,
        "git's configuration, hooks or info changed",
    );
    // Ignored by the world's repository: bound because the command names it.
    inner_re_asked("ignored", reroute, "the target changed");
    inner_re_asked(
        "ignored",
        plant_hook,
        "git's configuration, hooks or info changed",
    );
    // Named by its git directory.
    let w = World::new("gitdir");
    let inner = with_inner(&w, "ignored");
    let act = "git --git-dir=vendor/.git push origin feature";
    let before = fp(act, &w.world());
    reroute(&inner);
    assert_ne!(before.target, fp(act, &w.world()).target);
}

/// The other publish files by name: `.yarnrc` and `.cargo/config.toml`, ignored.
#[test]
fn ignored_yarnrc_and_cargo_config_are_part_of_the_plan() {
    let w = World::new("publish-files");
    let world = w.world();
    fs::write(world.join(".gitignore"), ".yarnrc\n.cargo/\n").unwrap();
    fs::create_dir_all(world.join(".cargo")).unwrap();
    fs::write(
        world.join(".yarnrc"),
        "registry \"https://registry.yarnpkg.com\"\n",
    )
    .unwrap();
    fs::write(world.join(".cargo/config.toml"), "[registry]\n").unwrap();
    let before = fp("cargo publish", &world);
    fs::write(world.join(".yarnrc"), "registry \"https://evil.example\"\n").unwrap();
    let yarnrc = fp("cargo publish", &world);
    assert_ne!(
        before.diff, yarnrc.diff,
        "an ignored .yarnrc is part of the plan"
    );
    fs::write(
        world.join(".cargo/config.toml"),
        "[registries.evil]\nindex = \"https://evil.example\"\n",
    )
    .unwrap();
    assert_ne!(
        yarnrc.diff,
        fp("cargo publish", &world).diff,
        "an ignored .cargo/config.toml too"
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
    let base = fp(act, &world);
    assert_eq!(base, fp(act, &world), "deterministic");
    assert!(base.head.is_some());

    // Untracked, not ignored.
    fs::write(world.join("new.txt"), "x").unwrap();
    let untracked = fp(act, &world);
    assert_ne!(base.diff, untracked.diff);
    // Ignored files are build output, not the plan.
    fs::write(world.join(".gitignore"), "out/\n").unwrap();
    let ignoring = fp(act, &world);
    fs::create_dir_all(world.join("out")).unwrap();
    fs::write(world.join("out/build.bin"), "x").unwrap();
    assert_eq!(ignoring, fp(act, &world));
    // Staging the same bytes changes the index, so the plan.
    git(&world, &["add", "src/a.txt"]);
    let staged = fp(act, &world);
    assert_ne!(ignoring.diff, staged.diff);
    // The executable bit is part of a file.
    fs::set_permissions(world.join("new.txt"), fs::Permissions::from_mode(0o755)).unwrap();
    assert_ne!(staged.diff, fp(act, &world).diff);
    // A different act is a different plan.
    assert_ne!(
        fp(act, &world).id(),
        fp("git push origin main", &world).id()
    );
}

#[test]
fn a_world_with_no_repository_is_bound_to_its_files() {
    let root = std::env::temp_dir().join(format!("mnemorph-plan-bare-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "A").unwrap();
    let before = fp("deploy", &root);
    assert_eq!(before.head, None);
    fs::write(root.join("a.txt"), "B").unwrap();
    let after = fp("deploy", &root);
    assert_eq!(after.changes(&after), Vec::<String>::new());
    assert_ne!(before.diff, after.diff);
    let _ = fs::remove_dir_all(&root);
}
