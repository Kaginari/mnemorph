//! The human gate: first answer wins, silence denies.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use mnemorph_tools::gate::AnsweredElsewhere;
use mnemorph_tools::{Answer, Pending};

#[test]
fn silence_denies_at_the_deadline() {
    let p = Pending::new();
    let started = Instant::now();
    let a = p.wait(Duration::from_millis(100));
    assert!(
        matches!(a, Answer::Deny(ref r) if r.contains("no answer")),
        "{a:?}"
    );
    assert!(started.elapsed() >= Duration::from_millis(100));
    assert_eq!(
        p.answer(Answer::Approve),
        Err(AnsweredElsewhere),
        "a late answer is told it came too late"
    );
}

#[test]
fn an_answer_before_the_deadline_is_the_answer() {
    let p = Pending::new();
    let q = Arc::clone(&p);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        q.answer(Answer::Approve).unwrap();
    });
    assert_eq!(p.wait(Duration::from_secs(5)), Answer::Approve);
}

/// Lesson: answering the same approval from the board and the terminal could race or hang.
#[test]
fn concurrent_answers_one_wins_the_rest_are_answered_elsewhere() {
    for _ in 0..50 {
        let p = Pending::new();
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let q = Arc::clone(&p);
                thread::spawn(move || {
                    q.answer(if i % 2 == 0 {
                        Answer::Approve
                    } else {
                        Answer::Deny(format!("channel {i}"))
                    })
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|r| **r == Err(AnsweredElsewhere))
                .count(),
            7
        );
        let first = p.wait(Duration::from_millis(1));
        assert!(
            !matches!(first, Answer::Deny(ref r) if r.contains("no answer")),
            "the winner stands: {first:?}"
        );
    }
}
