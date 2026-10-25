//! The bash tool: classified, shelved, gated, contained in a process group.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mnemorph_races::{Race, ToolClass};
use mnemorph_sandbox::Sandbox;
use mnemorph_tools::gate::Scripted;
use mnemorph_tools::{Answer, Context, Outcome, Rule, call, registry};

struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-shell-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world/src")).unwrap();
        fs::create_dir_all(root.join("outside")).unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn outside(&self) -> PathBuf {
        self.root.join("outside")
    }

    fn cx(&self, race: Race, gate: Arc<Scripted>) -> Context {
        Context::new(race, self.world(), &["src"])
            .with_gate(gate)
            .with_home("/h")
            .with_sandbox(Sandbox::None(
                "these tests check the gate and the process group".into(),
            ))
            .with_unguarded(true)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn bash(cx: &mut Context, command: &str) -> Outcome {
    let args = serde_json::json!({ "command": command }).to_string();
    call(&registry(), "bash", &args, cx)
}

#[test]
fn a_read_runs_without_asking() {
    let w = World::new("read");
    fs::write(w.world().join("src/a.txt"), "hello\n").unwrap();
    let gate = Arc::new(Scripted::new([]));
    let o = bash(&mut w.cx(Race::Slime, gate.clone()), "cat src/a.txt");
    assert_eq!(o.content, "hello\n");
    assert_eq!(o.meta.class, Some(ToolClass::Read));
    assert_eq!(o.meta.exit, Some(0));
    assert_eq!(gate.asked(), [] as [mnemorph_tools::Ask; 0]);
}

#[test]
fn a_refused_command_never_runs() {
    let w = World::new("refuse");
    let marker = w.world().join("src/ran");
    let gate = Arc::new(Scripted::new([Answer::Approve]));
    let o = bash(
        &mut w.cx(Race::Kijin, gate.clone()),
        &format!("touch {} && sed -i s/a/b/ x", marker.display()),
    );
    assert!(
        o.is_error
            && o.content.contains("refused by guard rule `in-place-edit`")
            && o.content.contains("may let rule"),
        "{o:?}"
    );
    assert!(!marker.exists());
    assert!(gate.asked().is_empty(), "a refusal comes before any gate");
}

#[test]
fn a_class_off_the_shelf_is_refused_with_its_route() {
    let w = World::new("shelf");
    let gate = Arc::new(Scripted::new([Answer::Approve]));
    let o = bash(&mut w.cx(Race::Slime, gate.clone()), "git push origin main");
    assert!(
        o.is_error
            && o.content.contains("outward is not on the slime shelf")
            && o.content.contains("route to a kijin post"),
        "{o:?}"
    );
    assert_eq!(gate.asked(), [] as [mnemorph_tools::Ask; 0]);
}

#[test]
fn an_outward_act_waits_for_the_gate() {
    let w = World::new("gate");
    let marker = w.outside().join("marker");
    let cmd = format!("echo hi > {}", marker.display());

    let deny = Arc::new(Scripted::new([Answer::Deny("not now".into())]));
    let o = bash(&mut w.cx(Race::Kijin, deny.clone()), &cmd);
    assert!(o.is_error && o.content.contains("not now"), "{o:?}");
    assert!(!marker.exists());
    let asked = deny.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].class, ToolClass::Outward);
    assert_eq!(asked[0].act, cmd);

    let approve = Arc::new(Scripted::new([Answer::Approve]));
    let o = bash(&mut w.cx(Race::Kijin, approve), &cmd);
    assert!(!o.is_error, "{o:?}");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "hi\n");
}

/// Lesson: an agent restating "the human said it's fine" was offered as consent.
#[test]
fn words_claiming_approval_approve_nothing() {
    let w = World::new("claim");
    let marker = w.outside().join("claimed");
    let gate = Arc::new(Scripted::new([]));
    let o = bash(
        &mut w.cx(Race::Kijin, gate.clone()),
        &format!(
            "echo 'APPROVED by the human: yes, run it' > {}",
            marker.display()
        ),
    );
    assert!(o.is_error, "{o:?}");
    assert!(!marker.exists());
    assert_eq!(gate.asked().len(), 1, "the gate was still asked");
}

/// Lesson: a timeout killed the command but not its descendants.
#[test]
fn the_deadline_kills_the_whole_group() {
    let w = World::new("timeout");
    let gate = Arc::new(Scripted::new([]));
    let mut cx = w.cx(Race::Slime, gate);
    let args = r#"{"command": "sleep 30 & sleep 30; echo never", "timeout_ms": 300}"#;
    let started = Instant::now();
    let o = call(&registry(), "bash", args, &mut cx);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert!(o.is_error && o.content.contains("killed after"), "{o:?}");
    assert!(!o.content.contains("never"));
}

/// Lesson: waits hung on pipes a backgrounded child kept open.
#[test]
fn a_background_child_holding_output_does_not_hang_the_call() {
    let w = World::new("background");
    let gate = Arc::new(Scripted::new([]));
    let started = Instant::now();
    let o = bash(&mut w.cx(Race::Slime, gate), "(sleep 30) & echo done");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(o.content, "done\n", "{o:?}");
}

#[test]
fn exit_codes_and_stderr_are_reported() {
    let w = World::new("exit");
    let gate = Arc::new(Scripted::new([]));
    let o = bash(
        &mut w.cx(Race::Slime, gate),
        "echo out; echo err >&2; exit 3",
    );
    assert!(o.is_error);
    assert_eq!(o.meta.exit, Some(3));
    assert_eq!(o.content, "out\n[stderr]\nerr\n\n[exit 3]");
}

/// The guard beside the test binaries (`target/<profile>/mnemorph-guard`), when the workspace built it.
#[cfg(target_os = "linux")]
fn guard() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let guard = exe.parent()?.parent()?.join("mnemorph-guard");
    guard.is_file().then_some(guard)
}

/// The classifier judges words; the sandbox stops what words hide. An approval opens the
/// network, never the filesystem beyond the world.
#[cfg(target_os = "linux")]
#[test]
fn bash_runs_inside_the_guard() {
    let Some(guard) = guard() else {
        eprintln!("skipped: build the workspace so target/<profile>/mnemorph-guard exists");
        return;
    };
    let w = World::new("guard");
    let home = w.root.join("home");
    fs::create_dir_all(home.join(".ssh")).unwrap();
    fs::write(home.join(".ssh/id_ed25519"), "PRIVATE KEY\n").unwrap();
    fs::write(
        w.world().join("src/peek.sh"),
        "cat \"$1\"/.ssh/id_ed25519\n",
    )
    .unwrap();
    let cx = |race, gate| {
        Context::new(race, w.world(), &["src"])
            .with_gate(gate)
            .with_home(&home)
            .with_sandbox(Sandbox::Guard(guard.clone()))
    };

    let o = bash(
        &mut cx(Race::Slime, Arc::new(Scripted::new([]))),
        "echo built > src/out.txt && cat src/out.txt",
    );
    assert_eq!(o.content, "built\n", "{o:?}");

    let o = bash(
        &mut cx(Race::Slime, Arc::new(Scripted::new([]))),
        &format!("bash src/peek.sh {}", home.display()),
    );
    assert!(o.is_error && !o.content.contains("PRIVATE KEY"), "{o:?}");

    // Approved, an act may write exactly the outside paths it named, and the card showed them.
    let marker = w.outside().join("approved");
    let gate = Arc::new(Scripted::new([Answer::Approve]));
    let o = bash(
        &mut cx(Race::Kijin, gate.clone()),
        &format!("echo hi > {}", marker.display()),
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "hi\n");
    assert_eq!(gate.asked()[0].outside, vec![marker.clone()]);

    // A new file opens alone — never its folder: a sibling written in the same command stays shut.
    let fresh = w.outside().join("fresh.txt");
    let sibling = w.outside().join("sibling.txt");
    let o = bash(
        &mut cx(Race::Kijin, Arc::new(Scripted::new([Answer::Approve]))),
        &format!(
            "echo new > {}; f={}; echo sneaky > \"$f\"",
            fresh.display(),
            sibling.display()
        ),
    );
    assert!(o.content.contains("Permission denied"), "{o:?}");
    assert_eq!(fs::read_to_string(&fresh).unwrap(), "new\n");
    assert!(!sibling.exists(), "approving a new file opened its folder");

    // A failed command leaves no empty file behind.
    let ghost = w.outside().join("ghost.txt");
    let o = bash(
        &mut cx(Race::Kijin, Arc::new(Scripted::new([Answer::Approve]))),
        &format!("false > {}", ghost.display()),
    );
    assert!(o.is_error);
    assert!(!ghost.exists());

    // Approval opens what was named, nothing beside it: a path the words hide stays shut.
    let hidden = w.outside().join("hidden");
    let o = bash(
        &mut cx(Race::Kijin, Arc::new(Scripted::new([Answer::Approve]))),
        &format!(
            "echo hi > {}; f={}; echo sneaky > \"$f\"",
            marker.display(),
            w.root.join("elsewhere").display()
        ),
    );
    assert!(o.content.contains("Permission denied"), "{o:?}");
    assert!(!w.root.join("elsewhere").exists() && !hidden.exists());
}

#[test]
fn every_rule_has_a_name_and_the_floor_is_four() {
    for rule in Rule::ALL {
        assert_eq!(Rule::from_name(rule.name()), Some(rule));
    }
    let floor: Vec<&str> = Rule::ALL
        .iter()
        .filter(|r| r.floor())
        .map(|r| r.name())
        .collect();
    assert_eq!(floor, ["device", "machine-tool", "root-path", "fork-bomb"]);
}

/// A rule the human configured asks instead of refusing; the answer decides.
#[test]
fn a_configured_rule_asks_the_human() {
    let w = World::new("asked");
    fs::write(w.world().join("src/f.txt"), "a\n").unwrap();
    let cx = |gate: Arc<Scripted>| {
        w.cx(Race::Slime, gate)
            .with_asked_rules([Rule::InPlaceEdit])
    };

    let deny = Arc::new(Scripted::new([Answer::Deny("no".into())]));
    let o = bash(&mut cx(deny.clone()), "sed -i.bak s/a/b/ src/f.txt");
    assert!(o.is_error, "{o:?}");
    assert!(
        deny.asked()[0].why[0].contains("guard rule `in-place-edit` asks"),
        "{:?}",
        deny.asked()
    );
    assert_eq!(
        fs::read_to_string(w.world().join("src/f.txt")).unwrap(),
        "a\n"
    );

    let approve = Arc::new(Scripted::new([Answer::Approve]));
    let o = bash(&mut cx(approve), "sed -i.bak s/a/b/ src/f.txt");
    assert!(!o.is_error, "{o:?}");
    assert_eq!(
        o.meta.class,
        Some(ToolClass::Write),
        "an in-place edit writes"
    );
    assert_eq!(
        fs::read_to_string(w.world().join("src/f.txt")).unwrap(),
        "b\n"
    );
}

/// The floor stays refused whatever the configuration lists, and no gate is asked.
#[test]
fn the_floor_never_asks() {
    let w = World::new("floor");
    let gate = Arc::new(Scripted::new([Answer::Approve, Answer::Approve]));
    let mut cx = w.cx(Race::Kijin, gate.clone()).with_asked_rules(Rule::ALL);
    for cmd in ["rm -rf /", ":(){ :|:& };:"] {
        let o = bash(&mut cx, cmd);
        assert!(
            o.is_error
                && o.content.starts_with("refused by guard rule")
                && !o.content.contains("may let"),
            "{cmd}: {o:?}"
        );
    }
    assert_eq!(gate.asked(), [] as [mnemorph_tools::Ask; 0]);
}

fn git(world: &std::path::Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args([
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(world)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

/// Git's executable configuration runs outside any sandbox the next time the human runs git:
/// a command that plants it finds it put back.
#[test]
fn the_tripwire_restores_git_config_and_hooks() {
    let w = World::new("tripwire-git");
    git(&w.world(), &["init", "-q"]);
    let config = fs::read_to_string(w.world().join(".git/config")).unwrap();
    // Written directly, the classifier asks first; the tripwire is for what it cannot see.
    fs::write(
        w.world().join("plant.sh"),
        "printf '[core]\\n\\tfsmonitor = touch /tmp/pwned\\n' >> .git/config; printf '#!/bin/sh\\necho pwned\\n' > .git/hooks/pre-commit; chmod +x .git/hooks/pre-commit; echo done\n",
    )
    .unwrap();
    let gate = Arc::new(Scripted::new([]));
    let o = bash(&mut w.cx(Race::Slime, gate.clone()), "bash plant.sh");
    assert!(
        o.is_error && o.content.contains("changed protected files"),
        "{o:?}"
    );
    assert_eq!(
        fs::read_to_string(w.world().join(".git/config")).unwrap(),
        config
    );
    assert!(!w.world().join(".git/hooks/pre-commit").exists());

    fs::write(
        w.world().join("swap.sh"),
        "rm -rf .git/hooks && ln -s /tmp .git/hooks\n",
    )
    .unwrap();
    let o = bash(&mut w.cx(Race::Slime, gate.clone()), "bash swap.sh");
    assert!(o.is_error, "{o:?}");
    assert!(
        w.world().join(".git/hooks").is_dir()
            && !fs::symlink_metadata(w.world().join(".git/hooks"))
                .unwrap()
                .file_type()
                .is_symlink()
    );

    // Ordinary work does not trip it.
    fs::write(w.world().join("src/a.rs"), "fn a() {}\n").unwrap();
    let o = bash(
        &mut w.cx(Race::Slime, gate),
        "git add -A && git -c user.email=t@t -c user.name=t commit -qm a && git log --oneline | wc -l",
    );
    assert!(!o.is_error && o.content.trim().ends_with('1'), "{o:?}");
}

#[test]
fn the_tripwire_keeps_the_worlds_records_and_spares_tmp() {
    let w = World::new("tripwire-records");
    fs::create_dir_all(w.world().join(".mnemorph")).unwrap();
    fs::write(w.world().join(".mnemorph/log.md"), "the truth\n").unwrap();
    fs::write(
        w.world().join("forge.sh"),
        "echo forged > .mnemorph/log.md; touch .mnemorph/new.toml; echo scratch > .mnemorph/tmp/x\n",
    )
    .unwrap();
    let gate = Arc::new(Scripted::new([]));
    let o = bash(&mut w.cx(Race::Slime, gate.clone()), "bash forge.sh");
    assert!(o.is_error, "{o:?}");
    assert_eq!(
        fs::read_to_string(w.world().join(".mnemorph/log.md")).unwrap(),
        "the truth\n"
    );
    assert!(!w.world().join(".mnemorph/new.toml").exists());
    assert!(
        w.world().join(".mnemorph/tmp/x").exists(),
        "tmp is the command's own"
    );

    let o = bash(
        &mut w.cx(Race::Slime, gate),
        "echo scratch > .mnemorph/tmp/y && mkdir -p .mnemorph/shared && echo '{}' >> .mnemorph/shared/queue.jsonl",
    );
    // The shared work queue moves only through the harness: a command that writes it touches the
    // world's records, so it is refused before it runs.
    assert!(o.is_error, "{o:?}");
    assert!(!w.world().join(".mnemorph/shared/queue.jsonl").exists());
}

#[test]
fn a_repository_the_command_creates_is_its_own() {
    let w = World::new("tripwire-init");
    let gate = Arc::new(Scripted::new([]));
    let o = bash(
        &mut w.cx(Race::Slime, gate),
        "git init -q && git status --short | wc -l",
    );
    assert!(!o.is_error, "{o:?}");
    assert!(w.world().join(".git/config").exists());
}

/// With no kernel guard in force nothing runs — unless the human allowed it.
#[test]
fn no_guard_no_command() {
    let w = World::new("unguarded");
    let marker = w.world().join("src/ran");
    let mut cx = Context::new(Race::Slime, w.world(), &["src"])
        .with_sandbox(Sandbox::None("the guard helper is missing".into()));
    let o = bash(&mut cx, &format!("touch {}", marker.display()));
    assert!(
        o.is_error
            && o.content
                .contains("no kernel guard is in force (the guard helper is missing)"),
        "{o:?}"
    );
    assert!(o.content.contains("sandbox.allow_unguarded"));
    assert!(!marker.exists());
    let mut cx = cx.with_unguarded(true);
    let o = bash(&mut cx, &format!("touch {}", marker.display()));
    assert!(!o.is_error, "{o:?}");
    assert!(marker.exists());
}

/// Esc reaches a running command: the whole group stops within a second, and what it printed is
/// kept.
#[test]
fn esc_stops_a_running_command_and_keeps_its_output() {
    let w = World::new("cancel");
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut cx = w
        .cx(Race::Slime, Arc::new(Scripted::new([])))
        .with_cancel(Arc::clone(&flag));
    let firing = Arc::clone(&flag);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        firing.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let started = Instant::now();
    let o = bash(
        &mut cx,
        "echo started; sleep 30 & echo $! > src/pid; sleep 30; echo never",
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(
        o.is_error
            && o.content.starts_with("started\n")
            && o.content.contains("interrupted by the human"),
        "{o:?}"
    );
    assert!(!o.content.contains("never"));
    let pid = fs::read_to_string(w.world().join("src/pid")).unwrap();
    let alive = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .status()
        .unwrap()
        .success();
    assert!(!alive, "the backgrounded child died with its group");
}

/// The run's clock reaches a running command, whatever the call's own limit.
#[test]
fn the_run_clock_stops_a_running_command() {
    let w = World::new("deadline");
    let mut cx = w.cx(Race::Slime, Arc::new(Scripted::new([])));
    cx.set_deadline(Some(Instant::now() + Duration::from_millis(300)));
    let started = Instant::now();
    let o = bash(&mut cx, "sleep 30");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(
        o.is_error && o.content.contains("killed after 0."),
        "it says when it was really killed: {o:?}"
    );
}

/// A commissioned creature's commands are granted exactly its soil: the guard cannot except a spot
/// inside an allowed folder, so the claim is granted part by part around another's spot, and the
/// kernel refuses a write there. The write tool refuses it by name.
#[cfg(target_os = "linux")]
#[test]
fn a_creatures_commands_are_granted_around_anothers_spot() {
    let Some(guard) = guard() else {
        eprintln!("skipped: build the workspace so target/<profile>/mnemorph-guard exists");
        return;
    };
    let w = World::new("grants");
    fs::create_dir_all(w.world().join("src/other")).unwrap();
    fs::create_dir_all(w.world().join("src/deep")).unwrap();
    fs::write(w.world().join("src/mine.rs"), "mine\n").unwrap();
    fs::write(w.world().join("src/deep/a.rs"), "a\n").unwrap();
    fs::write(w.world().join("src/other/theirs.rs"), "theirs\n").unwrap();
    fs::write(w.world().join("README.md"), "root\n").unwrap();
    let cx = || {
        Context::new(Race::Slime, w.world(), &["."])
            .with_soil(&["src/"], &["src/other/"], false)
            .with_build(&["target", ".mnemorph/instruments"])
            .with_gate(Arc::new(Scripted::new([])))
            .with_home(w.root.join("home"))
            .with_sandbox(Sandbox::Guard(guard.clone()))
    };
    // What git ignores is build output, granted — but never the records, even when ignored.
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(w.world())
            .args(args)
            .output()
            .unwrap()
    };
    git(&["init", "-q"]);
    fs::write(
        w.world().join(".gitignore"),
        "target/\n.mnemorph/instruments/\nsrc/other/\n",
    )
    .unwrap();
    fs::create_dir_all(w.world().join("target")).unwrap();
    fs::create_dir_all(w.world().join(".mnemorph/instruments")).unwrap();
    fs::write(w.world().join(".mnemorph/instruments/usage.jsonl"), "{}\n").unwrap();
    let grants = cx().grants();
    let world = w.world().canonicalize().unwrap();
    assert!(grants.contains(&world.join("src/mine.rs")), "{grants:?}");
    assert!(grants.contains(&world.join("src/deep")), "{grants:?}");
    assert!(
        grants
            .iter()
            .all(|g| !g.starts_with(world.join("src/other"))),
        "{grants:?}"
    );
    assert!(!grants.contains(&world), "{grants:?}");
    assert!(grants.contains(&world.join("target")), "{grants:?}");
    assert!(
        grants
            .iter()
            .all(|g| !g.starts_with(world.join(".mnemorph"))),
        "{grants:?}"
    );

    let o = bash(
        &mut cx(),
        "echo changed > src/mine.rs && echo new > src/deep/b.rs && cat README.md",
    );
    assert!(
        !o.is_error,
        "its own soil is its own, and the world is still read: {o:?}"
    );
    let o = bash(&mut cx(), "echo stolen > src/other/theirs.rs");
    assert!(o.is_error, "{o:?}");
    let o = bash(&mut cx(), "echo stolen > README.md");
    assert!(o.is_error, "{o:?}");
    let o = bash(&mut cx(), "echo built > target/out");
    assert!(!o.is_error, "build output is writable: {o:?}");
    let o = bash(
        &mut cx(),
        "echo forged >> .mnemorph/instruments/usage.jsonl",
    );
    assert!(o.is_error, "{o:?}");
    assert_eq!(
        fs::read_to_string(w.world().join("target/out")).unwrap(),
        "built\n"
    );
    assert_eq!(
        fs::read_to_string(w.world().join(".mnemorph/instruments/usage.jsonl")).unwrap(),
        "{}\n"
    );
    assert_eq!(
        fs::read_to_string(w.world().join("src/other/theirs.rs")).unwrap(),
        "theirs\n"
    );
    assert_eq!(
        fs::read_to_string(w.world().join("README.md")).unwrap(),
        "root\n"
    );

    let o = call(
        &registry(),
        "write",
        r#"{"path": "src/other/theirs.rs", "content": "stolen"}"#,
        &mut cx(),
    );
    assert!(
        o.is_error && o.content.contains("outside your soil"),
        "{o:?}"
    );
}

/// The guard resolves every rule to what it names, so a link in a creature's soil must never
/// grant its target: not another's spot, not another's zone, not a file outside the world.
#[cfg(target_os = "linux")]
#[test]
fn a_link_in_the_soil_grants_nothing_beyond_it() {
    let Some(guard) = guard() else {
        eprintln!("skipped: build the workspace so target/<profile>/mnemorph-guard exists");
        return;
    };
    let w = World::new("links");
    fs::create_dir_all(w.world().join("src/other")).unwrap();
    fs::create_dir_all(w.world().join("lib")).unwrap();
    fs::write(w.world().join("src/other/theirs.rs"), "theirs\n").unwrap();
    fs::write(w.world().join("lib/zone.rs"), "zone\n").unwrap();
    fs::write(w.outside().join("secret"), "outside\n").unwrap();
    let link = |target: PathBuf, name: &str| {
        std::os::unix::fs::symlink(target, w.world().join(name)).unwrap();
    };
    link(w.world().join("src/other/theirs.rs"), "src/to-theirs");
    link(w.world().join("lib"), "src/to-lib");
    link(w.outside().join("secret"), "src/to-outside");
    let cx = || {
        Context::new(Race::Slime, w.world(), &["."])
            .with_soil(&["src/"], &["src/other/"], false)
            .with_gate(Arc::new(Scripted::new([])))
            .with_home(w.root.join("home"))
            .with_sandbox(Sandbox::Guard(guard.clone()))
    };
    let grants = cx().grants();
    let world = w.world().canonicalize().unwrap();
    for never in [
        world.join("src/other/theirs.rs"),
        world.join("lib"),
        w.outside().join("secret").canonicalize().unwrap(),
    ] {
        assert!(
            !grants.contains(&never),
            "{never:?} granted through a link: {grants:?}"
        );
    }
    for (through, file) in [
        ("src/to-theirs", w.world().join("src/other/theirs.rs")),
        ("src/to-lib/zone.rs", w.world().join("lib/zone.rs")),
        ("src/to-outside", w.outside().join("secret")),
    ] {
        let before = fs::read_to_string(&file).unwrap();
        let o = bash(&mut cx(), &format!("echo stolen > {through}"));
        assert!(o.is_error, "{through}: {o:?}");
        assert_eq!(fs::read_to_string(&file).unwrap(), before, "{through}");
    }
}

/// What the review of the grants found, each held: a link handed as build output grants
/// nothing; a scratch nest that is a link is never used, and each creature's fallback scratch is
/// its own; hundreds of grants still start; `truncate` by path is refused where landlock cannot.
#[cfg(target_os = "linux")]
#[test]
fn build_links_scratch_many_grants_and_truncate() {
    let Some(guard) = guard() else {
        eprintln!("skipped: build the workspace so target/<profile>/mnemorph-guard exists");
        return;
    };
    let w = World::new("review");
    fs::create_dir_all(w.world().join("lib")).unwrap();
    fs::create_dir_all(w.world().join("src/other")).unwrap();
    fs::create_dir_all(w.world().join(".mnemorph/tmp/slime-b")).unwrap();
    fs::write(w.world().join("lib/zone.rs"), "zone\n").unwrap();
    std::os::unix::fs::symlink(w.world().join("lib"), w.world().join("src/x.log")).unwrap();
    std::os::unix::fs::symlink(
        w.world().join(".mnemorph/tmp/slime-b"),
        w.world().join(".mnemorph/tmp/slime-a"),
    )
    .unwrap();
    for i in 0..600 {
        fs::write(w.world().join(format!("src/f{i}.rs")), "x\n").unwrap();
    }
    let cx = || {
        Context::new(Race::Slime, w.world(), &["."])
            .with_soil(&["src/", ".mnemorph/tmp/slime-a/"], &["src/other/"], false)
            .with_build(&["src/x.log"])
            .with_gate(Arc::new(Scripted::new([])))
            .with_home(w.root.join("home"))
            .with_sandbox(Sandbox::Guard(guard.clone()))
    };
    let world = w.world().canonicalize().unwrap();
    let grants = cx().grants();
    assert!(grants.len() >= 600, "{}", grants.len());
    assert!(
        grants.iter().all(|g| !g.starts_with(world.join("lib"))),
        "a build link granted its target"
    );
    assert!(
        grants
            .iter()
            .all(|g| !g.starts_with(world.join(".mnemorph/tmp/slime-b")))
    );
    let (one, two) = (cx().scratch(), cx().scratch());
    assert!(!one.starts_with(&world) && one != two, "{one:?} {two:?}");
    // Hundreds of grants go through a rules file; the command starts and writes its own soil.
    let o = bash(&mut cx(), "echo y > src/f1.rs && cat src/f1.rs");
    assert_eq!(o.content, "y\n", "{o:?}");
    for cmd in [
        "echo stolen > src/x.log/zone.rs",
        "python3 -c \"import os; os.truncate('lib/zone.rs', 0)\"",
    ] {
        let o = bash(&mut cx(), cmd);
        assert!(o.is_error, "{cmd}: {o:?}");
    }
    assert_eq!(
        fs::read_to_string(w.world().join("lib/zone.rs")).unwrap(),
        "zone\n"
    );
}
