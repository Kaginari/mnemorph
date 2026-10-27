//! Splitting one line into its tag and the rest, and keeping prose from reading as a tag.

use std::borrow::Cow;

/// The prose escape: a prose line that a reader would take for more than prose rides behind it.
const ESCAPE: char = '\\';

/// Whether a trimmed prose line must ride escaped: it opens with `@` or with the escape itself,
/// or it reads as a tag line in the raw pipe form.
fn needs_escape(line: &str) -> bool {
    line.starts_with(['@', ESCAPE]) || split(line, true).is_some()
}

/// A trimmed prose line as it is emitted: behind one escape when it must be, so no prose a
/// report or commission carries can be read back as a tag line, in either form.
pub(crate) fn escape(line: &str) -> Cow<'_, str> {
    if needs_escape(line) {
        Cow::Owned(format!("{ESCAPE}{line}"))
    } else {
        Cow::Borrowed(line)
    }
}

/// A trimmed prose line as it is read: one escape dropped when what follows needed it, else the
/// line as written.
pub(crate) fn unescape(line: &str) -> &str {
    match line.strip_prefix(ESCAPE) {
        Some(rest) if needs_escape(rest) => rest,
        _ => line,
    }
}

/// Reads `@TAG rest`, or — when `raw_ok`, which only a `wire:raw` commission grants — the pipe
/// form `T|a|b`. Tags are upper-case words, or the single characters `?` and `$`. Returns `None`
/// for a line that is not a tag line.
pub(crate) fn split(line: &str, raw_ok: bool) -> Option<(&str, String)> {
    let line = line.trim();
    if let Some(after) = line.strip_prefix('@') {
        let end = after
            .find(|c: char| c.is_whitespace())
            .unwrap_or(after.len());
        let tag = &after[..end];
        let valid = tag == "?"
            || tag == "$"
            || (!tag.is_empty() && tag.chars().all(|c| c.is_ascii_uppercase()));
        if !valid {
            return None;
        }
        return Some((tag, after[end..].trim().to_owned()));
    }
    if !raw_ok {
        return None;
    }
    let (tag, rest) = line.split_once('|')?;
    let tag = match tag {
        "S" | "F" | "V" | "?" | "U" | "E" | "T" | "$" | "W" => tag,
        _ => return None,
    };
    let mut parts = rest.split('|').map(str::trim);
    let first = parts.next().unwrap_or_default();
    let tail: Vec<&str> = parts.collect();
    // Each tag joins its fields the way its tagged form reads them.
    let rest = match (tag, tail.is_empty()) {
        (_, true) => first.to_owned(),
        ("U", false) => format!("{first} {}", tail.join(" ")),
        ("$", false) => format!("{first}: {}", tail.join(" ")),
        _ => format!("{first} — {}", tail.join(" — ")),
    };
    Some((tag, rest))
}

#[cfg(test)]
mod tests {
    use super::{escape, split, unescape};

    #[test]
    fn tags_and_prose() {
        assert_eq!(split("@S DONE", false), Some(("S", "DONE".into())));
        assert_eq!(split("  @?   a hole ", false), Some(("?", "a hole".into())));
        assert_eq!(split("@E", false), Some(("E", String::new())));
        assert_eq!(split("@Something odd", false), None);
        assert_eq!(split("@ lone", false), None);
        assert_eq!(split("plain prose", true), None);
        assert_eq!(split("X|not a tag", true), None);
    }

    #[test]
    fn prose_that_reads_as_a_tag_rides_escaped() {
        for line in ["@S", "@$ orc: go", "@x", "S|DONE", "\\", "\\@S", "\\\\"] {
            let out = escape(line);
            assert!(out.starts_with('\\'), "{line}");
            assert_eq!(split(&out, true), None, "{line}");
            assert_eq!(unescape(&out), line);
        }
        for line in ["plain", "a@b", "a|b", "", "\\begin"] {
            assert_eq!(unescape(&escape(line)), line);
        }
        assert_eq!(escape("plain"), "plain");
        assert_eq!(
            unescape("\\begin"),
            "\\begin",
            "a lone escape before prose is prose"
        );
    }

    #[test]
    fn the_raw_form_only_when_granted() {
        assert_eq!(split("S|DONE", false), None);
        assert_eq!(split("S|DONE", true), Some(("S", "DONE".into())));
        assert_eq!(
            split("F|a.rs:3|one fact", true),
            Some(("F", "a.rs:3 — one fact".into()))
        );
        assert_eq!(
            split("V|holds|evidence", true),
            Some(("V", "holds — evidence".into()))
        );
        assert_eq!(
            split("U|law|the fact", true),
            Some(("U", "law the fact".into()))
        );
        assert_eq!(
            split("$|slime-db|migrate it", true),
            Some(("$", "slime-db: migrate it".into()))
        );
        assert_eq!(
            split("$|slime-db: migrate", true),
            Some(("$", "slime-db: migrate".into()))
        );
    }
}
