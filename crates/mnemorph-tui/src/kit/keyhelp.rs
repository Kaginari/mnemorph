//! Key help: `key action` pairs on one line, as many as fit, the most important first.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::text::cells;

/// One binding as the help shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// The key, as written (`esc`, `alt+enter`).
    pub key: &'static str,
    /// What it does.
    pub action: &'static str,
}

/// A row of bindings.
#[derive(Debug, Clone, Copy)]
pub struct KeyHelp {
    /// The key's style.
    pub key: Style,
    /// The action's style.
    pub action: Style,
    /// The separator's style.
    pub sep: Style,
}

impl KeyHelp {
    /// The bindings that fit in `width` cells, in order, a `·` between them; a binding that does
    /// not fit ends the row, so a narrow terminal keeps the first ones.
    #[must_use]
    pub fn view(&self, bindings: &[Binding], width: usize) -> Line<'static> {
        let mut spans = Vec::new();
        let mut used = 0;
        for (i, b) in bindings.iter().enumerate() {
            let sep = if i == 0 { 0 } else { 3 };
            let w = sep + cells(b.key) + 1 + cells(b.action);
            if used + w > width {
                break;
            }
            if i > 0 {
                spans.push(Span::styled(" · ", self.sep));
            }
            spans.push(Span::styled(b.key, self.key));
            spans.push(Span::styled(" ", self.action));
            spans.push(Span::styled(b.action, self.action));
            used += w;
        }
        Line::from(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::text::plain;

    #[test]
    fn it_keeps_what_fits() {
        let k = KeyHelp {
            key: Style::new(),
            action: Style::new(),
            sep: Style::new(),
        };
        let b = [
            Binding {
                key: "esc",
                action: "stop",
            },
            Binding {
                key: "ctrl+c",
                action: "quit",
            },
        ];
        assert_eq!(plain(&k.view(&b, 80)), "esc stop · ctrl+c quit");
        assert_eq!(plain(&k.view(&b, 12)), "esc stop");
        assert_eq!(plain(&k.view(&b, 3)), "");
    }
}
