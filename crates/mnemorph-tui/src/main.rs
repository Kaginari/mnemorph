//! `mnemorph` — a Mnemorph session in the terminal, the core hosted in this process: the terminal
//! app with no subcommand, one ask without it with `run`.

use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use mnemorph_tui::app::Model;
use mnemorph_tui::cli::{Command, HELP, VERSION, parse};
use mnemorph_tui::headless;
use mnemorph_tui::kit::highlight;
use mnemorph_tui::pictures::Pictures;
use mnemorph_tui::probe::{Late, probe};
use mnemorph_tui::run::{Host, channel, gate, header, observer, run};
use mnemorph_tui::theme::Variant;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match parse(&args) {
        Ok(c) => c,
        Err(e) => return fail(&e, 2),
    };
    match command {
        Command::Help => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("{VERSION}");
            ExitCode::SUCCESS
        }
        Command::Probe => {
            print!("{}", probe(&|k| std::env::var(k).ok()).report());
            ExitCode::SUCCESS
        }
        Command::Chat {
            world,
            resume,
            theme,
        } => chat(world, resume.as_deref(), theme.as_deref()),
        Command::Run {
            ask,
            world,
            json,
            gate_tty,
        } => ask_once(ask, world, json, gate_tty),
        Command::Status { world } => status(world),
    }
}

/// The world: the directory given, or the current one, canonical.
fn world_of(world: Option<PathBuf>) -> Result<PathBuf, String> {
    let world = world
        .map_or_else(std::env::current_dir, Ok)
        .map_err(|e| format!("the current directory: {e}"))?;
    match world.canonicalize() {
        Ok(w) if w.is_dir() => Ok(w),
        Ok(w) => Err(format!("{} is not a directory", w.display())),
        Err(e) => Err(format!("{}: {e}", world.display())),
    }
}

/// `run`: one ask, without the app.
fn ask_once(ask: Option<String>, world: Option<PathBuf>, json: bool, gate_tty: bool) -> ExitCode {
    let ask = match ask {
        Some(a) => a,
        None if std::io::stdin().is_terminal() => {
            return fail(
                "run needs an ask: mnemorph run \"<ask>\", or one on stdin",
                2,
            );
        }
        None => {
            let mut text = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut text) {
                return fail(&format!("stdin: {e}"), 2);
            }
            text
        }
    };
    let ask = ask.trim();
    if ask.is_empty() {
        return fail("run: nothing to ask", 2);
    }
    let world = match world_of(world) {
        Ok(w) => w,
        Err(e) => return fail(&e, 2),
    };
    let (tx, rx) = headless::channel();
    let gate = headless::gate(tx.clone(), gate_tty);
    let opened = mnemorph_loop::open(&world, gate.clone(), headless::observer(tx.clone()), |v| {
        std::env::var(v).ok()
    });
    let opened = match opened {
        Ok(o) => o,
        Err(e) => return fail(&e, 2),
    };
    #[cfg(unix)]
    if let Err(e) = headless::on_ctrl_c(opened.session.interrupt(), gate) {
        return fail(&format!("ctrl+c could not be caught: {e}"), 2);
    }
    let options = headless::Options {
        json,
        escape: std::io::stdout().is_terminal(),
    };
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr());
    match headless::run(opened, ask, options, (tx, rx), &mut out, &mut err) {
        Ok(code) => ExitCode::from(code),
        Err(e) => fail(&e, 1),
    }
}

/// `status`: what a session here would open with.
fn status(world: Option<PathBuf>) -> ExitCode {
    let world = match world_of(world) {
        Ok(w) => w,
        Err(e) => return fail(&e, 2),
    };
    let opened = mnemorph_loop::open(
        &world,
        Arc::new(mnemorph_tools::NoHuman),
        Arc::new(|_: &mnemorph_loop::Seen| {}),
        |v| std::env::var(v).ok(),
    );
    match opened {
        Ok(opened) => {
            for line in headless::status(&opened) {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("{:<9}{}", "mnemorph", env!("CARGO_PKG_VERSION"));
            println!(
                "{:<9}did not open — {}",
                "session",
                mnemorph_tui::safe::one_line(&e)
            );
            ExitCode::FAILURE
        }
    }
}

/// The terminal app.
fn chat(world: Option<PathBuf>, resume: Option<&str>, theme: Option<&str>) -> ExitCode {
    let world = match world_of(world) {
        Ok(w) => w,
        Err(e) => return fail(&e, 1),
    };
    let variant = Variant::choose(theme, |k| std::env::var(k).ok());
    highlight::warm();
    let (tx, rx) = channel();
    // The session opens before the terminal is taken, so a missing key reads as plain text.
    let env = |v: &str| std::env::var(v).ok();
    let opened = match resume {
        Some(run) => {
            mnemorph_loop::resume(&world, run, gate(tx.clone()), observer(tx.clone()), env)
        }
        None => mnemorph_loop::open(&world, gate(tx.clone()), observer(tx.clone()), env),
    };
    let opened = match opened {
        Ok(o) => o,
        Err(e) => return fail(&e, 1),
    };
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut model = Model::new(header(&opened, home.as_deref()), variant, (80, 24));
    model.sidebar.root = Some(opened.world.clone());
    model.session_id.clone_from(&opened.session_id);
    model.info(format!(
        "{} via {} · sandbox {}",
        opened.model, opened.provider, opened.sandbox
    ));
    if resume.is_some() {
        model.info(format!(
            "resumed run {} from its journal: ask to go on",
            opened.run_id
        ));
    }
    for hole in &opened.holes {
        model.info(format!("config hole: {hole}"));
    }
    // What the terminal draws, asked before the screen is taken: within 200 ms, never blocking.
    let caps = probe(&|k| std::env::var(k).ok());
    for note in &caps.notes {
        model.info(format!("portraits: {note}"));
    }
    model.pictures = Pictures::new(caps.protocol, caps.cell);
    // Without pixels, the portraits are glyphs as fine as the terminal's font draws.
    mnemorph_tui::kit::mnanim::set_glyphs(caps.glyphs);
    if caps.timed_out {
        model.late = Late::after(std::time::Instant::now());
    }
    let host = match Host::start(opened, tx.clone()) {
        Ok(h) => h,
        Err(e) => return fail(&e, 1),
    };
    // The sidebar's memory and races come from the world, read off the UI thread.
    host.run(model.survey());
    let result = run(model, &host, tx, &rx);
    host.stop();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("the terminal: {e}"), 1),
    }
}

/// Say why on stderr, as `mnemorph: <why>` — a script reads the last such line — and exit `code`.
fn fail(why: &str, code: u8) -> ExitCode {
    eprintln!("mnemorph: {}", mnemorph_tui::safe::one_line(why));
    ExitCode::from(code)
}
