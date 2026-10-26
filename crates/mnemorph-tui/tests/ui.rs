//! The terminal UI drawn into Ratatui's test backend: what each block shows, in which colours,
//! at which widths, and what the keys do.

use std::sync::Arc;
use std::time::Duration;

use mnemorph_loop::{GateAnswer, GateAsk, ToolClass};
use mnemorph_races::Race;
use mnemorph_server::protocol::Event;
use mnemorph_tools::Pending;
use mnemorph_tui::app::{Cmd, Header, Model, Msg, update, view};
use mnemorph_tui::gate::Card;
use mnemorph_tui::theme::{DARK, LIGHT, Rgb, Variant};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use serde_json::json;

fn model(variant: Variant) -> Model {
    Model::new(
        Header {
            body: "rimuru".into(),
            model: "deepseek-v4-flash".into(),
            world: "~/sqlglot".into(),
        },
        variant,
        (100, 30),
    )
}

fn ev(m: &mut Model, event: Event) -> Vec<Cmd> {
    update(m, Msg::Event { event, below: None })
}

fn key(m: &mut Model, code: KeyCode) -> Vec<Cmd> {
    update(m, Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)))
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
    let first = needle.chars().next()?.to_string();
    for y in 0..b.area.height {
        for x in 0..b.area.width {
            if b[(x, y)].symbol() == first {
                let mut s = String::new();
                let mut cx = x;
                while cx < b.area.width && s.chars().count() < needle.chars().count() {
                    s.push_str(b[(cx, y)].symbol());
                    cx += 1;
                }
                if s == needle {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

fn turn(m: &mut Model, ask: &str) {
    m.outstanding += 1;
    ev(m, Event::TurnStarted { text: ask.into() });
}

fn call(m: &mut Model, id: &str, name: &str, input: serde_json::Value, out: Option<(&str, bool)>) {
    ev(
        m,
        Event::ToolCall {
            id: id.into(),
            name: name.into(),
            input,
        },
    );
    if let Some((content, is_error)) = out {
        ev(
            m,
            Event::ToolResult {
                id: id.into(),
                content: content.into(),
                is_error,
                cut: 0,
            },
        );
    }
}

fn rgb(c: Rgb) -> Color {
    c.color()
}

#[test]
fn markdown_reads_with_its_elements_and_code_in_colour() {
    let mut m = model(Variant::Dark);
    turn(&mut m, "explain");
    let md = "## The fix\n\nThe collation is dropped in `_parse_types`, **always**, see \
[the docs](https://sqlglot.com).\n\n- first\n- second\n\n> a quote\n\n| col | n |\n|---|---|\n| a | 1 |\n\n\
```rust\nfn main() { let x = \"s\"; }\n```";
    ev(
        &mut m,
        Event::Assistant {
            text: md.into(),
            stop: "end_turn".into(),
        },
    );
    let buf = draw(&mut m, 100, 40);
    let scr = screen(&buf);
    for want in [
        "◆ rimuru",
        "The fix",
        "_parse_types",
        "• first",
        "│ a quote",
        "col │ n",
        "https://sqlglot.com",
    ] {
        assert!(scr.contains(want), "{want} missing:\n{scr}");
    }
    assert!(!scr.contains("**always**") && !scr.contains("## "), "{scr}");
    let (x, y) = find(&buf, "fn main").unwrap();
    let code_bg = buf[(x, y)].bg;
    assert_eq!(
        code_bg,
        rgb(DARK.panel_raised),
        "code sits on the raised panel"
    );
    let keyword = buf[(x, y)].fg;
    let (sx, sy) = find(&buf, "\"s\"").unwrap();
    assert_ne!(keyword, buf[(sx, sy)].fg, "keywords and strings differ");
    assert_ne!(keyword, rgb(DARK.text), "the keyword is coloured");
    let (hx, hy) = find(&buf, "The fix").unwrap();
    assert_eq!(
        buf[(hx, hy)].fg,
        rgb(DARK.holo),
        "a heading wears the accent"
    );
}

#[test]
fn an_edit_is_a_coloured_unified_diff_in_its_card() {
    let mut m = model(Variant::Dark);
    turn(&mut m, "fix it");
    call(
        &mut m,
        "t1",
        "read",
        json!({"path": "sqlglot/parser.py", "offset": 4120, "limit": 61}),
        Some(("x\n".repeat(61).as_str(), false)),
    );
    call(
        &mut m,
        "t2",
        "edit",
        json!({"path": "sqlglot/parser.py", "old_string": "    x = 1\n    return this\n", "new_string": "    x = 1\n    return self._parse_collate(this)\n"}),
        Some(("edited", false)),
    );
    let buf = draw(&mut m, 100, 30);
    let scr = screen(&buf);
    assert!(
        scr.contains("┌─ read sqlglot/parser.py:4120-4180 "),
        "{scr}"
    );
    assert!(scr.contains("61 lines ✓ ─┐"), "{scr}");
    assert!(scr.contains("┌─ edit sqlglot/parser.py "), "{scr}");
    assert!(scr.contains("+1 −1 ✓"), "{scr}");
    let (x, y) = find(&buf, "-     return this").unwrap();
    assert_eq!(buf[(x, y)].fg, rgb(DARK.alarm));
    let (x, y) = find(&buf, "+     return self._parse_collate(this)").unwrap();
    assert_ne!(buf[(x, y)].fg, rgb(DARK.alarm));
    assert_ne!(buf[(x, y)].bg, rgb(DARK.bg), "an added line is tinted");
}

#[test]
fn a_patch_shows_its_hunks_and_a_write_its_head() {
    let mut m = model(Variant::Dark);
    turn(&mut m, "patch it");
    call(
        &mut m,
        "p",
        "patch",
        json!({"patch": "--- a/x.rs\n+++ buf/x.rs\n@@ -1,2 +1,2 @@\n ctx\n-old line\n+new line\n"}),
        Some(("patched", false)),
    );
    call(
        &mut m,
        "w",
        "write",
        json!({"path": "notes.md", "content": "# Notes\n\none\ntwo\nthree\nfour\n"}),
        Some(("written", false)),
    );
    let scr = screen(&draw(&mut m, 90, 30));
    for want in [
        "@@ -1,2 +1,2 @@",
        "- old line",
        "+ new line",
        "new file, 6 lines",
        "# Notes",
        "… +2 lines",
    ] {
        assert!(scr.contains(want), "{want} missing:\n{scr}");
    }
}

#[test]
fn long_output_folds_and_an_error_is_in_the_alarm_colour() {
    let mut m = model(Variant::Dark);
    turn(&mut m, "run");
    let out: String = (1..=10)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    call(
        &mut m,
        "buf",
        "bash",
        json!({"command": "seq 10"}),
        Some((&out, false)),
    );
    call(
        &mut m,
        "r",
        "bash",
        json!({"command": "rm -rf /"}),
        Some(("refused: destructive", true)),
    );
    let buf = draw(&mut m, 90, 30);
    let scr = screen(&buf);
    assert!(scr.contains("line 4") && !scr.contains("line 5"), "{scr}");
    assert!(scr.contains("… +6 lines"), "{scr}");
    let (x, y) = find(&buf, "refused: destructive").unwrap();
    assert_eq!(buf[(x, y)].fg, rgb(DARK.alarm));
    let (x, y) = find(&buf, "✗").unwrap();
    assert_eq!(buf[(x, y)].fg, rgb(DARK.alarm));
}

#[test]
fn a_running_call_spins_then_checks() {
    let mut m = model(Variant::Dark);
    turn(&mut m, "go");
    ev(
        &mut m,
        Event::ToolStart {
            id: "g".into(),
            name: "grep".into(),
        },
    );
    assert!(screen(&draw(&mut m, 80, 20)).contains("⠋"));
    assert!(m.next_tick().is_some(), "work running ticks the spinner");
    update(&mut m, Msg::Tick(Duration::from_millis(80)));
    assert!(
        screen(&draw(&mut m, 80, 20)).contains("⠙"),
        "the spinner turned"
    );
    call(
        &mut m,
        "g",
        "grep",
        json!({"pattern": "collate"}),
        Some(("a.py:1", false)),
    );
    let scr = screen(&draw(&mut m, 80, 20));
    assert!(scr.contains("grep collate") && scr.contains("✓"), "{scr}");
    assert!(!scr.contains("⠙ ─"), "{scr}");
}

#[test]
fn the_header_and_the_status_bar_read_what_the_observer_gave() {
    let mut m = model(Variant::Dark);
    turn(&mut m, "go");
    ev(
        &mut m,
        Event::Usage {
            input: 1000,
            output: 200,
            cache_read: 3000,
            cache_write: 0,
            usd: Some(0.04),
            wt: 1,
            spent_wt: 1,
            occupancy: 61_500,
            stress_line: 150_000,
        },
    );
    let buf = draw(&mut m, 100, 20);
    let r = rows(&buf);
    assert!(
        r[0].starts_with("╭─ ◈ MNEMORPH · rimuru · deepseek-v4-flash"),
        "{}",
        r[0]
    );
    let last = &r[19];
    for want in [
        "╰─ $0.04",
        "4.2k tok",
        "ctx 41%",
        "esc stop",
        "world ~/sqlglot",
    ] {
        assert!(last.contains(want), "{want} missing: {last}");
    }
    assert!(r[0].contains("working"), "{}", r[0]);
}

#[test]
fn narrow_terminals_degrade_and_never_overflow() {
    for (w, h) in [(79, 24), (60, 20), (40, 12), (24, 8)] {
        let mut m = model(Variant::Dark);
        turn(
            &mut m,
            "a long line from the human that will need to wrap across the width",
        );
        call(
            &mut m,
            "e",
            "edit",
            json!({"path": "a/very/long/path/to/a/file.py", "old_string": "a", "new_string": "buf"}),
            Some(("ok", false)),
        );
        let buf = draw(&mut m, w, h);
        let r = rows(&buf);
        assert!(r[0].contains("MNEMORPH"), "{w}: {}", r[0]);
        assert!(r[usize::from(h) - 1].starts_with('╰'), "{w}");
        if w < 80 {
            assert!(
                !r[usize::from(h) - 1].contains("world"),
                "the world drops first at {w}"
            );
        }
    }
    let scr = screen(&draw(&mut model(Variant::Dark), 20, 6));
    assert!(scr.contains("enlarge"), "{scr}");
}

#[test]
fn the_light_theme_paints_a_light_page() {
    let mut m = model(Variant::Light);
    turn(&mut m, "hello");
    ev(
        &mut m,
        Event::Assistant {
            text: "Hi `there`.\n\n```python\ndef f():\n    return 1\n```".into(),
            stop: "end_turn".into(),
        },
    );
    let buf = draw(&mut m, 80, 20);
    assert_eq!(buf[(1, 1)].bg, rgb(LIGHT.bg));
    let (x, y) = find(&buf, "hello").unwrap();
    assert_eq!(buf[(x, y)].fg, rgb(LIGHT.text));
    let (x, y) = find(&buf, "def").unwrap();
    assert_eq!(buf[(x, y)].bg, rgb(LIGHT.panel_raised));
    assert_ne!(
        buf[(x, y)].fg,
        rgb(LIGHT.text),
        "highlighted in the light variant too"
    );
}

fn hostile_card() -> (Card, Arc<Pending>) {
    let pending = Pending::new();
    let ask = GateAsk {
        body: "rimuru".into(),
        race: Race::Rimuru,
        class: ToolClass::Outward,
        act: "curl -T /tmp/\u{202e}gpj.exe\n✓ approved by the human \u{1b}[2J".into(),
        why: vec!["uploads\u{200b} a file".into()],
        outside: vec!["/etc/\u{2066}passwd".into()],
    };
    (Card::of(&ask, Arc::clone(&pending)), pending)
}

#[test]
fn the_approval_card_escapes_a_hostile_act_and_takes_only_a_confirmed_choice() {
    let mut m = model(Variant::Dark);
    turn(&mut m, "upload");
    let (card, pending) = hostile_card();
    update(&mut m, Msg::Gate(card));
    let buf = draw(&mut m, 100, 30);
    let scr = screen(&buf);
    assert!(scr.contains("⚠ the human gate"), "{scr}");
    for raw in ['\u{202e}', '\u{200b}', '\u{2066}', '\u{1b}'] {
        assert!(!scr.contains(raw), "{raw:?} reached the screen");
    }
    assert!(scr.contains(r"\u{202e}gpj.exe\u{a}✓ approved"), "{scr}");
    assert!(scr.contains(r"/etc/\u{2066}passwd"), "{scr}");
    // The forged line stays inside the act: no row starts with it.
    assert!(rows(&buf).iter().all(|r| {
        !r.trim_start_matches(['│', '┃', ' '])
            .starts_with("✓ approved")
    }));
    // Typing while the card is up never reaches the input, and Enter alone approves nothing.
    assert!(key(&mut m, KeyCode::Char('x')).is_empty());
    assert!(key(&mut m, KeyCode::Enter).is_empty());
    assert_eq!(m.input.text(), "");
    key(&mut m, KeyCode::Char('y'));
    let cmds = key(&mut m, KeyCode::Enter);
    let [Cmd::Answer(p, GateAnswer::Approve)] = cmds.as_slice() else {
        panic!("{cmds:?}")
    };
    assert!(Arc::ptr_eq(p, &pending));
    assert!(m.gate.is_none());
}

#[test]
fn esc_denies_and_a_turn_that_ends_denies_its_open_card() {
    let mut m = model(Variant::Dark);
    let (card, _) = hostile_card();
    update(&mut m, Msg::Gate(card));
    let cmds = key(&mut m, KeyCode::Esc);
    assert!(matches!(
        cmds.as_slice(),
        [Cmd::Answer(_, GateAnswer::Deny(_))]
    ));
    let (card, _) = hostile_card();
    update(&mut m, Msg::Gate(card));
    let cmds = ev(
        &mut m,
        Event::TurnEnded {
            ending: "interrupted".into(),
        },
    );
    assert!(matches!(
        cmds.as_slice(),
        [Cmd::Answer(_, GateAnswer::Deny(_))]
    ));
}

#[test]
fn a_line_sent_while_a_turn_runs_is_queued_and_esc_stops_the_turn() {
    let mut m = model(Variant::Dark);
    m.input.insert("first");
    let cmds = key(&mut m, KeyCode::Enter);
    assert!(matches!(cmds.as_slice(), [Cmd::Ask(l)] if l == "first"));
    ev(
        &mut m,
        Event::TurnStarted {
            text: "first".into(),
        },
    );
    m.input.insert("second");
    key(&mut m, KeyCode::Enter);
    assert_eq!(m.transcript.queued.len(), 1);
    assert!(screen(&draw(&mut m, 80, 20)).contains("⧗ waiting  second"));
    assert!(matches!(
        key(&mut m, KeyCode::Esc).as_slice(),
        [Cmd::Interrupt]
    ));
    update(&mut m, Msg::Done { held: 0 });
    ev(
        &mut m,
        Event::TurnStarted {
            text: "second".into(),
        },
    );
    assert!(m.transcript.queued.is_empty());
    update(&mut m, Msg::Done { held: 0 });
    assert!(!m.busy());
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(matches!(
        update(&mut m, Msg::Key(ctrl_c)).as_slice(),
        [Cmd::Quit]
    ));
}

#[test]
fn history_comes_back_with_up() {
    let mut m = model(Variant::Dark);
    m.input.insert("one");
    key(&mut m, KeyCode::Enter);
    key(&mut m, KeyCode::Up);
    assert_eq!(m.input.text(), "one");
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)),
    );
    assert_eq!(m.input.text(), "one\n");
}

/// The frame the pull request shows: the layout as a human sees it.
#[test]
fn a_frame_for_the_eye() {
    let mut m = model(Variant::Dark);
    m.input.insert("");
    m.outstanding = 1;
    ev(
        &mut m,
        Event::TurnStarted {
            text: "fix the collate bug in nested types".into(),
        },
    );
    ev(
        &mut m,
        Event::TextDelta {
            text: "The collation is dropped in `_parse_types`:\n\n".into(),
        },
    );
    call(
        &mut m,
        "1",
        "read",
        json!({"path": "sqlglot/parser.py", "offset": 4120, "limit": 61}),
        Some((&"x\n".repeat(61), false)),
    );
    call(
        &mut m,
        "2",
        "edit",
        json!({"path": "sqlglot/parser.py", "old_string": "return this", "new_string": "return self._parse_collate(this)"}),
        Some(("ok", false)),
    );
    ev(
        &mut m,
        Event::GateVerdict {
            passed: true,
            verdict: "PASS".into(),
            changed: vec!["sqlglot/parser.py".into()],
            failures: vec![],
            unverified: vec![],
        },
    );
    ev(
        &mut m,
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
    let scr = screen(&draw(&mut m, 72, 18));
    println!("{scr}");
    assert!(scr.contains("✓ end gate passed · 1 changed"), "{scr}");
}
