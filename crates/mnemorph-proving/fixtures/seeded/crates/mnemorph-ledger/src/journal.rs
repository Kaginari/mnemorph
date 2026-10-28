//! The run journal: every command a body ran, as it ran.

use std::io::{self, Write};

/// `text` with every word that looks like a key replaced by `[redacted]`.
pub fn redact(text: &str) -> String {
    text.split(' ')
        .map(|w| if w.starts_with("FAKE-TOKEN-") { "[redacted]" } else { w })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Journals one command: the environment it was given, then its argv.
pub fn journal_command(
    journal: &mut impl Write,
    argv: &[String],
    env: &[(String, String)],
) -> io::Result<()> {
    let prefix: Vec<String> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
    writeln!(journal, "{} {}", prefix.join(" "), redact(&argv.join(" ")))
}
