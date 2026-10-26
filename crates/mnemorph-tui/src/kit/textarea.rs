//! A multi-line text input with history: Enter submits, Alt+Enter (or Shift+Enter, Ctrl+J)
//! breaks the line, ↑ ↓ move between lines and, at the edge, through the lines sent before.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;

use super::text::{Lines, cells};

/// What a key did to the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The text changed or the cursor moved.
    Edited,
    /// The human sent a line.
    Submit(String),
    /// The key is not the input's.
    Ignored,
}

/// The input's state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextArea {
    text: String,
    /// The cursor, a byte index into `text` on a grapheme boundary.
    cursor: usize,
    history: Vec<String>,
    /// Where ↑ ↓ are in the history; `history.len()` is the line being written.
    at: usize,
    /// The line being written, kept while browsing the history.
    draft: String,
}

impl TextArea {
    /// The text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The cursor's byte index.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Insert `s` at the cursor; a pasted `\r\n` or `\r` becomes `\n`.
    pub fn insert(&mut self, s: &str) {
        let clean = s.replace("\r\n", "\n").replace('\r', "\n");
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }

    /// Apply a key.
    pub fn update(&mut self, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Enter if alt || shift => self.insert("\n"),
            KeyCode::Char('j') if ctrl => self.insert("\n"),
            KeyCode::Enter => return self.submit(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.cursor = self.before(self.cursor),
            KeyCode::Right => self.cursor = self.after(self.cursor),
            KeyCode::Up => self.up(),
            KeyCode::Down => self.down(),
            KeyCode::Home => self.cursor = self.line_bounds().0,
            KeyCode::End => self.cursor = self.line_bounds().1,
            KeyCode::Char('a') if ctrl => self.cursor = self.line_bounds().0,
            KeyCode::Char('e') if ctrl => self.cursor = self.line_bounds().1,
            KeyCode::Char('u') if ctrl => self.kill_back(),
            KeyCode::Char('w') if ctrl => self.kill_word(),
            KeyCode::Char(c) if !ctrl && !alt => self.insert(c.encode_utf8(&mut [0; 4])),
            KeyCode::Tab => self.insert("    "),
            _ => return Outcome::Ignored,
        }
        Outcome::Edited
    }

    /// Send the text: it joins the history and the input empties. Blank text sends nothing.
    pub fn submit(&mut self) -> Outcome {
        let line = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.draft.clear();
        if line.trim().is_empty() {
            self.at = self.history.len();
            return Outcome::Edited;
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        self.at = self.history.len();
        Outcome::Submit(line)
    }

    fn before(&self, i: usize) -> usize {
        self.text[..i]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(b, _)| b)
    }

    fn after(&self, i: usize) -> usize {
        self.text[i..]
            .graphemes(true)
            .next()
            .map_or(self.text.len(), |g| i + g.len())
    }

    fn backspace(&mut self) {
        let start = self.before(self.cursor);
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    fn delete(&mut self) {
        let end = self.after(self.cursor);
        self.text.replace_range(self.cursor..end, "");
    }

    /// The start and end of the cursor's line.
    fn line_bounds(&self) -> (usize, usize) {
        let start = self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
        let end = self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| self.cursor + i);
        (start, end)
    }

    fn column(&self) -> usize {
        let (start, _) = self.line_bounds();
        self.text[start..self.cursor].graphemes(true).count()
    }

    fn at_column(&self, start: usize, end: usize, col: usize) -> usize {
        self.text[start..end]
            .grapheme_indices(true)
            .nth(col)
            .map_or(end, |(b, _)| start + b)
    }

    fn up(&mut self) {
        let (start, _) = self.line_bounds();
        if start > 0 {
            let col = self.column();
            let prev_end = start - 1;
            let prev_start = self.text[..prev_end].rfind('\n').map_or(0, |i| i + 1);
            self.cursor = self.at_column(prev_start, prev_end, col);
            return;
        }
        if self.at == 0 {
            return;
        }
        if self.at == self.history.len() {
            self.draft = self.text.clone();
        }
        self.at -= 1;
        self.text = self.history[self.at].clone();
        self.cursor = self.text.len();
    }

    fn down(&mut self) {
        let (_, end) = self.line_bounds();
        if end < self.text.len() {
            let col = self.column();
            let next_start = end + 1;
            let next_end = self.text[next_start..]
                .find('\n')
                .map_or(self.text.len(), |i| next_start + i);
            self.cursor = self.at_column(next_start, next_end, col);
            return;
        }
        if self.at >= self.history.len() {
            return;
        }
        self.at += 1;
        self.text = if self.at == self.history.len() {
            std::mem::take(&mut self.draft)
        } else {
            self.history[self.at].clone()
        };
        self.cursor = self.text.len();
    }

    fn kill_back(&mut self) {
        let (start, _) = self.line_bounds();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    fn kill_word(&mut self) {
        let head = &self.text[..self.cursor];
        let trimmed = head.trim_end();
        let start = trimmed.rfind(char::is_whitespace).map_or(0, |i| {
            i + trimmed[i..].chars().next().map_or(1, char::len_utf8)
        });
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    /// The input drawn in `width` cells, at most `rows` rows: the text broken at the width, the
    /// cursor an inverse cell (when `focused`), the rows around the cursor kept in view.
    #[must_use]
    pub fn view(&self, width: usize, rows: usize, text: Style, focused: bool) -> Lines {
        let width = width.max(1);
        let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
        let mut col = 0;
        let mut cursor_row = 0;
        let cursor_style = text.add_modifier(Modifier::REVERSED);
        let mut drawn = false;
        for (i, g) in self.text.grapheme_indices(true) {
            let here = focused && i == self.cursor;
            if g == "\n" {
                if here {
                    lines
                        .last_mut()
                        .into_iter()
                        .for_each(|l| l.push(Span::styled(" ", cursor_style)));
                    cursor_row = lines.len() - 1;
                    drawn = true;
                }
                lines.push(Vec::new());
                col = 0;
                continue;
            }
            let w = cells(g).max(1);
            if col + w > width {
                lines.push(Vec::new());
                col = 0;
            }
            let style = if here { cursor_style } else { text };
            if here {
                cursor_row = lines.len() - 1;
                drawn = true;
            }
            if let Some(l) = lines.last_mut() {
                l.push(Span::styled(g.to_owned(), style));
            }
            col += w;
        }
        if focused && !drawn {
            if col + 1 > width {
                lines.push(Vec::new());
            }
            cursor_row = lines.len() - 1;
            if let Some(l) = lines.last_mut() {
                l.push(Span::styled(" ", cursor_style));
            }
        }
        let rows = rows.max(1);
        let first = (cursor_row + 1).saturating_sub(rows);
        lines
            .into_iter()
            .skip(first)
            .take(rows)
            .map(Line::from)
            .collect()
    }

    /// How many rows the text takes at `width`, before any cap.
    #[must_use]
    pub fn height(&self, width: usize) -> usize {
        self.view(width, usize::MAX, Style::new(), true).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::text::plain;

    fn key(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    fn typed(t: &mut TextArea, s: &str) {
        for c in s.chars() {
            t.update(key(KeyCode::Char(c), KeyModifiers::NONE));
        }
    }

    #[test]
    fn alt_enter_breaks_the_line_and_enter_sends() {
        let mut t = TextArea::default();
        typed(&mut t, "one");
        assert_eq!(
            t.update(key(KeyCode::Enter, KeyModifiers::ALT)),
            Outcome::Edited
        );
        typed(&mut t, "two");
        assert_eq!(t.text(), "one\ntwo");
        assert_eq!(
            t.update(key(KeyCode::Enter, KeyModifiers::NONE)),
            Outcome::Submit("one\ntwo".into())
        );
        assert_eq!(t.text(), "");
    }

    #[test]
    fn up_and_down_walk_the_history_and_keep_the_draft() {
        let mut t = TextArea::default();
        typed(&mut t, "first");
        t.submit();
        typed(&mut t, "second");
        t.submit();
        typed(&mut t, "dra");
        t.update(key(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(t.text(), "second");
        t.update(key(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(t.text(), "first");
        t.update(key(KeyCode::Down, KeyModifiers::NONE));
        t.update(key(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(t.text(), "dra");
    }

    #[test]
    fn up_moves_between_lines_before_the_history() {
        let mut t = TextArea::default();
        t.insert("abc\nde");
        t.update(key(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(t.cursor(), 2);
        assert_eq!(t.text(), "abc\nde");
    }

    #[test]
    fn graphemes_are_one_step_and_the_view_wraps() {
        let mut t = TextArea::default();
        t.insert("e\u{301}x");
        t.update(key(KeyCode::Left, KeyModifiers::NONE));
        t.update(key(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(t.text(), "x");
        let mut t = TextArea::default();
        t.insert("abcdefgh");
        let v = t.view(5, 10, Style::new(), true);
        assert_eq!(v.iter().map(plain).collect::<Vec<_>>(), ["abcde", "fgh "]);
        t.update(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(t.text(), "");
    }
}
