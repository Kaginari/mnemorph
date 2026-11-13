//! The `## Done when` list as the chain carries it: lines named `D1`, `D2`, …, one fact a line.
//! An order names the lines it must make true; the brief carries their text; a verify line names
//! the lines it proves; and the report comes back with one mark a line, from the checks the
//! harness ran below the order, never from what a body said.

use std::path::Path;

use mnemorph_world::Roster;

use crate::gate::GateVerdict;

/// One line of a list: its name and its text.
pub type Line = (String, String);

/// Whether `word` is a line's name: `D` and a number.
#[must_use]
pub fn is_name(word: &str) -> bool {
    word.strip_prefix('D')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The lines under a plan's `## Done when`, each `- D1: <text>` (or `D1 <text>`), in order. A
/// line with no name is not part of the list the chain can point at.
#[must_use]
pub fn list(plan: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in plan.lines() {
        let t = line.trim();
        if let Some(head) = t.strip_prefix("## ") {
            inside = head.trim() == "Done when";
            continue;
        }
        if !inside {
            continue;
        }
        let t = t.trim_start_matches(['-', '*']).trim_start();
        let Some((name, rest)) = t.split_once([':', ' ']) else {
            continue;
        };
        if is_name(name) {
            out.push((name.to_owned(), rest.trim().to_owned()));
        }
    }
    out
}

/// The list a dispatcher's orders may name: the lines its own order carried down to it, or else
/// the `## Done when` of its own plan: its seat's `plan.md`, and for the Summoner the plan solo's
/// prompt has it write, `.mnemorph/tmp/plan.md`, or the collective's board.
#[must_use]
pub fn of_dispatcher(world: &Path, roster: &Roster, name: &str, carried: &[Line]) -> Vec<Line> {
    if !carried.is_empty() {
        return carried.to_vec();
    }
    let mut plans = Vec::new();
    if let Some(c) = roster.creatures.get(name) {
        plans.push(world.join(&c.seat).join("plan.md"));
    }
    if name == "summoner" {
        plans.push(world.join(".mnemorph/tmp/plan.md"));
        // In the collective the board is the Summoner's plan.
        plans.push(world.join(crate::collective::BOARD));
    }
    plans
        .iter()
        .filter(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file()))
        .map(|p| list(&std::fs::read_to_string(p).unwrap_or_default()))
        .find(|l| !l.is_empty())
        .unwrap_or_default()
}

/// The lines `names` picks from `list`, and the names that are not on it.
#[must_use]
pub fn pick(names: &[String], list: &[Line]) -> (Vec<Line>, Vec<String>) {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for n in names {
        match list.iter().find(|(name, _)| name == n) {
            Some(l) if !found.contains(l) => found.push(l.clone()),
            Some(_) => {}
            None => missing.push(n.clone()),
        }
    }
    (found, missing)
}

/// The brief's part for its lines: their text, each on its own line, and the names not found.
#[must_use]
pub fn brief(found: &[Line], missing: &[String]) -> String {
    use std::fmt::Write as _;
    if found.is_empty() && missing.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n\nDone when: the lines of your dispatcher's list this order must make true. Answer `@V \
<line> <what you ran and what it gave>` for each; the mark is the checks', not yours.",
    );
    for (name, text) in found {
        let _ = write!(out, "\n- {name}: {text}");
    }
    for name in missing {
        let _ = write!(
            out,
            "\n- {name} is not on your dispatcher's list: ask it what it means."
        );
    }
    out
}

/// The names a verify line carries: a trailing `# D4` or `# D4, D5`. None for a line with none.
#[must_use]
pub fn tags(command: &str) -> Vec<String> {
    mnemorph_world::verify::names(command)
}

/// `command` with the lines it proves written after it, `# D4, D5`.
///
/// # Errors
///
/// A name that is not `D` and a number.
pub fn tagged(command: &str, line: &str) -> Result<String, String> {
    let names: Vec<&str> = line
        .split([',', ' '])
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect();
    if names.is_empty() || !names.iter().all(|n| is_name(n)) {
        return Err(format!(
            "`{line}` names no line: a line is named `D1`, `D2`, … as on your brief's list"
        ));
    }
    Ok(format!("{} # {}", command.trim(), names.join(", ")))
}

/// One mark a name, from the gate verdicts judged below an order, the latest last: the newest
/// result of a verify line that carries the name decides. `PASS` and `FAIL` say whose pin the
/// line was and the line itself; `FAIL` adds the failure's first line; a line that gave no
/// result, or no line at all, leaves the name unchecked.
#[must_use]
pub fn marks(names: &[String], verdicts: &[GateVerdict]) -> Vec<String> {
    let mut seen: Vec<&String> = Vec::new();
    names
        .iter()
        .filter(|n| {
            let new = !seen.contains(n);
            seen.push(n);
            new
        })
        .map(|name| mark(name, verdicts))
        .collect()
}

fn mark(name: &str, verdicts: &[GateVerdict]) -> String {
    for v in verdicts.iter().rev() {
        let by = v.verified_by.join(", ");
        if let Some(line) = v.ran.iter().find(|l| tags(l).iter().any(|t| t == name)) {
            let quoted = format!("`{line}`");
            if v.unverified.iter().any(|u| u.contains(&quoted)) {
                return format!("{name} unchecked: {line} gave no result");
            }
            // Open: it failed before this landing and fails still. The landing stood; the line
            // is not true yet.
            if let Some(o) = v.open.iter().find(|o| o.contains(&quoted)) {
                let first = o
                    .split_once("landing:")
                    .map_or("", |(_, tail)| tail)
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("it failed");
                return format!(
                    "{name} FAIL  {by}: {line}: {first} (red before this landing too: it needs \
another part)"
                );
            }
            if let Some(f) = v.failures.iter().find(|f| f.contains(&quoted)) {
                let first = f
                    .split_once("failed:")
                    .map_or("", |(_, tail)| tail)
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("it failed");
                return format!("{name} FAIL  {by}: {line}: {first}");
            }
            return format!("{name} PASS  {by}: {line}");
        }
    }
    format!("{name} unchecked: no verify line names it")
}

/// `report` with `marks` under its status line.
#[must_use]
pub fn under_status(report: &str, marks: &[String]) -> String {
    if marks.is_empty() {
        return report.to_owned();
    }
    let (first, rest) = report.split_once('\n').unwrap_or((report, ""));
    format!("{first}\n{}\n{rest}", marks.join("\n"))
        .trim_end_matches('\n')
        .to_owned()
        + if report.ends_with('\n') { "\n" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::{is_name, list, marks, pick, tagged, tags, under_status};
    use crate::gate::GateVerdict;

    fn verdict(ran: &[&str], failures: &[&str]) -> GateVerdict {
        GateVerdict {
            changed: Vec::new(),
            failures: failures.iter().map(|s| (*s).to_owned()).collect(),
            unverified: Vec::new(),
            unguarded: false,
            wrote: false,
            unpinned: false,
            verified_by: vec!["orc-gen's pin".to_owned()],
            ran: ran.iter().map(|s| (*s).to_owned()).collect(),
            open: Vec::new(),
        }
    }

    /// The list reads named lines under `## Done when` only, in the forms a model writes them.
    #[test]
    fn the_list_is_the_named_lines_under_done_when() {
        let plan = "## Done when\n- D1: tsql to postgres, `DAY(x)` gives `EXTRACT(DAY FROM x)`\n* D2 keep: MySQL unchanged\n- a line with no name\n\n## Paths\n- D3: not in the list\n";
        let l = list(plan);
        assert_eq!(
            l.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["D1", "D2"]
        );
        assert_eq!(l[1].1, "keep: MySQL unchanged");
        let (found, missing) = pick(&["D2".into(), "D2".into(), "D7".into()], &l);
        assert_eq!((found.len(), missing), (1, vec!["D7".to_owned()]));
        assert!(is_name("D12") && !is_name("D") && !is_name("Dx") && !is_name("d1"));
    }

    /// A verify line's names ride after it, and only names: a comment in the command is not one.
    #[test]
    fn a_verify_line_carries_its_names_after_it() {
        assert_eq!(tagged("pytest -q", "D4, D5").unwrap(), "pytest -q # D4, D5");
        assert!(tagged("pytest -q", "the parser").is_err());
        assert_eq!(tags("pytest -q # D4, D5"), ["D4", "D5"]);
        assert_eq!(
            tags("grep -c '# TODO' src # not a name"),
            Vec::<String>::new()
        );
        assert_eq!(tags("pytest -q"), Vec::<String>::new());
    }

    /// The newest verdict on a named line decides its mark; a name no line carries is unchecked.
    #[test]
    fn the_newest_check_marks_a_line() {
        let old = verdict(
            &["t1 # D1", "t2 # D2"],
            &["traits hold: `t2 # D2` failed:\n\nAssertionError: not raised\nmore"],
        );
        let new = verdict(&["t1 # D1"], &[]);
        let names: Vec<String> = ["D1", "D2", "D3", "D1"].map(String::from).to_vec();
        assert_eq!(
            marks(&names, &[old, new]),
            [
                "D1 PASS  orc-gen's pin: t1 # D1",
                "D2 FAIL  orc-gen's pin: t2 # D2: AssertionError: not raised",
                "D3 unchecked: no verify line names it",
            ]
        );
    }

    /// A line that stands open is no pass: its mark is FAIL, and says the landing did not break
    /// it. It is not among the lines that held, so no proof cites it; a landing whose every line
    /// stands open was checked by nothing.
    #[test]
    fn an_open_line_is_marked_fail_and_holds_nothing() {
        let mut v = verdict(&["t1 # D1", "unit"], &[]);
        v.changed = vec![std::path::PathBuf::from("src/a.rs")];
        v.open = vec![
            "traits open: `t1 # D1` fails as it did before this landing:\n\nParseError: no\nmore"
                .to_owned(),
        ];
        assert!(v.passed(), "the landing stands");
        assert_eq!(v.held(), ["unit"]);
        assert!(!v.unchecked(), "its own check held");
        let names: Vec<String> = ["D1"].map(String::from).to_vec();
        assert_eq!(
            marks(&names, std::slice::from_ref(&v)),
            [
                "D1 FAIL  orc-gen's pin: t1 # D1: ParseError: no (red before this landing too: it \
needs another part)"
            ]
        );
        v.ran = vec!["t1 # D1".to_owned()];
        assert_eq!(v.held(), [] as [String; 0]);
        assert!(v.unchecked(), "nothing held over the change");
    }

    /// The marks go under the status line, the rest of the report after them.
    #[test]
    fn the_marks_go_under_the_status_line() {
        let m = vec!["D1 PASS  x".to_owned()];
        assert_eq!(
            under_status("@S DONE\n@W a\n@E", &m),
            "@S DONE\nD1 PASS  x\n@W a\n@E"
        );
        assert_eq!(under_status("@S DONE", &m), "@S DONE\nD1 PASS  x");
        assert_eq!(under_status("@S DONE\n@E", &[]), "@S DONE\n@E");
    }

    /// A line's own verdict is never read as one of the wire's typed `@V` forms: a label starts
    /// with `#`, an acceptance with `accept`.
    #[test]
    fn a_line_verdict_is_not_a_typed_verdict() {
        let r = mnemorph_wire::Report::parse("@S DONE\n@V D4 FAIL it raised nothing\n@E").unwrap();
        assert_eq!(r.verdicts, ["D4 FAIL it raised nothing"]);
        assert!(!r.verdicts[0].starts_with('#') && !r.verdicts[0].starts_with("accept"));
    }
}
