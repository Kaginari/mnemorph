//! The seeded fixture (`fixtures/seeded/`, keyed by `fixtures/seeded.toml`), held to the Dark Elf
//! machinery with no model: each plant routes to its speciality and loads its class's playbook; a
//! scripted report naming every plant is accepted, each finding valid with the fingerprint the
//! harness derives; a coverage ledger built from the key holds for every landing; and each
//! negative control is ruled out where it is routed, or routed nowhere.

#[path = "support/seeded.rs"]
mod seeded;

use std::collections::{BTreeMap, BTreeSet};

use mnemorph_proving::coverage::{Sources, State, validate_in};
use mnemorph_proving::{Tree, accept, validate};
use mnemorph_races::{Speciality, brief, for_landing, route};
use mnemorph_wire::Report;

use seeded::{Manifest, finding_line, landing, ledger, looked, manifest, names, prove, tree};

/// The key itself: ids unique, at least one plant per hunter, every class owned by its
/// speciality, every file present in the world, every symbol named in its file, and nothing in
/// the world that could be a real secret or run.
#[test]
fn the_key_matches_the_world() {
    let m = manifest();
    let t = tree();
    let mut ids = BTreeSet::new();
    for p in &m.plant {
        assert!(ids.insert(p.id.clone()), "{} twice", p.id);
        assert!(p.speciality().classes().contains(&p.class()), "{}", p.id);
        let text = String::from_utf8(t[&p.file].clone()).expect("utf-8");
        assert!(
            names(&text, &p.symbol),
            "{}: {} in {}",
            p.id,
            p.symbol,
            p.file
        );
    }
    for c in &m.control {
        assert!(ids.insert(c.id.clone()), "{} twice", c.id);
        assert!(c.speciality().classes().contains(&c.class()), "{}", c.id);
        let text = String::from_utf8(t[&c.file].clone()).expect("utf-8");
        assert!(
            names(&text, &c.symbol),
            "{}: {} in {}",
            c.id,
            c.symbol,
            c.file
        );
        assert!(m.plant.iter().any(|p| p.id == c.imitates), "{}", c.id);
    }
    for s in Speciality::HUNTERS {
        assert!(
            m.plant.iter().any(|p| p.speciality() == s),
            "{s} has a plant"
        );
    }
    assert!(m.plant.len() >= 12, "{} plants", m.plant.len());
    // Every file of the world is a plant or a control: nothing unkeyed.
    let keyed: BTreeSet<&str> = m
        .plant
        .iter()
        .map(|p| p.file.as_str())
        .chain(m.control.iter().map(|c| c.file.as_str()))
        .collect();
    assert_eq!(keyed, t.keys().map(String::as_str).collect());
    // Harmless: every token is a labelled fake, every host `.invalid`, and no file is executable.
    for (path, bytes) in &t {
        let text = String::from_utf8_lossy(bytes);
        assert!(
            text.match_indices("TOKEN-")
                .all(|(i, _)| text[..i].ends_with("FAKE-")),
            "{path}: a token not labelled fake"
        );
        for host in text.split("://").skip(1) {
            let host = host.split(['/', '"', ' ', '\n']).next().unwrap_or_default();
            assert!(host.ends_with(".invalid"), "{path}: {host}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(seeded::world_dir().join(path))
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0, "{path} is executable");
        }
    }
}

/// Each plant's path routes to its owning speciality, and that speciality's brief for the
/// landing carries the plant's class playbook.
#[test]
fn each_plant_routes_and_loads_its_playbook() {
    for p in &manifest().plant {
        let s = p.speciality();
        let routed = route(&[p.file.as_str()]);
        assert!(routed.contains(&s), "{}: {routed:?}", p.id);
        let loaded: Vec<_> = for_landing(s, &[p.file.as_str()])
            .into_iter()
            .map(|(c, _)| c)
            .collect();
        assert!(loaded.contains(&p.class()), "{}: {loaded:?}", p.id);
        let b = brief(s, &[p.file.as_str()]);
        assert!(b.contains(p.class().playbook().trim_end()), "{}", p.id);
        assert!(b.contains(&format!("### {}", p.class)), "{}", p.id);
    }
}

fn proven_world(m: &Manifest) -> (BTreeMap<String, Vec<u8>>, Vec<String>) {
    let mut t = tree();
    let lines = m
        .plant
        .iter()
        .map(|p| finding_line(p, &prove(&mut t, p)))
        .collect();
    (t, lines)
}

/// A scripted Dark Elf report naming every plant: `accept` counts each finding, valid, with the
/// fingerprint the harness derives from its root cause; no finding is demoted to a hole.
#[test]
fn a_report_naming_every_plant_is_accepted() {
    let m = manifest();
    let (t, lines) = proven_world(&m);
    let text = format!("@S DONE\n{}\n@E", lines.join("\n"));
    let report = Report::parse(&text).expect("a report");
    assert_eq!(report.findings.len(), m.plant.len());
    let accepted = accept(report, &t);
    assert_eq!(accepted.report.report.holes, [], "no plant demoted");
    let got: Vec<(String, String, String, String)> = accepted
        .valid()
        .iter()
        .map(|v| {
            let r = v.record();
            (
                r.speciality.clone(),
                r.class.clone(),
                r.file.clone(),
                r.symbol.clone(),
            )
        })
        .collect();
    let want: Vec<(String, String, String, String)> = m
        .plant
        .iter()
        .map(|p| {
            (
                p.speciality.clone(),
                p.class.clone(),
                p.file.clone(),
                p.symbol.clone(),
            )
        })
        .collect();
    assert_eq!(got, want);
    for (v, p) in accepted.valid().iter().zip(&m.plant) {
        let derived = mnemorph_wire::fingerprint(&p.speciality, &p.class, &p.file, &p.symbol);
        assert_eq!(Some(v.fingerprint().to_owned()), derived, "{}", p.id);
        assert_eq!(v.severity(), p.severity, "{}", p.id);
    }
}

/// The same report read from the world on disk, through [`Tree`]: the records and witnesses are
/// written to a copy of the fixture, and each record validates against it.
#[test]
fn each_record_validates_on_disk() {
    let m = manifest();
    let dir = tempfile::tempdir().expect("tempdir");
    let (t, _) = proven_world(&m);
    for (path, bytes) in &t {
        let at = dir.path().join(path);
        std::fs::create_dir_all(at.parent().expect("parent")).expect("mkdir");
        std::fs::write(at, bytes).expect("write");
    }
    let world = Tree::new(dir.path());
    for p in &m.plant {
        let text = String::from_utf8(t[&p.file].clone()).expect("utf-8");
        let r = seeded::record(p, &text);
        let read = mnemorph_proving::FindingRecord::parse(&r.rec, &t[&r.rec]).expect("parses");
        assert!(
            validate(&read, &world).is_ok(),
            "{}: {:?}",
            p.id,
            validate(&read, &world)
        );
    }
}

/// A forged id — a finding naming a plant under a fingerprint the harness does not derive — is
/// never counted: the harness's fingerprint is the only one.
#[test]
fn a_forged_fingerprint_is_not_counted() {
    let m = manifest();
    let (t, lines) = proven_world(&m);
    let p = &m.plant[0];
    let real = mnemorph_wire::fingerprint(&p.speciality, &p.class, &p.file, &p.symbol).unwrap();
    let forged = lines[0].replacen(
        &format!("#{real}"),
        &format!("#{}", "0".repeat(real.len())),
        1,
    );
    let report = Report::parse(&format!("@S DONE\n{forged}\n@E")).expect("a report");
    let accepted = accept(report, &t);
    assert_eq!(accepted.valid(), []);
    assert_eq!(accepted.report.report.holes.len(), 1);
}

/// For a landing of each keyed file alone, every routed hunter's ledger, built from the key,
/// holds against the world: plants checked, controls ruled out, every other pair skipped for its
/// own reason.
#[test]
fn a_ledger_from_the_key_holds_for_every_landing() {
    let m = manifest();
    let t = tree();
    let files: BTreeSet<&str> = m
        .plant
        .iter()
        .map(|p| p.file.as_str())
        .chain(m.control.iter().map(|c| c.file.as_str()))
        .collect();
    let mut ledgers = 0;
    for file in files {
        let written = vec![file.to_owned()];
        let l = landing(&format!("seeded:{file}"), &written);
        for r in &l.routed {
            if r.speciality == Speciality::General {
                continue;
            }
            let looked = looked(&m, &t, r.speciality, &written);
            let ledger = ledger(&l, r.speciality, &looked);
            let sources = Sources {
                tree: Some(&t),
                ran: None,
            };
            if let Err(refused) = validate_in(&ledger, &l, sources) {
                panic!("{file} for {}: {refused:?}", r.speciality);
            }
            ledgers += 1;
        }
    }
    assert!(ledgers >= m.plant.len(), "{ledgers} ledgers");
}

/// A ledger that rules a plant's class out on the plant's file, with the plant as its evidence,
/// still holds the table: the validator checks the shape of coverage, not its truth. That is the
/// verifier's and the paid evaluation's to catch; this test pins the limit so no one reads more
/// into a clean ledger.
#[test]
fn a_ledger_that_rules_out_a_plant_holds_the_table() {
    let m = manifest();
    let t = tree();
    let p = &m.plant[0];
    let written = vec![p.file.clone()];
    let l = landing("seeded:hidden", &written);
    let mut looked = looked(&m, &t, p.speciality(), &written);
    looked[0].checked.clear();
    looked[0].ruled_out.push((
        p.class(),
        "the classifier reads the full command line, prefixes included",
    ));
    let ledger = ledger(&l, p.speciality(), &looked);
    assert!(
        validate_in(
            &ledger,
            &l,
            Sources {
                tree: Some(&t),
                ran: None
            }
        )
        .is_ok()
    );
}

/// Each negative control is either routed nowhere, or ruled out — `not_applicable`, with the
/// control's file as its evidence — in a ledger that holds. A finding copied from the plant it
/// imitates cannot name that plant's symbol on it unless the key says the two share it: then
/// only judgement tells them apart, and the copy validates, which the key records.
#[test]
fn negative_controls_are_ruled_out_or_unrouted() {
    let m = manifest();
    let t = tree();
    for c in &m.control {
        let written = vec![c.file.clone()];
        let routed = route(&written);
        if !routed.contains(&c.speciality()) {
            assert_eq!(routed, [], "{}: routed nowhere", c.id);
            continue;
        }
        let l = landing(&format!("seeded:{}", c.id), &written);
        let looked = looked(&m, &t, c.speciality(), &written);
        let ledger = ledger(&l, c.speciality(), &looked);
        assert!(
            validate_in(
                &ledger,
                &l,
                Sources {
                    tree: Some(&t),
                    ran: None
                }
            )
            .is_ok(),
            "{}",
            c.id
        );
        let entry = ledger
            .entries
            .iter()
            .find(|e| e.class == c.class)
            .expect("an entry");
        assert!(
            matches!(&entry.state, State::NotApplicable { reason, .. } if reason == &c.reason),
            "{}: {:?}",
            c.id,
            entry.state
        );
        // The plant it imitates, filed against the control's file instead.
        let plant = m
            .plant
            .iter()
            .find(|p| p.id == c.imitates)
            .expect("imitated");
        let mut copy = plant.clone();
        copy.file.clone_from(&c.file);
        let mut world = t.clone();
        let text = String::from_utf8(t[&c.file].clone()).expect("utf-8");
        if names(&text, &plant.symbol) {
            assert!(
                c.same_symbol,
                "{}: the key must say it shares {}",
                c.id, plant.symbol
            );
            let r = prove(&mut world, &copy);
            assert!(validate(&r, &world).is_ok(), "{}", c.id);
        } else {
            assert!(!c.same_symbol, "{}", c.id);
            world.insert(
                seeded::poc_path(&copy),
                seeded::poc_body(&copy).into_bytes(),
            );
            let r = seeded::record(&copy, &text);
            let broken = validate(&r, &world).expect_err("refused");
            assert!(
                broken.contains(&mnemorph_proving::Rule::SymbolPresent),
                "{}: {broken:?}",
                c.id
            );
        }
    }
}
