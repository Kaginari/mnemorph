//! Text for recall: words, content words, and BM25 over a small set of documents.

use std::collections::BTreeSet;

/// Words with no meaning of their own: a query of only these recalls nothing. English only:
/// another language's stop words pass as words, and unspaced scripts read as one word a run.
const STOP: &[&str] = &[
    "a", "all", "an", "and", "any", "are", "as", "at", "be", "but", "by", "can", "could", "did",
    "do", "does", "for", "from", "had", "has", "have", "how", "i", "if", "in", "into", "is", "it",
    "its", "may", "me", "might", "my", "no", "not", "of", "on", "or", "our", "should", "so",
    "that", "the", "their", "them", "then", "there", "these", "they", "this", "those", "to", "us",
    "was", "we", "were", "what", "when", "where", "which", "who", "why", "will", "with", "would",
    "you", "your",
];

/// A text's words for BM25: lower-cased, alphanumeric runs.
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// BM25 of `query` against `doc` among `docs`.
#[must_use]
pub fn bm25(query: &[String], doc: &[String], docs: &[Vec<String>]) -> f64 {
    const K1: f64 = 1.2;
    const B: f64 = 0.75;
    #[allow(clippy::cast_precision_loss, reason = "word counts are small")]
    let len = |d: &[String]| d.len() as f64;
    #[allow(clippy::cast_precision_loss, reason = "document counts are small")]
    let n = docs.len() as f64;
    let avg = docs.iter().map(|d| len(d)).sum::<f64>() / n.max(1.0);
    query
        .iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|term| {
            #[allow(clippy::cast_precision_loss, reason = "counts are small")]
            let df = docs.iter().filter(|d| d.contains(term)).count() as f64;
            #[allow(clippy::cast_precision_loss, reason = "counts are small")]
            let tf = doc.iter().filter(|w| *w == term).count() as f64;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len(doc) / avg.max(1.0)))
        })
        .sum()
}

/// A text's content words: its words, the stop words aside.
#[must_use]
pub fn content_words(text: &str) -> Vec<String> {
    words(text)
        .into_iter()
        .filter(|w| !STOP.contains(&w.as_str()))
        .collect()
}

/// A text's specific terms: the tokens that name one thing — a path, a file, an identifier —
/// rather than a common word. A token is specific when it holds `/`, `.`, `_`, `::` or a digit,
/// and is not a bare number, a bare directory or a file name every project has; quotes,
/// backticks and closing punctuation aside, lowercased. Each also yields its specific parts —
/// what `::` and `:` separate, and a path's file name — so `src/a.rs:12` meets `a.rs`, and
/// `t.sql::test_x` meets `t.sql`.
#[must_use]
pub fn specific_terms(text: &str) -> std::collections::BTreeSet<String> {
    // One thing named: a path or a file (not a bare directory, not a stock file name), an
    // identifier, a number inside a name — never a bare number or an everyday word.
    let specific = |t: &str| {
        !t.is_empty()
            && !t.ends_with('/')
            && !t.chars().all(|c| c.is_ascii_digit() || ".,:".contains(c))
            && !COMMON_FILES.contains(&t)
            && (t.contains(['/', '.', '_'])
                || t.contains("::")
                || t.chars().any(|c| c.is_ascii_digit()))
    };
    let mut out = std::collections::BTreeSet::new();
    for token in text
        .split(|c: char| c.is_whitespace() || "`'\"()[]{}<>,;".contains(c))
        .map(|t| t.trim_matches(|c: char| ".:!?".contains(c)).to_lowercase())
        .filter(|t| specific(t))
    {
        let mut parts: Vec<String> = token
            .split(':')
            .filter(|p| !p.is_empty() && !p.chars().all(|c| c.is_ascii_digit()))
            .map(str::to_owned)
            .collect();
        if let Some(name) = parts.first().and_then(|p| p.rsplit('/').next()) {
            parts.push(name.to_owned());
        }
        out.extend(parts.into_iter().filter(|p| specific(p)));
        if specific(&token) {
            out.insert(token);
        }
    }
    out
}

/// File names every project has: naming one names nothing in particular.
const COMMON_FILES: [&str; 12] = [
    "lib.rs",
    "mod.rs",
    "main.rs",
    "__init__.py",
    "index.ts",
    "index.js",
    "main.py",
    "setup.py",
    "readme.md",
    "cargo.toml",
    "package.json",
    "pyproject.toml",
];

#[cfg(test)]
mod tests {
    use super::specific_terms;

    #[test]
    fn specific_terms_name_one_thing() {
        let t = specific_terms(
            "Run `uv` before pytest in sqlglot/dialects, see parser.py:12 and test_round_trip.",
        );
        for want in [
            "sqlglot/dialects",
            "parser.py:12",
            "parser.py",
            "test_round_trip",
        ] {
            assert!(t.contains(want), "{want}: {t:?}");
        }
        for not in ["run", "uv", "before", "see", "and", "12"] {
            assert!(!t.contains(not), "{not}: {t:?}");
        }
        // What every project or every report names, names nothing in particular.
        let common =
            specific_terms("2 failed, 1 passed in src/ and tests/ — see lib.rs, expected timeout");
        assert!(common.is_empty(), "{common:?}");
    }
}
