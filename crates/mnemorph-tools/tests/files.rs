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
        r#"{"path": "src/a.txt", "content": "one\n"}"#,
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
