### dependency-confusion
Invariant: every dependency resolves from the source and name it was meant to.
Look: git and path dependencies, `[patch]` and `[replace]`, registries in `.cargo/config.toml`, unscoped npm names, workspace globs, overrides, `replace` in `go.mod`, lockfile churn.
Check: does the lock pin a source and checksum for each? Does a new name shadow an internal one? Does a git dependency pin a rev, not a branch? Do CI and builds run `--locked` or `--frozen-lockfile`?
Witness: scanner output (`cargo deny check sources`) or the lock diff showing the changed source.
Not a finding: a public package under its public name, locked.
