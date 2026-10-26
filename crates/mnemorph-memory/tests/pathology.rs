//! The replay loop and lock-in, read by code: their windows, their acts and their findings.

use mnemorph_memory::pathology::{Trace, findings, fingerprint, replays};

fn t(fp: &str, outcome: Option<&str>, dissent: bool) -> Trace {
    Trace {
        time: 0,
        fingerprint: fp.into(),
        dissent,
        outcome: outcome.map(str::to_owned),
        forced: None,
    }
}

#[test]
fn a_fingerprint_is_sha256_over_query_anchors_and_office() {
    let a = fingerprint("q", &["n1".into(), "n2".into()], "findings");
    assert_eq!(a.len(), 64);
    assert_eq!(a, fingerprint("q", &["n1".into(), "n2".into()], "findings"));
    assert_ne!(a, fingerprint("q", &["n1".into(), "n2".into()], "verdict"));
    assert_ne!(
        a,
        fingerprint("q", &["n1n2".into()], "findings"),
        "the ids stay apart"
    );
}

#[test]
fn a_replay_needs_three_in_five_with_nothing_landed_and_no_dissent() {
    let past = [t("x", None, false), t("x", None, false)];
    assert!(replays(&past, "x", false));
    assert!(!replays(&past, "x", true), "a dissent shown now breaks it");
    assert!(!replays(&past[..1], "x", false), "twice is no loop");
    let landed = [t("x", Some("log.md gate"), false), t("x", None, false)];
    assert!(!replays(&landed, "x", false), "something landed");
    let shown = [t("x", None, true), t("x", None, false)];
    assert!(!replays(&shown, "x", false), "a correction was shown");
    // Out of the window: the last four, with the current one, make five.
    let old = [
        t("x", None, false),
        t("x", None, false),
        t("a", None, false),
        t("b", None, false),
        t("c", None, false),
        t("d", None, false),
    ];
    assert!(!replays(&old, "x", false));
}

#[test]
fn status_reads_a_replay_and_the_acts_forced() {
    let mut past = vec![
        t("x", None, false),
        t("y", None, false),
        t("x", None, false),
    ];
    past.push(Trace {
        forced: Some("replay".into()),
        ..t("x", None, false)
    });
    let lines = findings(&past);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].starts_with("replay loop: the same recall 3 times in the last 4"));
    assert_eq!(lines[1], "replay acted on 1 of the last 4 recalls");
    assert_eq!(
        findings(&[t("x", Some("landed"), false)]),
        Vec::<String>::new()
    );
}
