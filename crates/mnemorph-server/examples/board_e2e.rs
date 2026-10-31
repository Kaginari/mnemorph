//! The board end to end on its real server and bridge, for the board's Playwright suite: a board
//! over two fresh worlds, whose chats' children are this program run again as a real core — the
//! real loop, tools and gate — answering from a scripted model instead of a provider.
//!
//! `board_e2e <root>` makes `<root>/world` and `<root>/atelier` (each a `notes.txt` holding `a`;
//! the atelier also a creature, a note in its memory, a fact and a journal entry), the human's
//! global file (running unguarded: the test machine has no sandbox to rely on; an in-place edit
//! asks the human) and a registry naming both worlds, then serves the board on a free loopback
//! port. Its first line on stdout is the URL with the one-time key; it serves until killed. The
//! suite needs the board built first (`pnpm --filter @mnemorph/board build`), as `mnemorphd` does.
//!
//! Each chat's child is `board_e2e child <world> <root>`: the protocol on stdio over a script. In
//! `world`, turn one reads `notes.txt`, edits it and answers in Markdown; turn two answers in one
//! line. In `atelier`, the turn runs `sed -i` on `notes.txt` — which waits at the human gate —
//! then answers. A resume is refused: the script holds no past.
//!
//! While it serves, `board_e2e note <world> <owner> <text>` lands a note in a creature's memory
//! and `board_e2e fact <world> <about> <text>` a fact in the ontology, as a body would: the board
//! shows each live.

use std::ffi::OsString;
use std::io::{BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mnemorph_loop::model::Reply;
use mnemorph_loop::world::Facts;
use mnemorph_loop::world::facts::{Event, fact_id};
use mnemorph_loop::{Bound, Config, Paths, ScriptedModel, open_with};
use mnemorph_server::board::bridge::{Child, Limits};
use mnemorph_server::board::{Board, Lifetimes, listen};
use mnemorph_server::world::dimension::{hand_file, register_at};
use serde_json::json;

/// The human's global file: no sandbox needed, no provider named; an in-place edit asks.
const GLOBAL: &str =
    "[sandbox]\nallow_unguarded = true\n\n[gate]\nask_rules = [\"in-place-edit\"]\n";

/// The atelier's creature: an Orc under Rimuru.
const ORC: &str = "# orc-notes\n\n- **Race:** orc\n- **Reports to:** rimuru\n- **Domain:** docs/\n";

/// The atelier's journal.
const LOG: &str = "# Log\n\n### [2026-10-30T09:00:00+01:00] rimuru — the atelier opens\n\
- **Task:** keep the notes.\n- **Result:** done.\n";

/// What the scripted model answers in the atelier: an act the gate puts to the human, then words.
fn gated() -> Vec<Reply> {
    vec![
        ScriptedModel::tool("t1", "bash", json!({"command": "sed -i s/a/b/ notes.txt"})),
        ScriptedModel::text("The notes say b now: the human approved the edit."),
    ]
}

/// What the scripted model answers in `world`, call by call.
fn script() -> Vec<Reply> {
    vec![
        ScriptedModel::tool("t1", "read", json!({"path": "notes.txt"})),
        ScriptedModel::tool(
            "t2",
            "edit",
            json!({"path": "notes.txt", "old_string": "a\n", "new_string": "b\n"}),
        ),
        ScriptedModel::text(
            "The notes said **a**; they say `b` now.\n\n```rust\nfn main() {}\n```\n\n\
             See [the guide](https://example.com/guide) — not [this](javascript:alert(1)) \
             <img src=x onerror=alert(1)>.",
        ),
        ScriptedModel::text("The second turn, answered."),
    ]
}

fn child(world: &Path, root: &Path) -> Result<(), String> {
    let world = world.to_path_buf();
    let global = root.join("global.toml");
    let attic = root.join("attic");
    let gate = world.file_name().is_some_and(|n| n == "atelier");
    let mut script = Some(if gate { gated() } else { script() });
    let opener: mnemorph_server::Opener = Box::new(move |gate, observer| {
        let config = Config::load(&Paths::with_global(global.clone(), &world))?;
        let mut opened = open_with(
            &world,
            &config,
            Bound {
                model: Arc::new(ScriptedModel::new(script.take().unwrap_or_default())),
                model_id: "scripted-model",
                attic: Some(&attic),
            },
            gate,
            observer,
            |_| None,
        )?;
        opened.model = "scripted-model".into();
        Ok(opened)
    });
    mnemorph_server::serve(
        BufReader::new(std::io::stdin()),
        std::io::stdout(),
        "e2e",
        opener,
    )
}

/// A note `owner` proposes in `world`'s memory, as a body's turn lands one.
fn note(world: &Path, owner: &str, text: &str) -> Result<(), String> {
    mnemorph_loop::world::memory::propose(world, owner, text, Vec::new(), None)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A fact about `about` that `orc-notes` asserts in `world`'s ontology.
fn fact(world: &Path, about: &str, text: &str) -> Result<(), String> {
    let event = Event::Support {
        id: fact_id(about, text),
        about: about.to_owned(),
        text: text.to_owned(),
        by: "orc-notes".to_owned(),
        source: None,
    };
    Facts::record(world, &event).map_err(|e| e.to_string())
}

fn board(root: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| e.to_string();
    std::fs::write(root.join("global.toml"), GLOBAL).map_err(io)?;
    let root = root.canonicalize().map_err(io)?;
    let registry = root.join("state/mnemorph/worlds.jsonl");
    for name in ["world", "atelier"] {
        let world = root.join(name);
        std::fs::create_dir_all(world.join(".mnemorph")).map_err(io)?;
        std::fs::write(world.join("notes.txt"), "a\n").map_err(io)?;
        if name == "atelier" {
            std::fs::create_dir_all(world.join(".mnemorph/orc/orc-notes")).map_err(io)?;
            std::fs::write(world.join(".mnemorph/orc/orc-notes/README.md"), ORC).map_err(io)?;
            std::fs::write(world.join(".mnemorph/log.md"), LOG).map_err(io)?;
            note(&world, "orc-notes", "the notes file holds one line")?;
            fact(&world, "notes.txt", "the notes are plain text")?;
        }
        register_at(&registry, &world.canonicalize().map_err(io)?, None).map_err(io)?;
    }
    let program = std::env::current_exe().map_err(io)?;
    let home = root.clone();
    let child = Child {
        program,
        args: Arc::new(move |world, resume| {
            let mut a: Vec<OsString> = vec!["child".into(), world.into(), home.clone().into()];
            if let Some(run) = resume {
                a.extend(["--resume".into(), run.into()]);
            }
            a
        }),
        env: vec![
            ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
            ("HOME".into(), root.display().to_string()),
            (
                "XDG_STATE_HOME".into(),
                root.join("state").display().to_string(),
            ),
            (
                "XDG_CONFIG_HOME".into(),
                root.join("config").display().to_string(),
            ),
            ("LANG".into(), "C".into()),
        ],
        global: root.join("global.toml"),
        answers: {
            let state = root.join("state");
            Arc::new(move |w| Some(mnemorph_server::world::answers::answers_in(&state, w)))
        },
    };
    let listeners = listen(0, Some([127, 0, 0, 1].into()))?;
    let addrs: Vec<_> = listeners
        .iter()
        .map(std::net::TcpListener::local_addr)
        .collect::<Result<_, _>>()
        .map_err(io)?;
    let board = Board::bridged(
        &addrs,
        (registry.clone(), hand_file(&registry)),
        Box::new(|_| {}),
        Lifetimes::default(),
        child,
        Limits::default(),
    )?;
    let mut out = std::io::stdout();
    writeln!(out, "{}", board.url()).map_err(io)?;
    out.flush().map_err(io)?;
    mnemorph_server::board::run(listeners, &board)
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.as_slice() {
        [mode, world, root] if mode == "child" => child(Path::new(world), Path::new(root)),
        [mode, ..] if mode == "child" => Err("the e2e child resumes nothing".into()),
        [mode, world, owner, text] if mode == "note" => note(Path::new(world), owner, text),
        [mode, world, about, text] if mode == "fact" => fact(Path::new(world), about, text),
        [root] => board(&PathBuf::from(root)),
        _ => Err("usage: board_e2e <root>".into()),
    };
    match done {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("board_e2e: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
