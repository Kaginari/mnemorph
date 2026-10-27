//! What the act runner reads of the forge, through the harness's probe: who `gh` acts as, a
//! branch's tip, and the pull requests a typed act opened.
//!
//! A pull request is the act's only when every field the human approved reads back from the forge
//! as approved: the head and base branches, the head in the act's repository, the author the login
//! `gh` acts as, the title, and the description's SHA-256. Someone else's pull request with the
//! same branches is never taken for it.

use std::time::Duration;

use serde::Deserialize;

use crate::outward::sha_ok;
use crate::receipt::{excerpt, redact, sha256};
use crate::{Act, PROBE_TIMEOUT, Probe};

/// The most a forge list returns in one page: a full page cannot tell what is past it.
pub(crate) const PAGE: usize = 100;

/// Who a forge says someone is.
#[derive(Debug, Deserialize)]
pub(crate) struct Login {
    pub(crate) login: String,
}

/// `gh api` GET of `path` on `host`, as every forge probe runs it.
pub(crate) fn forge_get(host: &str, path: &str) -> Vec<String> {
    ["gh", "api", "--hostname", host, "--method", "GET", path]
        .map(str::to_owned)
        .to_vec()
}

/// Run one probe within `timeout`, keeping what it was and what it printed for the journal.
pub(crate) fn probed_within(
    probe: &dyn Probe,
    argv: &[String],
    timeout: Duration,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<Vec<u8>, String> {
    probes.push(redact(&argv.join(" ")));
    let s = probe
        .run(argv, timeout)
        .map_err(|e| format!("the probe failed: {}", redact(&e)))?;
    seen.push(excerpt(&s.out, crate::SEEN_BYTES));
    if s.code != Some(0) {
        return Err(format!(
            "the probe exited {}: {}",
            s.code
                .map_or_else(|| "on a signal".into(), |c| c.to_string()),
            excerpt(&s.err, 200)
        ));
    }
    if s.cut {
        return Err("the probe printed more than it reads: it cannot tell".into());
    }
    Ok(s.out)
}

/// [`probed_within`] the probe's own deadline.
pub(crate) fn probed(
    probe: &dyn Probe,
    argv: &[String],
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<Vec<u8>, String> {
    probed_within(probe, argv, PROBE_TIMEOUT, probes, seen)
}

/// The login `gh` acts as on `host`.
pub(crate) fn login(
    host: &str,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<String, String> {
    let me: Login = serde_json::from_slice(&probed(probe, &forge_get(host, "user"), probes, seen)?)
        .map_err(|e| format!("the login does not read: {e}"))?;
    Ok(me.login)
}

/// The commit `refs/heads/<branch>` names in `repo` on `host`; an error when it is not a branch
/// there, or cannot be read.
pub(crate) fn branch_tip(
    host: &str,
    repo: &str,
    branch: &str,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Object {
        sha: String,
    }
    #[derive(Deserialize)]
    struct Ref {
        #[serde(rename = "ref")]
        name: String,
        object: Object,
    }
    let path = format!("repos/{repo}/git/ref/heads/{branch}");
    let out = probed(probe, &forge_get(host, &path), probes, seen)?;
    let want = format!("refs/heads/{branch}");
    // One ref, read exactly: a list (an older forge's prefix match) counts only its exact entry.
    let refs: Vec<Ref> = match serde_json::from_slice::<Ref>(&out) {
        Ok(one) => vec![one],
        Err(_) => serde_json::from_slice(&out)
            .map_err(|e| format!("the branch's ref does not read: {e}"))?,
    };
    match refs
        .into_iter()
        .filter(|r| r.name == want)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [one] if sha_ok(&one.object.sha) => Ok(one.object.sha.clone()),
        [_] => Err(format!("{want} names an object that does not read")),
        [] => Err(format!("{repo} has no {want}")),
        _ => Err(format!("{repo} lists {want} more than once")),
    }
}

/// One side of a pull request: its branch and the repository it is in.
#[derive(Debug, Deserialize)]
pub(crate) struct Side {
    #[serde(rename = "ref")]
    branch: String,
    #[serde(default)]
    repo: Option<Full>,
}

#[derive(Debug, Deserialize)]
struct Full {
    full_name: String,
}

/// A pull request as the forge returns it, in a list or alone.
#[derive(Debug, Deserialize)]
pub(crate) struct Pull {
    pub(crate) number: u64,
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) merged_at: Option<String>,
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    user: Option<Login>,
    head: Side,
    base: Side,
}

/// The fields of a pull request act a pull request on the forge is read against.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wanted<'a> {
    pub(crate) repo: &'a str,
    pub(crate) base: &'a str,
    pub(crate) head: &'a str,
    title: &'a str,
    body: &'a str,
}

impl<'a> Wanted<'a> {
    /// The fields of `act`, when it opens a pull request.
    pub(crate) fn of(act: &'a Act) -> Option<Self> {
        match act {
            Act::OpenPullRequest {
                repo,
                base,
                head,
                title,
                body,
            } => Some(Self {
                repo,
                base,
                head,
                title,
                body,
            }),
            _ => None,
        }
    }

    /// Why `pull` is not this act's own, as the login `me` would have opened it; `None` when it is.
    pub(crate) fn not_ours(&self, pull: &Pull, me: &str) -> Option<String> {
        let n = pull.number;
        let in_repo = |side: &Side| {
            side.repo
                .as_ref()
                .is_some_and(|r| r.full_name.eq_ignore_ascii_case(self.repo))
        };
        if pull.head.branch != self.head || !in_repo(&pull.head) {
            return Some(format!(
                "#{n}'s head is {}, not {} in {}",
                excerpt(pull.head.branch.as_bytes(), 100),
                self.head,
                self.repo
            ));
        }
        if pull.base.branch != self.base || !in_repo(&pull.base) {
            return Some(format!(
                "#{n}'s base is {}, not {}",
                excerpt(pull.base.branch.as_bytes(), 100),
                self.base
            ));
        }
        if pull.user.as_ref().is_none_or(|u| u.login != me) {
            return Some(format!(
                "#{n} is by {}, not by {me}: it is not this act's",
                pull.user.as_ref().map_or("someone unknown", |u| &u.login)
            ));
        }
        if pull.title != self.title {
            return Some(format!("#{n}'s title is not the act's: it was edited"));
        }
        let (want, got) = (
            sha256(self.body),
            sha256(pull.body.as_deref().unwrap_or_default()),
        );
        if want != got {
            return Some(format!(
                "#{n}'s description sha256 is {got}, not {want}: it was edited"
            ));
        }
        None
    }
}

/// The open pull requests from `head` into `base` in `repo`, the head in `repo` itself.
pub(crate) fn open_pulls(
    host: &str,
    want: &Wanted<'_>,
    probe: &dyn Probe,
    timeout: Duration,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<Vec<Pull>, String> {
    let owner = want.repo.split_once('/').map_or(want.repo, |(o, _)| o);
    let path = format!(
        "repos/{}/pulls?head={owner}:{}&base={}&state=open&per_page={PAGE}",
        want.repo, want.head, want.base
    );
    let list: Vec<Pull> = serde_json::from_slice(&probed_within(
        probe,
        &forge_get(host, &path),
        timeout,
        probes,
        seen,
    )?)
    .map_err(|e| format!("the pull requests do not read: {e}"))?;
    if list.len() >= PAGE {
        return Err(format!(
            "{PAGE} pull requests or more are open from {} into {}: the rest are unread",
            want.head, want.base
        ));
    }
    Ok(list)
}

/// The act's pull request on the forge, read as strictly by a cut act's recovery as by `observe`:
/// `Some(number)` when exactly one is open from its head into its base, and it is the act's own
/// (by `gh`'s login, with its title and description — and, when the act printed one, its number);
/// `None` when none is open. Anything else — someone else's, an edited one, two, a probe that
/// fails — is an error: it cannot be told.
pub(crate) fn find(
    host: &str,
    want: &Wanted<'_>,
    known: Option<u64>,
    probe: &dyn Probe,
    timeout: Duration,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<Option<u64>, String> {
    let me = login(host, probe, probes, seen)?;
    let list = open_pulls(host, want, probe, timeout, probes, seen)?;
    match list.as_slice() {
        [] => Ok(None),
        [one] => {
            if let Some(why) = want.not_ours(one, &me) {
                return Err(why);
            }
            match known {
                Some(n) if n != one.number => Err(format!(
                    "#{} is open from {} into {}, not #{n}, the one the act printed",
                    one.number, want.head, want.base
                )),
                _ => Ok(Some(one.number)),
            }
        }
        many => Err(format!(
            "{} pull requests are open from {} into {}: which is the act's cannot be told",
            many.len(),
            want.head,
            want.base
        )),
    }
}

/// Pull request `number` of `repo` on `host`, read alone.
pub(crate) fn pull(
    host: &str,
    repo: &str,
    number: u64,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<Pull, String> {
    let path = format!("repos/{repo}/pulls/{number}");
    let p: Pull = serde_json::from_slice(&probed(probe, &forge_get(host, &path), probes, seen)?)
        .map_err(|e| format!("the pull request does not read: {e}"))?;
    if p.number == number {
        Ok(p)
    } else {
        Err(format!(
            "asked for #{number}, the forge returned #{}",
            p.number
        ))
    }
}

/// Whether pull request `number` is still the act's own and open: what its undo — a close —
/// needs. A merged one is never "undone" by closing it.
pub(crate) fn still_open_and_ours(
    host: &str,
    want: &Wanted<'_>,
    number: u64,
    probe: &dyn Probe,
    probes: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<(), String> {
    let me = login(host, probe, probes, seen)?;
    let p = pull(host, want.repo, number, probe, probes, seen)?;
    if p.merged_at.is_some() {
        return Err(format!("#{number} was merged: closing it undoes nothing"));
    }
    if p.state != "open" {
        return Err(format!(
            "#{number} is {}, not open",
            excerpt(p.state.as_bytes(), 40)
        ));
    }
    match want.not_ours(&p, &me) {
        Some(why) => Err(why),
        None => Ok(()),
    }
}
