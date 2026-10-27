### release-integrity
Invariant: a release artifact is bound to the commit and build that claim it.
Look: release workflows and tag triggers, checksums and signatures, `rust-toolchain.toml`, `build.rs`, rustflags, linker and runner in `.cargo/config.toml`, install scripts.
Check: can a job other than the build upload an artifact? Are checksums made from the uploaded bytes? Can a fork's tag release? Does a build script reach the network?
Witness: scanner or lint output, or a test over the workflow file, showing the unbound path.
Not a finding: an unsigned prerelease labelled as one.
