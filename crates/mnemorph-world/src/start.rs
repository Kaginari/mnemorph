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
/// the oldest are dropped first, and a lone entry longer than `cap` keeps its head. An entry
/// begins at its `### [` heading — whether a `---` rule precedes it or not — and runs to the next;
/// a rule inside it is its own. `None` when the journal holds no entry.
#[must_use]
pub fn recent(world: &Path, n: usize, cap: usize) -> Option<String> {
    let text = std::fs::read_to_string(world.join(".mnemorph/log.md"))
        .ok()?
        .replace("\r\n", "\n");
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices("\n### [").map(|(i, _)| i + 1))
        .filter(|&i| text[i..].starts_with("### ["))
        .collect();
    let entries: Vec<&str> = starts
        .iter()
        .enumerate()
        .map(|(k, &from)| {
            let to = starts.get(k + 1).copied().unwrap_or(text.len());
            // The rule that closes an entry belongs to the next one's opening, not to its text.
            let body = text[from..to].trim_end();
            body.strip_suffix("---").map_or(body, str::trim_end)
        })
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

#[cfg(test)]
mod tests {
    use super::recent;

    fn world(name: &str, log: &str) -> std::path::PathBuf {
        let w = std::env::temp_dir().join(format!("mnemorph-start-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        std::fs::create_dir_all(w.join(".mnemorph")).unwrap();
        std::fs::write(w.join(".mnemorph/log.md"), log).unwrap();
        w
    }

    #[test]
    fn entries_begin_at_their_headings_rule_or_none() {
        let log = "# Journal\n\n---\n\n### [1] a\none\n\n### [2] b\ntwo\n---\nstill two\n\n---\n\n### [3] c\nthree\n";
        let w = world("headings", log);
        assert_eq!(
            recent(&w, 2, 1_000).unwrap(),
            "### [2] b\ntwo\n---\nstill two\n\n### [3] c\nthree"
        );
        assert_eq!(recent(&w, 9, 1_000).unwrap().matches("### [").count(), 3);
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn the_cap_drops_the_oldest_and_cuts_a_lone_entry() {
        let w = world("cap", "### [1] a\naaaa\n\n---\n\n### [2] b\nbbbb\n");
        assert_eq!(recent(&w, 5, 15).unwrap(), "### [2] b\nbbbb");
        assert_eq!(recent(&w, 1, 6).unwrap(), "### [2…");
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn crlf_reads_and_an_empty_journal_is_none() {
        let w = world("crlf", "# Journal\r\n\r\n---\r\n\r\n### [1] a\r\none\r\n");
        assert_eq!(recent(&w, 5, 100).unwrap(), "### [1] a\none");
        let e = world("empty", "# Journal\n");
        assert_eq!(recent(&e, 5, 100), None);
        let _ = std::fs::remove_dir_all(&w);
        let _ = std::fs::remove_dir_all(&e);
    }
}
