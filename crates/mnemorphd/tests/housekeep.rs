//! `mnemorphd housekeep --verify` walks each chained journal, says its reading escaped, and
//! anchors the heads in `log.md`.

use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use sha2::{Digest as _, Sha256};

fn core() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_mnemorphd"));
    c.env_remove("ANTHROPIC_API_KEY")
        .env_remove("MNEMORPH_BODY");
    c
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// One chained row, sealed by hand as the book describes it.
fn sealed(prev: &str, genesis: bool, fields: &str) -> (String, String) {
    let mut covered = format!("{{\"prev\":\"{prev}\"");
    if genesis {
        covered.push_str(",\"genesis\":true");
    }
    covered.push(',');
    covered.push_str(fields);
    let digest = hex(covered.as_bytes());
    (format!("{covered},\"digest\":\"{digest}\"}}\n"), digest)
}

#[test]
fn verify_says_each_chain_and_anchors_its_head() {
    let world = std::env::temp_dir().join(format!("mnemorphd-housekeep-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    let usage = world.join(".mnemorph/instruments/usage.jsonl");
    fs::create_dir_all(usage.parent().unwrap()).unwrap();
    let (first, d1) = sealed(&hex(b""), true, "\"wt\":1");
    let (second, d2) = sealed(&d1, false, "\"wt\":2");
    fs::write(&usage, format!("{first}{second}")).unwrap();
    let w = world.to_str().unwrap();
    let state = world.join("state");
    let out = core()
        .args(["housekeep", "--verify", "--world", w])
        .env("XDG_STATE_HOME", &state)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("usage (") && text.contains(&format!("OK, head {d2} at line 2")),
        "{text}"
    );
    for name in ["value", "evolution", "gate", "minds", "uses", "revocations"] {
        assert!(text.contains(&format!("{name} (")), "{name}: {text}");
    }
    assert!(text.contains("anchored in log.md: ### ["), "{text}");
    let log = fs::read_to_string(world.join(".mnemorph/log.md")).unwrap();
    assert!(
        log.contains(&format!("- **usage chain:** OK, head {d2} at line 2")),
        "{log}"
    );

    // The last row dropped: every link holds, the head kept under the state directory does not.
    fs::write(&usage, &first).unwrap();
    let out = core()
        .args(["housekeep", "--verify", "--world", w])
        .env("XDG_STATE_HOME", &state)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("broken at line 2: the anchored head"),
        "{text}"
    );

    // The first row edited: found at its line, exit 1.
    fs::write(
        &usage,
        format!("{}{second}", first.replace("\"wt\":1", "\"wt\":9")),
    )
    .unwrap();
    let out = core()
        .args(["housekeep", "--verify", "--world", w])
        .env("XDG_STATE_HOME", &state)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("usage (") && text.contains("broken at line 1"),
        "{text}"
    );

    // --verify heals nothing.
    let out = core()
        .args(["housekeep", "--verify", "--heal", "--world", w])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let _ = fs::remove_dir_all(&world);
}
