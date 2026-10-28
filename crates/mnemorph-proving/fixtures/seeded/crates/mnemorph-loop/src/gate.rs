//! The gate's approval of a pull request: the human reads the body, then it is opened.

use std::io;
use std::path::Path;
use std::process::Command;

/// Shows the pull request's body to the human through `ask`; on a yes, opens it with that body.
pub fn approve_pr(body_file: &Path, ask: impl Fn(&str) -> bool) -> io::Result<bool> {
    let shown = std::fs::read_to_string(body_file)?;
    if !ask(&shown) {
        return Ok(false);
    }
    let status = Command::new("gh")
        .args(["pr", "create", "--fill-first", "--body-file"])
        .arg(body_file)
        .status()?;
    Ok(status.success())
}
