//! `mnemorph resume` and `--resume`: a run with nothing to resume is refused in plain text, before
//! the terminal is taken.

#[path = "../../mnemorphd/tests/support/isolated.rs"]
mod isolated;

#[test]
fn a_missing_run_is_refused_before_the_terminal_is_taken() {
    let world = std::env::temp_dir().join(format!("mnemorph-tui-resume-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&world);
    std::fs::create_dir_all(&world).unwrap();
    for args in [["--resume", "r9-9"], ["resume", "r9-9"]] {
        let out = isolated::bin(env!("CARGO_BIN_EXE_mnemorph"))
            .args(["--world", world.to_str().unwrap()])
            .args(args)
            .env("XDG_STATE_HOME", world.join("state"))
            .output()
            .unwrap();
        assert!(!out.status.success());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("no run `r9-9`"), "{err}");
    }
    let out = isolated::bin(env!("CARGO_BIN_EXE_mnemorph"))
        .arg("--resume")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("--resume needs"));
    let _ = std::fs::remove_dir_all(&world);
}
