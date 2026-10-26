//! A scripted session hosted in-process, as the binary hosts a real one: the model edits a file
//! and answers, the observer's events and the stream reach the model as messages, and the frame
//! shows the edit's card, its diff and the answer.

use std::fs;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use mnemorph_loop::open::{Bound, open_with};
use mnemorph_loop::{Config, Paths, ScriptedModel};
use mnemorph_tui::app::{Cmd, Header, Model, Msg, update, view};
use mnemorph_tui::run::{Host, channel, gate, observer};
use mnemorph_tui::theme::Variant;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use serde_json::json;

#[test]
fn a_scripted_turn_reaches_the_screen() {
    let world = std::env::temp_dir().join(format!("mnemorph-tui-session-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    fs::create_dir_all(world.join(".mnemorph")).unwrap();
    fs::write(world.join("parser.py"), "def f(this):\n    return this\n").unwrap();
    let model = std::sync::Arc::new(ScriptedModel::new([
        ScriptedModel::tool(
            "t1",
            "edit",
            json!({"path": "parser.py", "old_string": "    return this", "new_string": "    return collate(this)"}),
        ),
        ScriptedModel::text("Fixed: the collation is **kept**."),
    ]));
    let config = Config::load(&Paths::with_global(world.join("no-global.toml"), &world)).unwrap();
    let (tx, rx) = channel();
    let opened = open_with(
        &world,
        &config,
        Bound {
            model,
            model_id: "scripted",
            attic: None,
        },
        gate(tx.clone()),
        observer(tx.clone(), world.clone()),
        |_| None,
    )
    .unwrap();
    let host = Host::start(opened, tx).unwrap();
    let mut m = Model::new(
        Header {
            body: "rimuru".into(),
            model: "scripted".into(),
            world: "~/world".into(),
        },
        Variant::Dark,
        (90, 30),
    );
    m.input.insert("fix the collate bug");
    for cmd in update(
        &mut m,
        Msg::Key(ratatui::crossterm::event::KeyEvent::from(
            ratatui::crossterm::event::KeyCode::Enter,
        )),
    ) {
        assert!(matches!(cmd, Cmd::Ask(_)));
        host.run(cmd);
    }
    loop {
        match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(msg) => {
                let done = matches!(msg, Msg::Done { .. });
                for cmd in update(&mut m, msg) {
                    host.run(cmd);
                }
                if done {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                panic!("the turn never finished")
            }
        }
    }
    host.stop();
    assert!(!m.busy());
    let mut term = Terminal::new(TestBackend::new(90, 30)).unwrap();
    term.draw(|f| view(&m, f)).unwrap();
    let buf = term.backend().buffer();
    let screen: String = (0..30)
        .map(|y| {
            (0..90)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
                + "\n"
        })
        .collect();
    assert!(screen.contains("▸ you  fix the collate bug"), "{screen}");
    assert!(screen.contains("┌─ edit parser.py"), "{screen}");
    assert!(
        screen.contains("-     return this") && screen.contains("+     return collate(this)"),
        "{screen}"
    );
    assert!(screen.contains("Fixed: the collation is kept."), "{screen}");
    assert_eq!(
        fs::read_to_string(world.join("parser.py")).unwrap(),
        "def f(this):\n    return collate(this)\n"
    );
    let _ = fs::remove_dir_all(&world);
}
