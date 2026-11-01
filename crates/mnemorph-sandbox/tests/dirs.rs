//! A test process that runs unisolated is refused the human's state and config directories: this
//! binary, re-run without the isolation marker, panics where it would resolve them; with it, it
//! proceeds.

use std::process::Command;

use mnemorph_sandbox::dirs::{ISOLATED, config_home, state_home};

/// Resolves both directories: what the re-run below asks of this binary.
#[test]
fn resolves_the_directories() {
    let _ = state_home();
    let _ = config_home();
}

fn rerun(marked: bool) -> (bool, String) {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", "resolves_the_directories", "--test-threads", "1"])
        // Never the human's, whichever way it goes.
        .env(
            "XDG_STATE_HOME",
            std::env::temp_dir().join("mnemorph-dirs-unused/state"),
        )
        .env(
            "XDG_CONFIG_HOME",
            std::env::temp_dir().join("mnemorph-dirs-unused/config"),
        )
        .env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR"));
    if marked {
        c.env(ISOLATED, "1");
    } else {
        c.env_remove(ISOLATED);
    }
    let out = c.output().unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[cfg(debug_assertions)]
#[test]
fn an_unmarked_test_process_is_refused_and_a_marked_one_proceeds() {
    let (ok, said) = rerun(false);
    assert!(!ok, "{said}");
    assert!(
        said.contains("tests must run isolated: use `just test-isolated`"),
        "{said}"
    );
    let (ok, said) = rerun(true);
    assert!(ok, "{said}");
    assert!(said.contains("1 passed"), "{said}");
}
