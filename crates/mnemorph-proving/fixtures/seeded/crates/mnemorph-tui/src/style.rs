//! The terminal's own styling: escapes the harness writes itself, around text made safe.

/// Bold on.
pub const BOLD: &str = "\x1b[1m";
/// Every style off.
pub const RESET: &str = "\x1b[0m";

/// `text` with every control character shown as `?`.
pub fn sanitise(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// A heading: the harness's bold around text that cannot drive the terminal.
pub fn heading(text: &str) -> String {
    format!("{BOLD}{}{RESET}", sanitise(text))
}
