//! Glamour for Ratatui: Markdown rendered to styled lines at a width, every style read from a
//! [`StyleSheet`] the theme derives — headings, emphasis, inline and fenced code (highlighted),
//! lists, quotes, links, rules and simple tables. Partial Markdown, as it streams, renders as
//! well as whole: an open fence is a code block to the end, an open emphasis is its characters.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::highlight::highlight;
use super::text::{Align, Lines, cells, fit, line_cells, truncate, wrap};
use crate::theme::Theme;

/// The styles Markdown is drawn in.
#[derive(Debug, Clone, Copy)]
pub struct StyleSheet {
    /// Body text.
    pub text: Style,
    /// `#`.
    pub h1: Style,
    /// `##`.
    pub h2: Style,
    /// `###` and below.
    pub h3: Style,
    /// `*emphasis*`.
    pub emph: Style,
    /// `**strong**`.
    pub strong: Style,
    /// `~~struck~~`.
    pub strike: Style,
    /// `` `code` ``.
    pub code: Style,
    /// A fenced block's text and background.
    pub code_block: Style,
    /// A quote's text.
    pub quote: Style,
    /// A quote's bar.
    pub quote_bar: Style,
    /// A link's text.
    pub link: Style,
    /// A link's target.
    pub link_url: Style,
    /// List bullets and numbers.
    pub bullet: Style,
    /// Rules and table borders.
    pub rule: Style,
    /// A table's header.
    pub table_head: Style,
}

impl StyleSheet {
    /// The stylesheet of a theme.
    #[must_use]
    pub fn of(t: &Theme) -> Self {
        let text = t.text();
        Self {
            text,
            h1: t.holo().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            h2: t.holo().add_modifier(Modifier::BOLD),
            h3: text.add_modifier(Modifier::BOLD),
            emph: Style::new().add_modifier(Modifier::ITALIC),
            strong: Style::new().add_modifier(Modifier::BOLD),
            strike: Style::new().add_modifier(Modifier::CROSSED_OUT),
            code: Style::new()
                .fg(t.readable(t.races.oni).color())
                .bg(t.p.panel_raised.color()),
            code_block: text.bg(t.p.panel_raised.color()),
            quote: t.muted().add_modifier(Modifier::ITALIC),
            quote_bar: t.frame(),
            link: t.holo().add_modifier(Modifier::UNDERLINED),
            link_url: t.muted(),
            bullet: t.holo(),
            rule: t.frame(),
            table_head: text.add_modifier(Modifier::BOLD),
        }
    }
}

/// A block that nests others, as it prefixes their lines.
enum Container {
    Quote,
    List(Option<u64>),
    Item { marker: String, pending: bool },
}

/// A table being read.
#[derive(Default)]
struct Table {
    aligns: Vec<Alignment>,
    rows: Vec<Vec<Vec<Span<'static>>>>,
    head_rows: usize,
}

struct Renderer<'a> {
    sheet: StyleSheet,
    theme: &'a Theme,
    width: usize,
    out: Lines,
    /// The current paragraph's lines, split at hard breaks.
    inline: Vec<Vec<Span<'static>>>,
    styles: Vec<Style>,
    containers: Vec<Container>,
    code: Option<(String, String)>,
    heading: Option<HeadingLevel>,
    links: Vec<String>,
    table: Option<Table>,
}

/// `md` drawn at `width` cells in `t`'s stylesheet.
#[must_use]
pub fn render(md: &str, width: usize, t: &Theme) -> Lines {
    let mut r = Renderer {
        sheet: StyleSheet::of(t),
        theme: t,
        width: width.max(8),
        out: Vec::new(),
        inline: vec![Vec::new()],
        styles: Vec::new(),
        containers: Vec::new(),
        code: None,
        heading: None,
        links: Vec::new(),
        table: None,
    };
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(md, opts) {
        r.event(event);
    }
    // Partial Markdown: whatever is still open is drawn as it stands.
    if let Some((lang, code)) = r.code.take() {
        r.code_block(&lang, &code);
    }
    r.flush();
    while r.out.last().is_some_and(|l| line_cells(l) == 0) {
        r.out.pop();
    }
    r.out
}

impl Renderer<'_> {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some((_, code)) = &mut self.code {
                    code.push_str(&text);
                } else {
                    self.push(&text, Style::new());
                }
            }
            Event::Code(text) => self.push(&text, self.sheet.code),
            Event::Html(text) | Event::InlineHtml(text) => {
                self.push(text.trim_end_matches('\n'), self.sheet.link_url);
            }
            Event::SoftBreak => self.push(" ", Style::new()),
            Event::HardBreak => self.inline.push(Vec::new()),
            Event::Rule => {
                self.flush();
                self.gap();
                let w = self.width.saturating_sub(self.prefix_cells());
                let rule = Line::from(Span::styled("─".repeat(w), self.sheet.rule));
                self.emit(vec![rule]);
            }
            Event::TaskListMarker(done) => {
                self.push(if done { "☑ " } else { "☐ " }, self.sheet.bullet);
            }
            Event::FootnoteReference(name) => self.push(&format!("[{name}]"), self.sheet.link),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.flush();
                self.gap();
            }
            Tag::Heading { level, .. } => {
                self.flush();
                self.gap();
                self.heading = Some(level);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.gap();
                self.containers.push(Container::Quote);
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                self.gap();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_owned()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new()));
            }
            Tag::List(start) => {
                self.flush();
                if !self.in_item() {
                    self.gap();
                }
                self.containers.push(Container::List(start));
            }
            Tag::Item => {
                self.flush();
                let marker = match self.containers.last_mut() {
                    Some(Container::List(Some(n))) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "• ".to_owned(),
                };
                self.containers.push(Container::Item {
                    marker,
                    pending: true,
                });
            }
            Tag::Emphasis => self.styles.push(self.sheet.emph),
            Tag::Strong => self.styles.push(self.sheet.strong),
            Tag::Strikethrough => self.styles.push(self.sheet.strike),
            Tag::Link { dest_url, .. } => {
                self.styles.push(self.sheet.link);
                self.links.push(dest_url.to_string());
            }
            Tag::Table(aligns) => {
                self.flush();
                self.gap();
                self.table = Some(Table {
                    aligns,
                    ..Table::default()
                });
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(t) = &mut self.table {
                    t.rows.push(Vec::new());
                }
            }
            Tag::TableCell => {
                if let Some(row) = self.table.as_mut().and_then(|t| t.rows.last_mut()) {
                    row.push(Vec::new());
                }
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                self.flush();
                self.heading = None;
            }
            TagEnd::CodeBlock => {
                if let Some((lang, code)) = self.code.take() {
                    self.code_block(&lang, &code);
                }
            }
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item => {
                self.flush();
                self.containers.pop();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.styles.pop();
            }
            TagEnd::Link => {
                self.styles.pop();
                if let Some(url) = self.links.pop() {
                    let text: String = self
                        .inline
                        .last()
                        .map(|l| l.iter().map(|s| s.content.as_ref()).collect())
                        .unwrap_or_default();
                    if !url.starts_with('#') && !text.ends_with(url.as_str()) {
                        self.push(&format!(" ({url})"), self.sheet.link_url);
                    }
                }
            }
            TagEnd::TableHead => {
                if let Some(t) = &mut self.table {
                    t.head_rows = t.rows.len();
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    self.table_block(&t);
                }
            }
            _ => {}
        }
    }

    fn in_item(&self) -> bool {
        self.containers
            .iter()
            .any(|c| matches!(c, Container::Item { .. }))
    }

    /// Text in the current style.
    fn push(&mut self, text: &str, own: Style) {
        let mut style = self.sheet.text;
        if self
            .containers
            .iter()
            .any(|c| matches!(c, Container::Quote))
        {
            style = style.patch(self.sheet.quote);
        }
        if let Some(level) = self.heading {
            style = style.patch(match level {
                HeadingLevel::H1 => self.sheet.h1,
                HeadingLevel::H2 => self.sheet.h2,
                _ => self.sheet.h3,
            });
        }
        for s in &self.styles {
            style = style.patch(*s);
        }
        style = style.patch(own);
        if let Some(cell) = self
            .table
            .as_mut()
            .and_then(|t| t.rows.last_mut())
            .and_then(|r| r.last_mut())
        {
            cell.push(Span::styled(text.to_owned(), style));
            return;
        }
        if let Some(line) = self.inline.last_mut() {
            line.push(Span::styled(text.to_owned(), style));
        }
    }

    /// A blank line between blocks, unless one is there already or nothing is.
    fn gap(&mut self) {
        let last_blank = self.out.last().is_none_or(|l| {
            l.spans
                .iter()
                .all(|s| s.content.trim().is_empty() || s.content.trim() == "│")
        });
        if !last_blank && !self.item_pending() {
            let (_, rest) = self.prefixes(false);
            self.out.push(Line::from(rest).style(Style::new()));
        }
    }

    fn item_pending(&self) -> bool {
        self.containers
            .iter()
            .any(|c| matches!(c, Container::Item { pending: true, .. }))
    }

    /// The prefixes of the next line and of the lines after it; `take` marks a pending bullet
    /// as drawn.
    fn prefixes(&mut self, take: bool) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
        let mut first = Vec::new();
        let mut rest = Vec::new();
        for c in &mut self.containers {
            match c {
                Container::Quote => {
                    first.push(Span::styled("│ ", self.sheet.quote_bar));
                    rest.push(Span::styled("│ ", self.sheet.quote_bar));
                }
                Container::List(_) => {}
                Container::Item { marker, pending } => {
                    let blank = " ".repeat(cells(marker));
                    if *pending {
                        first.push(Span::styled(marker.clone(), self.sheet.bullet));
                        if take {
                            *pending = false;
                        }
                    } else {
                        first.push(Span::raw(blank.clone()));
                    }
                    rest.push(Span::raw(blank));
                }
            }
        }
        (first, rest)
    }

    fn prefix_cells(&self) -> usize {
        self.containers
            .iter()
            .map(|c| match c {
                Container::Quote => 2,
                Container::List(_) => 0,
                Container::Item { marker, .. } => cells(marker),
            })
            .sum()
    }

    /// Lines prefixed by the containers they sit in.
    fn emit(&mut self, lines: Lines) {
        for (i, line) in lines.into_iter().enumerate() {
            let (first, rest) = self.prefixes(true);
            let mut spans = if i == 0 { first } else { rest };
            spans.extend(line.spans);
            self.out.push(Line::from(spans));
        }
    }

    /// The paragraph so far, wrapped and emitted.
    fn flush(&mut self) {
        let parts = std::mem::replace(&mut self.inline, vec![Vec::new()]);
        if parts.iter().all(Vec::is_empty) {
            return;
        }
        let width = self.width.saturating_sub(self.prefix_cells()).max(4);
        let lines: Lines = parts.iter().flat_map(|p| wrap(p, width)).collect();
        self.emit(lines);
    }

    fn code_block(&mut self, lang: &str, code: &str) {
        let width = self.width.saturating_sub(self.prefix_cells()).max(6);
        let bg = self.sheet.code_block;
        let fill = Style::new().bg(bg.bg.unwrap_or_default());
        let code = code.strip_suffix('\n').unwrap_or(code);
        let mut lines: Lines = Vec::new();
        if !lang.is_empty() {
            let label = Line::from(Span::styled(
                format!("{lang} "),
                self.theme.muted().bg(bg.bg.unwrap_or_default()),
            ));
            lines.push(fit(label, width, Align::Right, fill));
        }
        for spans in highlight(code, lang, self.theme, bg) {
            let mut l = vec![Span::styled(" ", fill)];
            l.extend(spans);
            let line = truncate(Line::from(l), width.saturating_sub(1));
            lines.push(fit(line, width, Align::Left, fill));
        }
        self.emit(lines);
    }

    fn table_block(&mut self, t: &Table) {
        let cols = t.rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        let mut widths = vec![0; cols];
        for row in &t.rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.iter().map(|s| cells(&s.content)).sum());
            }
        }
        let room = self
            .width
            .saturating_sub(self.prefix_cells() + 3 * (cols - 1));
        // Too wide: each column gives up cells in proportion to its width.
        let total: usize = widths.iter().sum();
        if total > room {
            for w in &mut widths {
                *w = (*w * room / total.max(1)).max(3);
            }
        }
        let sep = Span::styled(" │ ", self.sheet.rule);
        let mut lines: Lines = Vec::new();
        for (r, row) in t.rows.iter().enumerate() {
            let mut spans = Vec::new();
            for (i, w) in widths.iter().enumerate() {
                if i > 0 {
                    spans.push(sep.clone());
                }
                let mut cell = Line::from(row.get(i).cloned().unwrap_or_default());
                if r < t.head_rows {
                    cell = cell.patch_style(self.sheet.table_head);
                }
                let align = match t.aligns.get(i) {
                    Some(Alignment::Right) => Align::Right,
                    Some(Alignment::Center) => Align::Center,
                    _ => Align::Left,
                };
                spans.extend(fit(cell, *w, align, Style::new()).spans);
            }
            lines.push(Line::from(spans));
            if r + 1 == t.head_rows {
                let rule: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
                lines.push(Line::from(Span::styled(rule.join("─┼─"), self.sheet.rule)));
            }
        }
        self.emit(lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::text::plain;
    use crate::theme::Variant;

    fn texts(md: &str, w: usize) -> Vec<String> {
        render(md, w, &Theme::new(Variant::Dark))
            .iter()
            .map(plain)
            .collect()
    }

    #[test]
    fn lists_quotes_and_rules() {
        let out = texts("- one\n- two\n  - deep\n\n1. a\n2. b\n\n> said\n\n---", 30);
        assert!(out.contains(&"• one".to_owned()), "{out:?}");
        assert!(out.contains(&"  • deep".to_owned()), "{out:?}");
        assert!(out.contains(&"2. b".to_owned()), "{out:?}");
        assert!(out.contains(&"│ said".to_owned()), "{out:?}");
        assert!(out.last().is_some_and(|l| l.starts_with("───")), "{out:?}");
    }

    #[test]
    fn a_long_item_hangs_under_its_text() {
        let out = texts("- the quick brown fox jumps", 14);
        assert_eq!(out[0], "• the quick");
        assert!(out[1].starts_with("  "), "{out:?}");
    }

    #[test]
    fn a_table_lines_up() {
        let out = texts("| a | bb |\n|---|---:|\n| ccc | d |", 40);
        assert_eq!(out[0], "a   │ bb");
        assert_eq!(out[1], "────┼───");
        assert_eq!(out[2], "ccc │  d");
    }

    #[test]
    fn half_streamed_markdown_never_breaks() {
        let md = "# Title\n\nSome **bold and `code` in a\n\n```rust\nfn main() {\n";
        for end in 0..=md.len() {
            if md.is_char_boundary(end) {
                let _ = render(&md[..end], 20, &Theme::new(Variant::Light));
            }
        }
        let out = texts(md, 40);
        assert!(out.iter().any(|l| l.contains("fn main()")), "{out:?}");
    }
}
