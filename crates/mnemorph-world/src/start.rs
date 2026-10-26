//! What a fresh session is given by the harness itself, never by a file the host may or may not
//! load: the world's law, and the journal's last entries — what earlier sessions learned.

use std::path::Path;

/// The most of the law a prompt carries, in characters.
pub const LAW_CAP: usize = 4_000;

/// The world's law (`.mnemorph/law.md`), cut at [`LAW_CAP`]; `None` when the world has none.
#[must_use]
pub fn law(world: &Path) -> Option<String> {
    let text = std::fs::read_to_string(world.join(".mnemorph/law.md")).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| cut(text, LAW_CAP))
}

/// The journal's last `n` entries (`.mnemorph/log.md`), oldest first, within `cap` characters:
/// the oldest are dropped first, and a lone entry longer than `cap` keeps its head. `None` when the
/// journal holds no entry.
#[must_use]
pub fn recent(world: &Path, n: usize, cap: usize) -> Option<String> {
    let text = std::fs::read_to_string(world.join(".mnemorph/log.md")).ok()?;
    let entries: Vec<&str> = text
        .split("\n---\n")
        .map(str::trim)
        .filter(|e| e.starts_with("### ["))
        .collect();
    let mut kept: Vec<&str> = entries[entries.len().saturating_sub(n)..].to_vec();
    while kept.len() > 1 && kept.iter().map(|e| e.chars().count() + 2).sum::<usize>() > cap {
        kept.remove(0);
    }
    let joined = kept.join("\n\n");
    (!joined.is_empty()).then(|| cut(&joined, cap))
}

/// `text` within `max` characters, an ellipsis marking a cut.
fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        text.chars().take(max).collect::<String>() + "…"
    }
}
