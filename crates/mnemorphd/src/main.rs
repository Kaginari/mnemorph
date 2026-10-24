//! `mnemorphd` — the Mnemorph core. It speaks the client protocol on stdio; the CLI and the board
//! are its clients, and decide nothing.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "mnemorphd — the Mnemorph core

Usage: mnemorphd [--world <dir>]
       mnemorphd onto owner <path>... [--world <dir>]

Speaks the client protocol (JSON-RPC 2.0, one message per line) on stdin and stdout. The
`mnemorph` CLI starts it; run it by hand only to test a client.

`onto owner` names each path's owner, the rule that gave it and the claim that matched, read
from the creature docs; it needs no key and opens no session. A path is relative to the
current directory, or absolute.

Options:
  --world <dir>   the world to work in (default: the current directory)
  -h, --help      this help
  -V, --version   the version
";

/// The version and the commit it was built from: `0.0.0 (a1b2c3d4e5)`.
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("MNEMORPH_COMMIT"),
    ")"
);

fn main() -> ExitCode {
    let mut world = None;
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if all.first().map(String::as_str) == Some("onto") {
        return onto(&all[1..]);
    }
    let mut args = all.into_iter();
    // Help and the version answer before any world is looked for.
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("mnemorphd {VERSION}");
                return ExitCode::SUCCESS;
            }
            "--world" => match args.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return fail("--world needs a directory"),
            },
            other => return fail(&format!("unknown argument `{other}`; see --help")),
        }
    }
    let world = match world.map_or_else(std::env::current_dir, Ok) {
        Ok(w) => w,
        Err(e) => return fail(&format!("the current directory: {e}")),
    };
    let world = match world.canonicalize() {
        Ok(w) if w.is_dir() => w,
        Ok(w) => return fail(&format!("{} is not a directory", w.display())),
        Err(e) => return fail(&format!("{}: {e}", world.display())),
    };
    let input = BufReader::new(std::io::stdin());
    match mnemorph_server::serve(
        input,
        std::io::stdout(),
        VERSION,
        mnemorph_server::opener(world),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

/// `onto owner <path>... [--world <dir>]`.
fn onto(args: &[String]) -> ExitCode {
    use mnemorph_server::world::{Owner, Roster, explain};
    let mut world = None;
    let mut paths = Vec::new();
    let mut words = args.iter();
    match words.next().map(String::as_str) {
        Some("owner") => {}
        Some(other) => return fail(&format!("onto has no `{other}`; see --help")),
        None => return fail("onto needs a command: owner; see --help"),
    }
    while let Some(w) = words.next() {
        if w == "--world" {
            match words.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return fail("--world needs a directory"),
            }
        } else if w.starts_with("--") {
            return fail(&format!("unknown argument `{w}`; see --help"));
        } else {
            paths.push(w.clone());
        }
    }
    if paths.is_empty() {
        return fail("onto owner needs a path");
    }
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    // Both sides canonical where they exist, so a symlinked path (macOS's `/var`) still matches.
    let canonical = |p: PathBuf| p.canonicalize().unwrap_or(p);
    let world = canonical(world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w)));
    let cwd = canonical(cwd);
    let roster = Roster::load(&world);
    for problem in &roster.problems {
        let _ = writeln!(std::io::stderr(), "mnemorphd: {problem}");
    }
    let mut out = std::io::stdout().lock();
    for path in &paths {
        // A path is the human's: relative to where they stand, read against the world.
        let full = absolute(&cwd, Path::new(path));
        // A path that exists is read through its links; one that does not, as written.
        let full = full.canonicalize().unwrap_or(full);
        let rel = full.strip_prefix(&world).map_or_else(
            |_| full.display().to_string(),
            |r| {
                let r = r.to_string_lossy().into_owned();
                if path.ends_with('/') {
                    format!("{r}/")
                } else {
                    r
                }
            },
        );
        let e = explain(&roster, &rel);
        let owner = match &e.owner {
            Owner::Creature(name) => name.clone(),
            Owner::Harness => "the harness".into(),
            Owner::Human => "the human".into(),
            Owner::Unowned => "nobody".into(),
        };
        let why = e.claim.map_or_else(
            || e.rule.words().to_owned(),
            |c| format!("{} {c}", e.rule.words()),
        );
        let _ = writeln!(out, "{path}\t{owner}\t{why}");
    }
    ExitCode::SUCCESS
}

/// `path` made absolute against `cwd` and read segment by segment, without touching the disk:
/// a path that does not exist yet still has an owner.
fn absolute(cwd: &Path, path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in cwd.join(path).components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn fail(why: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "mnemorphd: {why}");
    ExitCode::FAILURE
}
