//! `mnemorph run --gate-tty`: the card escapes what a model chose, keys typed before it was shown
//! are no answer, and only an explicit yes approves.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mnemorph_loop::{GateAnswer, GateAsk, ToolClass};
use mnemorph_races::Race;
use mnemorph_tools::Pending;
use mnemorph_tui::tty_gate::{Tty, approves, card, put};

/// A terminal: `typed` is what waits in its input; `settle` throws it away, as `tcflush` does, and
/// then `after` arrives — what the human types once the card is on the screen. Every call is
/// logged.
#[derive(Default)]
struct Fake {
    log: Arc<Mutex<Vec<String>>>,
    typed: VecDeque<String>,
    after: VecDeque<String>,
}

impl Tty for Fake {
    fn show(&mut self, _card: &str) -> io::Result<()> {
        self.log.lock().unwrap().push("show".into());
        Ok(())
    }

    fn settle(&mut self) -> io::Result<()> {
        self.log.lock().unwrap().push("settle".into());
        self.typed.clear();
        self.typed.append(&mut self.after);
        Ok(())
    }

    fn read_line(&mut self, line: &mut String) -> io::Result<usize> {
        self.log.lock().unwrap().push("read".into());
        Ok(self.typed.pop_front().map_or(0, |l| {
            line.push_str(&l);
            l.len()
        }))
    }
}

fn ask(act: &str) -> GateAsk {
    GateAsk {
        body: "rimuru".into(),
        race: Race::Rimuru,
        class: ToolClass::Write,
        act: act.into(),
        why: vec!["guard rule `in-place-edit` asks\r\u{1b}[2K· approved already".into()],
        outside: vec![],
        fingerprint: None,
    }
}

/// The answer when `typeahead` was typed before the card and `after` once it was shown, and the
/// order the terminal was used in.
fn answer(typeahead: &[&str], after: &[&str]) -> (GateAnswer, Vec<String>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let fake = Fake {
        log: Arc::clone(&log),
        typed: typeahead.iter().map(|s| (*s).to_owned()).collect(),
        after: after.iter().map(|s| (*s).to_owned()).collect(),
    };
    let got = put(
        fake,
        &ask("sed -i s/a/b/ f"),
        Duration::from_secs(5),
        &Pending::new(),
    );
    let log = log.lock().unwrap().clone();
    (got, log)
}

#[test]
fn a_hostile_act_is_shown_escaped_and_whole() {
    let long = format!("rm x{}\r\u{1b}[2Kecho harmless", " ".repeat(300));
    let c = card(&ask(&long), Duration::from_secs(600));
    for bad in ['\r', '\u{1b}'] {
        assert!(!c.contains(bad), "{bad:?} on the card: {c:?}");
    }
    assert!(c.contains("\\u{d}\\u{1b}[2Kecho harmless"), "{c}");
    assert!(c.contains("asks\\u{d}\\u{1b}[2K· approved already"), "{c}");
    assert!(c.contains(&long.replace('\r', "\\u{d}").replace('\u{1b}', "\\u{1b}")));
    assert!(c.contains("denied in 600s"));
}

#[test]
fn typeahead_before_the_card_never_approves() {
    let (got, log) = answer(&["y\n"], &[]);
    assert!(matches!(got, GateAnswer::Deny(_)), "{got:?}");
    assert_eq!(
        log,
        ["show", "settle", "read"],
        "flushed after showing, before reading"
    );
}

#[test]
fn only_a_yes_typed_after_the_card_approves() {
    assert_eq!(answer(&[], &["y\n"]).0, GateAnswer::Approve);
    assert_eq!(answer(&["n\n"], &["yes\n"]).0, GateAnswer::Approve);
    for no in ["n\n", "\n", "yy\n", "sure\n"] {
        assert!(
            matches!(answer(&[], &[no]).0, GateAnswer::Deny(_)),
            "{no:?}"
        );
    }
    assert!(approves(" Y \n") && approves("yes") && !approves("yes please"));
}

#[test]
fn the_end_of_input_denies() {
    let (got, _) = answer(&[], &[]);
    let GateAnswer::Deny(why) = got else {
        panic!("{got:?}")
    };
    assert!(why.contains("ended"), "{why}");
}
