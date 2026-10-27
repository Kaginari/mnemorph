//! `mnemorphd runs` lists the runs that may be resumed; `mnemorphd --resume` refuses one that
//! is not there as the `initialize` call's error.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn core() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_mnemorphd"));
    c.env_remove("ANTHROPIC_API_KEY");
    c
}

#[test]
fn runs_lists_the_stops_and_resume_refuses_a_missing_run() {
    let world = std::env::temp_dir().join(format!("mnemorphd-runs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&world);
    let w = world.to_str().unwrap();
    let out = core().args(["runs", "--world", w]).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "no run stopped to be resumed\n"
    );
    let dir = world.join(".mnemorph/instruments/loop");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("r5-5.jsonl"),
        "{\"beat\":\"ask\",\"text\":\"tidy the docs\"}\n{\"beat\":\"end\",\"ending\":\"checkpoint: steps: 40 taken\"}\n",
    )
    .unwrap();
    let out = core().args(["runs", "--world", w]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("r5-5  "), "{text}");
    assert!(text.contains("steps: 40 taken"), "{text}");
    assert!(text.contains("asked: tidy the docs"), "{text}");
    let out = core()
        .args(["runs", "--json", "--world", w])
        .output()
        .unwrap();
    let row: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert_eq!(row["run"], "r5-5");
    assert_eq!(row["why"], "steps: 40 taken");

    let mut child = core()
        .args(["--world", w, "--resume", "r9-9"])
        .env("XDG_STATE_HOME", world.join("state"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let reply = String::from_utf8_lossy(&out.stdout);
    assert!(
        reply.contains("-32001") && reply.contains("no run `r9-9`"),
        "{reply}"
    );
    let _ = fs::remove_dir_all(&world);
}
