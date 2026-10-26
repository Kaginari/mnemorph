//! Code in colour: syntect's grammars (pure-Rust regex engine) under a syntax theme derived from
//! the palette, so code reads in the dark and the light variant alike.

use std::str::FromStr;
use std::sync::OnceLock;

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SynColor, FontStyle, ScopeSelectors, StyleModifier, Theme as SynTheme, ThemeItem,
    ThemeSettings,
};
use syntect::parsing::SyntaxSet;

use crate::theme::{Rgb, Theme};

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();

/// The grammars, loaded once.
fn syntaxes() -> &'static SyntaxSet {
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The languages warmed ahead: a grammar compiles its patterns the first time it is used.
const COMMON: [&str; 6] = ["rust", "python", "ts", "go", "sh", "json"];

/// Load the grammars now, and compile the common languages' patterns, if not done yet.
pub fn preload() {
    let set = syntaxes();
    let theme = SynTheme::default();
    for lang in COMMON {
        if let Some(syntax) = set.find_syntax_by_token(lang) {
            let mut h = HighlightLines::new(syntax, &theme);
            let sample = "fn f(x: u32) -> u32 { let s = \"a\"; x + 1 } // c\n";
            let _ = h.highlight_line(sample, set);
        }
    }
}

/// Load the grammars on a thread of their own, so the first code block draws without waiting.
pub fn warm() {
    std::thread::spawn(preload);
}

fn syn(c: Rgb) -> SynColor {
    SynColor {
        r: c.0,
        g: c.1,
        b: c.2,
        a: 0xff,
    }
}

fn item(scope: &str, c: Rgb, font: FontStyle) -> ThemeItem {
    ThemeItem {
        scope: ScopeSelectors::from_str(scope).unwrap_or_default(),
        style: StyleModifier {
            foreground: Some(syn(c)),
            background: None,
            font_style: Some(font),
        },
    }
}

/// The syntax theme of a palette: keywords in the accent, strings in the elves' green, numbers
/// and constants in gold, functions in Rimuru's blue, types in the orcs' amber, comments muted.
#[must_use]
pub fn syntax_theme(t: &Theme) -> SynTheme {
    let r = |c: Rgb| t.readable(c);
    let none = FontStyle::empty();
    SynTheme {
        name: Some("mnemorph".into()),
        author: None,
        settings: ThemeSettings {
            foreground: Some(syn(t.p.text)),
            ..ThemeSettings::default()
        },
        scopes: vec![
            item(
                "comment, punctuation.definition.comment",
                t.p.text_muted,
                FontStyle::ITALIC,
            ),
            item("keyword, storage.modifier, keyword.control", t.p.holo, none),
            item("storage.type", r(t.races.orc), none),
            item(
                "string, string.quoted, constant.character",
                r(t.races.elf),
                none,
            ),
            item(
                "constant.numeric, constant.language, constant.other",
                t.p.gold,
                none,
            ),
            item(
                "entity.name.function, support.function, meta.function-call",
                r(t.races.rimuru),
                none,
            ),
            item(
                "entity.name.type, entity.name.class, support.type, support.class",
                r(t.races.orc),
                none,
            ),
            item("entity.name.tag, meta.attribute", r(t.races.dark_elf), none),
            item("variable.parameter", t.p.text, FontStyle::ITALIC),
            item(
                "punctuation, keyword.operator",
                t.p.text.mix(t.p.text_muted, 0.5),
                none,
            ),
        ],
    }
}

/// `code` highlighted as `lang` (a fence's info word or a file extension), one span list per
/// line; an unknown language comes back plain in `base`.
#[must_use]
pub fn highlight(code: &str, lang: &str, t: &Theme, base: Style) -> Vec<Vec<Span<'static>>> {
    let set = syntaxes();
    let syntax = (!lang.is_empty())
        .then(|| {
            set.find_syntax_by_token(lang)
                .or_else(|| set.find_syntax_by_extension(lang))
        })
        .flatten();
    let Some(syntax) = syntax else {
        return code
            .lines()
            .map(|l| vec![Span::styled(l.to_owned(), base)])
            .collect();
    };
    let theme = syntax_theme(t);
    let mut h = HighlightLines::new(syntax, &theme);
    let mut out = Vec::new();
    for line in code.split_inclusive('\n') {
        let Ok(regions) = h.highlight_line(line, set) else {
            out.push(vec![Span::styled(
                line.trim_end_matches('\n').to_owned(),
                base,
            )]);
            continue;
        };
        let spans = regions
            .into_iter()
            .filter_map(|(s, text)| {
                let text = text.trim_end_matches('\n');
                if text.is_empty() {
                    return None;
                }
                let mut style = base.fg(ratatui::style::Color::Rgb(
                    s.foreground.r,
                    s.foreground.g,
                    s.foreground.b,
                ));
                if s.font_style.contains(FontStyle::ITALIC) {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if s.font_style.contains(FontStyle::BOLD) {
                    style = style.add_modifier(Modifier::BOLD);
                }
                Some(Span::styled(text.to_owned(), style))
            })
            .collect();
        out.push(spans);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Variant;

    #[test]
    fn rust_gets_more_than_one_colour_in_both_variants() {
        for v in [Variant::Dark, Variant::Light] {
            let t = Theme::new(v);
            let lines = highlight("fn main() { let x = \"hi\"; }\n", "rust", &t, t.text());
            let colours: std::collections::HashSet<_> =
                lines[0].iter().filter_map(|s| s.style.fg).collect();
            assert!(colours.len() >= 3, "{v:?}: {colours:?}");
        }
    }

    #[test]
    fn an_unknown_language_is_plain() {
        let t = Theme::new(Variant::Dark);
        let lines = highlight("a\nb", "no-such-language", &t, t.text());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0][0].style, t.text());
    }
}
