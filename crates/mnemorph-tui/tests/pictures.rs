//! The portraits as real pixels, end to end through the model and the view: where a frame wants
//! them, never under an approval, the welcome's too, nothing re-sent for an unchanged frame,
//! and half blocks where the terminal draws no images.

use mnemorph_races::Race;
use mnemorph_server::protocol::Event;
use mnemorph_tools::Pending;
use mnemorph_tui::app::{Header, Model, Msg, update, view};
use mnemorph_tui::gate::Card;
use mnemorph_tui::kit::mnanim;
use mnemorph_tui::kit::pixels::{self, Protocol};
use mnemorph_tui::pictures::{Art, Pictures, Placer, Slot, Want};
use mnemorph_tui::theme::Variant;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

/// VS Code's terminal: cells of 8 × 17 pixels.
const CELL: (u16, u16) = (8, 17);

fn model(protocol: Protocol) -> Model {
    mnanim::warm();
    let mut m = Model::new(
        Header {
            body: "rimuru".into(),
            model: "deepseek-v4-flash".into(),
            world: "~/sqlglot".into(),
        },
        Variant::Dark,
        (120, 50),
    );
    m.pictures = Pictures::new(protocol, Some(CELL));
    m
}

fn draw(m: &mut Model, t: &mut Terminal<TestBackend>) -> Buffer {
    let size = t.backend().buffer().area;
    update(m, Msg::Resize(size.width, size.height));
    t.draw(|f| view(m, f)).unwrap();
    t.backend().buffer().clone()
}

fn card() -> Card {
    Card::of(
        &mnemorph_loop::GateAsk {
            body: "rimuru".into(),
            race: Race::Rimuru,
            class: mnemorph_loop::ToolClass::Outward,
            act: "git push".into(),
            why: vec!["reaches beyond the world".into()],
            outside: vec![],
        },
        Pending::new(),
    )
}

fn slot(wants: &[Want], s: Slot) -> Option<Want> {
    wants.iter().find(|w| w.slot == s).copied()
}

#[test]
fn the_sidebar_and_the_welcome_want_their_pictures_over_plain_cells() {
    let mut m = model(Protocol::Iterm);
    let mut t = Terminal::new(TestBackend::new(120, 50)).unwrap();
    let buf = draw(&mut m, &mut t);
    let wants = m.pictures.wanted();
    let side = slot(&wants, Slot::Sidebar).expect("the sidebar's head");
    // 32 columns of 8 px: 256 × 255 px in 15 rows.
    assert_eq!((side.area.width, side.area.height), (32, 15));
    assert_eq!(side.art, Art::Still(Race::Rimuru));
    assert_eq!(side.bg, m.theme.p.bg);
    let welcome = slot(&wants, Slot::Welcome).expect("the welcome's portrait");
    assert_eq!((welcome.area.width, welcome.area.height), (32, 15));
    assert_eq!(welcome.bg, mnemorph_tui::welcome::face(&m.theme));
    // Under each, plain cells on the picture's own background: no half block for it to hide.
    for w in [side, welcome] {
        for y in w.area.top()..w.area.bottom() {
            for x in w.area.left()..w.area.right() {
                let c = &buf[(x, y)];
                assert_eq!(c.symbol(), " ", "{:?} at {x},{y}", w.slot);
                assert_eq!(c.bg, w.bg.color(), "{:?} at {x},{y}", w.slot);
            }
        }
    }
    // Still, never played: a busy body asks no portrait ticks.
    m.outstanding = 1;
    assert!(!m.portrait_plays());
}

#[test]
fn no_picture_is_placed_while_an_approval_is_open() {
    let mut m = model(Protocol::Kitty);
    let mut t = Terminal::new(TestBackend::new(120, 50)).unwrap();
    let mut placer = Placer::new(&m.pictures);
    let buf = draw(&mut m, &mut t);
    let mut out = Vec::new();
    placer
        .sync(&m.pictures.wanted(), (120, 50), &buf, &mut out)
        .unwrap();
    assert_eq!(placer.sends, 2, "the sidebar and the welcome");

    m.outstanding = 1;
    update(&mut m, Msg::Gate(card()));
    assert!(m.gate.is_some());
    // Before the frame that opens the card, Kitty's pictures are deleted: never one frame over it.
    let mut before = Vec::new();
    placer.before(m.gate.is_some(), &mut before).unwrap();
    let before = String::from_utf8(before).unwrap();
    assert!(before.contains(&pixels::kitty_delete(Slot::Sidebar.id())));
    assert!(before.contains(&pixels::kitty_delete(Slot::Welcome.id())));
    let buf = draw(&mut m, &mut t);
    assert_eq!(
        m.pictures.wanted(),
        Vec::new(),
        "nothing wanted under the card"
    );
    let mut out = Vec::new();
    placer
        .sync(&m.pictures.wanted(), (120, 50), &buf, &mut out)
        .unwrap();
    assert_eq!(out, b"", "nothing sent while it is open");

    // Denied, the card goes, and the pictures come back.
    update(&mut m, Msg::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(m.gate.is_none());
    let buf = draw(&mut m, &mut t);
    assert!(slot(&m.pictures.wanted(), Slot::Sidebar).is_some());
    // The welcome moved while the turn ran (the activity strip): it is placed once settled.
    let now = std::time::Instant::now();
    placer
        .sync_at(&m.pictures.wanted(), (120, 50), &buf, &mut Vec::new(), now)
        .unwrap();
    assert_eq!(
        placer.sends, 3,
        "the sidebar at once, the moved welcome held back"
    );
    let later = now + mnemorph_tui::pictures::SETTLE;
    placer
        .sync_at(
            &m.pictures.wanted(),
            (120, 50),
            &buf,
            &mut Vec::new(),
            later,
        )
        .unwrap();
    assert_eq!(placer.sends, 4);
    assert_eq!(
        placer.builds, 2,
        "each built once, sent again from the cache"
    );
}

#[test]
fn an_unchanged_frame_sends_nothing_and_a_new_body_swaps_the_picture() {
    let mut m = model(Protocol::Iterm);
    let mut t = Terminal::new(TestBackend::new(120, 50)).unwrap();
    let mut placer = Placer::new(&m.pictures);
    let buf = draw(&mut m, &mut t);
    placer
        .sync(&m.pictures.wanted(), (120, 50), &buf, &mut Vec::new())
        .unwrap();
    let sent = placer.sends;
    for _ in 0..10 {
        let buf = draw(&mut m, &mut t);
        let mut out = Vec::new();
        placer
            .sync(&m.pictures.wanted(), (120, 50), &buf, &mut out)
            .unwrap();
        assert_eq!(out, b"");
    }
    assert_eq!(placer.sends, sent);

    // A commissioned orc works: the sidebar's picture is the orc's, sent once.
    m.outstanding = 1;
    update(
        &mut m,
        Msg::Below {
            event: Event::TurnStarted {
                text: "judge".into(),
            },
            chain: vec!["orc-sql".into()],
        },
    );
    let buf = draw(&mut m, &mut t);
    let side = slot(&m.pictures.wanted(), Slot::Sidebar).unwrap();
    assert_ne!(side.art, Art::Still(Race::Rimuru));
    let mut out = Vec::new();
    placer
        .sync(&m.pictures.wanted(), (120, 50), &buf, &mut out)
        .unwrap();
    let out = String::from_utf8(out).unwrap();
    let at = format!(
        "\x1b7\x1b[{};{}H\x1b]1337;File=",
        side.area.y + 1,
        side.area.x + 1
    );
    assert!(out.contains(&at), "the orc's picture at the sidebar's head");
}

#[test]
fn the_sidebar_folding_takes_its_picture_away() {
    let mut m = model(Protocol::Sixel);
    let mut t = Terminal::new(TestBackend::new(120, 50)).unwrap();
    draw(&mut m, &mut t);
    assert!(slot(&m.pictures.wanted(), Slot::Sidebar).is_some());
    // Under 100 columns the sidebar folds.
    let mut narrow = Terminal::new(TestBackend::new(90, 50)).unwrap();
    draw(&mut m, &mut narrow);
    assert!(slot(&m.pictures.wanted(), Slot::Sidebar).is_none());
    // Too short for the head: no picture either.
    let mut short = Terminal::new(TestBackend::new(120, 30)).unwrap();
    draw(&mut m, &mut short);
    assert!(slot(&m.pictures.wanted(), Slot::Sidebar).is_none());
}

#[test]
fn without_images_the_portraits_are_half_blocks_and_play() {
    let mut m = model(Protocol::Blocks);
    let mut t = Terminal::new(TestBackend::new(120, 50)).unwrap();
    let buf = draw(&mut m, &mut t);
    assert_eq!(m.pictures.wanted(), Vec::new());
    let head: String = (0..buf.area.width)
        .flat_map(|x| (2..18).map(move |y| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_owned())
        .collect();
    assert!(
        head.contains('▀') || head.contains('▄'),
        "half blocks drawn"
    );
    m.outstanding = 1;
    assert!(m.portrait_plays());
}

#[test]
fn a_late_probe_reply_never_answers_a_card_nor_types() {
    let mut m = model(Protocol::Blocks);
    let t0 = std::time::Instant::now();
    m.late = mnemorph_tui::probe::Late::after(t0);
    let soon = t0 + std::time::Duration::from_millis(300);
    // Kitty's reply, late, as the input reads it: Alt+_, `Gi=31;OK`, Alt+\.
    let mut reply = vec![KeyEvent::new(
        KeyCode::Char('_'),
        ratatui::crossterm::event::KeyModifiers::ALT,
    )];
    reply.extend("Gi=31;OK".chars().map(|c| KeyEvent::from(KeyCode::Char(c))));
    reply.push(KeyEvent::new(
        KeyCode::Char('\\'),
        ratatui::crossterm::event::KeyModifiers::ALT,
    ));
    for k in &reply {
        assert!(update(&mut m, Msg::KeyAt(*k, soon)).is_empty());
    }
    assert_eq!(m.input.text(), "", "nothing typed");
    m.outstanding = 1;
    update(&mut m, Msg::Gate(card()));
    for k in [KeyCode::Char('1'), KeyCode::Char('y'), KeyCode::Enter] {
        let cmds = update(&mut m, Msg::KeyAt(KeyEvent::from(k), soon));
        assert!(cmds.is_empty(), "{k:?} reached the card");
    }
    assert!(m.gate.is_some(), "the card still waits for the human");
}

#[test]
fn ctrl_c_and_esc_always_pass_a_late_window() {
    use mnemorph_tui::app::Cmd;
    use ratatui::crossterm::event::KeyModifiers;
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let t0 = std::time::Instant::now();
    let soon = t0 + std::time::Duration::from_millis(300);

    // A card open in the window: Esc denies it.
    let mut m = model(Protocol::Blocks);
    m.late = mnemorph_tui::probe::Late::after(t0);
    m.outstanding = 1;
    update(&mut m, Msg::Gate(card()));
    let cmds = update(&mut m, Msg::KeyAt(KeyEvent::from(KeyCode::Esc), soon));
    assert!(
        cmds.iter().any(|c| matches!(c, Cmd::Answer(..))),
        "Esc denies"
    );
    assert!(m.gate.is_none());

    // Ctrl+C denies a card too.
    update(&mut m, Msg::Gate(card()));
    let cmds = update(&mut m, Msg::KeyAt(ctrl_c, soon));
    assert!(
        cmds.iter().any(|c| matches!(c, Cmd::Answer(..))),
        "Ctrl+C denies"
    );
    assert!(m.gate.is_none());

    // Idle, no card: Ctrl+C quits, at once and in the window.
    let mut idle = model(Protocol::Blocks);
    idle.late = mnemorph_tui::probe::Late::after(t0);
    let mut quit = false;
    for _ in 0..2 {
        quit |= update(&mut idle, Msg::KeyAt(ctrl_c, soon))
            .iter()
            .any(|c| matches!(c, Cmd::Quit));
    }
    assert!(quit, "Ctrl+C quits in the window");

    // A plain `c` or `t`, the tail of a reply, is still dropped.
    let mut m = model(Protocol::Blocks);
    m.late = mnemorph_tui::probe::Late::after(t0);
    for c in ['c', 't'] {
        update(&mut m, Msg::KeyAt(KeyEvent::from(KeyCode::Char(c)), soon));
    }
    assert_eq!(m.input.text(), "");
}
