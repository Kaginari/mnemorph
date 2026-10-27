//! The attic: what commands and writes take away is kept outside the world, and comes back.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use mnemorph_races::Race;
use mnemorph_tools::gate::Scripted;
use mnemorph_tools::{
    Act, Attic, Context, Outcome, Restore, Sandbox, Snapshot, State, call, registry,
};

struct Ground {
    root: PathBuf,
}

impl Ground {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-attic-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world/src")).unwrap();
        fs::write(root.join("world/src/a.rs"), "fn a() {}\n").unwrap();
        fs::write(root.join("world/src/b.rs"), "fn b() {}\n").unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn attic(&self) -> Attic {
        Attic::at(&self.root.join("attic"), &self.world()).expect("git is installed")
    }

    fn cx(&self) -> Context {
        Context::new(Race::Slime, self.world(), &["."])
            .with_gate(Arc::new(Scripted::new([])))
            .with_home("/h")
            .with_sandbox(Sandbox::None("attic tests run unguarded".into()))
            .with_unguarded(true)
            .with_attic(self.attic())
    }
}

impl Drop for Ground {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn bash(cx: &mut Context, command: &str) -> Outcome {
    call(
        &registry(),
        "bash",
        &serde_json::json!({ "command": command }).to_string(),
        cx,
    )
}

fn tree_of(o: &Outcome) -> Snapshot {
    let at = o
        .content
        .find("snapshot ")
        .expect("the outcome names its snapshot")
        + "snapshot ".len();
    let short: String = o.content[at..]
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect();
    Snapshot {
        tree: short,
        skipped: Vec::new(),
    }
}

fn git(world: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
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
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_deleted_folder_is_reported_and_comes_back() {
    let g = Ground::new("rm");
    let mut cx = g.cx();
    let o = bash(&mut cx, "rm -rf src");
    assert!(
        o.content.contains("deleted 2 file(s) — kept in the attic"),
        "{o:?}"
    );
    assert!(!g.world().join("src").exists());
    let act = g.attic().seal(tree_of(&o), "after rm").unwrap();
    assert_eq!(
        g.attic().restore(&act, &["src"]).unwrap(),
        Restore::Restored(vec!["src/a.rs".into(), "src/b.rs".into()])
    );
    assert_eq!(
        fs::read_to_string(g.world().join("src/a.rs")).unwrap(),
        "fn a() {}\n"
    );
}

/// A script's deletions are not hidden by the script: the tree tells.
#[test]
fn a_script_cannot_hide_what_it_deletes() {
    let g = Ground::new("script");
    fs::write(g.world().join("wipe.sh"), "rm -rf src\n").unwrap();
    let mut cx = g.cx();
    let o = bash(&mut cx, "bash wipe.sh");
    assert!(o.content.contains("deleted 2 file(s)"), "{o:?}");
}

/// History survives an `rm -rf .git` hidden in a script: the attic keeps the objects by hard
/// link and the refs by copy, and puts the repository back.
#[test]
fn the_repository_survives_its_deletion() {
    let g = Ground::new("dotgit");
    git(&g.world(), &["init", "-q"]);
    git(&g.world(), &["add", "-A"]);
    git(&g.world(), &["commit", "-qm", "first"]);
    fs::write(g.world().join("wipe.sh"), "rm -rf .git\n").unwrap();
    let mut cx = g.cx();
    let o = bash(&mut cx, "bash wipe.sh");
    assert!(o.content.contains("put back from the attic"), "{o:?}");
    assert_eq!(git(&g.world(), &["log", "--format=%s"]).trim(), "first");
}

/// Deleting the repository or the world's records outright asks the human.
#[test]
fn deleting_the_records_directly_is_destructive() {
    let g = Ground::new("direct");
    let mut cx = g.cx();
    let o = bash(&mut cx, "rm -rf .git");
    assert!(
        o.is_error && o.content.contains("destructive is not on the slime shelf"),
        "{o:?}"
    );
}

#[test]
fn an_overwritten_file_comes_back() {
    let g = Ground::new("write");
    let attic = g.attic();
    let mut cx = g.cx();
    let o = call(
        &registry(),
        "write",
        r#"{"path": "src/a.rs", "content": "replaced\n"}"#,
        &mut cx,
    );
    assert!(!o.is_error, "{o:?}");
    let log = fs::read_to_string(attic.dir().join("snapshots.log")).unwrap();
    let tree = log
        .lines()
        .last()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .to_owned();
    let act = attic
        .seal(
            Snapshot {
                tree,
                skipped: Vec::new(),
            },
            "after a write",
        )
        .unwrap();
    assert!(matches!(
        attic.restore(&act, &["src/a.rs"]).unwrap(),
        Restore::Restored(_)
    ));
    assert_eq!(
        fs::read_to_string(g.world().join("src/a.rs")).unwrap(),
        "fn a() {}\n"
    );
}

const DAY: u64 = 24 * 60 * 60;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn two_snapshots(g: &Ground) -> (Attic, Snapshot, Snapshot) {
    let attic = g.attic();
    let old = attic.snapshot("first").unwrap();
    fs::write(g.world().join("src/a.rs"), "fn a2() {}\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let new = attic.snapshot("second").unwrap();
    (attic, old, new)
}

/// Snapshots past the keep window go; the newest always stays.
#[test]
fn old_snapshots_are_pruned_and_the_newest_stays() {
    let g = Ground::new("prune-age");
    let (attic, old, new) = two_snapshots(&g);
    attic.prune(now() + DAY).unwrap();
    assert!(attic.holds(&old, "src/a.rs"), "a day later both are kept");
    attic.prune(now() + 15 * DAY).unwrap();
    assert!(
        !attic.holds(&old, "src/a.rs"),
        "past the window the old one is gone"
    );
    assert!(attic.holds(&new, "src/a.rs"));
}

/// Over its size, the attic drops the oldest snapshots first — never the newest.
#[test]
fn the_size_cap_drops_the_oldest() {
    let g = Ground::new("prune-size");
    let (attic, old, new) = two_snapshots(&g);
    let attic = attic.with_limits(14, 0);
    attic.prune(now()).unwrap();
    assert!(!attic.holds(&old, "src/a.rs"));
    assert!(attic.holds(&new, "src/a.rs"));
}

/// A kept `.git` object the world itself no longer has ages out with the window.
#[test]
fn kept_git_objects_age_out_once_the_world_dropped_them() {
    let g = Ground::new("prune-git");
    git(&g.world(), &["init", "-q"]);
    git(&g.world(), &["add", "-A"]);
    git(&g.world(), &["commit", "-qm", "first"]);
    let attic = g.attic();
    attic.snapshot("keep").unwrap();
    let mirror = attic.dir().join("dotgit/objects");
    let count = |d: &Path| walk(d).len();
    let before = count(&mirror);
    // The world drops one object (as its own gc would); the attic's copy is then the only name.
    let dropped = walk(&g.world().join(".git/objects"))
        .into_iter()
        .find(|p| {
            p.parent()
                .is_some_and(|d| d.file_name().is_some_and(|n| n.len() == 2))
        })
        .unwrap();
    fs::remove_file(&dropped).unwrap();
    attic.prune(now() + 15 * DAY).unwrap();
    assert_eq!(
        count(&mirror),
        before - 1,
        "only the object the world dropped is forgotten"
    );
}

fn walk(d: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in fs::read_dir(d).into_iter().flatten().filter_map(Result::ok) {
        if e.file_type().unwrap().is_dir() {
            out.extend(walk(&e.path()));
        } else {
            out.push(e.path());
        }
    }
    out
}

/// The attic keeps its own rules: build output and dependencies never, ignored or not; nothing
/// over its size ceiling; and what becomes ignored leaves it.
#[test]
fn the_attic_keeps_only_what_is_worth_keeping() {
    let g = Ground::new("bounded");
    fs::create_dir_all(g.world().join("target/debug")).unwrap();
    fs::write(g.world().join("target/debug/app"), "binary").unwrap();
    fs::write(g.world().join("big.bin"), vec![0u8; 9 * 1024 * 1024]).unwrap();
    fs::write(g.world().join("café.md"), "accents\n").unwrap();
    fs::write(g.world().join("log.txt"), "noise\n").unwrap();
    let attic = g.attic();
    let snap = attic.snapshot("first").unwrap();
    assert_eq!(
        snap.skipped,
        ["big.bin"],
        "the oversized file is named, not kept"
    );
    let listed = git_in(&attic, &["ls-tree", "-r", "-z", "--name-only", &snap.tree]);
    assert!(
        !listed.contains("target/") && !listed.contains("big.bin"),
        "{listed}"
    );
    assert!(listed.contains("café.md") && listed.contains("log.txt"));
    assert!(
        attic.deleted_since(&snap).is_empty(),
        "nothing is reported deleted — accents included"
    );
    fs::write(g.world().join(".gitignore"), "log.txt\n").unwrap();
    let snap = attic.snapshot("second").unwrap();
    assert!(
        !git_in(&attic, &["ls-tree", "-r", "-z", "--name-only", &snap.tree]).contains("log.txt"),
        "what became ignored left"
    );
}

fn git_in(attic: &Attic, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .env("GIT_DIR", attic.dir().join("store.git"))
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A destructive act the human approved is the human's: the attic does not undo it.
#[test]
fn an_approved_deletion_of_the_repository_stays_done() {
    let g = Ground::new("approved");
    git(&g.world(), &["init", "-q"]);
    let mut cx = Context::new(Race::Kijin, g.world(), &["."])
        .with_gate(Arc::new(Scripted::new([mnemorph_tools::Answer::Approve])))
        .with_home("/h")
        .with_sandbox(Sandbox::None("attic tests run unguarded".into()))
        .with_unguarded(true)
        .with_attic(g.attic());
    let o = bash(&mut cx, "rm -rf .git");
    assert!(!o.is_error, "{o:?}");
    assert!(!o.content.contains("put back"), "{o:?}");
    assert!(!g.world().join(".git").exists());
}

/// A nested repository is a gitlink in a snapshot — its commit's name, none of its files — so
/// the attic never says it holds it, nor any file inside it.
#[test]
fn a_nested_repository_is_never_said_to_be_held() {
    let g = Ground::new("nested");
    let sub = g.world().join("vendor/sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("lib.rs"), "fn lib() {}\n").unwrap();
    let git = |dir: &Path, args: &[&str]| {
        Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap()
    };
    git(&sub, &["init", "-q"]);
    git(&sub, &["add", "-A"]);
    git(&sub, &["commit", "-qm", "sub"]);
    let snap = g.attic().snapshot("before").unwrap();
    assert!(g.attic().holds(&snap, "src/a.rs"));
    assert!(
        !g.attic().holds(&snap, "vendor/sub"),
        "a gitlink is not a copy"
    );
    assert!(!g.attic().holds(&snap, "vendor/sub/lib.rs"));
}

/// A worktree's or a submodule's `.git` is a file: the attic keeps it and puts it back.
#[test]
fn a_git_file_is_kept_and_put_back() {
    let g = Ground::new("gitfile");
    let pointer = "gitdir: /somewhere/else/.git/worktrees/w\n";
    fs::write(g.world().join(".git"), pointer).unwrap();
    g.attic().snapshot("before").unwrap();
    fs::remove_file(g.world().join(".git")).unwrap();
    let said = g.attic().repair_git().expect("it repairs");
    assert!(said.contains("put back"), "{said}");
    assert_eq!(fs::read_to_string(g.world().join(".git")).unwrap(), pointer);
    assert_eq!(g.attic().repair_git(), None, "nothing more to repair");
}

/// Two runs on one world snapshot in turn, never colliding on the attic's index.
#[test]
fn concurrent_snapshots_take_turns() {
    let g = Arc::new(Ground::new("concurrent"));
    let threads: Vec<_> = (0..4)
        .map(|t| {
            let g = Arc::clone(&g);
            std::thread::spawn(move || {
                for i in 0..5 {
                    fs::write(g.world().join(format!("src/t{t}-{i}.rs")), "x\n").unwrap();
                    g.attic().snapshot(&format!("run {t}, act {i}")).unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let last = g.attic().snapshot("after").unwrap();
    for t in 0..4 {
        assert!(g.attic().holds(&last, &format!("src/t{t}-4.rs")));
    }
}

/// `rm -rf` of a nested repository: the command is told it was not kept, never that it was.
#[test]
fn a_deleted_nested_repository_is_reported_lost() {
    let g = Ground::new("nested-lost");
    let sub = g.world().join("vendor/sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("lib.rs"), "fn lib() {}\n").unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(&sub)
            .output()
            .unwrap()
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-qm", "sub"]);
    let snap = g.attic().snapshot("before").unwrap();
    fs::remove_dir_all(g.world().join("vendor")).unwrap();
    fs::remove_file(g.world().join("src/a.rs")).unwrap();
    assert_eq!(g.attic().lost_since(&snap), ["vendor/sub"]);
    assert_eq!(g.attic().deleted_since(&snap), ["src/a.rs"]);
}

/// A world that turned its `.git` file into a directory is never repaired back to the old file.
#[test]
fn only_the_current_form_of_git_is_kept() {
    let g = Ground::new("git-forms");
    fs::write(g.world().join(".git"), "gitdir: /elsewhere\n").unwrap();
    g.attic().snapshot("a file").unwrap();
    fs::remove_file(g.world().join(".git")).unwrap();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(g.world())
        .output()
        .unwrap();
    g.attic().snapshot("a directory").unwrap();
    fs::remove_dir_all(g.world().join(".git")).unwrap();
    g.attic().repair_git().expect("it repairs");
    assert!(
        g.world().join(".git").is_dir(),
        "the directory came back, not the old file"
    );
}

/// An act on the ground's world, sealed: `act` runs between the two snapshots.
fn sealed(g: &Ground, act: impl FnOnce(&Path)) -> (Attic, Act) {
    let attic = g.attic();
    let before = attic.snapshot("before the act").unwrap();
    act(&g.world());
    let act = attic.seal(before, "after the act").unwrap();
    (attic, act)
}

fn conflicts(r: Restore) -> Vec<(String, State, State)> {
    match r {
        Restore::Conflict(c) => c.into_iter().map(|c| (c.path, c.left, c.now)).collect(),
        Restore::Restored(p) => panic!("restored {p:?}, expected a conflict"),
    }
}

fn read(g: &Ground, rel: &str) -> String {
    fs::read_to_string(g.world().join(rel)).unwrap()
}

/// A later edit is the later writer's: the undo is refused, names the path, and keeps the edit.
#[test]
fn a_restore_after_a_concurrent_edit_is_refused() {
    let g = Ground::new("guard-edit");
    let (attic, act) = sealed(&g, |w| fs::write(w.join("src/a.rs"), "the act\n").unwrap());
    fs::write(g.world().join("src/a.rs"), "the human\n").unwrap();
    let c = conflicts(attic.restore(&act, &[]).unwrap());
    assert_eq!(c.len(), 1, "{c:?}");
    assert_eq!(c[0].0, "src/a.rs");
    assert!(
        matches!((&c[0].1, &c[0].2), (State::File { blob: l, .. }, State::File { blob: n, .. }) if l != n),
        "{c:?}"
    );
    assert_eq!(read(&g, "src/a.rs"), "the human\n");
    let log = fs::read_to_string(attic.dir().join("snapshots.log")).unwrap();
    assert!(
        log.lines().last().unwrap().contains("refused: `src/a.rs`"),
        "the refusal is journaled: {log}"
    );
}

/// A deletion since the act is a conflict: the undo does not bring back what someone removed.
#[test]
fn a_later_deletion_is_a_conflict() {
    let g = Ground::new("guard-rm");
    let (attic, act) = sealed(&g, |w| fs::write(w.join("src/a.rs"), "the act\n").unwrap());
    fs::remove_file(g.world().join("src/a.rs")).unwrap();
    let c = conflicts(attic.restore(&act, &["src/a.rs"]).unwrap());
    assert!(
        matches!(&c[..], [(p, State::File { .. }, State::Absent)] if p == "src/a.rs"),
        "{c:?}"
    );
    assert!(!g.world().join("src/a.rs").exists());
}

/// A mode changed since the act is a conflict, the content being the same.
#[test]
fn a_later_mode_change_is_a_conflict() {
    use std::os::unix::fs::PermissionsExt as _;
    let g = Ground::new("guard-mode");
    let (attic, act) = sealed(&g, |w| fs::write(w.join("src/a.rs"), "the act\n").unwrap());
    let a = g.world().join("src/a.rs");
    fs::set_permissions(&a, fs::Permissions::from_mode(0o755)).unwrap();
    let c = conflicts(attic.restore(&act, &[]).unwrap());
    assert!(
        matches!(
            &c[..],
            [(
                _,
                State::File {
                    executable: false,
                    ..
                },
                State::File {
                    executable: true,
                    ..
                }
            )]
        ),
        "{c:?}"
    );
    assert_eq!(read(&g, "src/a.rs"), "the act\n");
}

/// A symlink made where the act deleted a file is a conflict: the undo neither writes through
/// it nor replaces it.
#[test]
fn a_new_symlink_is_a_conflict() {
    let g = Ground::new("guard-link");
    let outside = g.root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let (attic, act) = sealed(&g, |w| fs::remove_file(w.join("src/b.rs")).unwrap());
    std::os::unix::fs::symlink(outside.join("b.rs"), g.world().join("src/b.rs")).unwrap();
    let c = conflicts(attic.restore(&act, &[]).unwrap());
    assert!(
        matches!(&c[..], [(p, State::Absent, State::Link { target })] if p == "src/b.rs" && *target == outside.join("b.rs")),
        "{c:?}"
    );
    assert!(
        fs::symlink_metadata(g.world().join("src/b.rs"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        !outside.join("b.rs").exists(),
        "nothing written through the link"
    );
}

/// A directory swapped for a symlink leading out of the world: the path beyond it is a
/// conflict, even when the file out there holds what the act left.
#[test]
fn a_symlinked_directory_is_never_gone_through() {
    let g = Ground::new("guard-beyond");
    let outside = g.root.join("outside");
    let (attic, act) = sealed(&g, |w| fs::write(w.join("src/a.rs"), "the act\n").unwrap());
    fs::rename(g.world().join("src"), &outside).unwrap();
    std::os::unix::fs::symlink(&outside, g.world().join("src")).unwrap();
    let c = conflicts(attic.restore(&act, &[]).unwrap());
    assert!(
        c.iter()
            .any(|(p, _, now)| p == "src/a.rs" && *now == State::Beyond { lead: "src".into() }),
        "{c:?}"
    );
    assert_eq!(
        fs::read_to_string(outside.join("a.rs")).unwrap(),
        "the act\n"
    );
    assert!(
        fs::symlink_metadata(g.world().join("src"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

/// Nothing changed since the act: the undo puts the world back exactly as it was — what the act
/// changed, deleted, created, made executable or turned into a directory.
#[test]
fn an_unchanged_state_restores_exactly() {
    use std::os::unix::fs::PermissionsExt as _;
    let g = Ground::new("guard-exact");
    fs::write(g.world().join("run.sh"), "echo hi\n").unwrap();
    let (attic, act) = sealed(&g, |w| {
        fs::write(w.join("src/a.rs"), "the act\n").unwrap();
        fs::remove_file(w.join("src/b.rs")).unwrap();
        fs::create_dir(w.join("src/b.rs")).unwrap();
        fs::write(w.join("src/b.rs/inner.rs"), "inside\n").unwrap();
        fs::write(w.join("new.rs"), "new\n").unwrap();
        std::os::unix::fs::symlink("src/a.rs", w.join("link")).unwrap();
        fs::set_permissions(w.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    });
    let Restore::Restored(paths) = attic.restore(&act, &[]).unwrap() else {
        panic!("refused");
    };
    assert_eq!(paths.len(), 6, "{paths:?}");
    assert_eq!(
        attic.snapshot("after the undo").unwrap().tree,
        act.before.tree
    );
    assert_eq!(read(&g, "src/a.rs"), "fn a() {}\n");
    assert_eq!(read(&g, "src/b.rs"), "fn b() {}\n");
    assert!(!g.world().join("new.rs").exists());
    assert!(fs::symlink_metadata(g.world().join("link")).is_err());
    let mode = fs::metadata(g.world().join("run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o100, 0);
}

/// One path changed since the act, and nothing at all is undone.
#[test]
fn a_partial_conflict_restores_nothing() {
    let g = Ground::new("guard-partial");
    let (attic, act) = sealed(&g, |w| {
        fs::write(w.join("src/a.rs"), "act a\n").unwrap();
        fs::write(w.join("src/b.rs"), "act b\n").unwrap();
        fs::write(w.join("new.rs"), "new\n").unwrap();
    });
    fs::write(g.world().join("src/b.rs"), "human b\n").unwrap();
    let c = conflicts(attic.restore(&act, &[]).unwrap());
    assert_eq!(
        c.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(),
        ["src/b.rs"]
    );
    assert_eq!(read(&g, "src/a.rs"), "act a\n");
    assert_eq!(read(&g, "src/b.rs"), "human b\n");
    assert_eq!(read(&g, "new.rs"), "new\n");
    // Only the untouched path, asked for alone, is undone.
    assert_eq!(
        attic.restore(&act, &["src/a.rs"]).unwrap(),
        Restore::Restored(vec!["src/a.rs".into()])
    );
    assert_eq!(read(&g, "src/a.rs"), "fn a() {}\n");
}

/// A path that is not plain and inside the world is refused before anything is read.
#[test]
fn a_path_outside_the_world_is_refused() {
    let g = Ground::new("guard-path");
    let (attic, act) = sealed(&g, |w| fs::write(w.join("src/a.rs"), "x\n").unwrap());
    for bad in [
        "../x",
        "/etc/passwd",
        "src/../../x",
        "",
        "src//a.rs",
        ".git/config",
    ] {
        assert!(attic.restore(&act, &[bad]).is_err(), "{bad}");
    }
    assert_eq!(read(&g, "src/a.rs"), "x\n");
}
