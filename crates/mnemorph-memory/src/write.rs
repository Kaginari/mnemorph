//! The write rule: a body's words land personal and proposed, and only a route's proof grounds
//! them. That is a type, not a habit — a `Claim<Proposed>` becomes a `Claim<Grounded>` only
//! through a [`Proof`], and every proof is built from the evidence its route needs, refused
//! without it. A body cannot ground its own words.

use std::marker::PhantomData;
use std::path::Path;

use crate::note::{By, Change, Dependency, Note, Polarity, Status};
use crate::store::{Tier, append, event_id, now_ms};

/// A claim said, not proved.
#[derive(Debug)]
pub struct Proposed;

/// A claim a route proved.
#[derive(Debug)]
pub struct Grounded;

/// A body's claim, in the standing `S`.
#[derive(Debug)]
pub struct Claim<S> {
    /// The note it is.
    pub note: String,
    /// The creature that said it.
    pub owner: String,
    /// What it says.
    pub text: String,
    /// Its tags.
    pub tags: Vec<String>,
    /// What it points at.
    pub source: Option<String>,
    /// What it rests on: the files its run read or changed.
    pub evidence: Vec<String>,
    /// What it depends on: claims, facts, files with their hashes.
    pub depends_on: Vec<Dependency>,
    standing: PhantomData<S>,
}

/// The four routes from personal to shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The end gate passed and ran a verify line over a non-empty diff.
    Checked,
    /// The commissioner's verdict named the claim and accepted it.
    Accepted,
    /// A second creature landed a matching claim from different anchors.
    Corroborated,
    /// The human said so.
    Confirmed,
}

impl Route {
    /// Its word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Route::Checked => "checked",
            Route::Accepted => "accepted",
            Route::Corroborated => "corroborated",
            Route::Confirmed => "confirmed",
        }
    }

    /// The confidence a note grounded by it carries: a check or the human's word more than a
    /// verdict or a second voice.
    #[must_use]
    pub fn confidence(self) -> f64 {
        match self {
            Route::Checked | Route::Confirmed => 0.7,
            Route::Accepted | Route::Corroborated => 0.6,
        }
    }
}

/// A route's proof: built only from the evidence the route needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    route: Route,
    cites: String,
    /// What a **checked** proof observed: the specific terms of the paths its diff changed. A
    /// claim naming none of them is about the world beyond the diff, which a green gate passes as
    /// easily wrong as right. `None` for the routes that observe no diff.
    scope: Option<std::collections::BTreeSet<String>>,
}

impl Proof {
    /// **checked**: the end gate passed, citing its verdict, and ran `verify_lines` over a diff of
    /// `changed` paths. An answer that changed nothing, or a gate with no verify line, proves
    /// nothing of its claims.
    ///
    /// # Errors
    ///
    /// No verify line ran, or the diff was empty.
    pub fn checked(verdict: &str, verify_lines: usize, changed: &[String]) -> Result<Self, String> {
        if verify_lines == 0 {
            return Err("no verify line ran: the gate proved nothing of the claims".into());
        }
        if changed.is_empty() {
            return Err("the answer changed nothing: a pass over no diff proves no claim".into());
        }
        Ok(Self {
            route: Route::Checked,
            cites: format!(
                "{verdict} ({verify_lines} verify line(s) over {} path(s))",
                changed.len()
            ),
            scope: Some(
                changed
                    .iter()
                    .flat_map(|p| {
                        // The path, and each directory above it: `src/parser/mod.rs` is in
                        // `src/parser`.
                        let dirs = p
                            .match_indices('/')
                            .map(|(i, _)| p[..i].to_lowercase())
                            .filter(|d| d.contains('/'))
                            .collect::<Vec<_>>();
                        crate::text::specific_terms(p).into_iter().chain(dirs)
                    })
                    .collect(),
            ),
        })
    }

    /// Whether this proof observes a claim saying `text`, pointing at `source`: always for a route
    /// that observes no diff; for **checked**, only when the claim names a path its diff changed —
    /// a claim about the world beyond the diff needs another route.
    #[must_use]
    pub fn covers(&self, text: &str, source: Option<&str>) -> bool {
        let Some(scope) = &self.scope else {
            return true;
        };
        let mut named = crate::text::specific_terms(text);
        if let Some(s) = source {
            named.extend(crate::text::specific_terms(s));
        }
        named.iter().any(|t| scope.contains(t))
    }

    /// **accepted**: the commissioner's verdict, cited, names the claim and accepts it.
    ///
    /// # Errors
    ///
    /// The verdict does not name the claim.
    pub fn accepted(verdict: &str, names_it: bool) -> Result<Self, String> {
        if !names_it {
            return Err("the verdict does not name the claim: nothing was accepted".into());
        }
        Ok(Self {
            route: Route::Accepted,
            cites: verdict.to_owned(),
            scope: None,
        })
    }

    /// **corroborated**: `second`, another creature than the claim's `owner`, landed a matching
    /// claim citing `theirs`, none of which the claim's own anchors `ours` cite — the same note
    /// recalled twice is no second voice.
    ///
    /// # Errors
    ///
    /// The second is the owner, either side cites no anchor, or the anchors overlap.
    pub fn corroborated(
        owner: &str,
        second: &str,
        ours: &[String],
        theirs: &[String],
    ) -> Result<Self, String> {
        if second == owner {
            return Err("a creature does not corroborate itself".into());
        }
        if ours.is_empty() || theirs.is_empty() {
            return Err("a corroboration needs anchors on both sides".into());
        }
        if ours.iter().any(|a| theirs.contains(a)) {
            return Err("the two claims share an anchor: one piece of evidence, not two".into());
        }
        Ok(Self {
            route: Route::Corroborated,
            cites: format!("{second}: {}", theirs.join(", ")),
            scope: None,
        })
    }

    /// **confirmed**: the human said so, citing their answer.
    #[must_use]
    pub fn confirmed(answer: &str) -> Self {
        Self {
            route: Route::Confirmed,
            cites: answer.to_owned(),
            scope: None,
        }
    }

    /// Its route.
    #[must_use]
    pub fn route(&self) -> Route {
        self.route
    }
}

/// A body's claim, landed: personal, proposed, salience 0.6, confidence 0.5.
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), or the note file cannot be written.
pub fn propose(
    world: &Path,
    owner: &str,
    text: &str,
    tags: Vec<String>,
    source: Option<String>,
) -> std::io::Result<Claim<Proposed>> {
    propose_on(world, owner, text, tags, source, Vec::new())
}

/// [`propose`], with what the claim rests on: the files its run read or changed, which a second
/// voice must not share to corroborate it.
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), or the note file cannot be written.
pub fn propose_on(
    world: &Path,
    owner: &str,
    text: &str,
    tags: Vec<String>,
    source: Option<String>,
    evidence: Vec<String>,
) -> std::io::Result<Claim<Proposed>> {
    propose_resting(world, owner, text, tags, source, evidence, Vec::new())
}

/// [`propose_on`], with what the claim depends on besides its evidence's files: other claims by
/// id, facts of the ontology. Each evidence file is recorded with its content hash now, so a
/// landing that changes it marks the claim for review.
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), or the note file cannot be written.
pub fn propose_resting(
    world: &Path,
    owner: &str,
    text: &str,
    tags: Vec<String>,
    source: Option<String>,
    evidence: Vec<String>,
    depends_on: Vec<Dependency>,
) -> std::io::Result<Claim<Proposed>> {
    crate::audit::unfrozen(world, owner)?;
    let mut depends_on = depends_on;
    for f in crate::graph::files(world, &evidence) {
        if !depends_on.contains(&f) {
            depends_on.push(f);
        }
    }
    let note = format!("n{}", event_id(now_ms()));
    append(
        world,
        &Tier::Personal(owner.to_owned()),
        &note,
        By::Record,
        None,
        Change::Create {
            text: text.to_owned(),
            tags: tags.clone(),
            owner: Some(owner.to_owned()),
            polarity: Polarity::Support,
            salience: 0.6,
            confidence: 0.5,
            source: source.clone(),
            links: Vec::new(),
            evidence: evidence.clone(),
            depends_on: depends_on.clone(),
        },
    )?;
    Ok(Claim {
        note,
        owner: owner.to_owned(),
        text: text.to_owned(),
        tags,
        source,
        evidence,
        depends_on,
        standing: PhantomData,
    })
}

/// Ground a claim by its route's proof: in its owner's personal tier, and in shared memory, each
/// event citing the proof, at the route's confidence. The files its evidence names are recorded
/// again with their hashes now — what the grounding saw — and a review it waited under is answered.
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), or a note file cannot be written.
pub fn ground(
    world: &Path,
    claim: Claim<Proposed>,
    proof: &Proof,
) -> std::io::Result<Claim<Grounded>> {
    crate::audit::unfrozen(world, &claim.owner)?;
    if !proof.covers(&claim.text, claim.source.as_deref()) {
        return Err(std::io::Error::other(
            "the claim names nothing the checked diff changed: it is about the world beyond the \
             diff, and needs accepted, corroborated or confirmed",
        ));
    }
    let cites = format!("{}: {}", proof.route.word(), proof.cites);
    let confidence = proof.route.confidence();
    let personal = Tier::Personal(claim.owner.clone());
    let seen = crate::graph::files(world, &claim.evidence);
    let mut depends_on = claim.depends_on.clone();
    depends_on.retain(|d| !matches!(d, Dependency::File { .. }));
    depends_on.extend(seen.iter().cloned());
    if !seen.is_empty() {
        append(
            world,
            &personal,
            &claim.note,
            By::Route,
            Some(cites.clone()),
            Change::Depends { on: seen.clone() },
        )?;
    }
    append(
        world,
        &personal,
        &claim.note,
        By::Route,
        Some(cites.clone()),
        Change::Status {
            status: Status::Grounded,
        },
    )?;
    append(
        world,
        &personal,
        &claim.note,
        By::Route,
        Some(cites.clone()),
        Change::Confidence { confidence },
    )?;
    append(
        world,
        &Tier::Shared,
        &claim.note,
        By::Route,
        Some(cites.clone()),
        Change::Create {
            text: claim.text.clone(),
            tags: claim.tags.clone(),
            owner: Some(claim.owner.clone()),
            polarity: Polarity::Support,
            salience: 0.6,
            confidence,
            source: claim.source.clone(),
            links: Vec::new(),
            evidence: claim.evidence.clone(),
            depends_on: depends_on.clone(),
        },
    )?;
    // Shared memory may hold it already — a claim grounded again after a review: what this
    // grounding saw replaces what the first one did.
    if !seen.is_empty() {
        append(
            world,
            &Tier::Shared,
            &claim.note,
            By::Route,
            Some(cites.clone()),
            Change::Depends { on: seen },
        )?;
    }
    append(
        world,
        &Tier::Shared,
        &claim.note,
        By::Route,
        Some(cites),
        Change::Status {
            status: Status::Grounded,
        },
    )?;
    Ok(Claim {
        note: claim.note,
        owner: claim.owner,
        text: claim.text,
        tags: claim.tags,
        source: claim.source,
        evidence: claim.evidence,
        depends_on,
        standing: PhantomData,
    })
}

/// Reconsolidate a run-ending failure: one revise note per failed rule, grounded at once — the
/// failure is an observed fact — salience 1.0, confidence 0.9, written by the gate citing its
/// verdict. It links every anchor recalled into the failed run, 0.9 towards one the act cited, 0.4
/// towards one merely recalled, and never a dissenting note: corrections do not breed corrections.
/// It lands in shared memory when any note it links is shared, else in `owner`'s personal tier.
/// Returns the revise notes' ids; nothing when no anchor was recalled.
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), or a note file cannot be written.
pub fn revise(
    world: &Path,
    owner: &str,
    acts: &[String],
    recalled: &[Anchor],
    verdict: &str,
) -> std::io::Result<Vec<String>> {
    let targets: Vec<&Anchor> = recalled.iter().filter(|r| !r.dissent).collect();
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    crate::audit::unfrozen(world, owner)?;
    let tier = if targets.iter().any(|r| !r.personal) {
        Tier::Shared
    } else {
        Tier::Personal(owner.to_owned())
    };
    // One note per failed rule — `right author`, `traits hold`, … — not per line: a landing that
    // touched eight foreign files failed one rule. Each line is kept by its first line, cut: the
    // note says what failed, never a test's whole tail.
    let mut rules: Vec<(String, Vec<String>)> = Vec::new();
    for act in acts {
        let rule = act.split(':').next().unwrap_or(act).trim().to_owned();
        let first: String = act
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(160)
            .collect();
        match rules.iter_mut().find(|(r, _)| *r == rule) {
            Some((_, lines)) => lines.push(first),
            None => rules.push((rule, vec![first])),
        }
    }
    let mut made = Vec::new();
    for (_, lines) in rules {
        let mut act: String = lines.join("; ");
        if act.chars().count() > 600 {
            act = act.chars().take(600).collect::<String>() + "…";
        }
        let links = targets
            .iter()
            .map(|r| crate::note::Link {
                note: r.id.clone(),
                weight: if act.contains(&r.id) || r.cited {
                    0.9
                } else {
                    0.4
                },
            })
            .collect();
        let note = format!("r{}", event_id(now_ms()));
        append(
            world,
            &tier,
            &note,
            By::Gate,
            Some(verdict.to_owned()),
            Change::Create {
                text: act,
                tags: vec!["failure".into()],
                owner: Some(owner.to_owned()),
                polarity: Polarity::Revise,
                salience: 1.0,
                confidence: 0.9,
                source: Some(verdict.to_owned()),
                evidence: Vec::new(),
                links,
                depends_on: Vec::new(),
            },
        )?;
        append(
            world,
            &tier,
            &note,
            By::Gate,
            Some(verdict.to_owned()),
            Change::Status {
                status: Status::Grounded,
            },
        )?;
        made.push(note);
    }
    Ok(made)
}

/// One anchor recalled into a run, as reconsolidation weighs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// The note's id.
    pub id: String,
    /// From the asker's personal tier, not shared memory.
    pub personal: bool,
    /// A counter or a revise note: never revised.
    pub dissent: bool,
    /// The body's own text named it.
    pub cited: bool,
}

/// A body's semantic cache: a note in its machine-local short tier, proposed, dying with its
/// task. Returns the note's id.
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), or the note file cannot be written.
pub fn cache(world: &Path, body: &str, text: &str) -> std::io::Result<String> {
    crate::audit::unfrozen(world, body)?;
    let note = format!("c{}", event_id(now_ms()));
    append(
        world,
        &Tier::Short(body.to_owned()),
        &note,
        By::Record,
        None,
        Change::Create {
            text: text.to_owned(),
            tags: vec![],
            owner: Some(body.to_owned()),
            polarity: Polarity::Support,
            salience: 0.5,
            confidence: 0.5,
            source: None,
            evidence: Vec::new(),
            links: Vec::new(),
            depends_on: Vec::new(),
        },
    )?;
    Ok(note)
}

/// How many attributable failures a note must have caused before it is retired.
pub const STRIKES: usize = 2;
/// The confidence a first attributable failure takes from a note, and the floor it stops at.
pub const DOUBT: f64 = 0.1;
/// How low doubt brings a note's confidence.
pub const DOUBT_FLOOR: f64 = 0.3;

/// Whether a failure can be laid at a note's door: what failed names something the note names — a
/// path, a file, an identifier — once the failed rules' names are set aside. A slow test, a weak
/// model or an unrelated bug shares none of them, and strikes nothing.
#[must_use]
pub fn attributable(note: &str, failure: &str) -> bool {
    // A revise note reads `rule: line; rule: line`: the lines are what failed, the rules are not.
    let lines: String = failure
        .split("; ")
        .map(|part| part.split_once(": ").map_or(part, |(_, line)| line))
        .collect::<Vec<_>>()
        .join(" ");
    let said = crate::text::specific_terms(note);
    !said.is_empty()
        && crate::text::specific_terms(&lines)
            .iter()
            .any(|t| said.contains(t))
}

/// The evidence on one note since it was last grounded: the distinct failures laid at its door,
/// and the passing asks that cited it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Evidence {
    /// The verdicts of the attributable failures that cited it.
    pub strikes: std::collections::BTreeSet<String>,
    /// How many passing asks cited it.
    pub passes: usize,
}

/// The evidence on the note `id` of `rows` (its own tier), with `revisions` the grounded revise
/// notes that may strike it.
fn evidence(id: &str, rows: &[crate::note::Event], note: &Note, revisions: &[Note]) -> Evidence {
    let since = rows
        .iter()
        .filter(|e| {
            e.note == id
                && matches!(
                    e.change,
                    Change::Status {
                        status: Status::Grounded
                    }
                )
        })
        .map(|e| e.time)
        .max()
        .unwrap_or(0);
    let strikes = revisions
        .iter()
        .filter(|r| {
            r.born >= since
                && r.links.iter().any(|l| l.note == id && l.weight >= 0.9)
                && attributable(&note.text, &r.text)
        })
        .filter_map(|r| r.source.clone())
        .collect();
    let passes = rows
        .iter()
        .filter(|e| {
            e.note == id
                && e.time >= since
                && matches!(e.by, By::Gate)
                && matches!(e.change, Change::Confidence { .. })
                && e.proof
                    .as_deref()
                    .is_some_and(|p| p.starts_with("passed: "))
        })
        .count();
    Evidence { strikes, passes }
}

/// What a passing ask says of the notes it cited: each gains a pass, citing the verdict, and every
/// third pass since its last grounding raises its confidence by 0.05, to at most 0.95. Returns how
/// many notes were credited.
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), or a note file cannot be read or
/// written.
pub fn confirm(
    world: &Path,
    owner: &str,
    recalled: &[Anchor],
    verdict: &str,
) -> std::io::Result<usize> {
    let cited: Vec<&Anchor> = recalled.iter().filter(|a| a.cited && !a.dissent).collect();
    if cited.is_empty() {
        return Ok(0);
    }
    crate::audit::unfrozen(world, owner)?;
    let mut credited = 0;
    let mut ids = Vec::new();
    for a in cited {
        let tier = if a.personal {
            Tier::Personal(owner.to_owned())
        } else {
            Tier::Shared
        };
        let rows = crate::store::read(world, &tier)?.rows;
        let notes = crate::note::fold(&rows);
        // A proposed note may be struck, so it may be credited: evidence both ways.
        let Some(n) = notes
            .get(&a.id)
            .filter(|n| matches!(n.status, Status::Grounded | Status::Proposed))
        else {
            continue;
        };
        let passes = evidence(&a.id, &rows, n, &[]).passes + 1;
        let confidence = if passes.is_multiple_of(3) {
            (n.confidence + 0.05).min(0.95)
        } else {
            n.confidence
        };
        // The same note in either tier: the creature's copy and the shared one move together.
        for t in [Tier::Personal(owner.to_owned()), Tier::Shared] {
            let held = t == tier
                || crate::store::read(world, &t)?
                    .rows
                    .iter()
                    .any(|e| e.note == a.id);
            if held {
                append(
                    world,
                    &t,
                    &a.id,
                    By::Gate,
                    Some(format!("passed: {verdict}")),
                    Change::Confidence { confidence },
                )?;
            }
        }
        credited += 1;
        ids.push(a.id.clone());
    }
    // A pass over bytes the claim no longer describes vouches for nothing: marked for review.
    crate::graph::credited(world, &ids, verdict)?;
    Ok(credited)
}

/// What [`retire`] did to the notes the failing ask cited.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Retired {
    /// Retired: dormant, kept, no longer recalled.
    pub retired: Vec<String>,
    /// Doubted: a first attributable failure took [`DOUBT`] of their confidence.
    pub doubted: Vec<String>,
    /// Marked for review: the claims resting on a note retired or doubted, transitively.
    pub reviewed: Vec<String>,
}

/// Weigh what a run-ending failure says of the notes its ask cited. A failure counts against a
/// note only when it is [`attributable`] to it; then a first such failure doubts the note, and it
/// is retired — dormant, its events kept, no longer recalled — once [`STRIKES`] distinct
/// attributable failures outnumber the passing asks that cited it, all since it was last grounded.
/// The human wakes a retired note with [`wake`], and only evidence after that counts. A shared note
/// whose owner's memory is frozen is left for the human. Run after [`revise`].
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), or a note file cannot be read or
/// written.
pub fn retire(
    world: &Path,
    owner: &str,
    recalled: &[Anchor],
    verdict: &str,
) -> std::io::Result<Retired> {
    let cited: Vec<&Anchor> = recalled.iter().filter(|a| a.cited && !a.dissent).collect();
    let mut out = Retired::default();
    if cited.is_empty() {
        return Ok(out);
    }
    crate::audit::unfrozen(world, owner)?;
    let personal = Tier::Personal(owner.to_owned());
    let shared_rows = crate::store::read(world, &Tier::Shared)?.rows;
    let own_rows = crate::store::read(world, &personal)?.rows;
    // The revise notes that may strike, wherever `revise` put them: grounded, whoever's tier.
    let revisions: Vec<Note> = crate::note::fold(&shared_rows)
        .into_values()
        .chain(crate::note::fold(&own_rows).into_values())
        .filter(|r| r.polarity == Polarity::Revise && r.status == Status::Grounded)
        .collect();
    for a in cited {
        let rows = if a.personal { &own_rows } else { &shared_rows };
        let notes = crate::note::fold(rows);
        let Some(n) = notes.get(&a.id) else {
            continue;
        };
        if n.status != Status::Grounded && n.status != Status::Proposed {
            continue;
        }
        if let Some(o) = n.owner.as_deref()
            && o != owner
            && crate::audit::unfrozen(world, o).is_err()
        {
            continue;
        }
        let e = evidence(&a.id, rows, n, &revisions);
        if !e.strikes.contains(verdict) {
            continue;
        }
        let change = if e.strikes.len() >= STRIKES && e.strikes.len() > e.passes {
            out.retired.push(a.id.clone());
            Change::Status {
                status: Status::Dormant,
            }
        } else {
            out.doubted.push(a.id.clone());
            Change::Confidence {
                confidence: (n.confidence - DOUBT).max(DOUBT_FLOOR),
            }
        };
        // The same note in either tier: the creature's copy and the shared one move together.
        for (t, held) in [(&personal, &own_rows), (&Tier::Shared, &shared_rows)] {
            if held.iter().any(|e| e.note == a.id) {
                append(
                    world,
                    t,
                    &a.id,
                    By::Gate,
                    Some(verdict.to_owned()),
                    change.clone(),
                )?;
            }
        }
    }
    // A claim retired no longer holds up what rests on it, nor what rests on that; a first doubt —
    // one failure — marks only what rests on it directly.
    for (moved, id, how) in out
        .retired
        .iter()
        .map(|i| (crate::graph::Moved::Claim(i), i, "retired"))
        .chain(
            out.doubted
                .iter()
                .map(|i| (crate::graph::Moved::Doubted(i), i, "doubted by a failure")),
        )
    {
        for marked in crate::graph::invalidate(
            world,
            moved,
            &format!("`{id}` it rests on was {how} ({verdict})"),
        )? {
            if !out.reviewed.contains(&marked) {
                out.reviewed.push(marked);
            }
        }
    }
    Ok(out)
}

/// The human wakes the note `id` the gate retired: a grounded status, theirs, in every tier that
/// holds it — the creatures' and shared memory. Evidence before it no longer counts. Only a
/// retirement is undone: a note put to sleep otherwise, or never grounded, keeps its standing
/// and its route. Returns the files written.
///
/// # Errors
///
/// No tier holds the note as one the gate retired, or a note file cannot be read or written.
pub fn wake(world: &Path, id: &str) -> std::io::Result<Vec<String>> {
    let text = retired_note(world, id)?.unwrap_or_default();
    wake_by(world, id, "woken by the human", &seen(id, &text))
}

/// The text of the note `id` as the gate retired it, from the first tier that holds it retired —
/// shared memory, then each creature's; `None` when none does.
///
/// # Errors
///
/// A note file cannot be read.
pub fn retired_note(world: &Path, id: &str) -> std::io::Result<Option<String>> {
    for tier in tiers_of(world) {
        let rows = crate::store::read(world, &tier)?.rows;
        if gate_retired(&rows, id) {
            return Ok(crate::note::fold(&rows).get(id).map(|n| n.text.clone()));
        }
    }
    Ok(None)
}

/// Shared memory and every creature's tier, as `wake` walks them.
fn tiers_of(world: &Path) -> Vec<Tier> {
    let mut tiers = vec![Tier::Shared];
    let mut names: Vec<String> = std::fs::read_dir(world.join(".mnemorph/memory/creature"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    tiers.extend(names.into_iter().map(Tier::Personal));
    tiers
}

/// The digest a human act names: the SHA-256 of a note's or a fact's id and full text, in hex.
/// The page or the command shows the text; the act carries this; the function that writes reads
/// the text afresh and refuses when it differs, so what was read is what is vouched for.
#[must_use]
pub fn seen(id: &str, text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(id.as_bytes());
    h.update([0]);
    h.update(text.as_bytes());
    h.finalize()
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Why an act on `id` is refused: it changed since the human read it.
#[must_use]
pub fn changed(id: &str) -> String {
    format!("`{id}` changed since it was read: read it again before you act on it")
}

/// As [`wake`], the event citing `said` — `woken by the human` from the CLI, `woken by the human
/// (board)` from the board — and only while the note says what the human read: `seen_as` is its
/// [`seen`] digest, checked against every tier that holds it retired, read afresh right before
/// the first write. A note rewritten since is refused, and nothing is written.
///
/// # Errors
///
/// As [`wake`], or the note changed since it was read.
pub fn wake_by(world: &Path, id: &str, said: &str, seen_as: &str) -> std::io::Result<Vec<String>> {
    let tiers = tiers_of(world);
    let mut retired = Vec::new();
    for tier in tiers {
        let rows = crate::store::read(world, &tier)?.rows;
        if gate_retired(&rows, id) {
            let text = crate::note::fold(&rows)
                .get(id)
                .map(|n| n.text.clone())
                .unwrap_or_default();
            if seen(id, &text) != seen_as {
                return Err(std::io::Error::other(changed(id)));
            }
            retired.push(tier);
        }
    }
    let mut woke = Vec::new();
    for tier in retired {
        append(
            world,
            &tier,
            id,
            By::Human,
            Some(said.to_owned()),
            Change::Status {
                status: Status::Grounded,
            },
        )?;
        woke.push(tier.file().display().to_string());
    }
    if woke.is_empty() {
        return Err(std::io::Error::other(format!(
            "no note `{id}` the gate retired in this world's memory: only a retirement is woken"
        )));
    }
    Ok(woke)
}

/// Whether `rows` hold the note `id` retired by the gate: dormant, its last status the gate's.
fn gate_retired(rows: &[crate::note::Event], id: &str) -> bool {
    rows.iter()
        .filter(|e| e.note == id && matches!(e.change, Change::Status { .. }))
        .max_by_key(|e| (e.time, e.id.clone()))
        .is_some_and(|e| {
            matches!(e.by, By::Gate)
                && matches!(
                    e.change,
                    Change::Status {
                        status: Status::Dormant
                    }
                )
        })
}

/// The notes the gate retired that touch `owner` — its own tier, and shared notes it owns — each
/// with its text: what `memory status` lists for the human to review and, if wrong, wake.
///
/// # Errors
///
/// A note file cannot be read.
pub fn retired(world: &Path, owner: &str) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for tier in [Tier::Personal(owner.to_owned()), Tier::Shared] {
        let rows = crate::store::read(world, &tier)?.rows;
        for n in crate::note::fold(&rows).into_values() {
            if n.status != Status::Dormant
                || (matches!(tier, Tier::Shared) && n.owner.as_deref() != Some(owner))
            {
                continue;
            }
            // Retired, not put to sleep; a note in both tiers listed once.
            if gate_retired(&rows, &n.id) && !out.iter().any(|(id, _)| *id == n.id) {
                out.push((n.id.clone(), n.text.clone()));
            }
        }
    }
    Ok(out)
}

/// The human vouches for the claim `id`, whichever creature holds it proposed — the
/// **confirmed** route, citing `answer` — and it is grounded and shared. Returns its owner. What
/// `mnemorphd memory confirm` and the board's confirm both do; a body never runs either. Only while
/// the claim says what the human read: `seen_as` is its [`seen`] digest, checked against the claim
/// read afresh right before the writes, so a claim rewritten since is refused, not grounded.
///
/// # Errors
///
/// No creature holds `id` as a proposed claim (or one waiting under review), it changed since it
/// was read, a creature's memory cannot be read, or the claim cannot be grounded.
pub fn vouch(world: &Path, id: &str, answer: &str, seen_as: &str) -> Result<String, String> {
    let Some(claim) = pending(world, id)? else {
        return Err(format!(
            "no creature holds `{id}` as a proposed claim: `mnemorphd memory status` lists them"
        ));
    };
    // Read afresh right before the writes: a claim rewritten since the human read it is refused.
    if seen(id, &claim.text) != seen_as {
        return Err(changed(id));
    }
    let owner = claim.owner.clone();
    ground(world, claim, &Proof::confirmed(answer))
        .map(|_| owner)
        .map_err(|e| format!("{id} could not be grounded: {e}"))
}

/// The claim `id`, whichever creature holds it proposed (or grounded and waiting under review):
/// what [`vouch`] would ground. `None` when no creature holds it so.
///
/// # Errors
///
/// A creature's memory cannot be read.
pub fn pending(world: &Path, id: &str) -> Result<Option<Claim<Proposed>>, String> {
    for tier in tiers_of(world) {
        let Tier::Personal(owner) = tier else {
            continue;
        };
        match proposed(world, &owner, id) {
            Ok(Some(claim)) => return Ok(Some(claim)),
            Ok(None) => {}
            Err(e) => return Err(format!("{owner}'s memory cannot be read: {e}")),
        }
    }
    Ok(None)
}

/// The claims `owner` holds proposed in its personal tier, newest first: what waits for a route,
/// and what the human may vouch for.
///
/// # Errors
///
/// The personal tier cannot be read.
pub fn proposals(world: &Path, owner: &str) -> std::io::Result<Vec<Note>> {
    let rows = crate::store::read(world, &Tier::Personal(owner.to_owned()))?.rows;
    let mut open: Vec<Note> = crate::note::fold(&rows)
        .into_values()
        .filter(|n| n.status == Status::Proposed && n.owner.as_deref() == Some(owner))
        .collect();
    open.sort_by_key(|n| std::cmp::Reverse(n.born));
    Ok(open)
}

/// Whether `rows`, one tier's events, hold the note `id` retired by the gate — dormant, its last
/// status the gate's: what [`retired`] lists and [`wake`] undoes.
#[must_use]
pub fn retired_in(rows: &[crate::note::Event], id: &str) -> bool {
    gate_retired(rows, id)
}

/// The claim `id` of `owner`, still proposed in its personal tier — or grounded and waiting under
/// review for a route to ground it again: what a route may ground later — a commissioner's verdict
/// that accepts it, a second voice, the human. `None` when the owner holds no such note, or it is
/// neither.
///
/// # Errors
///
/// The personal tier cannot be read.
pub fn proposed(world: &Path, owner: &str, id: &str) -> std::io::Result<Option<Claim<Proposed>>> {
    let rows = crate::store::read(world, &Tier::Personal(owner.to_owned()))?.rows;
    let notes = crate::note::fold(&rows);
    Ok(notes
        .get(id)
        .filter(|n| {
            (n.status == Status::Proposed || (n.status == Status::Grounded && n.review.is_some()))
                && n.owner.as_deref() == Some(owner)
        })
        .map(|n| Claim {
            note: n.id.clone(),
            owner: owner.to_owned(),
            text: n.text.clone(),
            tags: n.tags.clone(),
            source: n.source.clone(),
            evidence: n.evidence.clone(),
            depends_on: n.depends_on.clone(),
            standing: PhantomData,
        }))
}

/// Correct the claim `id` of `owner`: its next version, under the same id, never an edit in place.
/// The new version rests on `depends_on`, with the files its evidence names hashed now; it is
/// proposed until a route grounds it, in every tier that holds it. Whatever rests on the claim
/// needs review: what it said has been superseded. Returns the new version's number and the ids
/// marked for review.
///
/// # Errors
///
/// The creature's memory is frozen ([`crate::unfrozen`]), `owner` holds no live claim `id`, or a
/// note file cannot be read or written.
pub fn amend(
    world: &Path,
    owner: &str,
    id: &str,
    text: &str,
    depends_on: Vec<Dependency>,
) -> std::io::Result<(u32, Vec<String>)> {
    crate::audit::unfrozen(world, owner)?;
    let personal = Tier::Personal(owner.to_owned());
    let rows = crate::store::read(world, &personal)?.rows;
    let notes = crate::note::fold(&rows);
    let Some(n) = notes.get(id).filter(|n| {
        n.owner.as_deref() == Some(owner) && matches!(n.status, Status::Proposed | Status::Grounded)
    }) else {
        return Err(std::io::Error::other(format!(
            "{owner} holds no live claim `{id}` to correct"
        )));
    };
    let mut depends_on = depends_on;
    for f in crate::graph::files(world, &n.evidence) {
        if !depends_on.contains(&f) {
            depends_on.push(f);
        }
    }
    let change = Change::Version {
        text: text.to_owned(),
        depends_on,
    };
    append(world, &personal, id, By::Record, None, change.clone())?;
    if crate::store::read(world, &Tier::Shared)?
        .rows
        .iter()
        .any(|e| e.note == id)
    {
        append(world, &Tier::Shared, id, By::Record, None, change)?;
    }
    let version = n.version + 1;
    let marked = crate::graph::invalidate(
        world,
        crate::graph::Moved::Claim(id),
        &format!("`{id}` it rests on was superseded by version {version}"),
    )?;
    Ok((version, marked))
}

/// What [`corroborate`] leaves: the claim grounded with the id of the one that corroborated it,
/// or the claim back, still proposed.
pub type Seconded = Result<(Claim<Grounded>, String), Claim<Proposed>>;

/// The **corroborated** route's detector: another creature's proposed claim that says the same —
/// the same content words, at least [`SAME_WORDS`] of them, negated alike — said within
/// [`SECOND_VOICE_DAYS`], resting on evidence disjoint from this claim's. `kin`, the creatures in
/// the claim owner's line, never count: a parent restating its child's offered claim is an echo,
/// not a second voice. This claim is grounded and shared citing the other; the other is grounded
/// in its owner's tier citing this one, and not shared again, so recall holds one copy. Returns
/// the grounded claim and the id of the one that corroborated it, or the claim back.
///
/// # Errors
///
/// A personal tier cannot be read, or a grounding cannot be written.
pub fn corroborate(
    world: &Path,
    claim: Claim<Proposed>,
    kin: &[String],
) -> std::io::Result<Seconded> {
    if claim.evidence.is_empty() {
        return Ok(Err(claim));
    }
    let since = now_ms().saturating_sub(SECOND_VOICE_DAYS * 86_400_000);
    let mut others: Vec<String> = std::fs::read_dir(world.join(".mnemorph/memory/creature"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .filter(|o| *o != claim.owner && !kin.contains(o))
                .collect()
        })
        .unwrap_or_default();
    others.sort();
    for other in others {
        let rows = crate::store::read(world, &Tier::Personal(other.clone()))?.rows;
        // The newest match first: a recent second voice, not the oldest.
        let mut notes: Vec<_> = crate::note::fold(&rows).into_values().collect();
        notes.sort_by_key(|n| std::cmp::Reverse(n.born));
        let Some(theirs) = notes.into_iter().find(|n| {
            n.status == Status::Proposed
                && n.owner.as_deref() == Some(other.as_str())
                && n.born >= since
                && same_claim(&n.text, &claim.text)
                && Proof::corroborated(&claim.owner, &other, &claim.evidence, &n.evidence).is_ok()
        }) else {
            continue;
        };
        let (Ok(ours), Ok(back)) = (
            Proof::corroborated(&claim.owner, &other, &claim.evidence, &theirs.evidence),
            Proof::corroborated(&other, &claim.owner, &theirs.evidence, &claim.evidence),
        ) else {
            continue;
        };
        // Both tiers writable before either is touched, so one side never stands alone.
        if crate::audit::unfrozen(world, &other).is_err() {
            continue;
        }
        let grounded = ground(world, claim, &ours)?;
        let cites = format!("{}: {}", back.route.word(), back.cites);
        let personal = Tier::Personal(other.clone());
        append(
            world,
            &personal,
            &theirs.id,
            By::Route,
            Some(cites.clone()),
            Change::Status {
                status: Status::Grounded,
            },
        )?;
        append(
            world,
            &personal,
            &theirs.id,
            By::Route,
            Some(cites),
            Change::Confidence {
                confidence: back.route.confidence(),
            },
        )?;
        return Ok(Ok((grounded, theirs.id)));
    }
    Ok(Err(claim))
}

/// The fewest content words a claim must hold to be matched: "tests pass" matches by chance.
pub const SAME_WORDS: usize = 3;

/// How recent a second voice must be, in days.
pub const SECOND_VOICE_DAYS: u64 = 30;

/// Whether two claims say the same: the same content words, and both negated or neither — stop
/// words drop `not`, so "X is not Y" would otherwise match "X is Y".
fn same_claim(a: &str, b: &str) -> bool {
    let set = |t: &str| {
        crate::text::content_words(t)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    };
    let negated = |t: &str| {
        crate::text::words(t).iter().any(|w| {
            matches!(
                w.as_str(),
                "not" | "no" | "never" | "none" | "nothing" | "without" | "cannot" | "nor" | "t"
            )
        })
    };
    let (sa, sb) = (set(a), set(b));
    sa.len() >= SAME_WORDS && sa == sb && negated(a) == negated(b)
}
