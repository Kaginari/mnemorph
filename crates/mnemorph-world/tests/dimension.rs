//! Relations between worlds: the registry, sightings from manifests, the named-twice proposal
//! that writes nothing, the human's yes and its revocation, expiry, and the gluing of two
//! manifests.

#![allow(
    clippy::many_single_char_names,
    reason = "worlds a, b, c, e of one fixture"
)]

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_world::dimension::{
    MANIFEST, Manifest, Proposal, Relation, SIGHTINGS, STALE_AFTER, Sighting, glue, known,
    load_sightings, proposals, register_at, relate, sight, sightings_of, stale_findings, unrelate,
    world_id,
};

struct Dim {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Dim {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        Self { _tmp: tmp, root }
    }

    /// A world named `name`, registered as opened once.
    fn world(&self, name: &str) -> PathBuf {
        let w = self.root.join(name);
        fs::create_dir_all(w.join(".mnemorph")).unwrap();
        register_at(&self.registry(), &w, None).unwrap();
        w
    }

    fn registry(&self) -> PathBuf {
        self.root.join("state/mnemorph/worlds.jsonl")
    }
}

fn write(at: &Path, text: &str) {
    if let Some(d) = at.parent() {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(at, text).unwrap();
}

#[test]
fn a_session_opening_is_a_registry_row() {
    let d = Dim::new();
    let a = d.world("a");
    register_at(&d.registry(), &a, Some("github.com/o/a".into())).unwrap();
    let text = fs::read_to_string(d.registry()).unwrap();
    assert_eq!(text.lines().count(), 2);
    let row: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(row["world"], a.display().to_string());
    assert_eq!(row["id"], world_id(&a));
    assert_eq!(row["remote"], "github.com/o/a");
    assert!(row["at"].as_str().unwrap().ends_with('Z'));
    assert!(world_id(&a).starts_with("a-"));
    let k = known(&d.registry());
    assert_eq!(k.len(), 1);
    assert_eq!(k[0].sessions, 2);
    assert_eq!(k[0].remote.as_deref(), Some("github.com/o/a"));
    assert!(k[0].first <= k[0].last);
}

#[test]
fn a_manifest_dependency_is_sighted_once_per_session() {
    let d = Dim::new();
    let a = d.world("a");
    let b = d.world("b");
    write(
        &a.join("Cargo.toml"),
        "[package]\nname = \"a\"\n\n[dependencies]\nb = { path = \"../b/crates/b\" }\nserde = \"1\"\n",
    );
    assert_eq!(sight(&a, &d.registry(), "elf-a", "Cargo.toml").unwrap(), 1);
    // The same read again in the same session adds nothing.
    assert_eq!(
        sight(&a, &d.registry(), "elf-a", "./Cargo.toml").unwrap(),
        0
    );
    let seen = load_sightings(&a);
    assert_eq!(
        seen,
        vec![Sighting {
            from: world_id(&a),
            to: b.display().to_string(),
            kind: "depends-on".into(),
            anchor: "Cargo.toml:5".into(),
            body: "elf-a".into(),
            session: 1,
        }]
    );
    // A file that is no manifest, and a world's own paths, are no sighting.
    write(&a.join("notes.md"), "see ../b\n");
    assert_eq!(sight(&a, &d.registry(), "elf-a", "notes.md").unwrap(), 0);
    write(
        &a.join("sub/Cargo.toml"),
        "[dependencies]\nc = { path = \"../c\" }\n",
    );
    assert_eq!(
        sight(&a, &d.registry(), "elf-a", "sub/Cargo.toml").unwrap(),
        0
    );
}

#[test]
fn a_remote_and_a_submodule_are_sighted() {
    let d = Dim::new();
    let a = d.world("a");
    let v = d.root.join("v");
    fs::create_dir_all(&v).unwrap();
    register_at(&d.registry(), &v, Some("github.com/o/v".into())).unwrap();
    write(
        &a.join(".gitmodules"),
        "[submodule \"v\"]\n\tpath = third/v\n\turl = https://github.com/O/v.git\n",
    );
    let seen = sightings_of(&a, &known(&d.registry()), "rimuru", ".gitmodules");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].kind, "vendors");
    assert_eq!(seen[0].to, v.display().to_string());
}

#[test]
fn nothing_outside_the_world_is_read_for_a_sighting() {
    let d = Dim::new();
    let a = d.world("a");
    let b = d.world("b");
    write(
        &b.join("Cargo.toml"),
        "[dependencies]\na = { path = \"../a\" }\n",
    );
    let k = known(&d.registry());
    // A path out of the world, and a link from inside it to a file outside, are never opened.
    assert_eq!(sightings_of(&a, &k, "elf", "../b/Cargo.toml"), vec![]);
    assert_eq!(
        sightings_of(&a, &k, "elf", &b.join("Cargo.toml").display().to_string()),
        vec![]
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(b.join("Cargo.toml"), a.join("Cargo.toml")).unwrap();
        assert_eq!(sightings_of(&a, &k, "elf", "Cargo.toml"), vec![]);
    }
    assert!(!a.join(SIGHTINGS).exists());
}

fn seen(to: &str, kind: &str, anchor: &str, body: &str, session: u64) -> Sighting {
    Sighting {
        from: "a-0".into(),
        to: to.into(),
        kind: kind.into(),
        anchor: anchor.into(),
        body: body.into(),
        session,
    }
}

#[test]
fn a_relation_is_proposed_only_when_named_twice_and_written_only_by_the_human() {
    let d = Dim::new();
    let a = d.world("a");
    let b = d.world("b");
    let bp = b.display().to_string();
    write(
        &a.join("Cargo.toml"),
        "[dependencies]\nb = { path = \"../b\" }\n",
    );
    write(
        &a.join("web/package.json"),
        "{\n  \"dependencies\": {\n    \"b\": \"file:../../b\"\n  }\n}\n",
    );
    let m = Manifest::load(&a).unwrap();

    sight(&a, &d.registry(), "elf-a", "Cargo.toml").unwrap();
    assert!(
        proposals(&load_sightings(&a), &m, 1).is_empty(),
        "one sighting"
    );
    // The same body from another anchor is not a second voice.
    sight(&a, &d.registry(), "elf-a", "web/package.json").unwrap();
    assert_eq!(proposals(&load_sightings(&a), &m, 1), vec![], "one body");
    // Another body from the same anchor is not a second piece of evidence.
    sight(&a, &d.registry(), "elf-b", "Cargo.toml").unwrap();
    // …but with the first body's other anchor, it is named twice.
    let p = proposals(&load_sightings(&a), &m, 1);
    assert_eq!(p.len(), 1, "{p:?}");
    let Proposal::Add { world, kind, .. } = &p[0] else {
        panic!("{p:?}")
    };
    assert_eq!((world.as_str(), kind.as_str()), (bp.as_str(), "depends-on"));
    assert!(p[0].diff().contains("+type = \"depends-on\""));
    // Proposing wrote nothing.
    assert!(!a.join(MANIFEST).exists());

    // The human's yes writes it; then it is stated, no longer proposed.
    assert!(relate(&a, &b, "depends-on").unwrap());
    assert!(!relate(&a, &b, "depends-on").unwrap());
    let m = Manifest::load(&a).unwrap();
    assert!(m.states(&bp, "depends-on"));
    assert_eq!(proposals(&load_sightings(&a), &m, 1), vec![]);

    // Revoked, it is gone, and named twice it is proposed again, never rewritten by itself.
    assert!(unrelate(&a, &b, "depends-on").unwrap());
    assert!(!unrelate(&a, &b, "depends-on").unwrap());
    let m = Manifest::load(&a).unwrap();
    assert_eq!(m.relations, vec![]);
    assert_eq!(proposals(&load_sightings(&a), &m, 2).len(), 1);
    assert_eq!(Manifest::load(&a).unwrap().relations, vec![]);
}

#[test]
fn a_new_type_most_pairs_already_carry_is_offered_as_the_old_one() {
    let mut m = Manifest::default();
    for w in ["/w1", "/w2", "/w3"] {
        m.relations.push(Relation {
            world: w.into(),
            kind: "deploys".into(),
            remote: None,
        });
    }
    let mut s = Vec::new();
    for w in ["/w1", "/w2", "/w3"] {
        s.push(seen(w, "ships", "k8s/a.yaml:1", "elf-a", 1));
        s.push(seen(w, "ships", "k8s/b.yaml:1", "elf-b", 1));
    }
    let p = proposals(&s, &m, 1);
    assert_eq!(p.len(), 3);
    for x in &p {
        let Proposal::Add {
            offered, carries, ..
        } = x
        else {
            panic!()
        };
        assert_eq!(offered.as_deref(), Some("deploys"));
        assert_eq!(carries, &vec!["deploys".to_owned()]);
        assert!(x.why().contains("state `deploys` instead?"));
    }
    // On fewer pairs the proposal only notes what the pair carries.
    let p = proposals(&s[..2], &m, 1);
    let Proposal::Add {
        offered, carries, ..
    } = &p[0]
    else {
        panic!()
    };
    assert_eq!(offered, &None);
    assert_eq!(carries, &vec!["deploys".to_owned()]);
}

#[test]
fn a_learned_relation_unseen_for_thirty_sessions_is_proposed_for_removal() {
    let mut m = Manifest::default();
    m.relations.push(Relation {
        world: "/b".into(),
        kind: "depends-on".into(),
        remote: None,
    });
    m.relations.push(Relation {
        world: "/c".into(),
        kind: "deploys".into(),
        remote: None,
    });
    let s = vec![
        seen("/b", "depends-on", "Cargo.toml:3", "elf-a", 4),
        seen("/b", "depends-on", "go.mod:3", "elf-b", 5),
    ];
    assert_eq!(proposals(&s, &m, 5 + STALE_AFTER - 1), vec![]);
    let p = proposals(&s, &m, 5 + STALE_AFTER);
    // The relation the human stated with no sighting carries no clock: only the learned one.
    assert_eq!(p.len(), 1);
    assert!(matches!(&p[0], Proposal::Remove { world, last: 5, .. } if world == "/b"));
    assert!(p[0].diff().contains("-type = \"depends-on\""));
    let f = stale_findings(&p);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].code, "stale-relation");
    assert!(!f[0].severity.blocks());
}

fn state(world: &Path, other: &Path, kind: &str) {
    let mut m = Manifest::load(world).unwrap();
    m.relations.push(Relation {
        world: other.display().to_string(),
        kind: kind.into(),
        remote: None,
    });
    m.save(world).unwrap();
}

#[test]
fn two_manifests_glue_on_what_they_share_and_obstruct_on_nothing() {
    let d = Dim::new();
    let a = d.world("a");
    let b = d.world("b");
    let c = d.world("c");
    let e = d.world("e");
    // a deploys b; b says it is deployed by a: glued.
    state(&a, &b, "deploys");
    state(&b, &a, "deployed-by");
    // a reads secrets from c; c says it deploys a: nothing in common.
    state(&a, &c, "reads-secrets-from");
    state(&c, &a, "deploys");
    // e alone says something of a: one-sided, lawful.
    state(&e, &a, "watches");
    let before: Vec<String> = [&b, &c, &e]
        .iter()
        .map(|w| fs::read_to_string(w.join(MANIFEST)).unwrap())
        .collect();
    let (glues, findings) = glue(&a, &known(&d.registry())).unwrap();
    let of = |w: &Path| {
        glues
            .iter()
            .find(|g| g.neighbour == w.display().to_string())
            .unwrap()
    };
    assert_eq!(of(&b).shared, vec!["deploys".to_owned()]);
    assert!(!of(&b).obstructed());
    assert!(of(&c).obstructed());
    assert_eq!(of(&c).theirs, vec!["deployed-by".to_owned()]);
    assert_eq!(of(&e).theirs, vec!["watches-by".to_owned()]);
    assert!(!of(&e).obstructed());
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].code, "dimension-obstruction");
    assert!(findings[0].severity.blocks());
    assert!(findings[0].message.contains(&c.display().to_string()));
    // The neighbours' manifests are read, never written.
    let after: Vec<String> = [&b, &c, &e]
        .iter()
        .map(|w| fs::read_to_string(w.join(MANIFEST)).unwrap())
        .collect();
    assert_eq!(before, after);
}
