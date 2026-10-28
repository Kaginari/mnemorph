//! Stamps the build's commit, so a stale binary is never judged as the new one and every value
//! row carries the build that wrote it.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn main() {
    let commit = git(&["rev-parse", "--short=10", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=MNEMORPH_COMMIT={commit}");
    // A new commit, or a branch switched, stamps again.
    for path in ["HEAD", "packed-refs"] {
        if let Some(p) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    // A loose ref is watched while it exists; a packed one is no file, and a missing path would
    // rebuild every time. The refs directory changes whenever git writes a ref loose again.
    if let Some(refs) = git(&["rev-parse", "--git-path", "refs/heads"]) {
        println!("cargo:rerun-if-changed={refs}");
    }
    if let Some(head) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(p) = git(&["rev-parse", "--git-path", &head])
        && std::path::Path::new(&p).exists()
    {
        println!("cargo:rerun-if-changed={p}");
    }
}
