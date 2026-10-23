//! isekai's wire tests (`wire/wire_test.go`, `wire/enqueues_test.go`), carried over as
//! fixtures. Where Tempest's wire deliberately differs, the test says so.

use tempest_wire::{
    Ask, Commission, DEFAULT_CAP, Finding, ForgeKind, Hole, Office, Report, UnsaidKind, Word,
    enqueues_of, forges_of,
};

#[test]
fn forge_round_trip() {
    let r = Report::parse(
        "@S DONE\n@FORGE tool:ui-wrangler | rewrite bare /v1 strings safe | python3 wrangle.py {{path}}\n\
         @FORGE skill:ttl-craft\n@FORGE nonsense\n@E 99",
    )
    .expect("on the wire");
    assert_eq!(r.forges.len(), 2);
    let (tool, skill) = (&r.forges[0], &r.forges[1]);
    assert_eq!(tool.kind, ForgeKind::Tool);
    assert_eq!(tool.name, "ui-wrangler");
    assert_eq!(
        tool.text,
        "rewrite bare /v1 strings safe | python3 wrangle.py {{path}}"
    );
    assert_eq!(
        (skill.kind, skill.name.as_str(), skill.text.as_str()),
        (ForgeKind::Skill, "ttl-craft", "")
    );
    assert_eq!(
        r.other,
        ["@FORGE nonsense"],
        "a draft with no kind is prose, not poison"
    );
    let back = Report::parse(&r.to_string()).expect("round trip");
    assert_eq!(back.forges, r.forges);
    assert_eq!(forges_of(&r.to_string()).len(), 2);
}

#[test]
fn commission() {
    let c = Commission::parse(
        "@ROOT /w/x\n@SCOPE pkgs a, b\n@ASK draft +unsaid wire:raw\n@CAP 512\n@DUMP .isekai/tmp/out.md\n\
         @SIZE 4000\nBuild the thing.\n\nSecond line.",
    );
    assert_eq!(c.root.as_deref(), Some("/w/x"));
    assert_eq!(c.scope.as_deref(), Some("pkgs a, b"));
    assert_eq!(c.ask, Ask::Office(Office::Draft));
    assert!(c.unsaid && c.raw);
    assert_eq!((c.cap, c.size), (Some(512), Some(4000)));
    assert_eq!(c.dump.as_deref(), Some(".isekai/tmp/out.md"));
    assert_eq!(c.body.len(), 2);
    assert_eq!(
        Commission::parse("just prose").cap_or_default(),
        DEFAULT_CAP
    );
    assert_eq!(Commission::parse(&c.to_string()), c);
}

#[test]
fn report() {
    let r = Report::parse(
        "Some preamble.\n@S PASS 3 checks\n@F a.go:12 the fact\n@V claim holds — evidence\n@? what about x\n\
         @U colony the colony knows\n@U untyped thing\n@T mind x — path — load≈10tok\n@E 123\n",
    )
    .expect("on the wire");
    assert_eq!(r.status, "PASS 3 checks");
    assert_eq!(r.word(), Word::Pass);
    assert_eq!(r.findings[0].at.as_deref(), Some("a.go:12"));
    assert_eq!(r.findings[0].claim, "the fact");
    assert_eq!((r.verdicts.len(), r.tools.len()), (1, 1));
    assert_eq!(r.holes, [Hole::Other("what about x".into())]);
    assert_eq!(r.bytes, Some(123));
    assert_eq!(r.other, ["Some preamble."]);
    assert_eq!(r.unsaid[0].kind, Some(UnsaidKind::Colony));
    assert_eq!(r.unsaid[0].text, "the colony knows");
    assert_eq!(
        (r.unsaid[1].kind, r.unsaid[1].text.as_str()),
        (None, "untyped thing")
    );

    assert!(
        Report::parse("no envelope here").is_none(),
        "no @S means not on the wire"
    );
    assert!(
        Report::parse("@Something odd").is_none(),
        "unknown tags are not @S"
    );

    // Tempest differs: the raw form is read only when the commission granted `wire:raw`.
    assert!(Report::parse("S|DONE\nF|a.go:3|one fact").is_none());
    let raw = Report::parse_raw("S|DONE\nF|a.go:3|one fact\n?|a hole\nE|40").expect("raw form");
    assert_eq!(raw.word(), Word::Done);
    assert_eq!(raw.findings[0].at.as_deref(), Some("a.go:3"));
    assert_eq!(raw.findings[0].claim, "one fact");
    assert_eq!(raw.holes, [Hole::Other("a hole".into())]);
    assert_eq!(raw.bytes, Some(40));
}

#[test]
fn emit_and_cap() {
    let r = Report::parse("@S PASS\n@F a.go:1 one\n@? h\n@U law l").expect("on the wire");
    let out = r.to_string();
    assert!(
        out.starts_with("@S PASS\n@? h\n@U law l\n@F a.go:1 one\n@E "),
        "order: {out:?}"
    );
    assert!(
        Report::is_whole(&out),
        "@E must equal the byte length: {out:?}"
    );

    let mut big = Report::parse("@S PASS\n@U territory kept\n@? kept too").expect("on the wire");
    for i in 0..100 {
        big.findings.push(Finding::parse(&format!(
            "f{i}.go:{i} a finding of some length"
        )));
    }
    for cap in [200, 500, 1000, 2048] {
        let out = big.emit(cap, Some(".tempest/tmp/full.md"));
        assert!(out.len() <= cap, "cap {cap}: {} bytes", out.len());
        for must in [
            "@U territory kept",
            "@? kept too",
            &format!("@? @CAP {cap}: "),
            "full report at .tempest/tmp/full.md",
        ] {
            assert!(out.contains(must), "cap {cap}: {must:?} missing in {out:?}");
        }
        assert!(Report::is_whole(&out), "cap {cap}: @E wrong");
    }
    assert!(big.emit(0, None).len() > 3000, "cap 0 means no ceiling");
    let tiny = Report::parse("@S PASS")
        .expect("on the wire")
        .emit(10, None);
    assert!(
        tiny.starts_with("@S PASS"),
        "a report that cannot fit still opens with @S: {tiny:?}"
    );
}

#[test]
fn enqueue_tag() {
    let r = Report::parse_raw(
        "@S DONE\n@$ orc-auth: gate the login flow\n$|slime-db: migrate the users table\n\
         @$ stray line with no body\n@E 96",
    )
    .expect("on the wire");
    assert_eq!(r.enqueues.len(), 2);
    assert_eq!(
        (r.enqueues[0].body.as_str(), r.enqueues[0].ask.as_str()),
        ("orc-auth", "gate the login flow")
    );
    assert_eq!(
        (r.enqueues[1].body.as_str(), r.enqueues[1].ask.as_str()),
        ("slime-db", "migrate the users table")
    );
    assert_eq!(
        r.other,
        ["@$ stray line with no body"],
        "a colonless @$ is prose"
    );
    let again = Report::parse(&r.to_string()).expect("round trip");
    assert_eq!(again.enqueues, r.enqueues);
}

#[test]
fn enqueues_of_any_text() {
    let text = "findings are strong\n\n@$ slime-tui-board: land the fixtures per the fence\n\
                @$ ring-wraith: cross the ford with the findings\n@E 0\n";
    let ords = enqueues_of(text);
    assert_eq!(ords.len(), 2);
    assert_eq!(ords[0].body, "slime-tui-board");
    assert_eq!(ords[0].ask, "land the fixtures per the fence");
    assert_eq!(ords[1].body, "ring-wraith");
    assert_eq!(enqueues_of("@$ missing colon\n@$ \nplain prose"), []);
    // Tempest differs: an order's body is one word, so prose with a colon is not an order.
    assert_eq!(enqueues_of("@$ note this: it matters"), []);
}
