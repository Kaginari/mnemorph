//! Every command a prompt or a hint names exists in the build: a `mnemorph …` or `mnemorphd …`
//! command written in backticks anywhere in the shipped sources is one the help of the CLI or the
//! core lists — so no hint ever tells a model to run what the binary does not ship.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.filter_map(Result::ok) {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() {
            if name != "node_modules" && name != "tests" && name != "target" {
                sources(&p, out);
            }
        } else if p.extension().is_some_and(|x| {
            ["rs", "ts", "tsx"]
                .iter()
                .any(|e| x.eq_ignore_ascii_case(e))
        }) && !name.contains(".test.")
        {
            out.push(p);
        }
    }
}

/// The words after `prog ` in `line`, while they are command words.
fn words_after<'a>(line: &'a str, prog: &str) -> Vec<Vec<&'a str>> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find(prog) {
        let after = &rest[i + prog.len()..];
        let ws: Vec<&str> = after
            .split(|c: char| c.is_whitespace() || c == '`')
            .take_while(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
            .take(2)
            .collect();
        if !ws.is_empty() {
            found.push(ws);
        }
        rest = after;
    }
    found
}

/// What a help text lists: each command, with the subcommand words it takes.
fn listed(help: &str, prog: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in help.lines() {
        for ws in words_after(line, &format!("{prog} ")) {
            let subs = out.entry(ws[0].to_owned()).or_default();
            if let Some(sub) = ws.get(1) {
                subs.insert((*sub).to_owned());
            }
            // `q list|show|add …` lists its words joined by bars.
            let after = line
                .split(&format!("{prog} {} ", ws[0]))
                .nth(1)
                .unwrap_or("");
            if let Some(first) = after.split_whitespace().next()
                && first.contains('|')
            {
                subs.extend(first.split('|').map(str::to_owned));
            }
        }
    }
    out
}

#[test]
fn every_command_a_prompt_names_exists_in_the_build() {
    let root = repo();
    let cli = std::fs::read_to_string(root.join("packages/cli/src/args.ts")).unwrap();
    let core = std::fs::read_to_string(root.join("crates/mnemorphd/src/main.rs")).unwrap();
    let cli_help = cli
        .split("export const HELP = `")
        .nth(1)
        .unwrap()
        .split('`')
        .next()
        .unwrap();
    let core_help = core
        .split("const HELP: &str = \"")
        .nth(1)
        .unwrap()
        .split("\";")
        .next()
        .unwrap();
    let mut known = listed(cli_help, "mnemorph");
    for (cmd, subs) in listed(core_help, "mnemorphd") {
        known.entry(cmd).or_default().extend(subs);
    }
    assert!(
        known.contains_key("memory") && known.contains_key("q"),
        "{known:?}"
    );
    let mut files = Vec::new();
    sources(&root.join("crates"), &mut files);
    sources(&root.join("packages"), &mut files);
    let mut unknown = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            for prog in ["`mnemorph ", "`mnemorphd "] {
                // A command is a short backticked span; a template string of prose that opens on
                // the program's name (`mnemorphd could not start: ${e}`) is none.
                let spans: Vec<&str> = line
                    .match_indices(prog)
                    .map(|(i, _)| {
                        let rest = &line[i + 1..];
                        rest.split('`').next().unwrap_or(rest)
                    })
                    .filter(|span| !span.contains("${") && span.split_whitespace().count() <= 8)
                    .collect();
                for ws in spans.iter().flat_map(|span| words_after(span, &prog[1..])) {
                    let ok = known.get(ws[0]).is_some_and(|subs| {
                        subs.is_empty()
                            || ws.get(1).is_none_or(|s| {
                                subs.contains(*s)
                                    || !subs
                                        .iter()
                                        .any(|k| k.chars().all(|c| c.is_ascii_lowercase()))
                            })
                    });
                    if !ok {
                        unknown.push(format!(
                            "{}:{}: {}{}",
                            f.display(),
                            n + 1,
                            prog.trim_start_matches('`'),
                            ws.join(" ")
                        ));
                    }
                }
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "named but not in the build:\n{}",
        unknown.join("\n")
    );
}
