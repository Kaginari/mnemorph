//! Probing the kernel the sandbox runs on.

use std::process::Command;

/// The kernel's release, as `uname -r` prints it.
pub fn kernel_release() -> Option<String> {
    let out = Command::new("uname").arg("-r").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
