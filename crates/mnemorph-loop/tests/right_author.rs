//! The end gate's right author: a commissioned creature lands only on its own soil, and a
//! critical or high finding blocks a landing it touches — not one that removes it.

use std::fs;
use std::path::PathBuf;

use mnemorph_loop::commission::context_for;
use mnemorph_loop::world::Roster;
use mnemorph_loop::{Author, EndGate, Verdict, snapshot};
use mnemorph_tools::{Sandbox, registry};

struct World(PathBuf);

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-author-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for dir in ["src", "lib"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        let w = World(root);
        w.doc(
            "orc",
            "o",
            "# orc-o\n\n- **Race:** orc\n- **Domain:** src/ lib/\n",
        );
        w.doc(
            "slime",
            "a",
            "# slime-a\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/\n",
        );
        w.doc(
            "slime",
            "b",
            "# slime-b\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** lib/\n",
        );
        w
    }

    fn doc(&self, race: &str, short: &str, body: &str) {
        let dir = self.0.join(".mnemorph").join(race).join(short);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("README.md"), body).unwrap();
    }

    /// Judge what changed since `before`, as slime-a's landing.
    fn judge(&self, before: &mnemorph_loop::Tree, roster: &Roster) -> Vec<String> {
        let gate = EndGate {
            verify: Vec::new(),
            author: Some(Author {
                name: "slime-a".into(),
                roster: roster.clone(),
            }),
        };
        let mut cx = context_for(&self.0, roster, "slime-a")
            .unwrap()
            .with_sandbox(Sandbox::None("the gate's judgement is under test".into()));
        gate.judge(before, "done", &registry(), &mut cx).failures
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_creature_lands_only_on_its_own_soil() {
    let w = World::new("soil");
    let roster = Roster::load(&w.0);
    let before = snapshot(&w.0);
    fs::write(w.0.join("src/mine.rs"), "mine\n").unwrap();
    assert_eq!(w.judge(&before, &roster), Vec::<String>::new());
    fs::write(w.0.join("lib/theirs.rs"), "theirs\n").unwrap();
    fs::write(w.0.join("NOTES.md"), "stray\n").unwrap();
    let failures = w.judge(&before, &roster);
    assert_eq!(
        failures,
        [
            "right author: `NOTES.md` is nobody's (the workspace NOTES.md), not slime-a's — Rimuru routes its birth",
            "right author: `lib/theirs.rs` is slime-b's (zone lib/), not slime-a's — ask slime-b",
        ]
    );
}

#[test]
fn a_finding_blocks_the_landing_it_touches_not_the_one_that_ends_it() {
    let w = World::new("finding");
    // slime-c claims inside slime-a's zone: a critical overlap on src/.
    w.doc(
        "slime",
        "c",
        "# slime-c\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** src/c/\n",
    );
    let roster = Roster::load(&w.0);
    let before = snapshot(&w.0);
    fs::write(w.0.join("src/mine.rs"), "mine\n").unwrap();
    let failures = w.judge(&before, &roster);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with("right author: a critical `overlap` finding touches this landing"),
        "{failures:?}"
    );
    // The landing that narrows the overlap away is judged on the tree it leaves.
    w.doc(
        "slime",
        "c",
        "# slime-c\n\n- **Race:** slime\n- **Reports to:** orc-o\n- **Territory:** etc/\n",
    );
    assert_eq!(w.judge(&before, &roster), Vec::<String>::new());
}

/// Build output is what git ignores where `own` names the creature or nobody: never another's
/// ignored files, never the workspace's root files.
#[test]
fn build_output_is_never_anothers() {
    let w = World::new("build");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&w.0)
            .args(args)
            .output()
            .unwrap()
    };
    git(&["init", "-q"]);
    fs::write(w.0.join(".gitignore"), ".env\ntarget/\n").unwrap();
    fs::write(w.0.join("lib/.env"), "theirs\n").unwrap();
    fs::write(w.0.join(".env"), "root\n").unwrap();
    fs::create_dir_all(w.0.join("target")).unwrap();
    fs::create_dir_all(w.0.join("lib/target")).unwrap();
    fs::write(w.0.join("src/mine.rs"), "x\n").unwrap();
    fs::write(w.0.join("lib/theirs.rs"), "x\n").unwrap();
    git(&["add", "src", "lib", ".gitignore"]);
    let roster = Roster::load(&w.0);
    let grants = context_for(&w.0, &roster, "slime-a").unwrap().grants();
    let real = w.0.canonicalize().unwrap();
    assert!(grants.contains(&real.join("target")), "{grants:?}");
    for never in ["lib/.env", "lib/target", ".env"] {
        assert!(!grants.contains(&real.join(never)), "{never}: {grants:?}");
    }
}

/// A same-length rewrite dressed in its old mtime still counts as changed: the gate sees the
/// inode's change time, which no process sets back.
#[test]
fn a_rewrite_in_its_old_clothes_is_still_seen() {
    let w = World::new("ctime");
    fs::write(w.0.join("lib/theirs.rs"), "aaaa\n").unwrap();
    let roster = Roster::load(&w.0);
    let before = snapshot(&w.0);
    let mtime = fs::metadata(w.0.join("lib/theirs.rs"))
        .unwrap()
        .modified()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(w.0.join("lib/theirs.rs"), "bbbb\n").unwrap();
    fs::File::options()
        .write(true)
        .open(w.0.join("lib/theirs.rs"))
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let failures = w.judge(&before, &roster);
    assert!(
        failures
            .iter()
            .any(|f| f.contains("`lib/theirs.rs` is slime-b's")),
        "{failures:?}"
    );
}

/// A check never writes: a verify line that writes a file git tracks fails the gate, naming it;
/// one that writes only what git ignores — a test run's build output — passes.
#[test]
fn a_verify_line_that_writes_fails_the_gate() {
    let w = World::new("verify-writes");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&w.0)
            .output()
            .unwrap()
    };
    git(&["init", "-q"]);
    fs::write(w.0.join(".gitignore"), "build/\n").unwrap();
    let roster = Roster::load(&w.0);
    let judge = |verify: &str| {
        let before = snapshot(&w.0);
        fs::write(w.0.join("src/mine.rs"), format!("{verify}\n")).unwrap();
        let gate = EndGate {
            verify: vec![verify.to_owned()],
            author: Some(Author {
                name: "slime-a".into(),
                roster: roster.clone(),
            }),
        };
        let mut cx = context_for(&w.0, &roster, "slime-a")
            .unwrap()
            .with_sandbox(Sandbox::None("the gate's judgement is under test".into()))
            .with_unguarded(true);
        gate.judge(&before, "done", &registry(), &mut cx).failures
    };
    let sneaky = judge("printf 'landed past the gate' > src/sneak.rs");
    assert!(
        sneaky
            .iter()
            .any(|f| f.contains("a verify line changed `src/sneak.rs` — a check never writes")),
        "{sneaky:?}"
    );
    let built = judge("mkdir -p build && echo out > build/cache");
    assert_eq!(built, Vec::<String>::new());
    // Printing the words is no write.
    let said = judge("echo 'a check never writes'");
    assert_eq!(said, Vec::<String>::new());
    // The local exclude file never hides a body's change from the gate.
    fs::write(w.0.join(".git/info/exclude"), "coverage/\n").unwrap();
    let before = snapshot(&w.0);
    fs::create_dir_all(w.0.join("lib/coverage")).unwrap();
    fs::write(w.0.join("lib/coverage/m.rs"), "m\n").unwrap();
    let seen = w.judge(&before, &roster);
    assert!(
        seen.iter().any(|f| f.contains("lib/coverage/m.rs")),
        "{seen:?}"
    );
    fs::remove_dir_all(w.0.join("lib/coverage")).unwrap();
    // The report folder is a check's to write.
    let report = judge("mkdir -p .mnemorph/report && echo ok > .mnemorph/report/junit.xml");
    assert_eq!(report, Vec::<String>::new());
}

/// PASS, FAIL or `NOT_VERIFIED`: a verify line that never ran — no kernel guard, none allowed
/// unguarded — gave no result, so the landing is not verified, never passed; a line that ran and
/// failed outweighs it.
#[test]
fn a_check_that_gave_no_result_is_not_verified() {
    let w = World::new("not-verified");
    let roster = Roster::load(&w.0);
    let judge = |verify: Vec<String>, unguarded: bool| {
        let before = snapshot(&w.0);
        fs::write(w.0.join("src/mine.rs"), format!("{verify:?}\n")).unwrap();
        let gate = EndGate {
            verify,
            author: Some(Author {
                name: "slime-a".into(),
                roster: roster.clone(),
            }),
        };
        let mut cx = context_for(&w.0, &roster, "slime-a")
            .unwrap()
            .with_sandbox(Sandbox::None("the gate's judgement is under test".into()))
            .with_unguarded(unguarded);
        gate.judge(&before, "done", &registry(), &mut cx)
    };
    let unrun = judge(vec!["true".into()], false);
    assert_eq!(unrun.verdict(), Verdict::NotVerified, "{unrun:?}");
    assert!(unrun.unrun(), "the machine's doing, not the body's");
    assert!(!unrun.passed());
    assert_eq!(unrun.failures, Vec::<String>::new());
    assert!(
        unrun.unverified[0].contains("`true` gave no result"),
        "{unrun:?}"
    );
    let ran = judge(vec!["true".into()], true);
    assert_eq!(ran.verdict(), Verdict::Pass, "{ran:?}");
    assert!(!ran.unrun());
    let failed = judge(vec!["false".into()], true);
    assert_eq!(failed.verdict(), Verdict::Fail, "{failed:?}");
    // Commands may run: a line that failed beside one refused (a credential read) is a failure,
    // and a refusal is never the machine's doing.
    let mixed = judge(vec!["false".into(), "cat ~/.ssh/id_rsa".into()], true);
    assert_eq!(mixed.verdict(), Verdict::Fail, "{mixed:?}");
    assert!(!mixed.unrun(), "{mixed:?}");
    let refused = judge(vec!["cat ~/.ssh/id_rsa".into()], true);
    assert!(!refused.unrun(), "{refused:?}");
    // A crash is a failure, not a missing result.
    let crashed = judge(vec!["kill -SEGV $$".into()], true);
    assert_eq!(crashed.verdict(), Verdict::Fail, "{crashed:?}");
    assert_eq!(Verdict::NotVerified.word(), "NOT_VERIFIED");
}
