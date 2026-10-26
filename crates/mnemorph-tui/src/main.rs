//! `mnemorph-tui` — a Mnemorph session in the terminal, the core hosted in this process.

use std::path::PathBuf;
use std::process::ExitCode;

use mnemorph_tui::app::Model;
use mnemorph_tui::kit::highlight;
use mnemorph_tui::pictures::Pictures;
use mnemorph_tui::probe::{Late, probe};
use mnemorph_tui::run::{Host, channel, gate, header, observer, run};
use mnemorph_tui::theme::Variant;

const HELP: &str = "mnemorph-tui — a Mnemorph session in the terminal

Usage: mnemorph-tui [--world <dir>] [--theme dark|light|auto] [--probe]

Options:
  --world <dir>     the world to work in (default: the current directory)
  --theme <name>    dark, light, or auto: MNEMORPH_THEME, else the terminal's background
  --probe           print what the terminal can draw (the portraits' image protocol), and exit
  -h, --help        this help
  -V, --version     the version

Portraits are real pixels where the terminal draws images — Kitty, iTerm2's inline images
(iTerm2, WezTerm, VS Code with terminal.integrated.enableImages), Sixel — else half blocks.
MNEMORPH_TUI_IMAGES=auto|kitty|iterm|sixel|blocks|off overrides the choice.

Keys: enter sends (queued while a turn runs) · alt+enter breaks the line · ↑↓ history ·
pgup/pgdn or the wheel scroll · esc stops the turn · ctrl+c stops, then quits ·
ctrl+o the sidebar · alt+↑↓ pick a card, ctrl+t or a click expands it · at an approval, 1 (y)
yes, 2 (n) no or 3 no with a reason, then enter. The mouse is the UI's: hold shift to select
text.
";

fn main() -> ExitCode {
    let mut world = None;
    let mut theme = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("mnemorph-tui {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--probe" => {
                print!("{}", probe(&|k| std::env::var(k).ok()).report());
                return ExitCode::SUCCESS;
            }
            "--world" => match args.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return fail("--world needs a directory"),
            },
            "--theme" => match args.next() {
                Some(t) if ["dark", "light", "auto"].contains(&t.as_str()) => theme = Some(t),
                _ => return fail("--theme is dark, light or auto"),
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
    let variant = Variant::choose(theme.as_deref(), |k| std::env::var(k).ok());
    highlight::warm();
    let (tx, rx) = channel();
    // The session opens before the terminal is taken, so a missing key reads as plain text.
    let opened = match mnemorph_loop::open(&world, gate(tx.clone()), observer(tx.clone()), |v| {
        std::env::var(v).ok()
    }) {
        Ok(o) => o,
        Err(e) => return fail(&e),
    };
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut model = Model::new(header(&opened, home.as_deref()), variant, (80, 24));
    model.sidebar.root = Some(opened.world.clone());
    model.session_id.clone_from(&opened.session_id);
    model.info(format!(
        "{} via {} · sandbox {}",
        opened.model, opened.provider, opened.sandbox
    ));
    for hole in &opened.holes {
        model.info(format!("config hole: {hole}"));
    }
    // What the terminal draws, asked before the screen is taken: within 200 ms, never blocking.
    let caps = probe(&|k| std::env::var(k).ok());
    for note in &caps.notes {
        model.info(format!("portraits: {note}"));
    }
    model.pictures = Pictures::new(caps.protocol, caps.cell);
    if caps.timed_out {
        model.late = Late::after(std::time::Instant::now());
    }
    let host = match Host::start(opened, tx.clone()) {
        Ok(h) => h,
        Err(e) => return fail(&e),
    };
    // The sidebar's memory and races come from the world, read off the UI thread.
    host.run(model.survey());
    let result = run(model, &host, tx, &rx);
    host.stop();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("the terminal: {e}")),
    }
}

fn fail(why: &str) -> ExitCode {
    eprintln!("mnemorph-tui: {why}");
    ExitCode::FAILURE
}
