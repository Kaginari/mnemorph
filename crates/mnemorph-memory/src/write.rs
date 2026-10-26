//! The write rule: a body's words land personal and proposed, and only a route's proof grounds
//! them. That is a type, not a habit — a `Claim<Proposed>` becomes a `Claim<Grounded>` only
//! through a [`Proof`], and every proof is built from the evidence its route needs, refused
//! without it. A body cannot ground its own words.

use std::marker::PhantomData;
use std::path::Path;

use crate::note::{By, Change, Polarity, Status};
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
}

impl Proof {
    /// **checked**: the end gate passed, citing its verdict, and ran `verify_lines` over a diff of
    /// `changed` paths. An answer that changed nothing, or a gate with no verify line, proves
    /// nothing of its claims.
    ///
    /// # Errors
    ///
    /// No verify line ran, or the diff was empty.
    pub fn checked(verdict: &str, verify_lines: usize, changed: usize) -> Result<Self, String> {
        if verify_lines == 0 {
            return Err("no verify line ran: the gate proved nothing of the claims".into());
        }
        if changed == 0 {
            return Err("the answer changed nothing: a pass over no diff proves no claim".into());
        }
        Ok(Self {
            route: Route::Checked,
            cites: format!("{verdict} ({verify_lines} verify line(s) over {changed} path(s))"),
        })
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
        })
    }

    /// **confirmed**: the human said so, citing their answer.
    #[must_use]
    pub fn confirmed(answer: &str) -> Self {
        Self {
            route: Route::Confirmed,
            cites: answer.to_owned(),
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
/// The note file cannot be written.
pub fn propose(
    world: &Path,
    owner: &str,
    text: &str,
    tags: Vec<String>,
    source: Option<String>,
) -> std::io::Result<Claim<Proposed>> {
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
        },
    )?;
    Ok(Claim {
        note,
        owner: owner.to_owned(),
        text: text.to_owned(),
        tags,
        source,
        standing: PhantomData,
    })
}

/// Ground a claim by its route's proof: in its owner's personal tier, and in shared memory, each
/// event citing the proof, at the route's confidence.
///
/// # Errors
///
/// A note file cannot be written.
pub fn ground(
    world: &Path,
    claim: Claim<Proposed>,
    proof: &Proof,
) -> std::io::Result<Claim<Grounded>> {
    let cites = format!("{}: {}", proof.route.word(), proof.cites);
    let confidence = proof.route.confidence();
    let personal = Tier::Personal(claim.owner.clone());
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
        },
    )?;
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
/// A note file cannot be written.
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
                links,
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
