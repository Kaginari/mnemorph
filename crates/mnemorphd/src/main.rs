//! `mnemorphd` — the Mnemorph core. It speaks the client protocol on stdio; the CLI and the board
//! are its clients, and decide nothing.

use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "mnemorphd — the Mnemorph core

Usage: mnemorphd [--world <dir>]

Speaks the client protocol (JSON-RPC 2.0, one message per line) on stdin and stdout. The
`mnemorph` CLI starts it; run it by hand only to test a client.

Options:
  --world <dir>   the world to work in (default: the current directory)
  -h, --help      this help
  -V, --version   the version
";

fn main() -> ExitCode {
    let mut world = None;
    let mut args = std::env::args().skip(1);
    // Help and the version answer before any world is looked for.
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("mnemorphd {}", env!("CARGO_PKG_VERSION"));
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
        env!("CARGO_PKG_VERSION"),
        mnemorph_server::opener(world),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

fn fail(why: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "mnemorphd: {why}");
    ExitCode::FAILURE
}
