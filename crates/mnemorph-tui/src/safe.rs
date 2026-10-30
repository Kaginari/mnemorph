//! Text from a model or a tool, made safe to draw: nothing it carries can move the cursor,
//! recolour the screen, reorder what the human reads or hide a character.
//!
//! The approval card goes further and uses [`mnemorph_loop::shown`] verbatim, the core's own
//! rule for what a human approves. The transcript keeps newlines (Markdown needs them), expands
//! tabs, drops terminal colour sequences a tool printed, and escapes every other
//! [`mnemorph_loop::hidden`] character as `\u{…}` but the emoji selector prose keeps
//! ([`mnemorph_loop::kept_in_prose`]). Code and a one-line field — a name, a path, a reason —
//! keep none, and follow the card's rule ([`Strict`]): a space other than U+0020 and a combining
//! mark past a base's second are escaped too.

use mnemorph_loop::wire::Strict;
use mnemorph_loop::{hidden, kept_in_prose};

/// The tab stop tool output is expanded to.
const TAB: usize = 4;

/// `text` with every line made safe; newlines kept.
#[must_use]
pub fn clean(text: &str) -> String {
    render(text, true)
}

/// Code a human may copy and run, made safe: newlines kept, and every [`hidden`] character
/// escaped, the emoji selector too.
#[must_use]
pub fn code(text: &str) -> String {
    render(text, false)
}

/// One line of `text`, made safe: the newlines escaped too, and every [`hidden`] character.
#[must_use]
pub fn one_line(text: &str) -> String {
    render(text, false).replace('\n', "\\u{a}")
}

/// [`clean`], [`code`] and [`one_line`]: with `prose`, what [`kept_in_prose`] names is kept;
/// without, the card's rule ([`Strict`]) applies.
fn render(text: &str, prose: bool) -> String {
    let text = strip_sgr(text);
    let mut out = String::with_capacity(text.len());
    let mut col = 0;
    let mut strict = Strict::default();
    for c in text.chars() {
        let escaped = if prose {
            hidden(c) && !kept_in_prose(c)
        } else {
            strict.escapes(c)
        };
        match c {
            '\n' => {
                out.push('\n');
                col = 0;
            }
            '\t' => {
                let n = TAB - col % TAB;
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            }
            '\r' => {}
            c if escaped => {
                let e: String = c.escape_unicode().collect();
                col += e.len();
                out.push_str(&e);
            }
            c => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// `text` without the colour sequences (`ESC [ … m`) a terminal program prints; any other escape
/// is left for [`clean`] to show.
fn strip_sgr(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('\u{1b}') {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("\u{1b}[") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 2..];
        let end = tail.find(|c: char| !(c.is_ascii_digit() || c == ';'));
        match end {
            Some(e) if tail[e..].starts_with('m') => rest = &tail[e + 1..],
            _ => {
                out.push('\u{1b}');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_drop_and_everything_hidden_shows() {
        assert_eq!(clean("\u{1b}[31mred\u{1b}[0m ok"), "red ok");
        assert_eq!(clean("a\u{1b}]0;title\u{7}b"), "a\\u{1b}]0;title\\u{7}b");
        assert_eq!(clean("x\u{202e}y\u{200b}z"), "x\\u{202e}y\\u{200b}z");
        assert_eq!(clean("a\tb\nc"), "a   b\nc");
        assert_eq!(one_line("a\nb"), "a\\u{a}b");
        assert_eq!(clean("back\\slash"), "back\\slash");
        // Prose keeps the emoji selector; a one-line field and every other invisible escape.
        assert_eq!(clean("ok \u{2714}\u{FE0F}"), "ok \u{2714}\u{FE0F}");
        assert_eq!(one_line("rm\u{FE0F}"), "rm\\u{fe0f}");
        assert_eq!(code("rm\u{FE0F}\nx"), "rm\\u{fe0f}\nx");
        // A one-line field follows the card: spaces and stacked marks; prose keeps both.
        assert_eq!(one_line("a\u{A0}b"), "a\\u{a0}b");
        assert_eq!(
            one_line("e\u{301}\u{302}\u{303}"),
            "e\u{301}\u{302}\\u{303}"
        );
        assert_eq!(
            clean("a\u{A0}e\u{301}\u{302}\u{303}"),
            "a\u{A0}e\u{301}\u{302}\u{303}"
        );
        assert_eq!(
            clean("tag\u{E0041}\u{61C}\u{180E}"),
            "tag\\u{e0041}\\u{61c}\\u{180e}"
        );
    }
}
