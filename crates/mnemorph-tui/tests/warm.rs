//! The races' portraits decode off the UI's thread: drawing never decodes one. Until the start-up
//! warm-up has reached a race, its portrait is simply not drawn; once it has, frames only read
//! the cache. Its own test binary, so no other test has warmed the cache first.

use mnemorph_server::protocol::Event;
use mnemorph_tui::app::{Header, Model, Msg, update, view};
use mnemorph_tui::kit::mnanim;
use mnemorph_tui::theme::Variant;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::time::Duration;

#[test]
fn frames_never_decode_a_portrait() {
    let mut m = Model::new(
        Header {
            body: "rimuru".into(),
            model: "m".into(),
            world: "~/w".into(),
        },
        Variant::Dark,
        (120, 40),
    );
    update(&mut m, Msg::Resize(120, 40));
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    // Idle (Veldora at the sidebar's head), then working (the body's), a child too.
    term.draw(|f| view(&m, f)).unwrap();
    m.outstanding = 1;
    update(
        &mut m,
        Msg::Below {
            event: Event::TurnStarted { text: "x".into() },
            chain: vec!["orc-a".into()],
        },
    );
    for _ in 0..10 {
        update(&mut m, Msg::Tick(Duration::from_millis(33)));
        term.draw(|f| view(&m, f)).unwrap();
    }
    assert_eq!(mnanim::decodes(), 0, "the UI decoded on its own thread");
    assert!(mnanim::ready(mnemorph_races::Race::Orc).is_none());
    // The warm-up, on a thread of its own as the runtime runs it.
    std::thread::spawn(mnanim::warm).join().unwrap();
    let after = mnanim::decodes();
    assert_eq!(after, mnemorph_races::Race::ALL.len());
    for _ in 0..10 {
        update(&mut m, Msg::Tick(Duration::from_millis(33)));
        term.draw(|f| view(&m, f)).unwrap();
    }
    assert!(mnanim::ready(mnemorph_races::Race::Orc).is_some());
    assert_eq!(
        mnanim::decodes(),
        after,
        "frames read the cache, never decode"
    );
}
