//! The end gate, as solo mode runs it: what the run changed — found by snapshotting the tree, so
//! a shell redirect counts the same as the write tool — judged before the answer goes up.
//!
//! Checks here: traits hold (the verify commands pass, run through the same classified and
//! guarded `bash` as any other command), duties done (a non-empty answer), tests intact (no
//! test file deleted, no new skip marker), and — for a commissioned creature — right author:
//! every changed path is the creature's by `own`, and no critical or high finding touches the
//! landing. In solo right author holds by construction (the whole repository is the soil, and
//! the tripwire keeps `.git` and `.mnemorph` out). Doc truthful and no seal come with the Dark
//! Elf.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use mnemorph_tools::{Context, Tool, call};
use mnemorph_world::doc::covers;
use mnemorph_world::{Finding, Owner, Roster, Rule, check, explain, own};
use serde_json::json;

/// What marks a test as skipped, across the usual frameworks.
const SKIP_MARKERS: &[&str] = &[
    "#[ignore",
    "it.skip(",
    "describe.skip(",
    "test.skip(",
    "xit(",
    "xdescribe(",
    "@pytest.mark.skip",
    "@unittest.skip",
    "t.Skip(",
    "@Disabled",
    "skip(\"",
    ".only(",
    "t.Skipf(",
    "t.SkipNow(",
    "it.todo(",
    "test.todo(",
    "pytest.skip(",
];

/// What declares a test, across the usual frameworks: a file with fewer than before lost one.
const TEST_MARKERS: &[&str] = &[
    "#[test]",
    "#[tokio::test",
    "\nfunc Test",
    "def test_",
    "\nit(",
    " it(",
    "\ntest(",
    " test(",
    "@Test",
];

/// The gate's configuration: the commands that must pass, and who must have written the
/// landing.
#[derive(Debug, Clone, Default)]
pub struct EndGate {
    /// Verify commands, each run through `bash` with a 120 s limit.
    pub verify: Vec<String>,
    /// The commissioned creature whose landing this is; `None` in solo, where the whole
    /// repository is the soil and right author holds by construction.
    pub author: Option<Author>,
}

/// Who must have written what a landing changed: the creature, by the world's own answer.
#[derive(Debug, Clone)]
pub struct Author {
    /// The creature.
    pub name: String,
    /// The roster as the commission found it: what the creature owned when it began.
    pub roster: Roster,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    len: u64,
    modified: Option<std::time::SystemTime>,
    /// The mode and the inode's change time: any write, chmod or touch moves the change time,
    /// and no process can set it back, so a same-length rewrite dressed in its old mtime still
    /// counts as changed.
    stamp: (u32, i64, i64),
    /// For a test file, read whole whatever its size: its skip markers and its tests.
    counts: Option<(usize, usize)>,
}

/// The files of a world at one moment (honouring `.gitignore`; `.git` and `.mnemorph` aside).
#[derive(Debug, Clone, Default)]
pub struct Tree {
    files: BTreeMap<PathBuf, Seen>,
}

/// Snapshot the world.
#[must_use]
pub fn snapshot(world: &Path) -> Tree {
    let mut files = BTreeMap::new();
    let walk = WalkBuilder::new(world)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git" && e.file_name() != ".mnemorph")
        .build();
    for entry in walk.filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        let rel = path.strip_prefix(world).unwrap_or(path).to_path_buf();
        let counts = is_test(&rel).then(|| {
            let text = String::from_utf8_lossy(&fs::read(path).unwrap_or_default()).into_owned();
            (count(&text, SKIP_MARKERS), count(&text, TEST_MARKERS))
        });

        files.insert(
            rel,
            Seen {
                len: meta.len(),
                modified: meta.modified().ok(),
                stamp: stamp(&meta),
                counts,
            },
        );
    }
    Tree { files }
}

/// A file's mode and change time, where the platform has them.
fn stamp(meta: &fs::Metadata) -> (u32, i64, i64) {
    use std::os::unix::fs::MetadataExt as _;
    (meta.mode(), meta.ctime(), meta.ctime_nsec())
}

/// A path that holds tests, by the usual conventions.
fn is_test(rel: &Path) -> bool {
    let in_test_dir = rel.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("tests" | "test" | "__tests__" | "spec" | "specs")
        )
    });
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let stem = name.split('.').next().unwrap_or("");
    in_test_dir
        || stem.ends_with("_test")
        || stem.ends_with("_spec")
        || stem.starts_with("test_")
        || name.contains(".test.")
        || name.contains(".spec.")
}

fn count(text: &str, markers: &[&str]) -> usize {
    markers.iter().map(|m| text.matches(m).count()).sum()
}

/// The gate's word on a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateVerdict {
    /// The paths the run added, changed or removed.
    pub changed: Vec<PathBuf>,
    /// What failed, one line each; empty when the gate passes.
    pub failures: Vec<String>,
}

impl GateVerdict {
    /// The gate passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

impl EndGate {
    /// Judge what changed between `before` and now, and the answer.
    pub fn judge(
        &self,
        before: &Tree,
        answer: &str,
        tools: &[Box<dyn Tool>],
        cx: &mut Context,
    ) -> GateVerdict {
        let after = snapshot(cx.world());
        let mut changed: Vec<PathBuf> = Vec::new();
        let mut failures = Vec::new();
        for (path, was) in &before.files {
            match after.files.get(path) {
                None => {
                    changed.push(path.clone());
                    if is_test(path) {
                        failures.push(format!(
                            "tests intact: the test file `{}` was deleted",
                            path.display()
                        ));
                    }
                }
                Some(now) if now != was => changed.push(path.clone()),
                Some(_) => {}
            }
        }
        for (path, now) in &after.files {
            let was = before.files.get(path);
            if was.is_none() {
                changed.push(path.clone());
            }
            if let Some((skips, tests)) = now.counts {
                let (was_skips, was_tests) = was.and_then(|w| w.counts).unwrap_or((0, 0));
                if skips > was_skips {
                    failures.push(format!(
                        "tests intact: `{}` skips a test it did not skip before",
                        path.display()
                    ));
                }
                if tests < was_tests {
                    failures.push(format!(
                        "tests intact: `{}` has {tests} tests, it had {was_tests}",
                        path.display()
                    ));
                }
            }
        }
        changed.sort();
        if changed.is_empty() {
            return GateVerdict { changed, failures };
        }
        if answer.trim().is_empty() {
            failures.push("duties done: the answer is empty; say what was done".into());
        }
        if let Some(author) = &self.author {
            failures.extend(right_author(author, &changed, cx.world()));
        }
        for command in &self.verify {
            let args = json!({"command": command, "timeout_ms": 120_000}).to_string();
            let o = call(tools, "bash", &args, cx);
            if o.is_error {
                let tail: String = o
                    .content
                    .chars()
                    .rev()
                    .take(600)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                failures.push(format!("traits hold: `{command}` failed:\n{tail}"));
            }
        }
        GateVerdict { changed, failures }
    }
}

/// Right author: every changed path is the creature's by `own` on the roster it began with, by
/// any rule but a domain; and no critical or high finding, read on the tree as the landing
/// leaves it, touches the landing — so a landing that removes a finding is not blocked by it.
fn right_author(author: &Author, changed: &[PathBuf], world: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    let me = Owner::Creature(author.name.clone());
    for path in changed {
        let rel = path.to_string_lossy();
        let e = explain(&author.roster, &rel);
        if e.owner == me && e.rule != Rule::Domain {
            continue;
        }
        let (whose, route) = match &e.owner {
            Owner::Creature(o) if *o == author.name => (
                format!("{o}'s to route, not to write"),
                "a Slime writes it".to_owned(),
            ),
            Owner::Creature(o) => (format!("{o}'s"), format!("ask {o}")),
            Owner::Harness => ("harness soil".into(), "only the harness writes it".into()),
            Owner::Human => (
                "the human's".into(),
                "the human changes it as a diff they apply".into(),
            ),
            Owner::Unowned => ("nobody's".into(), "Rimuru routes its birth".into()),
        };
        let by = e.claim.map_or_else(
            || e.rule.words().to_owned(),
            |c| format!("{} {c}", e.rule.words()),
        );
        failures.push(format!(
            "right author: `{rel}` is {whose} ({by}), not {}'s — {route}",
            author.name
        ));
    }
    let after = Roster::load(world);
    for f in check(&after, &[]) {
        if f.severity.blocks() && touches(&f, changed, &after) {
            failures.push(format!(
                "right author: a {} `{}` finding touches this landing ({}): {}",
                f.severity.word(),
                f.code,
                f.at,
                f.message
            ));
        }
    }
    failures
}

/// Whether a finding touches a landing: a written path lies under one of its paths, or a
/// creature it is about owns a written path.
fn touches(f: &Finding, changed: &[PathBuf], roster: &Roster) -> bool {
    changed.iter().any(|path| {
        let rel = path.to_string_lossy();
        f.paths.iter().any(|p| covers(p, &rel))
            || f.creatures
                .iter()
                .any(|c| own(roster, &rel) == Owner::Creature(c.clone()))
    })
}
