//! The end gate, as solo mode runs it: what the run changed — found by snapshotting the tree, so
//! a shell redirect counts the same as the write tool — judged before the answer goes up.
//!
//! Checks here: traits hold (the verify commands pass, run through the same classified and
//! guarded `bash` as any other command), duties done (a non-empty answer), tests intact (no
//! test file deleted, no new skip marker). Right author holds by construction in solo (the
//! whole repository is the soil, and the tripwire keeps `.git` and `.mnemorph` out). Doc
//! truthful and no seal come with the world's creatures and the Dark Elf.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use mnemorph_tools::{Context, Tool, call};
use serde_json::json;

/// Bytes of a test file the snapshot keeps, to count its skip markers.
const TEST_BYTES: u64 = 256 * 1024;

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
];

/// The gate's configuration: the commands that must pass.
#[derive(Debug, Clone, Default)]
pub struct EndGate {
    /// Verify commands, each run through `bash` with a 120 s limit.
    pub verify: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    len: u64,
    modified: Option<std::time::SystemTime>,
    test_text: Option<String>,
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
        let test_text = (is_test(&rel) && meta.len() <= TEST_BYTES)
            .then(|| fs::read_to_string(path).unwrap_or_default());
        files.insert(
            rel,
            Seen {
                len: meta.len(),
                modified: meta.modified().ok(),
                test_text,
            },
        );
    }
    Tree { files }
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

fn skips(text: &str) -> usize {
    SKIP_MARKERS.iter().map(|m| text.matches(m).count()).sum()
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
            if let Some(text) = &now.test_text {
                let before_skips = was.and_then(|w| w.test_text.as_deref()).map_or(0, skips);
                if skips(text) > before_skips {
                    failures.push(format!(
                        "tests intact: `{}` skips a test it did not skip before",
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
