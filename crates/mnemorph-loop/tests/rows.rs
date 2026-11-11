//! The statement rows' reading: what a script's output says, and what the loop tells the body.

use std::fmt::Write as _;

use mnemorph_loop::rows::{Rows, parse, writes};

#[test]
fn a_rows_script_is_read_line_by_line() {
    let sheet = parse(
        "noise\nROW 1 PASS alias kept\n  ROW 2 FAIL clause: missing\nROW 3 MAYBE x\nROW 4\nROW 1 FAIL alias kept: 'x'\n",
    );
    assert_eq!(sheet.len(), 2);
    assert_eq!(sheet["1"], (false, "alias kept: 'x'".to_owned()));
    assert_eq!(sheet["2"], (false, "clause: missing".to_owned()));
}

#[test]
fn only_a_row_that_passed_at_the_start_and_newly_fails_is_a_break() {
    let start = parse("ROW 1 PASS a\nROW 2 FAIL b\nROW 3 PASS c\n");
    let mut rows = Rows::begin(&start);
    // Row 2 never passed; row 3 broke.
    let said = rows
        .after_write(&parse("ROW 1 PASS a\nROW 2 FAIL b\nROW 3 FAIL c: gone\n"))
        .unwrap();
    assert!(
        said.contains("ROW 3 c: gone") && !said.contains("ROW 2"),
        "{said}"
    );
    // Still broken: not said again.
    assert_eq!(rows.after_write(&parse("ROW 3 FAIL c: gone\n")), None);
    // Fixed, then broken again: said again.
    assert_eq!(rows.after_write(&parse("ROW 3 PASS c\n")), None);
    assert!(rows.after_write(&parse("ROW 3 FAIL c: again\n")).is_some());
}

#[test]
fn what_still_fails_is_said_once_before_the_answer() {
    let mut rows = Rows::begin(&parse("ROW 1 FAIL a\n"));
    assert_eq!(rows.before_answer(&parse("ROW 1 PASS a\n")), None);
    let mut many = String::new();
    for i in 1..=10 {
        let _ = writeln!(many, "ROW {i} FAIL r{i}");
    }
    let said = rows.before_answer(&parse(&many)).unwrap();
    assert!(
        said.contains("ROW 1 r1") && said.contains("and 2 more"),
        "{said}"
    );
    assert_eq!(rows.before_answer(&parse(&many)), None);
}

#[test]
fn only_file_tools_count_as_writes() {
    assert!(writes("edit") && writes("multiedit") && writes("write"));
    assert!(writes("patch") && writes("move") && writes("delete"));
    assert!(!writes("bash") && !writes("read"));
}
