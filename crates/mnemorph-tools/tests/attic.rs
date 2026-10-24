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
