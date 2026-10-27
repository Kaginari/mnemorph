//! `mnemorph-tui --resume`: a run with nothing to resume is refused in plain text, before the
//! terminal is taken.

use std::process::Command;

#[test]
fn a_missing_run_is_refused_before_the_terminal_is_taken() {
    let world = std::env::temp_dir().join(format!("mnemorph-tui-resume-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&world);
    std::fs::create_dir_all(&world).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mnemorph-tui"))
        .args(["--world", world.to_str().unwrap(), "--resume", "r9-9"])
        .env("XDG_STATE_HOME", world.join("state"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no run `r9-9`"), "{err}");
    let out = Command::new(env!("CARGO_BIN_EXE_mnemorph-tui"))
        .arg("--resume")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("--resume needs"));
    let _ = std::fs::remove_dir_all(&world);
}
