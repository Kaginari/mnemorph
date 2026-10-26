//! Styled text measured in terminal cells: width, truncation, padding and word wrap over spans,
//! wide characters and graphemes counted as the terminal draws them.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A block of rendered lines: what every component's `view` returns.
pub type Lines = Vec<Line<'static>>;

/// The cells `s` takes.
#[must_use]
pub fn cells(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// The cells a line takes.
#[must_use]
pub fn line_cells(line: &Line<'_>) -> usize {
    line.spans.iter().map(|s| cells(&s.content)).sum()
}

/// The widest line of a block.
#[must_use]
pub fn block_cells(lines: &[Line<'_>]) -> usize {
    lines.iter().map(line_cells).max().unwrap_or(0)
}

/// `s` cut to `width` cells, ending in `…` when cut.
#[must_use]
pub fn clip(s: &str, width: usize) -> String {
    if cells(s) <= width {
        return s.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for g in s.graphemes(true) {
        let w = cells(g);
        if used + w > width - 1 {
            break;
        }
        out.push_str(g);
        used += w;
    }
    out.push('…');
    out
}

/// `s` cut to `width` cells from the left, starting with `…` when cut: a path keeps its name.
#[must_use]
pub fn clip_left(s: &str, width: usize) -> String {
    if cells(s) <= width {
        return s.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut kept: Vec<&str> = Vec::new();
    let mut used = 0;
    for g in s.graphemes(true).rev() {
        let w = cells(g);
        if used + w > width - 1 {
            break;
        }
        kept.push(g);
        used += w;
    }
    let mut out = String::from("…");
    out.extend(kept.into_iter().rev());
    out
}

/// A line cut to `width` cells, ending in `…` when cut; its spans keep their styles.
#[must_use]
pub fn truncate(line: Line<'static>, width: usize) -> Line<'static> {
    if line_cells(&line) <= width {
        return line;
    }
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    let mut last = Style::default();
    for span in line.spans {
        let w = cells(&span.content);
        last = span.style;
        if used + w < width {
            used += w;
            out.push(span);
            continue;
        }
        let room = width.saturating_sub(used);
        let cut = clip(&span.content, room);
        out.push(Span::styled(cut, span.style));
        used = width;
        break;
    }
    if used < width {
        out.push(Span::styled("…", last));
    }
    Line::from(out).style(line.style)
}

/// Where a line sits in a wider space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Flush left.
    #[default]
    Left,
    /// Centred, the odd cell on the right.
    Center,
    /// Flush right.
    Right,
}

/// A line cut or padded to exactly `width` cells, the padding in `fill`.
#[must_use]
pub fn fit(line: Line<'static>, width: usize, align: Align, fill: Style) -> Line<'static> {
    let line = truncate(line, width);
    let gap = width.saturating_sub(line_cells(&line));
    if gap == 0 {
        return line;
    }
    let (left, right) = match align {
        Align::Left => (0, gap),
        Align::Right => (gap, 0),
        Align::Center => (gap / 2, gap - gap / 2),
    };
    let style = line.style;
    let mut spans = Vec::with_capacity(line.spans.len() + 2);
    if left > 0 {
        spans.push(Span::styled(" ".repeat(left), fill));
    }
    spans.extend(line.spans);
    if right > 0 {
        spans.push(Span::styled(" ".repeat(right), fill));
    }
    Line::from(spans).style(style)
}

/// One piece of a line for wrapping: a word or a run of spaces, with its style.
struct Atom {
    text: String,
    style: Style,
    space: bool,
}

fn atoms(spans: &[Span<'_>]) -> Vec<Atom> {
    let mut out: Vec<Atom> = Vec::new();
    for span in spans {
        for word in span.content.split_word_bounds() {
            let space = word.chars().all(char::is_whitespace);
            match out.last_mut() {
                // A word split across two spans stays one word: the break is a style change.
                Some(a) if a.space == space && a.style == span.style => a.text.push_str(word),
                _ => out.push(Atom {
                    text: word.to_owned(),
                    style: span.style,
                    space,
                }),
            }
        }
    }
    out
}

/// One logical line of spans wrapped to `width` cells: words kept whole when they fit, a word
/// wider than the line broken by grapheme, spaces at a break dropped.
#[must_use]
pub fn wrap(spans: &[Span<'_>], width: usize) -> Lines {
    let width = width.max(1);
    let mut lines: Lines = Vec::new();
    let mut cur: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for atom in atoms(spans) {
        let w = cells(&atom.text);
        if atom.space {
            if used == 0 && !lines.is_empty() {
                continue;
            }
            if used + w > width {
                lines.push(Line::from(std::mem::take(&mut cur)));
                used = 0;
                continue;
            }
            used += w;
            cur.push(Span::styled(atom.text, atom.style));
            continue;
        }
        if used + w > width && used > 0 {
            trim_end(&mut cur);
            lines.push(Line::from(std::mem::take(&mut cur)));
            used = 0;
        }
        if w <= width - used {
            used += w;
            cur.push(Span::styled(atom.text, atom.style));
        } else {
            // A word wider than what is left: broken across lines.
            for g in atom.text.graphemes(true) {
                let gw = cells(g);
                if used + gw > width && used > 0 {
                    lines.push(Line::from(std::mem::take(&mut cur)));
                    used = 0;
                }
                used += gw;
                push_text(&mut cur, g, atom.style);
            }
        }
    }
    trim_end(&mut cur);
    if !cur.is_empty() || lines.is_empty() {
        lines.push(Line::from(cur));
    }
    lines
}

fn push_text(cur: &mut Vec<Span<'static>>, text: &str, style: Style) {
    match cur.last_mut() {
        Some(last) if last.style == style => {
            let mut s = last.content.to_string();
            s.push_str(text);
            last.content = s.into();
        }
        _ => cur.push(Span::styled(text.to_owned(), style)),
    }
}

fn trim_end(cur: &mut Vec<Span<'static>>) {
    while let Some(last) = cur.last_mut() {
        let t = last.content.trim_end().to_owned();
        if t.is_empty() {
            cur.pop();
        } else {
            last.content = t.into();
            break;
        }
    }
}

/// Plain text, wrapped: each `\n` starts a new line.
#[must_use]
pub fn wrap_text(text: &str, width: usize, style: Style) -> Lines {
    text.split('\n')
        .flat_map(|l| wrap(&[Span::styled(l.to_owned(), style)], width))
        .collect()
}

/// Each line with `first` before the first and `rest` before the others.
#[must_use]
pub fn prefixed(lines: Lines, first: &[Span<'static>], rest: &[Span<'static>]) -> Lines {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let mut spans = if i == 0 {
                first.to_vec()
            } else {
                rest.to_vec()
            };
            let style = l.style;
            spans.extend(l.spans);
            Line::from(spans).style(style)
        })
        .collect()
}

/// A line's text, its styles dropped.
#[must_use]
pub fn plain(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: &Lines) -> Vec<String> {
        lines.iter().map(plain).collect()
    }

    #[test]
    fn words_wrap_whole_and_long_words_break() {
        let l = wrap(&[Span::raw("the quick brown fox")], 10);
        assert_eq!(texts(&l), ["the quick", "brown fox"]);
        let l = wrap(&[Span::raw("abcdefghijkl")], 5);
        assert_eq!(texts(&l), ["abcde", "fghij", "kl"]);
        let l = wrap(&[Span::raw("")], 5);
        assert_eq!(texts(&l), [""]);
    }

    #[test]
    fn wide_characters_count_two_cells() {
        assert_eq!(cells("日本"), 4);
        let l = wrap(&[Span::raw("日本語のテキスト")], 6);
        assert!(l.iter().all(|l| line_cells(l) <= 6), "{:?}", texts(&l));
        assert_eq!(clip("日本語", 5), "日本…");
    }

    #[test]
    fn a_line_fits_exactly() {
        let l = fit(Line::from("abc"), 7, Align::Center, Style::default());
        assert_eq!(plain(&l), "  abc  ");
        let l = fit(Line::from("abcdefgh"), 5, Align::Left, Style::default());
        assert_eq!(plain(&l), "abcd…");
        assert_eq!(line_cells(&l), 5);
    }
}
