### path-traversal
Invariant: a path confined to a root resolves inside it, however it is spelled.
Look: tool arguments naming files, `rec:` and witness paths, ledger evidence, archive entries, world-relative paths joined onto the world.
Check: trace each joined path to its open: `..`, absolute paths, `//`, `\`, NUL, non-ASCII lookalikes, a symlinked component. Does the guard run on the normalised form, before symlinks are followed, at every entry?
Witness: a test passing the crafted path and showing a read or write outside the root.
Not a finding: a path the human typed for themselves; a `..` refused before any I/O.
