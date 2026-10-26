//! A long session stays smooth: 2,000 messages of Markdown, code and tool cards with diffs, the
//! sidebar open beside them with every file they changed and a tree of bodies, and a frame lays
//! out only what it shows. The timings print with `--nocapture`; the bounds hold
//! in a debug build, so an optimised one has room to spare.

use std::time::{Duration, Instant};

use mnemorph_server::protocol::Event;
use mnemorph_tui::app::{Header, Model, Msg, update, view};
use mnemorph_tui::theme::Variant;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde_json::json;

const MESSAGES: usize = 2_000;

fn session() -> Model {
    let mut m = Model::new(
        Header {
            body: "rimuru".into(),
            model: "m".into(),
            world: "~/w".into(),
        },
        Variant::Dark,
        (120, 40),
    );
    let ev = |m: &mut Model, event| {
        update(m, Msg::Event { event, below: None });
    };
    for i in 0..MESSAGES.div_ceil(3) {
        ev(
            &mut m,
            Event::TurnStarted {
                text: format!("ask number {i}: fix the thing in module {i}"),
            },
        );
        ev(
            &mut m,
            Event::Assistant {
                text: format!(
                    "## Step {i}\n\nThe *bug* is in `f{i}`; see **this**:\n\n- one\n- two\n\n```rust\nfn f{i}() -> u32 {{ {i} }}\n```"
                ),
                stop: "tool_use".into(),
            },
        );
        ev(
            &mut m,
            Event::ToolCall {
                id: format!("e{i}"),
                name: "edit".into(),
                input: json!({"path": format!("src/m{i}.rs"), "old_string": "a\nb\nc\n", "new_string": "a\nB\nc\nd\n"}),
            },
        );
        ev(
            &mut m,
            Event::ToolResult {
                id: format!("e{i}"),
                content: "edited".into(),
                is_error: false,
                cut: 0,
            },
        );
        ev(
            &mut m,
            Event::TurnEnded {
                ending: "answered".into(),
            },
        );
    }
    // The sidebar's world: a standing, and forty bodies two deep, half still working.
    update(
        &mut m,
        Msg::Surveyed(mnemorph_tui::sidebar::Survey {
            memory: mnemorph_loop::world::memory::Standing {
                shared: 120,
                proposed: 40,
                grounded: 300,
                latest: Some("the parser keeps collations in nested types".into()),
            },
            races: (0..40)
                .map(|i| (format!("slime-{i}"), mnemorph_races::Race::Slime))
                .collect(),
        }),
    );
    for i in 0..40 {
        let chain = vec![format!("orc-{}", i % 4), format!("slime-{i}")];
        let event = if i % 2 == 0 {
            Event::TurnEnded {
                ending: "answered".into(),
            }
        } else {
            Event::ToolStart {
                id: format!("s{i}"),
                name: "edit".into(),
            }
        };
        update(&mut m, Msg::Below { event, chain });
    }
    m
}

fn frame(t: &mut Terminal<TestBackend>, m: &Model) -> Duration {
    let start = Instant::now();
    t.draw(|f| view(m, f)).unwrap();
    start.elapsed()
}

#[test]
fn two_thousand_messages_stay_smooth() {
    // The grammars load once per process, off the UI thread in the binary: not a frame's cost.
    let load = Instant::now();
    mnemorph_tui::kit::highlight::preload();
    let load = load.elapsed();
    let mut m = session();
    assert!(m.transcript.entries().len() >= MESSAGES);
    let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let cold = frame(&mut t, &m);
    let shown: String = {
        let b = t.backend().buffer();
        (0..b.area.height)
            .flat_map(|y| (0..b.area.width).map(move |x| (x, y)))
            .map(|c| b[c].symbol().to_owned())
            .collect()
    };
    assert!(
        shown.contains(&format!("files · {}", MESSAGES.div_ceil(3)))
            && shown.contains("bodies · 44"),
        "the sidebar is drawn"
    );
    let warm: Duration = (0..50).map(|_| frame(&mut t, &m)).sum::<Duration>() / 50;
    // Streaming: a delta, a frame, three hundred times — every frame re-renders the live text.
    update(
        &mut m,
        Msg::Event {
            event: Event::TurnStarted {
                text: "stream".into(),
            },
            below: None,
        },
    );
    let mut streaming = Duration::ZERO;
    for i in 0..300 {
        let text = if i % 40 == 39 {
            "\n\n".to_owned()
        } else {
            format!("word{i} ")
        };
        update(
            &mut m,
            Msg::Event {
                event: Event::TextDelta { text },
                below: None,
            },
        );
        streaming += frame(&mut t, &m);
    }
    let streaming = streaming / 300;
    // A resize lays out again only what shows.
    let mut t2 = Terminal::new(TestBackend::new(90, 40)).unwrap();
    let resized = frame(&mut t2, &m);
    // Paging up through the history: each page lays out a page more.
    let mut paging = Duration::ZERO;
    for _ in 0..20 {
        update(&mut m, Msg::Key(KeyEvent::from(KeyCode::PageUp)));
        update(&mut m, Msg::Tick(Duration::from_secs(1)));
        paging += frame(&mut t, &m);
    }
    let paging = paging / 20;
    println!(
        "grammars {load:?} once · {MESSAGES} messages, 120x40: cold {cold:?} · warm {warm:?} · streaming {streaming:?}/frame · resize {resized:?} · page up {paging:?}/frame"
    );
    let frame_budget = Duration::from_millis(1000 / 30);
    assert!(warm < frame_budget, "warm frame {warm:?}");
    assert!(streaming < frame_budget, "streaming frame {streaming:?}");
    assert!(paging < frame_budget, "page-up frame {paging:?}");
    assert!(cold < Duration::from_millis(100), "cold frame {cold:?}");
    assert!(
        resized < Duration::from_millis(100),
        "resized frame {resized:?}"
    );
}
