//! Launching the sandbox helper for a world.

use std::io;
use std::process::{Child, Command};

/// Starts `helper` over the world whose root the world's own config names.
pub fn launch_helper(helper: &str, world_root: &str) -> io::Result<Child> {
    Command::new("sh")
        .arg("-c")
        .arg(format!("{helper} --root {world_root}"))
        .spawn()
}
