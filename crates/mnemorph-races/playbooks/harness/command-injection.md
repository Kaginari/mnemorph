### command-injection
Invariant: untrusted text reaches a process as one argv element, never as shell syntax or an option.
Look: `sh -c`, command lines built with `format!`, arguments that may start with `-`, a missing `--` before paths, the act runner's git and gh calls, verify lines, scanners.
Check: trace model, repository or tool-output text to each spawn; try `;`, `$( )`, backticks, a newline, a leading `-` (`--upload-pack=`, `-o`), spaces in a name.
Witness: a test feeding the crafted value and observing an extra process, file or option take effect.
Not a finding: a bash command the body writes itself inside the sandbox (the classifier and the sandbox are that boundary); a constant argv.
