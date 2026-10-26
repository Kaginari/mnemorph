//! What a file tool changes, as a diff: `edit` and `multiedit` from their old and new strings,
//! `patch` from its unified diff, `write` as the new file's head.

use serde_json::Value;
use similar::{ChangeTag, TextDiff};

/// One line of a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Unchanged context.
    Same(String),
    /// Added.
    Add(String),
    /// Removed.
    Del(String),
    /// A hunk's header or a gap between hunks.
    Hunk(String),
}

/// A file tool's change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A diff, and the lines it adds and removes.
    Diff {
        /// Its lines.
        lines: Vec<DiffLine>,
        /// Lines added.
        added: usize,
        /// Lines removed.
        removed: usize,
    },
    /// A file written whole.
    NewFile {
        /// Its path, for the head's colours.
        path: String,
        /// How many lines it has.
        lines: usize,
        /// Its first lines.
        head: Vec<String>,
    },
}

/// The context kept around each change.
const CONTEXT: usize = 2;
/// The lines of a written file shown.
pub const HEAD: usize = 4;

/// The change a call makes, when it is a file tool's.
#[must_use]
pub fn of(name: &str, input: &Value) -> Option<Change> {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    match name {
        "edit" => Some(edits(&[(s(input, "old_string")?, s(input, "new_string")?)])),
        "multiedit" => {
            let pairs: Vec<(String, String)> = input
                .get("edits")?
                .as_array()?
                .iter()
                .filter_map(|e| Some((s(e, "old_string")?, s(e, "new_string")?)))
                .collect();
            Some(edits(&pairs))
        }
        "patch" => Some(patch(&s(input, "patch")?)),
        "write" => {
            let content = s(input, "content")?;
            Some(Change::NewFile {
                path: s(input, "path").unwrap_or_default(),
                lines: content.lines().count(),
                head: content.lines().take(HEAD).map(str::to_owned).collect(),
            })
        }
        _ => None,
    }
}

/// Each old→new pair diffed by line, a gap between pairs and between distant hunks.
fn edits(pairs: &[(String, String)]) -> Change {
    let mut lines = Vec::new();
    let (mut added, mut removed) = (0, 0);
    for (old, new) in pairs {
        let diff = TextDiff::from_lines(old.as_str(), new.as_str());
        for group in diff.grouped_ops(CONTEXT) {
            if !lines.is_empty() {
                lines.push(DiffLine::Hunk("⋯".into()));
            }
            for op in &group {
                for c in diff.iter_changes(op) {
                    let text = c.value().trim_end_matches('\n').to_owned();
                    lines.push(match c.tag() {
                        ChangeTag::Equal => DiffLine::Same(text),
                        ChangeTag::Insert => {
                            added += 1;
                            DiffLine::Add(text)
                        }
                        ChangeTag::Delete => {
                            removed += 1;
                            DiffLine::Del(text)
                        }
                    });
                }
            }
        }
    }
    Change::Diff {
        lines,
        added,
        removed,
    }
}

/// A unified diff read line by line; its file headers dropped, its hunk headers kept.
fn patch(text: &str) -> Change {
    let mut lines = Vec::new();
    let (mut added, mut removed) = (0, 0);
    for l in text.lines() {
        if l.starts_with("+++") || l.starts_with("---") || l.starts_with("diff ") {
            continue;
        }
        lines.push(if let Some(r) = l.strip_prefix('+') {
            added += 1;
            DiffLine::Add(r.to_owned())
        } else if let Some(r) = l.strip_prefix('-') {
            removed += 1;
            DiffLine::Del(r.to_owned())
        } else if l.starts_with("@@") {
            DiffLine::Hunk(l.to_owned())
        } else {
            DiffLine::Same(l.strip_prefix(' ').unwrap_or(l).to_owned())
        });
    }
    Change::Diff {
        lines,
        added,
        removed,
    }
}

impl Change {
    /// The `+3 −1` summary, or the written file's size.
    #[must_use]
    pub fn stat(&self) -> String {
        match self {
            Change::Diff { added, removed, .. } => format!("+{added} −{removed}"),
            Change::NewFile { lines, .. } => format!("{lines} lines"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_edit_is_a_unified_diff() {
        let c = of(
            "edit",
            &json!({"path": "p.py", "old_string": "a\nreturn this\nb\n", "new_string": "a\nreturn self._parse_collate(this)\nb\n"}),
        )
        .unwrap();
        let Change::Diff {
            lines,
            added,
            removed,
        } = c
        else {
            panic!()
        };
        assert_eq!((added, removed), (1, 1));
        assert!(lines.contains(&DiffLine::Del("return this".into())));
        assert!(lines.contains(&DiffLine::Add("return self._parse_collate(this)".into())));
        assert!(lines.contains(&DiffLine::Same("a".into())));
    }

    #[test]
    fn a_patch_keeps_its_hunks() {
        let c = of(
            "patch",
            &json!({"patch": "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n ctx\n"}),
        )
        .unwrap();
        assert_eq!(c.stat(), "+1 −1");
        let Change::Diff { lines, .. } = c else {
            panic!()
        };
        assert_eq!(lines[0], DiffLine::Hunk("@@ -1 +1 @@".into()));
        assert_eq!(lines[3], DiffLine::Same("ctx".into()));
    }

    #[test]
    fn a_write_is_its_head() {
        let c = of(
            "write",
            &json!({"path": "n.md", "content": "1\n2\n3\n4\n5\n6\n"}),
        )
        .unwrap();
        assert_eq!(c.stat(), "6 lines");
        let Change::NewFile { head, .. } = c else {
            panic!()
        };
        assert_eq!(head.len(), HEAD);
        assert!(of("read", &json!({"path": "x"})).is_none());
    }
}
