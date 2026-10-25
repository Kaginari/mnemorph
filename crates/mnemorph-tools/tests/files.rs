//! The file tools against a real world on disk.

use std::fs;
use std::path::{Path, PathBuf};

use mnemorph_races::{Race, ToolClass};
use mnemorph_tools::{Context, Outcome, call, registry};

/// A fresh world with a soil (`src/`) and a directory outside it, removed on drop.
struct World {
    root: PathBuf,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world/src")).unwrap();
        fs::create_dir_all(root.join("outside")).unwrap();
        fs::write(root.join("outside/secret.txt"), "secret\n").unwrap();
        Self { root }
    }

    fn world(&self) -> PathBuf {
        self.root.join("world")
    }

    fn put(&self, rel: &str, content: &str) {
        let p = self.world().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    fn get(&self, rel: &str) -> String {
        fs::read_to_string(self.world().join(rel)).unwrap()
    }

    fn cx(&self) -> Context {
        Context::new(Race::Slime, self.world(), &["src"])
    }

    #[cfg(unix)]
    fn link(&self, rel: &str, to: &Path) {
        std::os::unix::fs::symlink(to, self.world().join(rel)).unwrap();
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn run(cx: &mut Context, name: &str, args: &str) -> Outcome {
    call(&registry(), name, args, cx)
}

#[test]
fn every_tool_has_a_schema_an_example_and_a_shelf_seat() {
    for tool in registry() {
        let schema = tool.schema();
        assert_eq!(schema["type"], "object", "{}", tool.name());
        let props = schema["properties"].as_object().unwrap();
        let example = tool.example();
        for key in example.as_object().unwrap().keys() {
            assert!(
                props.contains_key(key),
                "{}: example key `{key}` not in the schema",
                tool.name()
            );
        }
        for req in schema["required"].as_array().unwrap() {
            assert!(
                example.get(req.as_str().unwrap()).is_some(),
                "{}: example lacks `{req}`",
                tool.name()
            );
        }
        assert!(
            Race::ALL.iter().any(|r| r.shelf().contains(&tool.class())),
            "{} has no shelf seat",
            tool.name()
        );
        assert_ne!(tool.description(), "");
    }
}

#[test]
fn bad_arguments_are_named_errors_and_nothing_runs() {
    let w = World::new("args");
    let mut cx = w.cx();
    let o = run(&mut cx, "write", "{\"path\": \"src/a.rs\", \"content\": ");
    assert!(
        o.is_error && o.content.contains("arguments to write are not valid JSON"),
        "{o:?}"
    );
    let o = run(&mut cx, "write", r#"{"path": "src/a.rs", "contents": "x"}"#);
    assert!(
        o.is_error && o.content.contains("do not fit its schema"),
        "{o:?}"
    );
    let o = run(&mut cx, "read", r"{}");
    assert!(o.is_error && o.content.contains("path"), "{o:?}");
    let o = run(&mut cx, "launch", r"{}");
    assert!(
        o.is_error && o.content.contains("there is no tool `launch`"),
        "{o:?}"
    );
    assert!(!w.world().join("src/a.rs").exists());
}

#[test]
fn read_numbers_lines_and_withholds_an_unchanged_window() {
    let w = World::new("read");
    w.put("src/a.txt", "one\ntwo\nthree\nfour\n");
    let mut cx = w.cx();
    let o = run(
        &mut cx,
        "read",
        r#"{"path": "src/a.txt", "offset": 2, "limit": 2}"#,
    );
    assert_eq!(
        o.content,
        "     2\ttwo\n     3\tthree\n… 1 more lines (offset 4 continues)"
    );
    assert_eq!(o.meta.class, Some(ToolClass::Read));
    assert!(!o.meta.unchanged);

    let again = run(
        &mut cx,
        "read",
        r#"{"path": "src/a.txt", "offset": 2, "limit": 2}"#,
    );
    assert!(
        again.meta.unchanged && again.content.contains("unchanged"),
        "{again:?}"
    );

    let other = run(&mut cx, "read", r#"{"path": "src/a.txt"}"#);
    assert!(!other.meta.unchanged, "another window is read in full");

    w.put("src/a.txt", "one\nTWO\nthree\nfour\n");
    let changed = run(
        &mut cx,
        "read",
        r#"{"path": "src/a.txt", "offset": 2, "limit": 2}"#,
    );
    assert!(
        !changed.meta.unchanged && changed.content.contains("TWO"),
        "{changed:?}"
    );
}

#[test]
fn read_refuses_binary_files_and_offsets_past_the_end() {
    let w = World::new("binary");
    fs::write(w.world().join("src/b.bin"), [0u8, 1, 2, 3]).unwrap();
    w.put("src/short.txt", "a\n");
    let mut cx = w.cx();
    assert!(
        run(&mut cx, "read", r#"{"path": "src/b.bin"}"#)
            .content
            .contains("binary")
    );
    assert!(run(&mut cx, "read", r#"{"path": "src/short.txt", "offset": 9}"#).is_error);
}

#[test]
fn nothing_leaves_the_world() {
    let w = World::new("border");
    let mut cx = w.cx();
    for args in [
        r#"{"path": "../outside/secret.txt"}"#,
        r#"{"path": "/etc/hosts"}"#,
    ] {
        let o = run(&mut cx, "read", args);
        assert!(
            o.is_error && o.content.contains("outside the world"),
            "{args}: {o:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_world_is_refused_by_every_file_tool() {
    let w = World::new("symlink");
    let outside = w.root.join("outside");
    w.link("src/out", &outside);
    w.link("src/secret.txt", &outside.join("secret.txt"));
    let mut cx = w.cx();
    let attempts = [
        ("read", r#"{"path": "src/secret.txt"}"#),
        ("read", r#"{"path": "src/out/secret.txt"}"#),
        ("write", r#"{"path": "src/out/new.txt", "content": "x"}"#),
        ("write", r#"{"path": "src/secret.txt", "content": "x"}"#),
        (
            "edit",
            r#"{"path": "src/secret.txt", "old_string": "secret", "new_string": "gone"}"#,
        ),
        ("glob", r#"{"pattern": "*", "path": "src/out"}"#),
        ("grep", r#"{"pattern": "secret", "path": "src/out"}"#),
    ];
    for (name, args) in attempts {
        let o = run(&mut cx, name, args);
        assert!(
            o.is_error && o.content.contains("outside the world"),
            "{name} {args}: {o:?}"
        );
    }
    assert_eq!(
        fs::read_to_string(outside.join("secret.txt")).unwrap(),
        "secret\n"
    );
    assert!(!outside.join("new.txt").exists());
}

#[test]
fn writes_stay_in_the_soil() {
    let w = World::new("soil");
    let mut cx = w.cx();
    let o = run(&mut cx, "write", r#"{"path": "README.md", "content": "x"}"#);
    assert!(
        o.is_error && o.content.contains("outside your soil") && o.content.contains("src"),
        "{o:?}"
    );
    let o = run(
        &mut cx,
        "write",
        r#"{"path": "src/deep/new.rs", "content": "fn a() {}\n"}"#,
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(o.meta.class, Some(ToolClass::Write));
    assert_eq!(o.meta.wrote.len(), 1);
    assert_eq!(w.get("src/deep/new.rs"), "fn a() {}\n");
}

#[test]
fn a_write_resets_what_read_withholds() {
    let w = World::new("reset");
    w.put("src/a.txt", "one\n");
    let mut cx = w.cx();
    run(&mut cx, "read", r#"{"path": "src/a.txt"}"#);
    run(
        &mut cx,
        "write",
        r#"{"path": "src/a.txt", "content": "two\n"}"#,
    );
    let o = run(&mut cx, "read", r#"{"path": "src/a.txt"}"#);
    assert!(
        !o.meta.unchanged,
        "after a write the file is read in full again"
    );
}

#[test]
fn edit_is_strict_and_says_where() {
    let w = World::new("edit");
    w.put(
        "src/lib.rs",
        "fn alpha() {}\nfn beta() {}\nfn alpha_two() {}\n",
    );
    let mut cx = w.cx();

    let none = run(
        &mut cx,
        "edit",
        r#"{"path": "src/lib.rs", "old_string": "fn betta() {}", "new_string": "x"}"#,
    );
    assert!(
        none.is_error && none.content.contains("closest line is 2: `fn beta() {}`"),
        "{none:?}"
    );

    let many = run(
        &mut cx,
        "edit",
        r#"{"path": "src/lib.rs", "old_string": "fn alpha", "new_string": "fn a"}"#,
    );
    assert!(
        many.is_error
            && many.content.contains("occurs 2 times")
            && many.content.contains("lines 1, 3"),
        "{many:?}"
    );

    let one = run(
        &mut cx,
        "edit",
        r#"{"path": "src/lib.rs", "old_string": "fn beta()", "new_string": "fn gamma()"}"#,
    );
    assert!(
        !one.is_error && one.content.contains("from line 2"),
        "{one:?}"
    );
    assert_eq!(one.meta.wrote.len(), 1);

    let all = run(
        &mut cx,
        "edit",
        r#"{"path": "src/lib.rs", "old_string": "fn alpha", "new_string": "fn a", "replace_all": true}"#,
    );
    assert!(!all.is_error, "{all:?}");
    assert_eq!(
        w.get("src/lib.rs"),
        "fn a() {}\nfn gamma() {}\nfn a_two() {}\n"
    );

    assert!(
        run(
            &mut cx,
            "edit",
            r#"{"path": "src/lib.rs", "old_string": "", "new_string": "x"}"#
        )
        .is_error
    );
    assert!(
        run(
            &mut cx,
            "edit",
            r#"{"path": "src/lib.rs", "old_string": "fn a", "new_string": "fn a"}"#
        )
        .is_error
    );
}

#[test]
fn glob_lists_honours_gitignore_and_skips_git() {
    let w = World::new("glob");
    w.put("src/a.rs", "");
    w.put("src/nested/b.rs", "");
    w.put("src/c.txt", "");
    w.put("target/built.rs", "");
    w.put(".git/HEAD.rs", "");
    w.put(".gitignore", "target/\n");
    fs::create_dir_all(w.world().join(".git")).unwrap();
    let mut cx = w.cx();
    let o = run(&mut cx, "glob", r#"{"pattern": "**/*.rs"}"#);
    assert_eq!(o.content, "src/a.rs\nsrc/nested/b.rs", "{o:?}");
    let o = run(&mut cx, "glob", r#"{"pattern": "*.rs", "path": "src"}"#);
    assert_eq!(o.content, "src/a.rs");
}

#[test]
fn grep_finds_lines_or_files() {
    let w = World::new("grep");
    w.put("src/a.rs", "fn main() {}\nlet x = 1;\n");
    w.put("src/b.md", "Main idea\n");
    fs::write(w.world().join("src/c.bin"), b"fn main\0").unwrap();
    let mut cx = w.cx();
    let o = run(&mut cx, "grep", r#"{"pattern": "fn main"}"#);
    assert_eq!(
        o.content, "src/a.rs:1: fn main() {}",
        "binary files are skipped"
    );
    let o = run(
        &mut cx,
        "grep",
        r#"{"pattern": "main", "ignore_case": true, "files_only": true}"#,
    );
    assert_eq!(o.content, "src/a.rs\nsrc/b.md");
    let o = run(
        &mut cx,
        "grep",
        r#"{"pattern": "main", "ignore_case": true, "glob": "*.md"}"#,
    );
    assert_eq!(o.content, "src/b.md:1: Main idea");
    assert!(run(&mut cx, "grep", r#"{"pattern": "("}"#).is_error);
}

#[test]
fn protected_files_are_not_written_by_tools() {
    let w = World::new("protected");
    fs::create_dir_all(w.world().join(".git/hooks")).unwrap();
    fs::write(w.world().join(".git/config"), "[core]\n").unwrap();
    let mut cx = Context::new(Race::Slime, w.world(), &["."]);
    for (name, args) in [
        (
            "write",
            r#"{"path": ".git/config", "content": "[core]\n\tfsmonitor = x\n"}"#,
        ),
        (
            "write",
            r##"{"path": ".git/hooks/pre-commit", "content": "#!/bin/sh\n"}"##,
        ),
        (
            "write",
            r#"{"path": ".mnemorph/log.md", "content": "forged"}"#,
        ),
        (
            "edit",
            r#"{"path": ".git/config", "old_string": "[core]", "new_string": "[x]"}"#,
        ),
    ] {
        let o = run(&mut cx, name, args);
        assert!(
            o.is_error && o.content.contains("is protected"),
            "{name} {args}: {o:?}"
        );
    }
    assert_eq!(
        fs::read_to_string(w.world().join(".git/config")).unwrap(),
        "[core]\n"
    );
    let o = run(
        &mut cx,
        "write",
        r#"{"path": ".mnemorph/shared/queue.jsonl", "content": "{}\n"}"#,
    );
    // The shared work queue moves only through the harness, never by a body's tool.
    assert!(o.is_error && o.content.contains("is protected"), "{o:?}");
}

fn use_tool(cx: &mut Context, name: &str, args: &serde_json::Value) -> Outcome {
    call(&registry(), name, &args.to_string(), cx)
}

/// Several edits, in order, all or none.
#[test]
fn multiedit_writes_all_or_nothing() {
    let w = World::new("multiedit");
    w.put("src/a.rs", "fn one() {}\nfn two() {}\n");
    let mut cx = w.cx();
    let o = use_tool(
        &mut cx,
        "multiedit",
        &serde_json::json!({"path": "src/a.rs", "edits": [
            {"old_string": "fn one()", "new_string": "fn uno()"},
            {"old_string": "fn uno() {}\nfn two()", "new_string": "fn uno() {}\nfn dos()"}
        ]}),
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(w.get("src/a.rs"), "fn uno() {}\nfn dos() {}\n");
    let o = use_tool(
        &mut cx,
        "multiedit",
        &serde_json::json!({"path": "src/a.rs", "edits": [
            {"old_string": "fn uno()", "new_string": "fn first()"},
            {"old_string": "fn missing()", "new_string": "x"}
        ]}),
    );
    assert!(
        o.is_error
            && o.content.starts_with("edit 2 of 2:")
            && o.content.ends_with("Nothing was written."),
        "{o:?}"
    );
    assert_eq!(w.get("src/a.rs"), "fn uno() {}\nfn dos() {}\n");
}

/// A move inside the soil, a directory with its files, never over an existing file, never out —
/// and inside a folder that holds another's spot, where a command's `mv` is refused.
#[test]
fn move_stays_inside_the_soil() {
    let w = World::new("move");
    w.put("src/a.rs", "a\n");
    w.put("src/dir/x.rs", "x\n");
    w.put("src/other/theirs.rs", "theirs\n");
    w.put("src/b.rs", "b\n");
    let mut cx =
        Context::new(Race::Slime, w.world(), &["."]).with_soil(&["src/"], &["src/other/"], false);
    let o = use_tool(
        &mut cx,
        "move",
        &serde_json::json!({"from": "src/a.rs", "to": "src/sub/renamed.rs"}),
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(w.get("src/sub/renamed.rs"), "a\n");
    assert!(!w.world().join("src/a.rs").exists());
    let o = use_tool(
        &mut cx,
        "move",
        &serde_json::json!({"from": "src/dir", "to": "src/moved"}),
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(w.get("src/moved/x.rs"), "x\n");
    for (from, to, why) in [
        ("src/b.rs", "src/sub/renamed.rs", "already exists"),
        ("src/b.rs", "src/other/b.rs", "outside your soil"),
        ("src/other/theirs.rs", "src/stolen.rs", "outside your soil"),
        ("src/b.rs", "../outside/b.rs", "outside the world"),
        ("src/nothing.rs", "src/n.rs", "does not exist"),
    ] {
        let o = use_tool(
            &mut cx,
            "move",
            &serde_json::json!({"from": from, "to": to}),
        );
        assert!(
            o.is_error && o.content.contains(why),
            "{from} → {to}: {o:?}"
        );
    }
    assert_eq!(w.get("src/other/theirs.rs"), "theirs\n");
    assert_eq!(w.get("src/b.rs"), "b\n");
}

/// A delete keeps a copy in the attic first; a directory, or a body with no attic, is refused.
#[test]
fn delete_keeps_a_copy_first() {
    let w = World::new("delete");
    w.put("src/gone.rs", "gone\n");
    w.put("src/dir/x.rs", "x\n");
    let mut bare = w.cx();
    let o = use_tool(
        &mut bare,
        "delete",
        &serde_json::json!({"path": "src/gone.rs"}),
    );
    assert!(o.is_error && o.content.contains("no attic"), "{o:?}");
    assert!(w.world().join("src/gone.rs").exists());
    let attic =
        mnemorph_tools::Attic::at(&w.root.join("attic"), &w.world()).expect("git is installed");
    let mut cx = w.cx().with_attic(attic);
    let o = use_tool(&mut cx, "delete", &serde_json::json!({"path": "src/dir"}));
    assert!(o.is_error && o.content.contains("is a directory"), "{o:?}");
    let o = use_tool(
        &mut cx,
        "delete",
        &serde_json::json!({"path": "src/gone.rs"}),
    );
    assert!(
        !o.is_error && o.content.contains("the attic keeps a copy"),
        "{o:?}"
    );
    assert!(!w.world().join("src/gone.rs").exists());
    let o = use_tool(
        &mut cx,
        "delete",
        &serde_json::json!({"path": "../outside/secret.txt"}),
    );
    assert!(o.is_error, "{o:?}");
}

/// What the review of the file tools found, each held: a delete with no copy in the attic deletes
/// nothing; a link is deleted or moved as a link; a folder holding another's spot, even an empty
/// one, never moves whole; a move into itself leaves nothing behind; a multiedit that changes
/// nothing writes nothing; git's own files are never a tool's.
#[cfg(unix)]
#[test]
fn the_file_tools_review_held() {
    let w = World::new("review");
    w.put(".gitignore", "*.env\n");
    w.put("src/secret.env", "KEY=1\n");
    w.put("src/real.rs", "real\n");
    w.put("src/r2.rs", "r2\n");
    w.put("src/dir/x.rs", "x\n");
    w.put("src/a.rs", "aaa\n");
    w.put(".git/HEAD", "ref: refs/heads/main\n");
    fs::create_dir_all(w.world().join("src/box/spot/deeper")).unwrap();
    w.link("src/l", &w.world().join("src/real.rs"));
    w.link("src/l2", &w.world().join("src/r2.rs"));
    let attic =
        mnemorph_tools::Attic::at(&w.root.join("attic"), &w.world()).expect("git is installed");
    let mut cx = Context::new(Race::Slime, w.world(), &["."])
        .with_soil(&["src/", ".git/"], &["src/box/spot/"], false)
        .with_attic(attic);

    let o = use_tool(
        &mut cx,
        "delete",
        &serde_json::json!({"path": "src/secret.env"}),
    );
    assert!(o.is_error && o.content.contains("keeps no copy"), "{o:?}");
    assert!(w.world().join("src/secret.env").exists());

    let o = use_tool(&mut cx, "delete", &serde_json::json!({"path": "src/l"}));
    assert!(!o.is_error, "{o:?}");
    assert!(
        w.world().join("src/l").symlink_metadata().is_err() && w.get("src/real.rs") == "real\n"
    );
    let o = use_tool(
        &mut cx,
        "move",
        &serde_json::json!({"from": "src/l2", "to": "src/m2"}),
    );
    assert!(!o.is_error, "{o:?}");
    assert!(
        w.world()
            .join("src/m2")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(w.get("src/r2.rs"), "r2\n");

    let o = use_tool(
        &mut cx,
        "move",
        &serde_json::json!({"from": "src/box", "to": "src/box2"}),
    );
    assert!(
        o.is_error && o.content.contains("holds another's spot"),
        "{o:?}"
    );
    assert!(w.world().join("src/box/spot/deeper").is_dir());

    let o = use_tool(
        &mut cx,
        "move",
        &serde_json::json!({"from": "src/dir", "to": "src/dir/a/b"}),
    );
    assert!(o.is_error && o.content.contains("into itself"), "{o:?}");
    assert!(!w.world().join("src/dir/a").exists());

    let o = use_tool(
        &mut cx,
        "multiedit",
        &serde_json::json!({"path": "src/a.rs", "edits": [
            {"old_string": "aaa", "new_string": "bbb"},
            {"old_string": "bbb", "new_string": "aaa"}
        ]}),
    );
    assert!(o.is_error && o.content.contains("as it was"), "{o:?}");

    let o = use_tool(
        &mut cx,
        "write",
        &serde_json::json!({"path": ".git/HEAD", "content": "x"}),
    );
    assert!(o.is_error && o.content.contains("is git's"), "{o:?}");
    assert_eq!(w.get(".git/HEAD"), "ref: refs/heads/main\n");
}

/// What the review of the review added: a failed move leaves no directory behind; a delete takes
/// an empty directory; a write of what is already there writes nothing; a file that is not text
/// is never rewritten by an edit.
#[test]
fn refusals_leave_the_tree_as_it_was() {
    let w = World::new("leave");
    w.put("src/a.rs", "a\n");
    w.put("src/taken.rs", "taken\n");
    fs::create_dir_all(w.world().join("src/empty")).unwrap();
    fs::write(w.world().join("src/bin.dat"), [0xff, 0xfe, b'x']).unwrap();
    let attic =
        mnemorph_tools::Attic::at(&w.root.join("attic"), &w.world()).expect("git is installed");
    let mut cx = w.cx().with_attic(attic);
    // A move into new directories makes them, and reports the file at both ends.
    let o = use_tool(
        &mut cx,
        "move",
        &serde_json::json!({"from": "src/a.rs", "to": "src/new/deep/a.rs"}),
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(o.meta.wrote.len(), 2, "{o:?}");
    let o = use_tool(&mut cx, "delete", &serde_json::json!({"path": "src/empty"}));
    assert!(
        !o.is_error && !w.world().join("src/empty").exists(),
        "{o:?}"
    );
    let o = use_tool(
        &mut cx,
        "write",
        &serde_json::json!({"path": "src/taken.rs", "content": "taken\n"}),
    );
    assert!(
        o.is_error && o.content.contains("already holds exactly that"),
        "{o:?}"
    );
    let o = use_tool(
        &mut cx,
        "edit",
        &serde_json::json!({"path": "src/bin.dat", "old_string": "x", "new_string": "y"}),
    );
    assert!(o.is_error && o.content.contains("not UTF-8 text"), "{o:?}");
    assert_eq!(
        fs::read(w.world().join("src/bin.dat")).unwrap(),
        [0xff, 0xfe, b'x']
    );
}

/// A unified diff across files, whole or not at all: hunks land where they say or at their one
/// match, files are created and deleted, and a hunk that does not apply writes nothing anywhere.
#[test]
fn patch_applies_whole_or_not_at_all() {
    let w = World::new("patch");
    w.put("src/a.rs", "one\ntwo\nthree\nfour\n");
    w.put("src/b.rs", "alpha\nbeta\n");
    w.put("src/gone.rs", "bye\n");
    let attic =
        mnemorph_tools::Attic::at(&w.root.join("attic"), &w.world()).expect("git is installed");
    let mut cx = w.cx().with_attic(attic);
    let diff = "\
--- a/src/a.rs
+++ b/src/a.rs
@@ -2,1 +2,1 @@
-two
+TWO
@@ -9,1 +9,2 @@
 four
+five
--- a/src/b.rs
+++ b/src/b.rs
@@ -2 +2 @@
-beta
\\ No newline at end of file
+gamma
\\ No newline at end of file
--- /dev/null
+++ b/src/new.rs
@@ -0,0 +1,2 @@
+fn new() {}
+
--- a/src/gone.rs
+++ /dev/null
@@ -1 +0,0 @@
-bye
";
    let o = use_tool(&mut cx, "patch", &serde_json::json!({"patch": diff}));
    assert!(!o.is_error, "{o:?}");
    assert_eq!(
        w.get("src/a.rs"),
        "one\nTWO\nthree\nfour\nfive\n",
        "the second hunk found its one match"
    );
    assert_eq!(w.get("src/b.rs"), "alpha\ngamma");
    assert_eq!(w.get("src/new.rs"), "fn new() {}\n\n");
    assert!(!w.world().join("src/gone.rs").exists());

    // A pure insertion after an earlier hunk grew the file: it lands shifted by that growth.
    w.put("src/c.rs", "1\n2\n3\n4\n");
    let shifted =
        "--- a/src/c.rs\n+++ b/src/c.rs\n@@ -1 +1,2 @@\n-1\n+1a\n+1b\n@@ -3,0 +5 @@\n+after 3\n";
    let o = use_tool(&mut cx, "patch", &serde_json::json!({"patch": shifted}));
    assert!(!o.is_error, "{o:?}");
    assert_eq!(w.get("src/c.rs"), "1a\n1b\n2\n3\nafter 3\n4\n");

    // A hunk that does not apply: nothing anywhere is written.
    let bad = "\
--- a/src/a.rs
+++ b/src/a.rs
@@ -1 +1 @@
-one
+ONE
--- a/src/b.rs
+++ b/src/b.rs
@@ -1 +1 @@
-not there
+x
";
    let o = use_tool(&mut cx, "patch", &serde_json::json!({"patch": bad}));
    assert!(
        o.is_error
            && o.content.contains("hunk 1 of `src/b.rs` does not apply")
            && o.content.ends_with("Nothing was written."),
        "{o:?}"
    );
    assert_eq!(w.get("src/a.rs"), "one\nTWO\nthree\nfour\nfive\n");

    for (diff, why) in [
        (
            "--- a/src/a.rs\n+++ b/src/z.rs\n@@ -1 +1 @@\n-one\n+x\n",
            "use move",
        ),
        (
            "--- a/../outside/secret.txt\n+++ b/../outside/secret.txt\n@@ -1 +1 @@\n-secret\n+x\n",
            "outside the world",
        ),
        ("just prose", "unified diff"),
        (
            "--- /dev/null\n+++ b/src/a.rs\n@@ -0,0 +1 @@\n+x\n",
            "already exists",
        ),
    ] {
        let o = use_tool(&mut cx, "patch", &serde_json::json!({"patch": diff}));
        assert!(o.is_error && o.content.contains(why), "{why}: {o:?}");
    }
    assert_eq!(
        fs::read_to_string(w.root.join("outside/secret.txt")).unwrap(),
        "secret\n"
    );
}

/// What the patch review found, each held.
#[test]
fn patch_review_held() {
    let w = World::new("patch-review");
    w.put("src/a.rs", "1\n2\n3\n4\n5\n6\n7\n8\n");
    w.put("src/crlf.txt", "one\r\ntwo\r\nthree\r\n");
    w.put("src/dup.txt", "a\nb\n\nc\na\nb\n");
    w.put("src/q.sql", "select 1;\n-- old note\nselect 2;\n");
    w.put("src/file", "a file, not a directory\n");
    let mut cx = w.cx();
    let p =
        |cx: &mut Context, diff: &str| use_tool(cx, "patch", &serde_json::json!({"patch": diff}));

    // A path named twice: the second part applies to the first's result.
    let o = p(
        &mut cx,
        "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-1\n+one\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -8 +8 @@\n-8\n+eight\n",
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(w.get("src/a.rs"), "one\n2\n3\n4\n5\n6\n7\neight\n");
    assert_eq!(o.meta.wrote.len(), 1);

    // CRLF stays CRLF.
    let o = p(
        &mut cx,
        "--- a/src/crlf.txt\n+++ b/src/crlf.txt\n@@ -2 +2 @@\n-two\n+TWO\n",
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(w.get("src/crlf.txt"), "one\r\nTWO\r\nthree\r\n");

    // A trailing blank line is the writer's separator, not context: the hunk stays where it says.
    let o = p(
        &mut cx,
        "--- a/src/dup.txt\n+++ b/src/dup.txt\n@@ -5,2 +5,2 @@\n a\n-b\n+B\n\n",
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(w.get("src/dup.txt"), "a\nb\n\nc\na\nB\n");

    // A removed `-- comment` line is a removed line, not a file header.
    let o = p(
        &mut cx,
        "--- a/src/q.sql\n+++ b/src/q.sql\n@@ -1,3 +1,2 @@\n select 1;\n--- old note\n select 2;\n",
    );
    assert!(!o.is_error, "{o:?}");
    assert_eq!(w.get("src/q.sql"), "select 1;\nselect 2;\n");

    // A patch that changes nothing writes nothing.
    let o = p(
        &mut cx,
        "--- a/src/q.sql\n+++ b/src/q.sql\n@@ -1 +1 @@\n select 1;\n",
    );
    assert!(o.is_error && o.content.contains("as it was"), "{o:?}");

    // A failure at the write puts back what landed before it.
    let o = p(
        &mut cx,
        "--- a/src/q.sql\n+++ b/src/q.sql\n@@ -1 +1 @@\n-select 1;\n+select 3;\n--- /dev/null\n+++ b/src/file/new.rs\n@@ -0,0 +1 @@\n+x\n",
    );
    assert!(
        o.is_error && o.content.contains("every file was put back"),
        "{o:?}"
    );
    assert_eq!(w.get("src/q.sql"), "select 1;\nselect 2;\n");

    // What a text patch cannot apply is refused by name, never skipped.
    for (diff, why) in [
        (
            "diff --git a/src/e b/src/e\nnew file mode 100644\nindex 0000000..e69de29\n",
            "no text hunks",
        ),
        (
            "diff --git a/i.png b/i.png\nBinary files a/i.png and b/i.png differ\n",
            "binary",
        ),
        (
            "diff --git a/src/a.rs b/src/a.rs\nold mode 100644\nnew mode 100755\n",
            "mode change",
        ),
    ] {
        let o = p(&mut cx, diff);
        assert!(o.is_error && o.content.contains(why), "{why}: {o:?}");
    }
}

/// The instruments a learned value model trains on: lines added and removed by a write, counted
/// as a multiset (a moved line is neither), and an edit whose text was not in the file.
#[test]
fn writes_count_their_lines_and_a_missed_edit_is_marked() {
    let w = World::new("instruments");
    let mut cx = w.cx();

    let o = run(
        &mut cx,
        "write",
        r#"{"path": "src/a.txt", "content": "one\ntwo\nthree\n"}"#,
    );
    assert!(!o.is_error, "{}", o.content);
    assert_eq!(
        (o.meta.lines_added, o.meta.lines_removed),
        (3, 0),
        "a new file adds every line"
    );

    let o = run(
        &mut cx,
        "write",
        r#"{"path": "src/a.txt", "content": "three\none\nfour\n"}"#,
    );
    assert_eq!(
        (o.meta.lines_added, o.meta.lines_removed),
        (1, 1),
        "two lines moved, one replaced"
    );

    let o = run(
        &mut cx,
        "edit",
        r#"{"path": "src/a.txt", "old_string": "four\n", "new_string": "five\nsix\n"}"#,
    );
    assert!(!o.is_error, "{}", o.content);
    assert_eq!((o.meta.lines_added, o.meta.lines_removed), (2, 1));
    assert!(!o.meta.edit_miss);

    // Text that is not in the file: the body edited what it had not read.
    let o = run(
        &mut cx,
        "edit",
        r#"{"path": "src/a.txt", "old_string": "seven", "new_string": "eight"}"#,
    );
    assert!(o.is_error && o.meta.edit_miss, "{}", o.content);

    // Text that is there twice is refused, but it is no miss.
    w.put("src/b.txt", "x\nx\n");
    let o = run(
        &mut cx,
        "edit",
        r#"{"path": "src/b.txt", "old_string": "x", "new_string": "y"}"#,
    );
    assert!(o.is_error && !o.meta.edit_miss, "{}", o.content);

    // A multiedit whose second edit misses is marked, and writes nothing.
    let o = run(
        &mut cx,
        "multiedit",
        r#"{"path": "src/a.txt", "edits": [{"old_string": "one", "new_string": "uno"}, {"old_string": "nine", "new_string": "ten"}]}"#,
    );
    assert!(o.is_error && o.meta.edit_miss, "{}", o.content);
    assert_eq!(w.get("src/a.txt"), "three\none\nfive\nsix\n");

    let o = run(
        &mut cx,
        "multiedit",
        r#"{"path": "src/a.txt", "edits": [{"old_string": "one", "new_string": "uno"}]}"#,
    );
    assert_eq!((o.meta.lines_added, o.meta.lines_removed), (1, 1));
}

/// An Elf writes the plan in its own seat — the one record a body may write — and nothing else
/// there: not its doc, not another's plan.
#[test]
fn a_body_writes_its_own_plan_and_no_other_record() {
    let w = World::new("own-plan");
    w.put(".mnemorph/elf/e/README.md", "# elf-e\n");
    w.put(".mnemorph/elf/f/README.md", "# elf-f\n");
    let mut cx = Context::new(Race::Elf, w.world(), &[".mnemorph/elf/e"]);
    let plan = run(
        &mut cx,
        "write",
        r#"{"path": ".mnemorph/elf/e/plan.md", "content": "- [ ] orc-o: the parser\n"}"#,
    );
    assert!(!plan.is_error, "{plan:?}");
    let doc = run(
        &mut cx,
        "write",
        r##"{"path": ".mnemorph/elf/e/README.md", "content": "# elf-e\n- **Verify:** `true`\n"}"##,
    );
    assert!(
        doc.is_error && doc.content.contains("is protected"),
        "{doc:?}"
    );
    let other = run(
        &mut cx,
        "write",
        r#"{"path": ".mnemorph/elf/f/plan.md", "content": "x\n"}"#,
    );
    assert!(other.is_error, "{other:?}");
}
