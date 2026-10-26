//! Recall: meaning first, a seat for the dissent, what was cut said, silence when nothing is asked.

use mnemorph_memory::{Note, Polarity, Recalled, Relation, Status, Weights, recall};

const DAY: u64 = 86_400_000;
const NOW: u64 = 400 * DAY;

fn note(id: &str, text: &str, polarity: Polarity, status: Status) -> Note {
    Note {
        id: id.into(),
        text: text.into(),
        tags: vec![],
        owner: None,
        polarity,
        salience: 0.5,
        confidence: 0.5,
        status,
        source: None,
        proof: None,
        born: NOW - DAY,
    }
}

fn ids(r: &Recalled) -> Vec<&str> {
    r.anchors.iter().map(|h| h.note.id.as_str()).collect()
}

#[test]
fn a_query_of_no_content_words_recalls_nothing() {
    let notes = vec![(
        note(
            "n",
            "the parser drops the alias",
            Polarity::Support,
            Status::Grounded,
        ),
        Relation::Own,
    )];
    assert_eq!(
        recall("what is the of it", &notes, NOW, 6, &Weights::default()),
        Recalled::default()
    );
}

/// The only correcting note ranks below `k` and is still recalled: the dissent keeps a seat.
#[test]
fn the_only_correction_ranks_below_k_and_is_still_recalled() {
    let mut notes: Vec<(Note, Relation)> = (0..6)
        .map(|i| {
            let mut n = note(
                &format!("s{i}"),
                "the tokenizer splits interval strings on whitespace",
                Polarity::Support,
                Status::Grounded,
            );
            n.salience = 1.0;
            n.confidence = 1.0;
            (n, Relation::Own)
        })
        .collect();
    let mut correction = note(
        "fix",
        "interval strings with fractions are not split by the tokenizer",
        Polarity::Revise,
        Status::Grounded,
    );
    correction.salience = 0.0;
    correction.confidence = 0.1;
    notes.push((correction, Relation::Shared));
    // A memory holds more than the question: notes about other things.
    for i in 0..20 {
        notes.push((
            note(
                &format!("other{i}"),
                &format!("the optimizer pushes predicate {i} down through joins"),
                Polarity::Support,
                Status::Grounded,
            ),
            Relation::Shared,
        ));
    }
    let r = recall(
        "tokenizer interval strings",
        &notes,
        NOW,
        3,
        &Weights::default(),
    );
    assert_eq!(r.anchors.len(), 3);
    assert!(
        ids(&r).contains(&"fix"),
        "the correction holds the dissent's seat: {:?}",
        ids(&r)
    );
    assert_eq!(r.cut, 4, "the cut are counted");
    assert_eq!(r.cut_ids.len(), 3, "at most three of them named");
}

/// The best hit's meaning is a poor match: however high its priors, it stays under the floor.
#[test]
fn a_poor_match_stays_under_the_floor_however_high_its_priors() {
    // "parser" is in every note: it tells them apart by nothing, so it means little.
    let mut notes: Vec<(Note, Relation)> = (0..8)
        .map(|i| {
            (
                note(
                    &format!("n{i}"),
                    &format!("parser note number {i} about {i}"),
                    Polarity::Support,
                    Status::Grounded,
                ),
                Relation::Shared,
            )
        })
        .collect();
    let mut loud = note("loud", "parser", Polarity::Support, Status::Grounded);
    loud.salience = 1.0;
    loud.confidence = 1.0;
    loud.born = NOW;
    notes.push((loud, Relation::Own));
    let r = recall("parser", &notes, NOW, 6, &Weights::default());
    assert!(!ids(&r).contains(&"loud"), "{:?}", ids(&r));
    assert!(
        r.anchors
            .iter()
            .all(|h| h.meaning >= Weights::default().floor)
    );
}

#[test]
fn dormant_and_tombstoned_notes_are_never_recalled_and_the_own_ranks_first() {
    let text = "the generator quotes reserved identifiers";
    let notes = vec![
        (
            note("shared", text, Polarity::Support, Status::Grounded),
            Relation::Shared,
        ),
        (
            note("own", text, Polarity::Support, Status::Grounded),
            Relation::Own,
        ),
        (
            note("asleep", text, Polarity::Support, Status::Dormant),
            Relation::Own,
        ),
        (
            note("gone", text, Polarity::Support, Status::Tombstoned),
            Relation::Own,
        ),
        (
            note(
                "other",
                "nothing about that at all",
                Polarity::Support,
                Status::Grounded,
            ),
            Relation::Own,
        ),
    ];
    let r = recall(
        "generator reserved identifiers",
        &notes,
        NOW,
        6,
        &Weights::default(),
    );
    assert_eq!(ids(&r), ["own", "shared"]);
    assert_eq!(r.cut, 0);
}

/// The seats are the page's: the k − 2 best agreeing notes, the best dissent, then the fill — so
/// agreeing notes keep their seats when dissents outnumber them.
#[test]
fn the_seats_keep_k_minus_two_for_agreement_and_one_for_dissent() {
    let text = "the planner inlines common table expressions";
    let mut notes: Vec<(Note, Relation)> = (0..3)
        .map(|i| {
            let mut n = note(&format!("c{i}"), text, Polarity::Counter, Status::Grounded);
            n.salience = 1.0;
            (n, Relation::Own)
        })
        .collect();
    notes.push((
        note("s", text, Polarity::Support, Status::Grounded),
        Relation::Shared,
    ));
    for i in 0..20 {
        notes.push((
            note(
                &format!("x{i}"),
                &format!("unrelated note {i}"),
                Polarity::Support,
                Status::Grounded,
            ),
            Relation::Shared,
        ));
    }
    let r = recall(
        "planner inlines table expressions",
        &notes,
        NOW,
        3,
        &Weights::default(),
    );
    let got = ids(&r);
    assert!(
        got.contains(&"s"),
        "the agreeing note keeps its seat: {got:?}"
    );
    assert_eq!(got.len(), 3);
}

/// Another creature's proposed note is its guess: never weighed. The asker's own is.
#[test]
fn only_the_askers_own_proposed_notes_are_weighed() {
    let text = "the dialect quotes identifiers with backticks";
    let notes = vec![
        (
            note("mine", text, Polarity::Support, Status::Proposed),
            Relation::Own,
        ),
        (
            note("theirs", text, Polarity::Support, Status::Proposed),
            Relation::Shared,
        ),
        (
            note("kin", text, Polarity::Support, Status::Proposed),
            Relation::Race,
        ),
        (
            note(
                "other",
                "nothing like that",
                Polarity::Support,
                Status::Grounded,
            ),
            Relation::Shared,
        ),
    ];
    let r = recall(
        "dialect quotes identifiers backticks",
        &notes,
        NOW,
        6,
        &Weights::default(),
    );
    assert_eq!(ids(&r), ["mine"]);
}
