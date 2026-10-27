//! The `mnemorph` command line, read before anything else: help and the version never look for
//! a world. No subcommand opens the terminal app; `run` asks once without it.

use std::path::PathBuf;

/// The help, for `-h`, `--help` and `help`.
pub const HELP: &str = "mnemorph — a Mnemorph session in your terminal

Usage:
  mnemorph [options]                 a session in this world, in the terminal app
  mnemorph chat [options]            the same
  mnemorph resume <run|last>         a session on a run that stopped, picked up from its journal
  mnemorph run \"<ask>\" [options]     ask once without the app: the answer on stdout, exit
  mnemorph run [-]                   ask once, the ask read from stdin
  mnemorph status [--world <dir>]    the build, the world, the model, the key's origin, the
                                     sandbox, the budget and the configuration's holes
  mnemorph version | help

The world's admin commands are the core's: `mnemorphd runs`, `mnemorphd memory status`,
`mnemorphd onto check`, `mnemorphd q list`, `mnemorphd incident list`, `mnemorphd housekeep`,
`mnemorphd relief` (see mnemorphd --help).

Options:
  --world <dir>     the world to work in (default: the current directory)
  --resume <run>    the same as `resume <run>`
  --theme <name>    dark, light, or auto: MNEMORPH_THEME, else the terminal's background
  --probe           print what the terminal can draw (the portraits' image protocol), and exit
  --json            with run: every event as a JSON line on stdout, then a `result` line
  --gate-tty        with run: put each gated act to you on the terminal (/dev/tty); without it
                    run denies every gated act, since no one is there to approve it
  -h, --help        this help
  -V, --version     the version

`run` goes through the same session as the app, its end gate included. Progress goes to stderr,
the answer to stdout; any ending but an answer is said on stderr's last line as
`mnemorph: <ending>`. Exit codes: 0 answered and the gate passed; 1 failed, escalated or
interrupted (a failed end gate included); 2 the session could not start or the command line
did not read; 3 stopped at a checkpoint (`mnemorph resume <run>` goes on).

A run that stopped — at a checkpoint, an interrupt, or a turn the process died in — is resumed
through the same opening as a fresh session, the end gate included, on the model it rode unless
the configuration names another. `mnemorphd runs` lists them.

Portraits are real pixels where the terminal draws images — Kitty, iTerm2's inline images
(iTerm2, WezTerm, VS Code with terminal.integrated.enableImages), Sixel — else glyphs.
MNEMORPH_TUI_IMAGES=auto|kitty|iterm|sixel|blocks|off overrides the choice, and
MNEMORPH_TUI_GLYPHS=half|quadrants|sextants|octants the glyphs.

Keys: enter sends (queued while a turn runs) · alt+enter breaks the line · ↑↓ history ·
pgup/pgdn or the wheel scroll · esc stops the turn · ctrl+c stops, then quits ·
ctrl+o the sidebar · alt+↑↓ pick a card, ctrl+t or a click expands it · at an approval, 1 (y)
yes, 2 (n) no or 3 no with a reason, then enter. The mouse is the UI's: hold shift to select
text.
";

/// The version: `mnemorph 0.0.0`.
pub const VERSION: &str = concat!("mnemorph ", env!("CARGO_PKG_VERSION"));

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// The help.
    Help,
    /// The version.
    Version,
    /// What the terminal can draw.
    Probe,
    /// The terminal app.
    Chat {
        /// The world, when not the current directory.
        world: Option<PathBuf>,
        /// The stopped run to pick up — its id, or `last`.
        resume: Option<String>,
        /// `dark`, `light` or `auto`.
        theme: Option<String>,
    },
    /// One ask without the app.
    Run {
        /// The ask; `None` reads it from stdin.
        ask: Option<String>,
        /// The world, when not the current directory.
        world: Option<PathBuf>,
        /// Every event as a JSON line, then a result line.
        json: bool,
        /// Gated acts are put to the human on `/dev/tty`, not denied.
        gate_tty: bool,
    },
    /// What the session would open with.
    Status {
        /// The world, when not the current directory.
        world: Option<PathBuf>,
    },
}

/// The options, wherever they stand, and the words between them.
#[derive(Default)]
struct Flags {
    world: Option<PathBuf>,
    resume: Option<String>,
    theme: Option<String>,
    probe: bool,
    json: bool,
    gate_tty: bool,
    words: Vec<String>,
}

fn flags(args: &[String]) -> Result<Flags, String> {
    let mut f = Flags::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--world" => match it.next() {
                Some(dir) if !dir.starts_with("--") => f.world = Some(PathBuf::from(dir)),
                _ => return Err("--world needs a directory".into()),
            },
            "--resume" => match it.next() {
                Some(run) if !run.starts_with('-') => f.resume = Some(run.clone()),
                _ => return Err("--resume needs a run's id, or last".into()),
            },
            "--theme" => match it.next() {
                Some(t) if ["dark", "light", "auto"].contains(&t.as_str()) => {
                    f.theme = Some(t.clone());
                }
                _ => return Err("--theme is dark, light or auto".into()),
            },
            // The session runs in this process: a core's path is read and has no use.
            "--core" => {
                if it.next().is_none() {
                    return Err("--core needs a path".into());
                }
            }
            "--probe" => f.probe = true,
            "--json" => f.json = true,
            "--gate-tty" => f.gate_tty = true,
            other if other.starts_with("--") => {
                return Err(format!("unknown option `{other}`; see mnemorph --help"));
            }
            _ => f.words.push(a.clone()),
        }
    }
    Ok(f)
}

/// Read the arguments (without the program's name).
///
/// # Errors
///
/// What did not read, for the human.
pub fn parse(args: &[String]) -> Result<Command, String> {
    // Help and the version win wherever they appear: nothing else is read.
    if args.iter().any(|a| a == "-h" || a == "--help") {
        return Ok(Command::Help);
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        return Ok(Command::Version);
    }
    let Flags {
        world,
        resume,
        theme,
        probe,
        json,
        gate_tty,
        words,
    } = flags(args)?;
    let first = words.first().map(String::as_str);
    if first != Some("run") {
        if json {
            return Err("--json goes with run".into());
        }
        if gate_tty {
            return Err("--gate-tty goes with run".into());
        }
    }
    if probe {
        return if words.is_empty() {
            Ok(Command::Probe)
        } else {
            Err("--probe takes no command".into())
        };
    }
    let chat = |resume| Command::Chat {
        world: world.clone(),
        resume,
        theme: theme.clone(),
    };
    match first {
        None => Ok(chat(resume)),
        Some("chat") if words.len() == 1 => Ok(chat(resume)),
        Some("chat") => Err("chat takes no words; see mnemorph --help".into()),
        Some("resume") => match (&words[1..], &resume) {
            ([run], None) => Ok(chat(Some(run.clone()))),
            _ => Err("resume takes one run: its id, or last; see mnemorphd runs".into()),
        },
        Some("version") if words.len() == 1 => Ok(Command::Version),
        Some("help") if words.len() == 1 => Ok(Command::Help),
        Some("status") if words.len() == 1 && resume.is_none() => Ok(Command::Status {
            world: world.clone(),
        }),
        Some("status") => Err("status takes no words; see mnemorph --help".into()),
        Some("run") if resume.is_some() => {
            Err("--resume opens the app: use mnemorph resume <run>".into())
        }
        Some("run") => Ok(Command::Run {
            // Arguments win: stdin is read only for `run` with no ask, or `run -`.
            ask: match &words[1..] {
                [] => None,
                [dash] if dash == "-" => None,
                rest => Some(rest.join(" ")),
            },
            world: world.clone(),
            json,
            gate_tty,
        }),
        Some(other) => Err(format!(
            "unknown command `{other}`; see mnemorph --help (the world's admin commands are \
mnemorphd's)"
        )),
    }
}
