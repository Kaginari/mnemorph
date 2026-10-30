//! `hidden` and `shown` against the vectors the board's TypeScript port reads too
//! (`packages/board/src/safe/shown.test.ts`): one file, the rule's ranges included, so the two can
//! never drift apart.

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

/// The ranges the vectors list, as `(from, to)` code points.
fn ranges(v: &Value) -> Vec<(u32, u32)> {
    let hex = |s: &Value| u32::from_str_radix(s.as_str().expect("hex"), 16).expect("hex");
    v["escaped"]
        .as_array()
        .expect("an escaped list")
        .iter()
        .map(|r| (hex(&r["from"]), hex(&r["to"])))
        .collect()
}

#[test]
fn hidden_is_exactly_the_controls_and_the_shared_ranges() {
    let v = vectors();
    let listed = ranges(&v);
    assert_eq!(
        listed,
        mnemorph_loop::HIDDEN
            .iter()
            .map(|&(lo, hi)| (u32::from(lo), u32::from(hi)))
            .collect::<Vec<_>>(),
        "the table and the vectors list the same ranges"
    );
    for c in (0..=0x0010_FFFF).filter_map(char::from_u32) {
        let cp = u32::from(c);
        let want = c.is_control() || listed.iter().any(|&(lo, hi)| (lo..=hi).contains(&cp));
        assert_eq!(hidden(c), want, "U+{cp:04X}");
    }
}

#[test]
fn prose_keeps_only_what_the_vectors_name() {
    let v = vectors();
    let kept: Vec<u32> = v["prose"]
        .as_array()
        .expect("a prose list")
        .iter()
        .map(|s| u32::from_str_radix(s.as_str().expect("hex"), 16).expect("hex"))
        .collect();
    for c in (0..=0x0010_FFFF).filter_map(char::from_u32) {
        let cp = u32::from(c);
        assert_eq!(
            mnemorph_loop::kept_in_prose(c),
            kept.contains(&cp),
            "U+{cp:04X}"
        );
        if kept.contains(&cp) {
            assert!(
                hidden(c),
                "U+{cp:04X}: prose keeps only what is hidden elsewhere"
            );
        }
    }
}

proptest::proptest! {
    /// Whatever the text, what a human is shown holds no hidden character: each is escaped.
    #[test]
    fn shown_never_holds_a_hidden_character(text in "\\PC*|[\\u{0}-\\u{2fff}\\u{e0000}-\\u{e0fff}\\u{fe00}-\\u{feff}]*") {
        let out = shown(&text);
        proptest::prop_assert!(!out.chars().any(hidden), "{:?} → {:?}", text, out);
    }
}

#[test]
fn shown_never_holds_a_hidden_character_from_any_range() {
    // Every hidden code point, each between letters: none survives.
    for c in (0..=0x0010_FFFF)
        .filter_map(char::from_u32)
        .filter(|&c| hidden(c))
    {
        let out = shown(&format!("a{c}b"));
        assert!(!out.chars().any(hidden), "U+{:04X}", u32::from(c));
        assert_eq!(out, format!("a{}b", c.escape_unicode()));
    }
}
