//! `hidden` and `shown` against the vectors the board's TypeScript port reads too
//! (`packages/board/src/safe/shown.test.ts`): one file, so the two can never drift apart.

use mnemorph_loop::run::{hidden, shown};
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("shown-vectors.json")).expect("the vectors parse")
}

#[test]
fn hidden_matches_the_shared_vectors() {
    let v = vectors();
    let rows = v["hidden"].as_array().expect("a hidden list");
    assert!(rows.len() > 20);
    for row in rows {
        let cp = u32::from_str_radix(row["cp"].as_str().expect("cp"), 16).expect("hex");
        let c = char::from_u32(cp).expect("a char");
        let want = row["hidden"].as_bool().expect("hidden");
        assert_eq!(hidden(c), want, "U+{cp:04X}");
    }
}

#[test]
fn shown_matches_the_shared_vectors() {
    let v = vectors();
    let rows = v["shown"].as_array().expect("a shown list");
    assert!(rows.len() > 20);
    for row in rows {
        let input = row["in"].as_str().expect("in");
        let want = row["out"].as_str().expect("out");
        assert_eq!(shown(input), want, "{input:?}");
    }
}
