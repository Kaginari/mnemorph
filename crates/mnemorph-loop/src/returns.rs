//! Trees that come back: after every write that landed, the files the ask has changed are hashed,
//! and the count of each hash says how many times that tree has stood. A failed qwen run goes
//! round between a few trees and never leaves them; the stronger models never came back once in
//! the kept runs. Going back to the starter is a full revert, which may be right: it is said
//! apart and never counted. A reading only: it is journaled, and decides nothing.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use crate::beats::Beat;

/// What a file held, for the tree's hash: its bytes' digest, or none for a file that is not there.
type Held = Option<[u8; 32]>;

/// The trees an ask's landed writes have left, and the files as they were before the ask first
/// wrote each.
#[derive(Debug, Default, Clone)]
pub struct Returns {
    /// Each file's content before the ask's first write to it through a file tool; a file first
    /// written another way has none, and a tree holding it is never the starter.
    originals: HashMap<String, Held>,
    /// The files the ask's landed writes changed.
    changed: BTreeSet<String>,
    /// How many times each tree has stood after a landed write.
    seen: HashMap<String, u32>,
}

/// The paths a file tool's call may write, from its input as the model sent it: `path`, a move's
/// `to`, and the files a patch names.
#[must_use]
pub fn targets(tool: &str, input: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(input) else {
        return Vec::new();
    };
    let mut out: Vec<String> = ["path", "to"]
        .iter()
        .filter_map(|k| v[k].as_str().map(str::to_owned))
        .collect();
    if tool == "patch"
        && let Some(diff) = v["patch"].as_str()
    {
        for line in diff.lines() {
            let Some(rest) = line
                .strip_prefix("--- ")
                .or_else(|| line.strip_prefix("+++ "))
            else {
                continue;
            };
            let p = rest.split('\t').next().unwrap_or(rest).trim();
            if p == "/dev/null" {
                continue;
            }
            let p = p
                .strip_prefix("a/")
                .or_else(|| p.strip_prefix("b/"))
                .unwrap_or(p);
            out.push(p.to_owned());
        }
    }
    out
}

/// `path` as the world names it: relative, `./` gone.
fn rel(world: &Path, path: &Path) -> String {
    let p = path.strip_prefix(world).unwrap_or(path);
    p.to_string_lossy().trim_start_matches("./").to_owned()
}

fn held(world: &Path, rel: &str) -> Held {
    std::fs::read(world.join(rel))
        .ok()
        .map(|bytes| Sha256::digest(&bytes).into())
}

/// The hash of a tree: each path, sorted, with what it holds.
fn digest(files: &BTreeMap<&str, Held>) -> String {
    let mut h = Sha256::new();
    for (path, held) in files {
        h.update(path.as_bytes());
        h.update([0]);
        match held {
            Some(d) => {
                h.update([1]);
                h.update(d);
            }
            None => h.update([2]),
        }
    }
    h.finalize()
        .iter()
        .take(8)
        .fold(String::new(), |mut out, b| {
            use std::fmt::Write as _;
            let _ = write!(out, "{b:02x}");
            out
        })
}

impl Returns {
    /// Before a file tool runs: keep what each file it may write holds now, the first time only.
    pub fn before(&mut self, world: &Path, tool: &str, input: &str) {
        for t in targets(tool, input) {
            let r = rel(world, Path::new(&t));
            if !r.is_empty() && !self.originals.contains_key(&r) {
                let now = held(world, &r);
                self.originals.insert(r, now);
            }
        }
    }

    /// After a write landed (`wrote` is not empty): the tree the ask's changed files stand in, how
    /// many times it has stood, and whether it is the starter. `None` when nothing was written.
    pub fn after(&mut self, world: &Path, tool_use_id: &str, wrote: &[PathBuf]) -> Option<Beat> {
        if wrote.is_empty() {
            return None;
        }
        for p in wrote {
            let r = rel(world, p);
            if !r.is_empty() {
                self.changed.insert(r);
            }
        }
        let now: BTreeMap<&str, Held> = self
            .changed
            .iter()
            .map(|p| (p.as_str(), held(world, p)))
            .collect();
        let starter = self.changed.iter().all(|p| {
            self.originals
                .get(p)
                .is_some_and(|was| Some(was) == now.get(p.as_str()))
        });
        let tree = digest(&now);
        let visit = if starter {
            0
        } else {
            let n = self.seen.entry(tree.clone()).or_default();
            *n += 1;
            *n
        };
        Some(Beat::Tree {
            tool_use_id: tool_use_id.to_owned(),
            digest: tree,
            visit,
            files: u32::try_from(self.changed.len()).unwrap_or(u32::MAX),
            starter,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::targets;

    /// A file tool's targets: its `path`, a move's `to`, a patch's files on both sides.
    #[test]
    fn a_calls_targets_are_read_from_its_input() {
        assert_eq!(
            targets("write", r#"{"path":"src/a.py","content":"x"}"#),
            ["src/a.py"]
        );
        assert_eq!(targets("move", r#"{"path":"a","to":"b"}"#), ["a", "b"]);
        let diff = "--- a/src/x.py\n+++ b/src/x.py\n@@ -1 +1 @@\n-a\n+b\n--- /dev/null\n+++ b/new.py\t2026\n";
        let input = serde_json::json!({ "patch": diff }).to_string();
        assert_eq!(targets("patch", &input), ["src/x.py", "src/x.py", "new.py"]);
        assert_eq!(targets("bash", "not json"), Vec::<String>::new());
    }
}
