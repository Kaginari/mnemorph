//! Splitting one line into its tag and the rest.

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
    use super::split;

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
