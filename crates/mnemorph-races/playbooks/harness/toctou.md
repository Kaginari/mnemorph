### toctou
Invariant: the file a check approves is the file the use touches; nothing is swapped between.
Look: harness-side writes, deletes, renames and reads by path after a check (exists, canonicalize, inside-root, metadata); temp files; the edit and write tools; proving-ground cleanup.
Check: find each path resolved twice; can a sandboxed command plant or swap a symlink or directory in that window? The fix walks a pinned directory fd: `openat2` with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`, `O_NOFOLLOW`, `unlinkat` on the dirfd.
Witness: a test that swaps a path component for a symlink between check and use and shows the write or delete landing outside.
Not a finding: a window on a path no body can write; a race a user only wins against their own shell.
Regression: symlink swaps under harness-side writes and deletes; every such path goes through a pinned dirfd, never a re-resolved string.
