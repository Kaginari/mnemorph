### memory-safety
Invariant: no `unsafe` block or FFI call breaks Rust's guarantees, on any input.
Look: `unsafe`, `libc` and `rustix` calls (Landlock, seccomp, prctl, openat2), raw fds, `from_raw_parts`, `transmute`, C string conversions, code between fork and exec.
Check: state each block's contract and find the input that breaks it; fd ownership (double close, use after close); errno paths; async-signal safety after fork; run Miri or a sanitizer where it applies.
Witness: a crashing test, or a sanitizer or Miri report saved as scanner output.
Not a finding: `unsafe` whose stated contract holds; a panic in safe code (that is another class).
