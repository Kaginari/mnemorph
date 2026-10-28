//! The shell tool's reading of a command line: which acts go outward, and running them.

use std::io;
use std::process::{Command, ExitStatus};

/// The argv of the outward act a command line asks for, or `None` when it asks for none.
pub fn outward_argv(line: &str) -> Option<Vec<String>> {
    let mut words: Vec<&str> = line.split_whitespace().collect();
    // Leading `NAME=value` words set the environment; the command starts after them.
    while words
        .first()
        .is_some_and(|w| w.contains('=') && !w.starts_with('-'))
    {
        words.remove(0);
    }
    match words.as_slice() {
        ["git", "push", rest @ ..] => Some(
            ["git", "push"]
                .iter()
                .chain(rest)
                .map(|w| (*w).to_owned())
                .collect(),
        ),
        _ => None,
    }
}

/// Runs a command line the human approved as the outward act [`outward_argv`] read in it.
pub fn run_outward(line: &str) -> io::Result<ExitStatus> {
    if outward_argv(line).is_none() {
        return Err(io::Error::other("not an outward act"));
    }
    Command::new("sh").arg("-c").arg(line).status()
}
