//! The UI alive and progressive, driven through the Elm `update` and drawn into the test
//! backend: the status from the frame an ask is sent, its phases, the reasoning live then folded
//! to its duration, a call's card from its first byte, looks grouped, long output folded, the
//! working body's icon, the end gate's line and toast. `a_turn_frame_by_frame` prints one ask's
//! frames with `--nocapture`.

use std::time::Duration;

use mnemorph_races::Race;
use mnemorph_server::protocol::Event;
use mnemorph_tools::Pending;
use mnemorph_tui::activity::{self, Phase};
use mnemorph_tui::app::{Cmd, Header, Model, Msg, update, view};
use mnemorph_tui::gate::Card;
use mnemorph_tui::kit::mnanim;
use mnemorph_tui::theme::Variant;
use mnemorph_tui::transcript::Entry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

fn model() -> Model {
    let mut m = Model::new(
        Header {
            body: "rimuru".into(),
            model: "deepseek-v4-flash".into(),
            world: "~/sqlglot".into(),
        },
        Variant::Dark,
        (100, 32),
    );
    m.session_id = "20261026-1412-a3f9c2".into();
    m
}

fn ev(m: &mut Model, event: Event) -> Vec<Cmd> {
    update(m, Msg::Event { event, below: None })
}

fn tick(m: &mut Model, secs: f64) {
    update(m, Msg::Tick(Duration::from_secs_f64(secs)));
}

fn screen(m: &mut Model, cols: u16, lines: u16) -> String {
    update(m, Msg::Resize(cols, lines));
    let mut term = Terminal::new(TestBackend::new(cols, lines)).unwrap();
    term.draw(|f| view(m, f)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn send(m: &mut Model, line: &str) -> Vec<Cmd> {
    m.input.insert(line);
    update(m, Msg::Key(KeyEvent::from(KeyCode::Enter)))
}

fn ctrl(m: &mut Model, c: char) {
    update(
        m,
        Msg::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
    );
}

#[test]
fn the_status_shows_on_send_before_any_token() {
    let mut m = model();
    assert!(
        !screen(&mut m, 100, 32).contains("Thinking…"),
        "idle: no status"
    );
    let cmds = send(&mut m, "fix the collate bug");
    assert!(matches!(cmds.as_slice(), [Cmd::Ask(_)]));
    // No event has come back: the first frame after the send already says what happens.
    let scr = screen(&mut m, 100, 32);
    assert!(scr.contains("Thinking… (0s · esc to interrupt)"), "{scr}");
    assert!(scr.contains("rimuru · thinking"), "{scr}");
    assert!(scr.contains("● thinking 0s"), "the header's chip:\n{scr}");
    assert_eq!(activity::phase(&m), Some(Phase::Thinking));
    assert!(m.next_tick().is_some(), "the spinner turns");
    tick(&mut m, 3.2);
    assert!(screen(&mut m, 100, 32).contains("Thinking… (3s"));
}

#[test]
fn the_phase_follows_the_turn() {
    let mut m = model();
    send(&mut m, "run the tests");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "run the tests".into(),
        },
    );
    assert_eq!(activity::phase(&m), Some(Phase::Thinking));
    ev(
        &mut m,
        Event::TextDelta {
            text: "Running them".into(),
        },
    );
    assert_eq!(activity::phase(&m), Some(Phase::Writing));
    ev(
        &mut m,
        Event::ToolStart {
            id: "b".into(),
            name: "bash".into(),
        },
    );
    assert_eq!(activity::phase(&m), Some(Phase::Calling("bash".into())));
    assert!(screen(&mut m, 100, 32).contains("rimuru · calling bash"));
    ev(
        &mut m,
        Event::ToolCall {
            id: "b".into(),
            name: "bash".into(),
            input: json!({"command": "pytest -q tests/"}),
        },
    );
    assert_eq!(
        activity::phase(&m),
        Some(Phase::Running("bash".into(), "pytest -q tests/".into()))
    );
    let scr = screen(&mut m, 100, 32);
    assert!(scr.contains("Running bash…"), "{scr}");
    assert!(
        scr.contains("rimuru · running bash: pytest -q tests/"),
        "{scr}"
    );
    // An act at the gate: the status waits for the human.
    update(&mut m, Msg::Gate(card("git push")));
    assert_eq!(activity::phase(&m), Some(Phase::Approval));
    assert!(screen(&mut m, 100, 32).contains("◆ waiting for you"));
    update(&mut m, Msg::Key(KeyEvent::from(KeyCode::Esc)));
    ev(
        &mut m,
        Event::ToolResult {
            id: "b".into(),
            content: "3 passed".into(),
            is_error: false,
            cut: 0,
        },
    );
    assert_eq!(
        activity::phase(&m),
        Some(Phase::Thinking),
        "the model again"
    );
    // Past the stress line, the next answer not begun: the loop compacts.
    ev(&mut m, usage(160_000, 150_000));
    assert_eq!(activity::phase(&m), Some(Phase::Compacting));
    assert!(screen(&mut m, 100, 32).contains("ctx 106%"));
    ev(
        &mut m,
        Event::TurnEnded {
            ending: "answered".into(),
        },
    );
    update(&mut m, Msg::Done { held: 0 });
    assert_eq!(activity::phase(&m), None);
    assert!(screen(&mut m, 100, 32).contains("○ idle"));
}

fn usage(occupancy: u64, stress_line: u64) -> Event {
    Event::Usage {
        input: 1200,
        output: 400,
        cache_read: 0,
        cache_write: 0,
        usd: None,
        wt: 1,
        spent_wt: 1,
        occupancy,
        stress_line,
    }
}

fn card(act: &str) -> Card {
    Card::of(
        &mnemorph_loop::GateAsk {
            body: "rimuru".into(),
            race: Race::Rimuru,
            class: mnemorph_loop::ToolClass::Outward,
            act: act.into(),
            why: vec!["reaches beyond the world".into()],
            outside: vec![],
        },
        Pending::new(),
    )
}

#[test]
fn thinking_shows_live_then_folds_to_its_duration() {
    let mut m = model();
    send(&mut m, "why is it slow");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "why is it slow".into(),
        },
    );
    update(
        &mut m,
        Msg::Thinking("The parser walks the tree twice".into()),
    );
    tick(&mut m, 6.0);
    update(
        &mut m,
        Msg::Thinking("; once for types, once for collations.".into()),
    );
    tick(&mut m, 6.0);
    let scr = screen(&mut m, 100, 32);
    assert!(scr.contains("◌ thinking · 12s · "), "{scr}");
    assert!(
        scr.contains("once for collations."),
        "its latest words:\n{scr}"
    );
    // One line, updated in place: never the whole reasoning dumped.
    assert_eq!(scr.matches("thinking · ").count(), 1, "{scr}");
    ev(
        &mut m,
        Event::TextDelta {
            text: "It walks twice.".into(),
        },
    );
    let scr = screen(&mut m, 100, 32);
    assert!(scr.contains("✓ thought for 12s · ctrl+t"), "{scr}");
    assert!(!scr.contains("once for types"), "folded:\n{scr}");
    ctrl(&mut m, 't');
    let scr = screen(&mut m, 100, 32);
    assert!(scr.contains("once for types"), "expanded on demand:\n{scr}");
}

#[test]
fn reasoning_that_never_streamed_is_placed_where_its_step_began() {
    let mut m = model();
    send(&mut m, "plan it");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "plan it".into(),
        },
    );
    tick(&mut m, 4.0);
    ev(
        &mut m,
        Event::TextDelta {
            text: "Here is the plan.\n\n".into(),
        },
    );
    // The provider kept its reasoning to the end: the turn's blocks carry it whole.
    let blocks = vec![
        json!({"type": "opaque", "value": {"type": "thinking", "thinking": "Two steps.", "signature": "s"}}),
        json!({"type": "text", "text": "Here is the plan."}),
    ];
    let text = mnemorph_tui::run::reasoning(&blocks).unwrap();
    assert_eq!(text, "Two steps.");
    update(&mut m, Msg::Reasoned(text));
    let kinds: Vec<&str> = m
        .transcript
        .entries()
        .iter()
        .map(|e| match e {
            Entry::User(_) => "user",
            Entry::Thought(_) => "thought",
            Entry::Assistant { .. } => "text",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["user", "thought", "text"]);
    assert!(screen(&mut m, 100, 32).contains("✓ thought for 4s"));
    // A redacted block is reasoning too, its text hidden; a turn with none has none.
    let hidden =
        vec![json!({"type": "opaque", "value": {"type": "redacted_thinking", "data": "x"}})];
    assert_eq!(mnemorph_tui::run::reasoning(&hidden), Some(String::new()));
    assert_eq!(
        mnemorph_tui::run::reasoning(&[json!({"type": "text"})]),
        None
    );
    let openai = vec![
        json!({"type": "opaque", "value": {"type": "openai.reasoning_content", "text": "hm"}}),
    ];
    assert_eq!(mnemorph_tui::run::reasoning(&openai).as_deref(), Some("hm"));
}

#[test]
fn a_call_has_its_card_from_its_first_byte() {
    let mut m = model();
    send(&mut m, "test");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "test".into(),
        },
    );
    ev(
        &mut m,
        Event::ToolStart {
            id: "b".into(),
            name: "bash".into(),
        },
    );
    let scr = screen(&mut m, 100, 32);
    assert!(
        scr.contains("⣾ bash") && scr.contains("⎿ running…"),
        "{scr}"
    );
    update(
        &mut m,
        Msg::ToolInput {
            id: "b".into(),
            json: r#"{"command": "pytest -q te"#.into(),
        },
    );
    assert!(
        screen(&mut m, 100, 32).contains("bash  pytest -q te"),
        "the summary as its arguments arrive"
    );
    tick(&mut m, 2.0);
    ev(
        &mut m,
        Event::ToolCall {
            id: "b".into(),
            name: "bash".into(),
            input: json!({"command": "pytest -q tests/"}),
        },
    );
    update(
        &mut m,
        Msg::Classed {
            id: "b".into(),
            class: "read".into(),
        },
    );
    tick(&mut m, 0.5);
    ev(
        &mut m,
        Event::ToolResult {
            id: "b".into(),
            content: "3 passed".into(),
            is_error: false,
            cut: 0,
        },
    );
    let scr = screen(&mut m, 100, 32);
    assert!(scr.contains("● bash  pytest -q tests/  read"), "{scr}");
    assert!(scr.contains("⎿ ok · 2.5s · 1 line"), "{scr}");
}

#[test]
fn looks_in_a_row_are_one_compact_card() {
    let mut m = model();
    send(&mut m, "look");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "look".into(),
        },
    );
    let look = |m: &mut Model, id: &str, name: &str, input: serde_json::Value| {
        ev(
            m,
            Event::ToolCall {
                id: id.into(),
                name: name.into(),
                input,
            },
        );
    };
    look(&mut m, "1", "read", json!({"path": "a.py"}));
    look(&mut m, "2", "read", json!({"path": "b.py"}));
    look(&mut m, "3", "grep", json!({"pattern": "collate"}));
    let scr = screen(&mut m, 100, 32);
    assert!(scr.contains("read 2 files · grep 1 pattern"), "{scr}");
    assert!(scr.contains("⎿ grep collate"), "the one running:\n{scr}");
    for id in ["1", "2", "3"] {
        ev(
            &mut m,
            Event::ToolResult {
                id: id.into(),
                content: "x\ny".into(),
                is_error: false,
                cut: 0,
            },
        );
    }
    let groups = m
        .transcript
        .entries()
        .iter()
        .filter(|e| matches!(e, Entry::Group(_)))
        .count();
    assert_eq!(groups, 1);
    let scr = screen(&mut m, 100, 32);
    assert!(
        scr.contains("ctrl+t lists them") && !scr.contains("✓ read a.py"),
        "{scr}"
    );
    ctrl(&mut m, 't');
    let scr = screen(&mut m, 100, 32);
    assert!(
        scr.contains("✓ read a.py") && scr.contains("✓ grep collate"),
        "{scr}"
    );
    // A call that is not a look ends the group.
    look(&mut m, "4", "bash", json!({"command": "ls"}));
    look(&mut m, "5", "read", json!({"path": "c.py"}));
    let last = m.transcript.entries().last().unwrap();
    assert!(matches!(last, Entry::Tool(c) if c.name == "read"));
}

#[test]
fn large_output_stays_folded() {
    let mut m = model();
    send(&mut m, "dump");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "dump".into(),
        },
    );
    let out = (1..=500)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    ev(
        &mut m,
        Event::ToolCall {
            id: "b".into(),
            name: "bash".into(),
            input: json!({"command": "seq 500"}),
        },
    );
    ev(
        &mut m,
        Event::ToolResult {
            id: "b".into(),
            content: out,
            is_error: false,
            cut: 0,
        },
    );
    let scr = screen(&mut m, 100, 40);
    assert!(scr.contains("line 3") && !scr.contains("line 4"), "{scr}");
    assert!(
        scr.contains("… +497 lines · click or ctrl+t to expand"),
        "{scr}"
    );
}

/// The portrait at the sidebar's head: its rows, the padding that centres it cut away.
fn head(m: &Model) -> Vec<String> {
    let lines = mnemorph_tui::sidebar::view(m, 29, 37);
    lines[..mnanim::SIZE / 2]
        .iter()
        .map(|l| format!("{:?}", &l.spans[1..]))
        .collect()
}

/// `race`'s portrait at `clock`, as the head draws it.
fn veldora(m: &Model) -> Vec<String> {
    mnemorph_tui::sidebar::veldora_blocks(&m.theme)
        .iter()
        .map(|l| format!("{:?}", &l.spans[..]))
        .collect()
}

fn portrait(race: Race, clock: Duration) -> Vec<String> {
    let p = mnanim::portrait(race).unwrap();
    mnanim::lines(p.at(clock), None)
        .iter()
        .map(|l| format!("{:?}", &l.spans[..]))
        .collect()
}

#[test]
fn the_portrait_is_the_working_bodys_and_swaps_as_a_child_starts_and_stops() {
    mnanim::warm();
    let mut m = model();
    update(&mut m, Msg::Resize(120, 40));
    send(&mut m, "judge it");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "judge it".into(),
        },
    );
    assert_eq!(activity::actor(&m), ("rimuru".into(), Race::Rimuru));
    assert_eq!(head(&m), portrait(Race::Rimuru, m.portrait_clock));
    let below = |m: &mut Model, chain: &[&str], event: Event| {
        update(
            m,
            Msg::Below {
                event,
                chain: chain.iter().map(|s| (*s).to_owned()).collect(),
            },
        );
    };
    below(
        &mut m,
        &["orc-sql"],
        Event::TurnStarted {
            text: "judge".into(),
        },
    );
    assert_eq!(activity::actor(&m), ("orc-sql".into(), Race::Orc));
    assert_eq!(head(&m), portrait(Race::Orc, m.portrait_clock), "the orc's");
    assert!(screen(&mut m, 120, 40).contains("orc-sql · working"));
    below(
        &mut m,
        &["orc-sql", "slime-parser"],
        Event::ToolStart {
            id: "s".into(),
            name: "read".into(),
        },
    );
    assert_eq!(activity::actor(&m).1, Race::Slime);
    below(
        &mut m,
        &["orc-sql", "slime-parser"],
        Event::TurnEnded {
            ending: "answered".into(),
        },
    );
    assert_eq!(activity::actor(&m).1, Race::Orc, "back to its commissioner");
    below(
        &mut m,
        &["orc-sql"],
        Event::TurnEnded {
            ending: "answered".into(),
        },
    );
    assert_eq!(activity::actor(&m), ("rimuru".into(), Race::Rimuru));
    assert_eq!(head(&m), portrait(Race::Rimuru, m.portrait_clock));
}

#[test]
fn the_portrait_plays_only_while_a_body_works_and_shows() {
    mnanim::warm();
    let mut m = model();
    update(&mut m, Msg::Resize(120, 40));
    assert!(m.portrait_shown());
    assert_eq!(
        m.next_tick(),
        None,
        "idle: nothing ticks, the portrait still"
    );
    // Idle: Veldora, the human, still; a body's race and loop only while it works.
    assert_eq!(head(&m), veldora(&m));
    send(&mut m, "go");
    assert_eq!(
        m.next_tick(),
        Some(Duration::from_millis(1000 / 16)),
        "working: the loop's own pace"
    );
    tick(&mut m, 0.1);
    assert_eq!(m.portrait_clock, Duration::from_millis(100));
    assert_eq!(head(&m), portrait(Race::Rimuru, Duration::from_millis(100)));
    assert_ne!(
        head(&m),
        portrait(Race::Rimuru, Duration::ZERO),
        "it moved on a frame"
    );
    // The sidebar hidden: the spinner ticks, the portrait does not move.
    ctrl(&mut m, 'o');
    assert!(!m.portrait_shown());
    assert_eq!(m.next_tick(), Some(Duration::from_millis(100)));
    tick(&mut m, 1.0);
    assert_eq!(m.portrait_clock, Duration::from_millis(100));
    // A short terminal keeps the sidebar but not its head: no portrait, no ticks for it.
    ctrl(&mut m, 'o');
    update(&mut m, Msg::Resize(120, 30));
    assert!(!m.portrait_shown());
    // Idle again: no ticks, and Veldora rests in Rimuru's place.
    update(&mut m, Msg::Resize(120, 40));
    update(&mut m, Msg::Done { held: 0 });
    assert_eq!(m.next_tick(), None);
    assert_eq!(head(&m), veldora(&m));
}

#[test]
fn the_end_gate_is_one_line_and_a_toast() {
    let mut m = model();
    send(&mut m, "fix");
    ev(&mut m, Event::TurnStarted { text: "fix".into() });
    ev(
        &mut m,
        Event::GateVerdict {
            passed: false,
            verdict: "FAIL".into(),
            changed: vec!["a.py".into(), "b.py".into()],
            failures: vec!["tests: 2 failed".into()],
            unverified: vec!["lint".into()],
        },
    );
    let scr = screen(&mut m, 120, 32);
    assert!(
        scr.contains("✗ end gate · fail  · 2 changed · 1 failed · 1 unverified: tests: 2 failed"),
        "{scr}"
    );
    assert!(m.toast.is_some());
    for _ in 0..20 {
        tick(&mut m, 1.0 / 30.0);
    }
    assert!(
        screen(&mut m, 120, 32).contains("◆ end gate fail"),
        "the toast"
    );
    for _ in 0..200 {
        tick(&mut m, 1.0 / 30.0);
    }
    assert!(m.toast.is_none(), "it fades and leaves");
}

#[test]
// One ask's script, beat by beat: splitting it would hide the order it is about.
#[allow(clippy::too_many_lines)]
fn a_turn_frame_by_frame() {
    let mut m = model();
    let mut frames: Vec<(String, String)> = Vec::new();
    let mut snap = |m: &mut Model, what: &str| {
        frames.push((what.to_owned(), screen(m, 100, 30)));
    };
    snap(&mut m, "the welcome");
    send(&mut m, "why does the collation drop in nested types?");
    snap(&mut m, "sent: the status before any token");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "why does the collation drop in nested types?".into(),
        },
    );
    update(
        &mut m,
        Msg::Thinking(
            "The collation is parsed after the type, but nested types return early".into(),
        ),
    );
    tick(&mut m, 4.0);
    snap(&mut m, "thinking, live");
    ev(
        &mut m,
        Event::ToolStart {
            id: "g".into(),
            name: "grep".into(),
        },
    );
    update(
        &mut m,
        Msg::ToolInput {
            id: "g".into(),
            json: r#"{"pattern": "_parse_ty"#.into(),
        },
    );
    snap(&mut m, "a call from its first byte");
    ev(
        &mut m,
        Event::ToolCall {
            id: "g".into(),
            name: "grep".into(),
            input: json!({"pattern": "_parse_types"}),
        },
    );
    tick(&mut m, 0.3);
    ev(
        &mut m,
        Event::ToolResult {
            id: "g".into(),
            content: "sqlglot/parser.py:4120\nsqlglot/parser.py:4188".into(),
            is_error: false,
            cut: 0,
        },
    );
    ev(
        &mut m,
        Event::ToolCall {
            id: "r".into(),
            name: "read".into(),
            input: json!({"path": "sqlglot/parser.py", "offset": 4120, "limit": 61}),
        },
    );
    tick(&mut m, 0.2);
    ev(
        &mut m,
        Event::ToolResult {
            id: "r".into(),
            content: "x\n".repeat(61),
            is_error: false,
            cut: 0,
        },
    );
    ev(&mut m, usage(61_500, 150_000));
    ev(
        &mut m,
        Event::TextDelta {
            text: "Nested types return before `_parse_collate` runs.\n\nThe fix".into(),
        },
    );
    snap(&mut m, "the answer streaming");
    ev(
        &mut m,
        Event::TextDelta {
            text: " is to parse the collation in the nested branch too.".into(),
        },
    );
    ev(
        &mut m,
        Event::Assistant {
            text: "Nested types return before `_parse_collate` runs.\n\nThe fix is to parse the collation in the nested branch too.".into(),
            stop: "end_turn".into(),
        },
    );
    ev(
        &mut m,
        Event::GateVerdict {
            passed: true,
            verdict: "PASS".into(),
            changed: vec![],
            failures: vec![],
            unverified: vec![],
        },
    );
    ev(
        &mut m,
        Event::TurnEnded {
            ending: "answered".into(),
        },
    );
    update(&mut m, Msg::Done { held: 0 });
    tick(&mut m, 0.5);
    snap(&mut m, "answered");
    update(&mut m, Msg::Gate(card("git push origin main")));
    m.outstanding = 1;
    snap(&mut m, "the approval");
    for (what, scr) in &frames {
        println!("── {what} ──\n{scr}\n");
    }
    assert!(frames[1].1.contains("Thinking…"));
    assert!(frames[2].1.contains("◌ thinking · 4s"));
    assert!(frames[3].1.contains("grep  _parse_ty"));
    assert!(frames[4].1.contains("✓ thought for 4s"));
    assert!(frames[5].1.contains("✓ end gate · pass"));
    assert!(frames[6].1.contains("Approval · ⚠ the human gate"));
}

/// Past the card's arming, as a human would be.
fn armed() {
    std::thread::sleep(mnemorph_tui::approval::ARM + Duration::from_millis(30));
}

/// A pick, the pause a human takes, then Enter.
fn after_pause(m: &mut Model) -> Vec<Cmd> {
    std::thread::sleep(mnemorph_tui::approval::CONFIRM + Duration::from_millis(20));
    update(m, Msg::Key(KeyEvent::from(KeyCode::Enter)))
}

fn approved(cmds: &[Cmd]) -> bool {
    cmds.iter()
        .any(|c| matches!(c, Cmd::Answer(_, mnemorph_loop::GateAnswer::Approve)))
}

#[test]
fn arrows_never_land_on_yes_unasked() {
    let key = |m: &mut Model, code| update(m, Msg::Key(KeyEvent::from(code)));
    // ↓ from no pick is "No": Enter denies, it never approves.
    let mut m = model();
    update(&mut m, Msg::Gate(card("git push")));
    screen(&mut m, 100, 32);
    armed();
    key(&mut m, KeyCode::Down);
    let cmds = after_pause(&mut m);
    assert!(!approved(&cmds), "{cmds:?}");
    assert!(
        matches!(
            cmds.as_slice(),
            [Cmd::Answer(_, mnemorph_loop::GateAnswer::Deny(_))]
        ),
        "{cmds:?}"
    );
    // ↓ again and again stops on the last answer; it never wraps round to "Yes".
    let mut m = model();
    update(&mut m, Msg::Gate(card("git push")));
    screen(&mut m, 100, 32);
    armed();
    for _ in 0..5 {
        key(&mut m, KeyCode::Down);
    }
    assert_eq!(
        m.gate.as_ref().map(|a| (a.choice, a.why)),
        Some((Some(false), true))
    );
    // ↑ from no pick is "No" too; only a second ↑, asked for, reaches "Yes".
    let mut m = model();
    update(&mut m, Msg::Gate(card("git push")));
    screen(&mut m, 100, 32);
    armed();
    key(&mut m, KeyCode::Up);
    assert_eq!(m.gate.as_ref().and_then(|a| a.choice), Some(false));
    key(&mut m, KeyCode::Up);
    assert_eq!(m.gate.as_ref().and_then(|a| a.choice), Some(true));
    assert!(approved(&after_pause(&mut m)));
}

#[test]
fn a_group_with_a_stopped_call_says_so() {
    let mut m = model();
    send(&mut m, "look");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "look".into(),
        },
    );
    for (id, path) in [("1", "a.py"), ("2", "b.py")] {
        ev(
            &mut m,
            Event::ToolCall {
                id: id.into(),
                name: "read".into(),
                input: json!({"path": path}),
            },
        );
    }
    ev(
        &mut m,
        Event::ToolResult {
            id: "1".into(),
            content: "x".into(),
            is_error: false,
            cut: 0,
        },
    );
    ev(
        &mut m,
        Event::TurnEnded {
            ending: "interrupted".into(),
        },
    );
    let scr = screen(&mut m, 100, 32);
    assert!(scr.contains("⎿ 1 stopped"), "{scr}");
    assert!(!scr.contains("⎿ ok"), "{scr}");
}

#[test]
fn a_retried_stream_does_not_double_the_thought() {
    let mut m = model();
    send(&mut m, "think");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "think".into(),
        },
    );
    // The first attempt streamed its reasoning, failed before any answer, and the retry
    // streamed it again.
    update(&mut m, Msg::Thinking("Two steps.".into()));
    update(&mut m, Msg::Thinking("Two steps.".into()));
    ev(
        &mut m,
        Event::TextDelta {
            text: "Done.".into(),
        },
    );
    update(&mut m, Msg::Reasoned("Two steps.".into()));
    let thought = m.transcript.entries().iter().find_map(|e| match e {
        Entry::Thought(t) => Some(t.text.clone()),
        _ => None,
    });
    assert_eq!(thought.as_deref(), Some("Two steps."));
}

#[test]
fn a_toast_never_covers_the_approval() {
    let mut m = model();
    ev(
        &mut m,
        Event::GateVerdict {
            passed: true,
            verdict: "PASS".into(),
            changed: vec![],
            failures: vec![],
            unverified: vec![],
        },
    );
    for _ in 0..20 {
        tick(&mut m, 1.0 / 30.0);
    }
    assert!(screen(&mut m, 100, 32).contains("◆ end gate pass"));
    update(&mut m, Msg::Gate(card("git push")));
    let scr = screen(&mut m, 100, 32);
    assert!(!scr.contains("◆ end gate pass"), "{scr}");
    assert!(scr.contains("Approval · ⚠ the human gate"));
}

#[test]
fn two_working_children_keep_one_portrait_until_none_works() {
    mnanim::warm();
    let mut m = model();
    update(&mut m, Msg::Resize(120, 40));
    send(&mut m, "fan out");
    ev(
        &mut m,
        Event::TurnStarted {
            text: "fan out".into(),
        },
    );
    let below = |m: &mut Model, chain: &[&str], event: Event| {
        update(
            m,
            Msg::Below {
                event,
                chain: chain.iter().map(|s| (*s).to_owned()).collect(),
            },
        );
    };
    let start = |name: &str| Event::TurnStarted { text: name.into() };
    let step = |id: &str| Event::ToolStart {
        id: id.into(),
        name: "read".into(),
    };
    below(&mut m, &["orc-a"], start("a"));
    below(&mut m, &["slime-b"], start("b"));
    assert_eq!(activity::actor(&m).0, "slime-b", "the newest that works");
    tick(&mut m, 0.25);
    let clock = m.portrait_clock;
    assert!(clock > Duration::ZERO);
    // Their events alternate: the portrait stays, its loop runs on.
    for i in 0..6 {
        let who = if i % 2 == 0 { "orc-a" } else { "slime-b" };
        below(&mut m, &[who], step(&format!("{who}{i}")));
        assert_eq!(activity::actor(&m).0, "slime-b", "event {i}");
        assert_eq!(head(&m), portrait(Race::Slime, m.portrait_clock));
    }
    assert_eq!(m.portrait_clock, clock, "no event restarted the loop");
    // The slime ends while the orc works: the orc's portrait, from its first frame.
    below(
        &mut m,
        &["slime-b"],
        Event::TurnEnded {
            ending: "answered".into(),
        },
    );
    assert_eq!(activity::actor(&m), ("orc-a".into(), Race::Orc));
    assert_eq!(
        m.portrait_clock,
        Duration::ZERO,
        "a real swap restarts the loop"
    );
    // Rimuru only when none works.
    below(
        &mut m,
        &["orc-a"],
        Event::TurnEnded {
            ending: "answered".into(),
        },
    );
    assert_eq!(activity::actor(&m), ("rimuru".into(), Race::Rimuru));
}
