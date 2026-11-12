//! A run's numbers, read from its journal alone: where its time and its context went —
//! `mnemorphd runs --metrics <run>`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use mnemorph_ledger::Journal;

use serde::Serialize;
use serde_json::Value;

use crate::beats::Beat;

/// One tool's share of a run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ToolMetrics {
    /// Calls answered.
    pub calls: u64,
    /// Bytes of their results.
    pub bytes: u64,
    /// Lines of their results.
    pub lines: u64,
    /// Results a cap cut.
    pub truncated: u64,
    /// Results that were errors.
    pub errors: u64,
    /// Milliseconds the timed calls took.
    pub ms: u64,
}

/// A run's numbers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Metrics {
    /// Model turns.
    pub turns: u64,
    /// Milliseconds in the model, over the timed turns.
    pub model_ms: u64,
    /// Turns journaled without a time (written before it was kept).
    pub untimed_turns: u64,
    /// Tool calls answered.
    pub calls: u64,
    /// Milliseconds in tools, over the timed calls.
    pub tool_ms: u64,
    /// Calls without a time: written before it was kept, interrupted, or orders run at once.
    pub untimed_calls: u64,
    /// Bytes the conversation carried: asks, notes, the model's text and calls, every result.
    /// Thinking is not counted, nor what a compaction later dropped.
    pub context_bytes: u64,
    /// Of those, the results'.
    pub result_bytes: u64,
    /// Per tool, by name.
    pub tools: BTreeMap<String, ToolMetrics>,
    /// Reads journaled with what they looked at.
    pub reads: u64,
    /// Of those, reads of lines of a file already read in the run (an unchanged skip included).
    pub rereads: u64,
    /// Of those, reads answered with a pointer: the window was unchanged.
    pub unchanged: u64,
    /// Acts that wrote a file.
    pub writes: u64,
    /// Lines they added.
    pub lines_added: u64,
    /// Lines they removed.
    pub lines_removed: u64,
    /// Whether a test ran after the last change to a code file that landed: a command that names
    /// a test runner or runs a test file ([`is_test`]). `None` for a run that changed no code
    /// file. Reported per run, and not acted on.
    pub tested_after_last_write: Option<bool>,
    /// Journal lines that did not read, and are not counted.
    pub unread_lines: u64,
}

impl Metrics {
    /// The share of reads that re-read lines, 0 to 1; `None` with no journaled read.
    #[must_use]
    pub fn reread_ratio(&self) -> Option<f64> {
        ratio(self.rereads, self.reads)
    }
}

/// The tools that change a file.
const WRITES: [&str; 6] = ["edit", "multiedit", "patch", "write", "move", "delete"];

/// The words of a test runner, as the bench's count reads them.
const RUNNERS: [&str; 10] = [
    "pytest",
    "unittest",
    "make test",
    "make check",
    "make unit",
    "-m tests",
    "cargo test",
    "npm test",
    "go test",
    "nextest",
];

/// Whether a shell command runs tests: it names a test runner, or runs a file under `tests/` or
/// named `test_*.py`. A probe (`python -c …`) is not a test.
#[must_use]
pub fn is_test(command: &str) -> bool {
    RUNNERS.iter().any(|r| command.contains(r))
        || command.split_whitespace().any(|w| {
            let name = w.rsplit('/').next().unwrap_or(w);
            w.starts_with("tests/")
                || w.starts_with("test/")
                || (name.starts_with("test_")
                    && std::path::Path::new(name)
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("py")))
        })
}

/// Read a run's numbers from its beats.
#[must_use]
pub fn metrics(beats: &[Beat]) -> Metrics {
    let mut m = Metrics::default();
    let mut names: HashMap<String, String> = HashMap::new();
    // Each call's tool and what it touched: the path a write names, the command a shell runs.
    let mut inputs: HashMap<String, (String, String)> = HashMap::new();
    // Per file, the line windows read so far; `None` is the skeleton.
    let mut seen: HashMap<String, Vec<Option<(usize, usize)>>> = HashMap::new();
    for beat in beats {
        match beat {
            Beat::Ask { text } | Beat::Note { text } => m.context_bytes += text.len() as u64,
            Beat::Model { content, ms, .. } => {
                m.turns += 1;
                match ms {
                    Some(ms) => m.model_ms += ms,
                    None => m.untimed_turns += 1,
                }
                for b in content {
                    m.context_bytes += said(b);
                    if b["type"] == "tool_use"
                        && let (Some(id), Some(name)) = (b["id"].as_str(), b["name"].as_str())
                    {
                        names.insert(id.to_owned(), name.to_owned());
                        let touched = b["input"]["path"]
                            .as_str()
                            .or_else(|| b["input"]["command"].as_str())
                            .unwrap_or_default();
                        inputs.insert(id.to_owned(), (name.to_owned(), touched.to_owned()));
                    }
                }
            }
            Beat::Result {
                tool_use_id,
                content,
                is_error,
                facts,
                ..
            } => {
                if let Some((tool, touched)) = inputs.get(tool_use_id) {
                    if WRITES.contains(&tool.as_str())
                        && !is_error
                        && !touched.contains(".mnemorph")
                    {
                        m.tested_after_last_write = Some(false);
                    } else if tool == "bash"
                        && is_test(touched)
                        && m.tested_after_last_write.is_some()
                    {
                        m.tested_after_last_write = Some(true);
                    }
                }
                let bytes = facts.bytes.unwrap_or(content.len() as u64);
                let lines = facts.lines.unwrap_or(content.lines().count() as u64);
                m.calls += 1;
                m.context_bytes += bytes;
                m.result_bytes += bytes;
                match facts.ms {
                    Some(ms) => m.tool_ms += ms,
                    None => m.untimed_calls += 1,
                }
                let name = names
                    .get(tool_use_id)
                    .map_or("(unknown)", String::as_str)
                    .to_owned();
                let t = m.tools.entry(name).or_default();
                t.calls += 1;
                t.bytes += bytes;
                t.lines += lines;
                t.truncated += u64::from(facts.truncated);
                t.errors += u64::from(*is_error);
                t.ms += facts.ms.unwrap_or(0);
                if let Some(read) = &facts.read {
                    m.reads += 1;
                    m.unchanged += u64::from(read.unchanged);
                    let earlier = seen.entry(read.path.clone()).or_default();
                    if read.unchanged || earlier.iter().any(|e| overlaps(*e, read.lines)) {
                        m.rereads += 1;
                    }
                    earlier.push(read.lines);
                }
            }
            Beat::Guts {
                wrote,
                lines_added,
                lines_removed,
                ..
            } => {
                m.writes += u64::from(*wrote > 0);
                m.lines_added += u64::from(*lines_added);
                m.lines_removed += u64::from(*lines_removed);
            }
            _ => {}
        }
    }
    m
}

/// The numbers of the run `which` names in `world` — its id, `last` for the journal written last,
/// or a journal's path — with the run's id.
///
/// # Errors
///
/// No such run, or its journal cannot be read.
pub fn of_run(world: &Path, which: &str) -> Result<(String, Metrics), String> {
    let dir = crate::resume::journals(world);
    let path = if which == "last" {
        std::fs::read_dir(&dir)
            .map_err(|e| format!("no run journals in {}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
            .ok_or_else(|| format!("no run journal in {}", dir.display()))?
    } else if Path::new(which)
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("jsonl"))
    {
        world.join(which)
    } else {
        dir.join(format!("{which}.jsonl"))
    };
    if !path.is_file() {
        return Err(format!(
            "no run `{which}`: {} does not exist",
            path.display()
        ));
    }
    let run = path
        .file_stem()
        .map_or_else(|| which.to_owned(), |s| s.to_string_lossy().into_owned());
    let back = Journal::<Beat>::new(path.clone())
        .read()
        .map_err(|e| format!("run `{run}`: its journal cannot be read: {e}"))?;
    let mut m = metrics(&back.rows);
    m.unread_lines = back.skipped.len() as u64;
    Ok((run, m))
}

/// Whether two read windows share a line; two skeleton reads are the same read.
fn overlaps(a: Option<(usize, usize)>, b: Option<(usize, usize)>) -> bool {
    match (a, b) {
        (Some((a0, a1)), Some((b0, b1))) => a0 <= b1 && b0 <= a1,
        (None, None) => true,
        _ => false,
    }
}

/// The bytes a model block puts in the conversation: its text, or its call's name and input.
fn said(b: &Value) -> u64 {
    match b["type"].as_str() {
        Some("text") => b["text"].as_str().map_or(0, str::len) as u64,
        Some("tool_use") => {
            (b["name"].as_str().map_or(0, str::len) + b["input"].to_string().len()) as u64
        }
        _ => 0,
    }
}

fn ratio(part: u64, whole: u64) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)]
    (whole > 0).then(|| part as f64 / whole as f64)
}

fn pct(part: u64, whole: u64) -> String {
    ratio(part, whole).map_or_else(|| "—".to_owned(), |r| format!("{:.0}%", r * 100.0))
}

fn size(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let b = bytes as f64;
    if bytes >= 1 << 20 {
        format!("{:.1} MB", b / f64::from(1 << 20))
    } else if bytes >= 1 << 10 {
        format!("{:.1} KB", b / f64::from(1 << 10))
    } else {
        format!("{bytes} B")
    }
}

fn secs(ms: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let s = ms as f64 / 1000.0;
    format!("{s:.1}s")
}

/// The numbers as the human reads them.
#[must_use]
pub fn render(run: &str, m: &Metrics) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{run}: {} model turns, {} tool calls", m.turns, m.calls);
    let timed = m.model_ms + m.tool_ms;
    let _ = write!(
        s,
        "time      model {} ({}), tools {} ({})",
        secs(m.model_ms),
        pct(m.model_ms, timed),
        secs(m.tool_ms),
        pct(m.tool_ms, timed)
    );
    if m.untimed_turns + m.untimed_calls > 0 {
        let _ = write!(
            s,
            "; untimed: {} turns, {} calls",
            m.untimed_turns, m.untimed_calls
        );
    }
    let _ = writeln!(
        s,
        "\ncontext   {} carried, tool results {} ({})",
        size(m.context_bytes),
        size(m.result_bytes),
        pct(m.result_bytes, m.context_bytes)
    );
    let mut tools: Vec<_> = m.tools.iter().collect();
    tools.sort_by(|a, b| b.1.bytes.cmp(&a.1.bytes).then_with(|| a.0.cmp(b.0)));
    let wide = tools.iter().map(|(n, _)| n.len()).max().unwrap_or(4).max(4);
    let _ = writeln!(
        s,
        "\n{:<wide$}  {:>5}  {:>9}  {:>7}  {:>8}  {:>9}  {:>5}",
        "tool", "calls", "bytes", "context", "lines", "time", "cut"
    );
    for (name, t) in tools {
        let _ = writeln!(
            s,
            "{name:<wide$}  {:>5}  {:>9}  {:>7}  {:>8}  {:>9}  {:>5}",
            t.calls,
            size(t.bytes),
            pct(t.bytes, m.context_bytes),
            t.lines,
            secs(t.ms),
            t.truncated
        );
    }
    let _ = write!(s, "\nreads     ");
    if m.reads == 0 {
        let _ = writeln!(s, "none journaled with what they read");
    } else {
        let _ = writeln!(
            s,
            "{}, re-read {} ({}), skipped as unchanged {}",
            m.reads,
            m.rereads,
            pct(m.rereads, m.reads),
            m.unchanged
        );
    }
    let _ = writeln!(
        s,
        "written   +{} −{} lines in {} acts",
        m.lines_added, m.lines_removed, m.writes
    );
    if let Some(tested) = m.tested_after_last_write {
        let _ = writeln!(
            s,
            "tests     {} after the last code write",
            if tested { "a test ran" } else { "none" }
        );
    }
    if m.unread_lines > 0 {
        let _ = writeln!(
            s,
            "\n{} journal lines did not read and are not counted",
            m.unread_lines
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beats::Facts;
    use mnemorph_tools::ReadSpan;
    use serde_json::json;

    fn result(id: &str, content: &str, ms: Option<u64>, read: Option<ReadSpan>) -> Beat {
        Beat::Result {
            tool_use_id: id.into(),
            content: content.into(),
            is_error: false,
            references: Vec::new(),
            refused: None,
            facts: Facts {
                read,
                ..Facts::of(content, ms, &mnemorph_tools::Meta::default())
            },
        }
    }

    fn span(path: &str, lines: (usize, usize), unchanged: bool) -> ReadSpan {
        ReadSpan {
            path: path.into(),
            lines: Some(lines),
            unchanged,
        }
    }

    #[test]
    fn a_journal_reads_as_time_bytes_rereads_and_writes() {
        let beats = vec![
            Beat::Ask {
                text: "fix it".into(),
            },
            Beat::Model {
                content: vec![
                    json!({"type": "text", "text": "look"}),
                    json!({"type": "tool_use", "id": "a", "name": "read", "input": {"path": "x"}}),
                    json!({"type": "tool_use", "id": "b", "name": "read", "input": {"path": "x"}}),
                    json!({"type": "tool_use", "id": "c", "name": "read", "input": {"path": "x"}}),
                    json!({"type": "tool_use", "id": "d", "name": "bash", "input": {}}),
                ],
                stop: "tool_use".into(),
                at: None,
                ms: Some(3000),
                id: None,
            },
            result("a", "1\n2\n3", Some(10), Some(span("x", (1, 40), false))),
            result("b", "pointer", Some(5), Some(span("x", (1, 40), true))),
            result("c", "41", Some(5), Some(span("x", (41, 80), false))),
            result("d", "ok\n", Some(980), None),
            Beat::Guts {
                tool_use_id: "d".into(),
                tool: "bash".into(),
                score: 2,
                window: 2,
                warned: false,
                failed: false,
                wrote: 1,
                lines_added: 7,
                lines_removed: 2,
                edit_miss: false,
                bench: None,
                bench_outcome: None,
                exit: Some(0),
                classes: Vec::new(),
            },
        ];
        let m = metrics(&beats);
        assert_eq!(
            (m.turns, m.model_ms, m.calls, m.tool_ms),
            (1, 3000, 4, 1000)
        );
        assert_eq!((m.reads, m.rereads, m.unchanged), (3, 1, 1));
        assert_eq!(m.tools["read"].calls, 3);
        assert_eq!(m.tools["read"].bytes, 5 + 7 + 2);
        assert_eq!(m.tools["bash"].lines, 1);
        assert_eq!(m.result_bytes, 17);
        assert!(m.context_bytes > m.result_bytes);
        assert_eq!((m.writes, m.lines_added, m.lines_removed), (1, 7, 2));
        let shown = render("r1", &m);
        assert!(
            shown.contains("model 3.0s (75%), tools 1.0s (25%)"),
            "{shown}"
        );
        assert!(
            shown.contains("re-read 1 (33%), skipped as unchanged 1"),
            "{shown}"
        );
    }

    fn call(id: &str, name: &str, input: &serde_json::Value) -> Beat {
        Beat::Model {
            content: vec![json!({"type": "tool_use", "id": id, "name": name, "input": input})],
            stop: "tool_use".into(),
            at: None,
            ms: None,
            id: None,
        }
    }

    /// Whether a test ran after the last change to a code file: a command that names a test
    /// runner or runs a test file, after the last write outside `.mnemorph` that landed. A probe
    /// is not a test, a write to the plan is not code, and a run that wrote no code says neither.
    #[test]
    fn a_test_after_the_last_code_write_is_read_from_the_journal() {
        let bash = |id: &str, cmd: &str| call(id, "bash", &json!({"command": cmd}));
        let write = |id: &str, path: &str| call(id, "edit", &json!({"path": path}));
        let ok = |id: &str| result(id, "ok", None, None);
        let tested = |beats: Vec<Beat>| metrics(&beats).tested_after_last_write;
        assert_eq!(
            tested(vec![bash("a", "pytest -q"), ok("a")]),
            None,
            "no code written"
        );
        assert_eq!(
            tested(vec![
                write("a", "src/x.py"),
                ok("a"),
                bash("b", "python3 -m unittest tests.test_x"),
                ok("b")
            ]),
            Some(true)
        );
        assert_eq!(
            tested(vec![
                bash("a", "pytest -q"),
                ok("a"),
                write("b", "src/x.py"),
                ok("b"),
                bash("c", "python3 -c 'import x; print(x.f())'"),
                ok("c")
            ]),
            Some(false),
            "a probe after the last write is no test"
        );
        assert_eq!(
            tested(vec![
                write("a", "src/x.py"),
                ok("a"),
                bash("b", "python tests/test_x.py"),
                ok("b"),
                write("c", ".mnemorph/tmp/plan.md"),
                ok("c")
            ]),
            Some(true),
            "a write to the plan is not a code write"
        );
        let failed = Beat::Result {
            tool_use_id: "c".into(),
            content: "no match".into(),
            is_error: true,
            references: Vec::new(),
            refused: None,
            facts: Facts::of("no match", None, &mnemorph_tools::Meta::default()),
        };
        assert_eq!(
            tested(vec![
                write("a", "src/x.py"),
                ok("a"),
                bash("b", "make test"),
                ok("b"),
                write("c", "src/x.py"),
                failed
            ]),
            Some(true),
            "an edit that did not apply changed nothing"
        );
        let shown = render("r2", &metrics(&[write("a", "src/x.py"), ok("a")]));
        assert!(
            shown.contains("tests     none after the last code write"),
            "{shown}"
        );
    }

    #[test]
    fn an_old_journal_is_sized_from_its_text_and_counted_untimed() {
        let old = r#"{"beat":"result","tool_use_id":"a","content":"abc\ndef","is_error":false}"#;
        let beat: Beat = serde_json::from_str(old).unwrap();
        let m = metrics(&[beat]);
        assert_eq!((m.calls, m.untimed_calls, m.result_bytes), (1, 1, 7));
        assert_eq!(m.tools["(unknown)"].lines, 2);
        assert!(render("r0", &m).contains("none journaled with what they read"));
    }

    #[test]
    fn the_facts_lie_flat_beside_the_result_and_read_back() {
        let beat = result("a", "x", Some(4), Some(span("src/lib.rs", (3, 9), true)));
        let line = serde_json::to_value(&beat).unwrap();
        assert_eq!(line["ms"], 4);
        assert_eq!(line["bytes"], 1);
        assert_eq!(line["read"]["lines"], json!([3, 9]));
        assert!(line.get("truncated").is_none());
        let back: Beat = serde_json::from_value(line).unwrap();
        assert_eq!(back, beat);
    }
}
