### sandbox-escape
Invariant: a sandboxed command reaches only what its grant names: paths, network, processes, inherited fds.
Look: the grant builder (read and write roots, network, env passed through), each OS profile (Landlock and seccomp, Seatbelt), what runs outside the sandbox, child processes, fds left open across exec.
Check: for each root a grant opens, list what else lives under it; run a command one step past the grant (read, write, connect to a loopback listener you started, signal a child you spawned); compare the Linux and macOS profiles; follow symlinks inside a granted root that point out.
Witness: a test in the sandbox crate running the command and asserting it did what the grant forbids.
Not a finding: a path the grant names on purpose or the human granted; an escape that needs root already.
Regression: a broad read grant hands over credentials; that case is filed under credential-grant.
