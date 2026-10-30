//! Territory facts: statuses read by the table, keyed facts countering by code, evidence that
//! moves making a fact stale, an unlabelled collision left untriaged, and the projection.

use std::fs;
use std::path::PathBuf;

use mnemorph_world::fact_findings;
use mnemorph_world::facts::{Anchor, Event, Facts, Label, Status, digest, fact_id, from_unsaid};
use std::path::Path;

fn say(about: &str, text: &str, by: &str) -> Event {
    Event::Support {
        id: fact_id(about, text),
        about: about.into(),
        text: text.into(),
        by: by.into(),
        source: None,
    }
}

fn counter(about: &str, text: &str, by: &str) -> Event {
    Event::Counter {
        id: fact_id(about, text),
        by: by.into(),
        reason: "no".into(),
    }
}

fn status(events: &[Event], about: &str, text: &str) -> Status {
    Facts::fold(events).facts[&fact_id(about, text)].status()
}

#[test]
fn the_status_table() {
    let f = ("src/", "the parser is hand-written");
    assert_eq!(status(&[say(f.0, f.1, "a")], f.0, f.1), Status::Single);
    assert_eq!(
        status(&[say(f.0, f.1, "a"), say(f.0, f.1, "b")], f.0, f.1),
        Status::Agreed
    );
    let two_one = [
        say(f.0, f.1, "a"),
        say(f.0, f.1, "b"),
        counter(f.0, f.1, "c"),
    ];
    // Two to one: one dissenter in three is tolerated.
    assert_eq!(status(&two_one, f.0, f.1), Status::Agreed);
    assert_eq!(
        status(&[say(f.0, f.1, "a"), counter(f.0, f.1, "c")], f.0, f.1),
        Status::Contested
    );
    let one_two = [
        say(f.0, f.1, "a"),
        counter(f.0, f.1, "c"),
        counter(f.0, f.1, "d"),
    ];
    assert_eq!(status(&one_two, f.0, f.1), Status::Countered);
    let settled = [
        say(f.0, f.1, "a"),
        counter(f.0, f.1, "c"),
        Event::Settle {
            id: fact_id(f.0, f.1),
            by: "orc-o".into(),
            proof: "verdict".into(),
        },
    ];
    assert_eq!(status(&settled, f.0, f.1), Status::Settled);
    let reopened = [settled.to_vec(), vec![counter(f.0, f.1, "d")]].concat();
    assert_ne!(
        status(&reopened, f.0, f.1),
        Status::Settled,
        "a newer counter reopens it"
    );
}

/// The exit's case: two creatures write different `build=` values for one path, neither naming
/// the other, and the fact is contested; an inherited key collides from a path above.
#[test]
fn keyed_facts_counter_by_code() {
    let events = [
        say("src/wire/", "build=cargo build -p wire", "slime-a"),
        say("src/wire/", "build=make wire", "slime-b"),
    ];
    assert_eq!(
        status(&events, "src/wire/", "build=cargo build -p wire"),
        Status::Contested
    );
    assert_eq!(
        status(&events, "src/wire/", "build=make wire"),
        Status::Contested
    );
    let events = [
        say("src/", "generated=yes", "a"),
        say("src/gen/", "generated=no", "b"),
    ];
    assert_eq!(
        status(&events, "src/gen/", "generated=no"),
        Status::Contested
    );
    // `build` is not inherited: a path below may build its own way.
    let events = [
        say("src/", "build=make", "a"),
        say("src/x/", "build=cargo build", "b"),
    ];
    assert_eq!(
        status(&events, "src/x/", "build=cargo build"),
        Status::Single
    );
}

struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The exit's case: a fact whose cited lines change reads stale.
#[test]
fn a_fact_whose_cited_lines_change_reads_stale() {
    let w =
        World(std::env::temp_dir().join(format!("mnemorph-facts-stale-{}", std::process::id())));
    fs::create_dir_all(w.0.join("src")).unwrap();
    fs::write(w.0.join("src/a.rs"), "fn one() {}\nfn two() {}\n").unwrap();
    let anchor = Anchor {
        at: "src/a.rs:2".into(),
        digest: digest(&w.0, "src/a.rs:2"),
    };
    let e = Event::Support {
        id: fact_id("src/a.rs", "two is the entry"),
        about: "src/a.rs".into(),
        text: "two is the entry".into(),
        by: "slime-a".into(),
        source: Some(anchor),
    };
    let facts = Facts::fold(&[e]);
    assert_eq!(fact_findings(&facts, &w.0), Vec::new());
    fs::write(w.0.join("src/a.rs"), "fn one() {}\nfn deux() {}\n").unwrap();
    let found = fact_findings(&facts, &w.0);
    assert_eq!(
        found.iter().map(|f| f.code).collect::<Vec<_>>(),
        ["stale-fact"]
    );
}

/// The exit's case: a free fact resembling an older one reaches the parent as a candidate, and
/// left unlabeled raises `untriaged-fact`; labelled `support`, it merges.
#[test]
fn a_collision_reaches_the_parent_and_unlabelled_is_untriaged() {
    let old = say("src/wire/", "the parser rejects tabs", "slime-a");
    let new = say(
        "src/wire/parse.rs",
        "the wire parser rejects tab characters",
        "slime-b",
    );
    let facts = Facts::fold(&[old.clone(), new.clone()]);
    let fresh = &facts.facts[&fact_id(
        "src/wire/parse.rs",
        "the wire parser rejects tab characters",
    )];
    let shown = facts.candidates(fresh, 0.1);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].text, "the parser rejects tabs");
    let codes: Vec<_> = fact_findings(&facts, std::path::Path::new("/nonexistent"))
        .iter()
        .map(|f| f.code)
        .collect();
    assert_eq!(codes, ["untriaged-fact"]);
    let label = Event::Label {
        new: fact_id(
            "src/wire/parse.rs",
            "the wire parser rejects tab characters",
        ),
        label: Label::Support,
        of: Some(fact_id("src/wire/", "the parser rejects tabs")),
        by: "orc-o".into(),
    };
    let facts = Facts::fold(&[old, new, label]);
    assert_eq!(
        fact_findings(&facts, std::path::Path::new("/nonexistent")),
        Vec::new()
    );
    assert_eq!(
        facts.facts[&fact_id("src/wire/", "the parser rejects tabs")].status(),
        Status::Agreed
    );
}

#[test]
fn the_wire_form_and_the_projection() {
    assert!(matches!(
        from_unsaid("./README.md build=just", "a", None),
        Some(Event::Support { .. })
    ));
    assert!(
        matches!(from_unsaid("src/ !#abc123 it moved", "a", None), Some(Event::Counter { ref id, .. }) if id == "abc123")
    );
    assert_eq!(
        from_unsaid("parser is slow", "a", None),
        None,
        "the first word is no path"
    );
    let events = [
        say("src/", "old note", "a"),
        say("src/", "the build is cargo", "a"),
        counter("src/", "the build is cargo", "b"),
        say("src/", "newest note", "c"),
    ];
    let lines = Facts::fold(&events).projection(
        std::path::Path::new("/nonexistent"),
        |_| "slime-a".into(),
        &["src/x.rs"],
        2,
    );
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with("@ONTO src/x.rs: slime-a · #"),
        "{lines:?}"
    );
    assert!(
        lines[0].contains("contested the build is cargo (countered by b: no)"),
        "contested first: {lines:?}"
    );
    assert!(
        lines[0].contains("newest note") && !lines[0].contains("old note"),
        "{lines:?}"
    );
    assert_eq!(lines[1], "@ONTO -1");
}

/// What the review of facts found, each held.
#[test]
fn the_facts_review_held() {
    let nowhere = Path::new("/nonexistent");
    // `src` and `src/` are one path: different `build=` values collide, the same words agree.
    let events = [
        say("src", "build=make", "a"),
        say("src/", "build=cargo build", "b"),
    ];
    assert_eq!(status(&events, "src", "build=make"), Status::Contested);
    assert_eq!(fact_id("src", "x y"), fact_id("src/", "x  y"));
    // A path outside the world is no fact, and two such never share an id.
    assert_eq!(from_unsaid("../x is here", "a", None), None);
    assert_ne!(fact_id("/a", "x"), fact_id("/b", "x"));
    // A support or a counter reaching a merged fact counts on the fact it was merged into.
    let x = ("src/", "the parser rejects tabs");
    let y = ("src/p.rs", "the parser refuses tab characters");
    let merged = Event::Label {
        new: fact_id(y.0, y.1),
        label: Label::Support,
        of: Some(fact_id(x.0, x.1)),
        by: "orc".into(),
    };
    let events = [
        say(x.0, x.1, "a"),
        say(y.0, y.1, "b"),
        merged,
        say(y.0, y.1, "c"),
        counter(y.0, y.1, "d"),
    ];
    let facts = Facts::fold(&events);
    let into = &facts.facts[&fact_id(x.0, x.1)];
    assert_eq!(into.supporters.len(), 3, "{into:?}");
    assert_eq!(into.counters.len(), 1, "{into:?}");
    // A `counter` label never puts one creature on both sides.
    let lab = Event::Label {
        new: fact_id(y.0, y.1),
        label: Label::Counter,
        of: Some(fact_id(x.0, x.1)),
        by: "orc".into(),
    };
    let facts = Facts::fold(&[say(x.0, x.1, "a"), say(y.0, y.1, "a"), lab]);
    let f = &facts.facts[&fact_id(x.0, x.1)];
    assert!(f.supporters.is_disjoint(&f.counters), "{f:?}");
    // Untriaged only when something was shown to the parent.
    let facts = Facts::fold(&[
        say("src/", "old note about tabs", "a"),
        say("src/x.rs", "uses the tokio runtime", "b"),
    ]);
    assert_eq!(fact_findings(&facts, nowhere), Vec::new());
    // A settled keyed fact is reopened by a later colliding assertion.
    let settle = Event::Settle {
        id: fact_id("src/", "build=make"),
        by: "orc".into(),
        proof: "verdict".into(),
    };
    let events = [
        say("src/", "build=make", "a"),
        settle,
        say("src/", "build=just", "b"),
    ];
    assert_eq!(status(&events, "src/", "build=make"), Status::Contested);
    // A creature withdrawing its own only fact: countered, not contested.
    assert_eq!(
        status(
            &[say("src/", "z", "a"), counter("src/", "z", "a")],
            "src/",
            "z"
        ),
        Status::Countered
    );
    // One keyed value however it is written.
    assert_eq!(
        fact_id("src/", "build=./build.sh"),
        fact_id("src/", "build = build.sh")
    );
    let events = [
        say("src/", "deprecated=by ./src/new.rs", "a"),
        say("src/", "deprecated=by src/new.rs", "b"),
    ];
    assert_eq!(
        status(&events, "src/", "deprecated=by src/new.rs"),
        Status::Agreed
    );
    // The projection names the counter's reason, counts each fact cut once, and leaves no dangle.
    let events = [
        say("src/", "one", "a"),
        say("src/", "two", "a"),
        say("src/", "three", "a"),
        Event::Counter {
            id: fact_id("src/", "three"),
            by: "b".into(),
            reason: "it moved".into(),
        },
    ];
    let lines =
        Facts::fold(&events).projection(nowhere, |_| "o".into(), &["src/", "src/x.rs", "docs/"], 2);
    assert!(lines[0].contains("countered by b: it moved"), "{lines:?}");
    assert_eq!(lines[2], "@ONTO docs/: o");
    assert_eq!(lines[3], "@ONTO -1", "{lines:?}");
}

/// The parent's triage, read from its `@V` verdicts; anything else is no label.
#[test]
fn a_triage_label_is_read_from_a_verdict() {
    use mnemorph_world::facts::{Event, Label, label_of};
    let label = |v: &str| match label_of(v, "orc-o") {
        Some(Event::Label { new, label, of, by }) => Some((new, label, of, by)),
        _ => None,
    };
    assert_eq!(
        label("#aa counter #bb"),
        Some((
            "aa".into(),
            Label::Counter,
            Some("bb".into()),
            "orc-o".into()
        ))
    );
    assert_eq!(
        label("#aa distinct"),
        Some(("aa".into(), Label::Distinct, None, "orc-o".into()))
    );
    for bad in [
        "#aa support",
        "#aa distinct #bb",
        "aa counter #bb",
        "#aa merges #bb",
        "#aa refines #bb extra",
        "the landing holds",
    ] {
        assert_eq!(label(bad), None, "{bad}");
    }
}

/// A fact said with no anchor of its own is anchored where it is about: when that file changes,
/// the fact reads stale instead of standing as true.
#[test]
fn a_fact_is_anchored_where_it_is_about() {
    use mnemorph_world::facts::anchored;
    let w = std::env::temp_dir().join(format!("mnemorph-anchored-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join("src")).unwrap();
    fs::write(w.join("src/lexer.rs"), "fn lex() {}\n").unwrap();
    let said = from_unsaid("src/lexer.rs the lexer is pure", "slime-s", None).unwrap();
    let event = anchored(&w, said);
    let Event::Support {
        source: Some(a), ..
    } = &event
    else {
        panic!("{event:?}");
    };
    assert_eq!(a.at, "src/lexer.rs");
    assert!(a.digest.is_some(), "a file is anchored by its text");
    Facts::record(&w, &event).unwrap();
    let fresh = Facts::load(&w);
    assert!(!fresh.live().any(|f| f.stale(&w)), "unchanged: not stale");
    fs::write(w.join("src/lexer.rs"), "fn lex() { panic!() }\n").unwrap();
    assert!(
        Facts::load(&w).live().any(|f| f.stale(&w)),
        "its file moved: stale"
    );
    let _ = fs::remove_dir_all(&w);
}

/// The cases of the triage fixture: on one path, the older fact a new one restates, six older
/// facts that share its words but say something else, and the new fact, reworded.
const TRIAGE_CASES: &[(&str, &str, [&str; 6], &str)] = &[
    (
        "src/wire/",
        "the parser rejects tabs",
        [
            "the parser accepts trailing commas",
            "the lexer rejects unicode escapes in identifiers",
            "tabs in strings are kept verbatim",
            "the parser keeps comments",
            "the wire format rejects empty frames",
            "tab width is four in the formatter",
        ],
        "the wire parser refuses tab characters",
    ),
    (
        "src/db/",
        "migrations run inside one transaction",
        [
            "each migration has a down script",
            "the connection pool holds ten connections",
            "transactions retry twice on deadlock",
            "migrations are numbered by date",
            "one migration failed in staging",
            "the transaction log is rotated daily",
        ],
        "all migrations execute in a single transaction",
    ),
    (
        "tests/",
        "the integration tests need docker running",
        [
            "unit tests run without network",
            "docker images are pinned by digest",
            "the test suite takes four minutes",
            "tests running in docker share one network",
            "the integration suite needs a seeded database",
            "flaky tests are retried once",
        ],
        "integration tests require a running docker daemon",
    ),
    (
        "src/api/",
        "the api returns dates in utc",
        [
            "the api paginates at 50 items",
            "timestamps in logs are local time",
            "the api rejects requests without a token",
            "the api returns errors as json",
            "dates in the ui are shown in local time",
            "the api version is in the url",
        ],
        "dates from the api are utc",
    ),
    (
        "src/cache/",
        "cache entries expire after one hour",
        [
            "the cache is keyed by user id",
            "cache misses fall back to the database",
            "the hour column is stored as an integer",
            "cache entries are compressed",
            "the cache holds at most ten thousand entries",
            "sessions expire after one day",
        ],
        "cached entries live for sixty minutes",
    ),
    (
        "src/build/",
        "release builds strip debug symbols",
        [
            "debug builds enable assertions",
            "the build caches dependencies",
            "symbols are uploaded to the crash reporter",
            "release builds are signed",
            "the debug build is twice as slow",
            "builds strip the vendored tests",
        ],
        "debug symbols are stripped from release builds",
    ),
    (
        "src/auth/",
        "passwords are hashed with argon2",
        [
            "tokens are signed with ed25519",
            "the password minimum length is twelve",
            "hashes are compared in constant time",
            "passwords are never logged",
            "the hash of each file is stored",
            "argon2 parameters come from config",
        ],
        "argon2 hashes every password",
    ),
    (
        "config/",
        "the default port is 8080",
        [
            "the port can be set from the environment",
            "tls is on by default",
            "the admin port is 9090",
            "the default log level is info",
            "the port is checked at startup",
            "the default timeout is thirty seconds",
        ],
        "8080 is the default port",
    ),
];

/// `Pr[old fact not in top 3]`, the second term of the bound on a silent contradiction (09): over
/// the fixture, how often the older fact a new one restates is missing from the three a parent is
/// shown. Measured, not assumed; the doc quotes the rate this asserts.
#[test]
fn the_true_duplicate_is_shown_for_triage() {
    use mnemorph_world::facts::THETA_T;
    let (mut missed, mut ranks) = (Vec::new(), Vec::new());
    for (path, old, others, new) in TRIAGE_CASES {
        let mut events: Vec<Event> = others
            .iter()
            .enumerate()
            .map(|(i, t)| say(path, t, &format!("slime-{i}")))
            .collect();
        events.push(say(path, old, "slime-old"));
        events.push(say(path, new, "slime-new"));
        let facts = Facts::fold(&events);
        let fresh = &facts.facts[&fact_id(path, new)];
        let shown: Vec<&str> = facts
            .candidates(fresh, THETA_T)
            .iter()
            .map(|f| f.text.as_str())
            .collect();
        match shown.iter().position(|t| t == old) {
            Some(rank) => ranks.push(rank + 1),
            None => missed.push(format!("{new} → {shown:?}")),
        }
    }
    eprintln!(
        "Pr[old not in top 3] = {}/{}, the duplicate's ranks {ranks:?}",
        missed.len(),
        TRIAGE_CASES.len()
    );
    assert!(missed.is_empty(), "{missed:#?}");
    // Docs 09 quote these ranks: a change here is a change there.
    assert_eq!(ranks, [3, 1, 1, 1, 2, 1, 3, 1]);
}

/// A fact an event leaves contested moves the memory claims resting on it: each is marked for
/// review, once — a later event that keeps it contested marks nothing again.
#[test]
fn a_fact_newly_contested_marks_the_claims_resting_on_it() {
    use mnemorph_world::memory::{Change, Dependency, Tier, fold, propose_resting, read};
    let w = std::env::temp_dir().join(format!("mnemorph-fact-review-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    let f = ("src/", "the parser is hand-written");
    let id = fact_id(f.0, f.1);
    let claim = propose_resting(
        &w,
        "slime-s",
        "the grammar is hand-written too",
        vec![],
        None,
        vec![],
        vec![Dependency::Fact { id: id.clone() }],
    )
    .unwrap();
    let tier = Tier::Personal("slime-s".into());
    let reviews = || {
        read(&w, &tier)
            .unwrap()
            .rows
            .iter()
            .filter(|e| matches!(e.change, Change::Review { .. }))
            .count()
    };
    Facts::record(&w, &say(f.0, f.1, "a")).unwrap();
    Facts::record(&w, &say(f.0, f.1, "b")).unwrap();
    assert_eq!(reviews(), 0, "agreed: nothing moved");
    Facts::record(&w, &counter(f.0, f.1, "c")).unwrap();
    Facts::record(&w, &counter(f.0, f.1, "d")).unwrap();
    assert_eq!(Facts::load(&w).facts[&id].status(), Status::Contested);
    let note = fold(&read(&w, &tier).unwrap().rows)
        .remove(&claim.note)
        .unwrap();
    assert!(
        note.review
            .as_deref()
            .is_some_and(|c| c.contains(&id) && c.contains("contested")),
        "{:?}",
        note.review
    );
    let marked = reviews();
    Facts::record(&w, &counter(f.0, f.1, "e")).unwrap();
    Facts::record(&w, &say(f.0, f.1, "g")).unwrap();
    assert_eq!(reviews(), marked, "still contested: marked once");
    let _ = fs::remove_dir_all(&w);
}

/// A settle names the digest of the fact the human read: a fact that reads otherwise now is
/// refused and nothing is filed; as it reads, it is settled.
#[test]
fn a_settle_of_a_fact_that_reads_otherwise_is_refused() {
    use mnemorph_world::memory::{changed, seen};
    let w = std::env::temp_dir().join(format!("mnemorph-settle-seen-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(&w).unwrap();
    let (about, text) = ("src/", "src holds the code");
    Facts::record(&w, &say(about, text, "slime-a")).unwrap();
    let id = fact_id(about, text);
    let file = w.join(".mnemorph/ontology/facts.jsonl");
    let before = fs::read_to_string(&file).unwrap();
    assert_eq!(
        Facts::settle(
            &w,
            &id,
            "human",
            "the human's word",
            &seen(&id, "src is empty")
        ),
        Err(changed(&format!("#{id}")))
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), before, "nothing filed");
    assert_eq!(
        Facts::settle(&w, &id, "human", "the human's word", &seen(&id, text)),
        Ok(id.clone())
    );
    assert_eq!(Facts::load(&w).facts[&id].status(), Status::Settled);
    let _ = fs::remove_dir_all(&w);
}
