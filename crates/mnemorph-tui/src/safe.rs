//! Text from a model or a tool, made safe to draw: nothing it carries can move the cursor,
//! recolour the screen, reorder what the human reads or hide a character.
//!
//! The approval card goes further and uses [`mnemorph_loop::shown`] verbatim, the core's own
//! rule for what a human approves. The transcript keeps newlines (Markdown needs them), expands
//! tabs, drops terminal colour sequences a tool printed, and escapes every other
//! [`mnemorph_loop::hidden`] character as `\u{…}`.

use mnemorph_loop::hidden;

/// The tab stop tool output is expanded to.
const TAB: usize = 4;

/// `text` with every line made safe; newlines kept.
#[must_use]
pub fn clean(text: &str) -> String {
    let text = strip_sgr(text);
    let mut out = String::with_capacity(text.len());
    let mut col = 0;
    for c in text.chars() {
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
            c if hidden(c) => {
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

/// One line of `text`, made safe: the newlines escaped too.
#[must_use]
pub fn one_line(text: &str) -> String {
    clean(text).replace('\n', "\\u{a}")
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
    }
}
