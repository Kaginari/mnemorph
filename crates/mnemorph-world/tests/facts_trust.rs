//! Where a fact's trust comes from: the chained journal under the human's state directory (a
//! temporary one here), never the world's `.mnemorph/ontology/facts.jsonl`. A settle forged in the
//! in-world copy — committed, or brought by a clone — is ignored and flagged; a world with no
//! trusted journal imports its copy once, unsettled; a trusted journal that does not walk whole
//! leaves the facts unavailable, never settled.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mnemorph_world::fact_findings;
use mnemorph_world::facts::{Event, FACTS, Facts, IMPORTED, KIND, Status, fact_id, trusted_in};
use mnemorph_world::memory::seen;

const ABOUT: &str = "src/";
const TEXT: &str = "the parser is hand-written";

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        Self { _tmp: tmp, root }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    /// A world that keeps a journal, a git repository from its first commit.
    fn world(&self, name: &str) -> PathBuf {
        let w = self.root.join(name);
        fs::create_dir_all(w.join(".mnemorph/ontology")).unwrap();
        fs::create_dir_all(w.join("src")).unwrap();
        fs::write(w.join(".mnemorph/log.md"), "# log\n").unwrap();
        git(&w, &["init", "-q"]);
        commit(&w, "world");
        w
    }

    fn load(&self, world: &Path) -> Facts {
        Facts::load_in(Some(&self.state()), world)
    }
}

fn git(world: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(world)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

fn commit(world: &Path, message: &str) {
    git(world, &["add", "-A"]);
    git(world, &["commit", "-q", "--allow-empty", "-m", message]);
}

fn say(by: &str) -> Event {
    Event::Support {
        id: fact_id(ABOUT, TEXT),
        about: ABOUT.into(),
        text: TEXT.into(),
        by: by.into(),
        source: None,
    }
}

fn human_settle() -> Event {
    Event::Settle {
        id: fact_id(ABOUT, TEXT),
        by: "human".into(),
        proof: "the human's word".into(),
    }
}

/// `events` appended to the in-world copy as plain lines, as a stranger would write them.
fn forge(world: &Path, events: &[Event]) {
    let mut text = fs::read_to_string(world.join(FACTS)).unwrap_or_default();
    for e in events {
        text.push_str(&serde_json::to_string(e).unwrap());
        text.push('\n');
    }
    fs::write(world.join(FACTS), text).unwrap();
}

fn codes(facts: &Facts, world: &Path) -> Vec<&'static str> {
    fact_findings(facts, world).iter().map(|f| f.code).collect()
}

/// A body's commit of an edited copy persists in git and comes back on checkout: the settle it
/// forged "by the human" is ignored, and flagged high on the copy alone. The human's real settle
/// lands in the state directory and is trusted, and the copy is written again from it.
#[test]
fn a_forged_human_settle_in_a_committed_copy_is_ignored_and_flagged() {
    let f = Fixture::new();
    let w = f.world("w");
    let id = fact_id(ABOUT, TEXT);
    Facts::record_in(Some(&f.state()), &w, &say("slime-a")).unwrap();
    assert!(trusted_in(&f.state(), &w).is_file());
    assert!(!trusted_in(&f.state(), &w).starts_with(&w));
    // The copy reads as the trusted journal: nothing to flag.
    assert_eq!(f.load(&w).untrusted, vec![]);

    forge(&w, &[say("slime-b"), human_settle()]);
    commit(&w, "a body's edit");
    fs::remove_file(w.join(FACTS)).unwrap();
    git(&w, &["checkout", "--", FACTS]);
    assert!(
        fs::read_to_string(w.join(FACTS))
            .unwrap()
            .contains("\"settle\"")
    );

    let facts = f.load(&w);
    assert_eq!(facts.facts[&id].status(), Status::Single, "never settled");
    assert_eq!(
        facts.facts[&id].supporters.len(),
        1,
        "the forged voice is none"
    );
    assert_eq!(facts.untrusted.len(), 1, "{:?}", facts.untrusted);
    let u = &facts.untrusted[0];
    assert_eq!(
        (u.said, u.trusted, u.settled_by.as_deref()),
        (Status::Settled, Some(Status::Single), Some("human"))
    );
    let findings = fact_findings(&facts, &w);
    let flag = findings
        .iter()
        .find(|f| f.code == "untrusted-fact")
        .unwrap();
    assert!(flag.severity.blocks());
    assert_eq!(
        (flag.at.as_str(), &flag.paths[..]),
        (FACTS, &[FACTS.to_owned()][..])
    );
    assert!(
        flag.creatures.is_empty(),
        "no creature's landing is held by it"
    );
    assert!(flag.message.contains("settled") && flag.message.contains("`human`"));

    // The human's settle, as `onto settle` files it.
    let settled = Facts::settle_in(
        Some(&f.state()),
        &w,
        &id,
        "human",
        "the human's word",
        &seen(&id, TEXT),
    );
    assert_eq!(settled, Ok(id.clone()));
    let facts = f.load(&w);
    assert_eq!(facts.facts[&id].status(), Status::Settled);
    assert_eq!(
        facts.untrusted,
        vec![],
        "the copy is written again from the trusted journal"
    );
    assert!(
        fs::read_to_string(trusted_in(&f.state(), &w))
            .unwrap()
            .contains("\"settle\"")
    );
}

/// A cloned repository's `.mnemorph/` is a stranger's from the start: on the first read its facts
/// are imported once, each as one unproven voice, unsettled, and the settles it held are named for
/// the human to give again — in a finding, and in the world's journal.
#[test]
fn a_cloned_worlds_forged_settles_are_imported_unsettled_once() {
    let f = Fixture::new();
    let origin = f.world("origin");
    forge(&origin, &[say("slime-a"), say("slime-b"), human_settle()]);
    commit(&origin, "hostile facts");
    let clone = f.root.join("clone");
    let ok = Command::new("git")
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(&clone)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let id = fact_id(ABOUT, TEXT);

    let facts = f.load(&clone);
    let fact = &facts.facts[&id];
    assert_eq!(
        fact.status(),
        Status::Single,
        "two forged voices are one unproven"
    );
    assert_eq!(fact.supporters.iter().collect::<Vec<_>>(), [IMPORTED]);
    assert!(facts.unproven.contains(&id));
    assert_eq!(facts.untrusted, vec![], "the copy now reads as the import");
    assert_eq!(
        codes(&facts, &clone),
        ["unproven-settle"],
        "{:?}",
        fact_findings(&facts, &clone)
    );
    let log = fs::read_to_string(clone.join(".mnemorph/log.md")).unwrap();
    assert!(
        log.contains("imported, unsettled") && log.contains(&format!("#{id}")),
        "{log}"
    );
    assert!(log.contains("mnemorphd onto settle"), "{log}");

    // Once: a second read imports nothing more.
    let journal = trusted_in(&f.state(), &clone);
    let rows = fs::read_to_string(&journal).unwrap();
    let again = f.load(&clone);
    assert_eq!(fs::read_to_string(&journal).unwrap(), rows);
    assert_eq!(again.facts[&id].status(), Status::Single);

    // The human settles it again, and the note goes.
    Facts::settle_in(
        Some(&f.state()),
        &clone,
        &id,
        "human",
        "the human's word",
        &seen(&id, TEXT),
    )
    .unwrap();
    let facts = f.load(&clone);
    assert_eq!(facts.facts[&id].status(), Status::Settled);
    assert_eq!(codes(&facts, &clone), Vec::<&str>::new());
}

/// A trusted journal edited by hand, or removed once anchored, does not walk whole: no fact stands,
/// settled or not; nothing is filed onto it; and a removed one is never imported afresh.
#[test]
fn a_tampered_trusted_journal_reads_unavailable_never_settled() {
    let f = Fixture::new();
    let w = f.world("w");
    let id = fact_id(ABOUT, TEXT);
    let state = f.state();
    Facts::record_in(Some(&state), &w, &say("slime-a")).unwrap();
    Facts::settle_in(Some(&state), &w, &id, "human", "word", &seen(&id, TEXT)).unwrap();
    assert_eq!(f.load(&w).facts[&id].status(), Status::Settled);

    let journal = trusted_in(&state, &w);
    let text = fs::read_to_string(&journal).unwrap();
    fs::write(&journal, text.replace("slime-a", "slime-z")).unwrap();
    let facts = f.load(&w);
    assert!(facts.facts.is_empty(), "{:?}", facts.facts);
    assert!(
        facts
            .unavailable
            .as_deref()
            .is_some_and(|w| w.contains("do not verify")),
        "{:?}",
        facts.unavailable
    );
    assert_eq!(codes(&facts, &w), ["facts-unavailable"]);
    assert!(fact_findings(&facts, &w)[0].severity.blocks());
    assert!(Facts::record_in(Some(&state), &w, &say("slime-b")).is_err());
    assert!(Facts::settle_in(Some(&state), &w, &id, "human", "word", &seen(&id, TEXT)).is_err());
    assert_eq!(
        fs::read_to_string(&journal).unwrap(),
        text.replace("slime-a", "slime-z")
    );

    // Put back, anchored, then removed: the anchor finds it gone; the copy is not imported.
    fs::write(&journal, &text).unwrap();
    let head = mnemorph_world::ledger::chain::verify(&journal)
        .unwrap()
        .head
        .unwrap();
    let mut log = fs::read_to_string(w.join(".mnemorph/log.md")).unwrap();
    log.push_str(&format!(
        "\n- **{KIND} chain:** OK, head {} at line {}\n",
        head.digest, head.line
    ));
    fs::write(w.join(".mnemorph/log.md"), log).unwrap();
    assert_eq!(f.load(&w).facts[&id].status(), Status::Settled);
    fs::remove_file(&journal).unwrap();
    let facts = f.load(&w);
    assert!(
        facts.facts.is_empty() && facts.unavailable.is_some(),
        "{facts:?}"
    );
    assert!(!journal.exists(), "never imported afresh");
}

/// With no home there is no trusted journal: nothing stands, and nothing is filed.
#[test]
fn with_no_home_the_facts_are_unavailable() {
    let f = Fixture::new();
    let w = f.world("w");
    forge(&w, &[say("slime-a"), human_settle()]);
    let facts = Facts::load_in(None, &w);
    assert!(facts.facts.is_empty() && facts.unavailable.is_some());
    assert!(Facts::record_in(None, &w, &say("slime-b")).is_err());
}
