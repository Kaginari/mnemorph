//! The harness's binaries as a test spawns them: cut off from the human's own setup.
//!
//! A test that runs `mnemorph`, `mnemorphd` or `mnemorph-guard` on a developer's machine must not
//! read that developer's global config (a board that autostarts and opens, a provider's key
//! file), their state directory (a live board's control socket, its registry), or a binary they
//! installed on `PATH`. So the child's environment is cleared and made anew: a home of its own
//! under the temporary directory, every XDG directory inside it, and `PATH` without the
//! directories that hold an installed harness binary. A test then sets on top what it means to.
//!
//! Shared by every crate whose tests spawn a harness binary, through `#[path]`.
#![allow(dead_code)]

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// The variables a child keeps from the test's own environment: none is the human's setup.
/// `LLVM_PROFILE_FILE` keeps coverage writing where `cargo llvm-cov` reads it; `TMPDIR` keeps the
/// child's temporary directory the test's.
const KEPT: [&str; 2] = ["LLVM_PROFILE_FILE", "TMPDIR"];

/// The harness's binaries: a directory holding one is an installed harness, never on the path.
const HARNESS: [&str; 3] = ["mnemorph", "mnemorphd", "mnemorph-guard"];

/// The home every isolated child runs under: one per test process, made fresh under an
/// unpredictable name, `0700` like the human's state directory. It lives as long as the process:
/// under `tools/test-env.sh` the run's temporary directory, and it with it, is removed at the end.
#[must_use]
pub fn home() -> &'static Path {
    static HOME: OnceLock<tempfile::TempDir> = OnceLock::new();
    HOME.get_or_init(|| {
        let home = tempfile::Builder::new()
            .prefix("mnemorph-isolated-")
            .tempdir()
            .unwrap();
        for dir in [".config", ".local/state", ".local/share", ".cache", "run"] {
            std::fs::create_dir_all(home.path().join(dir)).unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            for dir in ["", ".local/state", "run"] {
                std::fs::set_permissions(
                    home.path().join(dir),
                    std::fs::Permissions::from_mode(0o700),
                )
                .unwrap();
            }
        }
        home
    })
    .path()
}

/// `PATH`, less every directory that holds an installed harness binary.
#[must_use]
pub fn path() -> OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let kept = std::env::split_paths(&path)
        .filter(|dir| !HARNESS.iter().any(|b| dir.join(b).is_file()))
        .collect::<Vec<_>>();
    std::env::join_paths(kept).unwrap_or_default()
}

/// `cmd`, its environment cleared and made anew under [`home`]: `HOME`, every XDG directory and
/// [`path`]. Call it first: what the test sets after it stands.
pub fn isolated(cmd: &mut Command) -> &mut Command {
    let home = home();
    cmd.env_clear()
        .env("PATH", path())
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("XDG_RUNTIME_DIR", home.join("run"))
        .env("MNEMORPH_ISOLATED", "1")
        .env("LANG", "C");
    for name in KEPT {
        if let Some(v) = std::env::var_os(name) {
            cmd.env(name, v);
        }
    }
    cmd
}

/// The program at `program`, isolated.
#[must_use]
pub fn bin(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    isolated(&mut cmd);
    cmd
}
