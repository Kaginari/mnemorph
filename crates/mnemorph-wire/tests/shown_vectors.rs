//! The escaping rule (`hidden`, `shown`) against the vectors the board's TypeScript port reads too
//! (`packages/board/src/safe/shown.test.ts`): one file, the rule's ranges included, so the two can
//! never drift apart.

use mnemorph_wire::{hidden, invisible, shown};
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

fn hex(s: &Value) -> u32 {
    u32::from_str_radix(s.as_str().expect("hex"), 16).expect("hex")
}

/// The ranges the vectors list under `key`, as `(from, to)` code points.
fn ranges_of(v: &Value, key: &str) -> Vec<(u32, u32)> {
    v[key]
        .as_array()
        .expect("a range list")
        .iter()
        .map(|r| match r {
            Value::Array(pair) => (hex(&pair[0]), hex(&pair[1])),
            _ => (hex(&r["from"]), hex(&r["to"])),
        })
        .collect()
}

fn ranges(v: &Value) -> Vec<(u32, u32)> {
    ranges_of(v, "escaped")
}

fn codes(table: &[(char, char)]) -> Vec<(u32, u32)> {
    table
        .iter()
        .map(|&(lo, hi)| (u32::from(lo), u32::from(hi)))
        .collect()
}

#[test]
fn the_spaces_and_marks_are_the_shared_ranges() {
    let v = vectors();
    assert_eq!(codes(mnemorph_wire::SPACES), ranges_of(&v, "spaces"));
    assert_eq!(codes(mnemorph_wire::MARKS), ranges_of(&v, "marks"));
    assert_eq!(
        Some(mnemorph_wire::MARKS_KEPT as u64),
        v["marks_kept"].as_u64()
    );
    let marks = ranges_of(&v, "marks");
    for c in (0..=0x0010_FFFF).filter_map(char::from_u32) {
        let cp = u32::from(c);
        let want = marks.iter().any(|&(lo, hi)| (lo..=hi).contains(&cp));
        assert_eq!(mnemorph_wire::mark(c), want, "U+{cp:04X}");
    }
}

#[test]
fn invisible_and_shown_agree_on_every_code_point() {
    // The one list the wire's prose trimming, the proving records and the card read: whatever
    // `invisible` names, `shown` escapes, and every control too.
    for c in (0..=0x0010_FFFF).filter_map(char::from_u32) {
        let escaped = shown(&format!("a{c}")) != format!("a{c}");
        let spaced = mnemorph_wire::SPACES
            .iter()
            .any(|&(lo, hi)| (lo..=hi).contains(&c));
        assert_eq!(
            escaped,
            c.is_control() || invisible(c) || spaced || c == '\\',
            "U+{:04X}",
            u32::from(c)
        );
    }
}

#[test]
fn hidden_is_exactly_the_controls_and_the_shared_ranges() {
    let v = vectors();
    let listed = ranges(&v);
    assert_eq!(
        listed,
        codes(mnemorph_wire::HIDDEN),
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
            mnemorph_wire::kept_in_prose(c),
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
    fn shown_never_holds_a_hidden_character(text in "\\PC*|[\\u{0}-\\u{3000}\\u{e0000}-\\u{e0fff}\\u{fe00}-\\u{feff}]*") {
        let out = shown(&text);
        proptest::prop_assert!(!out.chars().any(hidden), "{:?} → {:?}", text, out);
        let spaced = |c: char| mnemorph_wire::SPACES.iter().any(|&(lo, hi)| (lo..=hi).contains(&c));
        proptest::prop_assert!(!out.chars().any(spaced), "{:?} → {:?}", text, out);
        let mut run = 0;
        for c in out.chars() {
            run = if mnemorph_wire::mark(c) { run + 1 } else { 0 };
            proptest::prop_assert!(run <= mnemorph_wire::MARKS_KEPT, "{:?} → {:?}", text, out);
        }
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
