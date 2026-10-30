use std::fmt::Write as _;
use std::path::Path;

use super::{MAX_SYMBOLS, find, render, skeleton};

/// A fixture's skeleton as the model reads it.
fn outline(name: &str, text: &str) -> String {
    render(name, &skeleton(Path::new(name), text), MAX_SYMBOLS)
}

#[test]
fn rust_items_with_their_docs_attributes_and_impl_methods() {
    assert_eq!(
        outline("a.rs", include_str!("fixtures/a.rs")),
        "a.rs — rust, 49 lines
imports 3–4 (2): use std::fmt · use std::io::{self, Read}
6–7 const ANSWER
9–13 struct Stack
15–24 impl Stack<T>
  16–19 fn new
  21–23 fn push
26–30 impl fmt::Display for Stack<u8>
  27–29 fn fmt
32–34 trait Shape
  33–33 fn area
36–39 enum Kind
41–43 mod inner
  42–42 fn helper
45–49 macro twice
"
    );
}

#[test]
fn typescript_declarations_classes_and_namespaces() {
    assert_eq!(
        outline("a.ts", include_str!("fixtures/a.ts")),
        r#"a.ts — typescript, 37 lines
imports 1–2 (2): import { readFile } from "node:fs" · import * as path from "node:path"
4–4 const LIMIT
6–8 fn load
10–10 fn twice
12–14 interface Shape
16–16 type Id
18–28 class Stack
  21–23 method push
  25–27 method size
30–33 enum Color
35–37 namespace Util
  36–36 fn noop
"#
    );
}

#[test]
fn javascript_through_the_tsx_grammar() {
    assert_eq!(
        outline("a.js", include_str!("fixtures/a.js")),
        r#"a.js — javascript, 14 lines
imports 2–2 (1): import { join } from "path"
1–1 const fs
4–6 fn main
8–12 class Widget
  9–11 method render
14–14 var counter
"#
    );
}

#[test]
fn python_classes_methods_decorators_and_constants() {
    assert_eq!(
        outline("a.py", include_str!("fixtures/a.py")),
        "a.py — python, 28 lines
imports 1–2 (2): import os · from typing import List
4–4 const MAX
7–8 fn top
11–23 class Stack
  14–15 method __init__
  17–19 method size
  21–23 class Inner
    22–23 method deep
26–28 fn wrapped
"
    );
}

#[test]
fn go_types_methods_and_functions() {
    assert_eq!(
        outline("a.go", include_str!("fixtures/a.go")),
        r#"a.go — go, 26 lines
imports 4–5 (2): "fmt" · "os"
8–8 const Limit
10–13 struct Stack
15–17 interface Shape
19–22 method Stack.Push
24–26 fn main
"#
    );
}

#[test]
fn markdown_headings_nest_by_level_and_skip_fences() {
    assert_eq!(
        outline("a.md", include_str!("fixtures/a.md")),
        "a.md — markdown, 19 lines
1–19 # Title
  5–15 ## First
    13–15 ### Deep
  17–19 ## Second
"
    );
}

#[test]
fn toml_tables_skip_multiline_strings() {
    assert_eq!(
        outline("a.toml", include_str!("fixtures/a.toml")),
        "a.toml — toml, 13 lines
3–4 table package
6–10 table dependencies
12–13 array bin
"
    );
}

#[test]
fn json_keys_through_objects_not_arrays() {
    assert_eq!(
        outline("a.json", include_str!("fixtures/a.json")),
        "a.json — json, 10 lines
2–2 key name
3–6 key scripts
  4–4 key build
  5–5 key test
7–9 key list
"
    );
}

#[test]
fn an_unknown_language_has_a_line_count_and_no_skeleton() {
    assert_eq!(
        outline("run.sh", "echo a\necho b\n"),
        "run.sh — 2 lines; no skeleton for this file type: read it by range, or grep.\n"
    );
}

#[test]
fn nested_impls_show_and_deeper_nesting_collapses_with_a_count() {
    let text = "mod outer {
    impl A {
        fn one() {}
    }
    mod inner {
        impl B {
            fn two() {}
            fn three() {}
        }
    }
}
";
    assert_eq!(
        outline("n.rs", text),
        "n.rs — rust, 11 lines
1–11 mod outer
  2–4 impl A
    3–3 fn one
  5–10 mod inner
    6–9 impl B (+2 nested)
"
    );
}

#[test]
fn the_cap_counts_what_it_leaves_out() {
    let mut text = String::new();
    for i in 0..250 {
        let _ = writeln!(text, "fn f{i}() {{}}");
    }
    let out = outline("many.rs", &text);
    assert_eq!(out.lines().count(), 1 + MAX_SYMBOLS + 1);
    assert!(out.contains("200–200 fn f199\n"), "{out}");
    assert!(!out.contains("fn f200\n"), "{out}");
    assert!(
        out.ends_with("… 50 more symbols from line 201: read by range from there.\n"),
        "{out}"
    );
}

#[test]
fn a_symbol_is_found_bare_or_by_its_parent() {
    let s = skeleton(Path::new("a.rs"), include_str!("fixtures/a.rs"));
    let names =
        |q: &str| -> Vec<(usize, usize)> { find(&s, q).iter().map(|x| (x.start, x.end)).collect() };
    assert_eq!(names("push"), vec![(21, 23)]);
    assert_eq!(names("Stack::push"), vec![(21, 23)]);
    assert_eq!(names("Shape::area"), vec![(33, 33)]);
    // Two symbols named `Stack`: the struct and its impl.
    assert_eq!(names("Stack"), vec![(9, 13)]);
    assert_eq!(names("Kind::push"), Vec::<(usize, usize)>::new());
    let go = skeleton(Path::new("a.go"), include_str!("fixtures/a.go"));
    assert_eq!(find(&go, "Stack.Push").len(), 1);
    assert_eq!(find(&go, "Push").len(), 1);
}

#[test]
fn a_deeply_nested_file_has_a_skeleton_and_no_stack_overflow() {
    let depth = 100_000;
    let rust = format!(
        "fn f() {{\n    let x = {}{};\n}}\n",
        "[".repeat(depth),
        "]".repeat(depth)
    );
    let out = outline("deep.rs", &rust);
    assert!(
        out.contains("1–3 fn f") || out.contains("took too long"),
        "{out}"
    );
    let python = format!(
        "def f():\n    return {}{}\n",
        "(".repeat(depth),
        ")".repeat(depth)
    );
    let out = outline("deep.py", &python);
    assert!(
        out.contains("1–2 fn f") || out.contains("took too long"),
        "{out}"
    );
    let json = format!("{}{}", "{\"a\": ".repeat(depth), "}".repeat(depth));
    assert!(outline("deep.json", &json).contains("1–1 key a (+"));
}

#[test]
fn a_large_garbage_file_stops_at_the_parse_budget() {
    // Four megabytes of brackets, quotes and operators: error recovery's worst case.
    let alphabet = b"{}()[]<>'\"`/*;=+-!?:.,\\ \nfnclassdef";
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let text: String = (0..(4 << 20) - 64)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            char::from(alphabet[usize::try_from(x % alphabet.len() as u64).unwrap()])
        })
        .collect();
    let started = std::time::Instant::now();
    let out = outline("garbage.ts", &text);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert!(out.starts_with("garbage.ts — "), "{out}");
}

#[test]
fn control_characters_in_names_never_reach_the_skeleton() {
    let out = outline(
        "forged.json",
        "{\"a\n2–2 key admin\": 1, \"b\u{1b}[31m\": 2}\n",
    );
    assert!(!out.contains('\u{1b}'), "{out:?}");
    assert_eq!(out.lines().count(), 3, "{out:?}");
    assert!(out.contains("–2 key a 2–2 key admin\n"), "{out:?}");
}

#[test]
fn a_trait_impl_with_generics_is_found_by_its_type() {
    let s = skeleton(
        Path::new("it.rs"),
        "struct Foo;\nimpl Iterator<Item = u8> for Foo {\n    fn next(&mut self) -> Option<u8> { None }\n}\n",
    );
    let found: Vec<(usize, usize)> = find(&s, "Foo::next")
        .iter()
        .map(|x| (x.start, x.end))
        .collect();
    assert_eq!(found, vec![(3, 3)]);
}
