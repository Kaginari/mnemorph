//! Properties of the classifier that no list of rows can pin down.

use std::fs;
use std::path::PathBuf;

use mnemorph_races::ToolClass;
use mnemorph_tools::{Place, Verdict, classify};
use proptest::prelude::*;

/// Refusal ranks above every class.
fn rank(v: &Verdict) -> u8 {
    match v.class() {
        None => 4,
        Some(ToolClass::Read) => 0,
        Some(ToolClass::Write) => 1,
        Some(ToolClass::Outward) => 2,
        Some(_) => 3,
    }
}

const BASES: &[&str] = &[
    "ls",
    "cat",
    "git",
    "git push",
    "git branch",
    "git reflog",
    "git gc",
    "gh",
    "gh api",
    "cargo",
    "npm",
    "rm",
    "cp",
    "mv",
    "find .",
    "sed",
    "env",
    "sudo",
    "xargs",
    "timeout 5",
    "op",
    "security",
    "docker",
    "brew",
    "chmod",
    "dd",
    "sh -c",
    "echo",
    "touch",
    "curl",
];

const ARGS: &[&str] = &[
    "-f",
    "--force",
    "-d",
    "-D",
    "-i",
    "--delete",
    "-X",
    "DELETE",
    "--prune=now",
    "--expire=now",
    "--hard",
    "push",
    "status",
    "list",
    "install",
    "publish",
    "/",
    "~",
    "/tmp/x",
    "./x",
    "x",
    "origin",
    "main",
    "+main",
    ":main",
    "--tags",
    "-delete",
    "-exec",
    "rm",
    "{}",
    ";",
    "of=/dev/sda",
    "--amend",
    "-rf",
    "$HOME",
    "$X",
    "*",
    "..",
    "--dry-run",
    "-n",
    "1",
    "token",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Lesson: a classifier that read argv data could downgrade a command. Adding words never
    /// lowers the class.
    #[test]
    fn arguments_only_tighten(
        base in prop::sample::select(BASES),
        first in prop::collection::vec(prop::sample::select(ARGS), 0..4),
        more in prop::collection::vec(prop::sample::select(ARGS), 1..4),
    ) {
        let place = Place::new("/w", "/h");
        let before = format!("{base} {}", first.join(" "));
        let after = format!("{before} {}", more.join(" "));
        let (a, b) = (classify(&before, &place), classify(&after, &place));
        prop_assert!(rank(&b) >= rank(&a), "{before:?} → {a:?}\n{after:?} → {b:?}");
    }

    /// Any input gets a verdict: garbage is refused or classified, never a panic.
    #[test]
    fn never_panics(src in ".{0,80}") {
        let _ = classify(&src, &Place::new("/w", "/h"));
    }
}

/// Lesson: symlinks escaped the border because the check compared literal paths.
#[test]
fn a_symlink_out_of_the_world_is_outside() {
    let root = std::env::temp_dir().join(format!("mnemorph-classify-{}", std::process::id()));
    let world = root.join("world");
    let outside = root.join("outside");
    fs::create_dir_all(&world).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let link = world.join("link");
    let _ = fs::remove_file(&link);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let place = Place::new(world.clone(), PathBuf::from("/h"));

    let through = classify("rm -rf link/data", &place);
    let inside = classify("rm -rf data", &place);
    let _ = fs::remove_dir_all(&root);

    assert_eq!(through.class(), Some(ToolClass::Destructive), "{through:?}");
    assert_eq!(inside.class(), Some(ToolClass::Write), "{inside:?}");
}
