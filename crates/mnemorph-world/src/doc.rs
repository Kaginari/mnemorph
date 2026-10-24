//! A creature's document: its structured fields, read as data. Prose is never a claim.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mnemorph_races::Race;

/// A creature, as its doc says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creature {
    /// Its name: the doc's heading (`slime-wire`).
    pub name: String,
    /// Its race.
    pub race: Race,
    /// Its seat: the directory its doc lives in, relative to the world (`.mnemorph/slime/wire/`).
    pub seat: String,
    /// Who it reports to, by name.
    pub reports_to: Option<String>,
    /// A Slime's zone: the paths it writes — directories and files.
    pub zone: Vec<String>,
    /// An Orc's domain: the paths it routes and judges, never writes.
    pub domain: Vec<String>,
    /// A Kijin's post: the files and directories its outward acts own (CI, release, deploy).
    pub post: Vec<String>,
    /// An Elf's lane: the paths it plans across. It owns nothing.
    pub lane: Vec<String>,
    /// Its verify lines.
    pub verify: Vec<String>,
    /// The Minds it wears.
    pub minds: Vec<String>,
    /// Retired creatures own nothing; their seats and prefixes are skipped.
    pub retired: bool,
    /// The line each field first stands on, for a finding's `file:line`.
    pub lines: BTreeMap<String, usize>,
}

impl Creature {
    /// The doc's path, relative to the world.
    #[must_use]
    pub fn doc(&self) -> String {
        format!("{}README.md", self.seat)
    }

    /// Where a field stands: `doc:line`, or the doc alone when the field is absent.
    #[must_use]
    pub fn at(&self, field: &str) -> String {
        self.lines
            .get(field)
            .map_or_else(|| self.doc(), |n| format!("{}:{n}", self.doc()))
    }
}

/// A seat's directory as ownership reads it: relative, `/`-separated, ending in `/`, no `./`.
#[must_use]
pub fn prefix(raw: &str) -> String {
    let p = raw.trim().trim_start_matches("./").trim_matches('/');
    if p.is_empty() {
        String::new()
    } else {
        format!("{p}/")
    }
}

/// The value of `- **Field:** value`, if the line is that field.
fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    line.trim()
        .strip_prefix("- **")?
        .strip_prefix(name)?
        .strip_prefix(":**")
        .map(str::trim)
}

/// Words of a field: space- or comma-separated, backticks dropped.
fn words(value: &str) -> Vec<String> {
    value
        .split([' ', ','])
        .map(|w| w.trim_matches('`').trim())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// One claim of a ground field: a directory written with its trailing `/` (`src/`,
/// `.github/workflows/`), or a file written with a `/` in it (`./CHANGELOG.md`,
/// `packages/cli/package.json`). Inside the world, never absolute, never climbing out of it.
fn claim(word: &str) -> Option<String> {
    let path_like = word.contains('/') && !word.starts_with('/');
    if !path_like || word.chars().any(char::is_control) {
        return None;
    }
    if word.split('/').any(|s| s == "..") {
        return None;
    }
    let p = crate::own::normal(word)?;
    Some(p)
}

/// Whether `claim` covers `path`: a directory claim covers what lies under it, a file claim the
/// file alone.
#[must_use]
pub fn covers(claim: &str, path: &str) -> bool {
    if claim.ends_with('/') {
        path.starts_with(claim)
    } else {
        path == claim
    }
}

/// The claims of a ground field: a list of paths, never prose — prose is never a claim.
fn claims(value: &str, field: &str, seat: &Path) -> Result<Vec<String>, String> {
    words(value)
        .into_iter()
        .map(|w| {
            claim(&w).ok_or_else(|| {
                format!(
                    "{}: {field} is a list of paths inside the world — a directory with its \
                     trailing `/`, a file with a `/` in it (`./CHANGELOG.md`); `{w}` is not one",
                    seat.display()
                )
            })
        })
        .collect()
}

/// Read a creature's doc. `seat` is its directory, relative to the world.
///
/// # Errors
///
/// The doc has no `# name` heading, or no `Race` field, or names a race that does not exist.
pub fn parse(text: &str, seat: &Path) -> Result<Creature, String> {
    let name = text
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| format!("{}: the doc has no `# name` heading", seat.display()))?;
    let mut race = None;
    let mut c = Creature {
        name,
        race: Race::Slime,
        seat: prefix(&seat.to_string_lossy()),
        reports_to: None,
        zone: Vec::new(),
        domain: Vec::new(),
        post: Vec::new(),
        lane: Vec::new(),
        verify: Vec::new(),
        minds: Vec::new(),
        retired: false,
        lines: BTreeMap::new(),
    };
    let mut territory = Vec::new();
    // Only the header's fields count: a field-looking line under a later heading is prose.
    for (n, line) in text
        .lines()
        .enumerate()
        .take_while(|(_, l)| !l.starts_with("##"))
    {
        for name in [
            "Race",
            "Reports to",
            "Territory",
            "Domain",
            "Post",
            "Verify",
            "Minds",
            "Retired",
        ] {
            if field(line, name).is_some() {
                c.lines.entry(name.to_owned()).or_insert(n + 1);
            }
        }
        if let Some(v) = field(line, "Race") {
            race = Some(
                Race::from_name(v.trim_matches('`'))
                    .ok_or_else(|| format!("{}: no race `{v}`", seat.display()))?,
            );
        } else if let Some(v) = field(line, "Reports to") {
            c.reports_to = Some(v.trim_matches('`').to_owned()).filter(|v| !v.is_empty());
        } else if let Some(v) = field(line, "Territory") {
            territory.extend(claims(v, "Territory", seat)?);
        } else if let Some(v) = field(line, "Domain") {
            c.domain.extend(claims(v, "Domain", seat)?);
        } else if let Some(v) = field(line, "Post") {
            c.post.extend(claims(v, "Post", seat)?);
        } else if let Some(v) = field(line, "Verify") {
            c.verify.push(v.trim_matches('`').to_owned());
        } else if let Some(v) = field(line, "Minds") {
            c.minds.extend(words(v));
        } else if let Some(v) = field(line, "Retired") {
            c.retired = matches!(v.to_ascii_lowercase().as_str(), "yes" | "true");
        }
    }
    c.race = race.ok_or_else(|| format!("{}: the doc has no `Race` field", seat.display()))?;
    // Territory is the zone a Slime writes; for an Elf it is only the lane it plans across.
    let territory = territory.into_iter();
    match c.race.base() {
        Race::Slime => c.zone.extend(territory),
        Race::Elf => c.lane.extend(territory),
        _ => {
            let rest: Vec<String> = territory.collect();
            if !rest.is_empty() {
                return Err(format!(
                    "{}: a {} names its ground as {}, not Territory",
                    seat.display(),
                    c.race.name(),
                    match c.race.base() {
                        Race::Orc => "Domain",
                        Race::Kijin => "Post",
                        _ => "nothing: it owns no ground",
                    }
                ));
            }
        }
    }
    Ok(c)
}

/// Where a creature's seat directory is, for a doc found at `doc` (`…/README.md`).
#[must_use]
pub fn seat_of(doc: &Path) -> PathBuf {
    doc.parent().map(Path::to_path_buf).unwrap_or_default()
}
