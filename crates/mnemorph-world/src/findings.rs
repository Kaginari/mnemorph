//! The shapes the law demands, checked: every breach is a finding with a code and a severity,
//! never summed into a score.

use std::collections::{BTreeMap, BTreeSet};

use mnemorph_races::Race;

use crate::doc::covers;
use crate::own::{Owner, Rule, explain};
use crate::roster::Roster;

/// How much a finding weighs. Critical and high block a landing they touch; medium and low are
/// reported, never blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Reported.
    Low,
    /// Reported.
    Medium,
    /// Blocks a landing it touches.
    High,
    /// Blocks a landing it touches.
    Critical,
}

impl Severity {
    /// The severity, as findings print it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    /// Whether a finding of this severity blocks a landing it touches.
    #[must_use]
    pub fn blocks(self) -> bool {
        self >= Severity::High
    }
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Its code: `overlap`, `above-cycle`, ….
    pub code: &'static str,
    /// Its severity.
    pub severity: Severity,
    /// Where it stands: `file:line`, or a directory.
    pub at: String,
    /// The paths it is about: a landing under one of them is touched by it.
    pub paths: Vec<String>,
    /// The creatures it is about: a landing on soil one of them owns is touched by it.
    pub creatures: Vec<String>,
    /// What it says.
    pub message: String,
}

/// Every finding about the world's shape: its creatures from `roster`, its tracked files from
/// `tracked` (relative to the world). Most severe first.
#[must_use]
pub fn check(roster: &Roster, tracked: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    overlaps(roster, &mut out);
    cycles(roster, &mut out);
    bonds(roster, &mut out);
    heirs(roster, &mut out);
    unowned(roster, tracked, &mut out);
    out.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.at.cmp(&b.at))
            .then_with(|| a.code.cmp(b.code))
    });
    out
}

/// `overlap`: two creatures' write grounds — Slime zones, Kijin posts — equal or nested. An
/// Orc's domain routes and an Elf's lane plans; they nest the zones below them by design.
fn overlaps(roster: &Roster, out: &mut Vec<Finding>) {
    let grounds: Vec<(&str, &str, &String)> = roster
        .living()
        .flat_map(|c| {
            let (field, claims) = match c.race.base() {
                Race::Slime => ("Territory", &c.zone),
                Race::Kijin => ("Post", &c.post),
                _ => ("", &c.zone),
            };
            claims.iter().map(move |p| (c.name.as_str(), field, p))
        })
        .filter(|(_, field, _)| !field.is_empty())
        .collect();
    for (i, (a, field, p)) in grounds.iter().enumerate() {
        for (b, _, q) in &grounds[i + 1..] {
            if a == b || !(covers(p, q) || covers(q, p)) {
                continue;
            }
            let at = roster.creatures[*a].at(field);
            out.push(Finding {
                code: "overlap",
                severity: Severity::Critical,
                at,
                paths: vec![(*p).clone(), (*q).clone()],
                creatures: vec![(*a).to_owned(), (*b).to_owned()],
                message: format!("{a} claims `{p}` and {b} claims `{q}`: one ground, two writers"),
            });
        }
    }
}

/// `above-cycle`: the chain of command loops — one finding per loop.
fn cycles(roster: &Roster, out: &mut Vec<Finding>) {
    let mut seen: BTreeSet<Vec<String>> = BTreeSet::new();
    for start in roster.loops() {
        let mut members = vec![start.clone()];
        let mut at = roster.creatures[&start].reports_to.clone();
        while let Some(next) = at.filter(|n| *n != start) {
            members.push(next.clone());
            at = roster
                .creatures
                .get(&next)
                .and_then(|c| c.reports_to.clone());
        }
        // A loop through a retired creature is no loop: its bonds are gone (a `dead-bond`).
        if members
            .iter()
            .any(|m| roster.creatures.get(m).is_none_or(|c| c.retired))
        {
            continue;
        }
        let mut key = members.clone();
        key.sort();
        if !seen.insert(key.clone()) {
            continue;
        }
        out.push(Finding {
            code: "above-cycle",
            severity: Severity::Critical,
            at: roster.creatures[&key[0]].at("Reports to"),
            paths: Vec::new(),
            creatures: key,
            message: format!(
                "the chain of command loops: {} → {start}",
                members.join(" → ")
            ),
        });
    }
}

/// `dead-bond`: a creature reports to a name with no doc. `slime-truth`: a Slime that does not
/// report to exactly one Orc.
fn bonds(roster: &Roster, out: &mut Vec<Finding>) {
    for c in roster.living() {
        let parent = c.reports_to.as_ref().map(|p| (p, roster.creatures.get(p)));
        // A retired parent holds no bond: the graph drops it, so the report reaches nobody.
        let dead = match parent {
            Some((name, None)) => Some(format!("{} reports to `{name}`, which has no doc", c.name)),
            Some((name, Some(p))) if p.retired => {
                Some(format!("{} reports to `{name}`, which is retired", c.name))
            }
            _ => None,
        };
        if let Some(message) = dead {
            out.push(Finding {
                code: "dead-bond",
                severity: Severity::High,
                at: c.at("Reports to"),
                paths: Vec::new(),
                creatures: vec![c.name.clone()],
                message,
            });
        }
        let to_an_orc =
            matches!(parent, Some((_, Some(p))) if p.race.base() == Race::Orc && !p.retired);
        if c.race.base() == Race::Slime && !to_an_orc {
            out.push(Finding {
                code: "slime-truth",
                severity: Severity::High,
                at: c.at("Reports to"),
                paths: c.zone.clone(),
                creatures: vec![c.name.clone()],
                message: format!("{} reports its truth to no living Orc", c.name),
            });
        }
    }
}

/// `heir-dead`: a retired creature names an heir that is missing, or retired with no living line
/// below it — an heir relieved in turn lives on in its own heirs. Its ground has no living keeper.
fn heirs(roster: &Roster, out: &mut Vec<Finding>) {
    fn lives(roster: &Roster, name: &str, seen: &mut BTreeSet<String>) -> bool {
        if !seen.insert(name.to_owned()) {
            return false;
        }
        roster
            .creatures
            .get(name)
            .is_some_and(|h| !h.retired || h.heirs.iter().any(|g| lives(roster, g, seen)))
    }
    for c in roster.creatures.values().filter(|c| c.retired) {
        for heir in &c.heirs {
            let mut seen = BTreeSet::from([c.name.clone()]);
            if lives(roster, heir, &mut seen) {
                continue;
            }
            let why = if roster.creatures.contains_key(heir) {
                "is retired, and no heir of its lives"
            } else {
                "has no doc"
            };
            out.push(Finding {
                code: "heir-dead",
                severity: Severity::High,
                at: c.at("Heirs"),
                paths: c
                    .zone
                    .iter()
                    .chain(&c.domain)
                    .chain(&c.post)
                    .cloned()
                    .collect(),
                creatures: vec![c.name.clone()],
                message: format!(
                    "{}'s heir `{heir}` {why}: its ground has no living keeper",
                    c.name
                ),
            });
        }
    }
}

/// `unowned-soil`: tracked files `own` gives nobody — no claim, or a compiled soil or scratch
/// whose keeper does not live — one finding per directory.
fn unowned(roster: &Roster, tracked: &[String], out: &mut Vec<Finding>) {
    let mut by_dir: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in tracked {
        let o = explain(roster, path);
        if o.owner == Owner::Unowned && o.rule != Rule::Outside {
            let dir = path
                .rsplit_once('/')
                .map_or_else(String::new, |(d, _)| format!("{d}/"));
            by_dir.entry(dir).or_default().push(path.clone());
        }
    }
    for (dir, files) in by_dir {
        out.push(Finding {
            code: "unowned-soil",
            severity: Severity::Medium,
            at: if dir.is_empty() { "./".into() } else { dir },
            message: format!(
                "{} tracked file(s) here no creature owns: Rimuru routes their birth",
                files.len()
            ),
            paths: files,
            creatures: Vec::new(),
        });
    }
}

/// The facts' findings: `contested-fact` (medium), a fact read stale because its evidence moved
/// (`stale-fact`, low), a free fact that landed beside similar ones with no label from the
/// parent (`untriaged-fact`, low).
#[must_use]
pub fn fact_findings(facts: &crate::facts::Facts, world: &std::path::Path) -> Vec<Finding> {
    use crate::facts::Status;
    let mut out = Vec::new();
    for f in facts.live() {
        let mut push = |code, severity, message: String| {
            out.push(Finding {
                code,
                severity,
                at: f.about.clone(),
                paths: vec![f.about.clone()],
                creatures: f.supporters.iter().chain(&f.counters).cloned().collect(),
                message,
            });
        };
        if f.status() == Status::Contested {
            push(
                "contested-fact",
                Severity::Medium,
                format!(
                    "#{} `{}`: {} for, {} against — a check settles it, never recency",
                    f.id,
                    f.text,
                    f.supporters.len(),
                    f.counters.len()
                ),
            );
        }
        if f.stale(world) {
            push(
                "stale-fact",
                Severity::Low,
                format!(
                    "#{} `{}`: its evidence moved since it was said",
                    f.id, f.text
                ),
            );
        }
        if f.keyed.is_none()
            && !f.labelled
            && !facts.candidates(f, crate::facts::THETA_T).is_empty()
        {
            push(
                "untriaged-fact",
                Severity::Low,
                format!(
                    "#{} `{}` landed beside similar facts and the parent has not labelled it",
                    f.id, f.text
                ),
            );
        }
    }
    out
}

/// `prose-claim`: a Slime's doc names, in its prose, a path of the world its Territory does not
/// claim. Prose is never a claim, so the field and the prose disagree: one low finding per path, at
/// the line that names it. A mention is a backticked word read as a claim would be — with a `/`,
/// relative, no `..` — that exists in the world, on soil a Slime could claim (a zone, a domain,
/// or no one's), a directory read as one with or without its slash.
#[must_use]
pub fn prose_claims(roster: &Roster, world: &std::path::Path) -> Vec<Finding> {
    let mut out = Vec::new();
    for c in roster.living().filter(|c| c.race.base() == Race::Slime) {
        let Ok(text) = std::fs::read_to_string(world.join(c.doc())) else {
            continue;
        };
        let fields: BTreeSet<usize> = c.lines.values().copied().collect();
        let mut seen = BTreeSet::new();
        for (n, line) in text.lines().enumerate() {
            if fields.contains(&(n + 1)) || line.starts_with("# ") {
                continue;
            }
            for word in line.split('`').skip(1).step_by(2) {
                if word.contains(char::is_whitespace) {
                    continue;
                }
                let Some(mut path) = crate::doc::claim(word) else {
                    continue;
                };
                let on_disk = world.join(path.trim_end_matches('/'));
                if !on_disk.exists() {
                    continue;
                }
                // A directory named without its slash is still the directory.
                if on_disk.is_dir() && !path.ends_with('/') {
                    path.push('/');
                }
                // Only soil a Slime could claim: the workspace, the throne's, the harness's and
                // the human's outrank any zone, so naming them asks nothing of the field.
                let claimable = matches!(
                    explain(roster, &path).rule,
                    Rule::Zone | Rule::Domain | Rule::Unclaimed
                );
                if !claimable
                    || c.zone.iter().any(|z| covers(z, &path))
                    || !seen.insert(path.clone())
                {
                    continue;
                }
                out.push(Finding {
                    code: "prose-claim",
                    severity: Severity::Low,
                    at: format!("{}:{}", c.doc(), n + 1),
                    paths: vec![path.clone()],
                    creatures: vec![c.name.clone()],
                    message: format!(
                        "{}'s prose names `{path}`, which its Territory does not claim: prose is \
never a claim — claim it in the field, or say whose it is",
                        c.name
                    ),
                });
            }
        }
    }
    out
}

/// `orphan-mind`: a Mind under `.mnemorph/minds/` — a directory with a `README.md`, or a
/// `<name>.md` file, named for the entry — that no living creature wears, and whose doc is not
/// marked `- **Shared:** yes`.
#[must_use]
pub fn mind_findings(roster: &Roster, world: &std::path::Path) -> Vec<Finding> {
    let dir = world.join(".mnemorph").join("minds");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let worn: BTreeSet<&str> = roster
        .living()
        .flat_map(|c| c.minds.iter().map(|m| m.trim_matches('`')))
        .collect();
    let mut found: Vec<Finding> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            let (name, doc) = if path.is_dir() {
                (file.clone(), path.join("README.md"))
            } else {
                (file.strip_suffix(".md")?.to_owned(), path.clone())
            };
            if name.starts_with('.') || worn.contains(name.as_str()) {
                return None;
            }
            let shared = std::fs::read_to_string(&doc).is_ok_and(|t| {
                t.lines()
                    .take_while(|l| !l.starts_with("##"))
                    .filter_map(|l| l.trim().strip_prefix("- **Shared:**"))
                    .any(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "yes" | "true"))
            });
            (!shared).then(|| Finding {
                code: "orphan-mind",
                severity: Severity::Medium,
                at: format!(".mnemorph/minds/{file}"),
                paths: Vec::new(),
                creatures: Vec::new(),
                message: format!(
                    "the Mind `{name}` is worn by no living creature and not marked shared: give \
it to one in its Minds field, mark it `- **Shared:** yes`, or retire it"
                ),
            })
        })
        .collect();
    found.sort_by(|a, b| a.at.cmp(&b.at));
    found
}
