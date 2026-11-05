//! CI as an instrument: the forge's runs read through the harness's read-only probe and written
//! under the instruments, so a creature reads the real state and not a doc's word for it.

use std::path::Path;
use std::time::Duration;

use mnemorph_config::ci::{self, repo_of};
use mnemorph_config::{Probe, Seen};

struct Forge(String);

impl Probe for Forge {
    fn run(&self, argv: &[String], _: Duration) -> Result<Seen, String> {
        assert_eq!(&argv[..3], ["gh", "run", "list"], "{argv:?}");
        assert!(
            argv.windows(2).any(|w| w == ["--repo", "me/proj"]),
            "{argv:?}"
        );
        Ok(Seen {
            code: Some(0),
            out: self.0.clone().into_bytes(),
            ..Seen::default()
        })
    }
}

struct Down;

impl Probe for Down {
    fn run(&self, _: &[String], _: Duration) -> Result<Seen, String> {
        Ok(Seen {
            code: Some(1),
            err: b"HTTP 502".to_vec(),
            ..Seen::default()
        })
    }
}

fn run(id: u64, branch: &str, status: &str, conclusion: &str) -> String {
    format!(
        r#"{{"databaseId":{id},"headBranch":"{branch}","headSha":"abc1234def","workflowName":"ci","status":"{status}","conclusion":"{conclusion}","url":"https://github.com/me/proj/actions/runs/{id}","createdAt":"2026-11-05T10:00:{id:02}Z"}}"#
    )
}

fn runs(list: &[String]) -> Forge {
    Forge(format!("[{}]", list.join(",")))
}

fn rows(w: &Path) -> usize {
    std::fs::read_to_string(w.join(ci::INSTRUMENT)).map_or(0, |t| t.lines().count())
}

#[test]
fn the_runs_are_written_once_and_a_run_that_moved_again() {
    let dir = tempfile::tempdir().unwrap();
    let w = dir.path();
    let first = runs(&[
        run(2, "main", "in_progress", ""),
        run(1, "main", "completed", "failure"),
    ]);
    assert_eq!(ci::refresh(w, "me/proj", &first).unwrap(), 2);
    assert_eq!(rows(w), 2);
    assert_eq!(
        ci::refresh(w, "me/proj", &first).unwrap(),
        0,
        "nothing moved"
    );
    let later = runs(&[
        run(2, "main", "completed", "success"),
        run(1, "main", "completed", "failure"),
    ]);
    assert_eq!(ci::refresh(w, "me/proj", &later).unwrap(), 1, "run 2 moved");
    // The newest state of the newest run of each branch and workflow.
    let lines = ci::lines(w);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("main") && lines[0].contains("success") && lines[0].contains("abc1234"),
        "{lines:?}"
    );
    assert!(lines[0].contains("as last read"), "{lines:?}");
}

#[test]
fn each_branch_shows_its_own_state_failures_first() {
    let dir = tempfile::tempdir().unwrap();
    let w = dir.path();
    let f = runs(&[
        run(3, "main", "completed", "success"),
        run(2, "feat/x", "completed", "failure"),
    ]);
    ci::refresh(w, "me/proj", &f).unwrap();
    let lines = ci::lines(w);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].contains("feat/x") && lines[0].contains("failure"),
        "{lines:?}"
    );
}

#[test]
fn a_forge_that_cannot_be_read_writes_nothing_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let w = dir.path();
    let e = ci::refresh(w, "me/proj", &Down).unwrap_err();
    assert!(e.contains("HTTP 502"), "{e}");
    let e = ci::refresh(w, "me/proj", &Forge("not json".into())).unwrap_err();
    assert!(e.contains("cannot be read"), "{e}");
    assert_eq!(rows(w), 0);
    assert_eq!(ci::lines(w), Vec::<String>::new());
}

#[test]
fn a_line_a_body_wrote_badly_is_passed_over() {
    let dir = tempfile::tempdir().unwrap();
    let w = dir.path();
    ci::refresh(
        w,
        "me/proj",
        &runs(&[run(1, "main", "completed", "success")]),
    )
    .unwrap();
    let at = w.join(ci::INSTRUMENT);
    let text = std::fs::read_to_string(&at).unwrap();
    std::fs::write(&at, format!("not a row\n{text}")).unwrap();
    assert_eq!(ci::lines(w).len(), 1);
}

#[test]
fn a_repository_is_read_from_its_remote_url() {
    for url in [
        "https://github.com/me/proj.git",
        "https://github.com/me/proj",
        "git@github.com:me/proj.git",
        "ssh://git@github.com/me/proj.git",
    ] {
        assert_eq!(repo_of(url).as_deref(), Some("me/proj"), "{url}");
    }
    for url in [
        "",
        "/srv/git/proj.git",
        "https://github.com/me",
        "https://github.com/-x/proj",
    ] {
        assert_eq!(repo_of(url), None, "{url}");
    }
}
