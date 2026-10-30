//! A landing's Dark Elf challenge and its finding's verification, explored breadth-first over the
//! real records ([`Challenges`]): every state reachable from a freshly opened challenge to its
//! fixpoint (the walk ends when a layer finds nothing new, within [`DEPTH`]), with the shortest
//! event path to each, and the invariants asserted in every one. Each state is a copy of the
//! records file; each move runs on a copy of its parent.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use mnemorph_ledger::{Ascension, Standing, fold_findings};
use mnemorph_proving::coverage::{
    CommandRun, CoverageLedger, Entry, Evidence, FileRead, HIGH_RISK, Landing, Ran, State,
};
use mnemorph_proving::{
    FindingRecord, Impact, Likelihood, Severity, Step, StepKind, Verdict, Witness, WitnessKind,
    sha256_hex,
};
use mnemorph_races::{Race, Speciality};
use mnemorph_world::Roster;
use mnemorph_world::challenge::{
    Challenges, Decision, Filing, Found, Opening, Pinned, Refusal, Rerun, Ruling, Staff, Status,
    Waiver, route,
};

const FILE: &str = "crates/mnemorph-tools/src/files.rs";
const SOURCE: &str = "pub fn read_path(p: &str) -> String { todo!() }\n";
const POC: &str = ".mnemorph/dark-elf/dark-elf-harness/poc/read_path.rs";
const POC_BODY: &str = "#[test]\nfn escapes() {}\n";
const RERUN: &str = "rustc --test .mnemorph/dark-elf/dark-elf-harness/poc/read_path.rs";
const FINDER: &str = "dark-elf-harness";
const VERIFIER: &str = "dark-elf-agent";

/// How deep the walk may go; it reaches its fixpoint before.
const DEPTH: usize = 12;

fn tree_hash() -> String {
    "ab".repeat(32)
}

/// The landing's tree: the source and the finder's proof-of-concept.
fn tree() -> BTreeMap<String, Vec<u8>> {
    [
        (FILE.to_owned(), SOURCE.as_bytes().to_vec()),
        (POC.to_owned(), POC_BODY.as_bytes().to_vec()),
    ]
    .into_iter()
    .collect()
}

/// What the reviewers and the verifier ran: the tests, and the witness, failing.
fn ran() -> Ran {
    let mut r = Ran::default();
    r.record("cargo test", "ok");
    r.record_exit(RERUN, "test escapes ... FAILED: read ../secret", true);
    r
}

fn landing() -> Landing {
    route(&Roster::default(), "run-1:1", &[FILE.to_owned()])
}

/// A ledger that holds: each high-risk class checked on the written file, the rest ruled out.
fn ledger(l: &Landing, s: Speciality) -> CoverageLedger {
    CoverageLedger {
        landing: l.id.clone(),
        speciality: s,
        entries: l
            .required(s)
            .into_iter()
            .enumerate()
            .map(|(n, (surface, class))| Entry {
                surface,
                class: class.name().to_owned(),
                state: if HIGH_RISK.contains(&class) {
                    State::Checked {
                        method: format!("read {class} paths through the file and ran tests"),
                        evidence: Evidence {
                            files: vec![FileRead {
                                path: FILE.to_owned(),
                                symbols: vec!["read_path".to_owned()],
                                sha256: sha256_hex(SOURCE.as_bytes()),
                            }],
                            commands: vec![CommandRun {
                                run: "cargo test".to_owned(),
                                sha256: None,
                            }],
                        },
                    }
                } else {
                    State::NotApplicable {
                        reason: format!("fact {n}: the landing adds no reader of this kind"),
                        evidence: Evidence::default(),
                    }
                },
            })
            .collect(),
    }
}

/// The harness's finding at `severity`, its witness the proof-of-concept test.
fn found(severity: Severity) -> Found {
    let fp = mnemorph_wire::fingerprint(FINDER, "path-traversal", FILE, "read_path").unwrap();
    let record = FindingRecord {
        verdict: Verdict::Finding,
        fingerprint: fp.clone(),
        speciality: FINDER.into(),
        class: "path-traversal".into(),
        file: FILE.into(),
        symbol: "read_path".into(),
        claim: "read_path follows `..` out of the world".into(),
        trace: vec![Step {
            kind: StepKind::Sink,
            at: format!("{FILE}:1"),
            note: String::new(),
        }],
        witness: Some(Witness {
            kind: WitnessKind::Test,
            reference: POC.into(),
            sha256: sha256_hex(POC_BODY.as_bytes()),
            output: None,
            output_sha256: None,
        }),
        preconditions: vec!["an untrusted repository".into()],
        reachability: "a repository file names the path".into(),
        likelihood: Some(Likelihood::Medium),
        likelihood_reason: "worlds are cloned from strangers".into(),
        impact: Some(match severity {
            Severity::Critical => Impact::Bypass,
            Severity::High => Impact::Disclosure,
            _ => Impact::Crash,
        }),
        impact_reason: "the test reads a file outside the world".into(),
        severity: Some(severity),
        fix: "refuse `..`".into(),
        invariant: "no read leaves the world".into(),
        rec: mnemorph_proving::record_path(FINDER, &fp),
    };
    Found {
        finder: FINDER.into(),
        rec: record.rec.clone(),
        record,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Ev {
    /// A routed speciality files its ledger: the `n`-th in routing order.
    File(usize),
    /// A reviewer's finding is kept, at a severity.
    Found(Severity),
    /// The finding's verifier is commissioned.
    Assign,
    /// The verifier confirms it at its severity.
    Confirm,
    /// The verifier confirms it, lowered to medium.
    ConfirmLower,
    Refute,
    CannotVerify,
    /// The throne proposes it as a seal.
    Propose,
    /// The human upholds it.
    Uphold,
    /// The human dismisses it as a false positive.
    Dismiss,
    /// The human waives the challenge.
    Waive,
    /// A torn line naming the landing lands in the records.
    Torn,
}

/// A challenge's state as the walk tells them apart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Abs {
    /// `pending`, `verifying`, `blocked`, `cleared` or `waived`.
    status: &'static str,
    /// Routed specialities that filed.
    filed: usize,
    /// The finding: its severity as found and as it stands, its verdict, its standing, whether it
    /// was proposed as a seal.
    finding: Option<(Severity, Severity, Option<&'static str>, String, bool)>,
    /// The finding's verifier is commissioned.
    assigned: bool,
    /// A torn line holds it.
    torn: bool,
}

struct Node {
    tmp: tempfile::TempDir,
}

impl Node {
    fn file(&self) -> PathBuf {
        self.tmp.path().join("challenges.jsonl")
    }

    fn c(&self) -> Challenges {
        Challenges::at(&self.file())
    }

    fn fork(&self) -> Node {
        let n = Node {
            tmp: tempfile::tempdir().unwrap(),
        };
        fs::copy(self.file(), n.file()).unwrap();
        n
    }

    #[allow(clippy::many_single_char_names, reason = "the walk's own short names")]
    fn apply(&self, l: &Landing, e: Ev) {
        let c = self.c();
        let fp = found(Severity::Medium).fingerprint().to_owned();
        let filing = |ruling| Filing {
            landing: l.id.clone(),
            fingerprint: fp.clone(),
            verifier: VERIFIER.into(),
            tree: tree_hash(),
            ruling,
        };
        let confirmed = |severity| Ruling::Confirmed {
            severity,
            reason: "the read stays inside files the grant already shows".into(),
            reran: vec![Rerun {
                run: RERUN.into(),
                sha256: String::new(),
                failed: false,
            }],
        };
        let severity = || {
            c.record(&l.id)
                .and_then(|r| r.findings.first().map(|f| f.found.severity()))
        };
        let res = match e {
            Ev::File(n) => {
                let s = l.routed[n].speciality;
                c.file(ledger(l, s), &tree(), &ran()).map(|_| ())
            }
            Ev::Found(sev) => c.found(&l.id, found(sev)).map(|_| ()),
            Ev::Assign => c.assign(
                &l.id,
                &fp,
                Staff::Commissioned {
                    body: VERIFIER.into(),
                    by: "rimuru".into(),
                    wt: 3,
                    waited: 0,
                },
            ),
            Ev::Confirm => match severity() {
                Some(sev) => c
                    .verdict(filing(confirmed(sev)), &tree(), &ran())
                    .map(|_| ()),
                None => Ok(()),
            },
            Ev::ConfirmLower => c
                .verdict(filing(confirmed(Severity::Medium)), &tree(), &ran())
                .map(|_| ()),
            Ev::Refute => c
                .verdict(
                    filing(Ruling::Refuted {
                        reason: "read_path is only reached through the grant check in files.rs"
                            .into(),
                        files: vec![Pinned {
                            path: FILE.into(),
                            sha256: sha256_hex(SOURCE.as_bytes()),
                        }],
                        reran: Vec::new(),
                    }),
                    &tree(),
                    &ran(),
                )
                .map(|_| ()),
            Ev::CannotVerify => c
                .verdict(
                    filing(Ruling::CannotVerify {
                        blocker: "the proof-of-concept needs a network the sandbox denies".into(),
                    }),
                    &tree(),
                    &ran(),
                )
                .map(|_| ()),
            Ev::Propose => c.propose(&l.id, &fp, "rimuru").map(|_| ()),
            Ev::Uphold | Ev::Dismiss => {
                let d = Decision::human(
                    e == Ev::Uphold,
                    "the fix in the next landing proves it either way",
                )
                .unwrap();
                c.decide(&l.id, &fp, &d)
            }
            Ev::Waive => c
                .waive(
                    &l.id,
                    &Waiver::human("the release ships the read behind a flag").unwrap(),
                )
                .map(|_| ()),
            Ev::Torn => {
                let mut f = fs::OpenOptions::new()
                    .append(true)
                    .open(self.file())
                    .unwrap();
                writeln!(f, "{{\"row\":\"filed\",\"landing\":\"{}\",\"to", l.id).unwrap();
                Ok(())
            }
        };
        // A refusal changes nothing; the walk reads that from the state.
        let _ = res;
    }

    /// The invariants every state keeps, and the state, from one reading of the records.
    fn check(&self, l: &Landing, path: &[Ev]) -> Abs {
        let r = self.c().record(&l.id).unwrap();
        let (_, ledgers) = self.c().ledgers(&l.id).unwrap();
        let torn = !self.c().unreadable().is_empty();
        let critical = r
            .findings
            .iter()
            .any(|f| f.found.severity() == Severity::Critical);
        match &r.status {
            Status::Cleared => {
                // Cleared only with every routed ledger filed, nothing torn, every finding ruled,
                // none blocking, and no critical: a critical is the human's to settle.
                assert_eq!(ledgers.len(), l.routed.len(), "{path:?}");
                assert!(!torn, "{path:?}: cleared over a torn line");
                assert!(r.findings.iter().all(|f| f.verdict.is_some()), "{path:?}");
                assert!(r.findings.iter().all(|f| !f.blocks()), "{path:?}");
                assert!(!critical, "{path:?}: a critical cleared without the human");
            }
            Status::Blocked { findings } => {
                assert!(
                    !findings.is_empty()
                        && r.findings
                            .iter()
                            .any(mnemorph_world::challenge::Finding::blocks)
                );
            }
            Status::Verifying { findings } => {
                assert_eq!(ledgers.len(), l.routed.len(), "{path:?}");
                assert!(!findings.is_empty() && !torn, "{path:?}");
            }
            Status::Pending { awaiting, .. } => {
                assert!(!awaiting.is_empty() || torn, "{path:?}");
            }
            Status::Waived { .. } => {
                assert!(path.contains(&Ev::Waive), "{path:?}: waived by no one");
            }
        }
        // A critical finding not waived always blocks, whatever its verdict or decision.
        if critical && !matches!(r.status, Status::Waived { .. }) {
            assert!(
                matches!(r.status, Status::Blocked { .. }),
                "{path:?}: {:?}",
                r.status
            );
        }
        for f in &r.findings {
            // Only a confirmed finding counts; only one confirmed at high or more is a seal.
            assert_eq!(
                f.counts(),
                matches!(
                    f.verdict.as_ref().map(|v| &v.ruling),
                    Some(Ruling::Confirmed { .. })
                )
            );
            if f.proposal.is_some() {
                assert!(f.counts() && f.severity() >= Severity::High, "{path:?}");
            }
            // Never raised: the severity stands at or under the one found.
            assert!(f.severity() <= f.found.severity(), "{path:?}");
            // The human's word outranks the verdict in the value ledger.
            if let Some(d) = &f.decision {
                let want = if d.upheld() {
                    Standing::Upheld
                } else {
                    Standing::Dismissed
                };
                assert_eq!(f.standing(), want, "{path:?}");
            }
        }
        // The value ledger reads the same standings: a held critical is neither upheld nor
        // refuted, and the fold never proposes probation to a body that has not risen.
        let folds = fold_findings(&self.c().finding_rows());
        for fold in folds.values() {
            assert_eq!(
                fold.findings,
                fold.upheld + fold.refuted + fold.cannot_verify + fold.held + fold.unverified
            );
            assert_ne!(fold.proposal(Race::DarkElf), Some(Ascension::Probation));
        }
        Abs {
            status: status_of(&r.status),
            filed: ledgers.len(),
            finding: r.findings.first().map(|f| {
                (
                    f.found.severity(),
                    f.severity(),
                    f.verdict.as_ref().map(|v| v.ruling.word()),
                    format!("{:?}", f.standing()),
                    f.proposal.is_some(),
                )
            }),
            assigned: r.findings.first().is_some_and(|f| f.staff.is_some()),
            torn,
        }
    }
}

fn status_of(s: &Status) -> &'static str {
    match s {
        Status::Pending { .. } => "pending",
        Status::Verifying { .. } => "verifying",
        Status::Blocked { .. } => "blocked",
        Status::Cleared => "cleared",
        Status::Waived { .. } => "waived",
    }
}

/// The last move of `path` from `node`, on a copy of it: the copy and its state, every invariant
/// checked.
fn advance(l: &Landing, node: &Node, abs: &Abs, path: &[Ev]) -> (Node, Abs) {
    let e = *path.last().unwrap();
    let n = node.fork();
    n.apply(l, e);
    let after = n.check(l, path);
    // A waiver is final, and only the human's word waives.
    if abs.status == "waived" {
        assert_eq!(after.status, "waived", "{path:?}");
    }
    if after.status == "waived" && abs.status != "waived" {
        assert_eq!(e, Ev::Waive, "{path:?}");
    }
    // A verdict is filed once, and only by its assigned verifier.
    let verdict = |a: &Abs| a.finding.as_ref().and_then(|f| f.2);
    if verdict(abs).is_some() {
        assert_eq!(verdict(abs), verdict(&after), "{path:?}");
    }
    if verdict(&after).is_some() && verdict(abs).is_none() {
        assert!(abs.assigned, "{path:?}: a verdict unassigned");
    }
    (n, after)
}

/// Breadth-first from a freshly opened challenge: each state, with the shortest path to it.
#[allow(clippy::many_single_char_names, reason = "the walk's own short names")]
fn explore(l: &Landing) -> BTreeMap<Abs, Vec<Ev>> {
    let mut events: Vec<Ev> = (0..l.routed.len()).map(Ev::File).collect();
    events.extend([
        Ev::Found(Severity::Medium),
        Ev::Found(Severity::High),
        Ev::Found(Severity::Critical),
        Ev::Assign,
        Ev::Confirm,
        Ev::ConfirmLower,
        Ev::Refute,
        Ev::CannotVerify,
        Ev::Propose,
        Ev::Uphold,
        Ev::Dismiss,
        Ev::Waive,
        Ev::Torn,
    ]);
    let start = Node {
        tmp: tempfile::tempdir().unwrap(),
    };
    let opened = start
        .c()
        .open_at(
            l,
            &Opening {
                tree: tree_hash(),
                by: "orc-o".into(),
                ..Opening::default()
            },
        )
        .unwrap();
    assert_eq!(status_of(&opened), "pending");
    let first = start.check(l, &[]);
    let mut seen = BTreeMap::from([(first.clone(), Vec::new())]);
    let mut frontier = vec![(first, start)];
    let mut depth = 0;
    while !frontier.is_empty() {
        assert!(
            depth < DEPTH,
            "no fixpoint within {DEPTH}: {} states",
            seen.len()
        );
        depth += 1;
        // The layer's moves run on every core, each on its own copy; merged in order.
        let jobs: Vec<(usize, Ev)> = (0..frontier.len())
            .flat_map(|i| events.iter().map(move |e| (i, *e)))
            .collect();
        let taken = AtomicUsize::new(0);
        let done: Mutex<Vec<(usize, Node, Abs)>> = Mutex::new(Vec::new());
        let threads = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        let k = taken.fetch_add(1, Ordering::SeqCst);
                        let Some(&(i, e)) = jobs.get(k) else { break };
                        let (abs, node) = &frontier[i];
                        let mut path = seen[abs].clone();
                        path.push(e);
                        let (n, after) = advance(l, node, abs, &path);
                        done.lock().unwrap().push((k, n, after));
                    }
                });
            }
        });
        let mut done = done.into_inner().unwrap();
        done.sort_by_key(|(k, _, _)| *k);
        let mut next = Vec::new();
        for (k, n, after) in done {
            let (i, e) = jobs[k];
            if !seen.contains_key(&after) {
                let mut path = seen[&frontier[i].0].clone();
                path.push(e);
                seen.insert(after.clone(), path);
                next.push((after, n));
            }
        }
        frontier = next;
    }
    println!(
        "challenge: {} states, fixpoint at depth {depth}",
        seen.len()
    );
    seen
}

#[test]
fn a_challenge_and_its_finding_reach_their_states_and_no_others() {
    let l = landing();
    assert!(!l.routed.is_empty(), "{:?}", l.routed);
    let states = explore(&l);
    let find = |what: &str, f: &dyn Fn(&Abs) -> bool| {
        states
            .iter()
            .filter(|(a, _)| f(a))
            .map(|(_, p)| p.clone())
            .min_by_key(Vec::len)
            .unwrap_or_else(|| panic!("unreachable: {what}"))
    };
    let routed = l.routed.len();
    let all_filed: Vec<Ev> = (0..routed).map(Ev::File).collect();
    // Every status, with its shortest witness.
    let cleared = find("cleared", &|a| a.status == "cleared");
    assert_eq!(cleared, all_filed);
    let statuses: BTreeSet<&str> = states.keys().map(|a| a.status).collect();
    assert_eq!(
        statuses,
        ["blocked", "cleared", "pending", "verifying", "waived"].into()
    );
    let finding = |a: &Abs| a.finding.clone();
    let with = |status: &'static str, verdict: Option<&'static str>, standing: &'static str| {
        move |a: &Abs| {
            a.status == status && finding(a).is_some_and(|f| f.2 == verdict && f.3 == standing)
        }
    };
    find(
        "verifying an unverified finding",
        &with("verifying", None, "Unverified"),
    );
    find("cleared over a confirmed medium", &|a| {
        a.status == "cleared"
            && finding(a).is_some_and(|f| f.0 == Severity::Medium && f.3 == "Confirmed")
    });
    find("cleared over a refuted high", &|a| {
        a.status == "cleared"
            && finding(a).is_some_and(|f| f.0 == Severity::High && f.3 == "Refuted")
    });
    find("cleared over a high lowered to medium", &|a| {
        a.status == "cleared"
            && finding(a).is_some_and(|f| f.0 == Severity::High && f.1 == Severity::Medium)
    });
    find(
        "cleared over a cannot-verify",
        &with("cleared", Some("cannot-verify"), "CannotVerify"),
    );
    find("blocked by a high confirmed at high", &|a| {
        a.status == "blocked"
            && finding(a).is_some_and(|f| f.1 == Severity::High && f.2 == Some("confirmed"))
    });
    find(
        "a refuted critical held for the human",
        &with("blocked", Some("refuted"), "Held"),
    );
    find(
        "a refuted critical dismissed, still blocked",
        &with("blocked", Some("refuted"), "Dismissed"),
    );
    find("a critical upheld, still blocked", &|a| {
        a.status == "blocked"
            && finding(a).is_some_and(|f| f.0 == Severity::Critical && f.3 == "Upheld")
    });
    find("a critical waived", &|a| {
        a.status == "waived" && finding(a).is_some_and(|f| f.0 == Severity::Critical)
    });
    find("a seal proposed", &|a| finding(a).is_some_and(|f| f.4));
    find("held pending by a torn line, all filed", &|a| {
        a.status == "pending" && a.torn && a.filed == routed
    });
    // And nothing the lifecycle does not intend.
    for (a, path) in &states {
        if let Some((found, now, verdict, standing, sealed)) = &a.finding {
            // A seal is proposed only on a finding confirmed at high or more.
            if *sealed {
                assert!(
                    *now >= Severity::High && *verdict == Some("confirmed"),
                    "{path:?}"
                );
            }
            // A critical is never cleared: only the human's waiver settles it.
            if *found == Severity::Critical {
                assert!(matches!(a.status, "blocked" | "waived"), "{path:?}: {a:?}");
            }
            // Below the floor nothing is kept, so a kept finding is at medium or more.
            assert!(*found >= Severity::Medium);
            let _ = standing;
        }
        if a.torn {
            assert!(
                matches!(a.status, "pending" | "blocked" | "waived"),
                "{path:?}: {a:?}"
            );
        }
    }
}

/// The opening's own states: a landing routed to no speciality is cleared and never recorded; one
/// whose records cannot be kept is refused, and the loop holds it unrecorded, never landed.
#[test]
fn an_opening_is_recorded_cleared_unrouted_or_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let c = Challenges::at(&tmp.path().join("challenges.jsonl"));
    let none = route(&Roster::default(), "run-1:2", &["README.txt".to_owned()]);
    assert_eq!(none.routed, []);
    assert_eq!(c.open(&none), Ok(Status::Cleared));
    assert!(
        c.record(&none.id).is_none(),
        "an unrouted landing was recorded"
    );
    let l = landing();
    assert!(matches!(c.open(&l), Ok(Status::Pending { .. })));
    assert_eq!(c.open(&l), Err(Refusal::AlreadyOpen(l.id.clone())));
    // Records that cannot be kept: a folder where the file should be.
    let dir = tmp.path().join("broken.jsonl");
    fs::create_dir(&dir).unwrap();
    let refused = Challenges::at(&dir).open(&l);
    assert!(matches!(refused, Err(Refusal::Unwritten(_))), "{refused:?}");
}
