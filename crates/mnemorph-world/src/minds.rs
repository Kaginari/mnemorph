//! The toolbox's two levels over the world's Minds: a manifest — each Mind a body may wear, in one
//! line with its price — and the Mind itself, worn on demand. The price is of exactly the text
//! wearing injects, never the file's size.

use std::path::Path;

use crate::Roster;

/// A Mind costing more than this, in tokens, is heavy: worn by a body below, never by a throne
/// that can hand the work down — a worn Mind cannot be unloaded.
pub const HEAVY: u64 = 2_000;

/// One Mind under `.mnemorph/minds/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mind {
    /// Its name: the directory's, or the `<name>.md` file's.
    pub name: String,
    /// Its doc, relative to the world.
    pub doc: String,
    /// What it is for, in one line: the doc's first line of prose.
    pub about: String,
    /// Marked `- **Shared:** yes`: any body may wear it.
    pub shared: bool,
    /// Its whole text.
    pub text: String,
}

impl Mind {
    /// Exactly what wearing it injects.
    #[must_use]
    pub fn injected(&self) -> String {
        format!(
            "The Mind `{}` ({}), worn:\n\n{}",
            self.name, self.doc, self.text
        )
    }

    /// Its price in tokens: what wearing injects, at four bytes a token — the estimate the fold
    /// uses; the ledger counts the real tokens once it is worn.
    #[must_use]
    pub fn price(&self) -> u64 {
        u64::try_from(self.injected().len().div_ceil(4)).unwrap_or(u64::MAX)
    }

    /// Its manifest line.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "@T mind:{} — {} (~{} tokens)",
            self.name,
            self.about,
            self.price()
        )
    }
}

/// A description at most 120 characters long, cut at a word.
fn about_of(line: &str) -> String {
    if line.chars().count() <= 120 {
        return line.to_owned();
    }
    let head: String = line.chars().take(120).collect();
    let cut = head.rfind(' ').map_or(head.as_str(), |i| &head[..i]);
    format!("{cut}…")
}

/// The world's Minds, by name.
#[must_use]
pub fn load(world: &Path) -> Vec<Mind> {
    let dir = world.join(".mnemorph").join("minds");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut minds: Vec<Mind> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            let (name, doc) = if path.is_dir() {
                (file.clone(), format!(".mnemorph/minds/{file}/README.md"))
            } else {
                (
                    file.strip_suffix(".md")?.to_owned(),
                    format!(".mnemorph/minds/{file}"),
                )
            };
            // A README at the root indexes the Minds; it is none of them.
            if name.starts_with('.') || name == "README" {
                return None;
            }
            let text = std::fs::read_to_string(world.join(&doc)).ok()?;
            let header = || text.lines().take_while(|l| !l.starts_with("##"));
            let shared = header()
                .filter_map(|l| l.trim().strip_prefix("- **Shared:**"))
                .any(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "yes" | "true"));
            // The first line of prose: not a heading, a field, a bullet, a quote, a fence or a rule.
            let about = text
                .lines()
                .map(str::trim)
                .find(|l| {
                    !l.is_empty()
                        && !["#", "-", "*", ">", "`", "|", "="]
                            .iter()
                            .any(|p| l.starts_with(p))
                })
                .map_or_else(|| "(no description)".to_owned(), about_of);
            Some(Mind {
                name,
                doc,
                about,
                shared,
                text,
            })
        })
        .collect();
    minds.sort_by(|a, b| a.name.cmp(&b.name));
    minds
}

/// The Minds `body` may wear: those its doc's Minds field names, then the shared ones.
#[must_use]
pub fn wearable(world: &Path, roster: &Roster, body: &str) -> Vec<Mind> {
    let own: Vec<String> = roster
        .creatures
        .get(body)
        .map(|c| {
            c.minds
                .iter()
                .map(|m| m.trim_matches('`').to_owned())
                .collect()
        })
        .unwrap_or_default();
    load(world)
        .into_iter()
        .filter(|m| m.shared || own.contains(&m.name))
        .collect()
}

/// The level-one manifest for `body`: one line per Mind it may wear, its price, and how to wear
/// it. `None` when it may wear none.
#[must_use]
pub fn manifest(world: &Path, roster: &Roster, body: &str) -> Option<String> {
    let minds = wearable(world, roster, body);
    if minds.is_empty() {
        return None;
    }
    let lines: Vec<String> = minds.iter().map(Mind::line).collect();
    Some(format!(
        "Minds you may wear — know-how, loaded only when worn, with the `wear` tool; a worn Mind \
         stays in your context:\n{}",
        lines.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::{load, manifest, wearable};

    fn world(name: &str) -> std::path::PathBuf {
        let w = std::env::temp_dir().join(format!("mnemorph-minds-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        std::fs::create_dir_all(w.join(".mnemorph/minds/dialects")).unwrap();
        std::fs::write(
            w.join(".mnemorph/minds/dialects/README.md"),
            "# Dialects\n\n- **Shared:** yes\n\nHow a dialect overrides the generator.\n\n## Steps\n1. read it\n",
        )
        .unwrap();
        std::fs::write(
            w.join(".mnemorph/minds/release.md"),
            "# Release\n\nCutting a release by hand.\n",
        )
        .unwrap();
        std::fs::write(w.join(".mnemorph/minds/README.md"), "# Minds index\n").unwrap();
        w
    }

    #[test]
    fn a_mind_is_priced_by_exactly_what_wearing_injects() {
        let w = world("price");
        let minds = load(&w);
        assert_eq!(
            minds.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            ["dialects", "release"]
        );
        let d = &minds[0];
        assert!(d.shared);
        assert_eq!(d.about, "How a dialect overrides the generator.");
        assert_eq!(
            d.price(),
            u64::try_from(d.injected().len().div_ceil(4)).unwrap()
        );
        assert!(
            d.injected().ends_with(&d.text),
            "the whole text is what is worn"
        );
        assert_eq!(
            d.line(),
            format!(
                "@T mind:dialects — How a dialect overrides the generator. (~{} tokens)",
                d.price()
            )
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn a_body_may_wear_its_own_minds_and_the_shared_ones() {
        let w = world("wear");
        let roster = crate::Roster::load(&w);
        let names = |body: &str| -> Vec<String> {
            wearable(&w, &roster, body)
                .into_iter()
                .map(|m| m.name)
                .collect()
        };
        assert_eq!(
            names("rimuru"),
            ["dialects"],
            "a shared Mind only: release is worn by nobody"
        );
        assert!(
            manifest(&w, &roster, "rimuru")
                .unwrap()
                .contains("@T mind:dialects")
        );
        let bare = std::env::temp_dir().join(format!("mnemorph-minds-none-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&bare);
        assert_eq!(manifest(&bare, &crate::Roster::load(&bare), "rimuru"), None);
        let _ = std::fs::remove_dir_all(&w);
        let _ = std::fs::remove_dir_all(&bare);
    }
}
