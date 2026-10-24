//! The attic: what commands and writes take away is kept outside the world, and comes back.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use mnemorph_races::Race;
use mnemorph_tools::gate::Scripted;
use mnemorph_tools::{Attic, Context, Outcome, Sandbox, Snapshot, call, registry};

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
    Snapshot { tree: short }
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
    g.attic().restore(&tree_of(&o), &["src"]).unwrap();
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
    attic.restore(&Snapshot { tree }, &["src/a.rs"]).unwrap();
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
    assert!(
        attic.restore(&old, &["src/a.rs"]).is_ok(),
        "a day later both are kept"
    );
    attic.prune(now() + 15 * DAY).unwrap();
    assert!(
        attic.restore(&old, &["src/a.rs"]).is_err(),
        "past the window the old one is gone"
    );
    attic.restore(&new, &["src/a.rs"]).unwrap();
    assert_eq!(
        fs::read_to_string(g.world().join("src/a.rs")).unwrap(),
        "fn a2() {}\n"
    );
}

/// Over its size, the attic drops the oldest snapshots first — never the newest.
#[test]
fn the_size_cap_drops_the_oldest() {
    let g = Ground::new("prune-size");
    let (attic, old, new) = two_snapshots(&g);
    let attic = attic.with_limits(14, 0);
    attic.prune(now()).unwrap();
    assert!(attic.restore(&old, &["src/a.rs"]).is_err());
    assert!(attic.restore(&new, &["src/a.rs"]).is_ok());
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
