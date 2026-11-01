//! The human's state and config directories, resolved in one place — and refused to a test that
//! runs unisolated.
//!
//! A test binary resolves these from its environment like the harness does. Run without an
//! isolated home (plain `cargo test` on a developer's machine), that is the developer's real
//! `~/.local/state/mnemorph`, which a test must never write. So in a debug build, a test harness
//! process (a binary cargo keeps under `target/<profile>/deps`, run by cargo, which sets
//! `CARGO_MANIFEST_DIR`) panics here unless its environment carries [`ISOLATED`], which
//! `tools/test-env.sh`, nextest's `tools/test-home.sh` and the tests' isolated spawns set. A release
//! build never compiles the check; an installed binary never lives under `deps`.

use std::path::{Path, PathBuf};

/// The marker an isolated test environment carries.
pub const ISOLATED: &str = "MNEMORPH_ISOLATED";

/// The human's state directory: `$XDG_STATE_HOME` when absolute, else `~/.local/state`.
///
/// # Panics
///
/// In a debug build, in a test harness process not marked [`ISOLATED`].
#[must_use]
pub fn state_home() -> Option<PathBuf> {
    xdg("XDG_STATE_HOME", ".local/state")
}

/// The human's config directory: `$XDG_CONFIG_HOME` when absolute, else `~/.config`.
///
/// # Panics
///
/// As [`state_home`].
#[must_use]
pub fn config_home() -> Option<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config")
}

fn xdg(var: &str, under_home: &str) -> Option<PathBuf> {
    refuse_unisolated();
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(under_home)))
}

/// Panic when this is a test process running unisolated: see the module's doc.
fn refuse_unisolated() {
    #[cfg(debug_assertions)]
    if unisolated_test(
        std::env::current_exe().ok().as_deref(),
        std::env::var_os("CARGO_MANIFEST_DIR").is_some(),
        std::env::var_os(ISOLATED).is_some(),
    ) {
        panic!(
            "tests must run isolated: use `just test-isolated` (or `cargo nextest run` in the \
             workspace, or `tools/test-env.sh <command>`) — this test would read and write the \
             real state and config directories"
        );
    }
}

/// Whether a process at `exe`, run by cargo (`manifest`), unmarked (`!isolated`), is a test that
/// must not reach the human's directories.
#[must_use]
pub fn unisolated_test(exe: Option<&Path>, manifest: bool, isolated: bool) -> bool {
    let harness = exe
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .is_some_and(|d| d == "deps");
    harness && manifest && !isolated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_unmarked_test_harness_run_by_cargo_is_refused() {
        let test = Path::new("/w/target/debug/deps/escape-0123abcd");
        let app = Path::new("/w/target/debug/mnemorph");
        let installed = Path::new("/home/h/.local/bin/mnemorphd");
        assert!(unisolated_test(Some(test), true, false));
        assert!(!unisolated_test(Some(test), true, true), "marked isolated");
        assert!(
            !unisolated_test(Some(test), false, false),
            "not run by cargo"
        );
        assert!(!unisolated_test(Some(app), true, false), "`cargo run`");
        assert!(!unisolated_test(Some(installed), false, false));
        assert!(!unisolated_test(None, true, false));
    }
}
