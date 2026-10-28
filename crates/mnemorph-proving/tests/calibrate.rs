//! The reviewers' calibration, with no model: the gold set loads whole, its labels hold to the
//! class table and the playbooks, a reviewer's prompt carries the case and never its answer, a
//! verdict line reads as it says, and the scorer computes known numbers on scripted reviewers.
//! The live run over real models is `mnemorph-loop`'s `calibrate` (`just calibrate`).

#[path = "support/seeded.rs"]
mod seeded;

#[path = "support/calibrate.rs"]
mod calibrate;

use std::collections::BTreeSet;

use mnemorph_proving::Severity;
use mnemorph_races::Speciality;

use calibrate::{
    Answer, Case, Judgement, Label, Reviewer, Source, Tally, gold, judgement, report, score, wilson,
};

/// The set: every seeded plant and control, the session's ten real findings and the labelled
/// false positives, each id once.
#[test]
fn the_gold_set_loads() {
    let cases = gold();
    let count = |s: Source| cases.iter().filter(|c| c.source == s).count();
    let key = seeded::manifest();
    assert_eq!(count(Source::Plant), key.plant.len());
    assert_eq!(count(Source::Control), key.control.len());
    assert_eq!(count(Source::Session), 10);
    assert!(count(Source::FalsePositive) >= 5);
    let mut ids = BTreeSet::new();
    for c in &cases {
        assert!(ids.insert(c.id.as_str()), "{} twice", c.id);
        assert!(!c.code.trim().is_empty(), "{}: no code", c.id);
        assert!(!c.claim.trim().is_empty(), "{}: no claim", c.id);
    }
    // A sound case for every hunter that has a planted look-alike, and sound cases for the law.
    let law = cases
        .iter()
        .filter(|c| c.speciality() == Speciality::Law && c.label == Label::NotBug)
        .count();
    assert!(law >= 4, "{law}");
}

/// Each label holds to the class table: the class is its speciality's own; a bug carries its
/// severity and a sound case none but its reason; a real finding names its fix and the words its
/// class's Regression line keeps for it — all ten the playbooks name.
#[test]
fn the_labels_hold_to_the_class_table() {
    let cases = gold();
    for c in &cases {
        let s = c.speciality();
        assert!(Speciality::HUNTERS.contains(&s), "{}: a hunter", c.id);
        assert_eq!(c.class().owners(), [s], "{}: {} owns {}", c.id, s, c.class);
        match c.label {
            Label::Bug => {
                assert!(c.severity.is_some(), "{}: a bug's severity", c.id);
                assert!(c.reason.is_none(), "{}", c.id);
            }
            Label::NotBug => {
                assert!(c.severity.is_none(), "{}: a sound case has none", c.id);
                assert!(c.reason.is_some(), "{}: why it is sound", c.id);
            }
        }
        match c.source {
            Source::Plant | Source::Session => assert_eq!(c.label, Label::Bug, "{}", c.id),
            Source::Control | Source::FalsePositive => {
                assert_eq!(c.label, Label::NotBug, "{}", c.id);
            }
        }
        if c.source == Source::Session {
            let fixed = c.fixed_in.as_deref().unwrap_or_default();
            assert!(
                fixed.len() >= 7 && fixed.bytes().all(|b| b.is_ascii_hexdigit()),
                "{}: fixed in a commit",
                c.id
            );
            let words = c.regression.as_deref().expect("a regression");
            let line = c
                .class()
                .playbook()
                .split("\nRegression: ")
                .nth(1)
                .unwrap_or_else(|| panic!("{}: no regression line", c.class));
            assert!(line.contains(words), "{}: {words}", c.id);
        } else {
            assert!(c.fixed_in.is_none() && c.regression.is_none(), "{}", c.id);
        }
        // The fakes stay fakes.
        for word in ["ghp_", "github_pat_", "sk-or-", "AKIA", "BEGIN PRIVATE"] {
            assert!(!c.code.contains(word), "{}: {word}", c.id);
        }
    }
    let regressions: BTreeSet<&str> = cases
        .iter()
        .filter_map(|c| c.regression.as_deref())
        .collect();
    for named in [
        ".cargo/credentials",
        "symlink swaps",
        "GIT_SSH_COMMAND",
        "invisible characters",
        "cards and status",
        "pre-existing labels",
        "fingerprint not recomputed",
        "hollow coverage",
        "PR body file",
        "typeahead",
    ] {
        assert!(regressions.contains(named), "{named}");
    }
}

/// A Dark Elf is briefed with its own playbooks, the Fable review with none; both get the claim
/// and the code, and neither the label, the reason nor the fix.
#[test]
fn a_prompt_carries_the_case_and_never_its_answer() {
    let cases = gold();
    let fp = cases.iter().find(|c| c.id == "fp-deep-chain").unwrap();
    let (system, ask) = Reviewer::DarkElf(None).prompt(fp);
    assert!(system.contains("You are dark-elf-law"), "{system}");
    assert!(system.contains("### unbounded-delegation"), "{system}");
    assert!(ask.contains(&fp.claim) && ask.contains("fn may_dispatch"));
    assert!(ask.contains("VERDICT: bug SEVERITY:"));
    for c in &cases {
        for reviewer in [Reviewer::DarkElf(None), Reviewer::Fable] {
            let (system, ask) = reviewer.prompt(c);
            for hidden in [c.reason.as_deref(), c.fixed_in.as_deref()]
                .into_iter()
                .flatten()
            {
                assert!(
                    !system.contains(hidden) && !ask.contains(hidden),
                    "{}: {hidden}",
                    c.id
                );
            }
        }
    }
    let (system, _) = Reviewer::Fable.prompt(fp);
    assert!(!system.contains("## Playbooks"), "{system}");
    let law = Reviewer::of("dark-elf-law").unwrap();
    assert_eq!(law.name(), "dark-elf-law");
    assert!(law.judges(fp));
    let harness = cases
        .iter()
        .find(|c| c.speciality() == Speciality::Harness)
        .unwrap();
    assert!(!law.judges(harness));
    assert_eq!(Reviewer::of("fable"), Some(Reviewer::Fable));
    assert_eq!(Reviewer::of("dark-elf"), Some(Reviewer::DarkElf(None)));
    assert_eq!(Reviewer::of("dark-elf-nope"), None);
    assert_eq!(Reviewer::of("rimuru"), None);
}

#[test]
fn a_verdict_line_reads_as_it_says() {
    assert_eq!(
        judgement("It splices the root.\nVERDICT: bug SEVERITY: critical"),
        Judgement::Bug(Some(Severity::Critical))
    );
    assert_eq!(
        judgement("`verdict: Bug severity: High.`\n"),
        Judgement::Bug(Some(Severity::High))
    );
    assert_eq!(judgement("VERDICT: bug"), Judgement::Bug(None));
    assert_eq!(
        judgement("VERDICT: bug SEVERITY: dire"),
        Judgement::Bug(None)
    );
    assert_eq!(judgement("VERDICT: not-bug"), Judgement::NotBug);
    assert_eq!(judgement("VERDICT: not bug"), Judgement::NotBug);
    assert_eq!(judgement("VERDICT: abstain"), Judgement::Abstain);
    assert_eq!(judgement("I think it is fine."), Judgement::Abstain);
    // The last verdict stands: one quoted from the ask earlier does not.
    assert_eq!(
        judgement("The ask says `VERDICT: bug SEVERITY: low`.\nVERDICT: not-bug"),
        Judgement::NotBug
    );
}

#[test]
fn wilson_intervals_are_the_known_ones() {
    let near = |(lo, hi): (f64, f64), (a, b): (f64, f64)| {
        assert!((lo - a).abs() < 1e-6 && (hi - b).abs() < 1e-6, "{lo} {hi}");
    };
    near(wilson(3, 4).unwrap(), (0.300_636_052, 0.954_413_937));
    near(wilson(1, 4).unwrap(), (0.045_586_063, 0.699_363_948));
    near(wilson(0, 5).unwrap(), (0.0, 0.434_491_495));
    near(wilson(5, 5).unwrap(), (0.565_508_505, 1.0));
    assert_eq!(wilson(0, 0), None);
}

fn case(id: &str, speciality: &str, class: &str, severity: Option<Severity>) -> Case {
    Case {
        id: id.into(),
        source: if severity.is_some() {
            Source::Session
        } else {
            Source::FalsePositive
        },
        label: if severity.is_some() {
            Label::Bug
        } else {
            Label::NotBug
        },
        speciality: speciality.into(),
        class: class.into(),
        severity,
        file: "src/x.rs".into(),
        symbol: "x".into(),
        claim: "x".into(),
        code: "fn x() {}".into(),
        fixed_in: None,
        regression: None,
        reason: None,
    }
}

fn answer(case: &str, model: &str, judgement: Judgement) -> Answer {
    Answer {
        case: case.into(),
        model: model.into(),
        judgement,
    }
}

/// A scripted panel of three models over six cases: every count, rate and split is the one
/// worked out by hand.
#[test]
fn the_scorer_computes_known_numbers() {
    use Judgement::{Abstain, Bug, Failed, NotBug};
    use Severity::{Critical, High, Low, Medium};
    let law = "dark-elf-law";
    let harness = "dark-elf-harness";
    let cases = [
        case("b1", law, "ledger-integrity", Some(High)),
        case("b2", law, "phantom-effect", Some(Medium)),
        case("n1", law, "detached-work", None),
        case("b3", harness, "toctou", Some(Critical)),
        case("n2", harness, "toctou", None),
        case("n3", harness, "terminal-escape", None),
    ];
    let (a1, a2, b) = ("alpha/one:free", "alpha/two:free", "beta/three");
    let answers = [
        answer("b1", a1, Bug(Some(High))),
        answer("b2", a1, Bug(Some(Low))),
        answer("n1", a1, NotBug),
        answer("b3", a1, NotBug),
        answer("n2", a1, Bug(None)),
        answer("n3", a1, Abstain),
        answer("b1", a2, Bug(Some(High))),
        answer("b2", a2, NotBug),
        answer("n1", a2, Failed),
        answer("b1", b, NotBug),
        answer("nowhere", b, Bug(None)),
    ];
    let s = score(&cases, &answers);
    let l = s.speciality[law];
    assert_eq!(
        l,
        Tally {
            bugs: 5,
            caught: 3,
            passed: 2,
            severity_agreed: 2,
            sound: 2,
            flagged: 0,
            cleared: 1,
            abstained: 0,
            failed: 1,
        }
    );
    assert_eq!(l.recall(), Some(0.6));
    assert_eq!(l.precision(), Some(1.0));
    assert_eq!(l.coverage(), Some(6.0 / 7.0));
    assert_eq!(l.severity_agreement(), Some(2.0 / 3.0));
    assert_eq!((l.false_accept(), l.false_reject()), ((2, 5), (0, 1)));
    let h = s.speciality[harness];
    assert_eq!((h.caught, h.passed, h.flagged, h.cleared), (0, 1, 1, 0));
    assert_eq!((h.recall(), h.precision()), (Some(0.0), Some(0.0)));
    assert_eq!((h.abstained, h.coverage()), (1, Some(2.0 / 3.0)));
    assert_eq!(h.severity_agreement(), None);
    assert_eq!(s.all.asked(), 10, "the answer to no case is ignored");
    assert_eq!(
        (s.all.caught, s.all.passed, s.all.flagged, s.all.cleared),
        (3, 3, 1, 1)
    );
    assert_eq!(s.family["alpha"].asked(), 9);
    assert_eq!((s.family["beta"].bugs, s.family["beta"].passed), (1, 1));
    assert_eq!(
        s.panel,
        (2, 2),
        "b1 and b2 are each judged twice or more, and split"
    );
    assert_eq!(s.within.len(), 1);
    assert_eq!(
        s.within["alpha"],
        (2, 1),
        "alpha agrees on b1, splits on b2"
    );

    let text = report(
        Reviewer::DarkElf(None),
        &[a1.into(), a2.into(), b.into()],
        &cases,
        &answers,
    );
    assert!(text.starts_with("# Calibration: dark-elf\n"), "{text}");
    assert!(
        text.contains("| dark-elf-law | 7 | 86% | 0 | 1 | 60% | 100% | 2/5 ["),
        "{text}"
    );
    assert!(text.contains("| 0/1 [0–80%] |"), "{text}");
    assert!(text.contains("2 of 2 cases"), "{text}");
    assert!(text.contains("Within alpha: 1 of 2 split."), "{text}");
    assert!(
        text.contains("- false accept: `b3` by alpha/one:free"),
        "{text}"
    );
    assert!(
        text.contains("- false reject: `n2` by alpha/one:free"),
        "{text}"
    );
}

/// A reviewer that knows the truth scores perfectly on the whole set; one that flags everything
/// catches every bug and rejects every sound case.
#[test]
fn the_extremes_score_as_they_must() {
    let cases = gold();
    let truth: Vec<Answer> = cases
        .iter()
        .map(|c| {
            let j = match c.label {
                Label::Bug => Judgement::Bug(c.severity),
                Label::NotBug => Judgement::NotBug,
            };
            answer(&c.id, "oracle/truth", j)
        })
        .collect();
    let s = score(&cases, &truth);
    let bugs = cases.iter().filter(|c| c.label == Label::Bug).count();
    let sound = cases.len() - bugs;
    assert_eq!(
        (s.all.recall(), s.all.precision(), s.all.coverage()),
        (Some(1.0), Some(1.0), Some(1.0))
    );
    assert_eq!(s.all.severity_agreement(), Some(1.0));
    assert_eq!(
        (s.all.false_accept(), s.all.false_reject()),
        ((0, bugs), (0, sound))
    );
    assert_eq!(s.speciality.len(), Speciality::HUNTERS.len());
    let all_bug: Vec<Answer> = cases
        .iter()
        .map(|c| answer(&c.id, "eager/flag", Judgement::Bug(None)))
        .collect();
    let s = score(&cases, &all_bug);
    assert_eq!(s.all.recall(), Some(1.0));
    assert_eq!(s.all.false_reject(), (sound, sound));
    assert_eq!(s.all.severity_agreement(), Some(0.0));
    let text = report(Reviewer::Fable, &["eager/flag".into()], &cases, &[]);
    assert!(text.contains("| **all** | 0 | — |"), "{text}");
    assert!(text.contains("None."), "{text}");
}
