//! Fits recall's saturation against real memory: the notes the bench's runs wrote
//! (`tests/fixtures/bench-recall.json`). Each task's ask is the query; the notes its own runs
//! wrote about the task are what it should find, the other tasks' notes what it should not.
//! Prints precision, recall and F1 of clearing the floor, by query length, corpus size and `c`.
//!
//! `cargo run -p mnemorph-memory --example fit_saturation`

#![allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    reason = "a one-off measurement, read top to bottom"
)]

use std::collections::BTreeMap;

use mnemorph_memory::Weights;
use mnemorph_memory::text::{bm25, content_words};
use serde_json::Value;

const GRID: [f64; 16] = [
    0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0, 32.0, 48.0,
];
const SIZES: [usize; 7] = [1, 2, 5, 10, 25, 50, 101];

/// A small fixed generator: the same corpora every run.
fn next(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    *s >> 33
}

fn main() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/bench-recall.json"
    );
    let data: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let notes: Vec<(String, Vec<String>, bool)> = data["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            (
                n["task"].as_str().unwrap().to_owned(),
                content_words(n["text"].as_str().unwrap()),
                n["about_task"].as_bool().unwrap(),
            )
        })
        .collect();
    let floor = Weights::default().floor;
    let w = Weights::default().meaning;
    let mut seed = 7;
    // (bucket, size, c) -> (true positives, false positives, false negatives)
    let mut tally: BTreeMap<(usize, usize, usize), [u32; 3]> = BTreeMap::new();
    for (task, ask) in data["asks"].as_object().unwrap() {
        let words = content_words(ask.as_str().unwrap());
        let own: Vec<usize> = (0..notes.len())
            .filter(|i| notes[*i].0 == *task && notes[*i].2)
            .collect();
        let others: Vec<usize> = (0..notes.len()).filter(|i| notes[*i].0 != *task).collect();
        if own.is_empty() {
            continue;
        }
        // The ask whole, three of its words that its notes hold, and the one they hold least.
        let held = |w: &&String| own.iter().any(|i| notes[*i].1.contains(w));
        let mut hits: Vec<String> = words.iter().filter(held).cloned().collect();
        hits.dedup();
        let queries = [
            (0, hits.iter().take(1).cloned().collect::<Vec<_>>()),
            (1, hits.iter().take(3).cloned().collect()),
            (2, words.clone()),
        ];
        for size in SIZES {
            for _ in 0..8 {
                let mut corpus: Vec<usize> = vec![own[next(&mut seed) as usize % own.len()]];
                let mut pool = others.clone();
                while corpus.len() < size && !pool.is_empty() {
                    let at = next(&mut seed) as usize % pool.len();
                    corpus.push(pool.swap_remove(at));
                }
                let docs: Vec<Vec<String>> = corpus.iter().map(|i| notes[*i].1.clone()).collect();
                for (bucket, q) in &queries {
                    if q.is_empty() {
                        continue;
                    }
                    for (ci, c) in GRID.iter().enumerate() {
                        let t = tally.entry((*bucket, size, ci)).or_default();
                        for (at, doc) in corpus.iter().zip(&docs) {
                            let raw = bm25(q, doc, &docs);
                            let cleared = w * raw / (raw + c) >= floor;
                            let relevant = notes[*at].0 == *task;
                            match (cleared, relevant) {
                                (true, true) => t[0] += 1,
                                (true, false) => t[1] += 1,
                                (false, true) => t[2] += 1,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    let f1 = |bucket: usize, size: usize, ci: usize| {
        let [tp, fp, fn_] = tally.get(&(bucket, size, ci)).copied().unwrap_or_default();
        let (tp, fp, fn_) = (f64::from(tp), f64::from(fp), f64::from(fn_));
        let p = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
        let r = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
        (
            if p + r > 0.0 {
                2.0 * p * r / (p + r)
            } else {
                0.0
            },
            p,
            r,
        )
    };
    for (bucket, name) in ["1 word", "2-3 words", "4+ words"].iter().enumerate() {
        println!("\n{name}: the best c by corpus size");
        for size in SIZES {
            let best = (0..GRID.len())
                .max_by(|a, b| f1(bucket, size, *a).0.total_cmp(&f1(bucket, size, *b).0))
                .unwrap();
            let (f, p, r) = f1(bucket, size, best);
            let now = [0.5, 1.0, 1.5][bucket];
            let at = GRID.iter().position(|c| (*c - now).abs() < 1e-9).unwrap();
            let (fn0, ..) = f1(bucket, size, at);
            let fitted = Weights::default().saturation_for([1, 2, 4][bucket], size);
            println!(
                "  N={size:<4} best c={:<5} F1 {f:.2} (p {p:.2} r {r:.2})   fixed c={now}: F1 {fn0:.2}   formula c={fitted:.1}",
                GRID[best]
            );
        }
    }
}
