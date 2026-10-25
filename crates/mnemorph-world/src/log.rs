//! The journal's writer: `.mnemorph/log.md` is append-only. An entry is added at its end, whole,
//! in one write; nothing already written is ever rewritten, and the file is never written through
//! a link.

use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

use mnemorph_ledger::Timestamp;

/// Where the journal lives in a world.
pub const LOG: &str = ".mnemorph/log.md";

/// One entry: who, what, and its labelled lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The body it is about.
    pub body: String,
    /// What happened, in a few words.
    pub title: String,
    /// Its lines, each a label and its text.
    pub lines: Vec<(String, String)>,
}

impl Entry {
    /// The entry as it is written, stamped `at`.
    #[must_use]
    pub fn render(&self, at: Timestamp) -> String {
        let mut out = format!(
            "\n---\n\n### [{at}] {} — {}\n",
            clean(&self.body),
            clean(&self.title)
        );
        for (label, text) in &self.lines {
            out.push_str("- **");
            out.push_str(&clean(label));
            out.push_str(":** ");
            out.push_str(&clean(text));
            out.push('\n');
        }
        out
    }
}

/// One line, whatever it was: every control character and line separator becomes a space, so
/// text from a command or a file name can never open an entry of its own.
fn clean(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}' | '\u{85}') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// Append `entry` to `world`'s journal, stamped now. A world without `.mnemorph` has no journal:
/// nothing is written.
///
/// # Errors
///
/// The journal or its directory is a link, or it cannot be opened or written.
pub fn append(world: &Path, entry: &Entry) -> std::io::Result<()> {
    let path = world.join(LOG);
    for p in [path.parent(), Some(path.as_path())].into_iter().flatten() {
        if std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(std::io::Error::other(format!(
                "{} is a link: the journal is not written through it",
                p.display()
            )));
        }
    }
    if !path.parent().is_some_and(Path::is_dir) {
        return Ok(());
    }
    // A link put in its place after the check is refused by the kernel.
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)?;
    f.write_all(entry.render(Timestamp::now()).as_bytes())
}
