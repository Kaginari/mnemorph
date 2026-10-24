//! The classifier's corpus: every row must hold, every run (lessons: a deny rule held only
//! against the literal command; 37 of 90 must-block commands slipped past before a corpus).

use mnemorph_races::ToolClass;
use mnemorph_tools::{Place, Verdict, classify};

const CORPUS: &str = include_str!("corpus.txt");

fn place() -> Place {
    Place::new("/w", "/h")
}

fn label(v: &Verdict) -> &'static str {
    match v.class() {
        None => "refuse",
        Some(ToolClass::Read) => "read",
        Some(ToolClass::Write) => "write",
        Some(ToolClass::Outward) => "outward",
        Some(_) => "destructive",
    }
}

#[test]
fn every_row_holds() {
    let mut wrong = Vec::new();
    let mut rows = 0;
    for line in CORPUS.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (want, cmd) = line.split_once(' ').expect("a row is `<class> <command>`");
        let cmd = cmd.trim_start().replace("\\n", "\n");
        let got = classify(&cmd, &place());
        rows += 1;
        if label(&got) != want {
            wrong.push(format!(
                "{want:<11} got {:<11} {cmd:?}  {got:?}",
                label(&got)
            ));
        }
    }
    assert!(rows > 200, "the corpus lost rows: {rows}");
    assert!(
        wrong.is_empty(),
        "{} of {rows} rows wrong:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// A floor refusal is never reported under a rule the human may ask, whatever came first.
#[test]
fn the_floor_wins_over_an_askable_rule() {
    use mnemorph_tools::Rule;
    for cmd in [
        "sed -i s/a/b/ /dev/sda",
        "cp ~/.ssh/id_rsa /dev/sda",
        "cat ~/.ssh/id_rsa > /dev/sda",
    ] {
        match classify(cmd, &place()) {
            Verdict::Refuse { rule, .. } => {
                assert!(rule.floor() && rule == Rule::Device, "{cmd}: {rule:?}");
            }
            v @ Verdict::Run { .. } => panic!("{cmd}: {v:?}"),
        }
    }
}
