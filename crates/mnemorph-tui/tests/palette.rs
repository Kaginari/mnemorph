//! The Rust palette is the TypeScript palette: `packages/theme/src/index.ts` is read and every
//! colour of `dark`, `light` and `races` compared, both ways — none missing, none extra.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use mnemorph_tui::theme::{DARK, LIGHT, RACES, Rgb};

/// The `key: '#rrggbb'` pairs of the object literal that follows `head` in `src`.
fn object(src: &str, head: &str) -> BTreeMap<String, String> {
    let start = src.find(head).unwrap_or_else(|| panic!("{head} not found"));
    let body = &src[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find('}').unwrap()];
    body.lines()
        .filter_map(|l| {
            let (k, v) = l.trim().trim_end_matches(',').split_once(':')?;
            let v = v.trim().trim_matches('\'');
            v.starts_with('#')
                .then(|| (k.trim().to_owned(), v.to_lowercase()))
        })
        .collect()
}

fn ours(named: Vec<(&'static str, Rgb)>) -> BTreeMap<String, String> {
    named
        .into_iter()
        .map(|(k, c)| (k.to_owned(), c.css()))
        .collect()
}

#[test]
fn the_rust_palette_equals_the_typescript_one() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/theme/src/index.ts"
    );
    let src = std::fs::read_to_string(path).expect("the TypeScript theme");
    assert_eq!(
        ours(DARK.named()),
        object(&src, "export const dark"),
        "dark"
    );
    assert_eq!(
        ours(LIGHT.named()),
        object(&src, "export const light"),
        "light"
    );
    assert_eq!(
        ours(RACES.named()),
        object(&src, "export const races"),
        "races"
    );
}

/// Every scheme's palettes, dark and light, with the human's colour: the TypeScript holds them as
/// `export const <scheme>Dark` and `<scheme>Light`, and the board's CSS reads the same values. On
/// a difference the expected TypeScript is printed, to paste.
#[test]
fn every_scheme_equals_the_typescript_one() {
    use mnemorph_tui::theme::{Scheme, Theme, Variant};
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/theme/src/index.ts"
    );
    let src = std::fs::read_to_string(path).expect("the TypeScript theme");
    let mut want = String::new();
    let mut differs = Vec::new();
    for s in Scheme::ALL.into_iter().filter(|s| *s != Scheme::Hud) {
        for (v, name) in [(Variant::Dark, "Dark"), (Variant::Light, "Light")] {
            let t = Theme::of(s, v);
            let mut named = t.p.named();
            named.push(("player", t.races.player));
            let head = format!("export const {}{name}", s.word());
            let _ = writeln!(want, "{head}: SchemePalette = {{");
            for (k, c) in &named {
                let _ = writeln!(want, "  {k}: '{}',", c.css());
            }
            want.push_str("};\n\n");
            let theirs = if src.contains(&head) {
                object(&src, &head)
            } else {
                BTreeMap::new()
            };
            if ours(named) != theirs {
                differs.push(head);
            }
        }
    }
    assert!(
        differs.is_empty(),
        "{} differ; the TypeScript should read:\n\n{want}",
        differs.join(", ")
    );
}
