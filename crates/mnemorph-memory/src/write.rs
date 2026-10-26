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
