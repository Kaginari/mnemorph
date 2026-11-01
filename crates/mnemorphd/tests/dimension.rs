//! Relations between worlds through the binary: a session opening joins the registry; two
//! sightings by different bodies make exactly one proposal that writes nothing; only the
//! human's yes writes the manifest, and revokes it; `onto check --dimension` reports an
//! obstruction.

#[path = "support/isolated.rs"]
mod isolated;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use mnemorph_server::world::dimension::{MANIFEST, register_at, sight, sightings_file, world_id};

struct Dim {
    root: PathBuf,
}

impl Dim {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("mnemorph-dim-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
        }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn registry(&self) -> PathBuf {
        self.state().join("mnemorph/worlds.jsonl")
    }

    fn world(&self, name: &str) -> PathBuf {
        let w = self.root.join(name);
        fs::create_dir_all(w.join(".mnemorph")).unwrap();
        w
    }

    /// Run with no controlling terminal (a session of its own, on Linux): no question is ever
    /// answered by whoever runs the tests.
    fn run(&self, args: &[&str], body: Option<&str>) -> (Option<i32>, String, String) {
        let mut c = if cfg!(target_os = "linux") {
            let mut c = isolated::bin("setsid");
            c.args(["-w", env!("CARGO_BIN_EXE_mnemorphd")]);
            c
        } else {
            isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
        };
        c.args(args)
            .stdin(std::process::Stdio::null())
            .current_dir(&self.root)
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("MNEMORPH_BODY")
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.root.join("config"));
        if let Some(b) = body {
            c.env("MNEMORPH_BODY", b);
        }
        let out = c.output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }
}

impl Drop for Dim {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn a_session_opening_writes_a_registry_row() {
    let d = Dim::new("open");
    let w = d.world("w");
    // The provider and running unguarded are the human's global file's to set.
    fs::create_dir_all(d.root.join("config/mnemorph")).unwrap();
    fs::write(
        d.root.join("config/mnemorph/config.toml"),
        "[sandbox]\nallow_unguarded = true\n\n[models]\nprovider = \"local\"\n\n[providers.local]\nkind = \"openai\"\nbase_url = \"http://127.0.0.1:9\"\n",
    )
    .unwrap();
    let mut child = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(["--world", s(&w)])
        .env_remove("ANTHROPIC_API_KEY")
        .env("XDG_STATE_HOME", d.state())
        .env("XDG_CONFIG_HOME", d.root.join("config"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        "{}",
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
    )
    .unwrap();
    stdin.flush().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let answer = loop {
        let line = lines.next().expect("an answer to initialize").unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if v["id"] == 1 {
            break v;
        }
    };
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    assert!(answer.get("result").is_some(), "{answer}");
    let rows = fs::read_to_string(d.registry()).unwrap();
    let row: serde_json::Value = serde_json::from_str(rows.lines().next().unwrap()).unwrap();
    assert_eq!(rows.lines().count(), 1);
    assert_eq!(row["world"], s(&w));
    assert!(row["id"].as_str().unwrap().starts_with("w-"));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one relation's whole life, sighted to revoked, through the binary"
)]
fn a_relation_named_twice_is_proposed_written_by_the_human_and_revoked() {
    let d = Dim::new("relate");
    let a = d.world("a");
    let b = d.world("b");
    register_at(&d.registry(), &a, None).unwrap();
    register_at(&d.registry(), &b, None).unwrap();
    fs::write(
        a.join("Cargo.toml"),
        "[dependencies]\nb = { path = \"../b\" }\n",
    )
    .unwrap();
    fs::write(a.join("go.mod"), "module a\n\nreplace b => ../b\n").unwrap();
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "a",
        ],
    ] {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&a)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    sight(&a, &d.registry(), "elf-a", "Cargo.toml").unwrap();
    let (code, out, _) = d.run(&["onto", "relations", "--world", s(&a)], None);
    assert_eq!(code, Some(0));
    assert!(
        out.contains("none proposed"),
        "one sighting proposes nothing: {out}"
    );
    // A second session: one session's bodies are never two voices.
    register_at(&d.registry(), &a, None).unwrap();
    sight(&a, &d.registry(), "elf-b", "go.mod").unwrap();
    // The sightings are the harness's, outside the world, and name the other world by id only.
    assert!(!a.join(".mnemorph/ontology").exists());
    let kept = fs::read_to_string(sightings_file(&d.registry(), &a)).unwrap();
    assert!(
        kept.contains(&world_id(&b)) && !kept.contains(s(&b)),
        "{kept}"
    );
    let (code, out, _) = d.run(&["onto", "relations", "--world", s(&a)], None);
    assert_eq!(code, Some(0));
    assert_eq!(out.matches("proposed:").count(), 1, "{out}");
    assert!(out.contains(&format!("+world = \"{}\"", s(&b))), "{out}");
    assert!(out.contains("+type = \"depends-on\""), "{out}");
    assert!(!a.join(MANIFEST).exists(), "a proposal writes nothing");

    // A body never says yes, nor reads where other worlds live.
    for args in [
        &[
            "onto",
            "relate",
            s(&b),
            "depends-on",
            "--yes",
            "--world",
            s(&a),
        ][..],
        &[
            "onto",
            "inverse",
            "depends-on",
            "used-by",
            "--yes",
            "--world",
            s(&a),
        ],
        &["onto", "relations", "--world", s(&a)],
        &["onto", "check", "--dimension", "--world", s(&a)],
    ] {
        let (code, out, err) = d.run(args, Some("elf-a"));
        assert_eq!(code, Some(2), "{args:?}");
        assert!(err.contains("elf-a runs this"), "{err}");
        assert!(!out.contains(s(&b)) && !err.contains(s(&b)), "{out}{err}");
    }
    assert!(!a.join(MANIFEST).exists());

    // Piped input is no human's yes: nothing is written.
    let (code, _, err) = d.run(
        &["onto", "relate", s(&b), "depends-on", "--world", s(&a)],
        None,
    );
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("terminal"), "{err}");
    assert!(!a.join(MANIFEST).exists());
    let (code, out, err) = d.run(
        &[
            "onto",
            "relate",
            s(&b),
            "depends-on",
            "--yes",
            "--world",
            s(&a),
        ],
        None,
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(out.starts_with("stated depends-on"), "{out}");
    let (_, out, _) = d.run(&["onto", "relations", "--world", s(&a)], None);
    assert!(
        out.contains(&format!("stated\tdepends-on\t{}", s(&b))),
        "{out}"
    );
    assert!(!out.contains("proposed:"), "{out}");

    // b's in-world copy saying something of a is no relation of b's: nothing obstructs.
    fs::write(
        b.join(MANIFEST),
        format!("[[relation]]\nworld = \"{}\"\ntype = \"deploys\"\n", s(&a)),
    )
    .unwrap();
    let (_, out, _) = d.run(&["onto", "check", "--dimension", "--world", s(&a)], None);
    assert!(!out.contains("dimension-obstruction"), "{out}");
    // b states, by the human, something of a that cannot hold beside it: an obstruction, reported.
    let (code, _, err) = d.run(
        &[
            "onto",
            "relate",
            s(&a),
            "deploys",
            "--yes",
            "--world",
            s(&b),
        ],
        None,
    );
    assert_eq!(code, Some(0), "{err}");
    let (code, out, _) = d.run(&["onto", "check", "--dimension", "--world", s(&a)], None);
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains("high\tdimension-obstruction\t.mnemorph/dimension.toml"),
        "{out}"
    );
    assert!(out.contains("ours depends-on\ttheirs deployed-by"), "{out}");
    // Without `--dimension`, no neighbour is read.
    let (_, out, _) = d.run(&["onto", "check", "--world", s(&a)], None);
    assert!(!out.contains("dimension-obstruction"), "{out}");
    // Once b agrees, the pair glues.
    for (verb, kind) in [("unrelate", "deploys"), ("relate", "depended-on-by")] {
        let (code, _, err) = d.run(
            &["onto", verb, s(&a), kind, "--yes", "--world", s(&b)],
            None,
        );
        assert_eq!(code, Some(0), "{err}");
    }
    let (_, out, _) = d.run(&["onto", "check", "--dimension", "--world", s(&a)], None);
    assert!(!out.contains("dimension-obstruction"), "{out}");
    assert!(out.contains("glued depends-on"), "{out}");

    let (code, out, _) = d.run(
        &[
            "onto",
            "unrelate",
            s(&b),
            "depends-on",
            "--yes",
            "--world",
            s(&a),
        ],
        None,
    );
    assert_eq!(code, Some(0));
    assert!(out.starts_with("revoked"), "{out}");
    let (code, _, _) = d.run(
        &[
            "onto",
            "unrelate",
            s(&b),
            "depends-on",
            "--yes",
            "--world",
            s(&a),
        ],
        None,
    );
    assert_eq!(code, Some(2));
}
