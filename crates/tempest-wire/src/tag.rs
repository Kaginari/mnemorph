//! Splitting one line into its tag and the rest.

/// Reads `@TAG rest`, or the raw pipe form `T|rest` when `raw_ok`. Tags are upper-case words,
/// or the single characters `?` and `$`. Returns `None` for a line that is not a tag line.
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
    if raw_ok {
        // The pipe-dense form a mouth pair may agree on: `S|PASS`, `F|path:ln|claim`.
        let i = line.find('|')?;
        let tag = &line[..i];
        if matches!(tag, "S" | "F" | "V" | "?" | "U" | "E" | "T" | "$" | "W") {
            let rest = line[i + 1..].trim().replace('|', " — ");
            return Some((tag, rest));
        }
    }
    None
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
        assert_eq!(
            split("F|a.rs:3|one fact", true),
            Some(("F", "a.rs:3 — one fact".into()))
        );
        assert_eq!(split("F|a.rs:3|one fact", false), None);
        assert_eq!(split("X|not a tag", true), None);
    }
}
