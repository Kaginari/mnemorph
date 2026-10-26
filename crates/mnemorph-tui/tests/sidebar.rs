//! The sidebar and the mouse, drawn into Ratatui's test backend: each section with what the
//! observer and the world gave, the toggle and the narrow fold, the wheel on its spring, and a
//! card expanded and folded by a click or a key.

use std::collections::BTreeMap;
use std::time::Duration;

use mnemorph_loop::world::memory::Standing;
use mnemorph_races::Race;
use mnemorph_server::protocol::Event;
use mnemorph_tui::app::{Cmd, Header, Model, Msg, WHEEL, update, view};
use mnemorph_tui::sidebar::Survey;
use mnemorph_tui::theme::{RACES, Variant};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use serde_json::json;

fn model() -> Model {
    Model::new(
        Header {
            body: "rimuru".into(),
            model: "deepseek-v4-flash".into(),
            world: "~/sqlglot".into(),
        },
        Variant::Dark,
        (120, 40),
    )
}

fn ev(m: &mut Model, event: Event) -> Vec<Cmd> {
    update(m, Msg::Event { event, below: None })
}

fn below(m: &mut Model, chain: &[&str], event: Event) {
    update(
        m,
        Msg::Below {
            event,
            chain: chain.iter().map(|s| (*s).to_owned()).collect(),
        },
    );
}

fn ctrl(m: &mut Model, c: char) {
    update(
        m,
        Msg::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
    );
}

fn draw(m: &mut Model, w: u16, h: u16) -> Buffer {
    update(m, Msg::Resize(w, h));
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| view(m, f)).unwrap();
    t.backend().buffer().clone()
}

fn rows(b: &Buffer) -> Vec<String> {
    (0..b.area.height)
        .map(|y| {
            (0..b.area.width)
                .map(|x| b[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect()
}

fn screen(b: &Buffer) -> String {
    rows(b).join("\n")
}

/// Where `needle` starts on the screen, as a cell.
fn find(b: &Buffer, needle: &str) -> Option<(u16, u16)> {
    let r = rows(b);
    for (y, row) in r.iter().enumerate() {
        if let Some(byte) = row.find(needle) {
            let x = row[..byte].chars().count();
            return Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?));
        }
    }
    None
}

fn call(m: &mut Model, id: &str, name: &str, input: serde_json::Value, out: &str) {
    ev(
        m,
        Event::ToolCall {
            id: id.into(),
            name: name.into(),
            input,
        },
    );
    ev(
        m,
        Event::ToolResult {
            id: id.into(),
            content: out.into(),
            is_error: false,
            cut: 0,
        },
    );
}

fn click(m: &mut Model, (x, y): (u16, u16)) {
    update(
        m,
        Msg::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }),
    );
}

fn wheel(m: &mut Model, kind: MouseEventKind) {
    update(
        m,
        Msg::Mouse(MouseEvent {
            kind,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        }),
    );
}

/// What recall, the world and the spend gave: a claim with hidden characters in it.
fn world(m: &mut Model) {
    update(m, Msg::Recalled(3));
    update(
        m,
        Msg::Surveyed {
            seq: 0,
            survey: Survey {
                memory: Standing {
                    shared: 2,
                    proposed: 1,
                    grounded: 5,
                    latest: Some("collations drop in \u{202e}_parse_types\u{1b}[2J".into()),
                },
                races: BTreeMap::from([
                    ("orc-sql".to_owned(), Race::Orc),
                    ("slime-parser".to_owned(), Race::Slime),
                ]),
            },
        },
    );
    ev(
        m,
        Event::Usage {
            input: 20_000,
            output: 900,
            cache_read: 40_000,
            cache_write: 0,
            usd: Some(0.04),
            wt: 1,
            spent_wt: 1,
            occupancy: 61_500,
            stress_line: 150_000,
        },
    );
}

/// A session with something in every section, hostile text included.
fn busy_session(m: &mut Model) {
    m.outstanding = 1;
    ev(
        m,
        Event::TurnStarted {
            text: "fix the collate bug".into(),
        },
    );
    world(m);
    call(
        m,
        "e1",
        "edit",
        json!({"path": "sqlglot/parser.py", "old_string": "a", "new_string": "b"}),
        "ok",
    );
    update(
        m,
        Msg::Prior {
            id: "w1".into(),
            existed: false,
        },
    );
    call(
        m,
        "w1",
        "write",
        json!({"path": "tests/test_collate.py", "content": "x\n"}),
        "wrote",
    );
    call(
        m,
        "d1",
        "delete",
        json!({"path": "old\u{202e}.py"}),
        "deleted",
    );
    // A failed act changes nothing.
    ev(
        m,
        Event::ToolCall {
            id: "e2".into(),
            name: "edit".into(),
            input: json!({"path": "never.py", "old_string": "a", "new_string": "b"}),
        },
    );
    ev(
        m,
        Event::ToolResult {
            id: "e2".into(),
            content: "no match".into(),
            is_error: true,
            cut: 0,
        },
    );
    below(
        m,
        &["orc-sql"],
        Event::TurnStarted {
            text: "judge".into(),
        },
    );
    below(
        m,
        &["orc-sql", "slime-parser"],
        Event::ToolCall {
            id: "s1".into(),
            name: "edit".into(),
            input: json!({}),
        },
    );
    below(
        m,
        &["evil\u{1b}]0;x\u{7}"],
        Event::TurnEnded {
            ending: "failed: boom".into(),
        },
    );
}

#[test]
fn every_section_shows_what_the_observer_and_the_world_gave() {
    let mut m = model();
    busy_session(&mut m);
    // Tall enough for the portrait at the head and every section whole under it.
    let buf = draw(&mut m, 120, 52);
    let scr = screen(&buf);
    println!("{scr}");
    for want in [
        "vitals ─",
        "ᴄᴏꜱᴛ $0.04",
        "ᴛᴏᴋ  60.9k",
        "41%",
        "deepseek-v4-flash",
        "memory ─",
        "recalled 3 this ask",
        "grounded 5 · shared 2",
        "files · 3 ─",
        "sqlglot/",
        "└ M parser.py",
        "└ A test_collate.py",
        r"D old\u{202e}.py",
        "bodies · 3 · 2 live",
        "◆ rimuru",
        "● orc-sql",
        "● slime-parser",
    ] {
        assert!(scr.contains(want), "{want} missing:\n{scr}");
    }
    assert!(!scr.contains("M never.py"), "a failed act counted");
    // The claim, the path and the body name are escaped, never drawn raw.
    for raw in ['\u{202e}', '\u{1b}', '\u{7}'] {
        assert!(!scr.contains(raw), "{raw:?} reached the screen");
    }
    assert!(scr.contains(r"\u{202e}_parse_types"), "{scr}");
    assert!(scr.contains(r"evil\u{1b}]0;x\u{7}") || scr.contains(r"evil\u{1b}]0;x\u{…"));
    // The hierarchy: the slime under the orc that commissioned it, working, in its race colour.
    let (ox, oy) = find(&buf, "● orc-sql").unwrap();
    let (sx, sy) = find(&buf, "● slime-parser").unwrap();
    assert_eq!(sy, oy + 1);
    assert_eq!(sx, ox + 2, "indented under its commissioner");
    assert_eq!(buf[(sx, sy)].fg, RACES.slime.color());
    assert_eq!(buf[(ox, oy)].fg, RACES.orc.color());
    let slime_row = &rows(&buf)[usize::from(sy)];
    assert!(slime_row.contains("edit"), "{slime_row}");
    // The gauge is drawn against the stress line.
    let ctx_row = rows(&buf)
        .into_iter()
        .find(|r| r.contains("ᴄᴛx  ━"))
        .unwrap();
    assert!(
        ctx_row.contains('─') && ctx_row.contains("41%"),
        "{ctx_row}"
    );
}

#[test]
fn ctrl_o_toggles_the_sidebar_and_a_narrow_terminal_folds_it() {
    let mut m = model();
    busy_session(&mut m);
    assert!(screen(&draw(&mut m, 120, 40)).contains("memory ─"));
    ctrl(&mut m, 'o');
    let scr = screen(&draw(&mut m, 120, 40));
    assert!(!scr.contains("memory ─"), "{scr}");
    assert_eq!(m.main_area().0, 116, "the transcript takes the width");
    ctrl(&mut m, 'o');
    assert!(screen(&draw(&mut m, 120, 40)).contains("memory ─"));
    assert!(screen(&draw(&mut m, 100, 40)).contains("memory ─"));
    let narrow = screen(&draw(&mut m, 99, 40));
    assert!(!narrow.contains("memory ─"), "{narrow}");
    assert_eq!(m.main_area().0, 95);
    // Nothing overflows at any width either way.
    for w in [100, 110, 140] {
        for r in rows(&draw(&mut m, w, 24)) {
            assert_eq!(r.chars().count(), usize::from(w));
        }
    }
}

fn long_transcript(m: &mut Model) {
    for i in 0..60 {
        ev(
            m,
            Event::TurnStarted {
                text: format!("ask {i}"),
            },
        );
        ev(
            m,
            Event::TurnEnded {
                ending: "answered".into(),
            },
        );
    }
}

#[test]
fn the_wheel_scrolls_on_the_spring() {
    let mut m = model();
    long_transcript(&mut m);
    draw(&mut m, 120, 40);
    for _ in 0..5 {
        wheel(&mut m, MouseEventKind::ScrollUp);
    }
    assert_eq!(m.viewport.offset(), 0, "the spring has not moved yet");
    assert!(m.next_tick().is_some(), "the loop ticks while it moves");
    update(&mut m, Msg::Tick(Duration::from_millis(16)));
    let first = m.viewport.offset();
    assert!(
        first > 0 && first < 5 * WHEEL,
        "a step, not a jump: {first}"
    );
    for _ in 0..60 {
        update(&mut m, Msg::Tick(Duration::from_millis(16)));
    }
    assert_eq!(m.viewport.offset(), 5 * WHEEL);
    assert!(m.next_tick().is_none(), "at rest, the loop sleeps");
    let scr = screen(&draw(&mut m, 120, 40));
    assert!(scr.contains("esc to the end"), "{scr}");
    for _ in 0..5 {
        wheel(&mut m, MouseEventKind::ScrollDown);
    }
    for _ in 0..60 {
        update(&mut m, Msg::Tick(Duration::from_millis(16)));
    }
    assert_eq!(m.viewport.offset(), 0);
    assert!(m.viewport.following());
}

fn long_output(m: &mut Model) {
    ev(
        m,
        Event::TurnStarted {
            text: "run the tests".into(),
        },
    );
    let out = (1..=10)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    call(m, "b1", "bash", json!({"command": "pytest"}), &out);
    call(m, "b2", "bash", json!({"command": "ls"}), "a\nb");
}

#[test]
fn a_click_expands_a_folded_card_and_folds_it_again() {
    let mut m = model();
    long_output(&mut m);
    let buf = draw(&mut m, 120, 40);
    let scr = screen(&buf);
    assert!(
        scr.contains("… +7 lines · click or ctrl+t to expand"),
        "{scr}"
    );
    assert!(!scr.contains("line 10"), "{scr}");
    let at = find(&buf, "line 2").unwrap();
    click(&mut m, at);
    let scr = screen(&draw(&mut m, 120, 40));
    assert!(
        scr.contains("line 10") && !scr.contains("+7 lines"),
        "{scr}"
    );
    let buf = draw(&mut m, 120, 40);
    click(&mut m, find(&buf, "line 7").unwrap());
    let scr = screen(&draw(&mut m, 120, 40));
    assert!(
        scr.contains("+7 lines") && !scr.contains("line 10"),
        "{scr}"
    );
    // A click on a card with nothing folded, or on a row of the sidebar, changes nothing.
    let buf = draw(&mut m, 120, 40);
    click(&mut m, find(&buf, "bash  ls").unwrap());
    click(&mut m, find(&buf, "recalled").unwrap());
    assert_eq!(screen(&draw(&mut m, 120, 40)), screen(&buf));
    // A click on a section's title folds it, and another opens it again.
    click(&mut m, find(&buf, "▾ memory").unwrap());
    let folded = screen(&draw(&mut m, 120, 40));
    assert!(
        folded.contains("▸ memory") && !folded.contains("recalled"),
        "{folded}"
    );
    let buf = draw(&mut m, 120, 40);
    click(&mut m, find(&buf, "▸ memory").unwrap());
    assert!(screen(&draw(&mut m, 120, 40)).contains("recalled"));
}

#[test]
fn a_key_expands_the_focused_card_and_the_focus_moves_between_cards() {
    let mut m = model();
    long_output(&mut m);
    draw(&mut m, 120, 40);
    // With no card focused, ctrl+t opens the newest that has something folded.
    ctrl(&mut m, 't');
    let buf = draw(&mut m, 120, 40);
    assert!(screen(&buf).contains("line 10"));
    // The focused card wears the accent on its border.
    let (x, y) = find(&buf, "bash  pytest").unwrap();
    assert_eq!(buf[(x - 4, y)].symbol(), "▌", "the card's spine");
    assert_eq!(buf[(x - 4, y)].fg, m.theme.p.holo.color());
    ctrl(&mut m, 't');
    assert!(!screen(&draw(&mut m, 120, 40)).contains("line 10"));
    // alt+↓ past the newest card leaves the cards; alt+↑ comes back to the newest.
    let alt = |code| Msg::Key(KeyEvent::new(code, KeyModifiers::ALT));
    update(&mut m, alt(KeyCode::Down));
    update(&mut m, alt(KeyCode::Down));
    assert_eq!(m.transcript.focused(), None);
    update(&mut m, alt(KeyCode::Up));
    let newest = m.transcript.focused().unwrap();
    update(&mut m, alt(KeyCode::Up));
    assert!(m.transcript.focused().unwrap() < newest);
    // Esc lets the focus go.
    update(&mut m, Msg::Key(KeyEvent::from(KeyCode::Esc)));
    assert_eq!(m.transcript.focused(), None);
}

#[test]
fn the_focus_scrolls_an_old_card_into_view() {
    let mut m = model();
    long_output(&mut m);
    long_transcript(&mut m);
    let scr = screen(&draw(&mut m, 120, 40));
    assert!(!scr.contains("bash  pytest"), "{scr}");
    let alt = |code| Msg::Key(KeyEvent::new(code, KeyModifiers::ALT));
    update(&mut m, alt(KeyCode::Up));
    update(&mut m, alt(KeyCode::Up));
    for _ in 0..60 {
        update(&mut m, Msg::Tick(Duration::from_millis(16)));
    }
    let scr = screen(&draw(&mut m, 120, 40));
    assert!(scr.contains("bash  pytest"), "{scr}");
}

#[test]
fn a_finished_line_reads_the_world_again() {
    let mut m = model();
    m.outstanding = 1;
    let cmds = update(&mut m, Msg::Done { held: 0 });
    assert!(matches!(cmds.as_slice(), [Cmd::Survey(0)]), "{cmds:?}");
}

#[test]
fn a_survey_that_comes_back_late_never_overwrites_a_newer_one() {
    let mut m = model();
    let reading = |shared| Survey {
        memory: Standing {
            shared,
            ..Standing::default()
        },
        races: BTreeMap::new(),
    };
    let (Cmd::Survey(old), Cmd::Survey(new)) = (m.survey(), m.survey()) else {
        panic!("not surveys")
    };
    assert!(new > old);
    // The newer reading lands first; the older one, arriving after it, is dropped.
    update(
        &mut m,
        Msg::Surveyed {
            seq: new,
            survey: reading(9),
        },
    );
    update(
        &mut m,
        Msg::Surveyed {
            seq: old,
            survey: reading(1),
        },
    );
    assert_eq!(m.sidebar.memory.as_ref().map(|s| s.shared), Some(9));
    let Cmd::Survey(newest) = m.survey() else {
        panic!("not a survey")
    };
    update(
        &mut m,
        Msg::Surveyed {
            seq: newest,
            survey: reading(12),
        },
    );
    assert_eq!(m.sidebar.memory.as_ref().map(|s| s.shared), Some(12));
}

/// The frame the pull request shows: the sidebar beside a turn, as a human sees it.
#[test]
fn a_frame_for_the_eye_with_the_sidebar() {
    let mut m = model();
    m.outstanding = 1;
    ev(
        &mut m,
        Event::TurnStarted {
            text: "fix the collate bug in nested types".into(),
        },
    );
    world(&mut m);
    ev(
        &mut m,
        Event::TextDelta {
            text: "The collation is dropped in `_parse_types`:\n\n".into(),
        },
    );
    call(
        &mut m,
        "e1",
        "edit",
        json!({"path": "sqlglot/parser.py", "old_string": "return this", "new_string": "return self._parse_collate(this)"}),
        "ok",
    );
    below(
        &mut m,
        &["orc-sql"],
        Event::TurnStarted {
            text: "judge".into(),
        },
    );
    below(
        &mut m,
        &["orc-sql", "slime-parser"],
        Event::ToolCall {
            id: "s1".into(),
            name: "bash".into(),
            input: json!({"command": "pytest tests/test_parser.py"}),
        },
    );
    let scr = screen(&draw(&mut m, 108, 22));
    println!("{scr}");
    assert!(scr.contains("bodies · 2 · 2 live"), "{scr}");
}

#[test]
fn many_bodies_walk_as_a_tree_and_stay_cheap_to_draw() {
    let mut m = model();
    m.outstanding = 1;
    // Thirty orcs, each with a hundred slimes: three thousand and thirty bodies.
    for o in 0..30 {
        for s in 0..100 {
            below(
                &mut m,
                &[&format!("orc-{o}"), &format!("slime-{o}-{s}")],
                Event::TurnStarted {
                    text: "work".into(),
                },
            );
        }
        below(
            &mut m,
            &[&format!("orc-{o}")],
            Event::TurnEnded {
                ending: "answered".into(),
            },
        );
    }
    let bodies = &m.sidebar.bodies;
    assert_eq!(bodies.list().len(), 3_030);
    assert_eq!(
        bodies.live(),
        3_000,
        "every slime works, every orc answered"
    );
    let walked: Vec<(usize, String)> = bodies.walk().map(|(d, b)| (d, b.name.clone())).collect();
    assert_eq!(walked.len(), 3_030);
    assert_eq!(walked[0], (1, "orc-0".to_owned()));
    assert_eq!(walked[1], (2, "slime-0-0".to_owned()));
    assert_eq!(walked[100], (2, "slime-0-99".to_owned()));
    assert_eq!(walked[101], (1, "orc-1".to_owned()));
    // Tall enough for the portrait at the head and the bodies under it.
    let mut t = Terminal::new(TestBackend::new(120, 52)).unwrap();
    t.draw(|f| view(&m, f)).unwrap();
    let start = std::time::Instant::now();
    for _ in 0..20 {
        t.draw(|f| view(&m, f)).unwrap();
    }
    let frame = start.elapsed() / 20;
    assert!(frame < Duration::from_millis(33), "a frame took {frame:?}");
    let scr = screen(t.backend().buffer());
    assert!(scr.contains("bodies · 3030 · 3000 live"), "{scr}");
    assert!(scr.contains("● orc-0") && scr.contains("more"), "{scr}");
}

fn write_call(m: &mut Model, id: &str, path: &str) {
    ev(
        m,
        Event::ToolCall {
            id: id.into(),
            name: "write".into(),
            input: json!({"path": path, "content": "x\n"}),
        },
    );
}

fn landed(m: &mut Model, id: &str) {
    ev(
        m,
        Event::ToolResult {
            id: id.into(),
            content: "wrote".into(),
            is_error: false,
            cut: 0,
        },
    );
}

fn prior(m: &mut Model, id: &str, existed: bool) {
    update(
        m,
        Msg::Prior {
            id: id.into(),
            existed,
        },
    );
}

fn mark_of(m: &Model, path: &str) -> Option<&'static str> {
    m.sidebar
        .files
        .list()
        .iter()
        .find(|(p, _)| p == path)
        .map(|(_, mark)| mark.letter())
}

#[test]
fn a_write_path_is_looked_at_off_the_session_and_its_answer_may_come_late() {
    let mut m = model();
    // The observer names the path; the UI asks for a look, never the session's task.
    let cmds = update(
        &mut m,
        Msg::Writes {
            id: "w1".into(),
            path: "new.py".into(),
        },
    );
    assert!(
        matches!(cmds.as_slice(), [Cmd::Look { id, path }] if id == "w1" && path == "new.py"),
        "{cmds:?}"
    );
    // The look comes back between the call and its result.
    write_call(&mut m, "w1", "new.py");
    prior(&mut m, "w1", false);
    landed(&mut m, "w1");
    assert_eq!(mark_of(&m, "new.py"), Some("A"));
    // After the result: "not there" still proves the write created it.
    write_call(&mut m, "w2", "late.py");
    landed(&mut m, "w2");
    assert_eq!(mark_of(&m, "late.py"), Some("M"), "unknown until the look");
    prior(&mut m, "w2", false);
    assert_eq!(mark_of(&m, "late.py"), Some("A"));
    // "There" proves nothing — it may have raced the write: the path stays modified.
    write_call(&mut m, "w3", "old.py");
    landed(&mut m, "w3");
    prior(&mut m, "w3", true);
    assert_eq!(mark_of(&m, "old.py"), Some("M"));
    // A path the session deleted, then wrote again, was there before: modified, whatever the
    // look saw in between.
    call(
        &mut m,
        "d1",
        "delete",
        json!({"path": "gone.py"}),
        "deleted",
    );
    write_call(&mut m, "w4", "gone.py");
    landed(&mut m, "w4");
    prior(&mut m, "w4", false);
    assert_eq!(mark_of(&m, "gone.py"), Some("M"));
    // A failed write counts nothing, and its late look changes nothing.
    write_call(&mut m, "w5", "never.py");
    ev(
        &mut m,
        Event::ToolResult {
            id: "w5".into(),
            content: "refused".into(),
            is_error: true,
            cut: 0,
        },
    );
    prior(&mut m, "w5", false);
    assert_eq!(mark_of(&m, "never.py"), None);
}
