//! The scout's mirror: each ecosystem the world's lockfiles name kept offline under
//! `.mnemorph/intel/`, in the layout `osv-scanner` reads, so an audit needs no network.

use std::fs;
use std::path::Path;

use mnemorph_config::scout;

const ZIP: &[u8] = b"PK\x03\x04 an advisory database";

fn world(lockfiles: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for f in lockfiles {
        fs::write(dir.path().join(f), "x").unwrap();
    }
    dir
}

fn serves(url: &str, to: &Path) -> Result<(), String> {
    assert!(
        url.starts_with("https://osv-vulnerabilities.storage.googleapis.com/"),
        "{url}"
    );
    assert!(url.ends_with("/all.zip"), "{url}");
    fs::write(to, ZIP).map_err(|e| e.to_string())
}

#[test]
fn the_ecosystems_are_read_from_the_worlds_lockfiles() {
    let w = world(&["Cargo.lock", "pnpm-lock.yaml", "yarn.lock"]);
    assert_eq!(scout::ecosystems(w.path()), vec!["crates.io", "npm"]);
    assert_eq!(scout::ecosystems(world(&[]).path()), Vec::<&str>::new());
}

#[test]
fn each_ecosystem_is_mirrored_where_the_scanner_reads_and_said_in_status() {
    let w = world(&["Cargo.lock", "go.sum"]);
    let (said, whole) = scout::refresh(w.path(), &serves).unwrap();
    assert!(whole, "{said:?}");
    for eco in ["crates.io", "Go"] {
        let zip = w.path().join(scout::MIRROR).join(eco).join("all.zip");
        assert_eq!(fs::read(&zip).unwrap(), ZIP, "{eco}");
    }
    let lines = scout::lines(w.path());
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("crates.io:") && l.contains("today")),
        "{lines:?}"
    );
}

#[test]
fn a_failed_or_bad_download_keeps_the_mirror_it_had() {
    let w = world(&["Cargo.lock"]);
    scout::refresh(w.path(), &serves).unwrap();
    let zip = w.path().join(scout::MIRROR).join("crates.io/all.zip");
    let page = |_: &str, to: &Path| fs::write(to, "<html>sorry</html>").map_err(|e| e.to_string());
    let (said, whole) = scout::refresh(w.path(), &page).unwrap();
    assert!(!whole && said[0].contains("no zip"), "{said:?}");
    assert_eq!(fs::read(&zip).unwrap(), ZIP, "the old mirror stands");
    let down = |_: &str, _: &Path| Err("curl: (6) no such host".to_owned());
    let (said, whole) = scout::refresh(w.path(), &down).unwrap();
    assert!(!whole && said[0].contains("no such host"), "{said:?}");
    assert_eq!(fs::read(&zip).unwrap(), ZIP);
    let left: Vec<_> = fs::read_dir(zip.parent().unwrap()).unwrap().collect();
    assert_eq!(left.len(), 1, "no partial file left");
}

#[test]
fn a_world_with_no_lockfile_has_nothing_to_mirror() {
    let w = world(&[]);
    let (said, whole) = scout::refresh(w.path(), &serves).unwrap();
    assert!(whole && said[0].contains("no lockfile"), "{said:?}");
}

#[cfg(unix)]
#[test]
fn a_link_a_body_planted_in_the_mirror_is_refused() {
    let w = world(&["Cargo.lock"]);
    let elsewhere = tempfile::tempdir().unwrap();
    fs::create_dir_all(w.path().join(".mnemorph/intel")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), w.path().join(scout::MIRROR)).unwrap();
    let e = scout::refresh(w.path(), &serves).unwrap_err();
    assert!(e.contains("link"), "{e}");
    assert!(fs::read_dir(elsewhere.path()).unwrap().next().is_none());
}

const FOUND: &str = r#"{"results":[{"packages":[
  {"package":{"name":"time","version":"0.1.43","ecosystem":"crates.io"},
   "vulnerabilities":[{"id":"RUSTSEC-2020-0071"},{"id":"GHSA-wcg3-cvx6-7396"}],
   "groups":[{"max_severity":"6.2"}]},
  {"package":{"name":"quiet","version":"1.0.0","ecosystem":"crates.io"},"vulnerabilities":[]}
]}]}"#;

fn scanned(code: i32, out: &str) -> impl Fn(&[String]) -> Result<scout::Scanned, String> {
    let out = out.to_owned();
    move |_: &[String]| {
        Ok((
            Some(code),
            out.clone().into_bytes(),
            b"a last word\n".to_vec(),
        ))
    }
}

#[test]
fn an_audit_finds_what_the_scanner_found_and_says_it_in_status() {
    let w = world(&["Cargo.lock"]);
    scout::refresh(w.path(), &serves).unwrap();
    let a = scout::audit(w.path(), &scanned(1, FOUND)).unwrap();
    assert_eq!(a.lockfiles, vec!["Cargo.lock"]);
    assert_eq!(a.advisories.len(), 1, "a package with none is no finding");
    assert_eq!(a.advisories[0].ids[0], "RUSTSEC-2020-0071");
    let lines = scout::audit_lines(w.path());
    assert!(
        lines[0].contains("1 package(s)") && lines[0].contains("as last read"),
        "{lines:?}"
    );
    assert!(
        lines[1].contains("time 0.1.43") && lines[1].contains("severity 6.2"),
        "{lines:?}"
    );
    // Clean is an answer too.
    let a = scout::audit(w.path(), &scanned(0, r#"{"results":[]}"#)).unwrap();
    assert_eq!(a.advisories, Vec::new());
    assert!(scout::audit_lines(w.path())[0].contains("0 package(s)"));
}

#[test]
fn an_audit_that_could_not_look_never_says_it_found_nothing() {
    let w = world(&["Cargo.lock", "go.sum"]);
    // An ecosystem unmirrored: the scanner is not even run.
    let never = |_: &[String]| -> Result<scout::Scanned, String> { panic!("never run") };
    let e = scout::audit(w.path(), &never).unwrap_err();
    assert!(
        e.contains("is not mirrored") && e.contains("mnemorphd scout"),
        "{e}"
    );
    scout::refresh(w.path(), &serves).unwrap();
    // The scanner's own failure: 127, with an empty report that would otherwise read as clean.
    let e = scout::audit(w.path(), &scanned(127, r#"{"results":[]}"#)).unwrap_err();
    assert!(
        e.contains("the scanner failed") && e.contains("a last word"),
        "{e}"
    );
    let e = scout::audit(w.path(), &scanned(1, "not json")).unwrap_err();
    assert!(e.contains("report"), "{e}");
    assert_eq!(
        scout::audit_lines(w.path()),
        Vec::<String>::new(),
        "no audit is kept"
    );
    let e = scout::audit(world(&[]).path(), &never).unwrap_err();
    assert!(e.contains("nothing to audit"), "{e}");
}
