//! The Rust palette is the TypeScript palette: `packages/theme/src/index.ts` is read and every
//! colour of `dark`, `light` and `races` compared, both ways — none missing, none extra.

use std::collections::BTreeMap;

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
